use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, CaptureState};

/// A new capture (region or full-screen) supersedes anything already in flight: close any open
/// overlay/composer window and drop whatever pending state it was holding, so we never leave an
/// orphaned frozen frame or pending capture file behind, and a stale overlay never outlives the
/// capture it belonged to.
fn supersede_pending_capture(app: &AppHandle) {
    if let Some(o) = app.get_webview_window("overlay") {
        o.close().ok();
        if let Some(f) = app.state::<CaptureState>().0.lock().unwrap().frozen.take() {
            std::fs::remove_file(f.frame_png).ok();
        }
    }
    if let Some(c) = app.get_webview_window("composer") {
        c.close().ok();
        if let Some(path) = app.state::<CaptureState>().0.lock().unwrap().capture.take() {
            std::fs::remove_file(path).ok();
        }
    }
}

pub fn start_region_capture(app: &AppHandle) {
    if app.get_webview_window("overlay").is_some() {
        return; // capture already in progress
    }
    let focus = crate::sessions::foreground_title(); // before our windows take focus
    supersede_pending_capture(app);
    app.state::<CaptureState>().0.lock().unwrap().focus_title = focus;
    let frozen = match capture::freeze_monitor_under_cursor(app) {
        Ok(f) => f,
        Err(e) => return notify(app, "Capture failed", &e),
    };
    let (mx, my, mw, mh) = (frozen.mon_x, frozen.mon_y, frozen.mon_w, frozen.mon_h);
    app.state::<CaptureState>().0.lock().unwrap().frozen = Some(frozen);
    let win = match WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html?window=overlay".into()))
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()
    {
        Ok(w) => w,
        Err(_) => {
            if let Some(f) = app.state::<CaptureState>().0.lock().unwrap().frozen.take() {
                std::fs::remove_file(f.frame_png).ok();
            }
            return notify(app, "Capture failed", "could not open overlay window");
        }
    };
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
pub async fn region_selected(app: AppHandle, state: State<'_, CaptureState>, x: u32, y: u32, w: u32, h: u32) -> Result<(), String> {
    {
        let mut inner = state.0.lock().unwrap();
        let frozen = inner.frozen.take().ok_or("no frozen frame")?;
        let path = capture::save_crop(&app, &frozen.image, x, y, w, h)?;
        std::fs::remove_file(&frozen.frame_png).ok();
        inner.capture = Some(path);
    }
    if let Some(o) = app.get_webview_window("overlay") { o.close().ok(); }
    open_composer(&app);
    Ok(())
}

pub struct LastComposerPos(pub std::sync::Mutex<Option<(i32, i32)>>);

pub fn open_composer(app: &AppHandle) {
    // Defensively close any existing composer before opening a new one (label must be unique).
    if let Some(c) = app.get_webview_window("composer") {
        c.close().ok();
    }
    let win = match WebviewWindowBuilder::new(app, "composer", WebviewUrl::App("index.html?window=composer".into()))
        .title("Send to Claude Code")
        .inner_size(420.0, 380.0)
        .always_on_top(true)
        .resizable(false)
        .visible(false)
        .build()
    {
        Ok(w) => w,
        Err(_) => {
            // The capture itself already succeeded — only the composer window failed to open —
            // so "Capture failed" would be misleading here.
            return notify(
                app,
                "Could not open composer",
                "The capture is still pending — press Ctrl+Shift+Space to retry.",
            );
        }
    };

    let size = win.outer_size().unwrap_or(PhysicalSize::new(420, 380));
    let monitors: Vec<(i32, i32, u32, u32)> = win
        .available_monitors()
        .map(|ms| ms.iter().map(|m| (m.position().x, m.position().y, m.size().width, m.size().height)).collect())
        .unwrap_or_default();

    let data_dir = crate::retention::data_dir(app);
    let saved = crate::winpos::load(&data_dir)
        .filter(|&(x, y)| crate::winpos::rect_on_any_monitor(x, y, size.width as i32, size.height as i32, &monitors));
    let (mut x, mut y) = saved.unwrap_or_else(|| {
        // default: center of the primary monitor
        match win.primary_monitor().ok().flatten() {
            Some(m) => (
                m.position().x + (m.size().width as i32 - size.width as i32) / 2,
                m.position().y + (m.size().height as i32 - size.height as i32) / 2,
            ),
            None => (100, 100),
        }
    });

    // safety net: clamp onto the monitor the point falls on (or primary) exactly as the existing clamp did
    if let Some(mon) = win.current_monitor().ok().flatten().or_else(|| win.primary_monitor().ok().flatten()) {
        let mon_pos = mon.position();
        let mon_size = mon.size();
        let max_x = mon_pos.x + mon_size.width as i32 - size.width as i32;
        let max_y = mon_pos.y + mon_size.height as i32 - size.height as i32;
        if saved.is_none() {
            x = x.clamp(mon_pos.x.min(max_x), max_x.max(mon_pos.x));
            y = y.clamp(mon_pos.y.min(max_y), max_y.max(mon_pos.y));
        }
    }
    win.set_position(PhysicalPosition::new(x, y)).ok();
    // Seed the tracked position with what we actually applied, so a Destroyed event fired before
    // any Moved event (i.e. the user never dragged this composer) persists this position rather
    // than a stale value left behind by a previous composer session.
    *app.state::<LastComposerPos>().0.lock().unwrap() = Some((x, y));

    // track moves; persist on destroy
    let app2 = app.clone();
    win.on_window_event(move |event| match event {
        tauri::WindowEvent::Moved(p) => {
            *app2.state::<LastComposerPos>().0.lock().unwrap() = Some((p.x, p.y));
        }
        tauri::WindowEvent::Destroyed => {
            let pos = *app2.state::<LastComposerPos>().0.lock().unwrap();
            if let Some((x, y)) = pos {
                crate::winpos::save(&crate::retention::data_dir(&app2), x, y);
            }
        }
        _ => {}
    });

    win.show().ok();
    win.set_focus().ok();
}

#[tauri::command]
pub async fn cancel_capture(app: AppHandle, state: State<'_, CaptureState>) -> Result<(), ()> {
    {
        let mut inner = state.0.lock().unwrap();
        if let Some(f) = inner.frozen.take() { std::fs::remove_file(f.frame_png).ok(); }
        if let Some(c) = inner.capture.take() { std::fs::remove_file(c).ok(); }
    }
    for label in ["overlay", "composer"] {
        if let Some(w) = app.get_webview_window(label) { w.close().ok(); }
    }
    Ok(())
}

#[tauri::command]
pub fn get_capture(state: State<CaptureState>) -> Result<String, String> {
    state.0.lock().unwrap().capture.as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| "no capture".into())
}

