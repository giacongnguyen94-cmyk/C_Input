//! Chuyển đổi bảng mã tiếng Việt.
//!
//! Văn bản bảng mã cũ (TCVN3, VNI) được biểu diễn như Windows vẫn làm khi chép ra clipboard:
//! mỗi byte là một ký tự theo bảng mã Windows-1252. Gõ bằng các bảng mã này nghĩa là gửi
//! đúng các ký tự đó tới ứng dụng (đang dùng font .VnTime, VNI-Times...).

use std::collections::HashMap;
use std::sync::OnceLock;

use cinput_core::types::{Letter, Mark, Tone, compose, decompose};
use serde::{Deserialize, Serialize};

mod tables;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash, Serialize, Deserialize)]
pub enum Charset {
    #[default]
    Unicode,
    UnicodeComposite,
    Tcvn3,
    VniWindows,
    Viqr,
    NcrDecimal,
    NcrHex,
}

impl Charset {
    pub const ALL: [Charset; 7] = [
        Charset::Unicode,
        Charset::UnicodeComposite,
        Charset::Tcvn3,
        Charset::VniWindows,
        Charset::Viqr,
        Charset::NcrDecimal,
        Charset::NcrHex,
    ];

    /// Các bảng mã dùng được khi gõ.
    pub const TYPING: [Charset; 5] =
        [Charset::Unicode, Charset::UnicodeComposite, Charset::Tcvn3, Charset::VniWindows, Charset::Viqr];

    pub fn label(self) -> &'static str {
        match self {
            Charset::Unicode => "Unicode dựng sẵn",
            Charset::UnicodeComposite => "Unicode tổ hợp",
            Charset::Tcvn3 => "TCVN3 (ABC)",
            Charset::VniWindows => "VNI Windows",
            Charset::Viqr => "VIQR",
            Charset::NcrDecimal => "NCR thập phân",
            Charset::NcrHex => "NCR thập lục phân",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Charset::Unicode => "Unicode",
            Charset::UnicodeComposite => "Tổ hợp",
            Charset::Tcvn3 => "TCVN3",
            Charset::VniWindows => "VNI",
            Charset::Viqr => "VIQR",
            Charset::NcrDecimal => "NCR",
            Charset::NcrHex => "NCR hex",
        }
    }
}

// ---------------------------------------------------------------- Windows-1252

const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

fn byte_to_char(b: u8) -> char {
    if (0x80..0xA0).contains(&b) {
        char::from_u32(CP1252_HIGH[(b - 0x80) as usize] as u32).unwrap_or('?')
    } else {
        b as char
    }
}

fn char_to_byte(c: char) -> Option<u8> {
    let u = c as u32;
    if u < 0x80 || (0xA0..=0xFF).contains(&u) {
        return Some(u as u8);
    }
    CP1252_HIGH.iter().position(|&x| x as u32 == u).map(|i| 0x80 + i as u8)
}

// ---------------------------------------------------------------- bảng tra

/// Chỉ 186 dòng đầu là chữ cái tiếng Việt; phần còn lại là ký hiệu Windows (đã xử lý qua CP1252).
const LETTER_ROWS: usize = 186;

struct Maps {
    tcvn_enc: HashMap<char, u8>,
    tcvn_dec: HashMap<u8, char>,
    vni_enc: HashMap<char, u16>,
    vni_dec: HashMap<u16, char>,
}

fn maps() -> &'static Maps {
    static M: OnceLock<Maps> = OnceLock::new();
    M.get_or_init(|| {
        let rows = &tables::VN_TABLE[..LETTER_ROWS];
        let mut m = Maps {
            tcvn_enc: HashMap::new(),
            tcvn_dec: HashMap::new(),
            vni_enc: HashMap::new(),
            vni_dec: HashMap::new(),
        };
        for &(u, t, v) in rows {
            let c = char::from_u32(u).expect("mã Unicode hợp lệ");
            if t != 0 {
                m.tcvn_enc.insert(c, t);
            }
            m.vni_enc.insert(c, v);
            m.vni_dec.entry(v).or_insert(c);
        }
        // TCVN3 không có chữ hoa có dấu thanh: chữ hoa dùng chung mã với chữ thường.
        // Khi giải mã ưu tiên chữ thường (bảng xen kẽ: chẵn = hoa, lẻ = thường).
        for (i, &(u, t, _)) in rows.iter().enumerate() {
            if i % 2 == 1 && t != 0 {
                m.tcvn_dec.entry(t).or_insert(char::from_u32(u).unwrap());
            }
        }
        for (i, &(u, t, _)) in rows.iter().enumerate() {
            if i % 2 == 0 && t != 0 {
                m.tcvn_dec.entry(t).or_insert(char::from_u32(u).unwrap());
            }
        }
        m
    })
}

