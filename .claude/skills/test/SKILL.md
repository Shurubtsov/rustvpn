# /test Skill

Run all automated tests for RustVPN.

## Steps

1. **Rust unit tests**:
   ```bash
   cd src-tauri && cargo test
   ```
   Notable test suites: `config::tests` (xray config generation in proxy and TUN modes), `commands::tests` (validation), `uri::tests` (VLESS URI roundtrip).
2. **Frontend tests**: `pnpm test` if configured (currently the project relies on `pnpm check` for type-level safety only — flag if you add a test runner).
3. Report results summary.

There is no automated end-to-end test for `connect` / `disconnect` because they require a running xray sidecar and (on Linux) the privileged TUN helper. For UI/feature changes, run `pnpm tauri dev` and exercise the change manually before declaring success — `cargo test` and `pnpm check` verify code correctness, not feature correctness.
