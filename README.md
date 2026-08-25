# Taco 🌮

**Terminal Agent Context Optics** — a local-first Windows tray utility that captures
screenshots and injects them into an already-running Claude Code session inside WSL.

Taco is not an AI app. No chat, no model, no backend, no account. It only feeds
visual context into the agent conversation you already have open.

<p align="center">
  <img src="docs/brand/taco-logo.png" width="200" alt="Taco logo">
</p>

## How it works

1. Press the global hotkey (default `Ctrl+Shift+Space`) → frozen-frame region
   select, or capture the full screen / active window / a clipboard image from
   the tray.
2. A small composer window shows the capture: optionally annotate it, add a
   message, and pick quick actions.
3. **Send.** The PNG is saved under
   `%LOCALAPPDATA%\DeveloperVisualCompanion\captures\` and its WSL-visible
   `/mnt/c/...` path is delivered into your Claude Code session as text.
   Image transport is always a file path — never a clipboard image into WSL.

Delivery is tiered, evaluated per send:

- **Tier 1 — PTY shim.** `dvc-shim` is a static musl Linux binary (installed
  into WSL with your explicit consent, fully reversible) that wraps `claude`,
  owns the PTY, and accepts injection over a Unix socket with an ack.
- **Tier 2 — Clipboard assist.** Path + message are copied to the clipboard and
  a notification asks you to paste. Universal fallback; a send never hard-fails
  while Tier 2 is possible.

## Features (v0.1.1)

- Region / full-screen / active-window / clipboard-image capture
- Composer with message input, quick-action buttons, and annotation
- Capture history with re-send
- Configurable hotkeys with collision probing
- Windows startup toggle, tray menu, About window
- 24-hour capture retention, swept on startup

## Privacy

Local-first by design: screenshots never leave the machine except via your own
Send into your own terminal. No telemetry containing screenshot content, no
background OCR, no cloud storage.

## Repo layout

```
claude_companion/
├── app/          # Tauri v2 Windows app (Rust backend, Vue 3 + TS frontend)
├── shim/         # dvc-shim — Rust, musl-static Linux binary for WSL
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
# shim: build in WSL, then refresh the bundled resource
cd shim
cargo build --release --target x86_64-unknown-linux-musl
cp target/x86_64-unknown-linux-musl/release/dvc-shim ../app/src-tauri/resources/dvc-shim
```

`pnpm tauri dev` opens real windows on the Windows desktop — tray, hotkeys, and
the capture overlay can only be verified there.

## Docs

- Product spec (requirements source of truth): `docs/product-spec-v2.md`
- Technical design + milestones: `docs/superpowers/specs/2026-08-17-dvc-mvp-design.md`
- Implementation plans: `docs/superpowers/plans/`
