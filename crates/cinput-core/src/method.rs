//! Kiểu gõ: chỉ "phân loại" phím (dấu thanh, dấu phụ, chữ thường); engine quyết định áp dụng.

use std::collections::BTreeMap;

use crate::types::{Letter, Mark, Tone};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum InputMethod {
    #[default]
    Telex,
    /// Telex đơn giản: không có w đứng một mình thành ư, không dùng [ ] { }.
    SimpleTelex,
    Vni,
    /// VIQR: a' a` a? a~ a. (thanh), a^ a( o+ (dấu phụ), dd (đ).
    Viqr,
    /// Kiểu gõ tự định nghĩa (bảng phím `KeyMap`, đọc được file keymap.txt của UniKey).
    Custom,
}

impl InputMethod {
    pub const ALL: [InputMethod; 5] =
        [InputMethod::Telex, InputMethod::Vni, InputMethod::Viqr, InputMethod::SimpleTelex, InputMethod::Custom];

    pub fn label(self) -> &'static str {
        match self {
            InputMethod::Telex => "Telex",
            InputMethod::SimpleTelex => "Telex đơn giản",
            InputMethod::Vni => "VNI",
            InputMethod::Viqr => "VIQR",
            InputMethod::Custom => "Tự định nghĩa",
        }
    }

    pub fn is_telex(self) -> bool {
        matches!(self, InputMethod::Telex | InputMethod::SimpleTelex)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Role {
    Tone(Tone),
    RemoveTone,
    /// Telex: aa ee oo — dấu mũ cho đúng chữ này.
    Hat(char),
    /// VNI 6: dấu mũ cho a/e/o phù hợp.
    HatAny,
    /// Telex w: móc (ư ơ) hoặc trăng (ă).
    HornOrBreve,
    /// VNI 7: chỉ móc.
    Horn,
    /// VNI 8: chỉ trăng.
    Breve,
    /// dd / 9
    Stroke,
    /// Chèn thẳng một chữ có dấu (Telex: [ ] { }).
    Insert(Letter),
    Letter,
}

// ---------------------------------------------------------------- kiểu gõ tự định nghĩa

/// Chức năng gán cho một phím trong kiểu gõ tự định nghĩa. Tên theo đúng file keymap.txt của UniKey.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum KeyAction {
    /// 0 = xoá dấu, 1 sắc, 2 huyền, 3 hỏi, 4 ngã, 5 nặng.
    Tone(u8),
    /// Mũ cho a / e / o phù hợp.
    RoofAll,
    RoofA,
    RoofE,
    RoofO,
    /// Móc (ư ơ) hoặc trăng (ă) — như w của Telex.
    HookBowl,
    /// Như Hook-Bowl, và gõ riêng thì ra ư — đúng phím w của Telex.
    TelexW,
    /// Chỉ móc (ư ơ).
    Hook,
    /// Chỉ trăng (ă).
    Bowl,
    /// dd → đ.
    DMark,
    /// Chèn thẳng một chữ có dấu: â ê ô ă ơ ư đ.
    Letter(char),
}

const LETTER_ACTIONS: [(&str, char); 7] =
    [("A^", 'â'), ("E^", 'ê'), ("O^", 'ô'), ("A(", 'ă'), ("O+", 'ơ'), ("U+", 'ư'), ("DD", 'đ')];

impl KeyAction {
    /// Tên chức năng trong file keymap.txt (UniKey).
    pub fn name(self) -> String {
        match self {
            KeyAction::Tone(n) => format!("Tone{n}"),
            KeyAction::RoofAll => "Roof-All".into(),
            KeyAction::RoofA => "Roof-A".into(),
            KeyAction::RoofE => "Roof-E".into(),
            KeyAction::RoofO => "Roof-O".into(),
            KeyAction::HookBowl => "Hook-Bowl".into(),
            KeyAction::TelexW => "Telex-W".into(),
            KeyAction::Hook => "Hook-UO".into(),
            KeyAction::Bowl => "Bowl".into(),
            KeyAction::DMark => "D-Mark".into(),
            KeyAction::Letter(c) => {
                LETTER_ACTIONS.iter().find(|(_, l)| *l == c).map(|(n, _)| n.to_string()).unwrap_or_default()
            }
        }
    }

