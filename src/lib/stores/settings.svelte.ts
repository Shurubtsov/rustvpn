import { getSettings, updateSettings } from '$lib/api/tauri';
import type { AppSettings } from '$lib/types';

const DEFAULT_SETTINGS: AppSettings = {
	auto_connect: false,
	last_server_id: null,
	bypass_domains: ['claude.ai', 'anthropic.com', 'api.anthropic.com', 'wb.ru', 'wildberries.ru'],
	dpi_bypass: {
		enabled: true,
		packets: 'tlshello',
		length: '100-200',
		interval: '10-20'
	}
};

function createSettingsStore() {
	let settings = $state<AppSettings>({ ...DEFAULT_SETTINGS });
	let loaded = $state(false);

	async function load() {
		try {
			settings = await getSettings();
			loaded = true;
		} catch {
			settings = { ...DEFAULT_SETTINGS };
			loaded = true;
		}
	}

	async function setAutoConnect(value: boolean) {
		settings = { ...settings, auto_connect: value };
		try {
			await updateSettings(settings);
		} catch {
			// Ignore save errors
		}
	}

	async function setBypassDomains(domains: string[]) {
		settings = { ...settings, bypass_domains: domains };
		try {
			await updateSettings(settings);
		} catch {
			// Ignore save errors
		}
	}

	async function setDpiBypass(enabled: boolean) {
		settings = {
			...settings,
			dpi_bypass: { ...settings.dpi_bypass, enabled }
		};
		try {
			await updateSettings(settings);
		} catch {
			// Ignore save errors
		}
	}

	return {
		get settings() {
			return settings;
		},
		get loaded() {
			return loaded;
		},
		load,
		setAutoConnect,
		setBypassDomains,
		setDpiBypass
	};
}

export const settingsStore = createSettingsStore();