// ---------------------------------------------------------------- mã hoá

/// Mã hoá một chuỗi Unicode sang bảng mã đích.
pub fn encode(text: &str, cs: Charset) -> String {
    let m = maps();
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match cs {
            Charset::Unicode => out.push(c),
            Charset::UnicodeComposite => encode_composite(c, &mut out),
            Charset::Tcvn3 => match m.tcvn_enc.get(&c) {
                Some(&b) => out.push(byte_to_char(b)),
                None => out.push(c),
            },
            Charset::VniWindows => match m.vni_enc.get(&c) {
                Some(&v) => {
                    out.push(byte_to_char((v & 0xFF) as u8));
                    if v >> 8 != 0 {
                        out.push(byte_to_char((v >> 8) as u8));
                    }
                }
                None => out.push(c),
            },
            Charset::Viqr => encode_viqr(c, &mut out),
            Charset::NcrDecimal => {
                if c.is_ascii() {
                    out.push(c)
                } else {
                    out.push_str(&format!("&#{};", c as u32))
                }
            }
            Charset::NcrHex => {
                if c.is_ascii() {
                    out.push(c)
                } else {
                    out.push_str(&format!("&#x{:X};", c as u32))
                }
            }
        }
    }
    out
}

const COMBINING_TONES: [(Tone, char); 5] = [
    (Tone::Acute, '\u{301}'),
    (Tone::Grave, '\u{300}'),
    (Tone::Hook, '\u{309}'),
    (Tone::Tilde, '\u{303}'),
    (Tone::Dot, '\u{323}'),
];

fn encode_composite(c: char, out: &mut String) {
    match decompose(c) {
        Some((l, t)) if t != Tone::None => {
            out.push(compose(l, Tone::None));
            out.push(COMBINING_TONES.iter().find(|(x, _)| *x == t).unwrap().1);
        }
        _ => out.push(c),
    }
}

const VIQR_TONES: [(Tone, char); 5] =
    [(Tone::Acute, '\''), (Tone::Grave, '`'), (Tone::Hook, '?'), (Tone::Tilde, '~'), (Tone::Dot, '.')];

fn encode_viqr(c: char, out: &mut String) {
    let Some((l, t)) = decompose(c) else {
        out.push(c);
        return;
    };
    if l.mark == Mark::None && t == Tone::None {
        out.push(c);
        return;
    }
    let base = if l.upper { l.base.to_ascii_uppercase() } else { l.base };
    out.push(base);
    match l.mark {
        Mark::Hat => out.push('^'),
        Mark::Breve => out.push('('),
        Mark::Horn => out.push('+'),
        Mark::Stroke => out.push(base),
        Mark::None => {}
    }
    if let Some((_, k)) = VIQR_TONES.iter().find(|(x, _)| *x == t) {
        out.push(*k);
    }
}

// ---------------------------------------------------------------- giải mã

/// Giải mã từ bảng mã nguồn về Unicode dựng sẵn.
pub fn decode(text: &str, cs: Charset) -> String {
    match cs {
        Charset::Unicode | Charset::UnicodeComposite => decode_unicode(text),
        Charset::Tcvn3 => {
            let m = maps();
            text.chars()
                .map(|c| char_to_byte(c).and_then(|b| m.tcvn_dec.get(&b).copied()).unwrap_or(c))
                .collect()
        }
        Charset::VniWindows => decode_vni(text),
        Charset::Viqr => decode_viqr(text),
        Charset::NcrDecimal | Charset::NcrHex => decode_ncr(text),
    }
}

/// Chuyển văn bản giữa hai bảng mã.
pub fn convert(text: &str, from: Charset, to: Charset) -> String {
    encode(&decode(text, from), to)
}

fn combining_to_mark(c: char) -> Option<Result<Tone, Mark>> {
    if let Some((t, _)) = COMBINING_TONES.iter().find(|(_, x)| *x == c) {
        return Some(Ok(*t));
    }
    match c {
        '\u{302}' => Some(Err(Mark::Hat)),
        '\u{306}' => Some(Err(Mark::Breve)),
        '\u{31B}' => Some(Err(Mark::Horn)),
        _ => None,
    }
}

/// Unicode (dựng sẵn hoặc tổ hợp) → dựng sẵn.
fn decode_unicode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let Some((mut l, mut t)) = decompose(c) else {
            out.push(c);
            continue;
        };
        let mut combined = false;
        while let Some(&n) = chars.peek() {
            match combining_to_mark(n) {
                Some(Ok(tone)) if t == Tone::None && l.is_vowel() => t = tone,
                Some(Err(mark)) if l.mark == Mark::None && cinput_core::types::mark_fits(l.base, mark) => {
                    l.mark = mark
                }
                _ => break,
            }
            combined = true;
            chars.next();
        }
        out.push(if combined { compose(l, t) } else { c });
    }
    out
}

