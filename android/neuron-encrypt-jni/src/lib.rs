use std::fs::File;
use std::io::{self, Read, Seek};
use std::os::fd::{FromRawFd, RawFd};
use std::panic::{self, AssertUnwindSafe};

use jni::JNIEnv;
use jni::objects::{GlobalRef, JByteArray, JClass, JObject};
use jni::sys::{jint, jlong};
use jni::JavaVM;

use neuron_encrypt_core::crypto::{
    decrypt_stream, encrypt_stream, ProgressReporter, ThrottledReporter,
};
use neuron_encrypt_core::error::CryptoError;
use zeroize::Zeroizing;

const EXCEPTION_CLASS: &str = "com/neuronencrypt/app/NeuronEncryptException";
const CANCELLED_IO_MESSAGE: &str = "operation cancelled";

struct CancelableReader {
    inner: File,
    cancel: GlobalRef,
    vm: JavaVM,
}

impl CancelableReader {
    fn is_cancelled(&self) -> bool {
        if let Ok(mut env) = self.vm.attach_current_thread() {
            if let Ok(res) = env.call_method(&self.cancel, "get", "()Z", &[]) {
                if let Ok(val) = res.z() {
                    return val;
                }
            }
        }
        false
    }
}

impl Read for CancelableReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                CANCELLED_IO_MESSAGE,
            ));
        }
        self.inner.read(buf)
    }
}

impl Seek for CancelableReader {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        if self.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                CANCELLED_IO_MESSAGE,
            ));
        }
        self.inner.seek(pos)
    }
}

struct JniReporter {
    listener: GlobalRef,
    vm: JavaVM,
}

impl ProgressReporter for JniReporter {
    fn report(&self, progress: f32, message: &str) {
        if let Ok(mut env) = self.vm.attach_current_thread() {
            if let Ok(java_msg) = env.new_string(message) {
                let args = [
                    jni::objects::JValue::from(progress),
                    jni::objects::JValue::from(&java_msg),
                ];
                let _ = env.call_method(
                    &self.listener,
                    "onProgress",
                    "(FLjava/lang/String;)V",
                    &args,
                );
            }
        }
    }
}

fn map_error(env: &mut JNIEnv, err: CryptoError) -> jint {
    let (code, message) = match err {
        CryptoError::Io(ref io_err) if io_err.to_string() == CANCELLED_IO_MESSAGE => {
            (1, "Cancelled".to_owned())
        }
        CryptoError::DecryptionFailed
        | CryptoError::InvalidMagic
        | CryptoError::UnsupportedVersion(_)
        | CryptoError::FileTooSmall => (3, "Wrong passphrase or corrupted file.".to_owned()),

        CryptoError::PassphraseTooShort(min) => {
            (2, format!("Passphrase must be at least {min} bytes."))
        }
        CryptoError::FileAlreadyExists(_)
        | CryptoError::InvalidDestination(_)
        | CryptoError::SourceAndDestinationSame(_)
        | CryptoError::NotAFile(_)
        | CryptoError::FileTooLarge { .. } => (2, "Invalid input or output file.".to_owned()),

        CryptoError::Io(_) => (1, "Storage I/O failed.".to_owned()),
        CryptoError::Argon2Failed(_)
        | CryptoError::HkdfFailed(_)
        | CryptoError::EncryptionFailed(_)
        | CryptoError::InvalidSaltLength { .. }
        | CryptoError::InvalidNonceLength { .. }
        | CryptoError::LegacyFileTooLarge => (1, "Cryptographic operation failed.".to_owned()),
    };

    let _ = env.throw_new(EXCEPTION_CLASS, message);
    code
}

fn zero_java_array(env: &mut JNIEnv, array: &JByteArray) {
    if let Ok(len) = env.get_array_length(array) {
        if len > 0 {
            let zeros = vec![0i8; len as usize];
            let _ = env.set_byte_array_region(array, 0, &zeros);
        }
    }
}

fn fail_with_exception(
    env: &mut JNIEnv,
    password: &JByteArray,
    code: jint,
    message: &str,
) -> jint {
    zero_java_array(env, password);
    let _ = env.throw_new(EXCEPTION_CLASS, message);
    code
}