    pub fn parse(name: &str) -> Option<KeyAction> {
        let n = name.trim();
        if let Some(d) = n.strip_prefix("Tone").and_then(|d| d.parse::<u8>().ok()) {
            return (d <= 5).then_some(KeyAction::Tone(d));
        }
        Some(match n.to_ascii_lowercase().as_str() {
            "roof-all" => KeyAction::RoofAll,
            "roof-a" => KeyAction::RoofA,
            "roof-e" => KeyAction::RoofE,
            "roof-o" => KeyAction::RoofO,
            "hook-bowl" => KeyAction::HookBowl,
            "telex-w" => KeyAction::TelexW,
            "hook-uo" | "hook" | "hook-u" | "hook-o" => KeyAction::Hook,
            "bowl" => KeyAction::Bowl,
            "d-mark" | "dd-mark" => KeyAction::DMark,
            _ => return LETTER_ACTIONS.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, l)| KeyAction::Letter(*l)),
        })
    }

    /// Mô tả tiếng Việt để hiện trong giao diện.
    pub fn describe(self) -> &'static str {
        match self {
            KeyAction::Tone(0) => "Xoá dấu thanh",
            KeyAction::Tone(1) => "Dấu sắc",
            KeyAction::Tone(2) => "Dấu huyền",
            KeyAction::Tone(3) => "Dấu hỏi",
            KeyAction::Tone(4) => "Dấu ngã",
            KeyAction::Tone(_) => "Dấu nặng",
            KeyAction::RoofAll => "Mũ cho a / e / o (â ê ô)",
            KeyAction::RoofA => "Mũ cho a (â)",
            KeyAction::RoofE => "Mũ cho e (ê)",
            KeyAction::RoofO => "Mũ cho o (ô)",
            KeyAction::HookBowl => "Móc hoặc trăng (ư ơ ă)",
            KeyAction::TelexW => "Như w của Telex (ư ơ ă, gõ riêng ra ư)",
            KeyAction::Hook => "Móc (ư ơ)",
            KeyAction::Bowl => "Trăng (ă)",
            KeyAction::DMark => "Gạch d (đ)",
            KeyAction::Letter('â') => "Chữ â",
            KeyAction::Letter('ê') => "Chữ ê",
            KeyAction::Letter('ô') => "Chữ ô",
            KeyAction::Letter('ă') => "Chữ ă",
            KeyAction::Letter('ơ') => "Chữ ơ",
            KeyAction::Letter('ư') => "Chữ ư",
            KeyAction::Letter(_) => "Chữ đ",
        }
    }

    /// Mọi chức năng, theo thứ tự hiển thị.
    pub fn all() -> Vec<KeyAction> {
        let mut v: Vec<KeyAction> = (0..=5).map(KeyAction::Tone).collect();
        v.extend([
            KeyAction::RoofAll,
            KeyAction::RoofA,
            KeyAction::RoofE,
            KeyAction::RoofO,
            KeyAction::HookBowl,
            KeyAction::TelexW,
            KeyAction::Hook,
            KeyAction::Bowl,
            KeyAction::DMark,
        ]);
        v.extend(LETTER_ACTIONS.iter().map(|(_, l)| KeyAction::Letter(*l)));
        v
    }

    fn role(self, key: char) -> Role {
        let upper = key.is_uppercase();
        match self {
            KeyAction::Tone(0) => Role::RemoveTone,
            KeyAction::Tone(n) => Role::Tone(crate::types::Tone::from_index(n as usize)),
            KeyAction::RoofAll => Role::HatAny,
            KeyAction::RoofA => Role::Hat('a'),
            KeyAction::RoofE => Role::Hat('e'),
            KeyAction::RoofO => Role::Hat('o'),
            KeyAction::HookBowl | KeyAction::TelexW => Role::HornOrBreve,
            KeyAction::Hook => Role::Horn,
            KeyAction::Bowl => Role::Breve,
            KeyAction::DMark => Role::Stroke,
            KeyAction::Letter(c) => {
                let (l, _) = crate::types::decompose(c).expect("chữ có dấu hợp lệ");
                Role::Insert(Letter { upper, ..l })
            }
        }
    }
}

/// Bảng phím của kiểu gõ tự định nghĩa. Định dạng file giống keymap.txt của UniKey:
/// mỗi dòng `PHÍM = Chức năng` (vd `S = Tone1`, `W = Hook-Bowl`), dòng bắt đầu bằng ';' là chú thích.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyMap {
    keys: BTreeMap<char, KeyAction>,
}

impl KeyMap {
    /// Bảng mặc định: giống Telex (đúng như file keymap.txt UniKey tạo ra).
    pub fn telex_like() -> KeyMap {
        KeyMap::parse("Z = Tone0\nS = Tone1\nF = Tone2\nR = Tone3\nX = Tone4\nJ = Tone5\nW = Hook-Bowl\nA = Roof-A\nE = Roof-E\nO = Roof-O\nD = D-Mark\n")
    }

