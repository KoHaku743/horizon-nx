//! Symbolic links on the SD card, whose FAT file system has none.
//!
//! The game builds its resource-pack overlay (horizon-client-render `packs.rs`) and its mod
//! content root (horizon-client `mod_content.rs`) as folders of links into the vanilla resources
//! and the packs: a few hundred links stand in for ~650 MB of files. On the Switch every
//! `symlink` failed (newlib's SD card device has no `symlink_r`), so server resource packs and
//! `.hmod` mods never loaded.
//!
//! This module gives the SD card links. A folder's links are listed in a small file in that
//! folder, [`MANIFEST`], one line per change (`+name<TAB>target` adds, `-name` removes; a torn
//! last line is ignored), so they travel with the folder and need no state anywhere else. The
//! SD card's newlib device (`devoptab.rs`) is replaced by a copy whose path operations look the
//! links up: opening, `stat` and listing go through to the target, `lstat` and `readlink` report
//! the link, `unlink`, `rename` and `rmdir` keep the manifests right, and listings show the links
//! and hide the manifest. Everything above newlib (std, the game) sees ordinary Unix links, so the
//! overlay builders run their Unix code unchanged.
//!
//! The logic here is plain Rust over a [`Store`] (the manifests' bytes), tested on any host; only
//! `devoptab.rs` is Switch code.

#[cfg(target_os = "horizon")]
mod devoptab;
#[cfg(target_os = "horizon")]
pub use devoptab::install;
#[cfg(target_os = "horizon")]
mod io_stats;
pub(crate) mod pack;
#[cfg(target_os = "horizon")]
mod pack_device;
mod sjis;
#[cfg(target_os = "horizon")]
pub use pack_device::{build_in_background as build_pack_in_background, use_cache_pack, use_pack};
#[cfg(target_os = "horizon")]
pub use io_stats::report as io_report;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard, RwLock};

/// The file in a folder that lists that folder's links. Hidden from listings.
pub const MANIFEST: &str = ".hzlinks";

/// newlib's errno values (`sys/errno.h`).
pub(crate) mod errno {
    pub const ENOENT: i32 = 2;
    pub const EIO: i32 = 5;
    pub const EEXIST: i32 = 17;
    pub const ENOTDIR: i32 = 20;
    pub const EINVAL: i32 = 22;
    pub const ENOTEMPTY: i32 = 90;
    pub const ELOOP: i32 = 92;
}
use errno::*;

/// The most links one path may pass through, like Linux's 40.
const MAX_HOPS: usize = 40;

/// An absolute path on the SD card as its parts, without the device: `sdmc:/a/b` is `["a", "b"]`.
pub(crate) type Parts = Vec<String>;

/// `path` as absolute parts. A path without a device is relative to `cwd` (newlib's working
/// directory, `sdmc:/...`), or to the device root when it starts with `/`. A part naming a device
/// starts over from that root: `std::path::absolute` joins the working directory onto paths like
/// `sdmc:/x`, which Rust doesn't count as absolute, giving `sdmc:/switch/horizon/sdmc:/x`.
pub(crate) fn normalize(path: &str, cwd: &str) -> Parts {
    let joined;
    let full = if path.contains(':') || path.starts_with('/') {
        path
    } else {
        joined = format!("{cwd}/{path}");
        &joined
    };
    let mut parts = Parts::new();
    for part in full.split('/') {
        let part = match part.rfind(':') {
            Some(i) => {
                parts.clear();
                &part[i + 1..]
            }
            None => part,
        };
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part.to_owned()),
        }
    }
    parts
}

/// `parts` as an SD card path: `sdmc:/a/b`, or `sdmc:/` for the root.
pub(crate) fn render(parts: &[String]) -> String {
    let mut path = String::from("sdmc:/");
    path.push_str(&parts.join("/"));
    path
}

