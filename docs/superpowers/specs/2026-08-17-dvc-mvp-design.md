# Developer Visual Companion — MVP Technical Design

**Date:** 2026-08-17
**Status:** Approved
**Product spec:** `docs/product-spec-v2.md` (source of truth for requirements; this doc covers implementation)
**Delivery:** Approach A — walking skeleton. Tier 2 first (M1), Tier 1 shim second (M2), P1 extras after (M3).

## Development environment

- Repo lives on the Windows filesystem: `C:\Users\jvillafania\dev\claude_companion`
  (WSL view: `/mnt/c/Users/jvillafania/dev/claude_companion`). Windows toolchains
  fail on `\\wsl$` UNC paths, so the repo must not live in the WSL filesystem.
- Windows toolchain (installed via winget): rustup (MSVC host), Node LTS, pnpm.
- WSL toolchain (M2 only): rustup + `x86_64-unknown-linux-musl` target for the shim.
- Claude Code runs in WSL and edits files under `/mnt/c/...`; Windows builds run
  via interop: `powershell.exe -Command "cd C:\...\app; pnpm tauri dev"`.

## Repo layout

```
claude_companion/
├── CLAUDE.md
├── docs/
│   ├── product-spec-v2.md
│   └── superpowers/{specs,plans}/
├── app/                    # Tauri v2 Windows app
│   ├── src-tauri/          # Rust backend
│   └── src/                # Vue 3 + TypeScript + Vite frontend
└── shim/                   # M2: dvc-shim (Rust, musl-static Linux binary)
```

## Milestone 1 — Capture + Tier 2 clipboard-assist

Usable end-to-end on day one with zero WSL-side installation.

### Windows/webviews

The app has no main window; it lives in the system tray.

- **Overlay** — created on `Ctrl+Shift+Space`. Rust captures the monitor under
  the cursor *first* (frozen-frame), then shows the image in a fullscreen,
  borderless, always-on-top, undecorated webview with a dim layer and a
  drag-selection rectangle. Mouseup sends the rect to Rust.
  `// ponytail: monitor-under-cursor only; multi-monitor spanning later`
- **Composer** — small always-on-top window that remembers its last position
  (persisted across restarts) and defaults to centering on the primary
  monitor when there's no remembered position that's still on a connected
  monitor: capture preview, optional message input, quick-action buttons
  (spec §9 — plain string inserts into the message field), target label,
  Cancel / Send.
- **Tray menu** — Capture Region (hotkey label), Capture Screen (full-screen,
  skips overlay, opens composer directly), Exit.

### Rust backend

- Crates/plugins: `tauri` v2, `tauri-plugin-global-shortcut`,
  `tauri-plugin-notification`, `tauri-plugin-clipboard-manager`,
  `tauri-plugin-single-instance`, `xcap` (screen capture), `image` (crop + PNG).
- Capture storage: `%LOCALAPPDATA%\DeveloperVisualCompanion\captures\YYYY-MM-DD\capture-HHMMSS.png`.
- Retention: on startup, delete capture files older than 24 hours.
  `// ponytail: fixed 24h constant; settings UI when someone asks`
- Path mapping (pure function, unit-tested): `C:\Users\u\AppData\...` →
  `/mnt/c/Users/u/AppData/...` — lowercase drive letter, backslashes to
  forward slashes.
- Tier 2 send: clipboard text = message (or the spec §15.5 default
  instruction when empty) + `\n` + WSL path; then notification
  "Ready — paste into your Claude Code terminal".
- No `AgentAdapter` trait in M1 — one delivery path doesn't earn an
  abstraction. It is introduced in M2 when Tier 1 makes two.

### Data flow

```
Hotkey → capture monitor under cursor → overlay (frozen frame)
      → drag rect → crop + save PNG → composer
      → Send → map path → clipboard + notification → close composer
```

- Esc at any stage cancels and deletes the capture file.
- Send failure (clipboard write error) keeps the composer open with an error
  banner and Retry — the capture is never lost (spec §22).
- Capture failure → error notification.

## Milestone 2 — dvc-shim + Tier 1 socket delivery

One shim binary with two modes; this is the entire WSL bridge — no cross-OS
socket code exists anywhere.

### `dvc-shim run claude [args...]` (wrapper mode)

- openpty; spawn `claude` on the child side; put the real terminal in raw
  mode; relay stdin/stdout transparently; forward SIGWINCH window resizes.
  Must be invisible: identical TUI behavior to running `claude` directly.
- Listen on `$XDG_RUNTIME_DIR/dvc/<sid>.sock` (line-delimited JSON, spec §27.3:
  `send` → `ack`/`error`).
- Write registry file `$XDG_RUNTIME_DIR/dvc/<sid>.json`:
  `{ pid, cwd, distro, project, socket, started_at }` (distro from
  `$WSL_DISTRO_NAME`). Remove socket + registry on exit.
- Injection semantics (spec §27.4): write message text to the PTY, short
  delay, then `\r` separately. Gate on an input-idle window (no user
  keystrokes for ~1s); if not reached within a timeout, reply `error` — the
  companion falls back to Tier 2.

### `dvc-shim send --session <sid>` (client mode)

Connects to the session socket, writes the `send` line from stdin, prints the
`ack`/`error` line, exits nonzero on error/timeout. The Windows app invokes it
as `wsl.exe -d <distro> ~/.local/share/dvc/dvc-shim send --session <sid>`.

### App-side additions

- **SessionManager**: list sessions by reading `\\wsl$\<distro>\...\dvc\*.json`
  registry files; liveness = socket file still present + pid alive.
- **Tier selection per send**: shim-managed session → Tier 1; ack required
  before reporting success; anything else → Tier 2 fallback. A send never
  hard-fails while Tier 2 is possible (spec §15.2).
- **Shim deployer** (explicit consent, reversible): copy the musl binary to
  `~/.local/share/dvc/dvc-shim` via `\\wsl$`, append
  `alias claude='~/.local/share/dvc/dvc-shim run claude'` to the shell rc.
  Removal deletes both.
- Session selector UI in the composer (auto-select single session; picker for
  multiple; spec §13's focused-window matching is M3).

## Milestone 3 — P1 extras (planned when picked up)

Clipboard image capture (`Ctrl+Shift+V`), active-window capture, automatic
session selection by focused project, annotation, capture history, settings
UI, Windows startup. Deliberately unplanned now — each is independent and
gets scoped when wanted.

## Testing

Unit tests for logic that can silently break; manual smoke for UI.

- M1 Rust tests: path mapping, clipboard payload builder, retention sweep
  (against temp dirs).
- M2 Rust tests: JSONL protocol framing, registry lifecycle, and a PTY relay
  test against a fake child (`cat`) — send bytes through, assert transparent
  relay and injection ordering (text, delay, `\r`).
- Manual smoke per milestone: hotkey → region → composer → send → paste (M1)
  / instant appearance (M2) in a real Claude Code session.

## Rejected / deferred (do not revisit; spec §15.4)

Windows console input injection, headless sidecar (`claude -p --resume`),
clipboard image injection into WSL. Multi-monitor spanning, settings UI, and
Tier 3 hooks inbox are deferred, not rejected.
