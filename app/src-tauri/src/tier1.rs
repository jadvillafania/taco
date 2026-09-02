use std::io::Write;

pub enum Outcome {
    Ack,
    Rejected(String),
}

pub fn parse_response(stdout: &str, success: bool) -> Outcome {
    if success && stdout.contains(r#""type":"ack"#) {
        return Outcome::Ack;
    }
    let reason = serde_json::from_str::<serde_json::Value>(stdout.trim())
        .ok()
        .and_then(|v| v["reason"].as_str().map(String::from))
        .unwrap_or_else(|| "shim unreachable".into());
    Outcome::Rejected(reason)
}

/// ponytail: no explicit timeout here — shim's own 10s socket timeout bounds it;
/// add a watchdog if wsl.exe startup hangs ever become a real complaint.
pub fn send_via_shim(host: &crate::sessions::Host, sid: &str, payload: &str) -> Outcome {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = match host {
        crate::sessions::Host::Wsl { distro } => {
            let script = format!(r#""$HOME/.local/share/dvc/dvc-shim" send --session {sid}"#);
            let mut c = std::process::Command::new("wsl.exe");
            c.args(["-d", distro, "--", "sh", "-lc", &script]);
            c
        }
        crate::sessions::Host::Windows => {
            let mut c = std::process::Command::new(crate::deployer::native_shim_exe());
            c.args(["send", "--session", sid]);
            c
        }
    };
    let child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return Outcome::Rejected(format!("shim spawn failed: {e}")),
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(payload.as_bytes()).is_err() {
            return Outcome::Rejected("could not write payload".into());
        }
    }
    match child.wait_with_output() {
        Ok(out) => parse_response(&String::from_utf8_lossy(&out.stdout), out.status.success()),
        Err(e) => Outcome::Rejected(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_line_is_ack() {
        assert!(matches!(parse_response(r#"{"type":"ack","id":"1"}"#, true), Outcome::Ack));
    }

    #[test]
    fn error_line_and_failure_are_rejected() {
        match parse_response(r#"{"type":"error","id":"1","reason":"user-typing"}"#, false) {
            Outcome::Rejected(r) => assert!(r.contains("user-typing")),
            _ => panic!("expected rejection"),
        }
        assert!(matches!(parse_response("", false), Outcome::Rejected(_)));
    }

    #[test]
    fn windows_host_send_with_missing_exe_is_rejected_not_panic() {
        // no shim installed at %LOCALAPPDATA%...\bin during unit tests
        match send_via_shim(&crate::sessions::Host::Windows, "1", "x") {
            Outcome::Rejected(r) => assert!(!r.is_empty()),
            Outcome::Ack => panic!("cannot ack without a shim"),
        }
    }
}
