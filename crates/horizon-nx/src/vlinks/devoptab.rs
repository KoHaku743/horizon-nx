//! The SD card's newlib device, with links: a copy of libnx's `fsdev` device table whose path
//! operations go through [`super::Links`] first. File-handle operations (read, write, seek...)
//! are libnx's own; opens, reads, writes, closes, `stat`s and listings are counted and timed
//! (`io_stats.rs`).

use super::io_stats::{CLOSE, LIST, OPEN, PROBE, READ, STAT, WRITE};
use super::{Links, MANIFEST, Parts, Store, Then, normalize, render};
use super::{pack, pack_device};
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use std::ffi::CString;
use std::sync::OnceLock;
use std::time::Instant;

type R = *mut c_void;
type Stat = libc::stat;

/// newlib `DIR_ITER` (`sys/iosupport.h`).
#[repr(C)]
struct DirIter {
    device: c_int,
    dir_struct: *mut c_void,
}

/// newlib `devoptab_t` (`sys/iosupport.h`, devkitPro's newlib), field for field.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct DevOptab {
    name: *const c_char,
    struct_size: usize,
    pub(super) open_r:
        Option<unsafe extern "C" fn(R, *mut c_void, *const c_char, c_int, c_int) -> c_int>,
    pub(super) close_r: Option<unsafe extern "C" fn(R, *mut c_void) -> c_int>,
    write_r: Option<unsafe extern "C" fn(R, *mut c_void, *const c_char, usize) -> isize>,
    read_r: Option<unsafe extern "C" fn(R, *mut c_void, *mut c_char, usize) -> isize>,
    pub(super) seek_r: Option<unsafe extern "C" fn(R, *mut c_void, i64, c_int) -> i64>,
    fstat_r: Option<unsafe extern "C" fn(R, *mut c_void, *mut Stat) -> c_int>,
    stat_r: Option<unsafe extern "C" fn(R, *const c_char, *mut Stat) -> c_int>,
    link_r: Option<unsafe extern "C" fn(R, *const c_char, *const c_char) -> c_int>,
    unlink_r: Option<unsafe extern "C" fn(R, *const c_char) -> c_int>,
    chdir_r: Option<unsafe extern "C" fn(R, *const c_char) -> c_int>,
    rename_r: Option<unsafe extern "C" fn(R, *const c_char, *const c_char) -> c_int>,
    mkdir_r: Option<unsafe extern "C" fn(R, *const c_char, c_int) -> c_int>,
    dir_state_size: usize,
    diropen_r: Option<unsafe extern "C" fn(R, *mut DirIter, *const c_char) -> *mut DirIter>,
    dirreset_r: Option<unsafe extern "C" fn(R, *mut DirIter) -> c_int>,
    dirnext_r: Option<unsafe extern "C" fn(R, *mut DirIter, *mut c_char, *mut Stat) -> c_int>,
    dirclose_r: Option<unsafe extern "C" fn(R, *mut DirIter) -> c_int>,
    statvfs_r: Option<unsafe extern "C" fn(R, *const c_char, *mut c_void) -> c_int>,
    ftruncate_r: Option<unsafe extern "C" fn(R, *mut c_void, i64) -> c_int>,
    fsync_r: Option<unsafe extern "C" fn(R, *mut c_void) -> c_int>,
    device_data: *mut c_void,
    chmod_r: Option<unsafe extern "C" fn(R, *const c_char, libc::mode_t) -> c_int>,
    fchmod_r: Option<unsafe extern "C" fn(R, *mut c_void, libc::mode_t) -> c_int>,
    rmdir_r: Option<unsafe extern "C" fn(R, *const c_char) -> c_int>,
    lstat_r: Option<unsafe extern "C" fn(R, *const c_char, *mut Stat) -> c_int>,
    utimes_r: Option<unsafe extern "C" fn(R, *const c_char, *const c_void) -> c_int>,
    fpathconf_r: Option<unsafe extern "C" fn(R, *mut c_void, c_int) -> c_long>,
    pathconf_r: Option<unsafe extern "C" fn(R, *const c_char, c_int) -> c_long>,
    symlink_r: Option<unsafe extern "C" fn(R, *const c_char, *const c_char) -> c_int>,
    readlink_r: Option<unsafe extern "C" fn(R, *const c_char, *mut c_char, usize) -> isize>,
}
const _: () = assert!(size_of::<DevOptab>() == crate::logging::ffi::DEVOPTAB_WORDS * 8);

