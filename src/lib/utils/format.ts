export function formatSpeed(bytesPerSec: number): string {
	if (bytesPerSec < 1024) {
		return `${bytesPerSec} B/s`;
	} else if (bytesPerSec < 1024 * 1024) {
		return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
	} else if (bytesPerSec < 1024 * 1024 * 1024) {
		return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`;
	} else {
		return `${(bytesPerSec / (1024 * 1024 * 1024)).toFixed(2)} GB/s`;
	}
}

export function formatBytes(bytes: number): string {
	if (bytes < 1024) {
		return `${bytes} B`;
	} else if (bytes < 1024 * 1024) {
		return `${(bytes / 1024).toFixed(1)} KB`;
	} else if (bytes < 1024 * 1024 * 1024) {
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	} else {
		return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
	}
}
