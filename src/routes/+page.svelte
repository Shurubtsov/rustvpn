<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import { serversStore } from '$lib/stores/servers.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import ConnectButton from '$lib/components/ConnectButton.svelte';
	import StatusDisplay from '$lib/components/StatusDisplay.svelte';
	import SpeedGraph from '$lib/components/SpeedGraph.svelte';
	import ServerList from '$lib/components/ServerList.svelte';
	import ServerForm from '$lib/components/ServerForm.svelte';
	import ImportExportBar from '$lib/components/ImportExportBar.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { isMobile } from '$lib/utils/platform';
	import type { ServerConfig } from '$lib/types';

	const store = connectionStore;
	const servers = serversStore;
	const appSettings = settingsStore;

	// Timer state
	let elapsedSeconds = $state(0);
	let timerInterval: ReturnType<typeof setInterval> | null = null;

	// Form modal state
	let showForm = $state(false);
	let editingServer = $state<ServerConfig | null>(null);

	// Toast state
	let toast = $state<{ message: string; type: 'success' | 'error' } | null>(null);
	let toastTimer: ReturnType<typeof setTimeout> | null = null;

	function showToast(message: string, type: 'success' | 'error' = 'success') {
		if (toastTimer !== null) clearTimeout(toastTimer);
		toast = { message, type };
		toastTimer = setTimeout(() => {
			toast = null;
			toastTimer = null;
		}, 3000);
	}

	function startTimer() {
		if (timerInterval !== null) return;
		elapsedSeconds = 0;
		if (store.info.connected_since) {
			const nowSecs = Math.floor(Date.now() / 1000);
			elapsedSeconds = Math.max(0, nowSecs - store.info.connected_since);
		}
		timerInterval = setInterval(() => {
			elapsedSeconds += 1;
		}, 1000);
	}

	function stopTimer() {
		if (timerInterval !== null) {
			clearInterval(timerInterval);
			timerInterval = null;
		}
		elapsedSeconds = 0;
	}

	$effect(() => {
		if (store.info.status === 'connected') {
			startTimer();
		} else {
			stopTimer();
		}
	});

	async function handleToggle() {
		if (store.isLoading || store.isTransitioning) return;
		if (store.isConnected) {
			await store.disconnectVpn();
		} else {
			const selected = servers.selectedServer;
			if (selected) await store.connectVpn(selected);
		}
	}

	function openAdd() {
		editingServer = null;
		showForm = true;
	}

	function openEdit(server: ServerConfig) {
		editingServer = server;
		showForm = true;
	}

	function closeForm() {
		showForm = false;
		editingServer = null;
	}

	async function handleSave(server: ServerConfig) {
		try {
			if (editingServer !== null) {
				await servers.updateServer(server);
			} else {
				await servers.addServer(server);
			}
		} catch (e) {
			showToast(`Failed to save server: ${e}`, 'error');
		}
		closeForm();
	}

	async function handleImportJson(json: string) {
		try {
			const imported = await servers.importFromJson(json);
			showToast(`Imported ${imported.length} server(s)`);
		} catch (e) {
			showToast(`Import failed: ${e}`, 'error');
		}
	}

	async function handleImportUri(uri: string) {
		try {
			await servers.importFromUri(uri);
			showToast('Server imported from URI');
		} catch (e) {
			showToast(`Import failed: ${e}`, 'error');
		}
	}

	async function handleExportJson() {
		try {
			const json = await servers.exportToJson();
			return json;
		} catch (e) {
			showToast(`Export failed: ${e}`, 'error');
			return null;
		}
	}

	async function handleExportUri() {
		const selected = servers.selectedServer;
		if (!selected) {
			showToast('No server selected', 'error');
			return null;
		}
		try {
			return await servers.exportToUri(selected);
		} catch (e) {
			showToast(`Export failed: ${e}`, 'error');
			return null;
		}
	}

	onMount(async () => {
		await servers.load();
		await appSettings.load();
		store.refresh();
		store.startPolling();
	});

	onDestroy(() => {
		store.stopPolling();
		stopTimer();
		if (toastTimer !== null) clearTimeout(toastTimer);
	});
