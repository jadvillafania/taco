# Native Windows Tier 1 (M4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Full Tier 1 (shim socket injection) parity for Claude Code running natively on Windows, merged with the existing WSL flow into one host-tagged session list.

**Architecture:** The single `shim/` crate grows `#[cfg]` seams (runtime dir, AF_UNIX via `uds_windows`, console raw mode, resize polling) so the same relay/inject logic runs under ConPTY. The app gains a `Host` enum on `Session`; discovery merges a flat native registry with the (now gated) `\\wsl$` scan; `send_via_shim` and path mapping become host-aware; the deployer gets a PowerShell-`$PROFILE` twin of the `.bashrc` install.

**Tech Stack:** Rust (Tauri 2 app on Windows; shim crate built twice: musl in WSL + host toolchain on Windows), `uds_windows` 1.2.1, `windows-sys` (console APIs), Vue 3 + TS frontend.

**Spec:** `docs/superpowers/specs/2026-08-25-native-windows-tier1-design.md`

## Global Constraints

- Repo lives at `C:\Users\jvillafania\dev\claude_companion` (`/mnt/c/...` from WSL). Never move it into WSL.
- App builds/tests run **on Windows via interop**: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test"`.
- Shim Linux build/tests run **in WSL**: `cd /mnt/c/Users/jvillafania/dev/claude_companion/shim && cargo test`. Shim Windows tests run via interop: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\shim; cargo test"`.
- After changing `shim/`, rebuild BOTH resources: musl binary (`cargo build --release --target x86_64-unknown-linux-musl` in WSL, copy to `app/src-tauri/resources/dvc-shim`) and, from Task 12 on, the Windows exe (see Task 12).
- Tiered delivery invariant: a send must never hard-fail while Tier 2 (clipboard) is possible.
- Deliberate shortcuts get a `// ponytail:` comment naming the ceiling and upgrade path.
- Rejected approaches (spec §15.4) must not appear: no console input injection, no headless sidecar, no clipboard image into WSL.
- Native registry/bin dirs: `%LOCALAPPDATA%\DeveloperVisualCompanion\run\` and `...\bin\`. Marker lines for profile edits: `# >>> dvc-shim >>>` / `# <<< dvc-shim <<<` (same markers as `.bashrc`).
- Commit messages end with:
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`

---

### Task 1: Shim — replace SIGWINCH with a resize poll (both platforms)

SIGWINCH is Unix-only; a 500 ms poll of `terminal_size` is one code path for both platforms and deletes a dependency. Done first, on Linux, so later Windows tasks never touch signal code.

**Files:**
- Modify: `shim/src/relay.rs` (SIGWINCH block, ~lines 139–149)
- Modify: `shim/Cargo.toml` (remove `signal-hook`)

**Interfaces:**
- Consumes: existing `term_size() -> PtySize`, `master: Arc<Mutex<Box<dyn MasterPty + Send>>>`.
- Produces: no API change; behavior change only (resize latency ≤ ~500 ms).

- [ ] **Step 1: Replace the SIGWINCH thread with a poll thread**

In `shim/src/relay.rs`, delete this block:

```rust
    // SIGWINCH -> pty resize
    {
        let master = master.clone();
        if let Ok(mut signals) = signal_hook::iterator::Signals::new([signal_hook::consts::SIGWINCH]) {
            std::thread::spawn(move || {
                for _ in signals.forever() {
                    master.lock().unwrap().resize(term_size()).ok();
                }
            });
        }
    }
```

and put in its place:

```rust
    // terminal resize -> pty resize
    // ponytail: 500ms size poll instead of SIGWINCH/console events — one code path
    // for unix+windows; event-driven resize if the lag ever bothers anyone.
    {
        let master = master.clone();
        std::thread::spawn(move || {
            let mut prev = term_size();
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let now = term_size();
                if now.rows != prev.rows || now.cols != prev.cols {
                    master.lock().unwrap().resize(now).ok();
                    prev = now;
                }
            }
        });
    }
```

- [ ] **Step 2: Remove the dependency**

In `shim/Cargo.toml` delete the line `signal-hook = "0.4.4"`.

- [ ] **Step 3: Verify existing suite still passes (this is the test for this task — pure refactor, behavior covered by e2e)**

Run (in WSL): `cd /mnt/c/Users/jvillafania/dev/claude_companion/shim && cargo test`
Expected: all existing unit + e2e tests PASS; `signal_hook` no longer in `cargo tree`.

- [ ] **Step 4: Commit**

```bash
git add shim/src/relay.rs shim/Cargo.toml shim/Cargo.lock
git commit -m "refactor(shim): poll terminal size instead of SIGWINCH"
```

---

### Task 2: Shim — cfg seams: `sockets` module, Windows `runtime_dir`, target-specific deps

**Files:**
- Create: `shim/src/sockets.rs`
- Modify: `shim/src/main.rs` (add `mod sockets;`)
- Modify: `shim/src/registry.rs` (`runtime_dir`)
- Modify: `shim/src/relay.rs`, `shim/src/client.rs` (socket imports)
- Modify: `shim/Cargo.toml` (target-specific dependency tables)
- Test: existing tests in `shim/src/registry.rs`

**Interfaces:**
- Produces: `crate::sockets::{UnixListener, UnixStream}` — the only socket import allowed anywhere in the crate from now on. `registry::runtime_dir()` on Windows returns `%LOCALAPPDATA%\DeveloperVisualCompanion\run` (env override `DVC_RUNTIME_DIR` still wins on both platforms).

- [ ] **Step 1: Write the failing test (Windows branch of `runtime_dir`)**

Append to the `tests` module in `shim/src/registry.rs`:

```rust
    #[test]
    #[cfg(windows)]
    fn runtime_dir_uses_localappdata_on_windows() {
        unsafe { std::env::remove_var("DVC_RUNTIME_DIR") };
        let d = runtime_dir();
        let want = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap())
            .join("DeveloperVisualCompanion").join("run");
        assert_eq!(d, want);
    }
```

- [ ] **Step 2: Restructure `shim/Cargo.toml` dependencies**

Replace the current `[dependencies]` block with:

```toml
[dependencies]
portable-pty = "0.9.0"
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
terminal_size = "0.4.4"

[target.'cfg(unix)'.dependencies]
nix = { version = "0.31.3", features = ["term", "user"] }

