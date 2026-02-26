package com.rustvpn.vpn

/**
 * JNI helper for launching hev-socks5-tunnel with an inherited TUN file descriptor.
 *
 * Android's Runtime.exec() closes all non-standard FDs in the child process.
 * This helper uses native fork()/exec() to preserve the TUN FD.
 */
object TunHelper {
    init {
        System.loadLibrary("tunhelper")
    }

    /**
     * Fork and exec hev-socks5-tunnel, preserving the TUN FD in the child process.
     * @return child PID, or -1 on error
     */
    @JvmStatic
    external fun nativeStartWithTunFd(exePath: String, configPath: String, tunFd: Int): Int

    /**
     * Kill a child process by PID (SIGTERM, then SIGKILL if needed).
     * @return 0 on success, -1 on error
     */
    @JvmStatic
    external fun nativeKillProcess(pid: Int): Int
}
