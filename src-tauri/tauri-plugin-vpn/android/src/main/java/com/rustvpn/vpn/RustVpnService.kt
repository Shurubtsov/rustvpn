package com.rustvpn.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.net.InetAddress

class RustVpnService : VpnService() {

    companion object {
        const val TAG = "RustVpnService"
        const val ACTION_START = "com.rustvpn.vpn.START"
        const val ACTION_STOP = "com.rustvpn.vpn.STOP"
        const val NOTIFICATION_CHANNEL_ID = "rustvpn_vpn_channel"
        const val NOTIFICATION_ID = 1

        @Volatile
        var isRunning = false

        @Volatile
        var lastError: String? = null

        @Volatile
        var pendingConfigJson: String? = null

        @Volatile
        var pendingSocksPort: Int = 10808

        @Volatile
        var pendingServerAddress: String? = null
    }

    private var tunFd: ParcelFileDescriptor? = null
    private var xrayProcess: Process? = null
    private var hevPid: Int = -1

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startVpn()
            ACTION_STOP -> stopVpn()
        }
        return START_STICKY
    }

    private fun startVpn() {
        try {
            lastError = null
            isRunning = false

            createNotificationChannel()
            startForeground(NOTIFICATION_ID, buildNotification())

            val configJson = pendingConfigJson
                ?: throw IllegalStateException("No VPN config provided")
            val socksPort = pendingSocksPort
            val serverAddress = pendingServerAddress ?: ""

            // 1. Write xray config to internal storage
            val configFile = File(filesDir, "xray_config.json")
            FileOutputStream(configFile).use { it.write(configJson.toByteArray()) }
            Log.i(TAG, "Wrote xray config to ${configFile.absolutePath}")

            val nativeLibDir = applicationInfo.nativeLibraryDir

            // 2. Start xray
            val xrayPath = "$nativeLibDir/libxray.so"
            val xrayCmd = arrayOf(xrayPath, "run", "-c", configFile.absolutePath)
            Log.i(TAG, "Starting xray: ${xrayCmd.joinToString(" ")}")
            xrayProcess = Runtime.getRuntime().exec(xrayCmd)

            // 3. Wait for SOCKS5 port to be ready
            waitForPort(socksPort, timeoutMs = 10000)
            Log.i(TAG, "xray SOCKS5 port $socksPort is ready")

            // 4. Create TUN interface with proper routing
            val builder = Builder()
                .setSession("RustVPN")
                .addAddress("10.0.0.2", 30)
                .addRoute("0.0.0.0", 0)
                .addRoute("::", 0)
                .addDnsServer("1.1.1.1")
                .addDnsServer("8.8.8.8")
                .setMtu(1500)
                .setBlocking(true)

            // Exclude our own app from VPN routing.
            // This prevents a routing loop: xray's outbound traffic to the VPN server
            // must NOT go through the TUN (which would loop back to xray via hev).
            // Child processes (xray, hev) share our UID and are also excluded.
            // hev reads/writes the TUN FD directly, so the exclusion doesn't affect it.
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

            // 5. Start tun2socks (hev-socks5-tunnel) via JNI fork/exec
            // We use a JNI helper because Runtime.exec() closes all non-standard FDs
            // in the child process. The JNI fork/exec preserves the TUN FD so that
            // hev-socks5-tunnel can access it via the "fd:" config parameter.
            val hevPath = "$nativeLibDir/libhev.so"
            val hevConfigFile = File(filesDir, "hev_config.yml")
            writeHevConfig(hevConfigFile, fd, socksPort)

            Log.i(TAG, "Starting tun2socks via JNI fork: $hevPath (tunFd=$fd)")
            hevPid = TunHelper.nativeStartWithTunFd(hevPath, hevConfigFile.absolutePath, fd)
            if (hevPid <= 0) {
                throw IllegalStateException("Failed to start tun2socks (JNI fork returned $hevPid)")
            }
            Log.i(TAG, "tun2socks started with PID=$hevPid")

            isRunning = true
            Log.i(TAG, "VPN started successfully")

        } catch (e: Exception) {
            Log.e(TAG, "Failed to start VPN", e)
            lastError = e.message
            isRunning = false
            cleanup()
            stopSelf()
        }
    }

    private fun stopVpn() {
        Log.i(TAG, "Stopping VPN")
        cleanup()
        isRunning = false
        lastError = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun cleanup() {
        // Stop hev-socks5-tunnel via native kill
        if (hevPid > 0) {
            try {
                Log.i(TAG, "Killing tun2socks PID=$hevPid")
                TunHelper.nativeKillProcess(hevPid)
            } catch (e: Exception) {
                Log.w(TAG, "Error stopping tun2socks", e)
            }
            hevPid = -1
        }
        try {
            xrayProcess?.destroy()
            xrayProcess = null
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
        stopVpn()
        super.onRevoke()
    }

    override fun onDestroy() {
        cleanup()
        isRunning = false
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
        // Use "fd:" instead of "name:" so hev uses the inherited TUN file descriptor
        // directly rather than trying to open a TUN device by name (which requires root).
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
        FileOutputStream(file).use { it.write(config.toByteArray()) }
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
