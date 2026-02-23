# Tauri IPC Patterns

## invoke() Wrapper
All Tauri calls go through src/lib/api/tauri.ts:
```typescript
import { invoke } from '@tauri-apps/api/core';
import type { ServerProfile, ConnectionStatus } from '$lib/types';

export const api = {
  connect: (serverId: string) => invoke<void>('connect', { serverId }),
  disconnect: () => invoke<void>('disconnect'),
  getStatus: () => invoke<ConnectionStatus>('get_status'),
  getServers: () => invoke<ServerProfile[]>('get_servers'),
};
```

## Event Listening
```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen<ConnectionStatus>('connection-status', (event) => {
  connectionStore.status = event.payload.status;
});
```