struct Original(DevOptab);
// SAFETY: a copy of libnx's static table: function pointers, a name and libnx's device record,
// only ever read.
unsafe impl Send for Original {}
unsafe impl Sync for Original {}

/// libnx's own SD card device, which every operation ends up in.
static ORIGINAL: OnceLock<Original> = OnceLock::new();

pub(super) fn original() -> &'static DevOptab {
    // Set before the device that calls this is in newlib's table.
    &ORIGINAL
        .get()
        .expect("vlinks: the SD card device isn't installed")
        .0
}

unsafe extern "C" {
    /// libsysbase: the index in `devoptab_list` of the device a path names.
    fn FindDevice(name: *const c_char) -> c_int;
    fn getcwd(buf: *mut c_char, size: usize) -> *mut c_char;
    /// newlib: this thread's `struct _reent`, whose first field is `errno`.
    fn __getreent() -> R;
}

/// The links of the SD card.
static LINKS: Links<Sd> = Links::new(Sd { open: None });

/// Manifests on the SD card, read and written with libnx's own device (never through the links).
struct Sd {
    /// The manifest last appended to, kept open (and its folder): one write per link while a
    /// builder links a folder's worth of files, instead of an open, a write and a close.
    open: Option<(String, Vec<u64>)>,
}

/// A buffer for libnx's file record (`fsdev_file_t`).
pub(super) fn file_struct() -> Vec<u64> {
    vec![0; original().struct_size.div_ceil(8)]
}

/// This thread's `struct _reent`.
pub(super) fn reent() -> R {
    unsafe { __getreent() }
}

impl Sd {
    fn file_struct() -> Vec<u64> {
        file_struct()
    }
}

impl Store for Sd {
    fn read(&mut self, dir: &str) -> Option<Vec<u8>> {
        let started = Instant::now();
        // The pack's index lists every file of the packed folder: no manifest to look for.
        if super::pack_device::no_manifest(dir) {
            return None;
        }
        let found = (|| {
            let path = c(&super::join(dir, MANIFEST));
            let o = original();
            let mut file = Sd::file_struct();
            let r = unsafe { __getreent() };
            let fd = file.as_mut_ptr().cast::<c_void>();
            if unsafe { o.open_r?(r, fd, path.as_ptr(), libc::O_RDONLY, 0) } == -1 {
                return None;
            }
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = unsafe { o.read_r?(r, fd, chunk.as_mut_ptr().cast(), chunk.len()) };
                if n <= 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n as usize]);
            }
            unsafe { o.close_r?(r, fd) };
            Some(bytes)
        })();
        PROBE.add(started, u64::from(found.is_some()));
        found
    }

    fn append(&mut self, dir: &str, text: &str) -> Result<(), i32> {
        let o = original();
        let r = unsafe { __getreent() };
        let (Some(open_r), Some(write_r), Some(seek_r)) = (o.open_r, o.write_r, o.seek_r) else {
            return Err(super::EIO);
        };
        if self.open.as_ref().is_none_or(|(open, _)| open != dir) {
            self.release();
            let path = c(&super::join(dir, MANIFEST));
            let mut file = Sd::file_struct();
            let fd = file.as_mut_ptr().cast::<c_void>();
            if unsafe { open_r(r, fd, path.as_ptr(), libc::O_WRONLY | libc::O_CREAT, 0o666) } == -1
            {
                return Err(errno(r));
            }
            unsafe { seek_r(r, fd, 0, libc::SEEK_END) };
            self.open = Some((dir.to_owned(), file));
        }
        let (_, file) = self.open.as_mut().unwrap();
        let fd = file.as_mut_ptr().cast::<c_void>();
        let mut rest = text.as_bytes();
        while !rest.is_empty() {
            let n = unsafe { write_r(r, fd, rest.as_ptr().cast(), rest.len()) };
            if n <= 0 {
                let e = errno(r);
                self.release();
                return Err(e);
            }
            rest = &rest[n as usize..];
        }
        // On the card now, not when the file closes: a console that crashes keeps its links.
        if let Some(fsync_r) = o.fsync_r {
            unsafe { fsync_r(r, fd) };
        }
        Ok(())
    }

    fn remove(&mut self, dir: &str) -> Result<(), i32> {
        // The file system won't delete an open file.
        self.release();
        let path = c(&super::join(dir, MANIFEST));
        let r = unsafe { __getreent() };
        let Some(unlink_r) = original().unlink_r else {
            return Err(super::EIO);
        };
        if unsafe { unlink_r(r, path.as_ptr()) } == -1
            && self.kind(&super::join(dir, MANIFEST)).is_some()
        {
            return Err(errno(r));
        }
        Ok(())
    }

    fn kind(&mut self, path: &str) -> Option<bool> {
        let path = c(path);
        let mut st: Stat = unsafe { core::mem::zeroed() };
        let r = unsafe { __getreent() };
        if unsafe { original().stat_r?(r, path.as_ptr(), &mut st) } == -1 {
            return None;
        }
        Some(st.st_mode & libc::S_IFMT == libc::S_IFDIR)
    }

    fn release(&mut self) {
        if let Some((_, mut file)) = self.open.take() {
            let r = unsafe { __getreent() };
            if let Some(close_r) = original().close_r {
                unsafe { close_r(r, file.as_mut_ptr().cast()) };
            }
        }
    }
}

