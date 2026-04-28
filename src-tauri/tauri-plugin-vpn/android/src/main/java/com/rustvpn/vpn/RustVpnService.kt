package com.rustvpn.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.VpnService
import android.os.Build
import android.os.IBinder
import android.os.ParcelFileDescriptor
import android.util.Log
import go.Seq
import libv2ray.CoreCallbackHandler
import libv2ray.CoreController
import libv2ray.Libv2ray
import org.json.JSONObject
import java.io.File

// RustVpnService runs in the dedicated ":vpn" process (see AndroidManifest).
// Because of that, the activity-side VpnPlugin and this service cannot share
// statics — every interaction goes through the AIDL Stub returned from
// onBind(). All previous "companion object" pending* fields are gone; the
// VPN config travels via Intent extras instead, which lets the OS redeliver
// it on START_REDELIVER_INTENT after a low-memory restart.
class RustVpnService : VpnService() {

    companion object {
        const val TAG = "RustVpnService"
        const val ACTION_START = "com.rustvpn.vpn.START"
        const val ACTION_STOP = "com.rustvpn.vpn.STOP"
        const val EXTRA_CONFIG_JSON = "configJson"
        const val EXTRA_SOCKS_PORT = "socksPort"
        const val EXTRA_SERVER_ADDRESS = "serverAddress"
        const val NOTIFICATION_CHANNEL_ID = "rustvpn_vpn_channel"
        const val NOTIFICATION_ID = 1
    }

    // Per-instance mutable state. Lives in the :vpn process only — the activity
    // side reads it via AIDL (getStatusJson / getStatsJson).
    @Volatile private var isRunning = false
    @Volatile private var lastError: String? = null
    @Volatile private var xrayRunning = false
    @Volatile private var hevRunning = false
    @Volatile private var tunActive = false
    @Volatile private var xrayController: CoreController? = null
    private var tunFd: ParcelFileDescriptor? = null

    private val binder = object : IVpnService.Stub() {
        override fun startVpn(configJson: String, socksPort: Int, serverAddress: String) {
            // Re-enter via onStartCommand so the foreground promotion path is
            // identical whether the activity invoked us directly or the OS
            // redelivered the intent after a process restart.
            val intent = Intent(this@RustVpnService, RustVpnService::class.java).apply {
                action = ACTION_START
                putExtra(EXTRA_CONFIG_JSON, configJson)
                putExtra(EXTRA_SOCKS_PORT, socksPort)
                putExtra(EXTRA_SERVER_ADDRESS, serverAddress)
            }
            startForegroundService(intent)
        }

        override fun stopVpn() {
            this@RustVpnService.stopVpnInternal()
        }

        override fun getStatusJson(): String {
            // Re-check hev liveness on every poll — the tunnel thread can die
            // independently of our flag if the SOCKS upstream dropped.
            val hevLive = try { HevTunnel.nativeIsRunning() } catch (_: Throwable) { false }
            val obj = JSONObject().apply {
                put("is_running", isRunning)
                put("last_error", lastError)
                put("xray_running", xrayRunning)
                put("hev_running", hevLive)
                put("tun_active", tunActive)
            }
            return obj.toString()
        }

        override fun getStatsJson(): String {
            var upload = 0L
            var download = 0L
            try {
                val controller = xrayController
                if (controller != null) {
                    upload = controller.queryStats("proxy", "uplink")
                    download = controller.queryStats("proxy", "downlink")
                }
            } catch (e: Throwable) {
                Log.w(TAG, "queryStats failed", e)
            }
            return JSONObject().apply {
                put("upload", upload)
                put("download", download)
            }.toString()
        }
    }

