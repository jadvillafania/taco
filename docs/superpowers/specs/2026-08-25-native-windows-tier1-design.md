# Native Windows Tier 1 (M4) — Design

**Date:** 2026-08-25
**Status:** Approved (design review in chat)
**Amends:** `docs/product-spec-v2.md` §15 — Tier 1 becomes host-agnostic; WSL-first
wording is superseded. §15.4 rejections stand unchanged.

## Problem

Taco assumes WSL. On a machine running Claude Code natively (PowerShell/cmd,
no WSL):

- `list_sessions()` shells `wsl.exe -l -q` unconditionally and probes
  `\\wsl$` UNC paths — slow, and the dropdown ends up empty.
- `send_capture` hard-fails any capture path that doesn't map to `/mnt/c/...`.
- Payloads carry Linux paths a native `claude` can't read.

Known bugs folded into this work:

- `parse_wsl_list` treats `wsl.exe` error text as distro names (no
  `status.success()` check) — `sessions.rs`.
- `send_capture` hard-fails non-`/mnt/c`-convertible paths instead of falling
  to Tier 2 — `commands.rs`.

## Decision summary

Full Tier 1 parity on native Windows. Not a "mode": a machine with both WSL
and native sessions sees one merged, ranked session list. Decisions made:

- **IPC:** `uds_windows` crate (AF_UNIX on Win10 1803+). Spike verified
  v1.2.1 on this machine: `bind`/`incoming`/`connect`/`try_clone`/
  `set_read_timeout`/`read_line` all work — the exact API `relay.rs` and
  `client.rs` already use. Linux socket code is unchanged.
- **Wrapping:** marker-delimited `function claude { ... }` block in the
  PowerShell `$PROFILE` (CurrentUserAllHosts, pwsh 7 and WinPS 5.1 where
  present) — exact mirror of the `.bashrc` alias: consented, reversible.
  cmd.exe users fall to Tier 2 (documented limitation).
- **One shim crate**, `#[cfg]` seams only — no second shim.

## 1. Session model

`Session` gains a host; `distro` moves inside it:

```rust
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "host", rename_all = "lowercase")]
pub enum Host {
    Wsl { distro: String },
    Windows,
}

pub struct Session {
    pub sid: String,
    pub host: Host,
    pub project: String,
    pub cwd: String,
}
```

Discovery merges two sources:

- **Native:** read `%LOCALAPPDATA%\DeveloperVisualCompanion\run\*.json`
  directly (flat dir, no uid layer, no subprocess). Always scanned — it's a
  cheap local read.
- **WSL:** today's `wsl.exe -l -q` + `\\wsl$\<distro>\run\user\*\dvc\*.json`
  scan, **gated on a new `wsl_connected: bool` setting** (default `false`,
  flipped `true` by WSL shim install, `false` by removal). This gate is what
  removes the slow UNC probes and startup stall on non-WSL machines.
  `parse_wsl_list` gains the `status.success()` check.

`rank_sessions` is unchanged; focused-window ranking naturally disambiguates
the same project open on both hosts. Composer rows read `my-app · Ubuntu` /
`my-app · Windows`.

## 2. Shim: one crate, cfg seams

`portable-pty` already supports ConPTY, so `relay::run`'s spawn / relay
threads / idle-gate / inject logic is untouched. The seams:

| Concern | Unix (today) | Windows |
|---|---|---|
| `runtime_dir()` | `$XDG_RUNTIME_DIR/dvc` | `%LOCALAPPDATA%\DeveloperVisualCompanion\run` |
| Socket types | `std::os::unix::net` | `uds_windows` (same API, import swap) |
| Raw terminal | `nix` termios `RawGuard` | `SetConsoleMode`: raw stdin (`ENABLE_VIRTUAL_TERMINAL_INPUT`, clear line/echo/processed) + `ENABLE_VIRTUAL_TERMINAL_PROCESSING` on stdout; restore on drop |
| Resize | SIGWINCH via `signal-hook` | **deleted on both platforms** — poll `terminal_size` every 500 ms, resize PTY on change. One code path, one less dep; 500 ms resize lag is acceptable. |

Registry JSON: `distro` becomes `""`/absent on Windows (serde default keeps
old files parseable); `socket` field keeps holding the AF_UNIX path on both.

Client (`client.rs`): import swap only.

## 3. Delivery: one shape, host-varied command line

The app does **not** learn to speak the socket protocol. `tier1::send_via_shim`
takes `&Host` and varies only the spawned command:

