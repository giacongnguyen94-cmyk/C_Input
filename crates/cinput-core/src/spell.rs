//! Cấu trúc âm tiết tiếng Việt: tách phụ âm đầu / vần / phụ âm cuối, kiểm tra chính tả,
//! và quy tắc đặt dấu thanh.

use std::ops::Range;
use std::sync::OnceLock;

use crate::types::{Letter, Mark, Tone, decompose, letters_key};

const INITIALS: [&str; 28] = [
    "", "b", "c", "ch", "d", "đ", "g", "gh", "gi", "h", "k", "kh", "l", "m", "n", "ng", "ngh", "nh",
    "p", "ph", "qu", "r", "s", "t", "th", "tr", "v", "x",
];

const FINALS: [&str; 8] = ["c", "ch", "m", "n", "ng", "nh", "p", "t"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FinalRule {
    /// Vần không được có phụ âm cuối (vd: ai, ươi).
    Forbidden,
    /// Có hay không đều được (vd: a, oa).
    Optional,
    /// Bắt buộc có phụ âm cuối (vd: ă, iê, ươ).
    Required,
}

use FinalRule::*;

const NUCLEI: [(&str, FinalRule); 55] = [
    ("a", Optional), ("ă", Required), ("â", Required), ("e", Optional), ("ê", Optional),
    ("i", Optional), ("o", Optional), ("ô", Optional), ("ơ", Optional), ("u", Optional),
    ("ư", Optional), ("y", Forbidden),
    ("ai", Forbidden), ("ao", Forbidden), ("au", Forbidden), ("ay", Forbidden), ("âu", Forbidden),
    ("ây", Forbidden), ("eo", Forbidden), ("êu", Forbidden), ("ia", Forbidden), ("iu", Forbidden),
    ("iê", Required), ("yê", Required), ("oa", Optional), ("oă", Required), ("oe", Optional),
    ("oi", Forbidden), ("ôi", Forbidden), ("ơi", Forbidden), ("oo", Required), ("ua", Forbidden),
    ("uâ", Required), ("uê", Optional), ("ui", Forbidden), ("uô", Required), ("uơ", Forbidden),
    ("uy", Optional), ("ưa", Forbidden), ("ưi", Forbidden), ("ươ", Required), ("ưu", Forbidden),
    ("iêu", Forbidden), ("yêu", Forbidden), ("oai", Forbidden), ("oay", Forbidden), ("oeo", Forbidden),
    ("uây", Forbidden), ("uôi", Forbidden), ("ươi", Forbidden), ("ươu", Forbidden), ("uyê", Required),
    ("uya", Forbidden), ("uyu", Forbidden), ("oao", Forbidden),
];

type Nucleus = (Vec<(char, Mark)>, FinalRule);

fn nuclei() -> &'static [Nucleus] {
    static N: OnceLock<Vec<Nucleus>> = OnceLock::new();
    N.get_or_init(|| {
        NUCLEI
            .iter()
            .map(|(s, rule)| {
                let parts = s.chars().map(|c| {
                    let (l, _) = decompose(c).expect("bảng vần hợp lệ");
                    (l.base, l.mark)
                });
                (parts.collect(), *rule)
            })
            .collect()
    })
}

/// Vị trí ba phần của âm tiết trong dãy chữ cái.
#[derive(Debug, Clone)]
pub struct Parts {
    pub initial: Range<usize>,
    pub nucleus: Range<usize>,
    pub final_: Range<usize>,
    /// `false` nếu sau phụ âm cuối lại có nguyên âm (không phải một âm tiết).
    pub tail_ok: bool,
}

/// Tách âm tiết. Xử lý "qu" và "gi" như phụ âm đầu.
pub fn split(letters: &[Letter]) -> Parts {
    let n = letters.len();
    let mut i = 0;
    while i < n && !letters[i].is_vowel() {
        i += 1;
    }
    if i == 1 && n > 1 && letters[0].base == 'q' && letters[1].base == 'u' {
        i = 2;
    } else if i == 1
        && n > 2
        && letters[0].base == 'g'
        && letters[1].base == 'i'
        && letters[1].mark == Mark::None
        && letters[2].is_vowel()
    {
        i = 2;
    }
    let ns = i;
    while i < n && letters[i].is_vowel() {
        i += 1;
    }
    let ne = i;
    let tail_ok = letters[ne..].iter().all(|l| !l.is_vowel());
    Parts { initial: 0..ns, nucleus: ns..ne, final_: ne..n, tail_ok }
}

fn compatible(typed: &[Letter], pattern: &[(char, Mark)]) -> bool {
    typed
        .iter()
        .zip(pattern)
        .all(|(l, (b, m))| l.base == *b && (l.mark == *m || l.mark == Mark::None))
}

fn stop_final(f: &str) -> bool {
    f.starts_with('c') || f.starts_with('p') || f.starts_with('t')
}

