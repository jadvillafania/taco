# M1 — Capture + Tier 2 Clipboard-Assist Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Windows tray app: hotkey → region capture → composer → PNG saved + WSL path & message on clipboard, ready to paste into Claude Code.

**Architecture:** Tauri v2 app with no main window — tray + two on-demand webviews (overlay, composer). Rust owns capture (xcap), cropping (image), storage, path mapping, clipboard, notifications. Vue is dumb UI. Frozen-frame capture: grab the monitor under the cursor first, show that PNG in the overlay, crop after selection.

**Tech Stack:** Tauri 2 (Rust, MSVC), Vue 3 + TypeScript + Vite (pnpm), xcap, image, chrono. Plugins: global-shortcut, notification, clipboard-manager, single-instance.

**Spec:** `docs/superpowers/specs/2026-08-17-dvc-mvp-design.md` (product requirements: `docs/product-spec-v2.md`)

## Global Constraints

- Repo lives at `C:\Users\jvillafania\dev\claude_companion` (`/mnt/c/Users/jvillafania/dev/claude_companion` from WSL). Never move it into `\\wsl$`.
- All Windows commands run from WSL via interop. **Every** `powershell.exe -Command` string MUST start with the PATH refresh below (interop shells don't see PATH changes made by installers). Shell env does NOT persist between tool invocations — define `PSP` in the SAME shell call that uses it, e.g.:

```bash
PSP='$env:Path=[Environment]::GetEnvironmentVariable("Path","Machine")+";"+[Environment]::GetEnvironmentVariable("Path","User");' && \
powershell.exe -Command "$PSP cargo --version"
```

(Every `powershell.exe -Command "$PSP ..."` snippet in this plan assumes that same-call definition.)

- App dir: `app/` (frontend), `app/src-tauri/` (Rust). Automated checks: `cargo test` / `cargo check` in `app/src-tauri`, `pnpm build` in `app/`. `pnpm tauri dev` is for manual smoke only (long-running; launch in background, stop with `powershell.exe -Command "Stop-Process -Name DeveloperVisualCompanion -Force"`).
- Captures: `%LOCALAPPDATA%\DeveloperVisualCompanion\captures\YYYY-MM-DD\capture-HHMMSS-mmm.png`. Frozen frames: `...\DeveloperVisualCompanion\frames\frame.png`.
- Default instruction when message empty (spec §15.5, verbatim): `Analyze this screenshot in the context of the current task.`
- Notification copy (spec §15.2, verbatim): `Ready — paste into your Claude Code terminal`
- Hotkey: `Ctrl+Shift+Space`. Esc cancels at any stage and deletes the capture. A failed send keeps the composer open with retry — the capture file is never deleted on failure (spec §22).
- Monitor under cursor only (`// ponytail: multi-monitor spanning later`).
- Commit after every task. Git identity: if `git config user.email` is empty, run `git config user.email "john.villafania@vokke.com.au" && git config user.name "John Villafania"` once.

---

### Task 1: Windows toolchain

**Files:** none (system setup only)

**Interfaces:**
- Consumes: nothing
- Produces: working `cargo` (MSVC), `node`, `pnpm` on Windows, callable via interop with the `$PSP` prefix

- [ ] **Step 1: Install Rust, VS Build Tools, Node**

```bash
powershell.exe -Command "winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements --silent"
powershell.exe -Command "winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements --accept-package-agreements --override '--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'"
powershell.exe -Command "winget install --id OpenJS.NodeJS.LTS -e --accept-source-agreements --accept-package-agreements --silent"
```

The BuildTools install takes 10–20 minutes; run it in the background and poll. If winget requires elevation, tell the user to run these three commands in an elevated Windows terminal and wait for their confirmation.

- [ ] **Step 2: Install pnpm, verify everything**

```bash
PSP='$env:Path=[Environment]::GetEnvironmentVariable("Path","Machine")+";"+[Environment]::GetEnvironmentVariable("Path","User");'
powershell.exe -Command "$PSP npm install -g pnpm"
powershell.exe -Command "$PSP rustup default stable-msvc; rustc --version; cargo --version; node --version; pnpm --version"
```

Expected: four version lines, rustc host `x86_64-pc-windows-msvc`. WebView2 is preinstalled on Windows 11 (this machine: build 26200) — no action.

---

### Task 2: Scaffold the Tauri app

**Files:**
- Create: `app/` (via create-tauri-app: `app/src/`, `app/src-tauri/`, configs)
- Modify: `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml`, `app/src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: Task 1 toolchain
- Produces: compiling app skeleton; conf: identifier `com.dvc.companion`, productName `DeveloperVisualCompanion`, `"windows": []`, asset protocol enabled for `$LOCALDATA/DeveloperVisualCompanion/**`

- [ ] **Step 1: Scaffold**

```bash
cd /mnt/c/Users/jvillafania/dev/claude_companion
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion; pnpm create tauri-app@latest app --template vue-ts --manager pnpm --yes"
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app; pnpm install"
```

- [ ] **Step 2: Configure**

In `app/src-tauri/tauri.conf.json` set:

```json
{
  "productName": "DeveloperVisualCompanion",
  "identifier": "com.dvc.companion",
  "app": {
    "windows": [],
    "security": {
      "csp": null,
      "assetProtocol": {
        "enable": true,
        "scope": ["$LOCALDATA/DeveloperVisualCompanion/**"]
      }
    }
  }
}
```

(keep the template's `build` and `bundle` sections as generated). In `app/src-tauri/capabilities/default.json` set `"windows": ["main", "overlay", "composer"]` and leave `"permissions": ["core:default", ...]` as generated.

- [ ] **Step 3: Add Rust dependencies**

```bash
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo add tauri-plugin-global-shortcut tauri-plugin-notification tauri-plugin-clipboard-manager tauri-plugin-single-instance xcap image chrono"
```

- [ ] **Step 4: Verify it compiles**

```bash
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo check"
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app; pnpm build"
```

Expected: both succeed (warnings fine).

- [ ] **Step 5: Commit**

```bash
cd /mnt/c/Users/jvillafania/dev/claude_companion && git add -A && git commit -m "feat: scaffold Tauri v2 + Vue app skeleton"
```

---

### Task 3: Tray, single instance, keep-alive

**Files:**
- Modify: `app/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 2 skeleton
- Produces: `run()` wiring that later tasks extend: a `setup` closure containing tray creation, and menu event ids `"capture_region"`, `"capture_screen"`, `"quit"` (region/screen handlers are stubs until Tasks 8–9)

- [ ] **Step 1: Replace `lib.rs`**

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, RunEvent,
};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let region = MenuItem::with_id(
                app, "capture_region", "Capture Region\tCtrl+Shift+Space", true, None::<&str>,
            )?;
            let screen = MenuItem::with_id(app, "capture_screen", "Capture Screen", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&region, &screen, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Developer Visual Companion")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "capture_region" => { /* Task 8 */ }
                    "capture_screen" => { /* Task 9 */ }
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building tauri app")
        .run(|_app, event| {
            // no persistent windows: keep the process alive when overlay/composer close,
            // but let app.exit(0) (code is Some) actually quit
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
```

Note: `tauri::generate_context!()` requires the `tray-icon` feature — Tauri v2 enables tray on Windows by default; if `TrayIconBuilder` is unresolved, add `tauri = { version = "2", features = ["tray-icon"] }`.

- [ ] **Step 2: Build + manual verify**

```bash
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo check"
```

Then launch `pnpm tauri dev` in the background and confirm with the user: tray icon present, Exit quits, launching a second instance does nothing. Stop the dev process.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: tray app skeleton with keep-alive and single instance"
```

---

### Task 4: WSL path mapping (TDD)

**Files:**
- Create: `app/src-tauri/src/wslpath.rs`
- Modify: `app/src-tauri/src/lib.rs` (add `mod wslpath;`)

**Interfaces:**
- Consumes: nothing
- Produces: `pub fn to_wsl_path(win: &str) -> Option<String>` — used by Task 9's `send_capture`

- [ ] **Step 1: Write failing tests** (`wslpath.rs` bottom)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_drive_path() {
        assert_eq!(
            to_wsl_path(r"C:\Users\jv\AppData\Local\x\cap.png").as_deref(),
            Some("/mnt/c/Users/jv/AppData/Local/x/cap.png")
        );
    }

    #[test]
    fn lowercases_drive_letter() {
        assert_eq!(to_wsl_path(r"D:\a.png").as_deref(), Some("/mnt/d/a.png"));
    }

    #[test]
    fn rejects_unc_and_relative() {
        assert_eq!(to_wsl_path(r"\\wsl$\Ubuntu\home\x.png"), None);
        assert_eq!(to_wsl_path(r"captures\x.png"), None);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test wslpath"`
Expected: FAIL — `to_wsl_path` not found.

- [ ] **Step 3: Implement**

```rust
/// C:\foo\bar -> /mnt/c/foo/bar. None for UNC/relative paths.
pub fn to_wsl_path(win: &str) -> Option<String> {
    let bytes = win.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || (bytes[2] != b'\\' && bytes[2] != b'/') {
        return None;
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    let rest = win[3..].replace('\\', "/");
    Some(format!("/mnt/{}/{}", drive, rest))
}
```

- [ ] **Step 4: Run to verify pass** — same command, expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: windows-to-wsl path mapping"
```

---

### Task 5: Clipboard payload builder (TDD)

**Files:**
- Create: `app/src-tauri/src/payload.rs`
- Modify: `app/src-tauri/src/lib.rs` (add `mod payload;`)

**Interfaces:**
- Consumes: nothing
- Produces: `pub fn build_payload(message: Option<&str>, wsl_path: &str) -> String` — used by Task 9

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    const P: &str = "/mnt/c/x/cap.png";

    #[test]
    fn message_then_path() {
        assert_eq!(build_payload(Some("Why misaligned?"), P), "Why misaligned?\n/mnt/c/x/cap.png");
    }

    #[test]
    fn default_instruction_when_empty_or_none() {
        let want = "Analyze this screenshot in the context of the current task.\n/mnt/c/x/cap.png";
        assert_eq!(build_payload(None, P), want);
        assert_eq!(build_payload(Some("   "), P), want);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test payload"`
Expected: FAIL — `build_payload` not found.

- [ ] **Step 3: Implement**

```rust
const DEFAULT_INSTRUCTION: &str = "Analyze this screenshot in the context of the current task.";

pub fn build_payload(message: Option<&str>, wsl_path: &str) -> String {
    let msg = message.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(DEFAULT_INSTRUCTION);
    format!("{}\n{}", msg, wsl_path)
}
```

- [ ] **Step 4: Run to verify pass** — same command, expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: clipboard payload builder with default instruction"
```

---

### Task 6: Retention sweep (TDD), wired at startup

**Files:**
- Create: `app/src-tauri/src/retention.rs`
- Modify: `app/src-tauri/src/lib.rs` (add `mod retention;`, call from `setup`)

**Interfaces:**
- Consumes: nothing
- Produces: `pub fn sweep(root: &std::path::Path, max_age: std::time::Duration, now: std::time::SystemTime) -> std::io::Result<usize>`; `pub fn data_dir(app: &tauri::AppHandle) -> std::path::PathBuf` (`%LOCALAPPDATA%\DeveloperVisualCompanion`) — Tasks 7 and 9 build capture paths from `data_dir`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn deletes_old_keeps_new_removes_empty_dirs() {
        let root = std::env::temp_dir().join(format!("dvc-ret-{}", std::process::id()));
        let day = root.join("2026-08-16");
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(day.join("old.png"), b"x").unwrap();
        let now_plus = SystemTime::now() + Duration::from_secs(25 * 3600);
        let deleted = sweep(&root, Duration::from_secs(24 * 3600), now_plus).unwrap();
        assert_eq!(deleted, 1);
        assert!(!day.exists(), "empty day dir removed");

        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(day.join("new.png"), b"x").unwrap();
        let deleted = sweep(&root, Duration::from_secs(24 * 3600), SystemTime::now()).unwrap();
        assert_eq!(deleted, 0);
        assert!(day.join("new.png").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_root_is_ok() {
        let r = std::env::temp_dir().join("dvc-ret-nonexistent");
        assert_eq!(sweep(&r, Duration::from_secs(1), SystemTime::now()).unwrap(), 0);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test retention"`
Expected: FAIL — `sweep` not found.

- [ ] **Step 3: Implement**

```rust
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Delete files under root (one level of day-dirs) older than max_age; drop empty day dirs.
pub fn sweep(root: &Path, max_age: Duration, now: SystemTime) -> std::io::Result<usize> {
    let mut deleted = 0;
    if !root.exists() {
        return Ok(0);
    }
    for day in std::fs::read_dir(root)? {
        let day = day?.path();
        if !day.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&day)? {
            let f = f?.path();
            let old = f
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .map(|age| age > max_age)
                .unwrap_or(false);
            if old && std::fs::remove_file(&f).is_ok() {
                deleted += 1;
            }
        }
        let _ = std::fs::remove_dir(&day); // fails if non-empty; that's fine
    }
    Ok(deleted)
}

pub fn data_dir(app: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    app.path()
        .local_data_dir()
        .expect("no local data dir")
        .join("DeveloperVisualCompanion")
}
```

- [ ] **Step 4: Run to verify pass** — same command, expected: 2 passed.

- [ ] **Step 5: Wire into startup** — in `lib.rs` `setup`, before tray creation:

```rust
let captures = crate::retention::data_dir(app.handle()).join("captures");
std::thread::spawn(move || {
    let _ = crate::retention::sweep(
        &captures,
        std::time::Duration::from_secs(24 * 3600),
        std::time::SystemTime::now(),
    ); // ponytail: fixed 24h retention; settings UI when someone asks
});
```

- [ ] **Step 6: Verify + commit**

```bash
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test"
git add -A && git commit -m "feat: 24h capture retention sweep on startup"
```

---

### Task 7: Capture module

**Files:**
- Create: `app/src-tauri/src/capture.rs`
- Modify: `app/src-tauri/src/lib.rs` (add `mod capture;`, register `CaptureState`)

**Interfaces:**
- Consumes: `retention::data_dir`
- Produces (used by Tasks 8–9):
  - `pub struct CaptureState(pub Mutex<Inner>)` with `Inner { frozen: Option<Frozen>, capture: Option<PathBuf> }`; `Frozen { image: image::RgbaImage, frame_png: PathBuf, mon_x: i32, mon_y: i32, mon_w: u32, mon_h: u32 }`
  - `pub fn freeze_monitor_under_cursor(app: &AppHandle) -> Result<Frozen, String>` — captures + writes `frames/frame.png`
  - `pub fn save_crop(app: &AppHandle, img: &image::RgbaImage, x: u32, y: u32, w: u32, h: u32) -> Result<PathBuf, String>`
  - `pub fn save_full(app: &AppHandle) -> Result<PathBuf, String>` — full-monitor capture straight to captures dir

- [ ] **Step 1: Implement**

```rust
use image::RgbaImage;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::AppHandle;

pub struct Frozen {
    pub image: RgbaImage,
    pub frame_png: PathBuf,
    pub mon_x: i32,
    pub mon_y: i32,
    pub mon_w: u32,
    pub mon_h: u32,
}

#[derive(Default)]
pub struct Inner {
    pub frozen: Option<Frozen>,
    pub capture: Option<PathBuf>,
}
pub struct CaptureState(pub Mutex<Inner>);

fn monitor_under_cursor(app: &AppHandle) -> Result<xcap::Monitor, String> {
    let pos = app.cursor_position().map_err(|e| e.to_string())?;
    xcap::Monitor::from_point(pos.x as i32, pos.y as i32).map_err(|e| e.to_string())
}

fn capture_monitor(m: &xcap::Monitor) -> Result<(RgbaImage, i32, i32), String> {
    let img = m.capture_image().map_err(|e| e.to_string())?;
    // NOTE: newer xcap returns Result from x()/y(); add `?`/`.map_err` if the compiler complains.
    Ok((img, m.x().map_err(|e| e.to_string())?, m.y().map_err(|e| e.to_string())?))
}

fn capture_path(app: &AppHandle) -> PathBuf {
    let now = chrono::Local::now();
    let dir = crate::retention::data_dir(app)
        .join("captures")
        .join(now.format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir).ok();
    dir.join(format!("capture-{}.png", now.format("%H%M%S-%3f")))
}

pub fn freeze_monitor_under_cursor(app: &AppHandle) -> Result<Frozen, String> {
    let m = monitor_under_cursor(app)?;
    let (image, mon_x, mon_y) = capture_monitor(&m)?;
    let dir = crate::retention::data_dir(app).join("frames");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let frame_png = dir.join("frame.png");
    image.save(&frame_png).map_err(|e| e.to_string())?;
    Ok(Frozen { mon_w: image.width(), mon_h: image.height(), image, frame_png, mon_x, mon_y })
}

pub fn save_crop(app: &AppHandle, img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> Result<PathBuf, String> {
    let x = x.min(img.width().saturating_sub(1));
    let y = y.min(img.height().saturating_sub(1));
    let w = w.min(img.width() - x).max(1);
    let h = h.min(img.height() - y).max(1);
    let out = image::imageops::crop_imm(img, x, y, w, h).to_image();
    let path = capture_path(app);
    out.save(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn save_full(app: &AppHandle) -> Result<PathBuf, String> {
    let m = monitor_under_cursor(app)?;
    let (image, _, _) = capture_monitor(&m)?;
    let path = capture_path(app);
    image.save(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
```

In `lib.rs`: `mod capture;` and `.manage(capture::CaptureState(Default::default()))` on the builder.

- [ ] **Step 2: Verify compile + smoke test**

Add at the bottom of `capture.rs` a capture smoke test that needs a real display (runs on the Windows host, ignored by default):

```rust
#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // needs a real display; run manually with -- --ignored
    fn captures_primary_monitor() {
        let m = xcap::Monitor::all().unwrap().into_iter().next().unwrap();
        let img = m.capture_image().unwrap();
        assert!(img.width() > 0 && img.height() > 0);
    }
}
```

Run: `powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test -- --ignored"`
Expected: `captures_primary_monitor ... ok`.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: monitor capture, crop, and capture-file storage"
```

---

### Task 8: Hotkey + overlay + region selection

**Files:**
- Create: `app/src-tauri/src/commands.rs`, `app/src/windows/Overlay.vue`
- Modify: `app/src-tauri/src/lib.rs`, `app/src/App.vue`

**Interfaces:**
- Consumes: `capture::*`, `CaptureState`
- Produces:
  - Rust commands (Task 9 adds more to the same file): `get_frame() -> String`, `region_selected(x,y,w,h: u32)`, `cancel_capture()`
  - `pub fn start_region_capture(app: &AppHandle)` — called from hotkey and tray
  - `pub fn open_composer(app: &AppHandle, px: i32, py: i32)` — window creation used by Task 9 too (label `"composer"`, url `index.html?window=composer`, 420×380 logical, positioned at physical `(px, py)` clamped to the monitor)

- [ ] **Step 1: Rust — commands.rs**

```rust
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, CaptureState};

pub fn start_region_capture(app: &AppHandle) {
    if app.get_webview_window("overlay").is_some() {
        return; // capture already in progress
    }
    let frozen = match capture::freeze_monitor_under_cursor(app) {
        Ok(f) => f,
        Err(e) => return notify(app, "Capture failed", &e),
    };
    let (mx, my, mw, mh) = (frozen.mon_x, frozen.mon_y, frozen.mon_w, frozen.mon_h);
    app.state::<CaptureState>().0.lock().unwrap().frozen = Some(frozen);
    let win = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html?window=overlay".into()))
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()
        .expect("overlay window");
    win.set_position(PhysicalPosition::new(mx, my)).ok();
    win.set_size(PhysicalSize::new(mw, mh)).ok();
    win.show().ok();
    win.set_focus().ok();
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

#[tauri::command]
pub fn get_frame(state: State<CaptureState>) -> Result<String, String> {
    state.0.lock().unwrap().frozen.as_ref()
        .map(|f| f.frame_png.to_string_lossy().into_owned())
        .ok_or_else(|| "no frozen frame".into())
}

#[tauri::command]
pub fn region_selected(app: AppHandle, state: State<CaptureState>, x: u32, y: u32, w: u32, h: u32) -> Result<(), String> {
    let (path, px, py) = {
        let mut inner = state.0.lock().unwrap();
        let frozen = inner.frozen.take().ok_or("no frozen frame")?;
        let path = capture::save_crop(&app, &frozen.image, x, y, w, h)?;
        std::fs::remove_file(&frozen.frame_png).ok();
        inner.capture = Some(path.clone());
        (path, frozen.mon_x + x as i32, frozen.mon_y + (y + h) as i32 + 12)
    };
    let _ = path;
    if let Some(o) = app.get_webview_window("overlay") { o.close().ok(); }
    open_composer(&app, px, py);
    Ok(())
}

pub fn open_composer(app: &AppHandle, px: i32, py: i32) {
    let win = WebviewWindowBuilder::new(app, "composer", WebviewUrl::App("index.html?window=composer".into()))
        .title("Send to Claude Code")
        .inner_size(420.0, 380.0)
        .always_on_top(true)
        .resizable(false)
        .visible(false)
        .build()
        .expect("composer window");
    win.set_position(PhysicalPosition::new(px.max(0), py.max(0))).ok();
    win.show().ok();
    win.set_focus().ok();
}

#[tauri::command]
pub fn cancel_capture(app: AppHandle, state: State<CaptureState>) {
    {
        let mut inner = state.0.lock().unwrap();
        if let Some(f) = inner.frozen.take() { std::fs::remove_file(f.frame_png).ok(); }
        if let Some(c) = inner.capture.take() { std::fs::remove_file(c).ok(); }
    }
    for label in ["overlay", "composer"] {
        if let Some(w) = app.get_webview_window(label) { w.close().ok(); }
    }
}
```

`// ponytail: composer position = below-left of selection, clamped to >=0; smarter monitor-edge clamping when it annoys someone`

- [ ] **Step 2: Rust — wire hotkey, tray item, and handler in `lib.rs`**

Add `mod commands;`, then on the builder:

```rust
.plugin(
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                crate::commands::start_region_capture(app);
            }
        })
        .build(),
)
.invoke_handler(tauri::generate_handler![
    commands::get_frame,
    commands::region_selected,
    commands::cancel_capture
])
```

In `setup`, register the shortcut:

```rust
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
app.global_shortcut()
    .register(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space))?;
```

And in the tray `on_menu_event`, replace the `"capture_region"` stub body with `crate::commands::start_region_capture(app);`.

- [ ] **Step 3: Frontend — `App.vue`**

```vue
<script setup lang="ts">
import Overlay from "./windows/Overlay.vue";
import Composer from "./windows/Composer.vue";
const win = new URLSearchParams(location.search).get("window");
</script>

<template>
  <Overlay v-if="win === 'overlay'" />
  <Composer v-else-if="win === 'composer'" />
</template>
```

(Composer.vue arrives in Task 9 — create a placeholder `<template><div /></template>` file now so this compiles.)

- [ ] **Step 4: Frontend — `Overlay.vue`**

CSS px × `devicePixelRatio` = physical px = frozen-image px, because the window is sized to the monitor's physical size.

```vue
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

const frameSrc = ref("");
const drag = ref<{ x: number; y: number } | null>(null);
const rect = ref<{ x: number; y: number; w: number; h: number } | null>(null);

onMounted(async () => {
  frameSrc.value = convertFileSrc(await invoke<string>("get_frame"));
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") invoke("cancel_capture");
  });
});

function down(e: MouseEvent) {
  drag.value = { x: e.clientX, y: e.clientY };
  rect.value = { x: e.clientX, y: e.clientY, w: 0, h: 0 };
}
function move(e: MouseEvent) {
  if (!drag.value) return;
  rect.value = {
    x: Math.min(drag.value.x, e.clientX),
    y: Math.min(drag.value.y, e.clientY),
    w: Math.abs(e.clientX - drag.value.x),
    h: Math.abs(e.clientY - drag.value.y),
  };
}
function up() {
  const r = rect.value;
  drag.value = null;
  if (!r || r.w < 4 || r.h < 4) { rect.value = null; return; }
  const s = window.devicePixelRatio;
  invoke("region_selected", {
    x: Math.round(r.x * s), y: Math.round(r.y * s),
    w: Math.round(r.w * s), h: Math.round(r.h * s),
  });
}
</script>

<template>
  <div class="overlay" @mousedown="down" @mousemove="move" @mouseup="up">
    <img :src="frameSrc" class="frame" draggable="false" />
    <div class="dim" />
    <div v-if="rect" class="sel"
      :style="{ left: rect.x + 'px', top: rect.y + 'px', width: rect.w + 'px', height: rect.h + 'px' }">
      <img :src="frameSrc" class="frame"
        :style="{ left: -rect.x + 'px', top: -rect.y + 'px' }" draggable="false" />
    </div>
  </div>
</template>

<style scoped>
.overlay { position: fixed; inset: 0; cursor: crosshair; overflow: hidden; user-select: none; }
.frame { position: absolute; left: 0; top: 0; width: 100vw; height: 100vh; }
.dim { position: absolute; inset: 0; background: rgba(0, 0, 0, 0.45); }
.sel { position: absolute; overflow: hidden; outline: 2px solid #4da3ff; }
</style>
```

- [ ] **Step 5: Manual verify**

`cargo check` + `pnpm build` first, then `pnpm tauri dev` (background): hotkey dims the screen with a frozen frame, dragging shows a bright selection, releasing closes the overlay and a (placeholder) composer window opens, and a cropped PNG exists under `%LOCALAPPDATA%\DeveloperVisualCompanion\captures\<today>\`. Esc cancels. Confirm with the user; stop dev.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: hotkey-driven frozen-frame region capture overlay"
```

---

### Task 9: Composer + Tier 2 send

**Files:**
- Create: `app/src/windows/Composer.vue` (replace placeholder), `app/src/quickactions.ts`
- Modify: `app/src-tauri/src/commands.rs`, `app/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `wslpath::to_wsl_path`, `payload::build_payload`, `capture::save_full`, `open_composer`, `notify`, `CaptureState`
- Produces: commands `get_capture() -> String`, `send_capture(message: Option<String>) -> Result<(), String>`; tray "Capture Screen" working

- [ ] **Step 1: Rust — add to `commands.rs`**

```rust
#[tauri::command]
pub fn get_capture(state: State<CaptureState>) -> Result<String, String> {
    state.0.lock().unwrap().capture.as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| "no capture".into())
}

#[tauri::command]
pub fn send_capture(app: AppHandle, state: State<CaptureState>, message: Option<String>) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let path = state.0.lock().unwrap().capture.clone().ok_or("no capture")?;
    let wsl = crate::wslpath::to_wsl_path(&path.to_string_lossy())
        .ok_or("capture path is not on a Windows drive")?;
    let payload = crate::payload::build_payload(message.as_deref(), &wsl);
    app.clipboard().write_text(payload).map_err(|e| e.to_string())?;
    state.0.lock().unwrap().capture = None; // sent: keep the file, forget the pending state
    if let Some(w) = app.get_webview_window("composer") { w.close().ok(); }
    notify(&app, "Screenshot ready", "Ready — paste into your Claude Code terminal");
    Ok(())
}

pub fn start_screen_capture(app: &AppHandle) {
    match capture::save_full(app) {
        Ok(path) => {
            app.state::<CaptureState>().0.lock().unwrap().capture = Some(path);
            open_composer(app, 200, 200);
        }
        Err(e) => notify(app, "Capture failed", &e),
    }
}
```

Register `get_capture` and `send_capture` in `generate_handler!`, and replace the tray `"capture_screen"` stub with `crate::commands::start_screen_capture(app);`.

- [ ] **Step 2: Frontend — `quickactions.ts`** (texts from spec §9)

```ts
export const QUICK_ACTIONS: { label: string; text: string }[] = [
  { label: "Explain", text: "Explain what is shown in this screenshot and how it relates to the current application." },
  { label: "Debug", text: "Analyze this screenshot and determine what appears to be wrong. Inspect the relevant code and identify the likely cause. Do not modify anything yet." },
  { label: "Implement", text: "Use this screenshot as visual reference and implement the required changes in the current project." },
  { label: "Find source", text: "Identify which component, page, or code is responsible for the UI shown in this screenshot." },
  { label: "Review", text: "Review what is shown in this screenshot and point out any problems or improvements." },
];
```

- [ ] **Step 3: Frontend — `Composer.vue`**

```vue
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { QUICK_ACTIONS } from "../quickactions";

const previewSrc = ref("");
const message = ref("");
const error = ref("");
const sending = ref(false);

onMounted(async () => {
  previewSrc.value = convertFileSrc(await invoke<string>("get_capture"));
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") invoke("cancel_capture");
  });
});

