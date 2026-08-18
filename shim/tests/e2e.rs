use std::io::{BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub fn spawn_cat(tmp: &std::path::Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_dvc-shim"))
        .args(["run", "cat"])
        .env("DVC_RUNTIME_DIR", tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
}

pub fn wait_for<F: Fn() -> bool>(what: &str, f: F) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(10), "timeout waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn read_until(out: &mut impl Read, needle: &str) -> String {
    let mut buf = Vec::new();
    let t0 = Instant::now();
    let mut byte = [0u8; 1];
    while !String::from_utf8_lossy(&buf).contains(needle) {
        assert!(t0.elapsed() < Duration::from_secs(10), "timeout; got: {:?}", String::from_utf8_lossy(&buf));
        if out.read(&mut byte).unwrap_or(0) == 1 { buf.push(byte[0]); }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[test]
fn relays_io_and_manages_registry() {
    let tmp = std::env::temp_dir().join(format!("dvc-e2e-relay-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut child = spawn_cat(&tmp);
    let pid = child.id();
    let reg = tmp.join(format!("{pid}.json"));
    let sock = tmp.join(format!("{pid}.sock"));
    wait_for("registry", || reg.exists() && sock.exists());

    let info: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&reg).unwrap()).unwrap();
    assert_eq!(info["pid"], pid);
    assert!(info["socket"].as_str().unwrap().ends_with(&format!("{pid}.sock")));

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    stdin.write_all(b"hello\n").unwrap();
    stdin.flush().unwrap();
    read_until(&mut stdout, "hello");

    stdin.write_all(&[0x04]).unwrap(); // EOF -> cat exits -> shim cleans up
    stdin.flush().unwrap();
    child.wait().unwrap();
    assert!(!reg.exists(), "registry removed on exit");
    assert!(!sock.exists(), "socket removed on exit");
    std::fs::remove_dir_all(&tmp).ok();
}

fn sock_send(sock: &std::path::Path, text: &str) -> String {
    let mut s = UnixStream::connect(sock).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(format!(r#"{{"type":"send","id":"t1","text":"{text}"}}"#).as_bytes()).unwrap();
    s.write_all(b"\n").unwrap();
    let mut line = String::new();
    std::io::BufReader::new(s).read_line(&mut line).unwrap();
    line
}

#[test]
fn injects_when_idle() {
    let tmp = std::env::temp_dir().join(format!("dvc-e2e-inj-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut child = spawn_cat(&tmp);
    let sock = tmp.join(format!("{}.sock", child.id()));
    wait_for("socket", || sock.exists());
    std::thread::sleep(Duration::from_millis(1100)); // let the idle window elapse

    let resp = sock_send(&sock, "injected-hello");
    assert!(resp.contains(r#""type":"ack"#), "got: {resp}");
    let mut stdout = child.stdout.take().unwrap();
    read_until(&mut stdout, "injected-hello"); // cat echoed the injected text

    child.stdin.take().unwrap().write_all(&[0x04]).unwrap();
    child.wait().unwrap();
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn refuses_while_user_is_typing() {
    let tmp = std::env::temp_dir().join(format!("dvc-e2e-busy-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut child = spawn_cat(&tmp);
    let sock = tmp.join(format!("{}.sock", child.id()));
    wait_for("socket", || sock.exists());

    let mut stdin = child.stdin.take().unwrap();
    let typing = std::thread::spawn(move || {
        for _ in 0..40 { // ~8s of keystrokes every 200ms: idle window never reached
            if stdin.write_all(b"x").is_err() { return stdin; }
            stdin.flush().ok();
            std::thread::sleep(Duration::from_millis(200));
        }
        stdin
    });
    std::thread::sleep(Duration::from_millis(500)); // let keystrokes register before sending
    let resp = sock_send(&sock, "should-not-inject");
    assert!(resp.contains(r#""type":"error"#) && resp.contains("user-typing"), "got: {resp}");

    let mut stdin = typing.join().unwrap();
    // newline first: EOF on a non-empty canonical-mode line only flushes it
    stdin.write_all(b"\n").unwrap();
    stdin.write_all(&[0x04]).unwrap();
    child.wait().unwrap();
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn send_client_roundtrip_and_missing_session() {
    let tmp = std::env::temp_dir().join(format!("dvc-e2e-cli-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut child = spawn_cat(&tmp);
    let sid = child.id().to_string();
    let sock = tmp.join(format!("{sid}.sock"));
    wait_for("socket", || sock.exists());
    std::thread::sleep(Duration::from_millis(1100));

    let mut cli = Command::new(env!("CARGO_BIN_EXE_dvc-shim"))
        .args(["send", "--session", &sid])
        .env("DVC_RUNTIME_DIR", &tmp)
        .stdin(Stdio::piped()).stdout(Stdio::piped())
        .spawn().unwrap();
    cli.stdin.take().unwrap().write_all(b"from-client\n").unwrap();
    let out = cli.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "stdout: {}", String::from_utf8_lossy(&out.stdout));
    assert!(String::from_utf8_lossy(&out.stdout).contains(r#""type":"ack"#));

    let out = Command::new(env!("CARGO_BIN_EXE_dvc-shim"))
        .args(["send", "--session", "999999"])
        .env("DVC_RUNTIME_DIR", &tmp)
        .stdin(Stdio::null()).stdout(Stdio::piped())
        .output().unwrap();
    assert_eq!(out.status.code(), Some(3));

    child.stdin.take().unwrap().write_all(&[0x04]).unwrap();
    child.wait().unwrap();
    std::fs::remove_dir_all(&tmp).ok();
}
