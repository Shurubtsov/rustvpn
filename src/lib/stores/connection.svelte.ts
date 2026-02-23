import { getConnectionInfo, connect, disconnect, getSpeedStats } from '$lib/api/tauri';
import type { ConnectionInfo, ServerConfig, SpeedStats } from '$lib/types';

const DEFAULT_INFO: ConnectionInfo = {
	status: 'disconnected',
	server_name: null,
	server_address: null,
	connected_since: null,
	error_message: null
};

const DEFAULT_STATS: SpeedStats = {
	upload_speed: 0,
	download_speed: 0,
	total_upload: 0,
	total_download: 0
};

function createConnectionStore() {
	let info = $state<ConnectionInfo>({ ...DEFAULT_INFO });
	let isLoading = $state(false);
	let pollInterval: ReturnType<typeof setInterval> | null = null;
	let stats = $state<SpeedStats>({ ...DEFAULT_STATS });
	let speedHistory = $state<{ upload: number[]; download: number[] }>({ upload: [], download: [] });

	const isConnected = $derived(info.status === 'connected');
	const isTransitioning = $derived(
		info.status === 'connecting' || info.status === 'disconnecting'
	);

	function startPolling() {
		if (pollInterval !== null) return;
		pollInterval = setInterval(async () => {
			try {
				const result = await getConnectionInfo();
				info = result;
				if (result.status === 'connected') {
					const s = await getSpeedStats();
					stats = s;
					// Keep last 60 data points
					speedHistory = {
						upload: [...speedHistory.upload, s.upload_speed].slice(-60),
						download: [...speedHistory.download, s.download_speed].slice(-60)
					};
				} else {
					stats = { ...DEFAULT_STATS };
					speedHistory = { upload: [], download: [] };
				}
			} catch {
				// Ignore polling errors silently
			}
		}, 1000);
	}

	function stopPolling() {
		if (pollInterval !== null) {
			clearInterval(pollInterval);
			pollInterval = null;
		}
	}

	async function refresh() {
		try {
			info = await getConnectionInfo();
		} catch {
			// Ignore
		}
	}

	async function connectVpn(config: ServerConfig) {
		isLoading = true;
		try {
			await connect(config);
			await refresh();
			startPolling();
		} catch (err) {
			info = {
				...info,
				status: 'error',
				error_message: err instanceof Error ? err.message : String(err)
			};
		} finally {
			isLoading = false;
		}
	}

	async function disconnectVpn() {
		isLoading = true;
		try {
			await disconnect();
			await refresh();
		} catch (err) {
			info = {
				...info,
				status: 'error',
				error_message: err instanceof Error ? err.message : String(err)
			};
		} finally {
			isLoading = false;
			stopPolling();
			stats = { ...DEFAULT_STATS };
			speedHistory = { upload: [], download: [] };
		}
	}

	return {
		get info() {
			return info;
		},
		get isLoading() {
			return isLoading;
		},
		get isConnected() {
			return isConnected;
		},
		get isTransitioning() {
			return isTransitioning;
		},
		get stats() {
			return stats;
		},
		get speedHistory() {
			return speedHistory;
		},
		refresh,
		connectVpn,
		disconnectVpn,
		startPolling,
		stopPolling
	};
}

export const connectionStore = createConnectionStore();
