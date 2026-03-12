package com.example.flutter_deamon_sample

import android.util.Log
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    private val CHANNEL = "com.example.flutter_deamon_sample/native_logs"

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)

        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "triggerNativeLogs" -> {
                        NativeLogDemo.emitSampleLogs()
                        result.success(null)
                    }
                    "startPeriodicLogs" -> {
                        val intervalMs = call.argument<Int>("intervalMs") ?: 2000
                        NativeLogDemo.startPeriodicLogs(intervalMs.toLong())
                        result.success(null)
                    }
                    "stopPeriodicLogs" -> {
                        NativeLogDemo.stopPeriodicLogs()
                        result.success(null)
                    }
                    else -> result.notImplemented()
                }
            }
    }
}
