package com.rustvpn.vpn

/**
 * JNI wrapper for hev-socks5-tunnel shared library.
 *
 * Instead of fork/exec (which silently fails because the pre-built binary
 * is a Linux/glibc binary incompatible with Android's bionic libc), we use
 * dlopen() to load the NDK-compiled libhev.so and run it in a pthread.
 */
object HevTunnel {
    init {
        System.loadLibrary("hevjni")
    }

    /**
     * Load libhev.so via dlopen and start the tunnel in a background thread.
     * @param libPath Absolute path to libhev.so (from applicationInfo.nativeLibraryDir)
     * @param configPath Path to the hev YAML config file
     * @param tunFd The TUN file descriptor
     * @return true if started successfully, false on error
     */
    @JvmStatic
    external fun nativeStart(libPath: String, configPath: String, tunFd: Int): Boolean

    /**
     * Gracefully stop the tunnel by calling hev_socks5_tunnel_quit().
     */
    @JvmStatic
    external fun nativeStop()

    /**
     * Check if the tunnel thread is still running.
     */
    @JvmStatic
    external fun nativeIsRunning(): Boolean
}
