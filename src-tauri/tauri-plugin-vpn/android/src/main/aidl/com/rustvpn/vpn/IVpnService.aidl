// AIDL interface used to communicate between the Tauri activity (main process)
// and RustVpnService (running in the dedicated ":vpn" process). Returning JSON
// strings instead of declaring AIDL parcelables keeps the build simple — no
// kotlin-parcelize plugin or hand-written Parcelable boilerplate needed, and
// the payload is tiny so marshalling cost is negligible.
package com.rustvpn.vpn;

interface IVpnService {
    // Start the VPN with the given xray config. Returns immediately; callers
    // should poll getStatusJson() to observe readiness.
    void startVpn(String configJson, int socksPort, String serverAddress);

    // Tear down the VPN (xray + hev tunnel + TUN fd) and stop the foreground
    // service so the :vpn process can exit.
    void stopVpn();

    // JSON: {"is_running":bool,"last_error":string|null,
    //        "xray_running":bool,"hev_running":bool,"tun_active":bool}.
    String getStatusJson();

    // JSON: {"upload":long,"download":long}. Cumulative byte counters.
    String getStatsJson();
}