fn errno(r: R) -> c_int {
    match unsafe { *r.cast::<c_int>() } {
        0 => super::EIO,
        e => e,
    }
}

pub(super) fn fail(r: R, e: c_int) -> c_int {
    unsafe { *r.cast::<c_int>() = e };
    -1
}

/// newlib's working directory, `sdmc:/...`.
pub(super) fn cwd() -> String {
    let mut buf = [0u8; 1024];
    let at = unsafe { getcwd(buf.as_mut_ptr().cast(), buf.len()) };
    if at.is_null() {
        return "sdmc:/".into();
    }
    unsafe { CStr::from_ptr(at) }.to_string_lossy().into_owned()
}

/// A path argument as absolute parts; `None` (pass it on as it is) when it isn't UTF-8.
unsafe fn parts(path: *const c_char) -> Option<Parts> {
    let path = unsafe { CStr::from_ptr(path) }.to_str().ok()?;
    Some(normalize(path, &cwd()))
}

/// `path` as the SD card's file system takes it: in Shift-JIS (`sjis.rs`, #7).
fn c(path: &str) -> CString {
    // From parts of a C string: no NUL inside, and Shift-JIS adds none.
    CString::new(super::sjis::to_fs(path).into_owned()).unwrap_or_default()
}

/// Calls `f` with `path` gone through the links (with `follow`, its last part too).
unsafe fn through<T>(
    r: R,
    path: *const c_char,
    follow: bool,
    failed: T,
    f: impl FnOnce(*const c_char) -> T,
) -> T {
    let Some(asked) = (unsafe { parts(path) }) else {
        return f(path);
    };
    match LINKS.resolve(&asked, follow) {
        // Always the clean path, even with no link on it: libnx's own parser takes everything after
        // the first `:`, so `sdmc:/switch/horizon/sdmc:/x` (what `std::path::absolute` makes of
        // `sdmc:/x`, which Rust doesn't count as absolute) failed with EINVAL, and mods with it.
        Ok(real) => f(c(&render(&real)).as_ptr()),
        Err(e) => {
            fail(r, e);
            failed
        }
    }
}

unsafe extern "C" fn open_r(
    r: R,
    file: *mut c_void,
    path: *const c_char,
    flags: c_int,
    mode: c_int,
) -> c_int {
    let started = Instant::now();
    let rc = unsafe { open_inner(r, file, path, flags, mode) };
    OPEN.add(started, u64::from(rc == -1));
    rc
}

