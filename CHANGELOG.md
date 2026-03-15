# Changelog

## [0.6.0] - 2026-03-15

### Fixed

- **Corporate VPN DNS resolution in TUN mode** — Internal corporate hostnames (e.g.
  `gitlab-paygate.paywb.info`) that only exist on corporate DNS now resolve correctly.
  xray detects corporate DNS servers from `/etc/resolv.conf` and queries them with
  `expectIPs` scoped to VPN-routed subnets, accepting both private and public IPs
  returned by corporate DNS.

- **Corporate VPN traffic source IP** — Traffic to corporate subnets (including
  public IPs routed through the corporate VPN) was being sent with the wrong source IP
  (LAN IP via `sendThrough`) causing the corporate server to reject connections. A new
  `direct-vpn` xray outbound without `sendThrough` lets the kernel assign the correct
  VPN-assigned source IP via `ip rule to SUBNET lookup main`.

- **Corporate VPN breaks after RustVPN disconnect** — NetworkManager was recalculating
  routing and DNS for all connections when the RustVPN TUN device (`rvpn0`) appeared
  or disappeared, corrupting corporate VPN routes and dropping its DNS servers from
  `/etc/resolv.conf`. Fixed by marking `rvpn0` as unmanaged (`nmcli device set rvpn0
  managed no`) and restoring `/etc/resolv.conf` + reloading NM DNS on TUN teardown.

- **RFC-1918 traffic bypass** — When a corporate VPN is active, all RFC-1918 ranges are
  now added to the kernel `ip rule` bypass list, ensuring corporate DNS servers and
  LAN resources use the main routing table instead of the RustVPN TUN.

---

## [0.5.0] - 2026-03-07

### Fixed

- **TUN mode DNS hang** — Dropped `localhost` from xray DNS in TUN mode; only use
  `1.1.1.1` and `8.8.8.8` to avoid 30-second hangs caused by corporate VPN DNS
  being unroutable through the TUN.

- **TUN routing loops** — Added source-based policy routing (`ip rule add from
  LOCAL_IP lookup main`) so xray's `sendThrough` traffic exits via the physical
  interface rather than looping back through the TUN.

- **TUN cleanup on crash** — Added watchdog process and startup cleanup to remove
  stale TUN devices left behind by app crashes.

---

## [0.4.0] - 2026-02-20

### Added

- TUN mode for Linux — routes all traffic through the VPN, not just browser traffic.
- Corporate VPN detection — automatically detects active VPN interfaces and their
  subnets to configure bypass routing.
- Speed statistics — real-time upload/download counters via xray gRPC stats API.

### Fixed

- VLESS server IP bypass route — prevents the VLESS server itself from being routed
  through the TUN, which would create a routing loop.
