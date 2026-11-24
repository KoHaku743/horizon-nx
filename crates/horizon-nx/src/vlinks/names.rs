//! Non-ASCII names on the Switch's SD card, kept in ASCII (#7).
//!
//! The FS service can't hold them: it reads a written name's bytes as Shift-JIS (`Мир` saved as
//! `ﾐ慴ｸﾑ_`), and its listings leave out every entry whose name isn't ASCII, `Мир` written
//! by a computer included (a console's probes, 24 and 25 November), so such a world never showed
//! in the list or in any other app. Each path part with a non-ASCII character goes down as
//! `xn--` and its Punycode (RFC 3492, as an internationalised domain name), and a listed name
//! in that form comes back as the name it stands for: the game sees `Мир`, the card holds
//! `xn--o0a0dya`. A name a computer puts on the card outside ASCII stays hidden: the FS service
//! lists it to nobody.

use std::borrow::Cow;

const PREFIX: &str = "xn--";

/// `path` as it goes to the SD card: each part with a non-ASCII character in its ASCII form.
pub(super) fn to_fs(path: &str) -> Cow<'_, str> {
    if path.is_ascii() {
        return Cow::Borrowed(path);
    }
    let parts: Vec<Cow<'_, str>> = path
        .split('/')
        .map(|part| match encode(part) {
            Some(ascii) if !part.is_ascii() => Cow::Owned(format!("{PREFIX}{ascii}")),
            _ => Cow::Borrowed(part),
        })
        .collect();
    Cow::Owned(parts.join("/"))
}

/// The name a listed `name` stands for, when it's one [`to_fs`] made.
pub(super) fn from_fs(name: &str) -> Option<String> {
    let rest = name.get(..PREFIX.len())?.eq_ignore_ascii_case(PREFIX).then(|| &name[PREFIX.len()..])?;
    let decoded = decode(rest)?;
    (!decoded.is_ascii()).then_some(decoded)
}

const BASE: u32 = 36;
const T_MIN: u32 = 1;
const T_MAX: u32 = 26;
const SKEW: u32 = 38;
const DAMP: u32 = 700;
const INITIAL_BIAS: u32 = 72;
const INITIAL_N: u32 = 128;

fn adapt(delta: u32, points: u32, first: bool) -> u32 {
    let mut delta = if first { delta / DAMP } else { delta / 2 };
    delta += delta / points;
    let mut k = 0;
    while delta > ((BASE - T_MIN) * T_MAX) / 2 {
        delta /= BASE - T_MIN;
        k += BASE;
    }
    k + (BASE - T_MIN + 1) * delta / (delta + SKEW)
}

fn threshold(k: u32, bias: u32) -> u32 {
    if k <= bias {
        T_MIN
    } else if k >= bias + T_MAX {
        T_MAX
    } else {
        k - bias
    }
}

fn digit(d: u32) -> char {
    (if d < 26 { b'a' + d as u8 } else { b'0' + (d - 26) as u8 }) as char
}

/// RFC 3492's encoding of `input`.
fn encode(input: &str) -> Option<String> {
    let points: Vec<u32> = input.chars().map(u32::from).collect();
    let mut out: String = input.chars().filter(char::is_ascii).collect();
    let basic = out.len() as u32;
    let mut handled = basic;
    if basic > 0 {
        out.push('-');
    }
    let (mut n, mut delta, mut bias) = (INITIAL_N, 0u32, INITIAL_BIAS);
    let total = points.len() as u32;
    while handled < total {
        let m = *points.iter().filter(|&&c| c >= n).min()?;
        delta = delta.checked_add((m - n).checked_mul(handled + 1)?)?;
        n = m;
        for &c in &points {
            if c < n {
                delta = delta.checked_add(1)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = threshold(k, bias);
                    if q < t {
                        break;
                    }
                    out.push(digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit(q));
                bias = adapt(delta, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta = delta.checked_add(1)?;
        n += 1;
    }
    Some(out)
}

/// RFC 3492's decoding; `None` for anything that isn't a valid encoding.
fn decode(input: &str) -> Option<String> {
    let (basic, rest) = match input.rfind('-') {
        Some(i) => (&input[..i], &input[i + 1..]),
        None => ("", input),
    };
    if !basic.is_ascii() {
        return None;
    }
    let mut out: Vec<char> = basic.chars().collect();
    let (mut n, mut i, mut bias) = (INITIAL_N, 0u32, INITIAL_BIAS);
    let mut bytes = rest.bytes().peekable();
    while bytes.peek().is_some() {
        let old = i;
        let mut w = 1u32;
        let mut k = BASE;
        loop {
            let d = match bytes.next()? {
                b @ b'a'..=b'z' => b - b'a',
                b @ b'A'..=b'Z' => b - b'A',
                b @ b'0'..=b'9' => b - b'0' + 26,
                _ => return None,
            } as u32;
            i = i.checked_add(d.checked_mul(w)?)?;
            let t = threshold(k, bias);
            if d < t {
                break;
            }
            w = w.checked_mul(BASE - t)?;
            k += BASE;
        }
        let len = out.len() as u32 + 1;
        bias = adapt(i - old, len, old == 0);
        n = n.checked_add(i / len)?;
        i %= len;
        out.insert(i as usize, char::from_u32(n)?);
        i += 1;
    }
    Some(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::{from_fs, to_fs};

    /// Expected forms from Python's `str.encode("punycode")`, an independent RFC 3492 encoder.
    #[test]
    fn non_ascii_names_go_down_in_ascii_and_come_back_as_they_were() {
        let cases = [
            ("Мир", "xn--o0a0dya"),
            ("Новый мир (1)", "xn--  (1)-use1mzbh5as3a2l"),
            ("Base Мир", "xn--Base -tse0rmc"),
            ("日本語のワールド", "xn--u9j3jrdpa9d7726bo4bc84j"),
            ("Café", "xn--Caf-dma"),
            ("😀 emoji", "xn-- emoji-2v74e"),
        ];
        for (name, on_card) in cases {
            assert_eq!(to_fs(name), on_card, "{name}");
            assert_eq!(from_fs(on_card).as_deref(), Some(name), "{name}");
        }
        // Only the parts that need it, and ASCII paths untouched.
        assert_eq!(
            to_fs("sdmc:/switch/horizon/saves/Мир/level.dat"),
            "sdmc:/switch/horizon/saves/xn--o0a0dya/level.dat"
        );
        assert_eq!(to_fs("sdmc:/switch/horizon/saves/New World"), "sdmc:/switch/horizon/saves/New World");
        // Plain names, and names that only look like the form, list as they are.
        assert_eq!(from_fs("New World"), None);
        assert_eq!(from_fs("xn--abc-"), None);
        assert_eq!(from_fs("xn--!!"), None);
    }
}
