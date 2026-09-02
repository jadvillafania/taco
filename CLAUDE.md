# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

**Developer Visual Companion**: a local-first Windows tray utility (Tauri v2 —
Rust backend, Vue 3 + TS frontend) that captures screenshots and injects them
into an already-running Claude Code session inside WSL. No AI of its own, no
backend, no accounts.

- Requirements source of truth: `docs/product-spec-v2.md`
- Technical design + milestones: `docs/superpowers/specs/2026-08-17-dvc-mvp-design.md`
- Implementation plans: `docs/superpowers/plans/`

Build order: **M1** capture + Tier 2 clipboard-assist → **M2** `dvc-shim` +
Tier 1 socket delivery → **M3** P1 extras. Rejected approaches (spec §15.4 —
console input injection, headless sidecar, clipboard image into WSL) must not
be re-proposed.

## Development Environment — read this first

Claude Code runs in **WSL**, but the app is a **Windows** binary. The split:

- The repo must stay on the Windows filesystem
  (`C:\Users\jvillafania\dev\claude_companion`, i.e. `/mnt/c/...` from WSL).
  Windows toolchains fail on `\\wsl$` UNC paths — never move it into WSL.
- Edit files from WSL normally. Build/run/test the app **on Windows via
  interop**, e.g.:

```bash
# dev server with hot reload (long-running)
powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app; pnpm tauri dev"

# Rust backend tests
powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test"

# single test
powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test path_mapping"

# frontend deps / typecheck
powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app; pnpm install"
```

- The shim (`shim/`, from M2) is the opposite: a Linux musl binary built **in
  WSL** with `cargo build --release --target x86_64-unknown-linux-musl`.
  After changing `shim/`, rebuild and re-copy the bundled resource:
  `cargo build --release --target x86_64-unknown-linux-musl && cp target/x86_64-unknown-linux-musl/release/dvc-shim ../app/src-tauri/resources/dvc-shim`
- The shim also builds **on Windows** (Tier 1 for native sessions):
  `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\shim; cargo build --release"`,
  then copy `shim\target\release\dvc-shim.exe` to `app\src-tauri\resources\dvc-shim.exe`.
  After changing `shim/`, rebuild BOTH resources.
- `tauri dev` opens real windows on the Windows desktop — UI behavior
  (tray, global hotkey, capture overlay) can only be verified there, not
  in WSL.
- New logo pipeline: the master lands in `docs/brand/taco-logo.png` (and
  `app/src/assets/taco.png` for the About window); pad it to a square 1024×1024
  PNG on a TRANSPARENT canvas (any tool — a one-off Rust `image`-crate bin works;
  no ImageMagick/PIL in this WSL) saved as `docs/brand/taco-icon-1024.png`, then
  `pnpm tauri icon docs/brand/taco-icon-1024.png`.
- After regenerating `app/src-tauri/icons/` (`pnpm tauri icon docs/brand/taco-icon-1024.png`),
  incremental builds do NOT reliably re-embed the icon (it's baked in during
  macro expansion) — run `cargo clean -p app` and rebuild, or the tray/window
  icons stay stale. The Windows icon cache is NOT the culprit for a live tray icon.

## Architecture Invariants

- Image transport is always a **file path**: PNG saved under
  `%LOCALAPPDATA%\DeveloperVisualCompanion\captures\`, referenced in the
  message by the path flavor of the target session — `/mnt/c/...` for WSL,
  `C:/...` for native Windows. Never clipboard-image into WSL.
- Tiered delivery per send: Tier 1 (shim socket, ack required) → Tier 2
  (clipboard-assist). A send must never hard-fail while Tier 2 is possible;
  a failed send keeps the capture and offers retry.
- WSL communication goes through `wsl.exe` invocations of `dvc-shim` or
  `\\wsl$` file reads — no cross-OS socket code. Native sessions talk to
  `dvc-shim.exe` the same way (spawn + stdin), never to the socket directly.
- Shim install/removal requires explicit user consent and is reversible.
- Screenshots are sensitive temporary data: 24h retention, no telemetry,
  no background OCR, nothing leaves the machine except via the user's Send.
