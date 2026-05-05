# /release Skill

Cut a release build of RustVPN.

## Pre-flight

CLAUDE.md mandates that the version must match in **all three** files before tagging:

- `package.json` → `"version": "x.y.z"`
- `src-tauri/Cargo.toml` → `version = "x.y.z"`
- `src-tauri/tauri.conf.json` → `"version": "x.y.z"`

Verify they agree before building. The git tag must be `v<x.y.z>`.

## Steps

1. **Run tests**:
   ```bash
   cd src-tauri && cargo test
   ```
2. **Run lints** (must be clean):
   ```bash
   cargo fmt --check && cargo clippy --all-targets -- -D warnings
   ```
3. **Build**:
   ```bash
   pnpm tauri build
   ```
4. **Report**: bundle path under `src-tauri/target/release/bundle/`, file size, and any warnings emitted during the build.
5. Update `CHANGELOG.md` with the new version + summary of changes since the previous tag.

For Android, the matching command is `NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973 pnpm tauri android build --apk` — outputs land in `src-tauri/gen/android/app/build/outputs/apk/`.
