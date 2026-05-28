package com.neuronencrypt.app

class NeuronEncryptException(message: String) : Exception(message)

object NeuronEncryptNative {
    init {
        System.loadLibrary("neuron_encrypt_jni")
    }

    external fun nativeEncrypt(
        inputFd: Int,
        outputFd: Int,
        password: ByteArray,
        sourceSize: Long,
        progressListener: ProgressListener,
        cancelFlag: java.util.concurrent.atomic.AtomicBoolean
    ): Int

    external fun nativeDecrypt(
        inputFd: Int,
        outputFd: Int,
        password: ByteArray,
        totalSize: Long,
        progressListener: ProgressListener,
        cancelFlag: java.util.concurrent.atomic.AtomicBoolean
    ): Int
}
