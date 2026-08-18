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