unsafe fn open_inner(
    r: R,
    file: *mut c_void,
    path: *const c_char,
    flags: c_int,
    mode: c_int,
) -> c_int {
    let real_open = original().open_r.unwrap();
    let Some(asked) = (unsafe { parts(path) }) else {
        return unsafe { real_open(r, file, path, flags, mode) };
    };
    let real = match LINKS.resolve(&asked, true) {
        Ok(real) => real,
        Err(e) => return fail(r, e),
    };
    let writes = flags & libc::O_ACCMODE != libc::O_RDONLY
        || flags & (libc::O_CREAT | libc::O_TRUNC | libc::O_APPEND) != 0;
    if writes {
        pack_device::changing(&real);
    } else {
        match pack_device::answer(&real) {
            pack_device::Answer::Missing => return fail(r, super::ENOENT),
            pack_device::Answer::Entry(entry, mount) => {
                if let Some(at) = pack_device::packed_at(&entry) {
                    pack_device::open_packed(file, mount, at, &entry);
                    return 0;
                }
            }
            pack_device::Answer::Folder => {}
        }
    }
    unsafe { real_open(r, file, c(&render(&real)).as_ptr(), flags, mode) }
}

unsafe extern "C" fn read_r(r: R, file: *mut c_void, ptr: *mut c_char, len: usize) -> isize {
    let started = Instant::now();
    let n = if pack_device::is_packed(file) {
        pack_device::read(r, file, ptr.cast(), len)
    } else {
        unsafe { original().read_r.unwrap()(r, file, ptr, len) }
    };
    READ.add(started, n.max(0) as u64);
    n
}

unsafe extern "C" fn write_r(r: R, file: *mut c_void, ptr: *const c_char, len: usize) -> isize {
    if pack_device::is_packed(file) {
        return fail(r, libc::EBADF) as isize;
    }
    let started = Instant::now();
    let n = unsafe { original().write_r.unwrap()(r, file, ptr, len) };
    WRITE.add(started, n.max(0) as u64);
    n
}

unsafe extern "C" fn close_r(r: R, file: *mut c_void) -> c_int {
    let started = Instant::now();
    let rc = if pack_device::is_packed(file) {
        pack_device::close(file);
        0
    } else {
        unsafe { original().close_r.unwrap()(r, file) }
    };
    CLOSE.add(started, 0);
    rc
}

unsafe extern "C" fn seek_r(r: R, file: *mut c_void, pos: i64, whence: c_int) -> i64 {
    if pack_device::is_packed(file) {
        return pack_device::seek(r, file, pos, whence);
    }
    unsafe { original().seek_r.unwrap()(r, file, pos, whence) }
}

unsafe extern "C" fn fstat_r(r: R, file: *mut c_void, st: *mut Stat) -> c_int {
    if pack_device::is_packed(file) {
        return unsafe { pack_device::fstat(file, st) };
    }
    unsafe { original().fstat_r.unwrap()(r, file, st) }
}

unsafe extern "C" fn ftruncate_r(r: R, file: *mut c_void, len: i64) -> c_int {
    if pack_device::is_packed(file) {
        return fail(r, libc::EBADF);
    }
    unsafe { original().ftruncate_r.unwrap()(r, file, len) }
}

unsafe extern "C" fn fsync_r(r: R, file: *mut c_void) -> c_int {
    if pack_device::is_packed(file) {
        return 0;
    }
    unsafe { original().fsync_r.unwrap()(r, file) }
}

unsafe extern "C" fn fchmod_r(r: R, file: *mut c_void, mode: libc::mode_t) -> c_int {
    if pack_device::is_packed(file) {
        return fail(r, libc::EBADF);
    }
    unsafe { original().fchmod_r.unwrap()(r, file, mode) }
}

unsafe extern "C" fn fpathconf_r(r: R, file: *mut c_void, name: c_int) -> c_long {
    if pack_device::is_packed(file) {
        return fail(r, libc::EINVAL) as c_long;
    }
    unsafe { original().fpathconf_r.unwrap()(r, file, name) }
}

