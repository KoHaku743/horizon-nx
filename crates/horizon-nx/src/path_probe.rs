//! `HORIZON_NX_PATH_PROBE=1`: where a non-ASCII name gets changed on its way to the SD card (#7).
//!
//! A world named in Cyrillic saved into a folder whose name came out as Shift-JIS mojibake. This
//! makes one folder named `enc-std-Мир` through newlib and libnx's `fsdev` (the way the game
//! writes), and one named `enc-direct-Мир` straight through the FS service, skipping `fsdev`;
//! then it lists `switch/horizon` both ways and logs each probe folder's name as bytes. The
//! folders stay on the card so its names can be read on a computer too.

use core::ffi::{c_char, c_void};
use std::ffi::CString;

unsafe extern "C" {
    fn fsdevGetDeviceFileSystem(name: *const c_char) -> *mut c_void;
    fn fsFsCreateDirectory(fs: *mut c_void, path: *const c_char) -> u32;
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
    let word = "Мир";
    log(format_args!("[Path probe] the name's UTF-8: {word} = {}", hex(word.as_bytes())));

    // The game's way: std, newlib, the SD card device (vlinks), libnx fsdev.
    let std_dir = format!("sdmc:/switch/horizon/enc-std-{word}");
    log(format_args!("[Path probe] std::fs::create_dir({std_dir}): {:?}", std::fs::create_dir(&std_dir)));

    // Straight to the FS service.
    let fs = unsafe { fsdevGetDeviceFileSystem(c"sdmc".as_ptr()) };
    if fs.is_null() {
        log(format_args!("[Path probe] fsdevGetDeviceFileSystem(sdmc) gave nothing"));
    } else {
        let direct = CString::new(format!("/switch/horizon/enc-direct-{word}")).unwrap_or_default();
        let rc = unsafe { fsFsCreateDirectory(fs, direct.as_ptr()) };
        log(format_args!("[Path probe] fsFsCreateDirectory(/switch/horizon/enc-direct-{word}): {rc:#x}"));
    }

    // Listed through std.
    match std::fs::read_dir("sdmc:/switch/horizon") {
        Ok(entries) => {
            for e in entries.flatten() {
                let name = e.file_name();
                let bytes = name.as_encoded_bytes();
                if bytes.starts_with(b"enc-") {
                    log(format_args!(
                        "[Path probe] std lists {:?} = {}",
                        String::from_utf8_lossy(bytes),
                        hex(bytes)
                    ));
                }
            }
        }
        Err(e) => log(format_args!("[Path probe] std::fs::read_dir: {e}")),
    }

    // Listed through the FS service.
    if !fs.is_null() {
        let mut dir = FsDir([0; 8]);
        let rc = unsafe { fsFsOpenDirectory(fs, c"/switch/horizon".as_ptr(), READ_ALL, &mut dir) };
        if rc != 0 {
            log(format_args!("[Path probe] fsFsOpenDirectory: {rc:#x}"));
            return;
        }
        let empty = FsDirectoryEntry { name: [0; 0x301], pad: [0; 3], kind: 0, pad2: [0; 3], size: 0 };
        let mut buf = vec![empty; 64];
        loop {
            let mut total = 0i64;
            let rc = unsafe { fsDirRead(&mut dir, &mut total, buf.len(), buf.as_mut_ptr()) };
            if rc != 0 || total <= 0 {
                if rc != 0 {
                    log(format_args!("[Path probe] fsDirRead: {rc:#x}"));
                }
                break;
            }
            for entry in &buf[..total as usize] {
                let len = entry.name.iter().position(|&b| b == 0).unwrap_or(entry.name.len());
                let bytes = &entry.name[..len];
                if bytes.starts_with(b"enc-") {
                    log(format_args!(
                        "[Path probe] FS service lists {:?} = {}",
                        String::from_utf8_lossy(bytes),
                        hex(bytes)
                    ));
                }
            }
        }
        unsafe { fsDirClose(&mut dir) };
    }
}
