/*
 * JNI wrapper for hev-socks5-tunnel shared library.
 *
 * Instead of fork/exec (which fails because the pre-built binary is a Linux/glibc
 * binary incompatible with Android's bionic libc), we load the NDK-compiled
 * libhev.so at runtime via dlopen() and call its API directly in a pthread.
 *
 * This is the same approach used by v2rayNG and other Android VPN clients.
 */

#include <jni.h>
#include <dlfcn.h>
#include <pthread.h>
#include <string.h>
#include <stdlib.h>
#include <android/log.h>

#define TAG "HevJNI"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, TAG, __VA_ARGS__)

/* Function pointer types matching hev-socks5-tunnel's API */
typedef int (*hev_main_from_file_func)(const char *config_path);
typedef void (*hev_quit_func)(void);

/* State */
static void *hev_lib_handle = NULL;
static hev_main_from_file_func hev_main_from_file = NULL;
static hev_quit_func hev_quit = NULL;
static pthread_t hev_thread;
static volatile int hev_running = 0;

/* Thread argument */
struct hev_thread_args {
    char config_path[512];
};

static void *hev_thread_func(void *arg) {
    struct hev_thread_args *args = (struct hev_thread_args *)arg;

    LOGI("hev thread started, config=%s", args->config_path);

    int ret = hev_main_from_file(args->config_path);

    LOGI("hev_socks5_tunnel_main_from_file returned %d", ret);

    free(args);
    hev_running = 0;
    return NULL;
}

JNIEXPORT jboolean JNICALL
Java_com_rustvpn_vpn_HevTunnel_nativeStart(
    JNIEnv *env, jclass clz,
    jstring jLibPath, jstring jConfigPath, jint tunFd)
{
    if (hev_running) {
        LOGE("hev tunnel already running");
        return JNI_FALSE;
    }

    const char *libPath = (*env)->GetStringUTFChars(env, jLibPath, NULL);
    const char *configPath = (*env)->GetStringUTFChars(env, jConfigPath, NULL);

    if (!libPath || !configPath) {
        if (libPath) (*env)->ReleaseStringUTFChars(env, jLibPath, libPath);
        if (configPath) (*env)->ReleaseStringUTFChars(env, jConfigPath, configPath);
        LOGE("Failed to get string params");
        return JNI_FALSE;
    }

    /* Load the hev shared library */
    if (hev_lib_handle) {
        dlclose(hev_lib_handle);
        hev_lib_handle = NULL;
    }

    hev_lib_handle = dlopen(libPath, RTLD_NOW);
    if (!hev_lib_handle) {
        LOGE("dlopen(%s) failed: %s", libPath, dlerror());
        (*env)->ReleaseStringUTFChars(env, jLibPath, libPath);
        (*env)->ReleaseStringUTFChars(env, jConfigPath, configPath);
        return JNI_FALSE;
    }
    LOGI("Loaded hev library from %s", libPath);

    /* Resolve symbols */
    hev_main_from_file = (hev_main_from_file_func)dlsym(hev_lib_handle, "hev_socks5_tunnel_main_from_file");
    hev_quit = (hev_quit_func)dlsym(hev_lib_handle, "hev_socks5_tunnel_quit");

    if (!hev_main_from_file || !hev_quit) {
        LOGE("Failed to resolve hev symbols: main_from_file=%p, quit=%p",
             hev_main_from_file, hev_quit);
        dlclose(hev_lib_handle);
        hev_lib_handle = NULL;
        (*env)->ReleaseStringUTFChars(env, jLibPath, libPath);
        (*env)->ReleaseStringUTFChars(env, jConfigPath, configPath);
        return JNI_FALSE;
    }
    LOGI("Resolved hev symbols successfully");

    /* Prepare thread args */
    struct hev_thread_args *args = malloc(sizeof(struct hev_thread_args));
    if (!args) {
        LOGE("Failed to allocate thread args");
        (*env)->ReleaseStringUTFChars(env, jLibPath, libPath);
        (*env)->ReleaseStringUTFChars(env, jConfigPath, configPath);
        return JNI_FALSE;
    }
    strncpy(args->config_path, configPath, sizeof(args->config_path) - 1);
    args->config_path[sizeof(args->config_path) - 1] = '\0';

    (*env)->ReleaseStringUTFChars(env, jLibPath, libPath);
    (*env)->ReleaseStringUTFChars(env, jConfigPath, configPath);

    /* Start hev in a pthread */
    hev_running = 1;
    int ret = pthread_create(&hev_thread, NULL, hev_thread_func, args);
    if (ret != 0) {
        LOGE("pthread_create failed: %d", ret);
        free(args);
        hev_running = 0;
        return JNI_FALSE;
    }

    LOGI("hev tunnel started in thread (tunFd=%d)", tunFd);
    return JNI_TRUE;
}

JNIEXPORT void JNICALL
Java_com_rustvpn_vpn_HevTunnel_nativeStop(JNIEnv *env, jclass clz)
{
    if (!hev_running) {
        LOGI("hev tunnel not running, nothing to stop");
        return;
    }

    if (hev_quit) {
        LOGI("Calling hev_socks5_tunnel_quit");
        hev_quit();
    }

    /* Wait for the thread to finish */
    pthread_join(hev_thread, NULL);
    hev_running = 0;

    if (hev_lib_handle) {
        dlclose(hev_lib_handle);
        hev_lib_handle = NULL;
    }
    hev_main_from_file = NULL;
    hev_quit = NULL;

    LOGI("hev tunnel stopped");
}

JNIEXPORT jboolean JNICALL
Java_com_rustvpn_vpn_HevTunnel_nativeIsRunning(JNIEnv *env, jclass clz)
{
    return hev_running ? JNI_TRUE : JNI_FALSE;
}
