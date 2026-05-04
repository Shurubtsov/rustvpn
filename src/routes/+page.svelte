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
	import BackgroundModeModal from '$lib/components/BackgroundModeModal.svelte';
	import { isMobile, isDesktop } from '$lib/utils/platform';
	import { detectVpnInterfaces, isBatteryOptimizationIgnored } from '$lib/api/tauri';
	import type { ServerConfig, DetectedVpn } from '$lib/types';

	const BG_MODAL_DISMISS_KEY = 'rustvpn.bgModeModalDismissed';

	const store = connectionStore;
	const servers = serversStore;
	const appSettings = settingsStore;

	// Timer state
	let elapsedSeconds = $state(0);
	let timerInterval: ReturnType<typeof setInterval> | null = null;

	// Form modal state
	let showForm = $state(false);
	let editingServer = $state<ServerConfig | null>(null);

	// Detected VPNs state
	let detectedVpns = $state<DetectedVpn[]>([]);
	let vpnDetecting = $state(false);

	// Mobile background-mode setup state. The modal is shown on the first
	// connect attempt if battery optimization isn't already exempt and the
	// user hasn't checked "don't show again". `batteryOptIgnored` is also
	// used by the modal's step 1 indicator to flip to a checkmark once the
	// user grants the exemption from the system dialog.
	let showBgModal = $state(false);
	let batteryOptIgnored = $state(true);
	let pendingConnectServer = $state<ServerConfig | null>(null);

	async function refreshVpnDetection() {
		if (vpnDetecting) return;
		vpnDetecting = true;
		// Cap the detection call so a hung backend can't leave the button
		// stuck on "Detecting…" forever.
		const timeout = new Promise<never>((_, reject) =>
			setTimeout(() => reject(new Error('Timed out after 10s')), 10000)
		);
		try {
			detectedVpns = await Promise.race([detectVpnInterfaces(), timeout]);
		} catch (e) {
			showToast(`VPN detection failed: ${e}`, 'error');
		} finally {
			vpnDetecting = false;
		}
	}

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
			return;
		}
		const selected = servers.selectedServer;
		if (!selected) {
			showToast('No server selected', 'error');
			return;
		}
		// Cheap frontend sanity check — catches corrupted imports before
		// they become cryptic backend errors.
		if (!selected.address?.trim() || !selected.uuid?.trim() || !selected.port) {
			showToast('Selected server config is incomplete', 'error');
			return;
		}

		// On mobile: surface the background-mode setup modal once if the user
		// hasn't ticked "don't show again" and battery optimization isn't yet
		// exempt. The actual connect kicks off after the user dismisses the
		// modal, so they don't immediately see the VPN die after first launch.
		if (isMobile()) {
			const dismissed = localStorage.getItem(BG_MODAL_DISMISS_KEY) === '1';
			if (!dismissed) {
				try {
					batteryOptIgnored = await isBatteryOptimizationIgnored();
				} catch {
					// If the platform check fails treat it as exempt — better to
					// connect and have the user discover the issue than to block
					// every connect on a flaky probe.
					batteryOptIgnored = true;
				}
				if (!batteryOptIgnored) {
					pendingConnectServer = selected;
					showBgModal = true;
					return;
				}
			}
		}

		await store.connectVpn(selected);
	}

	function closeBgModal(dismissForever: boolean) {
		showBgModal = false;
		if (dismissForever) {
			localStorage.setItem(BG_MODAL_DISMISS_KEY, '1');
		}
		const pending = pendingConnectServer;
		pendingConnectServer = null;
		if (pending) {
			// User finished the setup flow — proceed with the connect they
			// originally requested. Run unawaited so the modal closes cleanly.
			store.connectVpn(pending);
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
		try {
			await servers.load();
		} catch (e) {
			showToast(`Failed to load servers: ${e}`, 'error');
		}
		await appSettings.load();
		if (appSettings.loadError) {
			showToast(
				`Failed to load settings — using defaults (${appSettings.loadError})`,
				'error'
			);
		}
		store.refresh();
		store.startPolling();
		if (isDesktop()) {
			refreshVpnDetection();
		}
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
				onchange={async (e) => {
					try {
						await appSettings.setAutoConnect(e.currentTarget.checked);
					} catch (err) {
						showToast(`Failed to save settings: ${err}`, 'error');
					}
				}}
				class="rounded border-border"
			/>
			Auto-connect
		</label>
	</div>

	<!-- Bypass domains (split tunneling) -->
	<details class="group">
		<summary class="text-xs text-muted-foreground cursor-pointer hover:text-foreground transition-colors px-1 flex items-center gap-1">
			<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="transition-transform group-open:rotate-90">
				<polyline points="9 18 15 12 9 6" />
			</svg>
			Bypass domains ({appSettings.settings.bypass_domains?.length ?? 0})
		</summary>
		<div class="mt-1.5 px-1">
			<textarea
				class="w-full bg-background border border-border rounded-lg px-3 py-2 text-xs text-foreground font-mono placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring resize-none"
				rows="3"
				placeholder="One domain per line, e.g.&#10;claude.ai&#10;anthropic.com"
				value={appSettings.settings.bypass_domains?.join('\n') ?? ''}
				onchange={async (e) => {
					// Accept only syntactically plausible hostnames: letters, digits,
					// dots, hyphens. Strip anything that looks like junk rather than
					// forwarding it to the backend, where it would silently break the
					// xray routing rules or gsettings bypass list.
					const domainRegex = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$/i;
					const raw = e.currentTarget.value
						.split('\n')
						.map((d) => d.trim().toLowerCase())
						.filter((d) => d.length > 0);
					const valid = raw.filter((d) => domainRegex.test(d));
					const invalid = raw.filter((d) => !domainRegex.test(d));
					if (invalid.length > 0) {
						showToast(`Ignored ${invalid.length} invalid domain(s)`, 'error');
					}
					try {
						const reloaded = await appSettings.setBypassDomains(valid);
						if (reloaded) {
							showToast('Reloaded VPN with new bypass list');
						}
					} catch (err) {
						showToast(`Failed to save settings: ${err}`, 'error');
					}
				}}
			></textarea>
			<p class="text-[10px] text-muted-foreground/60 mt-0.5">These domains bypass the VPN tunnel (one per line). Applied immediately — the VPN auto-reloads if connected.</p>
		</div>
	</details>

	<!-- Detected corporate VPNs (desktop only) -->
	{#if isDesktop()}
	<details class="group">
		<summary class="text-xs text-muted-foreground cursor-pointer hover:text-foreground transition-colors px-1 flex items-center gap-1">
			<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="transition-transform group-open:rotate-90">
				<polyline points="9 18 15 12 9 6" />
			</svg>
			Corporate VPNs ({detectedVpns.length})
		</summary>
		<div class="mt-1.5 px-1 space-y-1.5">
			{#if detectedVpns.length === 0}
				<p class="text-[10px] text-muted-foreground/60">No corporate VPN interfaces detected.</p>
			{:else}
				{#each detectedVpns as vpn}
					<div class="text-xs bg-muted/30 rounded-md px-2.5 py-1.5 border border-border/50">
						<div class="flex items-center gap-1.5">
							<span class="font-mono font-medium text-foreground">{vpn.interface}</span>
							<span class="text-muted-foreground">({vpn.vpn_type})</span>
						</div>
						{#if vpn.subnets.length > 0}
							<div class="text-[10px] text-muted-foreground/80 mt-0.5 font-mono">
								{vpn.subnets.join(', ')}
							</div>
						{/if}
						{#if vpn.server_ip}
							<div class="text-[10px] text-muted-foreground/80 mt-0.5 font-mono">
								Server: {vpn.server_ip}
							</div>
						{/if}
					</div>
				{/each}
			{/if}
			<div class="flex items-center justify-between">
				<p class="text-[10px] text-muted-foreground/60">Detected subnets are auto-bypassed on connect.</p>
				<button
					class="text-[10px] text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50"
					onclick={refreshVpnDetection}
					disabled={vpnDetecting}
				>
					{vpnDetecting ? 'Detecting...' : 'Refresh'}
				</button>
			</div>
		</div>
	</details>
	{/if}

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

<!-- Background mode setup modal (mobile only, on first connect) -->
{#if showBgModal}
	<BackgroundModeModal
		{batteryOptIgnored}
		onClose={closeBgModal}
		onBatteryOptChanged={(granted) => { batteryOptIgnored = granted; }}
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
