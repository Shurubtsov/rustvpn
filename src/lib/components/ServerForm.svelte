<script lang="ts">
	import { untrack } from 'svelte';
	import { cn } from '$lib/utils';
	import type { ServerConfig } from '$lib/types';

	interface Props {
		server?: ServerConfig | null;
		onSave: (server: ServerConfig) => void;
		onCancel: () => void;
	}

	const { server = null, onSave, onCancel }: Props = $props();

	const isEdit = $derived(server !== null);

	// Read the initial prop value without establishing a reactive dependency.
	// The form modal is always remounted when server changes, so we only need
	// the value at construction time.
	let name = $state(untrack(() => server?.name ?? ''));
	let address = $state(untrack(() => server?.address ?? ''));
	let port = $state(untrack(() => server?.port ?? 443));
	let uuid = $state(untrack(() => server?.uuid ?? ''));
	let flow = $state(untrack(() => server?.flow ?? 'xtls-rprx-vision'));
	let publicKey = $state(untrack(() => server?.reality.public_key ?? ''));
	let shortId = $state(untrack(() => server?.reality.short_id ?? ''));
	let serverName = $state(untrack(() => server?.reality.server_name ?? 'www.microsoft.com'));
	let fingerprint = $state(untrack(() => server?.reality.fingerprint ?? 'chrome'));

	let errors = $state<Record<string, string>>({});

	const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

	function validate(): boolean {
		const e: Record<string, string> = {};
		if (!address.trim()) e.address = 'Address is required';
		if (!UUID_RE.test(uuid.trim())) e.uuid = 'Invalid UUID format';
		if (port < 1 || port > 65535) e.port = 'Port must be between 1 and 65535';
		if (!publicKey.trim()) e.publicKey = 'Public key is required';
		if (!shortId.trim()) e.shortId = 'Short ID is required';
		if (!serverName.trim()) e.serverName = 'Server name is required';
		if (!fingerprint.trim()) e.fingerprint = 'Fingerprint is required';
		errors = e;
		return Object.keys(e).length === 0;
	}

	function handleSubmit(e: Event) {
		e.preventDefault();
		if (!validate()) return;
		onSave({
			id: server?.id ?? '',
			name: name.trim() || address.trim(),
			address: address.trim(),
			port,
			uuid: uuid.trim(),
			flow: flow.trim(),
			reality: {
				public_key: publicKey.trim(),
				short_id: shortId.trim(),
				server_name: serverName.trim(),
				fingerprint: fingerprint.trim()
			}
		});
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) onCancel();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onCancel();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Modal backdrop -->
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
		aria-label={isEdit ? 'Edit server' : 'Add server'}
		tabindex="-1"
	>
		<!-- Header -->
		<div class="flex items-center justify-between px-5 py-4 border-b border-border">
			<h2 class="text-base font-semibold text-foreground">
				{isEdit ? 'Edit Server' : 'Add Server'}
			</h2>
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

		<!-- Form -->
		<form onsubmit={handleSubmit} class="px-5 py-4 flex flex-col gap-4 max-h-[calc(100vh-8rem)] overflow-y-auto">

			<!-- Name -->
			<div class="flex flex-col gap-1">
				<label for="sf-name" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Name</label>
				<input
					id="sf-name"
					type="text"
					bind:value={name}
					placeholder="e.g. Finland VDS"
					class="w-full bg-background border border-border rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring"
				/>
			</div>

			<!-- Address -->
			<div class="flex flex-col gap-1">
				<label for="sf-address" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Address *</label>
				<input
					id="sf-address"
					type="text"
					bind:value={address}
					placeholder="e.g. 45.151.233.107"
					class={cn(
						'w-full bg-background border rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring',
						errors.address ? 'border-destructive' : 'border-border'
					)}
				/>
				{#if errors.address}<p class="text-xs text-destructive">{errors.address}</p>{/if}
			</div>

			<!-- Port + UUID row -->
			<div class="flex gap-3">
				<div class="flex flex-col gap-1 w-28 shrink-0">
					<label for="sf-port" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Port *</label>
					<input
						id="sf-port"
						type="number"
						bind:value={port}
						min="1"
						max="65535"
						class={cn(
							'w-full bg-background border rounded-lg px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring',
							errors.port ? 'border-destructive' : 'border-border'
						)}
					/>
					{#if errors.port}<p class="text-xs text-destructive">{errors.port}</p>{/if}
				</div>

				<div class="flex flex-col gap-1 flex-1">
					<label for="sf-uuid" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">UUID *</label>
					<input
						id="sf-uuid"
						type="text"
						bind:value={uuid}
						placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
						class={cn(
							'w-full bg-background border rounded-lg px-3 py-2 text-sm text-foreground font-mono placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring',
							errors.uuid ? 'border-destructive' : 'border-border'
						)}
					/>
					{#if errors.uuid}<p class="text-xs text-destructive">{errors.uuid}</p>{/if}
				</div>
			</div>

			<!-- Flow -->
			<div class="flex flex-col gap-1">
				<label for="sf-flow" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Flow</label>
				<input
					id="sf-flow"
					type="text"
					bind:value={flow}
					placeholder="xtls-rprx-vision"
					class="w-full bg-background border border-border rounded-lg px-3 py-2 text-sm text-foreground font-mono placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring"
				/>
			</div>

			<!-- REALITY section -->
			<div class="border border-border rounded-lg p-3 flex flex-col gap-3">
				<p class="text-xs font-semibold uppercase tracking-widest text-muted-foreground">REALITY Settings</p>

				<!-- Public key -->
				<div class="flex flex-col gap-1">
					<label for="sf-pubkey" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Public Key *</label>
					<input
						id="sf-pubkey"
						type="text"
						bind:value={publicKey}
						placeholder="Base64 public key"
						class={cn(
							'w-full bg-background border rounded-lg px-3 py-2 text-sm text-foreground font-mono placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring',
							errors.publicKey ? 'border-destructive' : 'border-border'
						)}
					/>
					{#if errors.publicKey}<p class="text-xs text-destructive">{errors.publicKey}</p>{/if}
				</div>

				<!-- Short ID -->
				<div class="flex flex-col gap-1">
					<label for="sf-shortid" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Short ID *</label>
					<input
						id="sf-shortid"
						type="text"
						bind:value={shortId}
						placeholder="e.g. d64736262cd50811"
						class={cn(
							'w-full bg-background border rounded-lg px-3 py-2 text-sm text-foreground font-mono placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring',
							errors.shortId ? 'border-destructive' : 'border-border'
						)}
					/>
					{#if errors.shortId}<p class="text-xs text-destructive">{errors.shortId}</p>{/if}
				</div>

				<!-- Server name (SNI) + Fingerprint row -->
				<div class="flex gap-3">
					<div class="flex flex-col gap-1 flex-1">
						<label for="sf-sni" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">SNI *</label>
						<input
							id="sf-sni"
							type="text"
							bind:value={serverName}
							placeholder="www.microsoft.com"
							class={cn(
								'w-full bg-background border rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring',
								errors.serverName ? 'border-destructive' : 'border-border'
							)}
						/>
						{#if errors.serverName}<p class="text-xs text-destructive">{errors.serverName}</p>{/if}
					</div>

					<div class="flex flex-col gap-1 w-28 shrink-0">
						<label for="sf-fp" class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Fingerprint *</label>
						<input
							id="sf-fp"
							type="text"
							bind:value={fingerprint}
							placeholder="chrome"
							class={cn(
								'w-full bg-background border rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring',
								errors.fingerprint ? 'border-destructive' : 'border-border'
							)}
						/>
						{#if errors.fingerprint}<p class="text-xs text-destructive">{errors.fingerprint}</p>{/if}
					</div>
				</div>
			</div>

			<!-- Actions -->
			<div class="flex gap-3 pt-1">
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
					{isEdit ? 'Save' : 'Add Server'}
				</button>
			</div>
		</form>
	</div>
</div>
