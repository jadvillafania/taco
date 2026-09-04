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
        // A script may legitimately never read stdin — the alias-block install
        // short-circuits its `cat` when grep finds the marker — which closes the pipe
        // under us (os error 109). Not a failure: the exit status below is the verdict.
        if let Err(e) = sin.write_all(bytes) {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(e.to_string());
            }
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn wsl_list(args: [&str; 2]) -> Result<std::process::Output, String> {
    std::process::Command::new("wsl.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())
}

/// Distros worth offering, real ones only.
pub fn distros() -> Vec<String> {
    wsl_list(["-l", "-q"]).map(|o| crate::sessions::distros_from(&o)).unwrap_or_default()
}

/// The one WSL itself considers default, falling back to the first listed.
pub fn default_distro() -> Result<String, String> {
    if let Some(d) = wsl_list(["-l", "-v"]).ok().and_then(|o| crate::sessions::default_from_verbose(&o.stdout)) {
        return Ok(d);
    }
    distros().into_iter().next().ok_or_else(|| "no WSL distribution found".into())
}

/// Where the shim goes: the distro picked in Settings, else WSL's default. Resolving
/// this must stay boot-free — the status probe calls it on every Settings open.
pub fn target_distro(app: &tauri::AppHandle) -> Result<String, String> {
    let pick = crate::settings::load(&crate::retention::data_dir(app)).wsl_distro;
    if !pick.is_empty() {
        return Ok(pick);
    }
    default_distro()
}

#[derive(serde::Serialize)]
pub struct DistroList {
    pub distros: Vec<String>,
    pub default: String,
}

#[tauri::command]
pub fn list_distros() -> DistroList {
    DistroList { distros: distros(), default: default_distro().unwrap_or_default() }
}

/// Applied on its own rather than through `set_settings`: both the Welcome window and
/// Settings offer this picker, and a window that echoes back a whole Settings struct it
/// doesn't fully render would reset the fields it never loaded.
#[tauri::command]
pub fn set_wsl_distro(app: tauri::AppHandle, distro: String) -> Result<(), String> {
    if !distro.is_empty() && !distros().contains(&distro) {
        return Err(format!("{distro}: no such WSL distribution"));
    }
    let dir = crate::retention::data_dir(&app);
    let mut s = crate::settings::load(&dir);
    s.wsl_distro = distro;
    crate::settings::save(&dir, &s).map_err(|e| e.to_string())
}

pub fn install(app: &tauri::AppHandle, distro: &str) -> Result<(), String> {
    use tauri::Manager;
    let bin = app.path()
        .resolve("resources/dvc-shim", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&bin).map_err(|e| format!("shim binary missing ({e}) — build shim/ first"))?;
    // Write-then-rename: truncating the shim in place fails with ETXTBSY while a
    // wrapped `claude` session is running it. rename(2) over a busy binary is fine —
    // the live process keeps the old inode and picks up the new one on next launch.
    // Paths are spelled out rather than held in a shell variable: a `d=...` assignment
    // did not survive wsl.exe's argument handling on some hosts, leaving mkdir with an
    // empty operand. Every command here must expand "$HOME" itself.
    wsl_sh(distro,
        r#"mkdir -p "$HOME/.local/share/dvc" && cat > "$HOME/.local/share/dvc/dvc-shim.new" && chmod +x "$HOME/.local/share/dvc/dvc-shim.new" && mv -f "$HOME/.local/share/dvc/dvc-shim.new" "$HOME/.local/share/dvc/dvc-shim""#,
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

pub fn native_cmd_wrapper() -> std::path::PathBuf {
    native_bin_dir().join("claude.cmd")
}

/// cmd.exe has no profile and no functions, so its wrapper has to be a real file on
/// PATH. It must invoke the *resolved* claude binary, never the bare name: our own
/// claude.cmd sits earlier on PATH and would re-enter itself forever.
/// ponytail: a .cmd/.bat claude (npm install) can't be spawned by CreateProcess —
/// same ceiling the PowerShell wrapper already has; wrap in `cmd /c` if anyone hits it.
pub fn cmd_wrapper_body(real_claude: &str) -> String {
    format!(
        "@echo off\r\n\"%LOCALAPPDATA%\\DeveloperVisualCompanion\\bin\\dvc-shim.exe\" run \"{real_claude}\" %*\r\n"
    )
}

/// First `where claude` hit that isn't our own wrapper (a reinstall would otherwise
/// bake a self-reference).
pub fn real_claude_path() -> Option<String> {
    let out = std::process::Command::new("where.exe")
        .arg("claude")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let ours = native_bin_dir();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .find(|l| !std::path::Path::new(l).starts_with(&ours))
        .map(str::to_string)
}

fn same_dir(a: &str, b: &str) -> bool {
    a.trim().trim_end_matches('\\').eq_ignore_ascii_case(b.trim().trim_end_matches('\\'))
}

/// `None` when it's already there — nothing to write.
pub fn path_prepend(existing: &str, bin: &str) -> Option<String> {
    if existing.split(';').any(|p| same_dir(p, bin)) { return None; }
    Some(if existing.is_empty() { bin.to_string() } else { format!("{bin};{existing}") })
}

pub fn path_remove(existing: &str, bin: &str) -> String {
    existing.split(';').filter(|p| !same_dir(p, bin)).collect::<Vec<_>>().join(";")
}

/// ponytail: the user PATH is read/written through .NET rather than the registry —
/// it broadcasts WM_SETTINGCHANGE for us, and `reg.exe` is blocked by policy on some
/// machines. Cost: a REG_EXPAND_SZ user PATH gets expanded once on write.
fn ps(script: &str) -> Option<String> {
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

fn user_path() -> String {
    ps("[Environment]::GetEnvironmentVariable('Path','User')").unwrap_or_default()
}

fn set_user_path(v: &str) -> Result<(), String> {
    let escaped = v.replace('\'', "''");
    ps(&format!("[Environment]::SetEnvironmentVariable('Path','{escaped}','User')"))
        .map(|_| ())
        .ok_or_else(|| "could not update the user PATH".to_string())
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
    let mut warnings: Vec<String> = exec_policy_warning().into_iter().collect();
    match real_claude_path() {
        Some(real) => {
            std::fs::write(native_cmd_wrapper(), cmd_wrapper_body(&real)).map_err(|e| e.to_string())?;
            let bin_str = bin.to_string_lossy().into_owned();
            if let Some(next) = path_prepend(&user_path(), &bin_str) {
                set_user_path(&next)?;
            }
            warnings.push("Open a new terminal — Command Prompt picks up the wrapper from PATH.".into());
        }
        None => warnings.push(
            "claude was not found on PATH, so the Command Prompt wrapper was skipped — PowerShell still works.".into(),
        ),
    }
    Ok((!warnings.is_empty()).then(|| warnings.join(" ")))
}

pub fn remove_windows() -> Result<(), String> {
    for p in profile_paths() {
        if let Ok(existing) = std::fs::read_to_string(&p) {
            std::fs::write(&p, strip_block(&existing)).map_err(|e| e.to_string())?;
        }
    }
    std::fs::remove_file(native_cmd_wrapper()).ok(); // absent wrapper is fine
    let bin_str = native_bin_dir().to_string_lossy().into_owned();
    let current = user_path();
    let stripped = path_remove(&current, &bin_str);
    if stripped != current { set_user_path(&stripped)?; }
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

/// Is the WSL shim really there? The `wsl_connected` flag is Windows-side state and
/// can go stale (wiped settings, shim installed by another install of the app), so
/// probe the distro when it's already running — `-l --running` never boots one, and a
/// boot just to grey a button would be worse than a stale answer. A probe that finds
/// the shim heals the flag, otherwise the send gate would stay off with the Install
/// button greyed out and no way back.
fn wsl_installed(app: &tauri::AppHandle) -> bool {
    let flag = crate::settings::load(&crate::retention::data_dir(app)).wsl_connected;
    let Ok(d) = target_distro(app) else { return flag };
    let up = std::process::Command::new("wsl.exe")
        .args(["-l", "-q", "--running"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| crate::sessions::distros_from(&o).contains(&d))
        .unwrap_or(false);
    if !up {
        return flag;
    }
    let installed = wsl_sh(&d, &format!(
        r#"test -x "$HOME/.local/share/dvc/dvc-shim" && grep -qF '{MARK_BEGIN}' "$HOME/.bashrc""#
    ), None).is_ok();
    if installed && !flag {
        let _ = set_wsl_connected(app, true);
    }
    installed
}

/// Which shims are installed, for enabling the right button. The native exe is a
/// plain file stat; WSL needs the probe above.
#[tauri::command]
pub fn shim_status(app: tauri::AppHandle) -> ShimStatus {
    ShimStatus {
        native: native_shim_exe().exists(),
        wsl: wsl_installed(&app),
    }
}

#[tauri::command]
pub async fn install_wsl_shim(app: tauri::AppHandle) -> Result<(), String> {
    let d = target_distro(&app)?;
    // Name the distro in the error: a failure in the wrong distro (or one with no user
    // home) is unreadable otherwise.
    install(&app, &d).map_err(|e| format!("{d}: {e}"))?;
    set_wsl_connected(&app, true)
}

#[tauri::command]
pub async fn remove_wsl_shim(app: tauri::AppHandle) -> Result<(), String> {
    let result = target_distro(&app).and_then(|d| remove(&d));
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
    fn cmd_wrapper_calls_the_resolved_binary_not_the_name() {
        let body = cmd_wrapper_body(r"C:\Users\x\.local\bin\claude.exe");
        assert!(body.contains(r#""C:\Users\x\.local\bin\claude.exe" %*"#), "{body}");
        assert!(!body.contains("run claude "), "bare name would re-enter this wrapper: {body}");
    }

    #[test]
    fn path_prepend_and_remove_round_trip() {
        let bin = r"C:\Users\x\AppData\Local\DeveloperVisualCompanion\bin";
        let orig = r"C:\Users\x\.local\bin;C:\other";
        let with = path_prepend(orig, bin).expect("added");
        assert!(with.starts_with(bin), "must win the PATH search: {with}");
        assert_eq!(path_prepend(&with, bin), None, "second install is a no-op");
        assert_eq!(path_remove(&with, bin), orig);
        assert_eq!(path_remove(orig, bin), orig, "remove without the entry is a no-op");
        // trailing-slash and case variants are the same directory
        assert_eq!(path_prepend(&format!("{}\\", bin.to_lowercase()), bin), None);
    }

    #[test]
    fn append_to_empty_profile() {
        let s = append_block("");
        assert!(s.starts_with("# >>> dvc-shim >>>"));
        assert!(s.ends_with('\n'));
        assert_eq!(strip_block(&s), "");
    }
}
