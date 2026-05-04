<script lang="ts">
	import { requestIgnoreBatteryOptimization, openOemBackgroundSettings } from '$lib/api/tauri';

	interface Props {
		// Whether battery optimization is currently ignored. Drives the styling of
		// step 1 (already-done vs needs-action).
		batteryOptIgnored: boolean;
		onClose: (dismissForever: boolean) => void;
		// Re-fetch battery-opt status from the backend after the system dialog
		// closes so the parent can update the indicator without a full refresh.
		onBatteryOptChanged: (granted: boolean) => void;
	}

	const { batteryOptIgnored, onClose, onBatteryOptChanged }: Props = $props();

	let dontShowAgain = $state(false);
	let busy = $state(false);
	let oemMessage = $state<string | null>(null);

	async function handleBatteryOpt() {
		if (busy) return;
		busy = true;
		try {
			const granted = await requestIgnoreBatteryOptimization();
			onBatteryOptChanged(granted);
		} finally {
			busy = false;
		}
	}

	async function handleOemSettings() {
		if (busy) return;
		busy = true;
		oemMessage = null;
		try {
			const result = await openOemBackgroundSettings();
			if (!result.opened) {
				oemMessage = "Couldn't open OEM settings — open Settings → Apps → RustVPN manually.";
			} else if (result.fallback) {
				oemMessage =
					'Opened generic app settings. Find "Battery" or "Auto-launch" in the list.';
			} else {
				oemMessage = 'Opened OEM background-activity settings.';
			}
		} catch (e) {
			oemMessage = `Failed: ${e}`;
		} finally {
			busy = false;
		}
	}

	function handleClose() {
		onClose(dontShowAgain);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') handleClose();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!--
	Backdrop is a real <button> so the click-to-close interaction has correct
	semantics (focusable, announced as a button, keyboard-activatable) without
	tripping Svelte's a11y_no_static_element_interactions lint. The keydown
	handler lives on <svelte:window> only — Escape is global, not scoped to the
	backdrop, and the duplicate per-element listener was redundant.
-->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
	<button
		type="button"
		class="absolute inset-0 w-full h-full cursor-default"
		aria-label="Close dialog"
		onclick={handleClose}
	></button>
	<div
		class="relative w-full max-w-md bg-card border border-border rounded-xl shadow-2xl overflow-hidden"
		role="dialog"
		aria-modal="true"
		aria-label="Background mode setup"
		tabindex="-1"
	>
		<div class="px-5 py-4 border-b border-border">
			<h2 class="text-base font-semibold text-foreground">Keep VPN alive in background</h2>
			<p class="text-xs text-muted-foreground mt-1">
				Some Android phones (Realme, Xiaomi, Huawei) kill the VPN when you swipe the app from
				recents. Two settings help RustVPN survive that.
			</p>
		</div>

		<div class="px-5 py-4 flex flex-col gap-4">
			<!-- Step 1: Battery optimization -->
			<div class="flex flex-col gap-2">
				<div class="flex items-center gap-2">
					{#if batteryOptIgnored}
						<span
							class="inline-flex items-center justify-center w-5 h-5 rounded-full bg-emerald-600/20 text-emerald-500"
							aria-label="Done"
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								width="12"
								height="12"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="3"
								stroke-linecap="round"
								stroke-linejoin="round"
							>
								<polyline points="20 6 9 17 4 12" />
							</svg>
						</span>
					{:else}
						<span
							class="inline-flex items-center justify-center w-5 h-5 rounded-full bg-zinc-700 text-foreground text-[10px] font-bold"
						>
							1
						</span>
					{/if}
					<h3 class="text-sm font-medium text-foreground">Disable battery optimization</h3>
				</div>
				<p class="text-xs text-muted-foreground pl-7">
					Without this, Android Doze will eventually throttle the VPN process even when its
					notification is showing.
				</p>
				<button
					type="button"
					onclick={handleBatteryOpt}
					disabled={busy || batteryOptIgnored}
					class="self-start ml-7 px-3 py-1.5 rounded-md bg-zinc-700 hover:bg-zinc-600 text-xs font-medium text-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
				>
					{batteryOptIgnored ? 'Already exempt' : 'Open battery settings'}
				</button>
			</div>

			<!-- Step 2: OEM background activity -->
			<div class="flex flex-col gap-2 pt-1">
				<div class="flex items-center gap-2">
					<span
						class="inline-flex items-center justify-center w-5 h-5 rounded-full bg-zinc-700 text-foreground text-[10px] font-bold"
					>
						2
					</span>
					<h3 class="text-sm font-medium text-foreground">Allow background activity / auto-launch</h3>
				</div>
				<p class="text-xs text-muted-foreground pl-7">
					On Realme/Xiaomi/Huawei this is a separate proprietary toggle. The button below opens it
					directly when possible, otherwise the generic app-info screen.
				</p>
				<button
					type="button"
					onclick={handleOemSettings}
					disabled={busy}
					class="self-start ml-7 px-3 py-1.5 rounded-md bg-zinc-700 hover:bg-zinc-600 text-xs font-medium text-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
				>
					Open OEM settings
				</button>
				{#if oemMessage}
					<p class="text-[11px] text-muted-foreground pl-7">{oemMessage}</p>
				{/if}
			</div>
		</div>

		<div class="px-5 py-4 border-t border-border flex items-center justify-between gap-3">
			<label class="flex items-center gap-2 text-xs text-muted-foreground cursor-pointer">
				<input
					type="checkbox"
					bind:checked={dontShowAgain}
					class="rounded border-border"
				/>
				Don't show again
			</label>
			<button
				type="button"
				onclick={handleClose}
				class="px-4 py-1.5 rounded-md bg-zinc-700 hover:bg-zinc-600 text-xs font-medium text-foreground transition-colors"
			>
				Continue
			</button>
		</div>
	</div>
</div>