[target.'cfg(windows)'.dependencies]
uds_windows = "1.2.1"
windows-sys = { version = "0.60", features = ["Win32_System_Console", "Win32_Foundation"] }
```

- [ ] **Step 3: Create `shim/src/sockets.rs` and switch imports**

```rust
//! One import site for AF_UNIX sockets: std on unix, uds_windows on Windows
//! (AF_UNIX is native on Win10 1803+; the crate mirrors std's API).
#[cfg(unix)]
pub use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
pub use uds_windows::{UnixListener, UnixStream};
```

Add `mod sockets;` to `shim/src/main.rs`. In `shim/src/relay.rs` replace `use std::os::unix::net::UnixListener;` with `use crate::sockets::UnixListener;` (and the `std::os::unix::net::UnixStream` in `handle_conn`'s signature with `crate::sockets::UnixStream`). In `shim/src/client.rs` replace `use std::os::unix::net::UnixStream;` with `use crate::sockets::UnixStream;`.

- [ ] **Step 4: Make `runtime_dir` cfg-aware**

In `shim/src/registry.rs`:

```rust
pub fn runtime_dir() -> PathBuf {
    if let Ok(d) = std::env::var("DVC_RUNTIME_DIR") {
        return PathBuf::from(d);
    }
    #[cfg(unix)]
    {
        if let Ok(x) = std::env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(x).join("dvc");
        }
        std::env::temp_dir().join(format!("dvc-{}", nix::unistd::getuid()))
    }
    #[cfg(windows)]
    {
        match std::env::var("LOCALAPPDATA") {
            Ok(l) => PathBuf::from(l).join("DeveloperVisualCompanion").join("run"),
            Err(_) => std::env::temp_dir().join("dvc"),
        }
    }
}
```

- [ ] **Step 5: Verify Linux still green; Windows check reaches RawGuard only**

Run (WSL): `cargo test` — expected: PASS.
Run (interop): `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\shim; cargo check 2>&1 | Select-Object -Last 20"`
Expected: FAILS, and every error is in `relay.rs`'s `RawGuard` (`nix` not found) — nothing socket- or registry-related. That's Task 3's job.

- [ ] **Step 6: Commit**

```bash
git add shim/src/sockets.rs shim/src/main.rs shim/src/registry.rs shim/src/relay.rs shim/src/client.rs shim/Cargo.toml shim/Cargo.lock
git commit -m "feat(shim): cfg seams — sockets module, Windows runtime_dir, target deps"
```

---

### Task 3: Shim — Windows `RawGuard` (console raw mode); Windows build green

**Files:**
- Modify: `shim/src/relay.rs` (RawGuard, ~lines 17–41)

**Interfaces:**
- Produces: `RawGuard::new()` / `Drop` with identical semantics on both platforms: best-effort raw mode, restore on drop, silently a no-op when stdio isn't a terminal (tests, pipes).

- [ ] **Step 1: cfg-gate the existing RawGuard and add the Windows twin**

Wrap the current `RawGuard` struct + impls in `#[cfg(unix)]`. Below it add:

```rust
#[cfg(windows)]
struct RawGuard {
    stdin_orig: Option<u32>,
    stdout_orig: Option<u32>,
}

#[cfg(windows)]
impl RawGuard {
    fn new() -> Self {
        use windows_sys::Win32::System::Console::*;
        unsafe fn set(handle_id: u32, f: impl Fn(u32) -> u32) -> Option<u32> {
            use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
            unsafe {
                let h = GetStdHandle(handle_id);
                if h == INVALID_HANDLE_VALUE || h.is_null() { return None; }
                let mut mode = 0u32;
                if GetConsoleMode(h, &mut mode) == 0 { return None; } // not a console (tests, pipes)
                if SetConsoleMode(h, f(mode)) == 0 { return None; }
                Some(mode)
            }
        }
        unsafe {
            RawGuard {
                // raw stdin: no line buffering/echo/ctrl-c cooking; VT input sequences on
                stdin_orig: set(STD_INPUT_HANDLE, |m| {
                    (m & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT))
                        | ENABLE_VIRTUAL_TERMINAL_INPUT
                }),
                // VT output so the child's escape sequences render
                stdout_orig: set(STD_OUTPUT_HANDLE, |m| {
                    m | ENABLE_VIRTUAL_TERMINAL_PROCESSING | ENABLE_PROCESSED_OUTPUT
                }),
            }
        }
    }
}

#[cfg(windows)]
impl Drop for RawGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Console::*;
        unsafe {
            if let Some(m) = self.stdin_orig { SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), m); }
            if let Some(m) = self.stdout_orig { SetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), m); }
        }
    }
}
```

- [ ] **Step 2: Both platforms compile; unit tests pass on both**

Run (WSL): `cargo test` — PASS.
Run (interop): `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\shim; cargo test --lib --bins 2>&1 | Select-Object -Last 15"`
Expected: build succeeds; unit tests (protocol, registry incl. the Task 2 Windows test, client arg parsing if any) PASS. e2e is Task 4.

If `windows-sys` 0.60 renames a constant, `cargo doc`/compiler errors name the right one — fix the import, don't downgrade the crate.

- [ ] **Step 3: Commit**

```bash
git add shim/src/relay.rs
git commit -m "feat(shim): Windows console raw-mode RawGuard; shim builds on Windows"
```

---

### Task 4: Shim — e2e suite cross-platform, green on Windows

**Files:**
- Modify: `shim/tests/e2e.rs`

**Interfaces:**
- Consumes: `crate::sockets` doesn't exist in integration tests — the test file needs its own cfg'd socket import.
- Produces: e2e helpers `spawn_echo(tmp) -> Child`, `send_line(stdin, s)`, `end_input(stdin)` used by all four tests.

- [ ] **Step 1: Make the helpers platform-aware**

At the top of `shim/tests/e2e.rs`, replace `use std::os::unix::net::UnixStream;` with:

```rust
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use uds_windows::UnixStream;
```

Rename `spawn_cat` to `spawn_echo` (update the three call sites) and cfg the child:

