//! Giao diện hai ngôn ngữ: chuỗi tiếng Việt trong mã nguồn là khoá; bản tiếng Anh nằm trong
//! `i18n_en.txt` (mỗi dòng "tiếng Việt<TAB>English"). Thiếu bản dịch thì giữ nguyên tiếng Việt.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);
const EN_TEXT: &str = include_str!("i18n_en.txt");

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| {
        EN_TEXT
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once('\t'))
            .map(|(vi, en)| (vi, en))
            .collect()
    })
}

/// Chọn ngôn ngữ giao diện: "en" hoặc "vi".
pub fn set_lang(code: &str) {
    ENGLISH.store(code.eq_ignore_ascii_case("en"), Ordering::Relaxed);
}

pub fn is_en() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Bản dịch của một chuỗi giao diện.
pub fn t(vi: &str) -> &str {
    if !is_en() {
        return vi;
    }
    table().get(vi).copied().unwrap_or(vi)
}

/// Điền tham số vào mẫu có các chỗ trống `{}` (theo thứ tự): `fill(t("Đã nhập {} gõ tắt"), &[&n])`.
pub fn fill(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    let mut i = 0;
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        if let Some(a) = args.get(i) {
            out.push_str(&a.to_string());
        }
        i += 1;
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_and_fallback() {
        assert_eq!(fill("Đã nhập {} gõ tắt ({})", &[&5, &"ok"]), "Đã nhập 5 gõ tắt (ok)");
        set_lang("vi");
        assert_eq!(t("Bộ gõ"), "Bộ gõ");
    }

    #[test]
    fn dictionary_is_well_formed() {
        for (n, line) in EN_TEXT.lines().enumerate() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let (vi, en) = line.split_once('\t').unwrap_or_else(|| panic!("dòng {} thiếu TAB", n + 1));
            assert!(!vi.is_empty() && !en.is_empty(), "dòng {}", n + 1);
            assert_eq!(vi.matches("{}").count(), en.matches("{}").count(), "số chỗ trống {{}} khác nhau ở dòng {}: {vi}", n + 1);
        }
    }
}
