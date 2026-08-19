use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, CaptureState};

/// A new capture (region or full-screen) supersedes any overlay already in flight: close it and
/// drop whatever frozen frame it was holding, so a stale overlay never outlives the capture it
/// belonged to. The composer and its accumulated captures are untouched — every capture source
/// appends to the same list (see `push_capture`).
pub fn supersede_pending_overlay(app: &AppHandle) {
    if let Some(o) = app.get_webview_window("overlay") {
        o.close().ok();
        if let Some(f) = app.state::<CaptureState>().0.lock().unwrap().frozen.take() {
            std::fs::remove_file(f.frame_png).ok();
        }
    }
}

/// Append a capture to the pending list and notify the frontend.
pub(crate) fn push_capture(app: &AppHandle, path: std::path::PathBuf) {
    app.state::<CaptureState>().0.lock().unwrap().captures.push(path);
    use tauri::Emitter;
    app.emit("captures-changed", ()).ok();
}

/// Show the composer window, opening it if it isn't already open.
pub fn ensure_composer(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("composer") {
        w.show().ok();
        w.set_focus().ok();
    } else {
        open_composer(app);
    }
}

pub fn start_region_capture(app: &AppHandle) {
    if app.get_webview_window("overlay").is_some() {
        return; // capture already in progress
    }
    let focus = crate::sessions::foreground_title(); // before our windows take focus
    supersede_pending_overlay(app);
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
    let path = {
        let mut inner = state.0.lock().unwrap();
        let frozen = inner.frozen.take().ok_or("no frozen frame")?;
        let path = capture::save_crop(&app, &frozen.image, x, y, w, h)?;
        std::fs::remove_file(&frozen.frame_png).ok();
        path
    };
    if let Some(o) = app.get_webview_window("overlay") { o.close().ok(); }
    push_capture(&app, path);
    ensure_composer(&app);
    Ok(())
}

#[derive(Default)]
pub struct ComposerGeom {
    pub pos: std::sync::Mutex<Option<(i32, i32)>>,
    pub size: std::sync::Mutex<Option<(u32, u32)>>,
}