</script>

<div class="min-h-screen bg-background text-foreground flex flex-col p-4 gap-4 pb-safe">

	<!-- App header -->
	<div class="flex items-center justify-between pt-2">
		<div class="w-8"></div>
		<div class="text-center">
			<h1 class="text-xl font-bold tracking-widest uppercase text-foreground/90">RustVPN</h1>
			<p class="text-xs text-muted-foreground mt-0.5">VLESS + REALITY</p>
		</div>
		<ThemeToggle />
	</div>

	<!-- Server list -->
	<div class="w-full">
		<ServerList onEdit={openEdit} onAdd={openAdd} />
	</div>

	<!-- Import/Export toolbar -->
	<ImportExportBar
		onImportJson={handleImportJson}
		onImportUri={handleImportUri}
		onExportJson={handleExportJson}
		onExportUri={handleExportUri}
		onToast={showToast}
	/>

	<!-- Settings bar -->
	<div class="flex items-center justify-between">
		<a
			href="/logs"
			class="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
		>
			<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
				<polyline points="14 2 14 8 20 8" />
				<line x1="16" x2="8" y1="13" y2="13" />
				<line x1="16" x2="8" y1="17" y2="17" />
				<line x1="10" x2="8" y1="9" y2="9" />
			</svg>
			Logs
		</a>
		<label class="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer">
			<input
				type="checkbox"
				checked={appSettings.settings.auto_connect}
				onchange={(e) => appSettings.setAutoConnect(e.currentTarget.checked)}
				class="rounded border-border"
			/>
			Auto-connect
		</label>
	</div>

	<!-- Divider -->
	<div class="border-t border-border"></div>

	<!-- Center section: status + button + info -->
	<div class="flex flex-col items-center gap-6 flex-1 justify-center py-2">
		<StatusDisplay
			info={store.info}
			{elapsedSeconds}
			uploadSpeed={store.stats.upload_speed}
			downloadSpeed={store.stats.download_speed}
			totalUpload={store.stats.total_upload}
			totalDownload={store.stats.total_download}
		/>

		<ConnectButton
			status={store.info.status}
			isLoading={store.isLoading}
			isTransitioning={store.isTransitioning}
			isConnected={store.isConnected}
			onclick={handleToggle}
		/>
	</div>

	<!-- Speed graph -->
	{#if store.info.status === 'connected'}
		<div class="w-full px-1">
			<SpeedGraph
				uploadHistory={store.speedHistory.upload}
				downloadHistory={store.speedHistory.download}
				uploadSpeed={store.stats.upload_speed}
				downloadSpeed={store.stats.download_speed}
			/>
		</div>
	{/if}

</div>

<!-- ServerForm modal -->
{#if showForm}
	<ServerForm
		server={editingServer}
		onSave={handleSave}
		onCancel={closeForm}
	/>
{/if}

<!-- Toast notification -->
{#if toast}
	<div
		class="fixed left-1/2 z-50 px-4 py-2.5 rounded-lg text-sm font-medium shadow-lg backdrop-blur-sm
			{toast.type === 'error'
				? 'bg-destructive/90 text-destructive-foreground'
				: 'bg-zinc-800/90 text-foreground border border-border'}"
		style="bottom: calc(1rem + env(safe-area-inset-bottom, 0px)); animation: toast-in 0.2s ease-out forwards"
		role="status"
		aria-live="polite"
	>
		<div class="flex items-center gap-2">
			{#if toast.type === 'error'}
				<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<circle cx="12" cy="12" r="10" />
					<line x1="15" y1="9" x2="9" y2="15" />
					<line x1="9" y1="9" x2="15" y2="15" />
				</svg>
			{:else}
				<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<polyline points="20 6 9 17 4 12" />
				</svg>
			{/if}
			{toast.message}
		</div>
	</div>
{/if}