/// `stat` from the pack's index when it can answer.
unsafe fn stat_packed(r: R, real: &[String], st: *mut Stat) -> Option<c_int> {
    match pack_device::answer(real) {
        pack_device::Answer::Entry(entry, _) => {
            unsafe { pack_device::fill(st, &entry) };
            Some(0)
        }
        pack_device::Answer::Missing => Some(fail(r, super::ENOENT)),
        pack_device::Answer::Folder => None,
    }
}

unsafe extern "C" fn stat_r(r: R, path: *const c_char, st: *mut Stat) -> c_int {
    let started = Instant::now();
    let rc = match unsafe { parts(path) }.map(|asked| LINKS.resolve(&asked, true)) {
        Some(Ok(real)) => match unsafe { stat_packed(r, &real, st) } {
            Some(rc) => rc,
            None => unsafe { original().stat_r.unwrap()(r, c(&render(&real)).as_ptr(), st) },
        },
        Some(Err(e)) => fail(r, e),
        None => unsafe { original().stat_r.unwrap()(r, path, st) },
    };
    STAT.add(started, u64::from(rc == -1));
    rc
}

unsafe extern "C" fn lstat_r(r: R, path: *const c_char, st: *mut Stat) -> c_int {
    let started = Instant::now();
    let rc = unsafe { lstat_inner(r, path, st) };
    STAT.add(started, u64::from(rc == -1));
    rc
}

unsafe fn lstat_inner(r: R, path: *const c_char, st: *mut Stat) -> c_int {
    let o = original();
    let real_lstat = o.lstat_r.or(o.stat_r).unwrap();
    let Some(asked) = (unsafe { parts(path) }) else {
        return unsafe { real_lstat(r, path, st) };
    };
    match LINKS.entry(&asked) {
        Ok((_, Some(target))) => {
            unsafe {
                core::ptr::write_bytes(st, 0, 1);
                (*st).st_mode = libc::S_IFLNK | 0o777;
                (*st).st_nlink = 1;
                (*st).st_size = target.len() as _;
            }
            0
        }
        Ok((at, None)) => match unsafe { stat_packed(r, &at, st) } {
            Some(rc) => rc,
            None => unsafe { real_lstat(r, c(&render(&at)).as_ptr(), st) },
        },
        Err(e) => fail(r, e),
    }
}

unsafe extern "C" fn unlink_r(r: R, path: *const c_char) -> c_int {
    let real = original().unlink_r.unwrap();
    let Some(asked) = (unsafe { parts(path) }) else {
        return unsafe { real(r, path) };
    };
    match LINKS.unlink(&asked) {
        Ok(Then::Real(at)) => {
            pack_device::changing(&normalize(&at, "sdmc:/"));
            unsafe { real(r, c(&at).as_ptr()) }
        }
        Ok(_) => 0,
        Err(e) => fail(r, e),
    }
}

unsafe extern "C" fn chdir_r(r: R, path: *const c_char) -> c_int {
    unsafe { through(r, path, true, -1, |p| original().chdir_r.unwrap()(r, p)) }
}

unsafe extern "C" fn rename_r(r: R, old: *const c_char, new: *const c_char) -> c_int {
    let real = original().rename_r.unwrap();
    let (Some(from), Some(to)) = (unsafe { parts(old) }, unsafe { parts(new) }) else {
        return unsafe { real(r, old, new) };
    };
    match LINKS.rename(&from, &to) {
        Ok(Then::Rename(from, to)) => {
            pack_device::changing(&normalize(&from, "sdmc:/"));
            pack_device::changing(&normalize(&to, "sdmc:/"));
            let rc = unsafe { real(r, c(&from).as_ptr(), c(&to).as_ptr()) };
            if rc == 0 {
                LINKS.renamed(&from, &to);
            }
            rc
        }
        Ok(_) => 0,
        Err(e) => fail(r, e),
    }
}

unsafe extern "C" fn mkdir_r(r: R, path: *const c_char, mode: c_int) -> c_int {
    let real = original().mkdir_r.unwrap();
    let Some(asked) = (unsafe { parts(path) }) else {
        return unsafe { real(r, path, mode) };
    };
    match LINKS.mkdir(&asked) {
        Ok(at) => {
            pack_device::changing(&normalize(&at, "sdmc:/"));
            unsafe { real(r, c(&at).as_ptr(), mode) }
        }
        Err(e) => fail(r, e),
    }
}