pub fn open_composer(app: &AppHandle) {
    // Defensively close any existing composer before opening a new one (label must be unique).
    if let Some(c) = app.get_webview_window("composer") {
        c.close().ok();
    }
    let win = match WebviewWindowBuilder::new(app, "composer", WebviewUrl::App("index.html?window=composer".into()))
        .title("Send to Claude Code")
        .inner_size(420.0, 380.0)
        .min_inner_size(420.0, 380.0)
        .always_on_top(true)
        .resizable(true)
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

    let data_dir = crate::retention::data_dir(app);
    let saved_size = crate::winpos::load_size(&data_dir);
    if let Some((w, h)) = saved_size {
        win.set_size(PhysicalSize::new(w, h)).ok();
    }

    let size = win.outer_size().unwrap_or(PhysicalSize::new(420, 380));
    let monitors: Vec<(i32, i32, u32, u32)> = win
        .available_monitors()
        .map(|ms| ms.iter().map(|m| (m.position().x, m.position().y, m.size().width, m.size().height)).collect())
        .unwrap_or_default();

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

    // Seed the tracked geometry with what we actually applied, so a Destroyed event fired before
    // any Moved/Resized event (i.e. the user never dragged/resized this composer) persists this
    // geometry rather than a stale value left behind by a previous composer session. Resized
    // reports inner size (and set_size sets inner size), so the fallback must use inner_size(),
    // not the outer_size()-derived `size` used for monitor/position math above — otherwise a
    // never-resized composer would persist its outer size and grow by the window-chrome delta
    // every session.
    *app.state::<ComposerGeom>().pos.lock().unwrap() = Some((x, y));
    *app.state::<ComposerGeom>().size.lock().unwrap() = Some(saved_size.unwrap_or_else(|| {
        win.inner_size().map(|s| (s.width, s.height)).unwrap_or((420, 380))
    }));

    // track moves/resizes; persist on destroy
    let app2 = app.clone();
    win.on_window_event(move |event| match event {
        tauri::WindowEvent::Moved(p) => {
            *app2.state::<ComposerGeom>().pos.lock().unwrap() = Some((p.x, p.y));
        }
        tauri::WindowEvent::Resized(s) => {
            *app2.state::<ComposerGeom>().size.lock().unwrap() = Some((s.width, s.height));
        }
        tauri::WindowEvent::Destroyed => {
            let geom = app2.state::<ComposerGeom>();
            let pos = *geom.pos.lock().unwrap();
            let size = *geom.size.lock().unwrap();
            if let (Some((x, y)), Some((w, h))) = (pos, size) {
                crate::winpos::save(&crate::retention::data_dir(&app2), x, y, w, h);
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
        inner.captures.clear(); // files stay: History/retention owns deletion
    }
    for label in ["overlay", "composer"] {
        if let Some(w) = app.get_webview_window(label) { w.close().ok(); }
    }
    Ok(())
}

#[tauri::command]
pub fn get_captures(state: State<CaptureState>) -> Vec<String> {
    state.0.lock().unwrap().captures.iter().map(|p| p.to_string_lossy().into_owned()).collect()
}

#[tauri::command]
pub fn remove_capture(app: AppHandle, state: State<CaptureState>, index: usize) -> Result<(), String> {
    let mut inner = state.0.lock().unwrap();
    if index >= inner.captures.len() { return Err("no such image".into()); }
    inner.captures.remove(index); // file stays: History owns deletion
    drop(inner);
    use tauri::Emitter;
    app.emit("captures-changed", ()).ok();
    Ok(())
}

#[tauri::command]
pub fn import_clipboard(app: AppHandle) -> Result<String, String> {
    let path = crate::capture::save_clipboard_image(&app)?;
    let s = path.to_string_lossy().into_owned();
    push_capture(&app, path);
    Ok(s)
}

#[tauri::command]
pub fn import_file(app: AppHandle, path: String) -> Result<String, String> {
    let dest = crate::capture::import_as_capture(&app, std::path::Path::new(&path))?;
    let s = dest.to_string_lossy().into_owned();
    push_capture(&app, dest);
    Ok(s)
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
    let paths = state.0.lock().unwrap().captures.clone();
    if paths.is_empty() { return Err("no capture".into()); }
    let wsl_paths = paths
        .iter()
        .map(|p| crate::wslpath::to_wsl_path(&p.to_string_lossy()).ok_or("capture path is not on a Windows drive"))
        .collect::<Result<Vec<_>, _>>()?;
    let default_instruction = crate::settings::load(&crate::retention::data_dir(&app)).default_instruction;
    let payload = crate::payload::build_payload(message.as_deref(), &wsl_paths, &default_instruction);

    if let Some(s) = &session {
        match crate::tier1::send_via_shim(&s.distro, &s.sid, &payload) {
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

    app.clipboard().write_text(payload).map_err(|e| e.to_string())?;
    state.0.lock().unwrap().captures.clear(); // sent: keep the files, forget the pending state
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
    supersede_pending_overlay(app);
    app.state::<CaptureState>().0.lock().unwrap().focus_title = focus;
    match capture::save_full(app) {
        Ok(path) => {
            push_capture(app, path);
            ensure_composer(app);
        }
        Err(e) => notify(app, "Capture failed", &e),
    }
}

pub fn start_window_capture(app: &AppHandle) {
    let focus = crate::sessions::foreground_title();
    supersede_pending_overlay(app);
    app.state::<CaptureState>().0.lock().unwrap().focus_title = focus;
    match capture::save_active_window(app) {
        Ok(path) => {
            push_capture(app, path);
            ensure_composer(app);
        }
        Err(e) => notify(app, "Capture failed", &e),
    }
}

pub fn start_clipboard_capture(app: &AppHandle) {
    let focus = crate::sessions::foreground_title();
    supersede_pending_overlay(app);
    app.state::<CaptureState>().0.lock().unwrap().focus_title = focus;
    match capture::save_clipboard_image(app) {
        Ok(path) => {
            push_capture(app, path);
            ensure_composer(app);
        }
        Err(e) => notify(app, "Clipboard capture failed", &e),
    }
}

pub fn decode_png_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let b64 = data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or("expected a PNG data URL")?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_annotated(state: State<CaptureState>, data_url: String, index: usize) -> Result<(), String> {
    let path = state.0.lock().unwrap().captures.get(index).cloned().ok_or("no capture")?;
    let bytes = decode_png_data_url(&data_url)?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_capture_data_url(state: State<CaptureState>, index: usize) -> Result<String, String> {
    use base64::Engine;
    let path = state.0.lock().unwrap().captures.get(index).cloned().ok_or("no capture")?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png_data_url_and_rejects_junk() {
        // 1x1 transparent PNG
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
        let bytes = decode_png_data_url(&format!("data:image/png;base64,{b64}")).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        assert!(decode_png_data_url("data:text/plain;base64,aGk=").is_err());
        assert!(decode_png_data_url("not a data url").is_err());
    }
}
