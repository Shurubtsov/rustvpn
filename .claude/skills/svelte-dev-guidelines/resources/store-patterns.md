# Store Patterns (Svelte 5)

## Class-based Store with Runes
```typescript
class ServerStore {
  servers = $state<ServerProfile[]>([]);
  selectedId = $state<string | null>(null);

  get selected() {
    return this.servers.find(s => s.id === this.selectedId) ?? null;
  }

  async load() {
    this.servers = await tauriApi.getServers();
  }
}
```

## Store Initialization
Initialize stores in root +layout.svelte using $effect.
