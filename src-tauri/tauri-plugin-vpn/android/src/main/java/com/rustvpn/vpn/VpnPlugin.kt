package com.rustvpn.vpn

import android.annotation.SuppressLint
import android.app.Activity
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.net.Uri
import android.net.VpnService
import android.os.IBinder
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@InvokeArg
class StartVpnArgs {
    lateinit var configJson: String
    var socksPort: Int = 10808
    var serverAddress: String = ""
}

@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        private const val TAG = "VpnPlugin"
        // Bound-service IPC has to wait for ServiceConnection.onServiceConnected
        // (which is dispatched on the main thread) to give us the IBinder. Cap
        // every wait so a wedged service can never hang a Tauri command forever.
        private const val BIND_TIMEOUT_MS = 1500L
    }

    // AIDL handle to the running VPN service. Null means we are not bound to
    // anything — either the :vpn process never started, it died, or we have
    // not yet attempted to bind on this activity instance.
    @Volatile private var vpnService: IVpnService? = null

    private val bindLock = Object()
    private var bindLatch: CountDownLatch? = null

    // pendingArgs is only ever touched on the main thread: written in
    // startVpn() and read/cleared in onVpnPermissionResult(), both of which
    // are dispatched there by the Tauri plugin runtime. Don't move the read
    // into the worker thread used by startServiceAndBind without adding
    // synchronization first.
    private var pendingArgs: StartVpnArgs? = null

    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName, binder: IBinder) {
            synchronized(bindLock) {
                vpnService = IVpnService.Stub.asInterface(binder)
                bindLatch?.countDown()
            }
            Log.i(TAG, "Bound to RustVpnService (:vpn process)")
        }

        override fun onServiceDisconnected(name: ComponentName) {
            // Process crash on the service side. The Android binder will
            // re-deliver onServiceConnected once the framework re-attaches us
            // to the rebooted service; until then mark ourselves unbound.
            synchronized(bindLock) {
                vpnService = null
            }
            Log.w(TAG, "RustVpnService disconnected")
        }

        override fun onBindingDied(name: ComponentName) {
            synchronized(bindLock) {
                vpnService = null
                bindLatch?.countDown()
                bindLatch = null
            }
            try { activity.unbindService(this) } catch (_: Exception) {}
            Log.w(TAG, "Binding to RustVpnService died — will rebind on next call")
        }

        override fun onNullBinding(name: ComponentName) {
            // Should never happen — onBind always returns our Stub — but
            // bookkeeping matters.
            synchronized(bindLock) {
                bindLatch?.countDown()
                bindLatch = null
            }
            try { activity.unbindService(this) } catch (_: Exception) {}
        }
    }

    /**
     * Make sure we have an IVpnService binder, blocking up to [timeoutMs] for
     * an in-flight bind to complete. Returns null if the :vpn service is not
     * running or bindService refused. We deliberately do NOT pass
     * BIND_AUTO_CREATE: a getStatus poll must not spawn an empty :vpn process
     * (no config, no foreground promotion) just to be told nothing is running.
     */
    private fun ensureBound(timeoutMs: Long): IVpnService? {
        vpnService?.let { return it }

        val latch: CountDownLatch
        synchronized(bindLock) {
            vpnService?.let { return it }
            if (bindLatch == null) {
                bindLatch = CountDownLatch(1)
                val intent = Intent(activity, RustVpnService::class.java)
                val accepted = try {
                    activity.bindService(intent, connection, 0)
                } catch (e: Exception) {
                    Log.w(TAG, "bindService threw: ${e.message}")
                    false
                }
                if (!accepted) {
                    bindLatch?.countDown()
                    bindLatch = null
                    return null
                }
            }
            latch = bindLatch!!
        }

        latch.await(timeoutMs, TimeUnit.MILLISECONDS)
        return vpnService
    }

    @Command
    fun startVpn(invoke: Invoke) {
        val args = invoke.parseArgs(StartVpnArgs::class.java)

        val prepareIntent = VpnService.prepare(activity)
        if (prepareIntent != null) {
            // Permission dialog needed; cache args for the callback because
            // startActivityForResult re-routes the resolution into onVpnPermissionResult.
            pendingArgs = args
            startActivityForResult(invoke, prepareIntent, "onVpnPermissionResult")
        } else {
            startServiceAndBind(invoke, args)
        }
    }

    @ActivityCallback
    private fun onVpnPermissionResult(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode != Activity.RESULT_OK) {
            pendingArgs = null
            invoke.reject("VPN permission denied by user")
            return
        }
        val args = pendingArgs
        pendingArgs = null
        if (args == null) {
            invoke.reject("Missing VPN start args after permission grant")
            return
        }
        startServiceAndBind(invoke, args)
    }

    private fun startServiceAndBind(invoke: Invoke, args: StartVpnArgs) {
        // Run on a worker so the bind wait below cannot deadlock the main
        // thread (ServiceConnection callbacks are delivered on main).
        Thread {
            try {
                val intent = Intent(activity, RustVpnService::class.java).apply {
                    action = RustVpnService.ACTION_START
                    putExtra(RustVpnService.EXTRA_CONFIG_JSON, args.configJson)
                    putExtra(RustVpnService.EXTRA_SOCKS_PORT, args.socksPort)
                    putExtra(RustVpnService.EXTRA_SERVER_ADDRESS, args.serverAddress)
                }
                // startForegroundService spawns the :vpn process if needed and
                // delivers ACTION_START to onStartCommand. The service then
                // promotes itself to foreground within Android's 5s window.
                activity.startForegroundService(intent)
                // Bind so subsequent getStatus/queryStats calls have an AIDL
                // handle. Failure is non-fatal — startVpn already issued the
                // start intent, so the service will run regardless of whether
                // the bind succeeded yet.
                ensureBound(BIND_TIMEOUT_MS)
                invoke.resolve(JSObject())
            } catch (e: Exception) {
                invoke.reject("Failed to start VPN service: ${e.message}")
            }
        }.start()
    }

    @Command
    fun stopVpn(invoke: Invoke) {
        Thread {
            try {
                val service = ensureBound(BIND_TIMEOUT_MS)
                if (service != null) {
                    service.stopVpn()
                } else {
                    // No live binder — either the service is already gone or
                    // bind never completed. Send the stop intent as a fallback;
                    // if no service is running, this is a no-op.
                    val intent = Intent(activity, RustVpnService::class.java).apply {
                        action = RustVpnService.ACTION_STOP
                    }
                    try { activity.startService(intent) } catch (_: Exception) {}
                }
                invoke.resolve(JSObject())
            } catch (e: Exception) {
                invoke.reject("Failed to stop VPN service: ${e.message}")
            }
        }.start()
    }

    @Command
    fun getVpnStatus(invoke: Invoke) {
        Thread {
            try {
                val service = ensureBound(BIND_TIMEOUT_MS)
                val result = if (service != null) {
                    val obj = JSONObject(service.getStatusJson())
                    JSObject().apply {
                        put("is_running", obj.optBoolean("is_running"))
                        put(
                            "last_error",
                            if (obj.isNull("last_error")) null
                            else obj.optString("last_error", null)
                        )
                        put("xray_running", obj.optBoolean("xray_running"))
                        put("hev_running", obj.optBoolean("hev_running"))
                        put("tun_active", obj.optBoolean("tun_active"))
                    }
                } else {
                    // Service not running (or bind timed out). Tauri callers
                    // treat this as a "disconnected" snapshot.
                    JSObject().apply {
                        put("is_running", false)
                        put("last_error", null as String?)
                        put("xray_running", false)
                        put("hev_running", false)
                        put("tun_active", false)
                    }
                }
                invoke.resolve(result)
            } catch (e: Exception) {
                invoke.reject("Failed to get status: ${e.message}")
            }
        }.start()
    }

    @Command
    fun queryStats(invoke: Invoke) {
        Thread {
            try {
                val service = ensureBound(BIND_TIMEOUT_MS)
                val result = if (service != null) {
                    val obj = JSONObject(service.getStatsJson())
                    JSObject().apply {
                        put("upload", obj.optLong("upload"))
                        put("download", obj.optLong("download"))
                    }
                } else {
                    JSObject().apply {
                        put("upload", 0L)
                        put("download", 0L)
                    }
                }
                invoke.resolve(result)
            } catch (e: Exception) {
                invoke.reject("Failed to query stats: ${e.message}")
            }
        }.start()
    }

    @Command
    fun isBatteryOptimizationIgnored(invoke: Invoke) {
        val pm = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        val ignored = pm.isIgnoringBatteryOptimizations(activity.packageName)
        invoke.resolve(JSObject().put("ignored", ignored))
    }

    // BatteryLife lint flags ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS because
    // Play Store rejects most apps that use it. VPN apps are an explicitly
    // permitted exception in Google's policy, so suppress at the function level
    // — that way moving the intent construction out of this body in the future
    // doesn't silently re-trigger the warning at the new location.
    @SuppressLint("BatteryLife")
    @Command
    fun requestIgnoreBatteryOptimization(invoke: Invoke) {
        val pm = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        if (pm.isIgnoringBatteryOptimizations(activity.packageName)) {
            invoke.resolve(JSObject().put("granted", true))
            return
        }
        val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
            data = Uri.parse("package:${activity.packageName}")
        }
        try {
            startActivityForResult(invoke, intent, "onBatteryOptResult")
        } catch (e: Exception) {
            invoke.reject("Failed to request battery exemption: ${e.message}")
        }
    }

    @ActivityCallback
    private fun onBatteryOptResult(invoke: Invoke, result: ActivityResult) {
        // The exemption screen does not return a meaningful resultCode — query
        // PowerManager directly to see whether the user actually granted it.
        val pm = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        val granted = pm.isIgnoringBatteryOptimizations(activity.packageName)
        invoke.resolve(JSObject().put("granted", granted))
    }

    @Command
    fun openOemBackgroundSettings(invoke: Invoke) {
        // Each OEM hides "auto-launch" / "background activity" in a different
        // proprietary settings page; there is no public API. We try a curated
        // list of known component paths and stop at the first one that resolves.
        // Anything that throws ActivityNotFoundException just means this user
        // is on a different ROM — try the next candidate.
        val candidates = listOf(
            // Realme / Oppo (ColorOS)
            ComponentName(
                "com.coloros.safecenter",
                "com.coloros.safecenter.permission.startup.StartupAppListActivity"
            ),
            ComponentName(
                "com.coloros.safecenter",
                "com.coloros.safecenter.startupapp.StartupAppListActivity"
            ),
            ComponentName(
                "com.oppo.safe",
                "com.oppo.safe.permission.startup.StartupAppListActivity"
            ),
            // Xiaomi MIUI
            ComponentName(
                "com.miui.securitycenter",
                "com.miui.permcenter.autostart.AutoStartManagementActivity"
            ),
            // Huawei EMUI
            ComponentName(
                "com.huawei.systemmanager",
                "com.huawei.systemmanager.startupmgr.ui.StartupNormalAppListActivity"
            ),
            ComponentName(
                "com.huawei.systemmanager",
                "com.huawei.systemmanager.optimize.process.ProtectActivity"
            ),
            // Vivo Funtouch
            ComponentName(
                "com.iqoo.secure",
                "com.iqoo.secure.ui.phoneoptimize.AddWhiteListActivity"
            ),
            // Samsung One UI
            ComponentName(
                "com.samsung.android.lool",
                "com.samsung.android.sm.ui.battery.BatteryActivity"
            ),
        )
        for (component in candidates) {
            val intent = Intent().apply {
                this.component = component
                flags = Intent.FLAG_ACTIVITY_NEW_TASK
            }
            try {
                activity.startActivity(intent)
                invoke.resolve(JSObject().put("opened", true).put("fallback", false))
                return
            } catch (_: Exception) {
                // try next
            }
        }
        // No OEM-specific page matched — fall back to the generic application
        // details screen so the user can at least find battery / autostart
        // toggles manually from there.
        try {
            val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.parse("package:${activity.packageName}")
                flags = Intent.FLAG_ACTIVITY_NEW_TASK
            }
            activity.startActivity(intent)
            invoke.resolve(JSObject().put("opened", true).put("fallback", true))
        } catch (e: Exception) {
            invoke.resolve(JSObject().put("opened", false).put("fallback", true))
        }
    }
}
