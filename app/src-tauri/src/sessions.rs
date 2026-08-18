use std::path::Path;

#[derive(Clone, serde::Serialize)]
pub struct Session {
    pub sid: String,
    pub distro: String,
    pub project: String,
    pub cwd: String,
}

pub fn parse_wsl_list(bytes: &[u8]) -> Vec<String> {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn sessions_under(base: &Path, distro: &str) -> Vec<Session> {
    let mut out = Vec::new();
    let Ok(uids) = std::fs::read_dir(base) else { return out };
    for uid in uids.flatten() {
        let dvc = uid.path().join("dvc");
        let Ok(files) = std::fs::read_dir(&dvc) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().map(|e| e != "json").unwrap_or(true) { continue; }
            let Ok(txt) = std::fs::read_to_string(&p) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else { continue };
            out.push(Session {
                sid: p.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
                distro: distro.to_string(),
                project: v["project"].as_str().unwrap_or("?").to_string(),
                cwd: v["cwd"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    out
}

pub fn list_sessions() -> Vec<Session> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("wsl.exe")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else { return Vec::new() };
    let mut sessions = Vec::new();
    for distro in parse_wsl_list(&out.stdout) {
        // stale registry files (crashed shims) surface here; Tier 1 send fails -> Tier 2 fallback
        let base = std::path::PathBuf::from(format!(r"\\wsl$\{distro}\run\user"));
        sessions.extend(sessions_under(&base, &distro));
    }
    sessions
}

#[tauri::command]
pub fn list_sessions_cmd() -> Vec<Session> {
    list_sessions()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utf16_distro_list() {
        let mut bytes = Vec::new();
        for u in "Ubuntu\r\nDebian\r\n\r\n".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(parse_wsl_list(&bytes), vec!["Ubuntu", "Debian"]);
    }

    #[test]
    fn scans_registry_layout() {
        let base = std::env::temp_dir().join(format!("dvc-sess-{}", std::process::id()));
        let dvc = base.join("1000").join("dvc");
        std::fs::create_dir_all(&dvc).unwrap();
        std::fs::write(
            dvc.join("42.json"),
            r#"{"pid":42,"cwd":"/home/j/my-app","distro":"Ubuntu","project":"my-app","socket":"/run/user/1000/dvc/42.sock","started_at":1}"#,
        ).unwrap();
        std::fs::write(dvc.join("junk.txt"), "x").unwrap();
        let s = sessions_under(&base, "Ubuntu");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].sid, "42");
        assert_eq!(s[0].project, "my-app");
        std::fs::remove_dir_all(&base).ok();
    }
}