fn native_encrypt_impl(
    env: &mut JNIEnv,
    input_fd: jint,
    output_fd: jint,
    password: &JByteArray,
    source_size: jlong,
    progress_listener: &JObject,
    cancel_flag: &JObject,
) -> jint {
    if input_fd < 0 || output_fd < 0 {
        return fail_with_exception(env, password, 2, "Invalid file descriptor.");
    }

    let vm = match env.get_java_vm() {
        Ok(v) => v,
        Err(_) => return fail_with_exception(env, password, 1, "Failed to access JVM."),
    };

    let cancel_ref = match env.new_global_ref(cancel_flag) {
        Ok(r) => r,
        Err(_) => {
            return fail_with_exception(env, password, 1, "Failed to retain cancellation flag.");
        }
    };

    let listener_ref = match env.new_global_ref(progress_listener) {
        Ok(r) => r,
        Err(_) => {
            return fail_with_exception(env, password, 1, "Failed to retain progress listener.");
        }
    };

    let password_bytes = match env.convert_byte_array(password) {
        Ok(b) => b,
        Err(_) => return fail_with_exception(env, password, 2, "Failed to read passphrase."),
    };
    let password_bytes = Zeroizing::new(password_bytes);

    let input_file = unsafe { File::from_raw_fd(input_fd as RawFd) };
    let mut output_file = unsafe { File::from_raw_fd(output_fd as RawFd) };

    let vm2 = match env.get_java_vm() {
        Ok(v) => v,
        Err(_) => return fail_with_exception(env, password, 1, "Failed to access JVM."),
    };

    let mut reader = CancelableReader {
        inner: input_file,
        cancel: cancel_ref,
        vm: vm2,
    };

    let reporter = JniReporter {
        listener: listener_ref,
        vm,
    };
    let throttled = ThrottledReporter::new(&reporter);

    let size_opt = if source_size >= 0 {
        Some(source_size as u64)
    } else {
        None
    };

    let run_result = encrypt_stream(
        &mut reader,
        &mut output_file,
        &password_bytes,
        size_opt,
        &throttled,
    );

    zero_java_array(env, password);

    match run_result {
        Ok(()) => 0,
        Err(e) => map_error(env, e),
    }
}

fn native_decrypt_impl(
    env: &mut JNIEnv,
    input_fd: jint,
    output_fd: jint,
    password: &JByteArray,
    total_size: jlong,
    progress_listener: &JObject,
    cancel_flag: &JObject,
) -> jint {
    if input_fd < 0 || output_fd < 0 {
        return fail_with_exception(env, password, 2, "Invalid file descriptor.");
    }

    let vm = match env.get_java_vm() {
        Ok(v) => v,
        Err(_) => return fail_with_exception(env, password, 1, "Failed to access JVM."),
    };

    let cancel_ref = match env.new_global_ref(cancel_flag) {
        Ok(r) => r,
        Err(_) => {
            return fail_with_exception(env, password, 1, "Failed to retain cancellation flag.");
        }
    };

    let listener_ref = match env.new_global_ref(progress_listener) {
        Ok(r) => r,
        Err(_) => {
            return fail_with_exception(env, password, 1, "Failed to retain progress listener.");
        }
    };

    let password_bytes = match env.convert_byte_array(password) {
        Ok(b) => b,
        Err(_) => return fail_with_exception(env, password, 2, "Failed to read passphrase."),
    };
    let password_bytes = Zeroizing::new(password_bytes);

    let input_file = unsafe { File::from_raw_fd(input_fd as RawFd) };
    let mut output_file = unsafe { File::from_raw_fd(output_fd as RawFd) };

    let vm2 = match env.get_java_vm() {
        Ok(v) => v,
        Err(_) => return fail_with_exception(env, password, 1, "Failed to access JVM."),
    };

    let mut reader = CancelableReader {
        inner: input_file,
        cancel: cancel_ref,
        vm: vm2,
    };

    let reporter = JniReporter {
        listener: listener_ref,
        vm,
    };
    let throttled = ThrottledReporter::new(&reporter);

    let size_opt = if total_size >= 0 {
        Some(total_size as u64)
    } else {
        None
    };

    let run_result = decrypt_stream(
        &mut reader,
        &mut output_file,
        &password_bytes,
        size_opt,
        &throttled,
    );

    zero_java_array(env, password);

    match run_result {
        Ok(()) => 0,
        Err(e) => map_error(env, e),
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_neuronencrypt_app_NeuronEncryptNative_nativeEncrypt(
    mut env: JNIEnv,
    _class: JClass,
    input_fd: jint,
    output_fd: jint,
    password: JByteArray,
    source_size: jlong,
    progress_listener: JObject,
    cancel_flag: JObject,
) -> jint {
    let res = panic::catch_unwind(AssertUnwindSafe(|| {
        native_encrypt_impl(
            &mut env,
            input_fd,
            output_fd,
            &password,
            source_size,
            &progress_listener,
            &cancel_flag,
        )
    }));

    match res {
        Ok(code) => code,
        Err(_) => {
            zero_java_array(&mut env, &password);
            let _ = env.throw_new(EXCEPTION_CLASS, "Unexpected native failure.");
            1
        }
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_neuronencrypt_app_NeuronEncryptNative_nativeDecrypt(
    mut env: JNIEnv,
    _class: JClass,
    input_fd: jint,
    output_fd: jint,
    password: JByteArray,
    total_size: jlong,
    progress_listener: JObject,
    cancel_flag: JObject,
) -> jint {
    let res = panic::catch_unwind(AssertUnwindSafe(|| {
        native_decrypt_impl(
            &mut env,
            input_fd,
            output_fd,
            &password,
            total_size,
            &progress_listener,
            &cancel_flag,
        )
    }));

    match res {
        Ok(code) => code,
        Err(_) => {
            zero_java_array(&mut env, &password);
            let _ = env.throw_new(EXCEPTION_CLASS, "Unexpected native failure.");
            1
        }
    }
}