/// `name` in the folder `dir` (an output of [`render`]).
pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// The links a manifest's text leaves in place.
fn parse(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut links = BTreeMap::new();
    let text = String::from_utf8_lossy(bytes);
    // Only whole lines: a crash mid-append leaves a torn last line, which never happened.
    let complete = text.rfind('\n').map_or("", |end| &text[..end]);
    for line in complete.lines() {
        if let Some(added) = line.strip_prefix('+') {
            if let Some((name, target)) = added.split_once('\t')
                && !name.is_empty()
                && !target.is_empty()
            {
                links.insert(name.to_owned(), target.to_owned());
            }
        } else if let Some(name) = line.strip_prefix('-') {
            links.remove(name);
        }
    }
    links
}

/// Where manifests live: the SD card itself, or memory in tests.
pub(crate) trait Store {
    /// The bytes of `dir`'s manifest, or `None` when it has none.
    fn read(&mut self, dir: &str) -> Option<Vec<u8>>;
    /// Adds `text` to the end of `dir`'s manifest, creating it.
    fn append(&mut self, dir: &str, text: &str) -> Result<(), i32>;
    /// Deletes `dir`'s manifest; a missing one is fine.
    fn remove(&mut self, dir: &str) -> Result<(), i32>;
    /// What is really at `path`: `Some(true)` a folder, `Some(false)` a file, `None` nothing.
    fn kind(&mut self, path: &str) -> Option<bool>;
    /// Lets go of any manifest kept open, before a folder holding it is renamed or removed.
    fn release(&mut self) {}
}

/// A folder's links by name; `None` when it has no manifest.
type Folder = Option<BTreeMap<String, String>>;

/// What [`Links::unlink`] and [`Links::rename`] leave for the real file system to do.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Then {
    /// The links were the whole job.
    Done,
    /// Do it to this path (`rename`: from the first to the second).
    Real(String),
    Rename(String, String),
}

