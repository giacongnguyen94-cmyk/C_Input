//! Chuyển đổi bảng mã tiếng Việt.
//!
//! Văn bản bảng mã cũ (TCVN3, VNI, VISCII...) được biểu diễn như Windows vẫn làm khi chép ra clipboard:
//! mỗi byte là một ký tự theo trang mã Windows-1252 (CP1258 theo trang mã 1258). Gõ bằng các bảng mã
//! này nghĩa là gửi đúng các ký tự đó tới ứng dụng (đang dùng font .VnTime, VNI-Times, VISCII...).

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
    Viscii,
    Vps,
    BkHcm1,
    BkHcm2,
    VietwareF,
    VietwareX,
    Cp1258,
    Utf8Literal,
    CString,
}

impl Charset {
    pub const ALL: [Charset; 16] = [
        Charset::Unicode,
        Charset::UnicodeComposite,
        Charset::Tcvn3,
        Charset::VniWindows,
        Charset::Viqr,
        Charset::Viscii,
        Charset::Vps,
        Charset::BkHcm1,
        Charset::BkHcm2,
        Charset::VietwareF,
        Charset::VietwareX,
        Charset::Cp1258,
        Charset::NcrDecimal,
        Charset::NcrHex,
        Charset::Utf8Literal,
        Charset::CString,
    ];

    /// Các bảng mã dùng được khi gõ.
    pub const TYPING: [Charset; 12] = [
        Charset::Unicode,
        Charset::UnicodeComposite,
        Charset::Tcvn3,
        Charset::VniWindows,
        Charset::Viqr,
        Charset::Viscii,
        Charset::Vps,
        Charset::BkHcm1,
        Charset::BkHcm2,
        Charset::VietwareF,
        Charset::VietwareX,
        Charset::Cp1258,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Charset::Unicode => "Unicode dựng sẵn",
            Charset::UnicodeComposite => "Unicode tổ hợp",
            Charset::Tcvn3 => "TCVN3 (ABC)",
            Charset::VniWindows => "VNI Windows",
            Charset::Viqr => "VIQR",
            Charset::NcrDecimal => "NCR thập phân",
            Charset::NcrHex => "NCR thập lục phân",
            Charset::Viscii => "VISCII",
            Charset::Vps => "VPS",
            Charset::BkHcm1 => "BK HCM 1",
            Charset::BkHcm2 => "BK HCM 2",
            Charset::VietwareF => "Vietware F",
            Charset::VietwareX => "Vietware X",
            Charset::Cp1258 => "Windows CP 1258",
            Charset::Utf8Literal => "UTF-8 Literal (sửa lỗi font)",
            Charset::CString => "Unicode C String (\\u1EBF)",
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
            Charset::Viscii => "VISCII",
            Charset::Vps => "VPS",
            Charset::BkHcm1 => "BK HCM 1",
            Charset::BkHcm2 => "BK HCM 2",
            Charset::VietwareF => "Vietware F",
            Charset::VietwareX => "Vietware X",
            Charset::Cp1258 => "CP1258",
            Charset::Utf8Literal => "UTF-8 Literal",
            Charset::CString => "C String",
        }
    }

    /// Tên bảng trong `tables::LEGACY` cho các bảng mã cũ dạng byte.
    fn legacy_table(self) -> Option<&'static str> {
        Some(match self {
            Charset::Tcvn3 => "tcvn3",
            Charset::VniWindows => "vni",
            Charset::Viscii => "viscii",
            Charset::Vps => "vps",
            Charset::BkHcm1 => "bkhcm1",
            Charset::BkHcm2 => "bkhcm2",
            Charset::VietwareF => "vietware_f",
            Charset::VietwareX => "vietware_x",
            Charset::Cp1258 => "cp1258",
            _ => return None,
        })
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

// ---------------------------------------------------------------- bảng mã cũ (dạng byte)

