// AIDL interface used to communicate between the Tauri activity (main process)
// and RustVpnService (running in the dedicated ":vpn" process). Returning JSON
// strings instead of declaring AIDL parcelables keeps the build simple — no
// kotlin-parcelize plugin or hand-written Parcelable boilerplate needed, and
// the payload is tiny so marshalling cost is negligible.
package com.rustvpn.vpn;

interface IVpnService {
    // Tear down the VPN (xray + hev tunnel + TUN fd) and stop the foreground
    // service so the :vpn process can exit.
    void stopVpn();

    // JSON: {"is_running":bool,"last_error":string|null,
    //        "xray_running":bool,"hev_running":bool,"tun_active":bool}.
    String getStatusJson();

    // JSON: {"upload":long,"download":long}. Cumulative byte counters.
    String getStatsJson();

    // NOTE: there is intentionally no startVpn() method here. Starting always
    // goes through Activity.startForegroundService(intent) so the OS-side
    // foreground promotion happens within Android's 5-second window — a
    // bound-service IPC start would race with that timer. The activity passes
    // the xray config via Intent extras; the service picks it up in
    // onStartCommand. If a future flow needs to (re)start the tunnel via an
    // existing binder, add it back here and have it re-issue the foreground
    // intent so the timing guarantee is preserved.
}
