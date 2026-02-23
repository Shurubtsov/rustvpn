<script lang="ts">
	import { cn } from '$lib/utils';
	import { isMobile } from '$lib/utils/platform';

	interface Props {
		status: 'disconnected' | 'connecting' | 'connected' | 'disconnecting' | 'error';
		isLoading: boolean;
		isTransitioning: boolean;
		isConnected: boolean;
		onclick: () => void;
	}

	const { status, isLoading, isTransitioning, isConnected, onclick }: Props = $props();

	const buttonLabel = $derived(() => {
		if (isLoading || isTransitioning) {
			return status === 'disconnecting' ? 'Disconnecting...' : 'Connecting...';
		}
		return isConnected ? 'Disconnect' : 'Connect';
	});

	const isDisabled = $derived(isLoading || isTransitioning);

	// On mobile, use a slightly larger button to ensure comfortable touch target
	const buttonSize = $derived(isMobile() ? 'w-40 h-40' : 'w-36 h-36');
</script>

<div class="relative">
	<!-- Connected glow ring -->
	{#if isConnected}
		<span
			class="absolute inset-0 rounded-full animate-[glow-pulse_2s_ease-in-out_infinite] pointer-events-none"
			style="box-shadow: 0 0 20px 4px rgba(34, 197, 94, 0.3), 0 0 40px 8px rgba(34, 197, 94, 0.1)"
		></span>
	{/if}

	<!-- Connecting/disconnecting ping -->
	{#if status === 'connecting' || status === 'disconnecting'}
		<span
			class="absolute inset-0 rounded-full border-4 border-yellow-500/40 animate-ping pointer-events-none"
		></span>
	{/if}

	<button
		{onclick}
		disabled={isDisabled}
		class={cn(
			'relative rounded-full border-4 font-semibold text-sm tracking-wide transition-all duration-500 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background',
			buttonSize,
			isConnected
				? 'border-green-500 text-green-400 hover:border-green-400 hover:text-green-300 active:scale-95'
				: status === 'error'
					? 'border-red-500 text-red-400 hover:border-red-400 hover:text-red-300 active:scale-95'
					: 'border-zinc-600 text-zinc-300 hover:border-zinc-400 hover:text-zinc-100 active:scale-95',
			isDisabled && 'opacity-60 cursor-not-allowed'
		)}
		aria-label={buttonLabel()}
	>
		{buttonLabel()}
	</button>
</div>
