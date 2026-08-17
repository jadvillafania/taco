use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, CaptureState};

pub fn start_region_capture(app: &AppHandle) {
    if app.get_webview_window("overlay").is_some() {
        return; // capture already in progress
    }
    // A new capture supersedes any open composer: close it and drop its pending capture file.
    if let Some(c) = app.get_webview_window("composer") {
        c.close().ok();
        if let Some(path) = app.state::<CaptureState>().0.lock().unwrap().capture.take() {
            std::fs::remove_file(path).ok();
        }
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
    let (_, px, py) = {
        let mut inner = state.0.lock().unwrap();
        let frozen = inner.frozen.take().ok_or("no frozen frame")?;
        let path = capture::save_crop(&app, &frozen.image, x, y, w, h)?;
        std::fs::remove_file(&frozen.frame_png).ok();
        inner.capture = Some(path.clone());
        (path, frozen.mon_x + x as i32, frozen.mon_y + (y + h) as i32 + 12)
    };
    if let Some(o) = app.get_webview_window("overlay") { o.close().ok(); }
    open_composer(&app, px, py);
    Ok(())
}

pub fn open_composer(app: &AppHandle, px: i32, py: i32) {
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
        Err(_) => return notify(app, "Capture failed", "could not open composer window"),
    };
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

// ponytail: composer position = below-left of selection, clamped to >=0; smarter monitor-edge clamping when it annoys someone