#[derive(serde::Deserialize)]
pub struct TargetSession { pub sid: String, pub distro: String, pub project: String }

#[tauri::command]
pub async fn send_capture(
    app: AppHandle,
    state: State<'_, CaptureState>,
    message: Option<String>,
    session: Option<TargetSession>,
) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let path = state.0.lock().unwrap().capture.clone().ok_or("no capture")?;
    let wsl = crate::wslpath::to_wsl_path(&path.to_string_lossy())
        .ok_or("capture path is not on a Windows drive")?;
    let payload = crate::payload::build_payload(message.as_deref(), &wsl);

    if let Some(s) = &session {
        match crate::tier1::send_via_shim(&s.distro, &s.sid, &payload) {
            crate::tier1::Outcome::Ack => {
                state.0.lock().unwrap().capture = None;
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

    app.clipboard().write_text(payload).map_err(|e| e.to_string())?;
    state.0.lock().unwrap().capture = None; // sent: keep the file, forget the pending state
    if let Some(w) = app.get_webview_window("composer") { w.close().ok(); }
    let body = if session.is_some() {
        "Session busy or unreachable — payload copied. Paste into your Claude Code terminal"
    } else {
        "Ready — paste into your Claude Code terminal"
    };
    notify(&app, "Screenshot ready", body);
    Ok(())
}

pub fn start_screen_capture(app: &AppHandle) {
    let focus = crate::sessions::foreground_title();
    supersede_pending_capture(app);
    app.state::<CaptureState>().0.lock().unwrap().focus_title = focus;
    match capture::save_full(app) {
        Ok(path) => {
            app.state::<CaptureState>().0.lock().unwrap().capture = Some(path);
            open_composer(app);
        }
        Err(e) => notify(app, "Capture failed", &e),
    }
}
