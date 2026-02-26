/*
 * JNI helper for launching hev-socks5-tunnel with an inherited TUN file descriptor.
 *
 * Android's Runtime.exec() closes all non-standard FDs in the child process before exec().
 * This JNI helper uses fork()/exec() directly, preserving the TUN FD so that
 * hev-socks5-tunnel can use it via the "fd:" config parameter.
 */

#include <jni.h>
#include <unistd.h>
#include <fcntl.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <errno.h>

JNIEXPORT jint JNICALL
Java_com_rustvpn_vpn_TunHelper_nativeStartWithTunFd(
    JNIEnv *env, jclass clz,
    jstring jExePath, jstring jConfigPath, jint tunFd)
{
    const char *exePath = (*env)->GetStringUTFChars(env, jExePath, NULL);
    const char *configPath = (*env)->GetStringUTFChars(env, jConfigPath, NULL);

    if (!exePath || !configPath) {
        if (exePath) (*env)->ReleaseStringUTFChars(env, jExePath, exePath);
        if (configPath) (*env)->ReleaseStringUTFChars(env, jConfigPath, configPath);
        return -1;
    }

    /* Copy strings before fork (JNI strings may not survive fork) */
    char exe_buf[512];
    char cfg_buf[512];
    strncpy(exe_buf, exePath, sizeof(exe_buf) - 1);
    exe_buf[sizeof(exe_buf) - 1] = '\0';
    strncpy(cfg_buf, configPath, sizeof(cfg_buf) - 1);
    cfg_buf[sizeof(cfg_buf) - 1] = '\0';

    (*env)->ReleaseStringUTFChars(env, jExePath, exePath);
    (*env)->ReleaseStringUTFChars(env, jConfigPath, configPath);

    pid_t pid = fork();
    if (pid < 0) {
        return -1;
    }

    if (pid == 0) {
        /* ---- Child process ---- */

        /* Clear CLOEXEC on the TUN FD so it survives exec() */
        int flags = fcntl(tunFd, F_GETFD);
        if (flags >= 0) {
            fcntl(tunFd, F_SETFD, flags & ~FD_CLOEXEC);
        }

        /* Close all FDs except stdin(0), stdout(1), stderr(2), and the TUN FD */
        long maxfd = sysconf(_SC_OPEN_MAX);
        if (maxfd < 0) maxfd = 1024;
        for (int i = 3; i < maxfd; i++) {
            if (i != tunFd) {
                close(i);
            }
        }

        /* Redirect stdin from /dev/null */
        int devnull = open("/dev/null", O_RDWR);
        if (devnull >= 0 && devnull != 0) {
            dup2(devnull, 0);
            if (devnull != tunFd) close(devnull);
        }

        /* Execute hev-socks5-tunnel */
        char *argv[] = { exe_buf, cfg_buf, NULL };
        execv(exe_buf, argv);

        /* If exec fails, exit */
        _exit(127);
    }

    /* Parent process — return the child PID */
    return pid;
}

JNIEXPORT jint JNICALL
Java_com_rustvpn_vpn_TunHelper_nativeKillProcess(
    JNIEnv *env, jclass clz, jint pid)
{
    if (pid <= 0) return -1;

    if (kill(pid, SIGTERM) == 0) {
        /* Give it a moment to exit gracefully */
        int status;
        int ret;
        for (int i = 0; i < 10; i++) {
            ret = waitpid(pid, &status, WNOHANG);
            if (ret != 0) break;
            usleep(100000); /* 100ms */
        }
        /* Force kill if still running */
        if (waitpid(pid, &status, WNOHANG) == 0) {
            kill(pid, SIGKILL);
            waitpid(pid, &status, 0);
        }
        return 0;
    }
    return -1;
}