fn tone_ok_with_final(tone: Tone, final_: &str) -> bool {
    !(stop_final(final_) && matches!(tone, Tone::Grave | Tone::Hook | Tone::Tilde))
}

/// Từ đang gõ dở có thể trở thành một âm tiết tiếng Việt hợp lệ không?
/// Cho phép thiếu dấu phụ (người dùng có thể thêm dấu sau — gõ dấu tự do).
pub fn is_valid_prefix(letters: &[Letter], tone: Tone) -> bool {
    let p = split(letters);
    if !p.tail_ok {
        return false;
    }
    let init = letters_key(&letters[p.initial.clone()]);
    let nuc = &letters[p.nucleus.clone()];
    let fin = letters_key(&letters[p.final_.clone()]);
    if letters[p.initial.clone()].iter().any(|l| l.mark != Mark::None && l.mark != Mark::Stroke)
        || letters[p.final_.clone()].iter().any(|l| l.mark != Mark::None)
    {
        return false;
    }
    if nuc.is_empty() {
        return tone == Tone::None && INITIALS.iter().any(|i| i.starts_with(init.as_str()));
    }
    if !INITIALS.contains(&init.as_str()) {
        return false;
    }
    if fin.is_empty() {
        return nuclei().iter().any(|(pat, _)| pat.len() >= nuc.len() && compatible(nuc, pat));
    }
    FINALS.contains(&fin.as_str())
        && tone_ok_with_final(tone, &fin)
        && nuclei()
            .iter()
            .any(|(pat, rule)| *rule != Forbidden && pat.len() == nuc.len() && compatible(nuc, pat))
}

/// Từ đã gõ xong có phải một âm tiết tiếng Việt hoàn chỉnh không?
pub fn is_valid_word(letters: &[Letter], tone: Tone) -> bool {
    let p = split(letters);
    if !p.tail_ok || p.nucleus.is_empty() {
        return false;
    }
    let init = letters_key(&letters[p.initial.clone()]);
    let nuc = &letters[p.nucleus.clone()];
    let fin = letters_key(&letters[p.final_.clone()]);
    if !INITIALS.contains(&init.as_str()) {
        return false;
    }
    if !fin.is_empty() && !FINALS.contains(&fin.as_str()) {
        return false;
    }
    if !tone_ok_with_final(tone, &fin) {
        return false;
    }
    nuclei().iter().any(|(pat, rule)| {
        pat.len() == nuc.len()
            && nuc.iter().zip(pat).all(|(l, (b, m))| l.base == *b && l.mark == *m)
            && match rule {
                Forbidden => fin.is_empty(),
                Optional => true,
                Required => !fin.is_empty(),
            }
    })
}

/// Vị trí chữ cái mang dấu thanh.
/// `modern = true`: đặt dấu kiểu mới (hoà, khoẻ, thuý); `false`: kiểu cũ (hòa, khỏe, thúy).
pub fn tone_position(letters: &[Letter], modern: bool) -> Option<usize> {
    let p = split(letters);
    let (ns, ne) = (p.nucleus.start, p.nucleus.end);
    if ns == ne {
        return None;
    }
    let nuc = &letters[ns..ne];
    if let Some(i) = nuc.iter().rposition(|l| matches!(l.mark, Mark::Hat | Mark::Horn | Mark::Breve)) {
        return Some(ns + i);
    }
    match nuc.len() {
        1 => Some(ns),
        2 => {
            if !p.final_.is_empty() {
                return Some(ns + 1);
            }
            let pair = (nuc[0].base, nuc[1].base);
            if modern && matches!(pair, ('o', 'a') | ('o', 'e') | ('u', 'y')) {
                Some(ns + 1)
            } else {
                Some(ns)
            }
        }
        _ => Some(ns + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn letters(s: &str) -> (Vec<Letter>, Tone) {
        let mut tone = Tone::None;
        let v = s
            .chars()
            .map(|c| {
                let (l, t) = decompose(c).unwrap();
                if t != Tone::None {
                    tone = t;
                }
                l
            })
            .collect();
        (v, tone)
    }

    #[test]
    fn valid_words() {
        for w in [
            "tiếng", "việt", "người", "quốc", "giữa", "gìn", "khuya", "khuỷu", "nghiêng", "thuở",
            "hoà", "hòa", "oán", "yêu", "uống", "xoong", "đường", "chuyện", "quyết", "giường", "ạ",
        ] {
            let (l, t) = letters(w);
            assert!(is_valid_word(&l, t), "{w} phải hợp lệ");
        }
    }

    #[test]
    fn invalid_words() {
        for w in ["text", "tẽt", "ưin", "user", "ủe", "str", "aoă", "ăo", "fan"] {
            let (l, t) = letters(w);
            assert!(!is_valid_word(&l, t), "{w} phải không hợp lệ");
        }
    }
}