struct Legacy {
    enc: HashMap<char, &'static str>,
    dec: HashMap<&'static str, char>,
    /// Độ dài (số ký tự) dài nhất của một chữ trong bảng — để giải mã tham lam.
    max_len: usize,
}

fn legacy(name: &'static str) -> &'static Legacy {
    static ALL: OnceLock<HashMap<&'static str, Legacy>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        tables::LEGACY
            .iter()
            .map(|(name, reps)| {
                let mut l = Legacy { enc: HashMap::new(), dec: HashMap::new(), max_len: 1 };
                for (i, rep) in reps.iter().enumerate() {
                    if !rep.is_empty() {
                        l.enc.insert(tables::UNICODE[i], *rep);
                        l.max_len = l.max_len.max(rep.chars().count());
                    }
                }
                // Một số bảng (TCVN3...) không có chữ hoa có dấu thanh: chữ hoa dùng chung mã với chữ
                // thường. Khi giải mã ưu tiên chữ thường (bảng xen kẽ: chẵn = hoa, lẻ = thường).
                for parity in [1, 0] {
                    for (i, rep) in reps.iter().enumerate() {
                        if i % 2 == parity && !rep.is_empty() {
                            l.dec.entry(*rep).or_insert(tables::UNICODE[i]);
                        }
                    }
                }
                (*name, l)
            })
            .collect()
    });
    &all[name]
}

fn encode_legacy(text: &str, name: &'static str) -> String {
    let l = legacy(name);
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match l.enc.get(&c) {
            Some(rep) => out.push_str(rep),
            None => out.push(c),
        }
    }
    out
}

fn decode_legacy(text: &str, name: &'static str) -> String {
    let l = legacy(name);
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'outer: while i < chars.len() {
        for n in (1..=l.max_len.min(chars.len() - i)).rev() {
            let piece: String = chars[i..i + n].iter().collect();
            if let Some(&c) = l.dec.get(piece.as_str()) {
                out.push(c);
                i += n;
                continue 'outer;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- UTF-8 Literal, C String

/// UTF-8 Literal: mỗi byte UTF-8 hiện như một ký tự Windows-1252 — đúng dạng chữ bị "lỗi font"
/// thường gặp ("Tiáº¿ng Viá»‡t"). Giải mã chiều ngược lại để sửa.
fn encode_utf8_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push(byte_to_char(b));
            }
        }
    }
    out
}

fn decode_utf8_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run: Vec<u8> = Vec::new();
    let mut run_text = String::new();
    let flush = |run: &mut Vec<u8>, run_text: &mut String, out: &mut String| {
        if !run.is_empty() {
            match std::str::from_utf8(run) {
                Ok(s) => out.push_str(s),
                Err(_) => out.push_str(run_text),
            }
            run.clear();
            run_text.clear();
        }
    };
    for c in text.chars() {
        match char_to_byte(c) {
            Some(b) => {
                run.push(b);
                run_text.push(c);
            }
            None => {
                flush(&mut run, &mut run_text, &mut out);
                out.push(c);
            }
        }
    }
    flush(&mut run, &mut run_text, &mut out);
    out
}

fn encode_c_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c as u32 {
            0..=0x7F => out.push(c),
            u @ 0x80..=0xFFFF => out.push_str(&format!("\\u{u:04X}")),
            u => out.push_str(&format!("\\U{u:08X}")),
        }
    }
    out
}

