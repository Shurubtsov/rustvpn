# Svelte Development Guidelines

## Component Pattern (Svelte 5)
```svelte
<script lang="ts">
  import { connectionStore } from '$lib/stores/connection.svelte';

  let { variant = 'default' }: { variant?: string } = $props();
  let count = $state(0);
  let doubled = $derived(count * 2);

  $effect(() => {
    console.log('count changed:', count);
  });
</script>
```

## Store Pattern (Svelte 5 Runes)
```typescript
// src/lib/stores/connection.svelte.ts
class ConnectionStore {
  status = $state<'disconnected' | 'connecting' | 'connected'>('disconnected');
  speed = $state({ up: 0, down: 0 });

  get isConnected() {
    return this.status === 'connected';
  }
}

export const connectionStore = new ConnectionStore();
```

## Tauri IPC Wrapper
```typescript
// src/lib/api/tauri.ts
import { invoke } from '@tauri-apps/api/core';

export async function connect(serverId: string): Promise<void> {
  return invoke('connect', { serverId });
}
```