    pub fn parse(text: &str) -> KeyMap {
        let mut m = KeyMap::default();
        for line in text.lines() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let mut kc = k.trim().chars();
            if let (Some(c), None, Some(a)) = (kc.next(), kc.next(), KeyAction::parse(v)) {
                m.set(c, Some(a));
            }
        }
        m
    }

    pub fn serialize(&self) -> String {
        let mut s = String::from("; C_Input / UniKey user-defined key mapping file\n");
        for (k, a) in &self.keys {
            s.push_str(&format!("{} = {}\n", k.to_ascii_uppercase(), a.name()));
        }
        s
    }

    /// Gán (hoặc bỏ gán khi `None`) một chức năng cho phím. Không phân biệt hoa thường.
    pub fn set(&mut self, key: char, action: Option<KeyAction>) {
        let k = key.to_ascii_lowercase();
        if !(k.is_ascii_graphic()) {
            return;
        }
        match action {
            Some(a) => {
                self.keys.insert(k, a);
            }
            None => {
                self.keys.remove(&k);
            }
        }
    }

    /// Đặt phím duy nhất cho một chức năng (bỏ các phím cũ của chức năng đó). `None`: bỏ gán.
    pub fn set_action_key(&mut self, action: KeyAction, key: Option<char>) {
        self.keys.retain(|_, a| *a != action);
        if let Some(k) = key {
            self.set(k, Some(action));
        }
    }

    pub fn get(&self, key: char) -> Option<KeyAction> {
        self.keys.get(&key.to_ascii_lowercase()).copied()
    }

    /// Phím đang gán cho một chức năng (phím đầu tiên nếu có nhiều).
    pub fn key_for(&self, action: KeyAction) -> Option<char> {
        self.keys.iter().find(|(_, a)| **a == action).map(|(k, _)| *k)
    }

    pub fn iter(&self) -> impl Iterator<Item = (char, KeyAction)> + '_ {
        self.keys.iter().map(|(k, a)| (*k, *a))
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    fn standalone_w(&self, key: char) -> bool {
        self.get(key) == Some(KeyAction::TelexW)
    }
}

// ---------------------------------------------------------------- phân loại phím

pub(crate) fn classify(method: InputMethod, keymap: Option<&KeyMap>, key: char) -> Role {
    let k = key.to_ascii_lowercase();
    match method {
        InputMethod::Telex | InputMethod::SimpleTelex => match k {
            's' => Role::Tone(Tone::Acute),
            'f' => Role::Tone(Tone::Grave),
            'r' => Role::Tone(Tone::Hook),
            'x' => Role::Tone(Tone::Tilde),
            'j' => Role::Tone(Tone::Dot),
            'z' => Role::RemoveTone,
            'a' | 'e' | 'o' => Role::Hat(k),
            'w' => Role::HornOrBreve,
            'd' => Role::Stroke,
            _ if method == InputMethod::Telex => match key {
                '[' => Role::Insert(Letter { base: 'o', mark: Mark::Horn, upper: false }),
                ']' => Role::Insert(Letter { base: 'u', mark: Mark::Horn, upper: false }),
                '{' => Role::Insert(Letter { base: 'o', mark: Mark::Horn, upper: true }),
                '}' => Role::Insert(Letter { base: 'u', mark: Mark::Horn, upper: true }),
                _ => Role::Letter,
            },
            _ => Role::Letter,
        },
        InputMethod::Vni => match k {
            '1' => Role::Tone(Tone::Acute),
            '2' => Role::Tone(Tone::Grave),
            '3' => Role::Tone(Tone::Hook),
            '4' => Role::Tone(Tone::Tilde),
            '5' => Role::Tone(Tone::Dot),
            '0' => Role::RemoveTone,
            '6' => Role::HatAny,
            '7' => Role::Horn,
            '8' => Role::Breve,
            '9' => Role::Stroke,
            _ => Role::Letter,
        },
        InputMethod::Viqr => match key {
            '\'' => Role::Tone(Tone::Acute),
            '`' => Role::Tone(Tone::Grave),
            '?' => Role::Tone(Tone::Hook),
            '~' => Role::Tone(Tone::Tilde),
            '.' => Role::Tone(Tone::Dot),
            '-' => Role::RemoveTone,
            '^' => Role::HatAny,
            '(' => Role::Breve,
            '+' => Role::Horn,
            'd' | 'D' => Role::Stroke,
            _ => Role::Letter,
        },
        InputMethod::Custom => keymap.and_then(|m| m.get(key)).map(|a| a.role(key)).unwrap_or(Role::Letter),
    }
}

