import type { ServerConfig } from '$lib/types';
import * as api from '$lib/api/tauri';

function createServersStore() {
	let servers = $state<ServerConfig[]>([]);
	let selectedId = $state<string | null>(null);

	const selectedServer = $derived(
		selectedId !== null ? (servers.find((s) => s.id === selectedId) ?? null) : null
	);

	const selectedIndex = $derived(
		selectedId !== null ? servers.findIndex((s) => s.id === selectedId) : -1
	);

	async function load() {
		const loaded = await api.getServers();
		servers = loaded;
		// Select first server if current selection is gone
		if (selectedId === null || !loaded.some((s) => s.id === selectedId)) {
			selectedId = loaded.length > 0 ? loaded[0].id : null;
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
		load,
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
