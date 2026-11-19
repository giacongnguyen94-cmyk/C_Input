//! Kiểu gõ: chỉ "phân loại" phím (dấu thanh, dấu phụ, chữ thường); engine quyết định áp dụng.

use crate::types::{Letter, Mark, Tone};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum InputMethod {
    #[default]
    Telex,
    /// Telex đơn giản: không có w đứng một mình thành ư, không dùng [ ] { }.
    SimpleTelex,
    Vni,
}

impl InputMethod {
    pub const ALL: [InputMethod; 3] = [InputMethod::Telex, InputMethod::Vni, InputMethod::SimpleTelex];

    pub fn label(self) -> &'static str {
        match self {
            InputMethod::Telex => "Telex",
            InputMethod::SimpleTelex => "Telex đơn giản",
            InputMethod::Vni => "VNI",
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

pub(crate) fn classify(method: InputMethod, key: char) -> Role {
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
    }
}

/// Phím ứng với dấu thanh (dùng khi mã hoá ngược một từ thành dãy phím).
pub(crate) fn tone_key(method: InputMethod, tone: Tone) -> Option<char> {
    let telex = ['s', 'f', 'r', 'x', 'j'];
    let vni = ['1', '2', '3', '4', '5'];
    let i = tone.index().checked_sub(1)?;
    Some(if method.is_telex() { telex[i] } else { vni[i] })
}

/// Phím ứng với dấu phụ của một chữ.
pub(crate) fn mark_key(method: InputMethod, l: &Letter) -> Option<char> {
    match (method.is_telex(), l.mark) {
        (_, Mark::None) => None,
        (true, Mark::Hat) => Some(l.base),
        (true, Mark::Horn | Mark::Breve) => Some('w'),
        (true, Mark::Stroke) => Some('d'),
        (false, Mark::Hat) => Some('6'),
        (false, Mark::Horn) => Some('7'),
        (false, Mark::Breve) => Some('8'),
        (false, Mark::Stroke) => Some('9'),
    }
}
