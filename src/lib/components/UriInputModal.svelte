<script lang="ts">
	interface Props {
		onImport: (uri: string) => void;
		onCancel: () => void;
	}

	const { onImport, onCancel }: Props = $props();

	let uri = $state('');
	let error = $state('');

	function handleSubmit(e: Event) {
		e.preventDefault();
		const trimmed = uri.trim();
		if (!trimmed) {
			error = 'Please enter a vless:// URI';
			return;
		}
		if (!trimmed.startsWith('vless://')) {
			error = 'URI must start with vless://';
			return;
		}
		onImport(trimmed);
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) onCancel();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onCancel();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
	onclick={handleBackdropClick}
	onkeydown={(e) => e.key === 'Escape' && onCancel()}
	role="presentation"
	tabindex="-1"
>
	<div
		class="w-full max-w-md mx-4 bg-card border border-border rounded-xl shadow-2xl overflow-hidden"
		role="dialog"
		aria-modal="true"
		aria-label="Import from vless:// URI"
		tabindex="-1"
	>
		<div class="flex items-center justify-between px-5 py-4 border-b border-border">
			<h2 class="text-base font-semibold text-foreground">Import from vless:// URI</h2>
			<button
				onclick={onCancel}
				class="text-muted-foreground hover:text-foreground transition-colors p-1 rounded hover:bg-accent"
				aria-label="Close"
			>
				<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<line x1="18" y1="6" x2="6" y2="18"/>
					<line x1="6" y1="6" x2="18" y2="18"/>
				</svg>
			</button>
		</div>

		<form onsubmit={handleSubmit} class="px-5 py-4 flex flex-col gap-4">
			<div class="flex flex-col gap-1">
				<label for="uri-input" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">
					vless:// URI
				</label>
				<textarea
					id="uri-input"
					bind:value={uri}
					placeholder="vless://UUID@address:port?params#name"
					rows="3"
					class="w-full bg-background border border-border rounded-lg px-3 py-2 text-sm text-foreground font-mono placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring resize-none"
					oninput={() => { error = ''; }}
				></textarea>
				{#if error}
					<p class="text-xs text-destructive">{error}</p>
				{/if}
			</div>

			<div class="flex gap-3">
				<button
					type="button"
					onclick={onCancel}
					class="flex-1 py-2 rounded-lg border border-border text-sm font-medium text-muted-foreground hover:text-foreground hover:border-zinc-500 transition-colors"
				>
					Cancel
				</button>
				<button
					type="submit"
					class="flex-1 py-2 rounded-lg bg-zinc-700 hover:bg-zinc-600 text-sm font-medium text-foreground transition-colors"
				>
					Import
				</button>
			</div>
		</form>
	</div>
</div>
