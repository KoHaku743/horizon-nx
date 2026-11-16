//! `horizon-prepare switch-export`: a copy of the prepared folder for a Nintendo Switch's SD card
//! (`/switch/horizon/resources`), made on the computer that prepared it.
//!
//! A plain copy of the folder works, slowly: its `horizon-data.bin` names the folder it was
//! written in, so the Switch ignores it and reads some 22,000 files one by one from the SD card
//! at its first start. This writes a portable `horizon-data.bin` instead
//! (`horizon_core::prepared_files::write_portable`), which the Switch uses as it is.
//!
//! `--slim` also leaves out the files the data file holds, a few thousand files to copy instead
//! of tens of thousands. The game reads those through the data file; a reader that opens one
//! from the folder itself would miss it, so the full copy stays the default until a Switch has
//! played from a slim one.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use horizon_core::prepared_files;

/// What a copy wrote.
#[derive(Debug, PartialEq, Eq)]
pub struct Exported {
    pub files: usize,
    pub bytes: u64,
}

/// Files of a prepared folder the game never reads: the preparer's own.
fn preparer_file(name: &str) -> bool {
    name == ".horizon-prepare.lock"
        || name.ends_with(".log")
        || name == prepared_files::FILE_NAME
        || (name.starts_with(prepared_files::FILE_NAME) && name.ends_with(".tmp"))
}

/// Copy `resources` (a prepared folder) into `dest` with a portable data file.
pub fn export(resources: &Path, dest: &Path, slim: bool) -> Result<Exported, String> {
    if !resources.join("horizon-resources.json").is_file() {
        return Err(format!(
            "{} is not a prepared folder: it has no horizon-resources.json",
            resources.display()
        ));
    }
    let mut files = Vec::new();
    walk(resources, "", slim, &mut files).map_err(|e| format!("{}: {e}", resources.display()))?;
    let mut exported = Exported { files: 0, bytes: 0 };
    for (name, path) in &files {
        let to = dest.join(name);
        let copied = (|| {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            // Bytes only: `fs::copy` carries macOS's extended attributes along, which a FAT
            // volume stores as a `._name` file beside each copy.
            io::copy(&mut fs::File::open(path)?, &mut fs::File::create(&to)?)
        })()
        .map_err(|e| format!("{} to {}: {e}", path.display(), to.display()))?;
        exported.files += 1;
        exported.bytes += copied;
    }
    let data = dest.join(prepared_files::FILE_NAME);
    prepared_files::write_portable(resources, &data)
        .map_err(|e| format!("{}: {e}", data.display()))?;
    exported.files += 1;
    exported.bytes += fs::metadata(&data).map_or(0, |m| m.len());
    Ok(exported)
}

/// The files under `dir` (named `name` from the root, `/`-separated) a copy takes.
fn walk(dir: &Path, name: &str, slim: bool, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if prepared_files::is_mac_metadata(&entry.file_name()) {
            continue;
        }
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            return Err(io::Error::other(format!(
                "{} is not UTF-8",
                entry.path().display()
            )));
        };
        let child = match name {
            "" => file_name.clone(),
            _ => format!("{name}/{file_name}"),
        };
        let path = entry.path();
        if fs::metadata(&path)?.is_dir() {
            walk(&path, &child, slim, out)?;
        } else if !preparer_file(&file_name) && !(slim && prepared_files::holds_file(&child)) {
            out.push((child, path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("hz-switch-export-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (name, bytes) in [
            ("horizon-resources.json", &b"{\"schema\":7}"[..]),
            ("facts.json", b"{}"),
            ("export.log", b"done"),
            (".horizon-prepare.lock", b""),
            ("horizon-data.bin", b"written for this computer's path"),
            ("data/minecraft/recipe/stick.json", b"{\"stick\":1}"),
            ("data/minecraft/recipe/._stick.json", b"\0\x05\x16\x07"),
            ("assets/minecraft/sounds/ambient/cave1.ogg", b"OggS"),
        ] {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        root
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        walk(dir, "", false, &mut out).unwrap();
        // `walk` leaves the data file out of a copy's sources; a copy has one.
        if dir.join(prepared_files::FILE_NAME).is_file() {
            out.push((prepared_files::FILE_NAME.into(), PathBuf::new()));
        }
        let mut names: Vec<_> = out.into_iter().map(|(n, _)| n).collect();
        names.sort();
        names
    }

    /// The copy has the game's files and a data file the copy's own path can open; the
    /// preparer's files and macOS's stay behind.
    #[test]
    fn the_copy_reads_through_its_own_data_file() {
        let root = prepared("full");
        let dest = root.with_extension("dest");
        let _ = fs::remove_dir_all(&dest);
        let exported = export(&root, &dest, false).unwrap();
        assert_eq!(
            names(&dest),
            [
                "assets/minecraft/sounds/ambient/cave1.ogg",
                "data/minecraft/recipe/stick.json",
                "facts.json",
                "horizon-data.bin",
                "horizon-resources.json",
            ]
        );
        assert_eq!(exported.files, 5);
        // Only the data file has the recipe once the loose one changes.
        fs::write(dest.join("data/minecraft/recipe/stick.json"), b"{}").unwrap();
        assert_eq!(
            prepared_files::read(&dest.join("data/minecraft/recipe/stick.json")).unwrap(),
            b"{\"stick\":1}"
        );
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&dest);
    }

    /// A slim copy leaves what the data file holds to the data file.
    #[test]
    fn a_slim_copy_keeps_only_what_the_data_file_lacks() {
        let root = prepared("slim");
        let dest = root.with_extension("dest");
        let _ = fs::remove_dir_all(&dest);
        export(&root, &dest, true).unwrap();
        assert_eq!(
            names(&dest),
            [
                "assets/minecraft/sounds/ambient/cave1.ogg",
                "facts.json",
                "horizon-data.bin",
                "horizon-resources.json",
            ]
        );
        assert_eq!(
            prepared_files::read(&dest.join("data/minecraft/recipe/stick.json")).unwrap(),
            b"{\"stick\":1}"
        );
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn a_folder_that_was_not_prepared_is_refused() {
        let dir =
            std::env::temp_dir().join(format!("hz-switch-export-none-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(export(&dir, &dir.join("out"), false)
            .unwrap_err()
            .contains("horizon-resources.json"));
        let _ = fs::remove_dir_all(&dir);
    }
}
