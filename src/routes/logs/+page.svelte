<script lang="ts">
	import LogViewer from '$lib/components/LogViewer.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
</script>

<div class="min-h-screen bg-background text-foreground flex flex-col p-4 gap-4 pb-safe">
	<!-- Header with back button and theme toggle -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-3">
			<a
				href="/"
				class="p-1.5 rounded-md hover:bg-muted transition-colors"
				aria-label="Back to dashboard"
			>
				<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<path d="m15 18-6-6 6-6" />
				</svg>
			</a>
			<h1 class="text-lg font-bold tracking-widest uppercase text-foreground/90">Logs</h1>
		</div>
		<ThemeToggle />
	</div>

	<!-- Log viewer fills remaining space -->
	<div class="flex-1 min-h-0">
		<LogViewer />
	</div>
</div>
