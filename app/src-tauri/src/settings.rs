use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub retention_hours: u64,
    pub default_instruction: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            retention_hours: 24,
            default_instruction: crate::payload::DEFAULT_INSTRUCTION.to_string(),
        }
    }
}

pub fn load(dir: &Path) -> Settings {
    std::fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, s: &Settings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("settings.json"), serde_json::to_string_pretty(s).expect("serialize"))
}

#[tauri::command]
pub fn get_settings(app: tauri::AppHandle) -> Settings {
    load(&crate::retention::data_dir(&app))
}

#[tauri::command]
pub fn set_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    save(&crate::retention::data_dir(&app), &settings).map_err(|e| e.to_string())
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

        // saved values come back
        save(&dir, &Settings { retention_hours: 72, default_instruction: "Look at this.".into() }).unwrap();
        let s = load(&dir);
        assert_eq!(s.retention_hours, 72);
        assert_eq!(s.default_instruction, "Look at this.");

        // corrupt file -> defaults, not a panic
        std::fs::write(dir.join("settings.json"), "{nope").unwrap();
        assert_eq!(load(&dir).retention_hours, 24);
        std::fs::remove_dir_all(&dir).ok();
    }
}
