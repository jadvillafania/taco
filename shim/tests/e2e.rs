use std::io::{Read, Write};
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
