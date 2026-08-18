use crate::protocol::{self, Msg};
use std::io::{BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub fn send(args: &[String]) -> i32 {
    let mut sid = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--session" { sid = it.next().cloned(); }
    }
    let Some(sid) = sid else {
        eprintln!("send: --session <sid> required");
        return 2;
    };
    // connect before reading stdin so a dead session reports 3 even with no payload
    let sock = crate::registry::runtime_dir().join(format!("{sid}.sock"));
    let mut stream = match UnixStream::connect(&sock) {
        Ok(s) => s,
        Err(e) => { eprintln!("send: cannot connect {}: {e}", sock.display()); return 3; }
    };
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() || text.trim().is_empty() {
        eprintln!("send: payload expected on stdin");
        return 2;
    }
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let id = format!("{}", std::process::id());
    let msg = Msg::Send { id, text: text.trim_end().to_string() };
    if stream.write_all(protocol::to_line(&msg).as_bytes()).is_err() { return 3; }
    let mut line = String::new();
    if std::io::BufReader::new(stream).read_line(&mut line).is_err() || line.is_empty() {
        eprintln!("send: no response");
        return 4;
    }
    print!("{line}");
    match protocol::parse(&line) {
        Ok(Msg::Ack { .. }) => 0,
        _ => 4,
    }
}
