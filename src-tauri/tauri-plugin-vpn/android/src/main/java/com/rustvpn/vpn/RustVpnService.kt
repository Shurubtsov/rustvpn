package com.rustvpn.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.VpnService
import android.os.ParcelFileDescriptor
import android.util.Log
import java.io.File
import java.io.FileOutputStream

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

        @Volatile
        var xrayRunning = false

        @Volatile
        var hevRunning = false

        @Volatile
        var tunActive = false
    }

    private var tunFd: ParcelFileDescriptor? = null
    private var xrayProcess: Process? = null

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
            xrayRunning = false
            hevRunning = false
            tunActive = false

            createNotificationChannel()
            startForeground(NOTIFICATION_ID, buildNotification())

            val configJson = pendingConfigJson
                ?: throw IllegalStateException("No VPN config provided")
            val socksPort = pendingSocksPort

            val nativeLibDir = applicationInfo.nativeLibraryDir

            // Verify required binaries exist
            val xrayPath = "$nativeLibDir/libxray.so"
            val hevLibPath = "$nativeLibDir/libhev.so"

            if (!File(xrayPath).exists()) {
                throw IllegalStateException("xray binary not found at $xrayPath")
            }
            if (!File(hevLibPath).exists()) {
                throw IllegalStateException("hev library not found at $hevLibPath")
            }
            Log.i(TAG, "Verified binaries: xray=$xrayPath, hev=$hevLibPath")

            // 1. Write xray config to internal storage
            val configFile = File(filesDir, "xray_config.json")
            FileOutputStream(configFile).use { it.write(configJson.toByteArray()) }
            Log.i(TAG, "Wrote xray config to ${configFile.absolutePath}")

            // 2. Start xray, capturing stdout/stderr for error reporting
            val xrayCmd = arrayOf(xrayPath, "run", "-c", configFile.absolutePath)
            Log.i(TAG, "Starting xray: ${xrayCmd.joinToString(" ")}")
            val pb = ProcessBuilder(*xrayCmd)
                .redirectErrorStream(true)
            xrayProcess = pb.start()

            // Start a thread to read xray output for logging
            val process = xrayProcess!!
            Thread {
                try {
                    process.inputStream.bufferedReader().forEachLine { line ->
                        Log.i(TAG, "xray: $line")
                    }
                } catch (e: Exception) {
                    Log.w(TAG, "xray output reader stopped: ${e.message}")
                }
            }.apply {
                isDaemon = true
                start()
            }

            // 3. Wait for SOCKS5 port to be ready
            waitForPort(socksPort, timeoutMs = 10000)
            Log.i(TAG, "xray SOCKS5 port $socksPort is ready")
            xrayRunning = true

            // 4. Create TUN interface
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

            // 5. Start tun2socks via JNI dlopen (not fork/exec)
            val hevConfigFile = File(filesDir, "hev_config.yml")
            writeHevConfig(hevConfigFile, fd, socksPort)

            Log.i(TAG, "Starting tun2socks via JNI dlopen: $hevLibPath (tunFd=$fd)")
            val started = HevTunnel.nativeStart(hevLibPath, hevConfigFile.absolutePath, fd)
            if (!started) {
                throw IllegalStateException("Failed to start tun2socks (HevTunnel.nativeStart returned false)")
            }

            // 6. Verify hev is actually running
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

    private fun stopVpn() {
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
