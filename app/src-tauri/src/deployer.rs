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
    crate::sessions::distros_from(&out)
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

pub const PROFILE_BLOCK: &str = concat!(
    "# >>> dvc-shim >>>\n",
    "function claude { & \"$env:LOCALAPPDATA\\DeveloperVisualCompanion\\bin\\dvc-shim.exe\" run claude @args }\n",
    "# <<< dvc-shim <<<\n",
);

pub fn append_block(existing: &str) -> String {
    if existing.contains(MARK_BEGIN) {
        return existing.to_string();
    }
    if existing.is_empty() {
        return PROFILE_BLOCK.to_string();
    }
    let sep = if existing.ends_with('\n') { "" } else { "\n" };
    format!("{existing}{sep}{PROFILE_BLOCK}")
}

pub fn strip_block(existing: &str) -> String {
    let (Some(b), Some(e)) = (existing.find(MARK_BEGIN), existing.find(MARK_END)) else {
        return existing.to_string();
    };
    let end = existing[e..].find('\n').map(|n| e + n + 1).unwrap_or(existing.len());
    format!("{}{}", &existing[..b], &existing[end..])
}

/// WinPS 5.1 profile always (present on every Windows); pwsh 7 profile when pwsh is installed.
/// ponytail: $HOME\Documents assumed — redirected Documents folders break this;
/// resolve via [Environment]::GetFolderPath if anyone hits it.
pub fn profile_paths() -> Vec<std::path::PathBuf> {
    let Ok(home) = std::env::var("USERPROFILE") else { return Vec::new() };
    let docs = std::path::PathBuf::from(home).join("Documents");
    let mut v = vec![docs.join("WindowsPowerShell").join("profile.ps1")];
    let has_pwsh = std::process::Command::new("where.exe").arg("pwsh")
        .creation_flags(CREATE_NO_WINDOW)
        .output().map(|o| o.status.success()).unwrap_or(false);
    if has_pwsh {
        v.push(docs.join("PowerShell").join("profile.ps1"));
    }
    v
}

/// Default client ExecutionPolicy is Restricted, under which profile.ps1 never
/// runs and the installed function is silently inert — warn instead of lying.
pub fn exec_policy_warning() -> Option<String> {
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", "Get-ExecutionPolicy"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let pol = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if pol == "Restricted" || pol == "AllSigned" {
        Some(format!(
            "PowerShell execution policy is {pol} — profile scripts are disabled, so the 'claude' wrapper won't load. Run: Set-ExecutionPolicy -Scope CurrentUser RemoteSigned"
        ))
    } else {
        None
    }
}

pub fn install_windows(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri::Manager;
    let src = app.path()
        .resolve("resources/dvc-shim.exe", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let bin = native_bin_dir();
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    std::fs::copy(&src, native_shim_exe())
        .map_err(|e| format!("could not install shim exe ({e}) — is a shim session still running, or was resources/dvc-shim.exe not built?"))?;
    for p in profile_paths() {
        if let Some(dir) = p.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
        let existing = std::fs::read_to_string(&p).unwrap_or_default();
        std::fs::write(&p, append_block(&existing)).map_err(|e| e.to_string())?;
    }
    Ok(exec_policy_warning())
}

pub fn remove_windows() -> Result<(), String> {
    for p in profile_paths() {
        if let Ok(existing) = std::fs::read_to_string(&p) {
            std::fs::write(&p, strip_block(&existing)).map_err(|e| e.to_string())?;
        }
    }
    std::fs::remove_file(native_shim_exe()).ok(); // absent exe is fine
    Ok(())
}

fn set_wsl_connected(app: &tauri::AppHandle, on: bool) -> Result<(), String> {
    let dir = crate::retention::data_dir(app);
    let mut s = crate::settings::load(&dir);
    s.wsl_connected = on;
    crate::settings::save(&dir, &s).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct ShimStatus {
    pub native: bool,
    pub wsl: bool,
}

/// Which shims are installed, for enabling the right button. Both checks are local:
/// the native exe is a file stat, and the WSL side reads the flag install/remove already
/// maintain — probing the distro would mean a wsl.exe spawn on every settings open.
#[tauri::command]
pub fn shim_status(app: tauri::AppHandle) -> ShimStatus {
    ShimStatus {
        native: native_shim_exe().exists(),
        wsl: crate::settings::load(&crate::retention::data_dir(&app)).wsl_connected,
    }
}

#[tauri::command]
pub async fn install_wsl_shim(app: tauri::AppHandle) -> Result<(), String> {
    let d = default_distro()?;
    install(&app, &d)?;
    set_wsl_connected(&app, true)
}

#[tauri::command]
pub async fn remove_wsl_shim(app: tauri::AppHandle) -> Result<(), String> {
    let result = default_distro().and_then(|d| remove(&d));
    set_wsl_connected(&app, false)?;
    result
}

#[tauri::command]
pub async fn install_native_shim(app: tauri::AppHandle) -> Result<Option<String>, String> {
    install_windows(&app)
}

#[tauri::command]
pub async fn remove_native_shim() -> Result<(), String> {
    remove_windows()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_is_idempotent_and_strip_restores() {
        let orig = "# my profile\nSet-Alias g git\n";
        let once = append_block(orig);
        assert!(once.contains("# >>> dvc-shim >>>"));
        assert!(once.contains(r#"function claude { & "$env:LOCALAPPDATA\DeveloperVisualCompanion\bin\dvc-shim.exe" run claude @args }"#));
        assert_eq!(append_block(&once), once, "second append is a no-op");
        assert_eq!(strip_block(&once), orig, "strip restores the original");
        assert_eq!(strip_block(orig), orig, "strip without block is a no-op");
    }

    #[test]
    fn append_to_empty_profile() {
        let s = append_block("");
        assert!(s.starts_with("# >>> dvc-shim >>>"));
        assert!(s.ends_with('\n'));
        assert_eq!(strip_block(&s), "");
    }
}
