import { applyBypassDomains, getSettings, updateSettings } from '$lib/api/tauri';
import type { AppSettings } from '$lib/types';

const DEFAULT_SETTINGS: AppSettings = {
	auto_connect: false,
	last_server_id: null,
	bypass_domains: ['claude.ai', 'anthropic.com', 'api.anthropic.com', 'wb.ru', 'wildberries.ru']
};

function createSettingsStore() {
	let settings = $state<AppSettings>({ ...DEFAULT_SETTINGS });
	let loaded = $state(false);
	let loadError = $state<string | null>(null);
	let saveError = $state<string | null>(null);

	async function load() {
		try {
			settings = await getSettings();
			loadError = null;
		} catch (err) {
			// Fall back to defaults but surface the error so the UI can warn
			// the user that saved settings couldn't be read — otherwise a
			// subsequent save would silently clobber them on disk.
			settings = { ...DEFAULT_SETTINGS };
			loadError = err instanceof Error ? err.message : String(err);
		} finally {
			loaded = true;
		}
	}

	async function save(next: AppSettings): Promise<void> {
		const prev = settings;
		settings = next;
		try {
			await updateSettings(next);
			saveError = null;
		} catch (err) {
			// Roll local state back so it doesn't diverge from disk.
			settings = prev;
			saveError = err instanceof Error ? err.message : String(err);
			throw err;
		}
	}

	async function setAutoConnect(value: boolean): Promise<void> {
		await save({ ...settings, auto_connect: value });
	}

	async function setBypassDomains(domains: string[]): Promise<boolean> {
		// No-op guard: the textarea's `onchange` can fire on blur even when the
		// user didn't edit anything, and without this check we'd tear down a live
		// VPN session (stop → start) on every focus/blur cycle.
		const current = settings.bypass_domains ?? [];
		const unchanged =
			current.length === domains.length && current.every((d, i) => d === domains[i]);
		if (unchanged) {
			return false;
		}
		// Route through the backend command that also reloads the running xray/TUN
		// stack if a session is active — otherwise edits would silently do nothing
		// until the user reconnected manually.
		const prev = settings;
		settings = { ...settings, bypass_domains: domains };
		try {
			const reloaded = await applyBypassDomains(domains);
			saveError = null;
			return reloaded;
		} catch (err) {
			settings = prev;
			saveError = err instanceof Error ? err.message : String(err);
			throw err;
		}
	}

	return {
		get settings() {
			return settings;
		},
		get loaded() {
			return loaded;
		},
		get loadError() {
			return loadError;
		},
		get saveError() {
			return saveError;
		},
		load,
		setAutoConnect,
		setBypassDomains
	};
}

export const settingsStore = createSettingsStore();
