use std::path::Path;
use tauri::Manager;

#[derive(serde::Serialize, Clone)]
pub struct CaptureEntry {
    pub path: String,
    pub name: String,
    pub modified: u64,
}

/// PNGs under root's day-dirs, newest first, capped at 100.
pub fn list_under(root: &Path) -> Vec<CaptureEntry> {
    // Sort key keeps sub-second precision (two captures a few ms apart must still
    // order correctly); the public `modified` field truncates to whole seconds,
    // which is all the frontend's `new Date(modified * 1000)` needs.
    let mut out: Vec<(CaptureEntry, std::time::Duration)> = Vec::new();
    let Ok(days) = std::fs::read_dir(root) else { return Vec::new() };
    for day in days.flatten() {
        let Ok(files) = std::fs::read_dir(day.path()) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().map(|e| !e.eq_ignore_ascii_case("png")).unwrap_or(true) {
                continue;
            }
            let since_epoch = p
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .unwrap_or_default();
            out.push((
                CaptureEntry {
                    path: p.to_string_lossy().into_owned(),
                    name: p.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                    modified: since_epoch.as_secs(),
                },
                since_epoch,
            ));
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out.truncate(100); // ponytail: hard cap; paging when someone hoards captures
    out.into_iter().map(|(e, _)| e).collect()
}

/// True when `path` canonicalizes inside `base` — rejects traversal and foreign paths.
pub fn is_under(base: &Path, path: &Path) -> bool {
    match (std::fs::canonicalize(base), std::fs::canonicalize(path)) {
        (Ok(b), Ok(p)) => p.starts_with(&b),
        _ => false,
    }
}

fn captures_root(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::retention::data_dir(app).join("captures")
}

#[tauri::command]
pub fn list_captures(app: tauri::AppHandle) -> Vec<CaptureEntry> {
    list_under(&captures_root(&app))
}

#[tauri::command]
pub async fn resend_capture(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err("file no longer exists".into());
    }
    if !is_under(&captures_root(&app), &p) {
        return Err("not a capture file".into());
    }
    crate::commands::supersede_pending_overlay(&app);
    {
        let state = app.state::<crate::capture::CaptureState>();
        state.0.lock().unwrap().focus_title = None;
    }
    crate::commands::push_capture(&app, p);
    if let Some(w) = app.get_webview_window("history") {
        w.close().ok();
    }
    crate::commands::ensure_composer(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_capture(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err("file no longer exists".into());
    }
    if !is_under(&captures_root(&app), &p) {
        return Err("not a capture file".into());
    }
    let state = app.state::<crate::capture::CaptureState>();
    let pending = state.0.lock().unwrap().captures.contains(&p);
    if pending {
        return Err("capture is open in the composer".into());
    }
    std::fs::remove_file(&p).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_captures(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("composer") {
        w.close().ok();
    }
    app.state::<crate::capture::CaptureState>().0.lock().unwrap().captures.clear();
    let root = captures_root(&app);
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_pngs_newest_first_and_guards_paths() {
        let root = std::env::temp_dir().join(format!("dvc-hist-{}", std::process::id()));
        let day = root.join("2026-08-19");
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(day.join("capture-a.png"), b"x").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(day.join("capture-b.png"), b"x").unwrap();
        std::fs::write(day.join("notes.txt"), b"x").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(day.join("capture-c.PNG"), b"x").unwrap();

        let entries = list_under(&root);
        assert_eq!(entries.len(), 3, "txt file excluded, uppercase .PNG included");
        assert_eq!(entries[0].name, "capture-c.PNG", "newest first, case-insensitive extension");
        assert_eq!(entries[1].name, "capture-b.png");
        assert_eq!(entries[2].name, "capture-a.png");
        assert!(entries[0].modified >= entries[1].modified);

        assert!(is_under(&root, &day.join("capture-a.png")));
        assert!(!is_under(&root, std::path::Path::new("C:\\Windows\\system32\\cmd.exe")));
        assert!(!is_under(&root, &root.join("..").join("escape.png")));
        std::fs::remove_dir_all(&root).ok();
    }
}
