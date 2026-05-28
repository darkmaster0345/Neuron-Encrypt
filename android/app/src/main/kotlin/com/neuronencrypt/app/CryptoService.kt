package com.neuronencrypt.app

import android.content.ContentResolver
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.concurrent.atomic.AtomicBoolean

suspend fun encrypt(
    contentResolver: ContentResolver,
    inputUri: Uri,
    outputUri: Uri,
    password: ByteArray,
    listener: ProgressListener,
    cancel: AtomicBoolean
): Int = withContext(Dispatchers.IO) {
    try {
        val inputPfd = contentResolver.openFileDescriptor(inputUri, "r")
            ?: throw NeuronEncryptException("Cannot open input file")
        inputPfd.use { input ->
            val outputPfd = contentResolver.openFileDescriptor(outputUri, "wt")
                ?: throw NeuronEncryptException("Cannot open output file")
            outputPfd.use { output ->
                val sourceSize = input.statSize

                // detachFd transfers ownership to native code; the JNI side closes them
                val inputFd = input.detachFd()
                val outputFd = output.detachFd()

                NeuronEncryptNative.nativeEncrypt(
                    inputFd, outputFd, password, sourceSize, listener, cancel
                )
            }
        }
    } finally {
        password.fill(0)
    }
}

suspend fun decrypt(
    contentResolver: ContentResolver,
    inputUri: Uri,
    outputUri: Uri,
    password: ByteArray,
    listener: ProgressListener,
    cancel: AtomicBoolean
): Int = withContext(Dispatchers.IO) {
    try {
        val inputPfd = contentResolver.openFileDescriptor(inputUri, "r")
            ?: throw NeuronEncryptException("Cannot open input file")
        inputPfd.use { input ->
            val outputPfd = contentResolver.openFileDescriptor(outputUri, "wt")
                ?: throw NeuronEncryptException("Cannot open output file")
            outputPfd.use { output ->
                val totalSize = input.statSize

                // detachFd transfers ownership to native code; the JNI side closes them
                val inputFd = input.detachFd()
                val outputFd = output.detachFd()

                NeuronEncryptNative.nativeDecrypt(
                    inputFd, outputFd, password, totalSize, listener, cancel
                )
            }
        }
    } finally {
        password.fill(0)
    }
}