unsafe extern "C" fn rmdir_r(r: R, path: *const c_char) -> c_int {
    let real = original().rmdir_r.unwrap();
    let Some(asked) = (unsafe { parts(path) }) else {
        return unsafe { real(r, path) };
    };
    match LINKS.rmdir(&asked) {
        Ok(at) => {
            pack_device::changing(&normalize(&at, "sdmc:/"));
            let rc = unsafe { real(r, c(&at).as_ptr()) };
            LINKS.forget(&at);
            rc
        }
        Err(e) => fail(r, e),
    }
}

unsafe extern "C" fn link_r(r: R, existing: *const c_char, new: *const c_char) -> c_int {
    let real = original().link_r.unwrap();
    let (Some(from), Some(to)) = (unsafe { parts(existing) }, unsafe { parts(new) }) else {
        return unsafe { real(r, existing, new) };
    };
    match (LINKS.resolve(&from, true), LINKS.mkdir(&to)) {
        (Ok(from), Ok(to)) => {
            pack_device::changing(&normalize(&to, "sdmc:/"));
            unsafe { real(r, c(&render(&from)).as_ptr(), c(&to).as_ptr()) }
        }
        (Err(e), _) | (_, Err(e)) => fail(r, e),
    }
}

unsafe extern "C" fn statvfs_r(r: R, path: *const c_char, buf: *mut c_void) -> c_int {
    unsafe {
        through(r, path, true, -1, |p| {
            original().statvfs_r.unwrap()(r, p, buf)
        })
    }
}

unsafe extern "C" fn chmod_r(r: R, path: *const c_char, mode: libc::mode_t) -> c_int {
    unsafe {
        through(r, path, true, -1, |p| {
            pack_device::changing(&normalize(&CStr::from_ptr(p).to_string_lossy(), "sdmc:/"));
            original().chmod_r.unwrap()(r, p, mode)
        })
    }
}

unsafe extern "C" fn utimes_r(r: R, path: *const c_char, times: *const c_void) -> c_int {
    unsafe {
        through(r, path, true, -1, |p| {
            pack_device::changing(&normalize(&CStr::from_ptr(p).to_string_lossy(), "sdmc:/"));
            original().utimes_r.unwrap()(r, p, times)
        })
    }
}

unsafe extern "C" fn pathconf_r(r: R, path: *const c_char, name: c_int) -> c_long {
    unsafe {
        through(r, path, true, -1, |p| {
            original().pathconf_r.unwrap()(r, p, name)
        })
    }
}

unsafe extern "C" fn symlink_r(r: R, target: *const c_char, link: *const c_char) -> c_int {
    let (Ok(target), Some(at)) = (unsafe { CStr::from_ptr(target) }.to_str(), unsafe {
        parts(link)
    }) else {
        return fail(r, super::EINVAL);
    };
    match LINKS.symlink(target, &at) {
        Ok(()) => {
            if let Ok(placed) = LINKS.resolve(&at, false) {
                pack_device::changing(&placed);
            }
            0
        }
        Err(e) => fail(r, e),
    }
}

unsafe extern "C" fn readlink_r(r: R, path: *const c_char, buf: *mut c_char, size: usize) -> isize {
    let Some(asked) = (unsafe { parts(path) }) else {
        return fail(r, super::EINVAL) as isize;
    };
    match LINKS.readlink(&asked) {
        Ok(target) => {
            // Like readlink(2): no terminating NUL, cut to the buffer.
            let n = target.len().min(size);
            unsafe { core::ptr::copy_nonoverlapping(target.as_ptr(), buf.cast(), n) };
            n as isize
        }
        Err(e) => fail(r, e) as isize,
    }
}

/// What a listing adds to libnx's: the folder's links, after its real entries. A folder the
/// pack's index lists is listed from it, without libnx (`packed`).
struct Listing {
    links: Vec<CString>,
    next: usize,
    real_done: bool,
    packed: Option<(Vec<(CString, pack::Entry)>, usize)>,
}