```rust
pub fn spawn_echo(tmp: &std::path::Path) -> Child {
    // an echoing child: cat on unix; `cmd /c more` on windows (conhost echoes
    // cooked input anyway, so read_until sees the text either way)
    #[cfg(unix)]
    let args: &[&str] = &["run", "cat"];
    #[cfg(windows)]
    let args: &[&str] = &["run", "cmd", "/c", "more"];
    Command::new(env!("CARGO_BIN_EXE_dvc-shim"))
        .args(args)
        .env("DVC_RUNTIME_DIR", tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
}

pub fn send_line(stdin: &mut impl Write, s: &str) {
    stdin.write_all(s.as_bytes()).unwrap();
    stdin.write_all(b"\r\n").unwrap(); // \r\n is a plain newline to cat, EOL to conhost
    stdin.flush().unwrap();
}

pub fn end_input(stdin: &mut impl Write) {
    // unix: flush any partial canonical-mode line, then EOF
    #[cfg(unix)]
    { stdin.write_all(b"\n").ok(); stdin.write_all(&[0x04]).unwrap(); }
    // windows: Ctrl-Z at start of line + Enter = console EOF
    #[cfg(windows)]
    { stdin.write_all(b"\r\n\x1a\r\n").unwrap(); }
    stdin.flush().ok();
}
```

- [ ] **Step 2: Route the four tests through the helpers**

- `relays_io_and_manages_registry`: replace `stdin.write_all(b"hello\n")...flush` with `send_line(&mut stdin, "hello")`; replace the `0x04` EOF write with `end_input(&mut stdin)`.
- `injects_when_idle`: replace `child.stdin.take().unwrap().write_all(&[0x04])` with `end_input(&mut child.stdin.take().unwrap())`.
- `refuses_while_user_is_typing`: after `typing.join()`, replace the newline+`0x04` block with `end_input(&mut stdin)` (keep the existing comment context).
- `send_client_roundtrip_and_missing_session`: replace the final `0x04` write with `end_input(...)`; the client payload write (`b"from-client\n"`) is the shim-client's stdin (a pipe, closed on drop) — leave it as is.

- [ ] **Step 3: Run on Linux — must stay green**

Run (WSL): `cargo test` — Expected: PASS (behavioral no-op there).

- [ ] **Step 4: Run on Windows**

Run (interop): `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\shim; cargo test 2>&1 | Select-Object -Last 25"`
Expected: all tests PASS, including e2e (registry file + `.sock` created, injection acked, `user-typing` refusal, client exit codes 0/3).

Known-risk note for the executor: if `cmd /c more` misbehaves under ConPTY (pagination prompt, EOF ignored), swap the Windows child to `&["run", "findstr", "/r", "^"]` — same echo semantics, same `end_input` EOF. If EOF still doesn't end the child, report back with the observed output rather than force-killing the shim (the registry-cleanup assertions depend on a clean child exit).

- [ ] **Step 5: Commit**

```bash
git add shim/tests/e2e.rs
git commit -m "test(shim): e2e suite runs on Windows (ConPTY + uds_windows)"
```

---

### Task 5: App — `Host` enum, host-tagged `Session`, native flat registry scan

**Files:**
- Modify: `app/src-tauri/src/sessions.rs`

**Interfaces:**
- Produces (later tasks depend on these exact shapes):

```rust
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "host", rename_all = "lowercase")]
pub enum Host {
    Wsl { distro: String },
    Windows,
}

#[derive(Clone, serde::Serialize)]
pub struct Session {
    pub sid: String,
    #[serde(flatten)]
    pub host: Host,
    pub project: String,
    pub cwd: String,
}

pub fn sessions_in_flat_dir(dir: &Path) -> Vec<Session>;   // native registry: run\*.json
pub fn native_run_dir() -> Option<std::path::PathBuf>;      // %LOCALAPPDATA%\DeveloperVisualCompanion\run
```

Serialized `Session` JSON: `{"sid":"42","host":"wsl","distro":"Ubuntu","project":"p","cwd":"/x"}` or `{"sid":"7","host":"windows","project":"p","cwd":"C:\\x"}` — flat, which is what the frontend (Task 9) reads.

- [ ] **Step 1: Write the failing tests**

In `sessions.rs` tests module — update the `sess` helper and add:

```rust
    fn sess(project: &str) -> Session {
        Session {
            sid: project.into(),
            host: Host::Wsl { distro: "Ubuntu".into() },
            project: project.into(),
            cwd: format!("/home/j/{project}"),
        }
    }

    #[test]
    fn session_json_is_flat_and_host_tagged() {
        let v = serde_json::to_value(sess("p")).unwrap();
        assert_eq!(v["host"], "wsl");
        assert_eq!(v["distro"], "Ubuntu");
        let w = Session { host: Host::Windows, ..sess("p") };
        let v = serde_json::to_value(w).unwrap();
        assert_eq!(v["host"], "windows");
        assert!(v.get("distro").is_none());
    }

    #[test]
    fn scans_flat_native_registry() {
        let dir = std::env::temp_dir().join(format!("dvc-nat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("7.json"),
            r#"{"pid":7,"cwd":"C:\\Users\\j\\dev\\my-app","distro":"","project":"my-app","socket":"C:\\x\\7.sock","started_at":1}"#,
        ).unwrap();
        std::fs::write(dir.join("junk.txt"), "x").unwrap();
        let s = sessions_in_flat_dir(&dir);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].sid, "7");
        assert_eq!(s[0].host, Host::Windows);
        assert_eq!(s[0].project, "my-app");
        std::fs::remove_dir_all(&dir).ok();
    }
```

Also update the `scans_registry_layout` test's assertions if needed (`s[0].host` should be `Host::Wsl { distro: "Ubuntu".into() }` — add that assert).

- [ ] **Step 2: Run to verify failure**

Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test sessions 2>&1 | Select-Object -Last 15"`
Expected: FAIL to compile (`Host` not defined, `Session` has no `host` field).

- [ ] **Step 3: Implement**

In `sessions.rs`: add the `Host` enum and new `Session` exactly as in **Interfaces**. Extract the per-file parse shared by both scans:

```rust
fn session_from_file(p: &Path, host: Host) -> Option<Session> {
    if p.extension().map(|e| e != "json").unwrap_or(true) { return None; }
    let txt = std::fs::read_to_string(p).ok()?;
    let v = serde_json::from_str::<serde_json::Value>(&txt).ok()?;
    Some(Session {
        sid: p.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
        host,
        project: v["project"].as_str().unwrap_or("?").to_string(),
        cwd: v["cwd"].as_str().unwrap_or("").to_string(),
    })
}

pub fn sessions_in_flat_dir(dir: &Path) -> Vec<Session> {
    let Ok(files) = std::fs::read_dir(dir) else { return Vec::new() };
    files.flatten().filter_map(|f| session_from_file(&f.path(), Host::Windows)).collect()
}

pub fn native_run_dir() -> Option<std::path::PathBuf> {
    std::env::var("LOCALAPPDATA").ok()
        .map(|l| std::path::PathBuf::from(l).join("DeveloperVisualCompanion").join("run"))
}
```

