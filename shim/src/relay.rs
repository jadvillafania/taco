use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::time::Instant;

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

    // SIGWINCH -> pty resize
    {
        let master = master.clone();
        if let Ok(mut signals) = signal_hook::iterator::Signals::new([signal_hook::consts::SIGWINCH]) {
            std::thread::spawn(move || {
                for _ in signals.forever() {
                    master.lock().unwrap().resize(term_size()).ok();
                }
            });
        }
    }

    crate::relay::spawn_socket_listener(listener, &shared); // Task 6 (no-op stub until then)

    let status = child.wait().map(|s| s.exit_code() as i32).unwrap_or(1);
    registry::remove(&dir, pid);
    std::fs::remove_file(&sock_path).ok();
    drop(raw);
    status
}

pub fn spawn_socket_listener(_listener: UnixListener, _shared: &Shared) {
    // Task 6
}
