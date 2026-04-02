package com.rustvpn.vpn

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
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
    var serverAddress: String = ""
}

@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {

    @Command
    fun startVpn(invoke: Invoke) {
        val args = invoke.parseArgs(StartVpnArgs::class.java)

        // Store config for the service to pick up
        RustVpnService.pendingConfigJson = args.configJson
        RustVpnService.pendingSocksPort = args.socksPort
        RustVpnService.pendingServerAddress = args.serverAddress

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
        // Also check live hev state in case the thread died
        val hevLive = try { HevTunnel.nativeIsRunning() } catch (_: Exception) { false }

        val result = JSObject().apply {
            put("is_running", RustVpnService.isRunning)
            put("last_error", RustVpnService.lastError)
            put("xray_running", RustVpnService.xrayRunning)
            put("hev_running", hevLive)
            put("tun_active", RustVpnService.tunActive)
        }
        invoke.resolve(result)
    }

    @Command
    fun isCellularNetwork(invoke: Invoke) {
        try {
            val cm = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            val network = cm.activeNetwork
            val caps = network?.let { cm.getNetworkCapabilities(it) }
            val cellular = caps?.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) == true
            val result = JSObject().apply {
                put("cellular", cellular)
            }
            invoke.resolve(result)
        } catch (e: Exception) {
            invoke.reject("Network detection failed: ${e.message}")
        }
    }

    @Command
    fun queryStats(invoke: Invoke) {
        try {
            val controller = RustVpnService.xrayController
            var upload = 0L
            var download = 0L

            if (controller != null) {
                upload = controller.queryStats("proxy", "uplink")
                download = controller.queryStats("proxy", "downlink")
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
