<script lang="ts">
	import { open, save } from '@tauri-apps/plugin-dialog';
	import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
	import UriInputModal from './UriInputModal.svelte';

	interface Props {
		onImportJson: (json: string) => Promise<void>;
		onImportUri: (uri: string) => Promise<void>;
		onExportJson: () => Promise<string | null>;
		onExportUri: () => Promise<string | null>;
		onToast: (message: string, type?: 'success' | 'error') => void;
	}

	const { onImportJson, onImportUri, onExportJson, onExportUri, onToast }: Props = $props();

	let showUriModal = $state(false);
	let showImportMenu = $state(false);
	let showExportMenu = $state(false);

	async function importFromFile() {
		showImportMenu = false;
		try {
			const path = await open({
				multiple: false,
				filters: [{ name: 'JSON', extensions: ['json'] }]
			});
			if (!path) return;
			const json = await readTextFile(path as string);
			await onImportJson(json);
		} catch (e) {
			onToast(`Import failed: ${e}`, 'error');
		}
	}

	function importFromUri() {
		showImportMenu = false;
		showUriModal = true;
	}

	async function handleUriImport(uri: string) {
		showUriModal = false;
		await onImportUri(uri);
	}

	async function exportToFile() {
		showExportMenu = false;
		try {
			const json = await onExportJson();
			if (json === null) return;
			const path = await save({
				defaultPath: 'servers.json',
				filters: [{ name: 'JSON', extensions: ['json'] }]
			});
			if (!path) return;
			await writeTextFile(path, json);
			onToast('Servers exported to file');
		} catch (e) {
			onToast(`Export failed: ${e}`, 'error');
		}
	}

	async function copyVlessUri() {
		showExportMenu = false;
		try {
			const uri = await onExportUri();
			if (uri === null) return;
			await navigator.clipboard.writeText(uri);
			onToast('vless:// URI copied to clipboard');
		} catch (e) {
			onToast(`Copy failed: ${e}`, 'error');
		}
	}

	function closeMenus() {
		showImportMenu = false;
		showExportMenu = false;
	}
</script>

<svelte:window onclick={closeMenus} />

<div class="flex items-center gap-2 justify-end">
	<!-- Import button with dropdown -->
	<div class="relative">
		<button
			onclick={(e) => { e.stopPropagation(); showExportMenu = false; showImportMenu = !showImportMenu; }}
			class="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors px-2.5 py-1.5 rounded-lg border border-border hover:border-zinc-600 hover:bg-zinc-800/40"
			aria-label="Import servers"
			aria-expanded={showImportMenu}
		>
			<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
				<polyline points="17 8 12 3 7 8"/>
				<line x1="12" y1="3" x2="12" y2="15"/>
			</svg>
			Import
			<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<polyline points="6 9 12 15 18 9"/>
			</svg>
		</button>

		{#if showImportMenu}
			<div
				class="absolute right-0 top-full mt-1 z-40 bg-card border border-border rounded-lg shadow-xl overflow-hidden min-w-[160px]"
				role="menu"
			>
				<button
					onclick={(e) => { e.stopPropagation(); importFromFile(); }}
					class="w-full text-left px-3 py-2 text-sm text-foreground hover:bg-zinc-700/60 transition-colors flex items-center gap-2"
					role="menuitem"
				>
					<svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
						<polyline points="14 2 14 8 20 8"/>
					</svg>
					From File
				</button>
				<button
					onclick={(e) => { e.stopPropagation(); importFromUri(); }}
					class="w-full text-left px-3 py-2 text-sm text-foreground hover:bg-zinc-700/60 transition-colors flex items-center gap-2"
					role="menuitem"
				>
					<svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
						<path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
					</svg>
					From vless:// URI
				</button>
			</div>
		{/if}
	</div>

	<!-- Export button with dropdown -->
	<div class="relative">
		<button
			onclick={(e) => { e.stopPropagation(); showImportMenu = false; showExportMenu = !showExportMenu; }}
			class="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors px-2.5 py-1.5 rounded-lg border border-border hover:border-zinc-600 hover:bg-zinc-800/40"
			aria-label="Export servers"
			aria-expanded={showExportMenu}
		>
			<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
				<polyline points="7 10 12 15 17 10"/>
				<line x1="12" y1="15" x2="12" y2="3"/>
			</svg>
			Export
			<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<polyline points="6 9 12 15 18 9"/>
			</svg>
		</button>

		{#if showExportMenu}
			<div
				class="absolute right-0 top-full mt-1 z-40 bg-card border border-border rounded-lg shadow-xl overflow-hidden min-w-[160px]"
				role="menu"
			>
				<button
					onclick={(e) => { e.stopPropagation(); exportToFile(); }}
					class="w-full text-left px-3 py-2 text-sm text-foreground hover:bg-zinc-700/60 transition-colors flex items-center gap-2"
					role="menuitem"
				>
					<svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
						<polyline points="14 2 14 8 20 8"/>
					</svg>
					To File
				</button>
				<button
					onclick={(e) => { e.stopPropagation(); copyVlessUri(); }}
					class="w-full text-left px-3 py-2 text-sm text-foreground hover:bg-zinc-700/60 transition-colors flex items-center gap-2"
					role="menuitem"
				>
					<svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
						<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
					</svg>
					Copy vless:// URI
				</button>
			</div>
		{/if}
	</div>
</div>

{#if showUriModal}
	<UriInputModal onImport={handleUriImport} onCancel={() => { showUriModal = false; }} />
{/if}
