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
    }

    private var tunFd: ParcelFileDescriptor? = null
    private var xrayProcess: Process? = null
    private var tun2socksProcess: Process? = null

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

            // 4. Create TUN interface
            val builder = Builder()
                .setSession("RustVPN")
                .addAddress("10.0.0.2", 30)
                .addRoute("0.0.0.0", 0)
                .addDnsServer("1.1.1.1")
                .addDnsServer("8.8.8.8")
                .setMtu(1500)
                .setBlocking(true)

            tunFd = builder.establish()
                ?: throw IllegalStateException("Failed to establish TUN interface")

            val fd = tunFd!!.fd
            Log.i(TAG, "TUN interface established with fd=$fd")

            // 5. Start tun2socks (hev-socks5-tunnel)
            val hevPath = "$nativeLibDir/libhev.so"
            val hevConfigFile = File(filesDir, "hev_config.yml")
            writeHevConfig(hevConfigFile, fd, socksPort)

            val hevCmd = arrayOf(hevPath, hevConfigFile.absolutePath)
            Log.i(TAG, "Starting tun2socks: ${hevCmd.joinToString(" ")}")
            tun2socksProcess = Runtime.getRuntime().exec(hevCmd)

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
        try {
            tun2socksProcess?.destroy()
            tun2socksProcess = null
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
              name: tun0
              mtu: 1500
              multi-queue: false
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
