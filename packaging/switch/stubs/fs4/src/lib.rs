//! fs4 on the Switch: newlib on Horizon OS has no flock or statvfs. The game only asks how much
//! space a drive has (`save.rs`, `server_perf/system.rs`), and takes an error as "unknown".
use std::io;
use std::path::Path;

fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "no statvfs on the Switch")
}

pub fn available_space(_path: impl AsRef<Path>) -> io::Result<u64> {
    Err(unsupported())
}

pub fn free_space(_path: impl AsRef<Path>) -> io::Result<u64> {
    Err(unsupported())
}

pub fn total_space(_path: impl AsRef<Path>) -> io::Result<u64> {
    Err(unsupported())
}
