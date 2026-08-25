use std::io::Write;
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MARK_BEGIN: &str = "# >>> dvc-shim >>>";
const MARK_END: &str = "# <<< dvc-shim <<<";

fn wsl_sh(distro: &str, script: &str, stdin_bytes: Option<&[u8]>) -> Result<String, String> {
    let mut child = std::process::Command::new("wsl.exe")
        .args(["-d", distro, "--", "sh", "-lc", script])
        .stdin(if stdin_bytes.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    if let (Some(bytes), Some(mut sin)) = (stdin_bytes, child.stdin.take()) {
        sin.write_all(bytes).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

pub fn default_distro() -> Result<String, String> {
    let out = std::process::Command::new("wsl.exe")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    crate::sessions::parse_wsl_list(&out.stdout)
        .into_iter()
        .next()
        .ok_or_else(|| "no WSL distribution found".into())
    // ponytail: first distro only; per-distro picker when someone runs several
}

pub fn install(app: &tauri::AppHandle, distro: &str) -> Result<(), String> {
    use tauri::Manager;
    let bin = app.path()
        .resolve("resources/dvc-shim", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&bin).map_err(|e| format!("shim binary missing ({e}) — build shim/ first"))?;
    wsl_sh(distro,
        r#"mkdir -p "$HOME/.local/share/dvc" && cat > "$HOME/.local/share/dvc/dvc-shim" && chmod +x "$HOME/.local/share/dvc/dvc-shim""#,
        Some(&bytes))?;
    // alias block is streamed via stdin: it contains single quotes, so it must
    // never be embedded inside a single-quoted shell string
    let alias = format!(
        "{MARK_BEGIN}\nalias claude='\"$HOME/.local/share/dvc/dvc-shim\" run claude'\n{MARK_END}\n"
    );
    wsl_sh(distro, &format!(
        r#"grep -qF '{MARK_BEGIN}' "$HOME/.bashrc" 2>/dev/null || cat >> "$HOME/.bashrc""#
    ), Some(alias.as_bytes()))?;
    Ok(())
}

pub fn remove(distro: &str) -> Result<(), String> {
    wsl_sh(distro, &format!(
        r#"sed -i '/{MARK_BEGIN}/,/{MARK_END}/d' "$HOME/.bashrc" 2>/dev/null; rm -f "$HOME/.local/share/dvc/dvc-shim""#
    ), None)?;
    Ok(())
}

pub fn native_bin_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into()))
        .join("DeveloperVisualCompanion").join("bin")
}

pub fn native_shim_exe() -> std::path::PathBuf {
    native_bin_dir().join("dvc-shim.exe")
}
