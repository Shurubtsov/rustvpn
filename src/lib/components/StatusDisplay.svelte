<script lang="ts">
	import { cn } from '$lib/utils';
	import { formatSpeed, formatBytes } from '$lib/utils/format';
	import type { ConnectionInfo } from '$lib/types';

	interface Props {
		info: ConnectionInfo;
		elapsedSeconds: number;
		uploadSpeed: number;
		downloadSpeed: number;
		totalUpload: number;
		totalDownload: number;
	}

	const { info, elapsedSeconds, uploadSpeed, downloadSpeed, totalUpload, totalDownload }: Props = $props();

	const statusLabel = $derived(() => {
		switch (info.status) {
			case 'connected':
				return 'Connected';
			case 'connecting':
				return 'Connecting...';
			case 'disconnecting':
				return 'Disconnecting...';
			case 'error':
				return 'Error';
			default:
				return 'Disconnected';
		}
	});

	const statusColor = $derived(() => {
		switch (info.status) {
			case 'connected':
				return 'bg-green-500';
			case 'connecting':
			case 'disconnecting':
				return 'bg-yellow-500';
			case 'error':
				return 'bg-red-500';
			default:
				return 'bg-zinc-500';
		}
	});

	function formatElapsed(seconds: number): string {
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		const s = seconds % 60;
		return [h, m, s].map((v) => String(v).padStart(2, '0')).join(':');
	}
</script>

<!-- Status dot + label -->
<div class="flex items-center gap-2">
	<span class={cn('w-2.5 h-2.5 rounded-full', statusColor())}></span>
	<span class="text-sm font-medium text-foreground/80">{statusLabel()}</span>
</div>

<!-- Connection timer -->
{#if info.status === 'connected'}
	<div class="text-center">
		<p class="text-3xl font-mono font-light text-foreground/70 tracking-widest">
			{formatElapsed(elapsedSeconds)}
		</p>
		<p class="text-xs text-muted-foreground mt-1">Connected</p>
	</div>
{/if}

<!-- Server info -->
{#if info.server_name || info.server_address}
	<div class="w-full bg-card rounded-lg border border-border p-4 flex flex-col gap-2">
		{#if info.server_name}
			<div class="flex justify-between text-sm">
				<span class="text-muted-foreground">Server</span>
				<span class="text-foreground font-medium">{info.server_name}</span>
			</div>
		{/if}
		{#if info.server_address}
			<div class="flex justify-between text-sm">
				<span class="text-muted-foreground">Address</span>
				<span class="text-foreground font-mono">{info.server_address}</span>
			</div>
		{/if}
	</div>
{/if}

<!-- Speed stats -->
{#if info.status === 'connected'}
	<div class="w-full grid grid-cols-2 gap-3" style="animation: fadeIn 0.3s ease-out">
		<div class="bg-card rounded-lg border border-border p-3 text-center">
			<p class="text-[10px] text-muted-foreground mb-1 uppercase tracking-wider">Download</p>
			<p class="text-lg font-mono font-medium text-green-500">{formatSpeed(downloadSpeed)}</p>
			<p class="text-[10px] text-muted-foreground mt-0.5">{formatBytes(totalDownload)}</p>
		</div>
		<div class="bg-card rounded-lg border border-border p-3 text-center">
			<p class="text-[10px] text-muted-foreground mb-1 uppercase tracking-wider">Upload</p>
			<p class="text-lg font-mono font-medium text-blue-500">{formatSpeed(uploadSpeed)}</p>
			<p class="text-[10px] text-muted-foreground mt-0.5">{formatBytes(totalUpload)}</p>
		</div>
	</div>
{/if}

<!-- Error message -->
{#if info.status === 'error' && info.error_message}
	<div class="w-full bg-destructive/10 border border-destructive/40 rounded-lg p-3">
		<p class="text-xs text-destructive text-center">{info.error_message}</p>
	</div>
{/if}
