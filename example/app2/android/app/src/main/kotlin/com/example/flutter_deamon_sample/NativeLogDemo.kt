package com.example.flutter_deamon_sample

import android.util.Log
import java.util.Timer
import java.util.TimerTask

object NativeLogDemo {
    private const val TAG_DEMO = "NativeDemo"
    private const val TAG_PLUGIN = "MyPlugin"
    private const val TAG_GO = "GoLog"
    private const val TAG_NETWORK = "OkHttp"

    private var periodicTimer: Timer? = null
    private var logCounter = 0

    /**
     * Emit a burst of sample native logs across different tags and priorities.
     * These simulate what a real Flutter app with native plugins would produce.
     */
    fun emitSampleLogs() {
        // Simulate a native plugin initialization sequence
        Log.i(TAG_DEMO, "Native log demo triggered from Flutter")
        Log.d(TAG_PLUGIN, "Plugin initializing native components")
        Log.i(TAG_PLUGIN, "Plugin v2.1.0 loaded successfully")

        // Simulate Go/gomobile output (common in cross-platform Flutter apps)
        Log.d(TAG_GO, "Go runtime initialized, GOMAXPROCS=4")
        Log.i(TAG_GO, "gRPC client connected to backend:8443")
        Log.w(TAG_GO, "TLS certificate expires in 7 days")

        // Simulate network library output
        Log.d(TAG_NETWORK, "--> GET https://api.example.com/data")
        Log.d(TAG_NETWORK, "--> END GET")
        Log.d(TAG_NETWORK, "<-- 200 OK (45ms)")
        Log.d(TAG_NETWORK, "<-- END HTTP (1234-byte body)")

        // Simulate various priority levels
        Log.v(TAG_DEMO, "Verbose: detailed trace information")
        Log.d(TAG_DEMO, "Debug: diagnostic information")
        Log.i(TAG_DEMO, "Info: general information")
        Log.w(TAG_DEMO, "Warning: potential issue detected")
        Log.e(TAG_DEMO, "Error: something went wrong (simulated)")

        // Simulate a multi-line native log (stack trace style)
        Log.e(TAG_PLUGIN, "NullPointerException: Attempt to invoke virtual method on null reference")
        Log.e(TAG_PLUGIN, "  at com.example.plugin.DataManager.fetchData(DataManager.kt:42)")
        Log.e(TAG_PLUGIN, "  at com.example.plugin.PluginHandler.handleCall(PluginHandler.kt:87)")
    }

    /**
     * Start emitting periodic native logs (simulates ongoing plugin activity).
     */
    fun startPeriodicLogs(intervalMs: Long) {
        stopPeriodicLogs()
        logCounter = 0
        periodicTimer = Timer().apply {
            scheduleAtFixedRate(object : TimerTask() {
                override fun run() {
                    logCounter++
                    when (logCounter % 4) {
                        0 -> Log.i(TAG_GO, "Heartbeat #$logCounter: connection alive")
                        1 -> Log.d(TAG_NETWORK, "Request #$logCounter: GET /api/status -> 200 (${(10..200).random()}ms)")
                        2 -> Log.i(TAG_PLUGIN, "Event #$logCounter: sensor data received")
                        3 -> Log.d(TAG_DEMO, "Tick #$logCounter: background task running")
                    }
                }
            }, 0L, intervalMs)
        }
    }

    fun stopPeriodicLogs() {
        periodicTimer?.cancel()
        periodicTimer = null
    }
}