fn decode_vni(text: &str) -> String {
    let m = maps();
    let bytes: Vec<Option<u8>> = text.chars().map(char_to_byte).collect();
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if let (Some(b), Some(Some(nb))) = (bytes[i], bytes.get(i + 1)) {
            let code = ((*nb as u16) << 8) | b as u16;
            if let Some(&c) = m.vni_dec.get(&code) {
                out.push(c);
                i += 2;
                continue;
            }
        }
        match bytes[i].and_then(|b| m.vni_dec.get(&(b as u16))) {
            Some(&c) => out.push(c),
            None => out.push(chars[i]),
        }
        i += 1;
    }
    out
}

fn decode_viqr(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let mut l = Letter::plain(c);
        let mut t = Tone::None;
        let mut j = i + 1;
        if l.base == 'd' && chars.get(j).is_some_and(|n| n.eq_ignore_ascii_case(&'d')) {
            l.mark = Mark::Stroke;
            j += 1;
        } else if l.is_vowel() {
            let mark = match chars.get(j) {
                Some('^') => Some(Mark::Hat),
                Some('(') => Some(Mark::Breve),
                Some('+') => Some(Mark::Horn),
                _ => None,
            };
            if let Some(mk) = mark.filter(|mk| cinput_core::types::mark_fits(l.base, *mk)) {
                l.mark = mk;
                j += 1;
            }
            if let Some(&tc) = chars.get(j) {
                if let Some((tone, _)) = VIQR_TONES.iter().find(|(_, k)| *k == tc) {
                    // '.' cuối câu (theo sau là khoảng trắng / hết chuỗi) là dấu chấm, không phải dấu nặng.
                    // Muốn gõ dấu câu ngay sau nguyên âm thì dùng '\' (vd: "ba\?").
                    let sentence_end = tc == '.'
                        && chars.get(j + 1).is_none_or(|n| n.is_whitespace() || *n == '.');
                    if !sentence_end {
                        t = *tone;
                        j += 1;
                    }
                }
            }
        }
        out.push(compose(l, t));
        i = j;
    }
    out
}

fn decode_ncr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("&#") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 2..];
        let parsed = after.find(';').and_then(|end| {
            let body = &after[..end];
            let n = if let Some(h) = body.strip_prefix(['x', 'X']) {
                u32::from_str_radix(h, 16).ok()
            } else {
                body.parse::<u32>().ok()
            };
            n.and_then(char::from_u32).map(|c| (c, end))
        });
        match parsed {
            Some((c, end)) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push_str("&#");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------- tiện ích

/// Bỏ dấu tiếng Việt: "Tiếng Việt" → "Tieng Viet".
pub fn remove_tones(text: &str) -> String {
    text.chars()
        .map(|c| match decompose(c) {
            Some((l, _)) => {
                if l.upper {
                    l.base.to_ascii_uppercase()
                } else {
                    l.base
                }
            }
            None => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Tiếng Việt có dấu: ĐƯỜNG Đi, người ở thuở xưa, khuỷu tay, Ấn Độ, ăn uống.";

    #[test]
    fn roundtrip_all() {
        for cs in Charset::ALL {
            let enc = encode(SAMPLE, cs);
            let dec = decode(&enc, cs);
            let expect = if cs == Charset::Tcvn3 { SAMPLE.to_lowercase() } else { SAMPLE.to_string() };
            let got = if cs == Charset::Tcvn3 { dec.to_lowercase() } else { dec };
            assert_eq!(got, expect, "{cs:?}: {enc}");
        }
    }

    #[test]
    fn known_bytes() {
        // TCVN3: "á" = 0xB8; VNI: "á" = 'a' + 0xF9.
        assert_eq!(encode("á", Charset::Tcvn3), "\u{B8}");
        assert_eq!(encode("á", Charset::VniWindows), "a\u{F9}");
        assert_eq!(encode("Việt", Charset::Viqr), "Vie^.t");
        assert_eq!(decode("Vie^.t Nam.", Charset::Viqr), "Việt Nam.");
        assert_eq!(encode("ế", Charset::UnicodeComposite), "ê\u{301}");
        assert_eq!(decode("e\u{302}\u{301}", Charset::Unicode), "ế");
        assert_eq!(encode("ệ", Charset::NcrDecimal), "&#7879;");
        assert_eq!(remove_tones("Đường Việt"), "Duong Viet");
    }
}
