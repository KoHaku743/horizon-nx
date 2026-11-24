//! Names as the Switch's SD card file system takes them: in Shift-JIS, not UTF-8 (#7).
//!
//! The FS service reads a path's bytes as Shift-JIS when it writes a FAT long name, and gives
//! names back in Shift-JIS: a folder the game named `Мир` (UTF-8 `D0 9C D0 B8 D1 80`) came out
//! as `ﾐ慴ｸﾑ_` on the card, made through libnx or straight through the service alike (a
//! console's probe, 24 November), and the world list skipped it. Paths go down in Shift-JIS and
//! names come up in UTF-8, so Cyrillic, Greek and Japanese names are stored as themselves. A
//! character Shift-JIS has no code for (`é`, `ї`, emoji) can't be stored at all; it goes down as
//! `_`. An ASCII path, nearly every one, passes as it is.

use std::borrow::Cow;

/// `path` as the file system takes it.
pub(super) fn to_fs(path: &str) -> Cow<'_, [u8]> {
    if path.is_ascii() {
        return Cow::Borrowed(path.as_bytes());
    }
    let mut out = Vec::with_capacity(path.len());
    let mut one = [0u8; 4];
    for ch in path.chars() {
        if ch.is_ascii() {
            out.push(ch as u8);
            continue;
        }
        let (bytes, _, unmappable) = encoding_rs::SHIFT_JIS.encode(ch.encode_utf8(&mut one));
        // `¥` and `‾` encode as 0x5C and 0x7E, which the file system would read as `\` and `~`.
        if unmappable || bytes.first().is_none_or(|&b| b < 0x80) {
            out.push(b'_');
        } else {
            out.extend_from_slice(&bytes);
        }
    }
    Cow::Owned(out)
}

/// A name the file system gave, in UTF-8; `None` when it isn't valid Shift-JIS (pass it on as
/// it came).
pub(super) fn from_fs(name: &[u8]) -> Option<Cow<'_, str>> {
    if name.is_ascii() {
        return std::str::from_utf8(name).ok().map(Cow::Borrowed);
    }
    encoding_rs::SHIFT_JIS.decode_without_bom_handling_and_without_replacement(name)
}

#[cfg(test)]
mod tests {
    use super::{from_fs, to_fs};

    #[test]
    fn names_go_down_in_shift_jis_and_come_back_as_they_were() {
        // ASCII is untouched.
        assert_eq!(&*to_fs("sdmc:/switch/horizon/saves/New World"), b"sdmc:/switch/horizon/saves/New World");
        // Cyrillic: JIS X 0208's row 7, two bytes a letter.
        assert_eq!(&*to_fs("saves/Мир"), b"saves/\x84\x4d\x84\x79\x84\x82");
        for name in ["saves/Мир", "Новый мир (1)", "日本語のワールド", "ΑΒΓ αβγ", "ｶﾀｶﾅ"] {
            assert_eq!(from_fs(&to_fs(name)).as_deref(), Some(name), "{name}");
        }
        // What Shift-JIS can't hold goes down as `_`, one per character.
        assert_eq!(&*to_fs("Café ї 😀 ¥"), b"Caf_ _ _ _");
        // A folder the earlier builds made from UTF-8 bytes lists, and opens, as that same name.
        let old = b"\xd0\x9c\xd0\xb8\xd1_";
        let listed = from_fs(old).unwrap();
        assert_eq!(listed, "ﾐ慴ｸﾑ_");
        assert_eq!(&*to_fs(&listed), old);
        // Bytes that aren't Shift-JIS stay with the caller.
        assert_eq!(from_fs(b"\x80\xff"), None);
    }
}
