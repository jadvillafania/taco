use std::path::Path;

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "host", rename_all = "lowercase")]
pub enum Host {
    Wsl { distro: String },
    Windows,
}

#[derive(Clone, serde::Serialize)]
pub struct Session {
    pub sid: String,
    #[serde(flatten)]
    pub host: Host,
    pub project: String,
    pub cwd: String,
    #[serde(skip)]
    pub pid: u32,
}

pub fn parse_wsl_list(bytes: &[u8]) -> Vec<String> {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn session_from_file(p: &Path, host: Host) -> Option<Session> {
    if p.extension().map(|e| e != "json").unwrap_or(true) { return None; }
    let txt = std::fs::read_to_string(p).ok()?;
    let v = serde_json::from_str::<serde_json::Value>(&txt).ok()?;
    Some(Session {
        sid: p.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
        host,
        project: v["project"].as_str().unwrap_or("?").to_string(),
        cwd: v["cwd"].as_str().unwrap_or("").to_string(),
        pid: v["pid"].as_u64().unwrap_or(0) as u32,
    })
}

fn windows_pid_alive(pid: u32) -> bool {
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => {
                windows::Win32::Foundation::CloseHandle(h).ok();
                true
            }
            Err(_) => false,
        }
    }
}

/// A killed shim (closed terminal, SIGKILL) can't remove its registry file, so the
/// dropdown would show the dead session forever — check liveness at list time.
fn is_alive(s: &Session) -> bool {
    if s.pid == 0 {
        return true; // ponytail: unknown pid — better a stale row than hiding a live session
    }
    match &s.host {
        Host::Wsl { distro } => Path::new(&format!(r"\\wsl$\{distro}\proc\{}", s.pid)).exists(),
        Host::Windows => windows_pid_alive(s.pid),
    }
}

pub fn sessions_under(base: &Path, distro: &str) -> Vec<Session> {
    let mut out = Vec::new();
    let Ok(uids) = std::fs::read_dir(base) else { return out };
    for uid in uids.flatten() {
        let dvc = uid.path().join("dvc");
        let Ok(files) = std::fs::read_dir(&dvc) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if let Some(s) = session_from_file(&p, Host::Wsl { distro: distro.to_string() }) {
                out.push(s);
            }
        }
    }
    out
}

pub fn sessions_in_flat_dir(dir: &Path) -> Vec<Session> {
    let Ok(files) = std::fs::read_dir(dir) else { return Vec::new() };
    files.flatten().filter_map(|f| session_from_file(&f.path(), Host::Windows)).collect()
}

pub fn native_run_dir() -> Option<std::path::PathBuf> {
    std::env::var("LOCALAPPDATA").ok()
        .map(|l| std::path::PathBuf::from(l).join("DeveloperVisualCompanion").join("run"))
}

pub fn list_sessions(wsl_connected: bool) -> Vec<Session> {
    let mut sessions: Vec<Session> = native_run_dir()
        .map(|d| sessions_in_flat_dir(&d))
        .unwrap_or_default();
    if wsl_connected {
        sessions.extend(wsl_sessions());
    }
    sessions.retain(is_alive);
    sessions
}

/// Guard first: a failed wsl.exe prints error text, not a distro list (bug fix).
pub fn distros_from(out: &std::process::Output) -> Vec<String> {
    if !out.status.success() { return Vec::new(); }
    parse_wsl_list(&out.stdout)
}

fn wsl_sessions() -> Vec<Session> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("wsl.exe")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else { return Vec::new() };
    let mut sessions = Vec::new();
    for distro in distros_from(&out) {
        // stale registry files (crashed shims) surface here; Tier 1 send fails -> Tier 2 fallback
        let base = std::path::PathBuf::from(format!(r"\\wsl$\{distro}\run\user"));
        sessions.extend(sessions_under(&base, &distro));
    }
    sessions
}

/// Spec §13: the session matching the focused project moves to index 0, where the
/// composer's existing auto-select picks it up. No match leaves the order alone.
// ponytail: substring match on project name; short names ("app") can false-match —
// the manual dropdown stays as the fallback.
pub fn rank_sessions(mut sessions: Vec<Session>, focus_title: Option<&str>) -> Vec<Session> {
    if let Some(title) = focus_title {
        let title = title.to_lowercase();
        if let Some(i) = sessions.iter().position(|s| {
            !s.project.is_empty() && title.contains(&s.project.to_lowercase())
        }) {
            let hit = sessions.remove(i);
            sessions.insert(0, hit);
        }
    }
    sessions
}

