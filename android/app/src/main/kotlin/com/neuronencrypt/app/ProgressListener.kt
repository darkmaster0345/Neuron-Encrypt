package com.neuronencrypt.app

interface ProgressListener {
    fun onProgress(fraction: Float, stage: String)
}
