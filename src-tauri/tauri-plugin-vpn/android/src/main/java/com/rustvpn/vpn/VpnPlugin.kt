package com.rustvpn.vpn

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class StartVpnArgs {
    lateinit var configJson: String
    var socksPort: Int = 10808
}

@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {

    @Command
    fun startVpn(invoke: Invoke) {
        val args = invoke.parseArgs(StartVpnArgs::class.java)

        // Store config for the service to pick up
        RustVpnService.pendingConfigJson = args.configJson
        RustVpnService.pendingSocksPort = args.socksPort

        val prepareIntent = VpnService.prepare(activity)
        if (prepareIntent != null) {
            // Need to request VPN permission — use Tauri's activity result API
            startActivityForResult(invoke, prepareIntent, "onVpnPermissionResult")
        } else {
            // Already have permission, start directly
            startService(invoke)
        }
    }

    @ActivityCallback
    private fun onVpnPermissionResult(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode == Activity.RESULT_OK) {
            startService(invoke)
        } else {
            invoke.reject("VPN permission denied by user")
        }
    }

    private fun startService(invoke: Invoke) {
        try {
            val intent = Intent(activity, RustVpnService::class.java).apply {
                action = RustVpnService.ACTION_START
            }
            activity.startForegroundService(intent)
            invoke.resolve(JSObject())
        } catch (e: Exception) {
            invoke.reject("Failed to start VPN service: ${e.message}")
        }
    }

    @Command
    fun stopVpn(invoke: Invoke) {
        try {
            val intent = Intent(activity, RustVpnService::class.java).apply {
                action = RustVpnService.ACTION_STOP
            }
            activity.startService(intent)
            invoke.resolve(JSObject())
        } catch (e: Exception) {
            invoke.reject("Failed to stop VPN service: ${e.message}")
        }
    }

    @Command
    fun getVpnStatus(invoke: Invoke) {
        val result = JSObject().apply {
            put("is_running", RustVpnService.isRunning)
            put("last_error", RustVpnService.lastError)
        }
        invoke.resolve(result)
    }

    @Command
    fun queryStats(invoke: Invoke) {
        try {
            val nativeLibDir = activity.applicationInfo.nativeLibraryDir
            val xrayPath = "$nativeLibDir/libxray.so"

            val process = Runtime.getRuntime().exec(
                arrayOf(xrayPath, "api", "statsquery", "-s", "127.0.0.1:10085")
            )
            val output = process.inputStream.bufferedReader().readText()
            process.waitFor()

            // Parse the JSON output to extract upload/download
            var upload = 0L
            var download = 0L
            try {
                val json = org.json.JSONObject(output)
                val stats = json.optJSONArray("stat")
                if (stats != null) {
                    for (i in 0 until stats.length()) {
                        val entry = stats.getJSONObject(i)
                        val name = entry.optString("name", "")
                        val value = entry.optLong("value", 0)
                        when (name) {
                            "outbound>>>proxy>>>traffic>>>uplink" -> upload = value
                            "outbound>>>proxy>>>traffic>>>downlink" -> download = value
                        }
                    }
                }
            } catch (_: Exception) {
                // Stats parsing failed, return zeros
            }

            val result = JSObject().apply {
                put("upload", upload)
                put("download", download)
            }
            invoke.resolve(result)
        } catch (e: Exception) {
            invoke.reject("Failed to query stats: ${e.message}")
        }
    }
}
