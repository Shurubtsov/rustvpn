<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { detectPlatform } from '$lib/utils/platform';

	let { children } = $props();

	onMount(async () => {
		const saved = localStorage.getItem('theme');
		if (saved === 'light') {
			document.documentElement.classList.remove('dark');
		}
		// 'dark' is already the default from app.html

		// Detect platform for responsive adaptations
		await detectPlatform();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}
