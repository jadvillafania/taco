use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::protocol::{self, Msg};
use crate::registry::{self, SessionInfo};

pub const IDLE_WINDOW_MS: u64 = 1000;

pub struct Shared {
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub last_input: Arc<Mutex<Instant>>,
}

struct RawGuard(Option<nix::sys::termios::Termios>);
impl RawGuard {
    fn new() -> Self {
        use nix::sys::termios::*;
        let stdin = std::io::stdin();
        match tcgetattr(&stdin) {
            Ok(orig) => {
                let mut raw = orig.clone();
                cfmakeraw(&mut raw);
                tcsetattr(&stdin, SetArg::TCSANOW, &raw).ok();
                RawGuard(Some(orig))
            }
            Err(_) => RawGuard(None), // not a tty (tests, pipes): relay without raw mode
        }
    }
}
impl Drop for RawGuard {
    fn drop(&mut self) {
        if let Some(orig) = &self.0 {
            use nix::sys::termios::*;
            tcsetattr(&std::io::stdin(), SetArg::TCSANOW, orig).ok();
        }
    }
}

fn term_size() -> PtySize {
    let (cols, rows) = terminal_size::terminal_size()
        .map(|(w, h)| (w.0, h.0))
        .unwrap_or((80, 24));
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

pub fn run(args: &[String]) -> i32 {
    let dir = registry::runtime_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("dvc-shim: cannot create {}: {e}", dir.display());
        return 1;
    }
    let pid = std::process::id();
    let sock_path = dir.join(format!("{pid}.sock"));
    std::fs::remove_file(&sock_path).ok();
    let listener = match UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(e) => { eprintln!("dvc-shim: bind failed: {e}"); return 1; }
    };

    let pty = native_pty_system();
    let pair = match pty.openpty(term_size()) {
        Ok(p) => p,
        Err(e) => { eprintln!("dvc-shim: openpty failed: {e}"); return 1; }
    };
    let mut cmd = CommandBuilder::new(&args[0]);
    cmd.args(&args[1..]);
    let cwd = std::env::current_dir().unwrap_or_else(|_| "/".into());
    cmd.cwd(&cwd);
    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(e) => { eprintln!("dvc-shim: spawn {} failed: {e}", args[0]); return 1; }
    };
    drop(pair.slave);

    let info = SessionInfo {
        pid,
        cwd: cwd.to_string_lossy().into_owned(),
        distro: std::env::var("WSL_DISTRO_NAME").unwrap_or_default(),
        project: cwd.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        socket: sock_path.to_string_lossy().into_owned(),
        started_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
    };
    registry::write(&dir, &info).ok();

    let raw = RawGuard::new();
    let master = Arc::new(Mutex::new(pair.master));
    let mut reader = master.lock().unwrap().try_clone_reader().expect("clone reader");
    let shared = Shared {
        writer: Arc::new(Mutex::new(master.lock().unwrap().take_writer().expect("take writer"))),
        last_input: Arc::new(Mutex::new(Instant::now() - std::time::Duration::from_millis(IDLE_WINDOW_MS))),
    };

    // pty -> stdout
    std::thread::spawn(move || {
        let mut out = std::io::stdout();
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if out.write_all(&buf[..n]).is_err() { break; }
                    out.flush().ok();
                }
            }
        }
    });

    // stdin -> pty (tracks last_input for the injection idle gate)
    {
        let writer = shared.writer.clone();
        let last = shared.last_input.clone();
        std::thread::spawn(move || {
            let mut inp = std::io::stdin();
            let mut buf = [0u8; 8192];
            loop {
                match inp.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        *last.lock().unwrap() = Instant::now();
                        let mut w = writer.lock().unwrap();
                        if w.write_all(&buf[..n]).is_err() { break; }
                        w.flush().ok();
                    }
                }
            }
        });
    }

    // terminal resize -> pty resize
    // ponytail: 500ms size poll instead of SIGWINCH/console events — one code path
    // for unix+windows; event-driven resize if the lag ever bothers anyone.
    {
        let master = master.clone();
        std::thread::spawn(move || {
            let mut prev = term_size();
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let now = term_size();
                if now.rows != prev.rows || now.cols != prev.cols {
                    master.lock().unwrap().resize(now).ok();
                    prev = now;
                }
            }
        });
    }

    crate::relay::spawn_socket_listener(listener, &shared); // Task 6 (no-op stub until then)

    let status = child.wait().map(|s| s.exit_code() as i32).unwrap_or(1);
    registry::remove(&dir, pid);
    std::fs::remove_file(&sock_path).ok();
    drop(raw);
    status
}

const IDLE_TIMEOUT: Duration = Duration::from_secs(5);
const ENTER_DELAY: Duration = Duration::from_millis(150);

pub fn spawn_socket_listener(listener: UnixListener, shared: &Shared) {
    let writer = shared.writer.clone();
    let last = shared.last_input.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            handle_conn(stream, &writer, &last);
        }
    });
}

fn handle_conn(
    stream: std::os::unix::net::UnixStream,
    writer: &Arc<Mutex<Box<dyn Write + Send>>>,
    last_input: &Arc<Mutex<Instant>>,
) {
    use std::io::BufRead;
    let mut out = match stream.try_clone() { Ok(s) => s, Err(_) => return };
    let mut line = String::new();
    if std::io::BufReader::new(stream).read_line(&mut line).is_err() { return; }
    let (id, text) = match protocol::parse(&line) {
        Ok(Msg::Send { id, text }) => (id, text),
        _ => {
            out.write_all(protocol::to_line(&Msg::Error { id: "?".into(), reason: "bad-request".into() }).as_bytes()).ok();
            return;
        }
    };
    let reply = match inject(writer, last_input, &text) {
        Ok(()) => Msg::Ack { id },
        Err(reason) => Msg::Error { id, reason },
    };
    out.write_all(protocol::to_line(&reply).as_bytes()).ok();
}

/// Spec 27.4: wait for input-idle, write text, short delay, then Enter separately.
fn inject(
    writer: &Arc<Mutex<Box<dyn Write + Send>>>,
    last_input: &Arc<Mutex<Instant>>,
    text: &str,
) -> Result<(), String> {
    let start = Instant::now();
    loop {
        if last_input.lock().unwrap().elapsed() >= Duration::from_millis(IDLE_WINDOW_MS) {
            break;
        }
        if start.elapsed() > IDLE_TIMEOUT {
            return Err("user-typing".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    {
        let mut w = writer.lock().unwrap();
        w.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        w.flush().ok();
    }
    std::thread::sleep(ENTER_DELAY);
    let mut w = writer.lock().unwrap();
    w.write_all(b"\r").map_err(|e| e.to_string())?;
    w.flush().ok();
    Ok(())
}