- `Wsl { distro }` → `wsl.exe -d <distro> -- sh -lc '"$HOME/.local/share/dvc/dvc-shim" send --session <sid>'` (unchanged)
- `Windows` → `%LOCALAPPDATA%\DeveloperVisualCompanion\bin\dvc-shim.exe send --session <sid>`

Same stdin payload, same ack/error parse, `CREATE_NO_WINDOW` on both, zero
new app dependencies.

**Path mapping is per-target:**

- WSL target → `to_wsl_path` (`/mnt/c/...`). A path that won't convert now
  **falls back to Tier 2 with Windows paths** instead of erroring (bug fix).
- Windows target → the capture's own path with forward slashes
  (`C:/Users/...`) — quoting-safe in every shell and accepted by Windows APIs.

**Tier 2 with no session selected:** payload uses WSL paths when
`wsl_connected`, else Windows paths. This single branch is where "the
default" lives.

## 4. Install & packaging

`deployer` gains the Windows twin:

- `install_windows()`: copy bundled `dvc-shim.exe` →
  `%LOCALAPPDATA%\DeveloperVisualCompanion\bin\`, append marker-delimited
  block to `$PROFILE.CurrentUserAllHosts` for each installed PowerShell
  (pwsh 7: `~\Documents\PowerShell\profile.ps1`; WinPS 5.1:
  `~\Documents\WindowsPowerShell\profile.ps1`):

  ```powershell
  # >>> dvc-shim >>>
  function claude { & "$env:LOCALAPPDATA\DeveloperVisualCompanion\bin\dvc-shim.exe" run claude @args }
  # <<< dvc-shim <<<
  ```

- `remove_windows()`: strip marker blocks, delete the exe.
- Settings UI: two independent consent rows — "Windows (native)" and "WSL"
  — each install/remove on its own. WSL row also drives `wsl_connected`.
- Existing open shells need a restart to pick up the function — same caveat
  as the `.bashrc` alias today, shown in the consent copy.

Build/packaging:

- `shim/Cargo.toml`: `nix`/`signal-hook` move under
  `[target.'cfg(unix)'.dependencies]`; `uds_windows` + `windows-sys` (console
  APIs) under `[target.'cfg(windows)'.dependencies]`; `signal-hook` deleted.
- Windows shim builds with the host toolchain (`cargo build --release` in
  `shim/` via interop) → `app/src-tauri/resources/dvc-shim.exe`; musl build
  unchanged. Both listed as Tauri resources.

## 5. Error handling

- Native send failure (dead socket, stale registry, user-typing) → same
  Tier 2 fallback path as WSL today; stale `run\*.json` cleaned up
  opportunistically when connect gets `NotFound`/refused.
- No PowerShell profile writable / user declines → Tier 2 still works;
  nothing hard-fails.
- Old registry files without `host`/`distro` fields parse via serde defaults.

## 6. Testing

- `wslpath`: native mapping (backslash→forward slash, no `/mnt` prefix).
- `payload`: host-aware path selection.
- `sessions`: flat native registry scan; `wsl_connected` gate short-circuits
  before any `wsl.exe` spawn; `parse_wsl_list` rejects error output.
- `deployer`: profile block append/strip idempotence (against temp files).
- `shim/tests/e2e.rs`: cfg'd to drive `cmd /c` on Windows, existing sh-based
  flow on Unix; run in both toolchains.
- Manual: real `claude` session in PowerShell, capture → Tier 1 inject;
  kill shim → Tier 2 fallback; mixed WSL+native list on this machine.

## 7. Milestone ordering

1. **M4.1** Shim cfg-split (runtime_dir, sockets, RawGuard, resize-poll) +
   e2e green on Windows and Linux.
2. **M4.2** Discovery merge: `Host` enum, native registry scan,
   `wsl_connected` gate, `parse_wsl_list` fix.
3. **M4.3** Send path: host-aware `send_via_shim`, per-target path mapping,
   Tier 2 path-mapping bug fix, composer host labels.
4. **M4.4** Deployer + Settings consent rows + packaging.
5. **M4.5** Spec §15 amendment + docs.

## Non-goals

- cmd.exe wrapping (no profile mechanism; Tier 2 covers it).
- Tray-launched sessions (escape hatch deferred; note in improvements.md).
- Any change to rejected approaches (§15.4) — the shim owns a real ConPTY,
  console input injection remains rejected.
