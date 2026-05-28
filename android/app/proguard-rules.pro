# JNI reflective access
-keepclasseswithmembernames class * {
    native <methods>;
}

-keep class com.neuronencrypt.app.NeuronEncryptNative {
    native <methods>;
}

-keep interface com.neuronencrypt.app.ProgressListener {
    *;
}

-keep class com.neuronencrypt.app.NeuronEncryptException {
    *;
}
