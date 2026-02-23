import { platform } from '@tauri-apps/plugin-os';

export type Platform = 'windows' | 'macos' | 'linux' | 'android' | 'ios';

let currentPlatform: Platform = 'linux';

export async function detectPlatform(): Promise<Platform> {
	try {
		currentPlatform = (await platform()) as Platform;
	} catch {
		currentPlatform = 'linux'; // fallback
	}
	return currentPlatform;
}

export function getPlatform(): Platform {
	return currentPlatform;
}

export function isMobile(): boolean {
	return currentPlatform === 'android' || currentPlatform === 'ios';
}

export function isDesktop(): boolean {
	return !isMobile();
}