/// Where in a `DIR_ITER`'s state our `Listing` pointer sits: after libnx's own state.
fn listing_offset() -> usize {
    original().dir_state_size.next_multiple_of(8)
}

unsafe fn listing<'a>(dir: *mut DirIter) -> &'a mut Listing {
    unsafe {
        let slot = (*dir)
            .dir_struct
            .cast::<u8>()
            .add(listing_offset())
            .cast::<*mut Listing>();
        &mut **slot
    }
}

unsafe extern "C" fn diropen_r(r: R, dir: *mut DirIter, path: *const c_char) -> *mut DirIter {
    let started = Instant::now();
    let opened = unsafe { diropen_inner(r, dir, path) };
    LIST.add(started, 0);
    opened
}

unsafe fn diropen_inner(r: R, dir: *mut DirIter, path: *const c_char) -> *mut DirIter {
    let real = original().diropen_r.unwrap();
    let asked = unsafe { parts(path) };
    let at = match asked.as_deref().map(|a| LINKS.resolve(a, true)) {
        Some(Ok(at)) => Some(at),
        Some(Err(e)) => {
            fail(r, e);
            return core::ptr::null_mut();
        }
        None => None,
    };
    let packed = at.as_deref().and_then(pack_device::listing).map(|names| {
        let names = names
            .into_iter()
            .map(|(name, entry)| (c(&name), entry))
            .collect();
        (names, 0)
    });
    let opened = match (&at, &packed) {
        (_, Some(_)) => dir,
        (Some(at), None) => unsafe { real(r, dir, c(&render(at)).as_ptr()) },
        (None, None) => unsafe { real(r, dir, path) },
    };
    if opened.is_null() {
        return opened;
    }
    let links = at
        .map(|at| LINKS.names(&at).iter().map(|n| c(n)).collect())
        .unwrap_or_default();
    let real_done = packed.is_some();
    let listing = Box::into_raw(Box::new(Listing {
        links,
        next: 0,
        real_done,
        packed,
    }));
    unsafe {
        (*dir)
            .dir_struct
            .cast::<u8>()
            .add(listing_offset())
            .cast::<*mut Listing>()
            .write(listing);
    }
    opened
}

unsafe extern "C" fn dirreset_r(r: R, dir: *mut DirIter) -> c_int {
    let listing = unsafe { listing(dir) };
    listing.next = 0;
    if let Some((_, at)) = &mut listing.packed {
        *at = 0;
        return 0;
    }
    listing.real_done = false;
    unsafe { original().dirreset_r.unwrap()(r, dir) }
}

unsafe extern "C" fn dirnext_r(r: R, dir: *mut DirIter, name: *mut c_char, st: *mut Stat) -> c_int {
    let started = Instant::now();
    let rc = unsafe { dirnext_inner(r, dir, name, st) };
    // Time into the listing's total; one more entry when one came.
    LIST.add_time(started, u64::from(rc == 0));
    rc
}

