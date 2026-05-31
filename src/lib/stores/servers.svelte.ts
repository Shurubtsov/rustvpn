import type { ServerConfig } from '$lib/types';
import * as api from '$lib/api/tauri';

function createServersStore() {
	let servers = $state<ServerConfig[]>([]);
	let selectedId = $state<string | null>(null);
	let loadError = $state<string | null>(null);

	const selectedServer = $derived(
		selectedId !== null ? (servers.find((s) => s.id === selectedId) ?? null) : null
	);

	const selectedIndex = $derived(
		selectedId !== null ? servers.findIndex((s) => s.id === selectedId) : -1
	);

	async function load() {
		try {
			const loaded = await api.getServers();
			// Never let a transient empty read wipe an already-populated list. The
			// UI can't delete down to zero, so getting [] back while we already hold
			// servers is a backend hiccup (seen on Android resume), not real state —
			// clobbering it is what makes the server row "disappear".
			if (loaded.length === 0 && servers.length > 0) {
				loadError = null;
				return;
			}
			servers = loaded;
			// Select first server if current selection is gone
			if (selectedId === null || !loaded.some((s) => s.id === selectedId)) {
				selectedId = loaded.length > 0 ? loaded[0].id : null;
			}
			loadError = null;
		} catch (err) {
			loadError = err instanceof Error ? err.message : String(err);
			throw err;
		}
	}

	/**
	 * Load the server list, tolerating a cold/not-yet-ready Tauri backend.
	 *
	 * This is the real fix for the Android "server list is empty on reopen"
	 * bug. ColorOS (and Android generally under memory pressure) destroys the
	 * Activity while backgrounded and reloads the WebView on resume. When the
	 * page re-runs onMount it fires `get_servers` immediately — but the IPC
	 * bridge / Rust backend may not be ready yet, so that first call either
	 * *rejects* or resolves with `[]`. A single attempt then leaves the list
	 * empty until something forces a fresh mount (navigating to /logs and back),
	 * which is exactly the symptom observed.
	 *
	 * So we retry on BOTH failure modes — a thrown/rejected invoke AND a
	 * successful-but-empty result — with a short backoff, stopping as soon as we
	 * have servers. A genuinely empty install (first run) simply exhausts the
	 * attempts quickly and shows the empty/add-server UI, which is fine.
	 */
	async function loadWithRetry(attempts = 8, delayMs = 250): Promise<void> {
		for (let i = 0; i < attempts; i++) {
			try {
				await load();
				// Got real data — done. (If we already hold servers from a warm
				// store, servers.length stays > 0 and we stop immediately.)
				if (servers.length > 0) return;
			} catch {
				// Backend not ready yet — fall through to the backoff and retry.
			}
			// Last attempt: don't sleep needlessly.
			if (i < attempts - 1) {
				await new Promise((r) => setTimeout(r, delayMs));
			}
		}
	}

	async function addServer(server: ServerConfig): Promise<ServerConfig> {
		const created = await api.addServer(server);
		await load();
		selectedId = created.id;
		return created;
	}

	async function updateServer(server: ServerConfig): Promise<void> {
		await api.updateServer(server);
		await load();
	}

	async function deleteServer(id: string): Promise<void> {
		await api.deleteServer(id);
		await load();
	}

	function selectServer(id: string) {
		if (servers.some((s) => s.id === id)) {
			selectedId = id;
		}
	}

	/** Select by numeric index (kept for backward compat with existing UI code) */
	function selectServerByIndex(index: number) {
		if (index >= 0 && index < servers.length) {
			selectedId = servers[index].id;
		}
	}

	async function importFromJson(json: string): Promise<ServerConfig[]> {
		const imported = await api.importServers(json);
		await load();
		if (imported.length > 0) {
			selectedId = imported[0].id;
		}
		return imported;
	}

	async function importFromUri(uri: string): Promise<ServerConfig> {
		const parsed = await api.parseVlessUri(uri);
		const created = await api.addServer(parsed);
		await load();
		selectedId = created.id;
		return created;
	}

	async function exportToJson(): Promise<string> {
		return await api.exportServers();
	}

	async function exportToUri(server: ServerConfig): Promise<string> {
		return await api.exportVlessUri(server);
	}

	return {
		get servers() {
			return servers;
		},
		get selectedId() {
			return selectedId;
		},
		get selectedIndex() {
			return selectedIndex;
		},
		get selectedServer() {
			return selectedServer;
		},
		get loadError() {
			return loadError;
		},
		load,
		loadWithRetry,
		addServer,
		updateServer,
		deleteServer,
		selectServer,
		selectServerByIndex,
		importFromJson,
		importFromUri,
		exportToJson,
		exportToUri
	};
}

export const serversStore = createServersStore();