/// Title of the foreground window, sampled at capture start (before our overlay takes focus).
pub fn foreground_title() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() { return None; }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len <= 0 { return None; }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Frame bounds (l, t, r, b) of the foreground window, DWM-accurate (no drop-shadow margins).
pub fn foreground_rect() -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut rect = RECT::default();
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        )
        .ok()?;
        Some((rect.left, rect.top, rect.right, rect.bottom))
    }
}

#[tauri::command]
pub fn list_sessions_cmd(app: tauri::AppHandle, state: tauri::State<crate::capture::CaptureState>) -> Vec<Session> {
    let wsl = crate::settings::load(&crate::retention::data_dir(&app)).wsl_connected;
    let title = state.0.lock().unwrap().focus_title.clone();
    rank_sessions(list_sessions(wsl), title.as_deref())
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

    fn sess(project: &str) -> Session {
        Session {
            sid: project.into(),
            host: Host::Wsl { distro: "Ubuntu".into() },
            project: project.into(),
            cwd: format!("/home/j/{project}"),
            pid: 1,
        }
    }

    #[test]
    fn detects_live_and_dead_windows_pids() {
        assert!(windows_pid_alive(std::process::id()), "our own pid is alive");
        assert!(!windows_pid_alive(3), "pids are multiples of 4 — 3 can never exist");
    }

    #[test]
    fn session_json_is_flat_and_host_tagged() {
        let v = serde_json::to_value(sess("p")).unwrap();
        assert_eq!(v["host"], "wsl");
        assert_eq!(v["distro"], "Ubuntu");
        let w = Session { host: Host::Windows, ..sess("p") };
        let v = serde_json::to_value(w).unwrap();
        assert_eq!(v["host"], "windows");
        assert!(v.get("distro").is_none());
    }

    #[test]
    fn scans_flat_native_registry() {
        let dir = std::env::temp_dir().join(format!("dvc-nat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("7.json"),
            r#"{"pid":7,"cwd":"C:\\Users\\j\\dev\\my-app","distro":"","project":"my-app","socket":"C:\\x\\7.sock","started_at":1}"#,
        ).unwrap();
        std::fs::write(dir.join("junk.txt"), "x").unwrap();
        let s = sessions_in_flat_dir(&dir);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].sid, "7");
        assert_eq!(s[0].host, Host::Windows);
        assert_eq!(s[0].project, "my-app");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rank_moves_focused_project_first() {
        let ranked = rank_sessions(vec![sess("other"), sess("my-app")], Some("main.ts - my-app - Visual Studio Code"));
        assert_eq!(ranked[0].project, "my-app");
        assert_eq!(ranked[1].project, "other");
    }

    #[test]
    fn rank_is_case_insensitive() {
        let ranked = rank_sessions(vec![sess("other"), sess("My-App")], Some("MY-APP — file.rs"));
        assert_eq!(ranked[0].project, "My-App");
    }

    #[test]
    fn rank_keeps_order_when_no_match_or_no_title() {
        let ranked = rank_sessions(vec![sess("a"), sess("b")], Some("unrelated window"));
        assert_eq!(ranked[0].project, "a");
        let ranked = rank_sessions(vec![sess("a"), sess("b")], None);
        assert_eq!(ranked[0].project, "a");
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
        assert_eq!(s[0].host, Host::Wsl { distro: "Ubuntu".into() });
        assert_eq!(s[0].project, "my-app");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn wsl_error_output_yields_no_distros() {
        use std::os::windows::process::ExitStatusExt;
        let utf16 = |s: &str| s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect::<Vec<u8>>();
        let fail = std::process::Output {
            status: std::process::ExitStatus::from_raw(1),
            stdout: utf16("There is no distribution with the supplied name."),
            stderr: Vec::new(),
        };
        assert!(distros_from(&fail).is_empty(), "error text is not a distro list");
        let ok = std::process::Output { status: std::process::ExitStatus::from_raw(0), stdout: utf16("Ubuntu\r\n"), stderr: Vec::new() };
        assert_eq!(distros_from(&ok), vec!["Ubuntu"]);
    }

    #[test]
    fn wsl_scan_is_gated() {
        // gate off: must return instantly with no wsl.exe spawn — timing is the tell
        let t0 = std::time::Instant::now();
        let _ = list_sessions(false);
        assert!(t0.elapsed() < std::time::Duration::from_millis(200),
            "gated list_sessions must not touch wsl.exe / UNC paths");
    }
}
