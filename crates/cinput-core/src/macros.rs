//! Bảng gõ tắt. Định dạng file tương thích UniKey: mỗi dòng "từ_tắt:nội dung",
//! dòng bắt đầu bằng ';' là chú thích.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MacroTable {
    /// Khoá viết thường → (từ tắt như người dùng định nghĩa, nội dung).
    items: BTreeMap<String, (String, String)>,
    /// Tự viết hoa theo chữ gõ: VN → VIỆT NAM, Vn → Việt nam.
    pub auto_case: bool,
}

impl MacroTable {
    pub fn new(auto_case: bool) -> MacroTable {
        MacroTable { items: BTreeMap::new(), auto_case }
    }

    pub fn parse(text: &str, auto_case: bool) -> MacroTable {
        let mut t = MacroTable::new(auto_case);
        for line in text.lines() {
            let line = line.trim_start_matches('\u{feff}');
            if line.trim().is_empty() || line.starts_with(';') {
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                t.insert(k.trim(), v.trim_end_matches('\r'));
            }
        }
        t
    }

    pub fn serialize(&self) -> String {
        let mut s = String::from(";C_Input macro file — mỗi dòng: từ_tắt:nội dung\n");
        for (k, v) in self.items.values() {
            s.push_str(k);
            s.push(':');
            s.push_str(v);
            s.push('\n');
        }
        s
    }

    pub fn insert(&mut self, key: &str, value: &str) {
        let key = key.trim();
        if key.is_empty() || key.contains(':') {
            return;
        }
        self.items.insert(key.to_lowercase(), (key.to_string(), value.to_string()));
    }

    pub fn remove(&mut self, key: &str) {
        self.items.remove(&key.to_lowercase());
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.items.values().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Nội dung thay thế cho từ vừa gõ, hoặc `None`.
    pub fn expand(&self, word: &str) -> Option<String> {
        let (key, value) = self.items.get(&word.to_lowercase())?;
        if !self.auto_case {
            return (key == word).then(|| value.clone());
        }
        let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
        let all_upper = letters.len() > 1 && letters.iter().all(|c| c.is_uppercase());
        let first_upper = word.chars().next().is_some_and(|c| c.is_uppercase());
        let key_first_upper = key.chars().next().is_some_and(|c| c.is_uppercase());
        Some(if all_upper && key != word {
            value.to_uppercase()
        } else if first_upper && !key_first_upper {
            let mut cs = value.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().chain(cs).collect(),
                None => String::new(),
            }
        } else {
            value.clone()
        })
    }
}
