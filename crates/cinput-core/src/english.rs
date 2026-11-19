//! Danh sách từ tiếng Anh: khi người dùng gõ đúng một từ trong danh sách (dãy phím thô),
//! kết thúc từ thì giữ nguyên tiếng Anh thay vì âm tiết tiếng Việt mà Telex tạo ra (test ≠ tét).
//!
//! Danh sách có sẵn (english_words.txt) được kiểm thử để không trùng cách gõ thông thường của bất kỳ
//! âm tiết tiếng Việt nào. Người dùng có thể thêm từ của riêng mình (file english.txt).

use std::collections::HashSet;

const BUILTIN: &str = include_str!("english_words.txt");

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnglishWords {
    words: HashSet<String>,
}

fn words_of(text: &str) -> impl Iterator<Item = String> + '_ {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .flat_map(str::split_whitespace)
        .filter(|w| w.len() >= 2 && w.chars().all(|c| c.is_ascii_alphabetic()))
        .map(str::to_ascii_lowercase)
}

impl EnglishWords {
    /// Danh sách có sẵn.
    pub fn builtin() -> EnglishWords {
        EnglishWords { words: words_of(BUILTIN).collect() }
    }

    /// Thêm từ từ một văn bản (mỗi dòng một hoặc nhiều từ, dòng bắt đầu bằng '#' là chú thích).
    pub fn extend_from_text(&mut self, text: &str) {
        self.words.extend(words_of(text));
    }

    /// Dãy phím (không phân biệt hoa thường) có phải một từ tiếng Anh trong danh sách,
    /// kể cả dạng chia -ed, -d, -ing, -ly (passed, using, stopped, copied...).
    /// Không suy đuôi -s / -es / -er: s và r là phím dấu sắc / hỏi của Telex ("songs" là cách gõ "sóng") —
    /// số nhiều thông dụng (tests, files...) được ghi thẳng trong danh sách.
    pub fn contains(&self, raw: &str) -> bool {
        if raw.len() < 2 || !raw.is_ascii() {
            return false;
        }
        let w = raw.to_ascii_lowercase();
        if self.words.contains(&w) {
            return true;
        }
        let has = |s: &str| s.len() >= 2 && self.words.contains(s);
        for suffix in ["ed", "d", "ing", "ly"] {
            let Some(stem) = w.strip_suffix(suffix) else { continue };
            if has(stem) {
                return true;
            }
            // using → use, saved → save
            if matches!(suffix, "ing" | "ed") && has(&format!("{stem}e")) {
                return true;
            }
            // stopped → stop, running → run
            let b = stem.as_bytes();
            if matches!(suffix, "ing" | "ed") && b.len() >= 3 && b[b.len() - 1] == b[b.len() - 2] {
                if has(&stem[..stem.len() - 1]) {
                    return true;
                }
            }
            // copied → copy
            if suffix == "ed" {
                if let Some(s) = stem.strip_suffix('i') {
                    if has(&format!("{s}y")) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.words.iter().map(String::as_str)
    }
}
