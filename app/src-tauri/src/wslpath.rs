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
}