Rewrite `sessions_under`'s inner loop to call `session_from_file(&p, Host::Wsl { distro: distro.to_string() })`. `list_sessions()` keeps compiling by constructing `Host::Wsl` (its gate changes in Task 6). `rank_sessions` and `foreground_*` are untouched.

- [ ] **Step 4: Run tests to verify they pass**

Run: same command as Step 2. Expected: all `sessions` tests PASS. Then full `cargo test` — other modules may not compile yet if they reference `Session.distro` (that's `commands.rs`/`tier1.rs`, fixed in Tasks 7–8); if so, add the minimal mechanical fixes there now (pattern-match `Host::Wsl { distro }`) WITHOUT changing behavior, and note it in the commit body.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/sessions.rs app/src-tauri/src/commands.rs app/src-tauri/src/tier1.rs
git commit -m "feat(app): Host enum on Session + flat native registry scan"
```

---

### Task 6: App — `wsl_connected` setting, gated WSL scan, `parse_wsl_list` status fix

**Files:**
- Modify: `app/src-tauri/src/settings.rs`
- Modify: `app/src-tauri/src/sessions.rs` (`list_sessions`, `list_sessions_cmd`)

**Interfaces:**
- Produces: `Settings.wsl_connected: bool` (default `false`); `list_sessions(wsl_connected: bool) -> Vec<Session>` — native scan always, WSL scan (including the `wsl.exe -l -q` spawn) only when `wsl_connected`. Task 11 flips the flag on shim install/remove.

- [ ] **Step 1: Write the failing tests**

`settings.rs` — extend `roundtrip_and_defaults`:

```rust
        // wsl_connected defaults to false and persists
        assert!(!load(&dir).wsl_connected);
        save(&dir, &Settings { wsl_connected: true, ..Default::default() }).unwrap();
        assert!(load(&dir).wsl_connected);
```

(Insert before the corrupt-file assertions; note the earlier saves in that test will need `..Default::default()` untouched — the struct gains a field, existing literal already uses `..Default::default()`.)

`sessions.rs`:

```rust
    #[test]
    fn wsl_scan_is_gated() {
        // gate off: must return instantly with no wsl.exe spawn — timing is the tell
        let t0 = std::time::Instant::now();
        let _ = list_sessions(false);
        assert!(t0.elapsed() < std::time::Duration::from_millis(200),
            "gated list_sessions must not touch wsl.exe / UNC paths");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test 2>&1 | Select-Object -Last 15"`
Expected: compile FAIL (`wsl_connected` field, `list_sessions` arity).

- [ ] **Step 3: Implement**

`settings.rs`: add `pub wsl_connected: bool` to the struct and `wsl_connected: false` to `Default` (serde `#[serde(default)]` on the struct already covers old files on disk).

`sessions.rs`:

```rust
pub fn list_sessions(wsl_connected: bool) -> Vec<Session> {
    let mut sessions: Vec<Session> = native_run_dir()
        .map(|d| sessions_in_flat_dir(&d))
        .unwrap_or_default();
    if wsl_connected {
        sessions.extend(wsl_sessions());
    }
    sessions
}

fn wsl_sessions() -> Vec<Session> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("wsl.exe")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else { return Vec::new() };
    if !out.status.success() {
        return Vec::new(); // error text is not a distro list (bug fix)
    }
    let mut sessions = Vec::new();
    for distro in parse_wsl_list(&out.stdout) {
        // stale registry files (crashed shims) surface here; Tier 1 send fails -> Tier 2 fallback
        let base = std::path::PathBuf::from(format!(r"\\wsl$\{distro}\run\user"));
        sessions.extend(sessions_under(&base, &distro));
    }
    sessions
}

#[tauri::command]
pub fn list_sessions_cmd(app: tauri::AppHandle, state: tauri::State<crate::capture::CaptureState>) -> Vec<Session> {
    let wsl = crate::settings::load(&crate::retention::data_dir(&app)).wsl_connected;
    let title = state.0.lock().unwrap().focus_title.clone();
    rank_sessions(list_sessions(wsl), title.as_deref())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: same command. Expected: PASS (including the timing test — on this machine WSL exists, so it proves the gate short-circuits, not that WSL is absent).

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/settings.rs app/src-tauri/src/sessions.rs
git commit -m "feat(app): gate WSL session scan behind wsl_connected; fix wsl.exe error-as-distro bug"
```

---

### Task 7: App — native path mapping + host-aware `send_via_shim`

**Files:**
- Modify: `app/src-tauri/src/wslpath.rs`
- Modify: `app/src-tauri/src/tier1.rs`
- Modify: `app/src-tauri/src/deployer.rs` (`native_bin_dir`/`native_shim_exe` helpers — Task 10 reuses them)

**Interfaces:**
- Produces:
  - `wslpath::to_native_path(win: &str) -> String` — backslashes → forward slashes, no other change.
  - `wslpath::payload_paths(paths: &[std::path::PathBuf], wsl: bool) -> (Vec<String>, bool)` — `(mapped, degraded)`; `wsl=true` maps via `to_wsl_path`, and if ANY path won't convert, returns ALL paths native with `degraded=true` (Tier 2 fallback signal). `wsl=false` always native, never degraded.
  - `tier1::send_via_shim(host: &crate::sessions::Host, sid: &str, payload: &str) -> Outcome`.
  - `deployer::native_bin_dir() -> PathBuf` (= `%LOCALAPPDATA%\DeveloperVisualCompanion\bin`), `deployer::native_shim_exe() -> PathBuf` (= `native_bin_dir().join("dvc-shim.exe")`).

- [ ] **Step 1: Write the failing tests**

`wslpath.rs` tests module:

```rust
    #[test]
    fn native_path_uses_forward_slashes() {
        assert_eq!(to_native_path(r"C:\Users\jv\cap.png"), "C:/Users/jv/cap.png");
    }

    #[test]
    fn payload_paths_maps_per_target() {
        use std::path::PathBuf;
        let paths = vec![PathBuf::from(r"C:\x\a.png"), PathBuf::from(r"C:\x\b.png")];
        assert_eq!(payload_paths(&paths, true), (vec!["/mnt/c/x/a.png".into(), "/mnt/c/x/b.png".into()], false));
        assert_eq!(payload_paths(&paths, false), (vec!["C:/x/a.png".into(), "C:/x/b.png".into()], false));
        // unconvertible path degrades the whole send to native paths (Tier 2)
        let unc = vec![PathBuf::from(r"C:\x\a.png"), PathBuf::from(r"\\wsl$\U\x\b.png")];
        let (mapped, degraded) = payload_paths(&unc, true);
        assert!(degraded);
        assert_eq!(mapped, vec!["C:/x/a.png".to_string(), "//wsl$/U/x/b.png".to_string()]);
    }
```

`tier1.rs` tests module:

```rust
    #[test]
    fn windows_host_send_with_missing_exe_is_rejected_not_panic() {
        // no shim installed at %LOCALAPPDATA%...\bin during unit tests
        match send_via_shim(&crate::sessions::Host::Windows, "1", "x") {
            Outcome::Rejected(r) => assert!(!r.is_empty()),
            Outcome::Ack => panic!("cannot ack without a shim"),
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test 2>&1 | Select-Object -Last 15"`
Expected: compile FAIL (`to_native_path`, `payload_paths`, `send_via_shim` signature).

- [ ] **Step 3: Implement**

`wslpath.rs`:

```rust
pub fn to_native_path(win: &str) -> String {
    win.replace('\\', "/")
}

/// (mapped paths, degraded): wsl=true converts to /mnt/...; any unconvertible
/// path degrades the whole send to native paths — caller must skip Tier 1.
pub fn payload_paths(paths: &[std::path::PathBuf], wsl: bool) -> (Vec<String>, bool) {
    if wsl {
        if let Some(v) = paths.iter().map(|p| to_wsl_path(&p.to_string_lossy())).collect::<Option<Vec<_>>>() {
            return (v, false);
        }
        return (paths.iter().map(|p| to_native_path(&p.to_string_lossy())).collect(), true);
    }
    (paths.iter().map(|p| to_native_path(&p.to_string_lossy())).collect(), false)
}
```

`deployer.rs` (top-level helpers):

```rust
pub fn native_bin_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into()))
        .join("DeveloperVisualCompanion").join("bin")
}

pub fn native_shim_exe() -> std::path::PathBuf {
    native_bin_dir().join("dvc-shim.exe")
}
```

`tier1.rs` — new signature; only the spawned command varies:

```rust
pub fn send_via_shim(host: &crate::sessions::Host, sid: &str, payload: &str) -> Outcome {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = match host {
        crate::sessions::Host::Wsl { distro } => {
            let script = format!(r#""$HOME/.local/share/dvc/dvc-shim" send --session {sid}"#);
            let mut c = std::process::Command::new("wsl.exe");
            c.args(["-d", distro, "--", "sh", "-lc", &script]);
            c
        }
        crate::sessions::Host::Windows => {
            let mut c = std::process::Command::new(crate::deployer::native_shim_exe());
            c.args(["send", "--session", sid]);
            c
        }
    };
    let child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    // ... rest identical to today: write payload to stdin, wait_with_output, parse_response
}
```

(Keep `parse_response` and its tests untouched.) Fix the one call site in `commands.rs` mechanically for now: `send_via_shim(&s.host, &s.sid, &payload)` — `TargetSession` gains `host` in Task 8; if that ordering hurts, do the minimal `TargetSession` field change here and say so in the commit body.

- [ ] **Step 4: Run tests to verify they pass**

Run: same command. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/wslpath.rs app/src-tauri/src/tier1.rs app/src-tauri/src/deployer.rs app/src-tauri/src/commands.rs
git commit -m "feat(app): per-target path mapping and host-aware Tier 1 send"
```

---

### Task 8: App — `send_capture` per-target payload + Tier 2 degradation (bug fix)

**Files:**
- Modify: `app/src-tauri/src/commands.rs` (`TargetSession`, `send_capture`)

**Interfaces:**
- Consumes: `payload_paths`, `send_via_shim(&Host, ...)`, `Settings.wsl_connected`.
- Produces: `TargetSession { sid: String, #[serde(flatten)] host: Host, project: String }` — frontend (Task 9) sends `{sid, host, distro?, project}` flat.

- [ ] **Step 1: Rewrite `TargetSession` and the top of `send_capture`**

```rust
#[derive(serde::Deserialize)]
pub struct TargetSession {
    pub sid: String,
    #[serde(flatten)]
    pub host: crate::sessions::Host,
    pub project: String,
}
```

In `send_capture`, replace the path-mapping block (the `wsl_paths` `collect::<Result<...>>`? line — the old hard-fail) and the Tier 1/Tier 2 flow with:

```rust
    let paths = state.0.lock().unwrap().captures.clone();
    if paths.is_empty() { return Err("no capture".into()); }
    let settings = crate::settings::load(&crate::retention::data_dir(&app));
    // target decides path flavor; with no session, wsl_connected is "the default"
    let want_wsl = match &session {
        Some(s) => matches!(s.host, crate::sessions::Host::Wsl { .. }),
        None => settings.wsl_connected,
    };
    let (mapped, degraded) = crate::wslpath::payload_paths(&paths, want_wsl);
    let payload = crate::payload::build_payload(message.as_deref(), &mapped, &settings.default_instruction);

    if let (Some(s), false) = (&session, degraded) {
        match crate::tier1::send_via_shim(&s.host, &s.sid, &payload) {
            crate::tier1::Outcome::Ack => {
                state.0.lock().unwrap().captures.clear();
                if let Some(w) = app.get_webview_window("composer") { w.close().ok(); }
                notify(&app, "Screenshot sent to Claude Code", &s.project); // spec §21 copy
                return Ok(());
            }
            crate::tier1::Outcome::Rejected(reason) => {
                // Tier 2 fallback below — a send never hard-fails while Tier 2 is possible
                let _ = reason;
            }
        }
    }
```

The clipboard tail stays as-is (payload now already holds the right flavor). Delete the old `default_instruction` load further down (it moved up). The old `let wsl_paths = ... .collect::<Result<Vec<_>, _>>()?;` hard-fail must be gone.

- [ ] **Step 2: Add a compile-level regression test for TargetSession's wire shape**

In `commands.rs` tests module:

```rust
    #[test]
    fn target_session_deserializes_both_hosts() {
        let w: TargetSession = serde_json::from_str(r#"{"sid":"7","host":"windows","project":"p"}"#).unwrap();
        assert_eq!(w.host, crate::sessions::Host::Windows);
        let u: TargetSession = serde_json::from_str(r#"{"sid":"42","host":"wsl","distro":"Ubuntu","project":"p"}"#).unwrap();
        assert_eq!(u.host, crate::sessions::Host::Wsl { distro: "Ubuntu".into() });
    }
```

- [ ] **Step 3: Run tests**

Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test 2>&1 | Select-Object -Last 15"`
Expected: PASS, full suite.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/commands.rs
git commit -m "feat(app): host-aware send_capture; unconvertible paths degrade to Tier 2 instead of failing"
```

---

### Task 9: Frontend — host-tagged sessions in the composer

**Files:**
- Modify: `app/src/windows/Composer.vue` (Session type ~line 18, send payload ~line 197–200, option markup ~line 252)

**Interfaces:**
- Consumes: flat `Session` JSON from `list_sessions_cmd` (`host: "wsl" | "windows"`, `distro` only when wsl); `send_capture`'s flat `TargetSession`.

- [ ] **Step 1: Update the type, the send payload, and the label**

```ts
type Session = { sid: string; host: "wsl" | "windows"; distro?: string; project: string; cwd: string };
```

Send call (`session:` field):

```ts
      session: s ? { sid: s.sid, host: s.host, distro: s.distro, project: s.project } : null,
```

(`distro: undefined` is dropped by JSON.stringify — the flat shape Task 8 deserializes.)

Option markup:

```html
      <option v-for="(s, i) in sessions" :key="s.host + s.sid" :value="String(i)">
        Claude Code: {{ s.project }} ({{ s.host === "wsl" ? s.distro : "Windows" }} · {{ s.cwd }})
      </option>
```

(`:key` gains the host prefix — a native and a WSL session can share a pid-derived sid.)

- [ ] **Step 2: Typecheck**

Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app; pnpm vue-tsc --noEmit 2>&1 | Select-Object -Last 10"`
Expected: no errors. (If `vue-tsc` isn't a script/dep, `pnpm build` exercises the same check.)

- [ ] **Step 3: Commit**

```bash
git add app/src/windows/Composer.vue
git commit -m "feat(ui): composer shows host-tagged sessions (Ubuntu vs Windows)"
```

---

### Task 10: App — Windows deployer (profile block append/strip, exe copy)

**Files:**
- Modify: `app/src-tauri/src/deployer.rs`

**Interfaces:**
- Produces:
  - `deployer::PROFILE_BLOCK: &str` — the exact marker-delimited function block.
  - `deployer::append_block(existing: &str) -> String` — idempotent append.
  - `deployer::strip_block(existing: &str) -> String` — removes the marker block, leaves everything else byte-identical.
  - `deployer::profile_paths() -> Vec<PathBuf>` — WinPS 5.1 profile always; pwsh 7 profile when `pwsh` is on PATH.
  - `deployer::install_windows(app: &tauri::AppHandle) -> Result<(), String>`, `deployer::remove_windows() -> Result<(), String>`.

- [ ] **Step 1: Write the failing tests (pure text functions — the file IO wrapper stays thin and untested)**

Add a tests module to `deployer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_is_idempotent_and_strip_restores() {
        let orig = "# my profile\nSet-Alias g git\n";
        let once = append_block(orig);
        assert!(once.contains("# >>> dvc-shim >>>"));
        assert!(once.contains(r#"function claude { & "$env:LOCALAPPDATA\DeveloperVisualCompanion\bin\dvc-shim.exe" run claude @args }"#));
        assert_eq!(append_block(&once), once, "second append is a no-op");
        assert_eq!(strip_block(&once), orig, "strip restores the original");
        assert_eq!(strip_block(orig), orig, "strip without block is a no-op");
    }

    #[test]
    fn append_to_empty_profile() {
        let s = append_block("");
        assert!(s.starts_with("# >>> dvc-shim >>>"));
        assert!(s.ends_with('\n'));
        assert_eq!(strip_block(&s), "");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test deployer 2>&1 | Select-Object -Last 10"`
Expected: compile FAIL (functions not defined).

- [ ] **Step 3: Implement**

```rust
pub const PROFILE_BLOCK: &str = concat!(
    "# >>> dvc-shim >>>\n",
    "function claude { & \"$env:LOCALAPPDATA\\DeveloperVisualCompanion\\bin\\dvc-shim.exe\" run claude @args }\n",
    "# <<< dvc-shim <<<\n",
);

pub fn append_block(existing: &str) -> String {
    if existing.contains(MARK_BEGIN) {
        return existing.to_string();
    }
    if existing.is_empty() {
        return PROFILE_BLOCK.to_string();
    }
    let sep = if existing.ends_with('\n') { "" } else { "\n" };
    format!("{existing}{sep}{PROFILE_BLOCK}")
}

pub fn strip_block(existing: &str) -> String {
    let (Some(b), Some(e)) = (existing.find(MARK_BEGIN), existing.find(MARK_END)) else {
        return existing.to_string();
    };
    let end = existing[e..].find('\n').map(|n| e + n + 1).unwrap_or(existing.len());
    format!("{}{}", &existing[..b], &existing[end..])
}

/// WinPS 5.1 profile always (present on every Windows); pwsh 7 profile when pwsh is installed.
/// ponytail: $HOME\Documents assumed — redirected Documents folders break this;
/// resolve via [Environment]::GetFolderPath if anyone hits it.
pub fn profile_paths() -> Vec<std::path::PathBuf> {
    let Ok(home) = std::env::var("USERPROFILE") else { return Vec::new() };
    let docs = std::path::PathBuf::from(home).join("Documents");
    let mut v = vec![docs.join("WindowsPowerShell").join("profile.ps1")];
    let has_pwsh = std::process::Command::new("where.exe").arg("pwsh")
        .creation_flags(CREATE_NO_WINDOW)
        .output().map(|o| o.status.success()).unwrap_or(false);
    if has_pwsh {
        v.push(docs.join("PowerShell").join("profile.ps1"));
    }
    v
}

pub fn install_windows(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let src = app.path()
        .resolve("resources/dvc-shim.exe", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let bin = native_bin_dir();
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    std::fs::copy(&src, native_shim_exe())
        .map_err(|e| format!("shim exe missing ({e}) — build shim/ on Windows first"))?;
    for p in profile_paths() {
        if let Some(dir) = p.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
        let existing = std::fs::read_to_string(&p).unwrap_or_default();
        std::fs::write(&p, append_block(&existing)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn remove_windows() -> Result<(), String> {
    for p in profile_paths() {
        if let Ok(existing) = std::fs::read_to_string(&p) {
            std::fs::write(&p, strip_block(&existing)).map_err(|e| e.to_string())?;
        }
    }
    std::fs::remove_file(native_shim_exe()).ok(); // absent exe is fine
    Ok(())
}
```

(`use std::os::windows::process::CommandExt;` and `CREATE_NO_WINDOW` already exist at the top of `deployer.rs`.)

- [ ] **Step 4: Run tests to verify they pass**

Run: same command as Step 2. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/deployer.rs
git commit -m "feat(app): Windows shim deployer — PowerShell profile block + exe copy"
```

---

### Task 11: Settings UI — two consent rows; tray items removed; `wsl_connected` flips

**Files:**
- Modify: `app/src-tauri/src/deployer.rs` (four new `#[tauri::command]`s)
- Modify: `app/src-tauri/src/lib.rs` (register commands; remove `install_shim`/`remove_shim` tray items + handlers)
- Modify: `app/src/windows/Settings.vue` (new "Instant delivery" section)

**Interfaces:**
- Produces commands (all `Result<(), String>`): `install_wsl_shim(app)`, `remove_wsl_shim(app)`, `install_native_shim(app)`, `remove_native_shim()`. WSL pair also writes `wsl_connected` true/false. The Settings row + its explanatory copy IS the consent surface (replaces the old blocking dialog).

- [ ] **Step 1: Add the commands to `deployer.rs`**

```rust
fn set_wsl_connected(app: &tauri::AppHandle, on: bool) -> Result<(), String> {
    let dir = crate::retention::data_dir(app);
    let mut s = crate::settings::load(&dir);
    s.wsl_connected = on;
    crate::settings::save(&dir, &s).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_wsl_shim(app: tauri::AppHandle) -> Result<(), String> {
    let d = default_distro()?;
    install(&app, &d)?;
    set_wsl_connected(&app, true)
}

#[tauri::command]
pub async fn remove_wsl_shim(app: tauri::AppHandle) -> Result<(), String> {
    let d = default_distro()?;
    remove(&d)?;
    set_wsl_connected(&app, false)
}

#[tauri::command]
pub async fn install_native_shim(app: tauri::AppHandle) -> Result<(), String> {
    install_windows(&app)
}

#[tauri::command]
pub async fn remove_native_shim() -> Result<(), String> {
    remove_windows()
}
```

- [ ] **Step 2: Wire `lib.rs`**

- Add to `generate_handler![...]`: `deployer::install_wsl_shim, deployer::remove_wsl_shim, deployer::install_native_shim, deployer::remove_native_shim`.
- Delete the `install_shim` and `remove_shim` `MenuItem::with_id` lines, remove `&install_shim, &remove_shim,` from `Menu::with_items` (keep `&autostart`), and delete both `"install_shim" => { ... }` and `"remove_shim" => { ... }` match arms (including the dialog code — consent moved to Settings).

- [ ] **Step 3: Add the Settings section**

In `Settings.vue` `<script setup>` add:

```ts
type ShimHost = "native" | "wsl";
const shimBusy = ref<ShimHost | "">("");
const shimMsg = ref<Partial<Record<ShimHost, string>>>({});

async function shimAction(host: ShimHost, action: "install" | "remove") {
  shimBusy.value = host;
  shimMsg.value = { ...shimMsg.value, [host]: "" };
  const cmd = `${action}_${host === "wsl" ? "wsl" : "native"}_shim`;
  try {
    await invoke(cmd);
    shimMsg.value = {
      ...shimMsg.value,
      [host]: action === "install"
        ? "Installed — restart your terminal, then run 'claude' as usual."
        : "Removed — wrapper and binary deleted.",
    };
  } catch (e) {
    shimMsg.value = { ...shimMsg.value, [host]: String(e) };
  } finally {
    shimBusy.value = "";
  }
}
```

In the template, after the existing hotkeys section (match surrounding markup/classes — reuse the section/label styles already in the file):

```html
    <div class="section">
      <label class="label">Instant delivery (Tier 1)</label>
      <div class="shim-row">
        <div class="shim-text">
          <strong>Windows (native)</strong>
          <small>Copies dvc-shim.exe into %LOCALAPPDATA% and adds a 'claude' function to your PowerShell profile. Reversible. cmd.exe sessions keep using clipboard delivery.</small>
        </div>
        <button :disabled="shimBusy !== ''" @click="shimAction('native', 'install')">Install</button>
        <button :disabled="shimBusy !== ''" @click="shimAction('native', 'remove')">Remove</button>
      </div>
      <p v-if="shimMsg.native" class="shim-msg">{{ shimMsg.native }}</p>
      <div class="shim-row">
        <div class="shim-text">
          <strong>WSL</strong>
          <small>Copies dvc-shim into your WSL distro (~/.local/share/dvc/) and adds a 'claude' alias to ~/.bashrc. Also enables WSL session discovery. Reversible.</small>
        </div>
        <button :disabled="shimBusy !== ''" @click="shimAction('wsl', 'install')">Install</button>
        <button :disabled="shimBusy !== ''" @click="shimAction('wsl', 'remove')">Remove</button>
      </div>
      <p v-if="shimMsg.wsl" class="shim-msg">{{ shimMsg.wsl }}</p>
    </div>
```

Add matching scoped styles (`.shim-row { display: flex; gap: 8px; align-items: center; }` etc. — follow the file's existing token usage). The Settings window is 420×460; if the new section overflows, bump the `inner_size` height in `lib.rs`'s settings-window builder to 620.

- [ ] **Step 4: Build + typecheck + manual smoke**

Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test"` → PASS.
Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app; pnpm vue-tsc --noEmit"` → clean.
Manual (leave for the milestone verify with John): tray no longer shows shim items; Settings shows both rows.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/deployer.rs app/src-tauri/src/lib.rs app/src/windows/Settings.vue
git commit -m "feat(app): shim install/remove consent rows in Settings for both hosts"
```

---

### Task 12: Packaging — Windows shim binary as a bundled resource

**Files:**
- Modify: `app/src-tauri/tauri.conf.json` (`resources` array, line 25)
- Create: `app/src-tauri/resources/dvc-shim.exe` (build artifact)
- Modify: `CLAUDE.md` (build instructions)

- [ ] **Step 1: Build the Windows shim and copy it into resources**

Run:
```bash
powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\shim; cargo build --release; Copy-Item target\release\dvc-shim.exe ..\app\src-tauri\resources\dvc-shim.exe"
```
Expected: exe exists at `app/src-tauri/resources/dvc-shim.exe`.

- [ ] **Step 2: Declare the resource**

In `app/src-tauri/tauri.conf.json`:

```json
    "resources": ["resources/dvc-shim", "resources/dvc-shim.exe"],
```

- [ ] **Step 3: Update CLAUDE.md's shim build note**

In the Development Environment section, extend the existing shim bullet: after the musl rebuild instruction, add:

```markdown
- The shim also builds **on Windows** (Tier 1 for native sessions):
  `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\shim; cargo build --release"`,
  then copy `shim\target\release\dvc-shim.exe` to `app\src-tauri\resources\dvc-shim.exe`.
  After changing `shim/`, rebuild BOTH resources.
```

- [ ] **Step 4: Verify the bundle picks it up**

Run: `powershell.exe -Command "cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo build 2>&1 | Select-Object -Last 5"`
Expected: builds clean (resource resolution is runtime; the build proves the conf parses).

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/tauri.conf.json CLAUDE.md
git commit -m "build: bundle dvc-shim.exe as a Tauri resource"
```

(`resources/dvc-shim.exe` follows whatever git policy the musl `resources/dvc-shim` already has — if that one is committed, commit the exe too; if ignored, extend the ignore rule.)

---

### Task 13: Docs — spec §15 amendment + improvements notes

**Files:**
- Modify: `docs/product-spec-v2.md` (§15)
- Modify: `docs/improvements.md`
- Modify: `CLAUDE.md` (architecture invariants wording)

- [ ] **Step 1: Amend the spec**

At the end of §15 in `docs/product-spec-v2.md`, add:

```markdown
### 15.6 Native Windows Tier 1 (M4 amendment)

Tier 1 is host-agnostic. `dvc-shim` builds for Linux (musl, WSL) and Windows
(ConPTY + AF_UNIX via uds_windows); sessions from both hosts appear in one
merged, ranked list, each tagged with its host. Path flavor follows the
target: `/mnt/c/...` for WSL sessions, `C:/...` for native ones; with no
session selected, the `wsl_connected` setting picks the flavor. The WSL
session scan (wsl.exe + \\wsl$ probes) only runs when `wsl_connected` is on
(flipped by WSL shim install/remove). §15.4 rejections stand unchanged —
the shim owns a real ConPTY; console input injection remains rejected.
Design: docs/superpowers/specs/2026-08-25-native-windows-tier1-design.md.
```

- [ ] **Step 2: Record deferred scope in `docs/improvements.md`**

Append (match the file's existing list format):

```markdown
- M4 deferred: cmd.exe has no profile mechanism — native Tier 1 covers
  PowerShell only; a tray "New Claude Code session" launcher would cover cmd
  users (rejected for now: only Taco-started sessions would be injectable).
- M4 upgrade note: `wsl_connected` defaults to false — existing WSL-shim users
  must reinstall the WSL shim (or the flag flips on their next install) before
  WSL sessions reappear in the composer.
- deployer assumes `$USERPROFILE\Documents` — redirected Documents folders
  break profile edits (ponytail comment in deployer.rs).
- M4 deferred: no opportunistic cleanup of stale native `run\*.json` (crashed
  shims) — stale entries surface in the list and fall to Tier 2 on send, same
  as stale WSL entries today; add a connect-probe sweep if the noise bothers.
```

- [ ] **Step 3: Update CLAUDE.md invariants**

In `CLAUDE.md` Architecture Invariants, replace the bullet
"Image transport is always a **file path** ... referenced by its `/mnt/c/...` path in the message. ..."
with:

```markdown
- Image transport is always a **file path**: PNG saved under
  `%LOCALAPPDATA%\DeveloperVisualCompanion\captures\`, referenced in the
  message by the path flavor of the target session — `/mnt/c/...` for WSL,
  `C:/...` for native Windows. Never clipboard-image into WSL.
```

and replace "All Windows↔WSL communication goes through `wsl.exe` invocations..." with:

```markdown
- WSL communication goes through `wsl.exe` invocations of `dvc-shim` or
  `\\wsl$` file reads — no cross-OS socket code. Native sessions talk to
  `dvc-shim.exe` the same way (spawn + stdin), never to the socket directly.
```

- [ ] **Step 4: Commit**

```bash
git add docs/product-spec-v2.md docs/improvements.md CLAUDE.md
git commit -m "docs: spec §15.6 native Tier 1 amendment; M4 deferred-scope notes"
```

---

### Task 14: Milestone verification (manual, with John)

No code. Run through on this machine (has both WSL and native-capable PowerShell):

- [ ] `pnpm tauri dev` → Settings shows both consent rows; tray has no shim items.
- [ ] Install native shim → new PowerShell window → `claude` starts via shim → session appears in composer as `(Windows · C:/...)`.
- [ ] Capture → send to the native session → text lands + Enter, ack notification.
- [ ] Type in the session while sending → "busy" → Tier 2 clipboard fallback with `C:/` paths.
- [ ] WSL row install → WSL session appears alongside; send still works; both hosts in one dropdown, focused-project ranking picks the right one.
- [ ] Remove both shims → profile/bashrc clean (markers gone), sessions list empty, Tier 2 default follows `wsl_connected`.

Report results; John merges on his verify.

---

## Self-review notes

- Spec coverage: §1→Tasks 5–6, §2→Tasks 1–4, §3→Tasks 7–9, §4→Tasks 10–12, §5 (error handling)→Tasks 7–8 (degraded fallback, Rejected paths; stale-registry opportunistic cleanup deliberately dropped — stale entries already fall to Tier 2 per existing comment, noted here rather than silently), §6→tests within each task + Task 14, §7 ordering preserved, non-goals→Task 13 improvements notes.
- Type consistency: `Host` defined once (Task 5), consumed by Tasks 6–9; `payload_paths` signature identical in Tasks 7 and 8; marker constants shared between bash and PowerShell installers.
