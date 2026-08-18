use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionInfo {
    pub pid: u32,
    pub cwd: String,
    pub distro: String,
    pub project: String,
    pub socket: String,
    pub started_at: u64,
}

pub fn runtime_dir() -> PathBuf {
    if let Ok(d) = std::env::var("DVC_RUNTIME_DIR") {
        return PathBuf::from(d);
    }
    if let Ok(x) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(x).join("dvc");
    }
    std::env::temp_dir().join(format!("dvc-{}", nix::unistd::getuid()))
}

pub fn write(dir: &Path, info: &SessionInfo) -> std::io::Result<PathBuf> {
    let path = dir.join(format!("{}.json", info.pid));
    std::fs::write(&path, serde_json::to_string_pretty(info).expect("serialize"))?;
    Ok(path)
}

pub fn remove(dir: &Path, pid: u32) {
    std::fs::remove_file(dir.join(format!("{}.json", pid))).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("dvc-reg-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn write_read_remove_roundtrip() {
        let dir = tmp();
        let info = SessionInfo {
            pid: 42, cwd: "/home/j/proj".into(), distro: "Ubuntu".into(),
            project: "proj".into(), socket: "/run/user/1000/dvc/42.sock".into(), started_at: 1,
        };
        let path = write(&dir, &info).unwrap();
        assert_eq!(path, dir.join("42.json"));
        let back: SessionInfo = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.project, "proj");
        remove(&dir, 42);
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn runtime_dir_honors_override() {
        unsafe { std::env::set_var("DVC_RUNTIME_DIR", "/tmp/dvc-test-x") };
        assert_eq!(runtime_dir(), std::path::PathBuf::from("/tmp/dvc-test-x"));
        unsafe { std::env::remove_var("DVC_RUNTIME_DIR") };
    }
}