fn decode_c_string(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && matches!(chars[i + 1], 'u' | 'U') {
            let n = if chars[i + 1] == 'u' { 4 } else { 8 };
            if i + 2 + n <= chars.len() {
                let hex: String = chars[i + 2..i + 2 + n].iter().collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                    i += 2 + n;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- mã hoá

/// Mã hoá một chuỗi Unicode sang bảng mã đích.
pub fn encode(text: &str, cs: Charset) -> String {
    if let Some(name) = cs.legacy_table() {
        return encode_legacy(text, name);
    }
    match cs {
        Charset::Utf8Literal => return encode_utf8_literal(text),
        Charset::CString => return encode_c_string(text),
        _ => {}
    }
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match cs {
            Charset::UnicodeComposite => encode_composite(c, &mut out),
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
            _ => out.push(c),
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
    if let Some(name) = cs.legacy_table() {
        // CP1258 thực chất là chữ gốc + dấu tổ hợp: chuẩn hoá thêm cho chắc.
        let d = decode_legacy(text, name);
        return if cs == Charset::Cp1258 { decode_unicode(&d) } else { d };
    }
    match cs {
        Charset::Viqr => decode_viqr(text),
        Charset::NcrDecimal | Charset::NcrHex => decode_ncr(text),
        Charset::Utf8Literal => decode_utf8_literal(text),
        Charset::CString => decode_c_string(text),
        _ => decode_unicode(text),
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

    /// Bảng mã thiếu chữ hoa có dấu thanh (dùng chung mã với chữ thường): so sánh không phân biệt hoa thường.
    fn lowercase_only(cs: Charset) -> bool {
        // VISCII (theo bảng của UniKey) cũng dùng chung mã chữ thường cho 7 chữ hoa Ẫ Ẳ Ẵ Õ Ỷ Ỹ Ỵ
        // thay vì vùng mã điều khiển 0x02, 0x05... mà nhiều ứng dụng Windows không nhận.
        matches!(cs, Charset::Tcvn3 | Charset::Vps | Charset::BkHcm1 | Charset::VietwareF | Charset::Viscii)
    }

    #[test]
    fn roundtrip_all() {
        for cs in Charset::ALL {
            let enc = encode(SAMPLE, cs);
            let dec = decode(&enc, cs);
            let (got, expect) = if lowercase_only(cs) {
                (dec.to_lowercase(), SAMPLE.to_lowercase())
            } else {
                (dec, SAMPLE.to_string())
            };
            assert_eq!(got, expect, "{cs:?}: {enc}");
        }
    }

    #[test]
    fn every_letter_every_legacy_charset() {
        // Mọi chữ cái tiếng Việt, mọi bảng mã cũ: mã hoá rồi giải mã phải ra đúng chữ đó.
        for cs in Charset::ALL.into_iter().filter(|c| c.legacy_table().is_some()) {
            let mut lossy = Vec::new();
            for &c in tables::UNICODE.iter() {
                let back = decode(&encode(&c.to_string(), cs), cs);
                let ok = back == c.to_string() || (lowercase_only(cs) && back == c.to_lowercase().to_string());
                // Giới hạn sẵn có của bảng BK HCM 1 (UniKey cũng vậy): Ụ trùng mã với Ặ, Ỵ trùng mã với Ị.
                let known = cs == Charset::BkHcm1 && matches!(c, 'Ụ' | 'Ỵ');
                if !ok && !known {
                    lossy.push(format!("{c}→{back}"));
                }
            }
            assert!(lossy.is_empty(), "{cs:?} sai: {}", lossy.join(" "));
        }
    }

    #[test]
    fn fixes_mojibake_and_c_strings() {
        // Chữ bị lỗi font kiểu UTF-8 đọc nhầm thành Windows-1252.
        assert_eq!(decode("Tiáº¿ng Viá»‡t", Charset::Utf8Literal), "Tiếng Việt");
        assert_eq!(encode("ế", Charset::Utf8Literal), "áº¿");
        assert_eq!(encode("Việt", Charset::CString), "Vi\\u1EC7t");
        assert_eq!(decode("Vi\\u1EC7t \\U0001F600", Charset::CString), "Việt 😀");
        // CP1258: chữ gốc + dấu tổ hợp.
        assert_eq!(encode("ế", Charset::Cp1258), "ê\u{301}");
        // VISCII (bảng UniKey): "ẳ" là byte 0xC6.
        assert_eq!(encode("ẳ", Charset::Viscii), "\u{C6}");
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