/// Phím Telex-W của kiểu gõ: gõ riêng (chưa có nguyên âm) thì ra ư.
pub(crate) fn standalone_w(method: InputMethod, keymap: Option<&KeyMap>, key: char) -> bool {
    match method {
        InputMethod::Telex => key.eq_ignore_ascii_case(&'w'),
        InputMethod::Custom => keymap.is_some_and(|m| m.standalone_w(key)),
        _ => false,
    }
}

/// Ký tự có thể là phím dấu của kiểu gõ (ngoài chữ cái): VNI số, VIQR dấu câu, Telex [ ], tự định nghĩa.
pub(crate) fn is_mark_char(method: InputMethod, keymap: Option<&KeyMap>, c: char) -> bool {
    match method {
        InputMethod::Telex => matches!(c, '[' | ']' | '{' | '}'),
        InputMethod::SimpleTelex => false,
        InputMethod::Vni => c.is_ascii_digit(),
        InputMethod::Viqr => matches!(c, '\'' | '`' | '?' | '~' | '.' | '-' | '^' | '(' | '+'),
        InputMethod::Custom => keymap.is_some_and(|m| m.get(c).is_some()),
    }
}

/// Phím ứng với dấu thanh (dùng khi mã hoá ngược một từ thành dãy phím).
pub(crate) fn tone_key(method: InputMethod, keymap: Option<&KeyMap>, tone: Tone) -> Option<char> {
    let i = tone.index().checked_sub(1)?;
    match method {
        InputMethod::Telex | InputMethod::SimpleTelex => Some(['s', 'f', 'r', 'x', 'j'][i]),
        InputMethod::Vni => Some(['1', '2', '3', '4', '5'][i]),
        InputMethod::Viqr => Some(['\'', '`', '?', '~', '.'][i]),
        InputMethod::Custom => keymap?.key_for(KeyAction::Tone(i as u8 + 1)),
    }
}

/// Phím ứng với dấu phụ của một chữ. `None` khi chữ không có dấu phụ;
/// `Some(None)` khi kiểu gõ không có phím cho dấu đó (không mã hoá ngược được).
pub(crate) fn mark_key(method: InputMethod, keymap: Option<&KeyMap>, l: &Letter) -> Option<Option<char>> {
    if l.mark == Mark::None {
        return None;
    }
    Some(match method {
        InputMethod::Telex | InputMethod::SimpleTelex => Some(match l.mark {
            Mark::Hat => l.base,
            Mark::Stroke => 'd',
            _ => 'w',
        }),
        InputMethod::Vni => Some(match l.mark {
            Mark::Hat => '6',
            Mark::Horn => '7',
            Mark::Breve => '8',
            _ => '9',
        }),
        InputMethod::Viqr => Some(match l.mark {
            Mark::Hat => '^',
            Mark::Horn => '+',
            Mark::Breve => '(',
            _ => 'd',
        }),
        InputMethod::Custom => {
            let m = keymap?;
            let hat = match l.base {
                'a' => KeyAction::RoofA,
                'e' => KeyAction::RoofE,
                _ => KeyAction::RoofO,
            };
            let options: &[KeyAction] = match l.mark {
                Mark::Hat => &[hat, KeyAction::RoofAll],
                Mark::Horn => &[KeyAction::Hook, KeyAction::HookBowl, KeyAction::TelexW],
                Mark::Breve => &[KeyAction::Bowl, KeyAction::HookBowl, KeyAction::TelexW],
                _ => &[KeyAction::DMark],
            };
            options.iter().find_map(|a| m.key_for(*a))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keymap_roundtrip_unikey_format() {
        // Đúng nội dung file keymap.txt UniKey tạo ra.
        let text = "; This is UniKey user-defined key mapping file, generated from UniKey (Windows)\n\nZ = Tone0\nS = Tone1\nF = Tone2\nR = Tone3\nX = Tone4\nJ = Tone5\nW = Hook-Bowl\nA = Roof-A\nE = Roof-E\nO = Roof-O\nD = D-Mark\n";
        let m = KeyMap::parse(text);
        assert_eq!(m.get('s'), Some(KeyAction::Tone(1)));
        assert_eq!(m.get('W'), Some(KeyAction::HookBowl));
        assert_eq!(m, KeyMap::telex_like());
        assert_eq!(KeyMap::parse(&m.serialize()), m);
        let extra = KeyMap::parse("[ = O+\n] = U+\n6 = Roof-All\n");
        assert_eq!(extra.get('['), Some(KeyAction::Letter('ơ')));
        assert_eq!(extra.get('6'), Some(KeyAction::RoofAll));
    }
}