async function send() {
  sending.value = true;
  error.value = "";
  try {
    await invoke("send_capture", { message: message.value || null });
  } catch (e) {
    error.value = String(e); // capture is preserved; user can retry (spec §22)
  } finally {
    sending.value = false;
  }
}
</script>

<template>
  <div class="composer">
    <img :src="previewSrc" class="preview" />
    <div class="actions">
      <button v-for="a in QUICK_ACTIONS" :key="a.label" @click="message = a.text">{{ a.label }}</button>
    </div>
    <textarea v-model="message" rows="3" placeholder="Optional message… (default: analyze in current context)" />
    <p class="target">→ clipboard — paste into your Claude Code terminal</p>
    <p v-if="error" class="error">{{ error }} — Send again to retry.</p>
    <div class="buttons">
      <button @click="invoke('cancel_capture')">Cancel</button>
      <button class="primary" :disabled="sending" @click="send">Send</button>
    </div>
  </div>
</template>

<style scoped>
.composer { display: flex; flex-direction: column; gap: 8px; padding: 10px; height: 100vh; box-sizing: border-box; font-family: system-ui; }
.preview { max-height: 160px; object-fit: contain; border: 1px solid #ccc; }
.actions { display: flex; gap: 6px; flex-wrap: wrap; }
textarea { resize: none; }
.target { font-size: 12px; color: #666; margin: 0; }
.error { font-size: 12px; color: #c00; margin: 0; }
.buttons { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; }
.primary { font-weight: 600; }
</style>
```

- [ ] **Step 4: Verify**

```bash
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app\src-tauri; cargo test"
powershell.exe -Command "$PSP cd C:\Users\jvillafania\dev\claude_companion\app; pnpm build"
```

Expected: all Rust tests pass, frontend typechecks.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: composer window with quick actions and Tier 2 clipboard send"
```

---

### Task 10: End-to-end smoke

**Files:** none

**Interfaces:**
- Consumes: everything above
- Produces: verified M1; failures loop back to the owning task

- [ ] **Step 1: Run the checklist with the user** (`pnpm tauri dev` in background)

1. Tray icon visible; menu shows Capture Region / Capture Screen / Exit.
2. `Ctrl+Shift+Space` → frozen dimmed overlay < ~1s; drag region; composer appears near it with correct preview.
3. Type a message → Send → notification appears; paste in a WSL terminal running `claude`: message + `/mnt/c/...` path arrive; Claude reads the image.
4. Empty message → Send → clipboard starts with the default instruction line.
5. Quick action button fills the textarea.
6. Esc in overlay and in composer → windows close, no leftover capture file for the cancelled attempt.
7. Capture Screen (tray) → composer directly, full-monitor PNG.
8. Second app instance exits silently; Exit quits and hotkey stops responding.

- [ ] **Step 2: Fix anything broken** (return to the relevant task), then commit any fixes and tag:

```bash
git tag m1 && git log --oneline -1
```
