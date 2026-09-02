# Taco 🌮

**Terminal Agent Context Optics** — a local-first Windows tray utility that captures
screenshots and injects them into an already-running Claude Code session, whether that
session runs inside WSL or natively on Windows in PowerShell.

Taco is not an AI app. No chat, no model, no backend, no account. It only feeds
visual context into the agent conversation you already have open.

<p align="center">
  <img src="docs/brand/taco-logo.png" width="200" alt="Taco logo">
</p>

## How it works

1. Press the global hotkey (default `Ctrl+Shift+Space`) → frozen-frame region
   select across every monitor, or capture the full screen / active window / a
   clipboard image from the tray.
2. A small composer window shows the capture: optionally annotate it, add a
   message, and pick the target session.
3. **Send.** The PNG is saved under
   `%LOCALAPPDATA%\DeveloperVisualCompanion\captures\` and its path is delivered
   into your Claude Code session as text, in the flavor that session can read —
   `/mnt/c/...` for WSL targets, `C:/...` for native Windows ones. Image
   transport is always a file path — never a clipboard image into WSL.

Delivery is tiered, evaluated per send:

- **Tier 1 — PTY shim.** `dvc-shim` wraps `claude`, owns the PTY, and accepts
  injection with an ack. It ships in two flavors: a static musl Linux binary for
  WSL and `dvc-shim.exe` for native Windows PowerShell. Either is installed only
  with your explicit consent and is fully reversible.
- **Tier 2 — Clipboard assist.** Path + message are copied to the clipboard and
  a notification asks you to paste. Universal fallback; a send never hard-fails
  while Tier 2 is possible.

## Features (v0.1.2)

- Region / full-screen / active-window / clipboard-image capture
- Multi-monitor region capture: every screen is frozen and overlaid, so a capture
  started from the tray can still select on a secondary monitor
- Composer with message input, quick-action buttons, and annotation
- Capture history with re-send
- Configurable hotkeys with collision probing
- Windows startup toggle, tray menu, About window
- 24-hour capture retention, swept on startup

### Windows-native support

Taco no longer assumes WSL. A Claude Code session started in Windows PowerShell
is a first-class target:

- **Native Tier 1.** Installing the Windows shim copies `dvc-shim.exe` into
  `%LOCALAPPDATA%\DeveloperVisualCompanion\bin\` and adds a `claude` function to
  your PowerShell profile (Windows PowerShell 5.1, plus pwsh 7 when present).
  Sessions started through it accept instant injection, same as WSL.
  `cmd.exe` has no profile to hook, so those sessions use Tier 2 clipboard assist.
- **Native session discovery.** Shim-wrapped PowerShell sessions register under
  `%LOCALAPPDATA%\DeveloperVisualCompanion\run\` and appear in the composer's
  session picker alongside WSL ones. Sessions whose process has exited are
  filtered out rather than lingering in the list.
- **Per-target path flavor.** The message carries `C:/...` for a native session
  and `/mnt/c/...` for a WSL one; a path that cannot be mapped degrades to Tier 2
  instead of failing the send.
- **WSL is opt-in.** Distro scanning and `\\wsl$` session discovery only run once
  the WSL shim is installed, so machines without WSL pay no cost for it.

Both shims are installed and removed from **Settings**, independently, and each
one reports whether it is currently installed. If your PowerShell execution
policy is `Restricted` or `AllSigned`, the install warns you — profile scripts
are disabled under those policies, so the wrapper would never load.

## Privacy

Local-first by design: screenshots never leave the machine except via your own
Send into your own terminal. No telemetry containing screenshot content, no
background OCR, no cloud storage.

## Repo layout

```
claude_companion/
├── app/          # Tauri v2 Windows app (Rust backend, Vue 3 + TS frontend)
├── shim/         # dvc-shim — one Rust source, built for both WSL (musl) and Windows
└── docs/         # product spec, technical design, implementation plans
```

## Development

The repo must live on the Windows filesystem (`C:\Users\<you>\dev\claude_companion`)
— Windows toolchains fail on `\\wsl$` UNC paths. Edit from WSL or Windows; the
app builds **on Windows**, the shim builds **in WSL**.

Prerequisites: rustup (MSVC host), Node LTS, pnpm on Windows; rustup with the
`x86_64-unknown-linux-musl` target in WSL.

```powershell
# app: dev server with hot reload
cd app; pnpm install; pnpm tauri dev

# app: Rust backend tests
cd app/src-tauri; cargo test

# app: release bundle (installer)
cd app; pnpm tauri build
```

```bash
# shim (WSL flavor): build in WSL, then refresh the bundled resource
cd shim
cargo build --release --target x86_64-unknown-linux-musl
cp target/x86_64-unknown-linux-musl/release/dvc-shim ../app/src-tauri/resources/dvc-shim
```

```powershell
# shim (Windows flavor): build on Windows, then refresh the bundled resource
cd shim; cargo build --release
copy target\release\dvc-shim.exe ..\app\src-tauri\resources\dvc-shim.exe
```

Both resources are bundled into the installer, so after changing `shim/` rebuild
**both** — otherwise one host keeps shipping a stale binary.

`pnpm tauri dev` opens real windows on the Windows desktop — tray, hotkeys, and
the capture overlay can only be verified there.

## Docs

- Product spec (requirements source of truth): `docs/product-spec-v2.md`
- Technical design + milestones: `docs/superpowers/specs/2026-08-17-dvc-mvp-design.md`
- Implementation plans: `docs/superpowers/plans/`