/// The links of every folder seen so far, over a [`Store`].
///
/// Folders are read from the store the first time a path goes through them and kept: nothing but
/// this process writes manifests while it runs. Every change holds `store`'s lock for its whole
/// read-check-write, so changes happen one at a time and in the manifests' order; lookups only
/// take it to read a folder not seen yet.
pub(crate) struct Links<S> {
    folders: RwLock<BTreeMap<String, Folder>>,
    store: Mutex<S>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl<S: Store> Links<S> {
    pub(crate) const fn new(store: S) -> Self {
        Links {
            folders: RwLock::new(BTreeMap::new()),
            store: Mutex::new(store),
        }
    }

    /// The target of the link `name` in `dir`. `held` is the store when the caller holds it.
    fn link_in(&self, dir: &str, name: &str, held: Option<&mut S>) -> Option<String> {
        {
            let folders = self.folders.read().unwrap_or_else(|e| e.into_inner());
            if let Some(folder) = folders.get(dir) {
                return folder.as_ref().and_then(|links| links.get(name).cloned());
            }
        }
        let bytes = match held {
            Some(store) => store.read(dir),
            None => lock(&self.store).read(dir),
        };
        let mut folders = self.folders.write().unwrap_or_else(|e| e.into_inner());
        // A change made meanwhile read the folder itself, under the store's lock: keep its view.
        let folder = folders
            .entry(dir.to_owned())
            .or_insert_with(|| bytes.as_deref().map(parse));
        folder.as_ref().and_then(|links| links.get(name).cloned())
    }

    /// `dir`'s links, read if not seen yet.
    fn folder(&self, dir: &str, store: &mut S) -> BTreeMap<String, String> {
        self.link_in(dir, "", Some(store));
        let folders = self.folders.read().unwrap_or_else(|e| e.into_inner());
        folders.get(dir).cloned().flatten().unwrap_or_default()
    }

    /// `path` with every link on it replaced by its target; the last part only with `follow`
    /// (so `follow: false` gives the entry itself, as `lstat`, `unlink` and `rename` act on).
    pub(crate) fn resolve(&self, path: &[String], follow: bool) -> Result<Parts, i32> {
        self.resolve_in(path, follow, None)
    }

    fn resolve_in(
        &self,
        path: &[String],
        follow: bool,
        mut held: Option<&mut S>,
    ) -> Result<Parts, i32> {
        let mut pending: VecDeque<String> = path.iter().cloned().collect();
        let mut at = Parts::new();
        let mut hops = 0;
        while let Some(part) = pending.pop_front() {
            if part == ".." {
                at.pop();
                continue;
            }
            let last = pending.is_empty();
            if (follow || !last)
                && let Some(target) = self.link_in(&render(&at), &part, held.as_deref_mut())
            {
                hops += 1;
                if hops > MAX_HOPS {
                    return Err(ELOOP);
                }
                for part in normalize(&target, "sdmc:/").into_iter().rev() {
                    pending.push_front(part);
                }
                at.clear();
                continue;
            }
            at.push(part);
        }
        Ok(at)
    }

    /// The entry at `path` (not followed) and, when it is a link, its target.
    pub(crate) fn entry(&self, path: &[String]) -> Result<(Parts, Option<String>), i32> {
        let at = self.resolve(path, false)?;
        let target = match at.split_last() {
            Some((name, dir)) => self.link_in(&render(dir), name, None),
            None => None,
        };
        Ok((at, target))
    }

    /// The target of the link at `path`; `EINVAL` when it isn't one.
    pub(crate) fn readlink(&self, path: &[String]) -> Result<String, i32> {
        self.entry(path)?.1.ok_or(EINVAL)
    }

    /// The names of the links in the folder at `dir` (already resolved).
    pub(crate) fn names(&self, dir: &[String]) -> Vec<String> {
        let mut store = lock(&self.store);
        self.folder(&render(dir), &mut store).into_keys().collect()
    }

    /// Makes `link` a link to `target`, which, when relative, is relative to the link's folder.
    pub(crate) fn symlink(&self, target: &str, link: &[String]) -> Result<(), i32> {
        let mut store = lock(&self.store);
        let at = self.resolve_in(link, false, Some(&mut store))?;
        let Some((name, dir)) = at.split_last() else {
            return Err(EEXIST);
        };
        if name == MANIFEST
            || target.is_empty()
            || name.contains(['\t', '\n'])
            || target.contains(['\t', '\n'])
        {
            return Err(EINVAL);
        }
        let dir = render(dir);
        match store.kind(&dir) {
            Some(true) => {}
            Some(false) => return Err(ENOTDIR),
            None => return Err(ENOENT),
        }
        if self.link_in(&dir, name, Some(&mut store)).is_some()
            || store.kind(&render(&at)).is_some()
        {
            return Err(EEXIST);
        }
        let target = render(&normalize(target, &dir));
        store.append(&dir, &format!("+{name}\t{target}\n"))?;
        let mut folders = self.folders.write().unwrap_or_else(|e| e.into_inner());
        folders
            .entry(dir)
            .or_insert(None)
            .get_or_insert_with(BTreeMap::new)
            .insert(name.clone(), target);
        Ok(())
    }

    /// Takes the link `name` out of `dir`, deleting the manifest with the last one.
    fn drop_link(&self, store: &mut S, dir: &str, name: &str) -> Result<(), i32> {
        let mut links = self.folder(dir, store);
        links.remove(name);
        if links.is_empty() {
            store.remove(dir)?;
        } else {
            store.append(dir, &format!("-{name}\n"))?;
        }
        let mut folders = self.folders.write().unwrap_or_else(|e| e.into_inner());
        folders.insert(dir.to_owned(), (!links.is_empty()).then_some(links));
        Ok(())
    }

    /// `unlink`: removes the link at `path`, or names the real file to remove.
    pub(crate) fn unlink(&self, path: &[String]) -> Result<Then, i32> {
        let mut store = lock(&self.store);
        let at = self.resolve_in(path, false, Some(&mut store))?;
        let Some((name, dir)) = at.split_last() else {
            return Ok(Then::Real(render(&at)));
        };
        let dir = render(dir);
        if self.link_in(&dir, name, Some(&mut store)).is_none() {
            return Ok(Then::Real(render(&at)));
        }
        self.drop_link(&mut store, &dir, name)?;
        Ok(Then::Done)
    }

    /// `mkdir`: the real path to create; `EEXIST` when a link is there.
    pub(crate) fn mkdir(&self, path: &[String]) -> Result<String, i32> {
        match self.entry(path)? {
            (_, Some(_)) => Err(EEXIST),
            (at, None) => Ok(render(&at)),
        }
    }

    /// `rmdir`: the real folder to remove. A folder still holding links isn't empty; an empty
    /// manifest goes first, so the real folder can be.
    pub(crate) fn rmdir(&self, path: &[String]) -> Result<String, i32> {
        let mut store = lock(&self.store);
        let at = self.resolve_in(path, false, Some(&mut store))?;
        if let Some((name, dir)) = at.split_last()
            && self.link_in(&render(dir), name, Some(&mut store)).is_some()
        {
            return Err(ENOTDIR);
        }
        let dir = render(&at);
        if !self.folder(&dir, &mut store).is_empty() {
            return Err(ENOTEMPTY);
        }
        store.release();
        store.remove(&dir)?;
        self.forget(&dir);
        Ok(dir)
    }

    /// `rename`: moves a link between folders' manifests, or names the real rename to make
    /// (after which call [`Links::renamed`]). A link in the way of either is replaced, as a Unix
    /// rename replaces its destination.
    pub(crate) fn rename(&self, old: &[String], new: &[String]) -> Result<Then, i32> {
        let mut store = lock(&self.store);
        let from = self.resolve_in(old, false, Some(&mut store))?;
        let to = self.resolve_in(new, false, Some(&mut store))?;
        let (Some((old_name, old_dir)), Some((new_name, new_dir))) =
            (from.split_last(), to.split_last())
        else {
            return Err(EINVAL);
        };
        if from == to {
            return Ok(Then::Done);
        }
        let (old_dir, new_dir) = (render(old_dir), render(new_dir));
        let moved = self.link_in(&old_dir, old_name, Some(&mut store));
        let replaced = self.link_in(&new_dir, new_name, Some(&mut store));
        let Some(target) = moved else {
            if replaced.is_some() {
                self.drop_link(&mut store, &new_dir, new_name)?;
            }
            // A folder being renamed may hold the manifest the store keeps open.
            store.release();
            return Ok(Then::Rename(render(&from), render(&to)));
        };
        match store.kind(&new_dir) {
            Some(true) => {}
            Some(false) => return Err(ENOTDIR),
            None => return Err(ENOENT),
        }
        if replaced.is_some() {
            self.drop_link(&mut store, &new_dir, new_name)?;
        } else if store.kind(&render(&to)).is_some() {
            return Err(EEXIST);
        }
        store.append(&new_dir, &format!("+{new_name}\t{target}\n"))?;
        {
            let mut folders = self.folders.write().unwrap_or_else(|e| e.into_inner());
            folders
                .entry(new_dir)
                .or_insert(None)
                .get_or_insert_with(BTreeMap::new)
                .insert(new_name.clone(), target);
        }
        self.drop_link(&mut store, &old_dir, old_name)?;
        Ok(Then::Done)
    }

    /// After a real rename: the manifests under both paths moved, so read them again.
    pub(crate) fn renamed(&self, old: &str, new: &str) {
        self.forget(old);
        self.forget(new);
    }

    /// Drops what is known of `dir` and the folders under it.
    pub(crate) fn forget(&self, dir: &str) {
        let below = join(dir, "");
        let mut folders = self.folders.write().unwrap_or_else(|e| e.into_inner());
        folders.retain(|path, _| path != dir && !path.starts_with(&below));
    }
}

#[cfg(test)]
mod tests;
