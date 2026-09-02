mod client;
mod protocol;
mod registry;
mod relay;
mod sockets;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") if args.len() > 1 => relay::run(&args[1..]),
        Some("send") => client::send(&args[1..]),
        _ => {
            eprintln!("usage: dvc-shim run <cmd> [args...] | dvc-shim send --session <sid>");
            2
        }
    };
    std::process::exit(code);
}