    override fun onBind(intent: Intent?): IBinder? {
        // The system VPN framework binds with action="android.net.VpnService" —
        // for that path we return the default VpnService binder so VpnService.prepare
        // and the system VPN dialog continue to work. For our own AIDL bind
        // (no action set on the Intent), return our IVpnService.Stub so the
        // activity can issue startVpn / stopVpn / getStatus over IPC.
        return if (intent?.action == SERVICE_INTERFACE) {
            super.onBind(intent)
        } else {
            binder
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                val configJson = intent.getStringExtra(EXTRA_CONFIG_JSON)
                val socksPort = intent.getIntExtra(EXTRA_SOCKS_PORT, 10808)
                val serverAddress = intent.getStringExtra(EXTRA_SERVER_ADDRESS) ?: ""
                if (configJson.isNullOrBlank()) {
                    // Either a buggy caller or START_REDELIVER_INTENT redelivered
                    // a stripped intent. Don't enter foreground without a config —
                    // we'd just sit there holding a notification with nothing
                    // running. Stop so the OS releases us cleanly.
                    Log.w(TAG, "ACTION_START with no config; stopping")
                    stopSelf(startId)
                    return START_NOT_STICKY
                }

                // Foreground promotion must happen on the main thread within ~5s
                // of startForegroundService, so do it before any heavy work.
                createNotificationChannel()
                startInForeground()
                Thread { startVpn(configJson, socksPort, serverAddress) }.start()
            }
            ACTION_STOP -> stopVpnInternal()
        }
        // START_REDELIVER_INTENT (vs START_STICKY) makes the OS hand us back the
        // original intent — with the config extras intact — if it has to recreate
        // the service after a low-memory kill. With START_STICKY the redelivered
        // intent would be null and we'd be unable to resume the tunnel.
        return START_REDELIVER_INTENT
    }

    private fun startInForeground() {
        val notification = buildNotification()
        // FOREGROUND_SERVICE_TYPE_SPECIAL_USE is only defined from API 34
        // (UPSIDE_DOWN_CAKE). On API 34+ targeting apps, the type must match
        // the manifest declaration or Android throws a SecurityException.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Activity task swiped from recents. With android:process=":vpn" the
        // activity's process death no longer drags us with it, but stock Android
        // still calls onTaskRemoved on bound services from the same package.
        // Re-assert foreground state defensively in case the system tries to
        // tear us down.
        Log.i(TAG, "onTaskRemoved: keeping VPN alive in :vpn process")
        if (isRunning) {
            startInForeground()
        }
        super.onTaskRemoved(rootIntent)
    }

    private fun startVpn(configJson: String, socksPort: Int, serverAddress: String) {
        try {
            lastError = null
            isRunning = false
            xrayRunning = false
            hevRunning = false
            tunActive = false

            val nativeLibDir = applicationInfo.nativeLibraryDir

            // Verify hev library exists (xray is now loaded via AAR, no binary needed)
            val hevLibPath = "$nativeLibDir/libhev.so"
            if (!File(hevLibPath).exists()) {
                throw IllegalStateException("hev library not found at $hevLibPath")
            }
            Log.i(TAG, "Verified hev library: $hevLibPath")

            // 1. Start xray in-process via libv2ray AAR (no child process)
            Log.i(TAG, "Initializing libv2ray AAR...")
            Seq.setContext(applicationContext)
            Libv2ray.initCoreEnv(filesDir.absolutePath, "")

            val handler = object : CoreCallbackHandler {
                override fun startup(): Long = 0
                override fun shutdown(): Long = 0
                override fun onEmitStatus(p0: Long, p1: String?): Long {
                    Log.d(TAG, "xray status: $p0 $p1")
                    return 0
                }
            }
            val controller = Libv2ray.newCoreController(handler)
            // tunFd=0 tells xray not to use built-in TUN; xray only opens SOCKS5 on port $socksPort.
            // hev-socks5-tunnel bridges TUN→SOCKS5 separately.
            Log.i(TAG, "Starting xray in-process via libv2ray AAR... server=$serverAddress")
            controller.startLoop(configJson, 0)
            xrayController = controller
            Log.i(TAG, "xray started in-process via libv2ray AAR")

            // 2. Wait for SOCKS5 port to be ready
            waitForPort(socksPort, timeoutMs = 10000)
            Log.i(TAG, "xray SOCKS5 port $socksPort is ready")
            xrayRunning = true

            // 3. Create TUN interface
            val builder = Builder()
                .setSession("RustVPN")
                .addAddress("10.0.0.2", 30)
                .addRoute("0.0.0.0", 0)
                .addRoute("::", 0)
                .addDnsServer("1.1.1.1")
                .addDnsServer("8.8.8.8")
                .setMtu(1500)
                .setBlocking(true)

            try {
                builder.addDisallowedApplication(packageName)
                Log.i(TAG, "Excluded own package from VPN routing: $packageName")
            } catch (e: Exception) {
                Log.w(TAG, "Failed to exclude own package: ${e.message}")
            }

            tunFd = builder.establish()
                ?: throw IllegalStateException("Failed to establish TUN interface")

            val fd = tunFd!!.fd
            Log.i(TAG, "TUN interface established with fd=$fd")
            tunActive = true

            // 4. Start tun2socks via JNI dlopen (not fork/exec)
            val hevConfigFile = File(filesDir, "hev_config.yml")
            writeHevConfig(hevConfigFile, fd, socksPort)

            Log.i(TAG, "Starting tun2socks via JNI dlopen: $hevLibPath (tunFd=$fd)")
            val started = HevTunnel.nativeStart(hevLibPath, hevConfigFile.absolutePath, fd)
            if (!started) {
                throw IllegalStateException("Failed to start tun2socks (HevTunnel.nativeStart returned false)")
            }

            // 5. Verify hev is actually running
            Thread.sleep(200)
            if (!HevTunnel.nativeIsRunning()) {
                throw IllegalStateException("tun2socks started but exited immediately")
            }
            hevRunning = true
            Log.i(TAG, "tun2socks started and verified running")

            isRunning = true
            Log.i(TAG, "VPN started successfully (xray=OK, hev=OK, tun=OK)")

        } catch (e: Exception) {
            Log.e(TAG, "Failed to start VPN", e)
            lastError = e.message
            isRunning = false
            xrayRunning = false
            hevRunning = false
            tunActive = false
            cleanup()
            stopSelf()
        }
    }

    private fun stopVpnInternal() {
        Log.i(TAG, "Stopping VPN")
        cleanup()
        isRunning = false
        xrayRunning = false
        hevRunning = false
        tunActive = false
        lastError = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun cleanup() {
        // Stop hev-socks5-tunnel via JNI
        try {
            if (HevTunnel.nativeIsRunning()) {
                Log.i(TAG, "Stopping tun2socks via HevTunnel.nativeStop()")
                HevTunnel.nativeStop()
            }
        } catch (e: Exception) {
            Log.w(TAG, "Error stopping tun2socks", e)
        }

        try {
            xrayController?.stopLoop()
            xrayController = null
            Log.i(TAG, "xray stopped via libv2ray AAR")
        } catch (e: Exception) {
            Log.w(TAG, "Error stopping xray", e)
        }
        try {
            tunFd?.close()
            tunFd = null
        } catch (e: Exception) {
            Log.w(TAG, "Error closing TUN fd", e)
        }
    }

    override fun onRevoke() {
        Log.i(TAG, "VPN permission revoked")
        stopVpnInternal()
        super.onRevoke()
    }

    override fun onDestroy() {
        cleanup()
        isRunning = false
        xrayRunning = false
        hevRunning = false
        tunActive = false
        super.onDestroy()
    }

    private fun waitForPort(port: Int, timeoutMs: Long) {
        val start = System.currentTimeMillis()
        while (System.currentTimeMillis() - start < timeoutMs) {
            try {
                java.net.Socket("127.0.0.1", port).use { return }
            } catch (_: Exception) {
                Thread.sleep(200)
            }
        }
        throw IllegalStateException("Timeout waiting for port $port")
    }

    private fun writeHevConfig(file: File, tunFd: Int, socksPort: Int) {
        val config = """
            tunnel:
              fd: $tunFd
              mtu: 1500
              ipv4: 10.0.0.2

            socks5:
              port: $socksPort
              address: 127.0.0.1
              udp: udp

            misc:
              task-stack-size: 81920
              log-level: info
        """.trimIndent()
        java.io.FileOutputStream(file).use { it.write(config.toByteArray()) }
    }

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            NOTIFICATION_CHANNEL_ID,
            "RustVPN Connection",
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "Shows when VPN is active"
        }
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(channel)
    }

    private fun buildNotification(): Notification {
        val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
        val pendingIntent = PendingIntent.getActivity(
            this, 0, launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        return Notification.Builder(this, NOTIFICATION_CHANNEL_ID)
            .setContentTitle("RustVPN")
            .setContentText("VPN is active")
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .build()
    }
}
