//! One import site for AF_UNIX sockets: std on unix, uds_windows on Windows
//! (AF_UNIX is native on Win10 1803+; the crate mirrors std's API).
#[cfg(unix)]
pub use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
pub use uds_windows::{UnixListener, UnixStream};
