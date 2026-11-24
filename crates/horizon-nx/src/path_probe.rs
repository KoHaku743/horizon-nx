//! `HORIZON_NX_PATH_PROBE=1`: every name in `switch/horizon/saves`, as bytes, the way the game
//! lists it (std, through the SD card device) and the way the FS service gives it (#7).
//!
//! A folder named `Мир` on a computer didn't show in the world list, or in any other app on the
//! console: this says what the FS service hands back for such a name.

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    fn fsdevGetDeviceFileSystem(name: *const c_char) -> *mut c_void;
    fn fsFsOpenDirectory(fs: *mut c_void, path: *const c_char, mode: u32, out: *mut FsDir) -> u32;
    fn fsDirRead(dir: *mut FsDir, total: *mut i64, max: usize, buf: *mut FsDirectoryEntry) -> u32;
    fn fsDirClose(dir: *mut FsDir);
}

/// libnx `FsDir` (a `Service`), with room to spare.
#[repr(C)]
struct FsDir([u64; 8]);

/// libnx `FsDirectoryEntry`: 0x310 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct FsDirectoryEntry {
    name: [u8; 0x301],
    pad: [u8; 3],
    kind: i8,
    pad2: [u8; 3],
    size: i64,
}

const _: () = assert!(core::mem::size_of::<FsDirectoryEntry>() == 0x310);

/// `FsDirOpenMode_ReadDirs | FsDirOpenMode_ReadFiles`.
const READ_ALL: u32 = 3;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

fn log(args: std::fmt::Arguments<'_>) {
    crate::logging::write_log_line(args);
}

pub(crate) fn run_from_env() {
    if std::env::var_os("HORIZON_NX_PATH_PROBE").is_none_or(|v| v != "1") {
        return;
    }
    log(format_args!("[Path probe] Мир in UTF-8: {}", hex("Мир".as_bytes())));

    match std::fs::read_dir("sdmc:/switch/horizon/saves") {
        Ok(entries) => {
            let mut n = 0;
            for e in entries {
                match e {
                    Ok(e) => {
                        n += 1;
                        let name = e.file_name();
                        let bytes = name.as_encoded_bytes();
                        log(format_args!(
                            "[Path probe] std lists {:?} = {}",
                            String::from_utf8_lossy(bytes),
                            hex(bytes)
                        ));
                    }
                    Err(err) => log(format_args!("[Path probe] std listing error: {err}")),
                }
            }
            log(format_args!("[Path probe] std listed {n} entries"));
        }
        Err(e) => log(format_args!("[Path probe] std::fs::read_dir: {e}")),
    }

    let fs = unsafe { fsdevGetDeviceFileSystem(c"sdmc".as_ptr()) };
    if fs.is_null() {
        log(format_args!("[Path probe] fsdevGetDeviceFileSystem(sdmc) gave nothing"));
        return;
    }
    let mut dir = FsDir([0; 8]);
    let rc = unsafe { fsFsOpenDirectory(fs, c"/switch/horizon/saves".as_ptr(), READ_ALL, &mut dir) };
    if rc != 0 {
        log(format_args!("[Path probe] fsFsOpenDirectory: {rc:#x}"));
        return;
    }
    let empty = FsDirectoryEntry { name: [0; 0x301], pad: [0; 3], kind: 0, pad2: [0; 3], size: 0 };
    let mut buf = vec![empty; 16];
    let mut n = 0;
    loop {
        let mut total = 0i64;
        let rc = unsafe { fsDirRead(&mut dir, &mut total, buf.len(), buf.as_mut_ptr()) };
        log(format_args!("[Path probe] fsDirRead: rc {rc:#x}, {total} entries"));
        if rc != 0 || total <= 0 {
            break;
        }
        for entry in &buf[..(total as usize).min(buf.len())] {
            n += 1;
            let len = entry.name.iter().position(|&b| b == 0).unwrap_or(entry.name.len());
            let bytes = &entry.name[..len];
            log(format_args!(
                "[Path probe] FS service lists {:?} = {} (type {})",
                String::from_utf8_lossy(bytes),
                hex(bytes),
                entry.kind
            ));
        }
    }
    log(format_args!("[Path probe] FS service listed {n} entries"));
    unsafe { fsDirClose(&mut dir) };
}
