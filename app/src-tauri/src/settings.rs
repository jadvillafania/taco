use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub retention_hours: u64,
    pub default_instruction: String,
    pub hotkey_region: String,
    pub hotkey_window: String,
    pub hotkey_clipboard: String,
    pub wsl_connected: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            retention_hours: 24,
            default_instruction: crate::payload::DEFAULT_INSTRUCTION.to_string(),
            hotkey_region: "Ctrl+Shift+Space".into(),
            hotkey_window: "Ctrl+Alt+Space".into(),
            hotkey_clipboard: "Ctrl+Alt+V".into(),
            wsl_connected: false,
        }
    }
}

pub fn load(dir: &Path) -> Settings {
    let parsed = std::fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    let mut s: Settings = parsed
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    // Migration: a settings file that parses but predates the wsl_connected gate belongs
    // to a WSL-first install — don't silently disconnect it on upgrade. A corrupt file
    // must stay fail-safe (gate off), so this keys off the parsed object, not raw text.
    if let Some(v) = &parsed {
        if v.get("wsl_connected").is_none() {
            s.wsl_connected = true;
        }
    }
    s.retention_hours = s.retention_hours.max(1);
    s
}

pub fn save(dir: &Path, s: &Settings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("settings.json"), serde_json::to_string_pretty(s).map_err(std::io::Error::other)?)
}

#[tauri::command]
pub fn get_settings(app: tauri::AppHandle) -> Settings {
    load(&crate::retention::data_dir(&app))
}

#[tauri::command]
pub fn get_default_settings() -> Settings {
    Settings::default()
}

/// The frontend's Save posts only the fields it edits; deployer-owned state
/// (wsl_connected) must survive by re-reading it from disk.
pub fn merge_frontend(dir: &Path, mut incoming: Settings) -> Settings {
    incoming.wsl_connected = load(dir).wsl_connected;
    incoming
}

#[tauri::command]
pub fn set_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    let dir = crate::retention::data_dir(&app);
    let settings = merge_frontend(&dir, settings);
    save(&dir, &settings).map_err(|e| e.to_string())?;
    crate::hotkeys::apply(&app, &settings);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults() {
        let dir = std::env::temp_dir().join(format!("dvc-set-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // missing file -> defaults
        let s = load(&dir);
        assert_eq!(s.retention_hours, 24);
        assert!(s.default_instruction.contains("Analyze this screenshot"));
        assert_eq!(s.hotkey_region, "Ctrl+Shift+Space");
        assert_eq!(s.hotkey_window, "Ctrl+Alt+Space");
        assert_eq!(s.hotkey_clipboard, "Ctrl+Alt+V");

        // saved values come back
        save(&dir, &Settings { retention_hours: 72, default_instruction: "Look at this.".into(), ..Default::default() }).unwrap();
        let s = load(&dir);
        assert_eq!(s.retention_hours, 72);
        assert_eq!(s.default_instruction, "Look at this.");

        // corrupt file -> defaults, not a panic
        std::fs::write(dir.join("settings.json"), "{nope").unwrap();
        assert_eq!(load(&dir).retention_hours, 24);

        // zero retention_hours is clamped to a floor of 1
        std::fs::write(dir.join("settings.json"), r#"{"retention_hours":0}"#).unwrap();
        assert_eq!(load(&dir).retention_hours, 1);

        // wsl_connected persists both ways once written (a key-less file migrates to true,
        // covered by pre_gate_settings_migrate_to_wsl_connected)
        save(&dir, &Settings { wsl_connected: false, ..Default::default() }).unwrap();
        assert!(!load(&dir).wsl_connected);
        save(&dir, &Settings { wsl_connected: true, ..Default::default() }).unwrap();
        assert!(load(&dir).wsl_connected);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pre_gate_settings_migrate_to_wsl_connected() {
        let dir = std::env::temp_dir().join(format!("dvc-mig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // v0.1.1-era file: predates the gate, that install was WSL-first
        std::fs::write(dir.join("settings.json"), r#"{"retention_hours":24}"#).unwrap();
        assert!(load(&dir).wsl_connected, "existing install stays connected after upgrade");
        // explicit false (post-gate remove-shim) is respected
        std::fs::write(dir.join("settings.json"), r#"{"wsl_connected":false}"#).unwrap();
        assert!(!load(&dir).wsl_connected);
        // a half-written file (save() is non-atomic, wsl_connected serializes last)
        // must stay fail-safe rather than migrate itself to connected
        std::fs::write(dir.join("settings.json"), r#"{"retention_hours":24,"hotkey_re"#).unwrap();
        assert!(!load(&dir).wsl_connected, "corrupt file does not fail open");
        std::fs::remove_dir_all(&dir).ok();
        // fresh install: no file at all -> gate stays off
        assert!(!load(&dir).wsl_connected);
    }

    #[test]
    fn frontend_save_cannot_clobber_wsl_connected() {
        let dir = std::env::temp_dir().join(format!("dvc-merge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        save(&dir, &Settings { wsl_connected: true, ..Default::default() }).unwrap();
        // incoming from the frontend: wsl_connected serde-defaulted to false
        let merged = merge_frontend(&dir, Settings { retention_hours: 72, ..Default::default() });
        assert!(merged.wsl_connected, "deployer-owned flag survives a frontend save");
        assert_eq!(merged.retention_hours, 72);
        std::fs::remove_dir_all(&dir).ok();
    }
}