unsafe fn dirnext_inner(r: R, dir: *mut DirIter, name: *mut c_char, st: *mut Stat) -> c_int {
    let listing = unsafe { listing(dir) };
    // A packed folder's own entries, then its links (a resource pack overlay's folders mix both).
    if let Some((names, at)) = &mut listing.packed
        && let Some((entry_name, entry)) = names.get(*at)
    {
        *at += 1;
        let bytes = entry_name.as_bytes_with_nul();
        let n = bytes.len().min(256);
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), name.cast::<u8>(), n);
            *name.add(255) = 0;
        }
        if !st.is_null() {
            unsafe { pack_device::fill(st, entry) };
        }
        return 0;
    }
    while !listing.real_done {
        let rc = unsafe { original().dirnext_r.unwrap()(r, dir, name, st) };
        if rc == 0 {
            let listed = unsafe { CStr::from_ptr(name) }.to_bytes();
            if listed == MANIFEST.as_bytes() {
                continue;
            }
            // The file system's Shift-JIS name, in UTF-8 (#7). One that won't fit readdir's
            // buffer, or isn't Shift-JIS, stays as it came: passed back, it opens as it is.
            if !listed.is_ascii()
                && let Some(utf8) = super::sjis::from_fs(listed)
                && utf8.len() < 256
            {
                let utf8 = utf8.into_owned();
                unsafe {
                    core::ptr::copy_nonoverlapping(utf8.as_ptr(), name.cast::<u8>(), utf8.len());
                    *name.add(utf8.len()) = 0;
                }
            }
            return 0;
        }
        // libnx says ENOENT at the end of the folder; anything else is an error to report.
        if errno(r) != super::ENOENT {
            return rc;
        }
        listing.real_done = true;
    }
    // readdir's buffer is NAME_MAX + 1 (256) bytes: a longer name can't be listed.
    let Some(link) = listing.links[listing.next..]
        .iter()
        .find(|link| link.as_bytes_with_nul().len() <= 256)
    else {
        listing.next = listing.links.len();
        return fail(r, super::ENOENT);
    };
    listing.next = listing
        .links
        .iter()
        .position(|l| core::ptr::eq(l, link))
        .unwrap()
        + 1;
    let bytes = link.as_bytes_with_nul();
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), name.cast::<u8>(), bytes.len()) };
    if !st.is_null() {
        unsafe {
            core::ptr::write_bytes(st, 0, 1);
            (*st).st_mode = libc::S_IFLNK | 0o777;
            (*st).st_nlink = 1;
        }
    }
    0
}

unsafe extern "C" fn dirclose_r(r: R, dir: *mut DirIter) -> c_int {
    unsafe {
        let slot = (*dir)
            .dir_struct
            .cast::<u8>()
            .add(listing_offset())
            .cast::<*mut Listing>();
        let listing = Box::from_raw(slot.read());
        if listing.packed.is_some() {
            return 0;
        }
        original().dirclose_r.unwrap()(r, dir)
    }
}

/// Puts the SD card device with links in newlib's table, in place of libnx's. Call it once at
/// boot, before any other thread runs and while no folder is open (a listing opened before has
/// no room for the links' state). Existing links come from the manifests as paths reach them.
pub fn install() -> Result<(), &'static str> {
    use crate::logging::ffi::{STD_MAX, devoptab_list};
    unsafe {
        let index = FindDevice(c"sdmc:".as_ptr());
        if index < 0 || index as usize >= STD_MAX {
            return Err("no SD card device in newlib's table");
        }
        let slot = (&raw mut devoptab_list)
            .cast::<*const DevOptab>()
            .add(index as usize);
        let sd = slot.read();
        if sd.is_null() {
            return Err("no SD card device in newlib's table");
        }
        let o = sd.read();
        if o.open_r.is_none()
            || o.stat_r.is_none()
            || o.diropen_r.is_none()
            || o.dirnext_r.is_none()
        {
            return Err("the SD card device lacks open, stat or listing");
        }
        super::io_stats::boot();
        if ORIGINAL.set(Original(o)).is_err() {
            return Err("already installed");
        }
        let mut with_links = o;
        macro_rules! wrap {
            ($($field:ident),*) => {$(
                with_links.$field = o.$field.map(|_| $field as _);
            )*};
        }
        wrap!(
            open_r,
            read_r,
            write_r,
            close_r,
            seek_r,
            fstat_r,
            ftruncate_r,
            fsync_r,
            fchmod_r,
            fpathconf_r,
            stat_r,
            unlink_r,
            chdir_r,
            rename_r,
            mkdir_r,
            diropen_r,
            dirreset_r,
            dirnext_r,
            dirclose_r,
            statvfs_r,
            chmod_r,
            rmdir_r,
            utimes_r,
            pathconf_r,
            link_r
        );
        with_links.lstat_r = Some(lstat_r);
        with_links.symlink_r = Some(symlink_r);
        with_links.readlink_r = Some(readlink_r);
        with_links.dir_state_size = listing_offset() + size_of::<*mut Listing>();
        slot.write(Box::leak(Box::new(with_links)) as *const DevOptab);
    }
    Ok(())
}
