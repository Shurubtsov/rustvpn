export interface RealitySettings {
	public_key: string;
	short_id: string;
	server_name: string;
	fingerprint: string;
}

export interface ServerConfig {
	id: string;
	name: string;
	address: string;
	port: number;
	uuid: string;
	flow: string;
	reality: RealitySettings;
}

export type ConnectionStatus =
	| 'disconnected'
	| 'connecting'
	| 'connected'
	| 'disconnecting'
	| 'error';

export interface ConnectionInfo {
	status: ConnectionStatus;
	server_name: string | null;
	server_address: string | null;
	connected_since: number | null;
	error_message: string | null;
}

export interface SpeedStats {
	upload_speed: number;
	download_speed: number;
	total_upload: number;
	total_download: number;
}

export interface LogEntry {
	timestamp: number;
	level: string;
	message: string;
}

export interface AppSettings {
	auto_connect: boolean;
	last_server_id: string | null;
}

// Platform types for cross-platform UI adaptations
export type PlatformType = 'windows' | 'macos' | 'linux' | 'android' | 'ios';

export interface PlatformInfo {
	platform: PlatformType;
	isMobile: boolean;
	isDesktop: boolean;
}
