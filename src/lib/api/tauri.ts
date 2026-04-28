import { invoke } from '@tauri-apps/api/core';
import type { AppSettings, ConnectionInfo, DetectedVpn, LogEntry, ServerConfig, SpeedStats } from '$lib/types';

export async function connect(config: ServerConfig): Promise<void> {
	await invoke<void>('connect', { serverConfig: config });
}

export async function disconnect(): Promise<void> {
	await invoke<void>('disconnect');
}

export async function getConnectionInfo(): Promise<ConnectionInfo> {
	return await invoke<ConnectionInfo>('get_connection_info');
}

// Server CRUD
export async function getServers(): Promise<ServerConfig[]> {
	return await invoke<ServerConfig[]>('get_servers');
}

export async function addServer(config: ServerConfig): Promise<ServerConfig> {
	return await invoke<ServerConfig>('add_server', { serverConfig: config });
}

export async function updateServer(config: ServerConfig): Promise<void> {
	await invoke<void>('update_server', { serverConfig: config });
}

export async function deleteServer(id: string): Promise<void> {
	await invoke<void>('delete_server', { id });
}

// Import / Export
export async function exportServers(): Promise<string> {
	return await invoke<string>('export_servers');
}

export async function importServers(json: string): Promise<ServerConfig[]> {
	return await invoke<ServerConfig[]>('import_servers', { json });
}

// VLESS URI
export async function parseVlessUri(uri: string): Promise<ServerConfig> {
	return await invoke<ServerConfig>('parse_vless_uri_cmd', { uri });
}

export async function exportVlessUri(config: ServerConfig): Promise<string> {
	return await invoke<string>('export_vless_uri', { serverConfig: config });
}

export async function getSpeedStats(): Promise<SpeedStats> {
	return await invoke<SpeedStats>('get_speed_stats');
}

// Logs
export async function getLogs(): Promise<LogEntry[]> {
	return await invoke<LogEntry[]>('get_logs');
}

export async function clearLogs(): Promise<void> {
	await invoke<void>('clear_logs');
}

// Settings
export async function getSettings(): Promise<AppSettings> {
	return await invoke<AppSettings>('get_settings');
}

export async function updateSettings(settings: AppSettings): Promise<void> {
	await invoke<void>('update_settings', { settings });
}

/**
 * Persist a new bypass-domain list and, if the VPN is currently active, restart
 * the xray+TUN stack so the change takes effect immediately. Returns true if a
 * reconnect was performed, false if only the setting was saved.
 */
export async function applyBypassDomains(domains: string[]): Promise<boolean> {
	return await invoke<boolean>('apply_bypass_domains', { domains });
}

// VPN detection
export async function detectVpnInterfaces(): Promise<DetectedVpn[]> {
	return await invoke<DetectedVpn[]>('detect_vpn_interfaces');
}

// Mobile background-mode setup. On desktop these always resolve to true / no-op
// because there is no Doze and no OEM background-activity policy to deal with.
export async function isBatteryOptimizationIgnored(): Promise<boolean> {
	return await invoke<boolean>('is_battery_optimization_ignored');
}

export async function requestIgnoreBatteryOptimization(): Promise<boolean> {
	return await invoke<boolean>('request_ignore_battery_optimization');
}

export async function openOemBackgroundSettings(): Promise<{ opened: boolean; fallback: boolean }> {
	const [opened, fallback] = await invoke<[boolean, boolean]>('open_oem_background_settings');
	return { opened, fallback };
}
