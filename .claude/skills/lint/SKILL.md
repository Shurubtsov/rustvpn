# /lint Skill

Run all linters for RustVPN.

## Steps

1. **Rust** (must be clean — CI gate):
   ```bash
   cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings
   ```
2. **Svelte / TypeScript**:
   ```bash
   pnpm check         # svelte-check type checking
   pnpm lint          # ESLint
   pnpm format --check # Prettier
   ```
3. Report any issues found, grouped by tool.

If `cargo clippy` flags anything, fix the underlying issue rather than `#[allow(...)]`-ing it (CLAUDE.md: "Keep clippy clean with no warnings").
