# Component Patterns

## Props with Defaults
```svelte
<script lang="ts">
  let { size = 'md', disabled = false }: { size?: 'sm' | 'md' | 'lg'; disabled?: boolean } = $props();
</script>
```

## Event Handling
```svelte
<button onclick={() => handleClick()}>Click</button>
```

## Conditional Classes (Tailwind)
```svelte
<div class="rounded-full {status === 'connected' ? 'bg-green-500' : 'bg-gray-500'}">
```
