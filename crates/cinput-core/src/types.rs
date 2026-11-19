//! Kiểu dữ liệu cơ bản: dấu thanh, dấu phụ, chữ cái và bảng ghép chữ có dấu.

/// Dấu thanh (thuộc về cả âm tiết, vị trí đặt được tính khi hiển thị).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Tone {
    #[default]
    None,
    /// sắc
    Acute,
    /// huyền
    Grave,
    /// hỏi
    Hook,
    /// ngã
    Tilde,
    /// nặng
    Dot,
}

impl Tone {
    pub(crate) fn index(self) -> usize {
        match self {
            Tone::None => 0,
            Tone::Acute => 1,
            Tone::Grave => 2,
            Tone::Hook => 3,
            Tone::Tilde => 4,
            Tone::Dot => 5,
        }
    }

    pub(crate) fn from_index(i: usize) -> Tone {
        [Tone::None, Tone::Acute, Tone::Grave, Tone::Hook, Tone::Tilde, Tone::Dot][i]
    }
}

/// Dấu phụ gắn vào từng chữ cái.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Mark {
    #[default]
    None,
    /// mũ: â ê ô
    Hat,
    /// móc: ơ ư
    Horn,
    /// trăng: ă
    Breve,
    /// gạch ngang: đ
    Stroke,
}

/// Một chữ cái trong từ đang gõ: chữ gốc (a–z viết thường), dấu phụ và chữ hoa/thường.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Letter {
    pub base: char,
    pub mark: Mark,
    pub upper: bool,
}

impl Letter {
    pub fn plain(c: char) -> Letter {
        Letter { base: lower(c), mark: Mark::None, upper: c.is_uppercase() }
    }

    pub fn with_mark(c: char, mark: Mark) -> Letter {
        Letter { base: lower(c), mark, upper: c.is_uppercase() }
    }

    pub fn is_vowel(&self) -> bool {
        is_vowel(self.base)
    }
}

pub fn is_vowel(base: char) -> bool {
    matches!(base, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

pub(crate) fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn upper(c: char) -> char {
    c.to_uppercase().next().unwrap_or(c)
}

/// Bảng nguyên âm: (chữ gốc, dấu phụ, [không dấu, sắc, huyền, hỏi, ngã, nặng]).
const VOWELS: [(char, Mark, [char; 6]); 12] = [
    ('a', Mark::None, ['a', 'á', 'à', 'ả', 'ã', 'ạ']),
    ('a', Mark::Hat, ['â', 'ấ', 'ầ', 'ẩ', 'ẫ', 'ậ']),
    ('a', Mark::Breve, ['ă', 'ắ', 'ằ', 'ẳ', 'ẵ', 'ặ']),
    ('e', Mark::None, ['e', 'é', 'è', 'ẻ', 'ẽ', 'ẹ']),
    ('e', Mark::Hat, ['ê', 'ế', 'ề', 'ể', 'ễ', 'ệ']),
    ('i', Mark::None, ['i', 'í', 'ì', 'ỉ', 'ĩ', 'ị']),
    ('o', Mark::None, ['o', 'ó', 'ò', 'ỏ', 'õ', 'ọ']),
    ('o', Mark::Hat, ['ô', 'ố', 'ồ', 'ổ', 'ỗ', 'ộ']),
    ('o', Mark::Horn, ['ơ', 'ớ', 'ờ', 'ở', 'ỡ', 'ợ']),
    ('u', Mark::None, ['u', 'ú', 'ù', 'ủ', 'ũ', 'ụ']),
    ('u', Mark::Horn, ['ư', 'ứ', 'ừ', 'ử', 'ữ', 'ự']),
    ('y', Mark::None, ['y', 'ý', 'ỳ', 'ỷ', 'ỹ', 'ỵ']),
];

/// Kiểm tra một chữ gốc có nhận được dấu phụ này không (vd: 'a' nhận mũ/trăng, 'u' chỉ nhận móc).
pub fn mark_fits(base: char, mark: Mark) -> bool {
    match mark {
        Mark::None => true,
        Mark::Stroke => base == 'd',
        _ => VOWELS.iter().any(|(b, m, _)| *b == base && *m == mark),
    }
}

/// Ghép chữ cái (kèm dấu thanh nếu chữ này mang dấu thanh) thành ký tự Unicode dựng sẵn.
pub fn compose(l: Letter, tone: Tone) -> char {
    let c = if l.base == 'd' && l.mark == Mark::Stroke {
        'đ'
    } else if let Some((_, _, forms)) = VOWELS.iter().find(|(b, m, _)| *b == l.base && *m == l.mark) {
        forms[tone.index()]
    } else if let Some((_, _, forms)) = VOWELS.iter().find(|(b, m, _)| *b == l.base && *m == Mark::None) {
        forms[tone.index()]
    } else {
        l.base
    };
    if l.upper { upper(c) } else { c }
}

/// Tách một ký tự (có thể có dấu) thành chữ cái + dấu thanh. `None` nếu không phải chữ cái.
pub fn decompose(c: char) -> Option<(Letter, Tone)> {
    let is_upper = c.is_uppercase();
    let lc = lower(c);
    if lc == 'đ' {
        return Some((Letter { base: 'd', mark: Mark::Stroke, upper: is_upper }, Tone::None));
    }
    for (base, mark, forms) in VOWELS.iter() {
        if let Some(i) = forms.iter().position(|f| *f == lc) {
            return Some((Letter { base: *base, mark: *mark, upper: is_upper }, Tone::from_index(i)));
        }
    }
    if lc.is_ascii_lowercase() {
        return Some((Letter { base: lc, mark: Mark::None, upper: is_upper }, Tone::None));
    }
    None
}

/// Chuỗi viết thường, không dấu thanh nhưng giữ dấu phụ (dùng để so khớp chính tả).
pub(crate) fn letters_key(letters: &[Letter]) -> String {
    letters
        .iter()
        .map(|l| compose(Letter { upper: false, ..*l }, Tone::None))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_roundtrip() {
        for c in "aáàảãạâấầẩẫậăắằẳẵặeéèẻẽẹêếềểễệiíìỉĩịoóòỏõọôốồổỗộơớờởỡợuúùủũụưứừửữựyýỳỷỹỵđ".chars() {
            for ch in [c, upper(c)] {
                let (l, t) = decompose(ch).unwrap();
                assert_eq!(compose(l, t), ch);
            }
        }
    }
}
