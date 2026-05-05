# Svelte Development Guidelines

Svelte 5 runes only — no legacy `writable`/`readable` stores. TypeScript strict mode.

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

## Store Pattern (factory + `.svelte.ts`)

Stores live in `src/lib/stores/*.svelte.ts` (the `.svelte.ts` extension is required for `$state` / `$derived` to compile). Prefer a factory that returns an object with getters — this keeps the underlying `$state` reactive across module boundaries:

```typescript
// src/lib/stores/settings.svelte.ts
import { getSettings, updateSettings } from '$lib/api/tauri';
import type { AppSettings } from '$lib/types';

function createSettingsStore() {
  let settings = $state<AppSettings>({ auto_connect: false, last_server_id: null, bypass_domains: [] });
  let saveError = $state<string | null>(null);

  async function save(next: AppSettings): Promise<void> {
    const prev = settings;
    settings = next;
    try {
      await updateSettings(next);
      saveError = null;
    } catch (err) {
      // Roll local state back so it doesn't diverge from disk.
      settings = prev;
      saveError = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  return {
    get settings() { return settings; },
    get saveError() { return saveError; },
    save,
  };
}

export const settingsStore = createSettingsStore();
```

The current stores are `connectionStore`, `serversStore`, `settingsStore`.

## Tauri IPC Wrapper

Every `invoke()` call goes through `src/lib/api/tauri.ts` — components and stores must not call `invoke` directly. Argument names are camelCase on the JS side and Tauri auto-converts to snake_case for the Rust handler:

```typescript
// src/lib/api/tauri.ts
import { invoke } from '@tauri-apps/api/core';
import type { ServerConfig } from '$lib/types';

export async function connect(config: ServerConfig): Promise<void> {
  await invoke<void>('connect', { serverConfig: config });
}
```

Mirror the Rust struct shapes in `src/lib/types/index.ts` — keep field names in `snake_case` to match the serialized form.

## UI Primitives

Use shadcn-svelte primitives from `src/lib/components/ui/` (Button, Dialog, Input, ...) before introducing new component boilerplate. Tailwind utilities are merged via the `cn()` helper from `src/lib/utils/index.ts`.

## Routing

The app is a static SPA: `src/routes/+layout.ts` sets `prerender = true`, `ssr = false`. The two routes are `/` (main) and `/logs` (log viewer). New routes must keep the same flags or `pnpm tauri build` will fail.

## Style Rules

- Run `pnpm check` (svelte-check) before committing — must be clean.
- Keep components small; lift complex state into a store.
- For UI features, exercise the change in `pnpm tauri dev` — type checks catch shape errors but not behaviour regressions.
