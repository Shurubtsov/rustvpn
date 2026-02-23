<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { getLogs, clearLogs } from '$lib/api/tauri';
	import type { LogEntry } from '$lib/types';

	let logs = $state<LogEntry[]>([]);
	let search = $state('');
	let autoScroll = $state(true);
	let pollInterval: ReturnType<typeof setInterval> | null = null;
	let scrollContainer: HTMLDivElement;

	const filteredLogs = $derived(
		search.trim()
			? logs.filter((l) => l.message.toLowerCase().includes(search.toLowerCase()))
			: logs
	);

	async function fetchLogs() {
		try {
			logs = await getLogs();
			if (autoScroll && scrollContainer) {
				requestAnimationFrame(() => {
					scrollContainer.scrollTop = scrollContainer.scrollHeight;
				});
			}
		} catch {
			// Ignore
		}
	}

	async function handleClear() {
		try {
			await clearLogs();
			logs = [];
		} catch {
			// Ignore
		}
	}

	function formatTime(timestamp: number): string {
		const date = new Date(timestamp * 1000);
		return date.toLocaleTimeString('en-GB', {
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit'
		});
	}

	function levelColor(level: string): string {
		switch (level) {
			case 'error':
				return 'text-red-400';
			case 'warning':
				return 'text-yellow-400';
			default:
				return 'text-zinc-400';
		}
	}

	function levelBadge(level: string): string {
		switch (level) {
			case 'error':
				return 'bg-red-500/15 text-red-400';
			case 'warning':
				return 'bg-yellow-500/15 text-yellow-400';
			default:
				return 'bg-zinc-500/15 text-zinc-400';
		}
	}

	onMount(() => {
		fetchLogs();
		pollInterval = setInterval(fetchLogs, 2000);
	});

	onDestroy(() => {
		if (pollInterval !== null) {
			clearInterval(pollInterval);
		}
	});
</script>

<div class="flex flex-col h-full gap-3">
	<!-- Toolbar -->
	<div class="flex items-center gap-2">
		<input
			type="text"
			placeholder="Search logs..."
			bind:value={search}
			class="flex-1 bg-card border border-border rounded-md px-3 py-1.5 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
		/>
		<label class="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer">
			<input
				type="checkbox"
				bind:checked={autoScroll}
				class="rounded border-border"
			/>
			Auto-scroll
		</label>
		<button
			onclick={handleClear}
			class="px-3 py-1.5 text-xs bg-zinc-700 hover:bg-zinc-600 text-foreground rounded-md transition-colors"
		>
			Clear
		</button>
	</div>

	<!-- Log entries -->
	<div
		bind:this={scrollContainer}
		class="flex-1 overflow-y-auto bg-card border border-border rounded-lg p-2 font-mono text-xs leading-relaxed min-h-0"
	>
		{#if filteredLogs.length === 0}
			<p class="text-center text-muted-foreground py-8">No log entries</p>
		{:else}
			{#each filteredLogs as entry}
				<div class="flex gap-2 py-0.5 hover:bg-muted/30 px-1 rounded">
					<span class="text-muted-foreground shrink-0">{formatTime(entry.timestamp)}</span>
					<span class={`shrink-0 px-1.5 rounded text-[10px] uppercase font-semibold ${levelBadge(entry.level)}`}>
						{entry.level}
					</span>
					<span class={levelColor(entry.level)}>{entry.message}</span>
				</div>
			{/each}
		{/if}
	</div>
</div>
