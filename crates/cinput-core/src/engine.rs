//! Engine gõ tiếng Việt.
//!
//! Cách làm: lưu dãy phím thô của từ đang gõ, mỗi lần có phím mới thì tính lại toàn bộ từ
//! từ dãy phím đó (`transform`). Kết quả so với chữ đang hiển thị để ra một phép sửa
//! (xoá bao nhiêu ký tự cuối, chèn chuỗi gì). Cách này đơn giản, dễ kiểm thử, và tự khôi phục
//! phím (khi từ không phải tiếng Việt) chỉ là trả lại dãy phím thô.

use crate::english::EnglishWords;
use crate::macros::MacroTable;
use crate::method::{InputMethod, KeyMap, Role, classify, is_mark_char, mark_key, standalone_w, tone_key};
use crate::spell::{is_valid_prefix, is_valid_word, split, tone_position};
use crate::types::{Letter, Mark, Tone, compose, decompose, mark_fits};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub method: InputMethod,
    /// Đặt dấu kiểu mới: oà, uý (thay cho òa, úy).
    pub modern_tone: bool,
    /// Kiểm tra chính tả: không bỏ dấu vào từ sai chính tả, và trả lại phím gốc
    /// khi gõ xong một từ không phải tiếng Việt (vd: "windows", "text").
    pub spell_check: bool,
    /// Gõ đúng một từ tiếng Anh trong danh sách (test, error...) thì giữ tiếng Anh khi kết thúc từ.
    /// Độc lập với `spell_check`.
    pub english_words: bool,
    /// Gõ dấu tự do: phím dấu áp được cho nguyên âm ở bất kỳ đâu trong từ ("tiengs" → tiếng).
    /// Tắt: dấu phụ (mũ, móc, trăng, đ) chỉ áp cho chữ vừa gõ ngay trước; dấu thanh vẫn tự do.
    pub free_marking: bool,
    /// Bảng phím cho kiểu gõ tự định nghĩa.
    pub keymap: Option<std::sync::Arc<KeyMap>>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            method: InputMethod::Telex,
            modern_tone: true,
            spell_check: true,
            english_words: true,
            free_marking: true,
            keymap: None,
        }
    }
}

impl Options {
    fn km(&self) -> Option<&KeyMap> {
        self.keymap.as_deref()
    }
}

#[derive(Clone, Debug, Default)]
struct Word {
    letters: Vec<Letter>,
    tone: Tone,
    /// Đã gõ lặp phím để bỏ dấu (vd "ss", "aaa"): các phím sau được giữ nguyên.
    literal: bool,
    /// Từ không thể là tiếng Việt: hiển thị đúng dãy phím đã gõ.
    raw: bool,
    /// Có ít nhất một phím đã được biến đổi.
    transformed: bool,
    /// Số phím gõ tiếp sau lần bấm lặp để bỏ dấu.
    keys_after_undo: usize,
    /// Chữ được tạo thẳng bởi một phím (Telex: w → ư, [ → ơ, ] → ư): (vị trí, phím viết thường).
    /// Bấm lặp phím đó thì trả về đúng ký tự đã gõ ("ww" → "w", "[[" → "["), không còn "uw".
    created: Option<(usize, char)>,
}

/// Kết quả tính lại một từ từ dãy phím.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transformed {
    pub text: String,
    pub raw_fallback: bool,
    pub literal: bool,
    pub transformed: bool,
    pub has_vowel: bool,
    pub valid_word: bool,
    /// Số phím gõ tiếp sau lần bấm lặp để bỏ dấu (0 = phím bỏ dấu là phím cuối).
    pub keys_after_undo: usize,
}

impl Transformed {
    /// Gõ xong từ (gặp dấu cách...) thì có nên trả lại nguyên dãy phím không? (Chưa xét danh sách
    /// từ tiếng Anh — xem `Engine::end_word`.)
    /// - Có: đã biến đổi mà ra từ không phải tiếng Việt (windows, user...).
    /// - Không: người dùng đã bấm lặp để bỏ dấu ("ass" → "as", "tesst" → "test") — giữ đúng ý họ.
    pub fn should_restore(&self) -> bool {
        !self.raw_fallback && self.has_vowel && !self.valid_word && !self.literal && self.transformed
    }
}

/// Tính chữ hiển thị cho một dãy phím.
pub fn transform(keys: &[char], opt: &Options) -> Transformed {
    let mut w = Word::default();
    for &k in keys {
        if w.raw {
            continue;
        }
        if w.literal {
            w.letters.push(Letter::plain(k));
            w.keys_after_undo += 1;
            continue;
        }
        let before = w.clone();
        let applied = apply(&mut w, k, opt);
        normalize_uo(&mut w.letters);
        if applied {
            w.transformed = true;
        }
        if w.literal {
            // Người dùng vừa bấm lặp để bỏ dấu: giữ nguyên ý định, không kiểm tra chính tả nữa.
            continue;
        }
        if opt.spell_check && !is_valid_prefix(&w.letters, w.tone) {
            if applied {
                // Thử coi phím này là chữ thường.
                w = before;
                w.letters.push(Letter::plain(k));
                if is_valid_prefix(&w.letters, w.tone) {
                    continue;
                }
            }
            w.raw = true;
        }
    }
    let raw_text: String = keys.iter().collect();
    let has_vowel = !split(&w.letters).nucleus.is_empty();
    let valid_word = is_valid_word(&w.letters, w.tone);
    let text = if w.raw { raw_text } else { render(&w.letters, w.tone, opt.modern_tone) };
    Transformed {
        text,
        raw_fallback: w.raw,
        literal: w.literal,
        transformed: w.transformed,
        has_vowel,
        valid_word,
        keys_after_undo: w.keys_after_undo,
    }
}

/// Dựng chuỗi Unicode từ chữ cái + dấu thanh.
pub fn render(letters: &[Letter], tone: Tone, modern: bool) -> String {
    let pos = if tone == Tone::None { None } else { tone_position(letters, modern) };
    letters
        .iter()
        .enumerate()
        .map(|(i, l)| compose(*l, if Some(i) == pos { tone } else { Tone::None }))
        .collect()
}

/// "uơ" / "ươ": chữ u có móc hay không tuỳ phía sau. Có phụ âm cuối hoặc nguyên âm theo sau
/// (thương, người, rượu) → "ươ"; đứng cuối từ (thuở, huơ) → "uơ".
fn normalize_uo(letters: &mut [Letter]) {
    let p = split(letters);
    let nr = p.nucleus.clone();
    for i in nr.start..nr.end.saturating_sub(1) {
        if letters[i].base == 'u' && letters[i + 1].base == 'o' && letters[i + 1].mark == Mark::Horn {
            let followed = i + 2 < nr.end || !p.final_.is_empty();
            letters[i].mark = if followed { Mark::Horn } else { Mark::None };
        }
    }
}

fn nucleus_range(w: &Word) -> std::ops::Range<usize> {
    split(&w.letters).nucleus
}

/// Áp dụng một phím. Trả về `true` nếu phím được dùng như dấu (không chỉ là chữ thường).
fn apply(w: &mut Word, key: char, opt: &Options) -> bool {
    let upper = key.is_uppercase();
    let push = |w: &mut Word| {
        w.letters.push(Letter::plain(key));
        false
    };
    let undo = |w: &mut Word| {
        w.letters.push(Letter::plain(key));
        w.literal = true;
        true
    };
    let last = w.letters.len().wrapping_sub(1);
    match classify(opt.method, opt.km(), key) {
        Role::Letter => push(w),
        Role::Insert(l) => {
            let k = key.to_ascii_lowercase();
            if let Some((i, ck)) = w.created {
                if i + 1 == w.letters.len() && ck == k {
                    // "[[" → "[": thay chữ vừa tạo bằng chính phím đã gõ.
                    w.letters[i] = Letter::plain(key);
                    w.created = None;
                    w.literal = true;
                    return true;
                }
            }
            w.letters.push(Letter { upper: l.upper || upper, ..l });
            w.created = Some((w.letters.len() - 1, k));
            true
        }
        Role::Tone(t) => {
            if nucleus_range(w).is_empty() {
                return push(w);
            }
            if w.tone == t {
                w.tone = Tone::None;
                return undo(w);
            }
            w.tone = t;
            true
        }
        Role::RemoveTone => {
            if w.tone != Tone::None {
                w.tone = Tone::None;
                true
            } else {
                push(w)
            }
        }
        Role::Stroke if !opt.free_marking && w.letters.len() != 1 => push(w),
        Role::Stroke => match w.letters.first().map(|l| (l.base, l.mark)) {
            Some(('d', Mark::None)) => {
                w.letters[0].mark = Mark::Stroke;
                true
            }
            Some(('d', Mark::Stroke)) => {
                w.letters[0].mark = Mark::None;
                undo(w)
            }
            _ => push(w),
        },
        Role::Hat(target) => {
            let nr = nucleus_range(w);
            match w.letters[nr.clone()].iter().rposition(|l| l.base == target) {
                Some(i) if !opt.free_marking && nr.start + i != last => push(w),
                Some(i) => {
                    let i = nr.start + i;
                    if w.letters[i].mark == Mark::Hat {
                        w.letters[i].mark = Mark::None;
                        undo(w)
                    } else {
                        w.letters[i].mark = Mark::Hat;
                        true
                    }
                }
                None => push(w),
            }
        }
        Role::HatAny => apply_mark(w, key, opt, Mark::Hat, &['a', 'e', 'o']),
        Role::Breve => apply_mark(w, key, opt, Mark::Breve, &['a']),
        Role::Horn | Role::HornOrBreve => {
            let nr = nucleus_range(w);
            let nuc = &w.letters[nr.clone()];
            // "uo" → "ươ" (cả hai cùng có móc).
            let uo = nuc.windows(2).position(|p| p[0].base == 'u' && p[1].base == 'o');
            if let Some(i) = uo.filter(|&i| opt.free_marking || nr.start + i + 1 == last) {
                let (a, b) = (nr.start + i, nr.start + i + 1);
                if w.letters[b].mark == Mark::Horn {
                    w.letters[a].mark = Mark::None;
                    w.letters[b].mark = Mark::None;
                    return undo(w);
                }
                w.letters[a].mark = Mark::Horn;
                w.letters[b].mark = Mark::Horn;
                return true;
            }
            if nuc.is_empty() {
                // Telex: w đứng riêng thành ư (trừ Telex đơn giản; kiểu tự định nghĩa: phím Telex-W).
                if standalone_w(opt.method, opt.km(), key) {
                    w.letters.push(Letter { base: 'u', mark: Mark::Horn, upper });
                    w.created = Some((w.letters.len() - 1, key.to_ascii_lowercase()));
                    return true;
                }
                return push(w);
            }
            let targets: &[char] = if matches!(classify(opt.method, opt.km(), key), Role::Horn) {
                &['u', 'o']
            } else {
                &['u', 'o', 'a']
            };
            apply_horn(w, key, opt, targets)
        }
    }
}

/// Dấu móc/trăng: thử từng nguyên âm phù hợp, chọn cách cho ra vần hợp lệ.
fn apply_horn(w: &mut Word, key: char, opt: &Options, targets: &[char]) -> bool {
    let nr = nucleus_range(w);
    // Bấm lặp lại khi đã có dấu → bỏ dấu.
    if let Some(i) = w.letters[nr.clone()]
        .iter()
        .position(|l| matches!(l.mark, Mark::Horn | Mark::Breve) && targets.contains(&l.base))
    {
        let idx = nr.start + i;
        if w.created == Some((idx, key.to_ascii_lowercase())) {
            // "ww" → "w": chữ ư do chính phím w tạo ra → trả lại chữ w (giữ hoa/thường của lần gõ đầu).
            let up = w.letters[idx].upper;
            w.letters[idx] = Letter { base: 'w', mark: Mark::None, upper: up };
            w.created = None;
            w.literal = true;
            return true;
        }
        w.letters[idx].mark = Mark::None;
        w.letters.push(Letter::plain(key));
        w.literal = true;
        return true;
    }
    let mut candidates = Vec::new();
    let last = w.letters.len().wrapping_sub(1);
    for &t in targets {
        for i in nr.clone() {
            if w.letters[i].base == t && w.letters[i].mark == Mark::None && (opt.free_marking || i == last) {
                candidates.push(i);
            }
        }
    }
    let mark_for = |base: char| if base == 'a' { Mark::Breve } else { Mark::Horn };
    let mut chosen = None;
    for &i in &candidates {
        let mut trial = w.letters.clone();
        trial[i].mark = mark_for(trial[i].base);
        if is_valid_prefix(&trial, w.tone) {
            chosen = Some(i);
            break;
        }
    }
    if chosen.is_none() && !opt.spell_check {
        chosen = candidates.first().copied();
    }
    match chosen {
        Some(i) => {
            w.letters[i].mark = mark_for(w.letters[i].base);
            true
        }
        None => {
            w.letters.push(Letter::plain(key));
            false
        }
    }
}

/// VNI 6 / 8: đặt dấu mũ hoặc trăng cho nguyên âm phù hợp (ưu tiên chữ sau cùng).
fn apply_mark(w: &mut Word, key: char, opt: &Options, mark: Mark, targets: &[char]) -> bool {
    let nr = nucleus_range(w);
    if let Some(i) = w.letters[nr.clone()].iter().position(|l| l.mark == mark) {
        w.letters[nr.start + i].mark = Mark::None;
        w.letters.push(Letter::plain(key));
        w.literal = true;
        return true;
    }
    let candidates: Vec<usize> = nr
        .clone()
        .rev()
        .filter(|&i| targets.contains(&w.letters[i].base) && mark_fits(w.letters[i].base, mark))
        .filter(|&i| opt.free_marking || i + 1 == w.letters.len())
        .collect();
    let chosen = candidates
        .iter()
        .copied()
        .find(|&i| {
            let mut trial = w.letters.clone();
            trial[i].mark = mark;
            is_valid_prefix(&trial, w.tone)
        })
        .or(if opt.spell_check { None } else { candidates.first().copied() });
    match chosen {
        Some(i) => {
            w.letters[i].mark = mark;
            true
        }
        None => {
            w.letters.push(Letter::plain(key));
            false
        }
    }
}

/// Mã hoá ngược một từ đã hiển thị thành dãy phím (dùng sau khi người dùng xoá lùi).
pub fn encode(text: &str, method: InputMethod) -> Option<Vec<char>> {
    encode_with(text, method, None)
}

/// Như `encode`, có bảng phím cho kiểu gõ tự định nghĩa.
pub fn encode_with(text: &str, method: InputMethod, keymap: Option<&KeyMap>) -> Option<Vec<char>> {
    let mut keys = Vec::new();
    let mut tone = Tone::None;
    for c in text.chars() {
        let (l, t) = decompose(c)?;
        if t != Tone::None {
            tone = t;
        }
        keys.push(if l.upper { l.base.to_ascii_uppercase() } else { l.base });
        match mark_key(method, keymap, &l) {
            None => {}
            Some(Some(k)) => keys.push(k),
            Some(None) => return None,
        }
    }
    if tone != Tone::None {
        keys.push(tone_key(method, keymap, tone)?);
    }
    Some(keys)
}

/// Một phép sửa văn bản: xoá `delete` ở cuối rồi chèn `insert`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Edit {
    pub delete: String,
    pub insert: String,
}

impl Edit {
    pub fn is_empty(&self) -> bool {
        self.delete.is_empty() && self.insert.is_empty()
    }
}

/// So hai chuỗi, giữ phần đầu chung.
pub fn diff(old: &str, new: &str) -> Edit {
    let common = old
        .char_indices()
        .zip(new.chars())
        .take_while(|((_, a), b)| a == b)
        .count();
    let split_at = |s: &str| s.char_indices().nth(common).map(|(i, _)| i).unwrap_or(s.len());
    Edit { delete: old[split_at(old)..].to_string(), insert: new[split_at(new)..].to_string() }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Cho phím đi qua bình thường.
    Pass,
    /// Chặn phím, thay bằng phép sửa này.
    Replace(Edit),
}

/// Theo dõi ranh giới câu để tự viết hoa chữ đầu câu.
#[derive(Debug, Clone, Default)]
struct Sentence {
    /// Chữ cái gõ tiếp theo là đầu câu.
    cap_next: bool,
    /// Vừa gặp dấu kết thúc câu (. ? !), chờ dấu cách / xuống dòng.
    pending_end: bool,
    /// Số dấu chấm liền nhau ("..." không phải kết thúc câu).
    dots: u8,
    /// Từ ngay trước dấu ngắt (viết thường) — để nhận ra chữ viết tắt "v.v.", "TP.".
    prev_word: String,
}

/// Viết tắt thường gặp: dấu chấm sau chúng không kết thúc câu.
const ABBREVIATIONS: [&str; 22] = [
    "tp", "ts", "ths", "pgs", "gs", "bs", "ks", "cn", "st", "tt", "ubnd", "mr", "mrs", "ms", "dr", "vs", "etc",
    "eg", "ie", "no", "jr", "sr",
];

impl Sentence {
    fn word_started(&mut self) {
        self.cap_next = false;
        self.pending_end = false;
        self.dots = 0;
    }

    fn clear(&mut self) {
        *self = Sentence::default();
    }

    fn separator(&mut self, c: char) {
        match c {
            '.' => {
                self.dots = self.dots.saturating_add(1);
                let w = self.prev_word.as_str();
                // "v.v.", "Nguyễn V. A." (một chữ cái) và các chữ viết tắt quen thuộc không kết thúc câu.
                let abbreviation = self.dots == 1 && (w.chars().count() == 1 || ABBREVIATIONS.contains(&w));
                self.pending_end = self.dots == 1 && !abbreviation;
            }
            '?' | '!' => {
                self.pending_end = true;
                self.dots = 0;
            }
            // Đóng ngoặc / nháy sau dấu kết thúc câu: vẫn chờ dấu cách.
            '"' | '\'' | ')' | ']' | '\u{201D}' | '\u{2019}' => {}
            ' ' => {
                if self.pending_end {
                    self.cap_next = true;
                }
                self.pending_end = false;
                self.dots = 0;
            }
            '\n' => {
                self.cap_next = true;
                self.pending_end = false;
                self.dots = 0;
            }
            _ => {
                self.cap_next = false;
                self.pending_end = false;
                self.dots = 0;
            }
        }
        self.prev_word.clear();
    }
}

/// Trạng thái gõ của một từ, dùng bởi lớp giao tiếp hệ điều hành.
#[derive(Debug, Clone, Default)]
pub struct Engine {
    pub opts: Options,
    /// `false` = chế độ tiếng Anh (vẫn theo dõi từ để gõ tắt nếu được bật).
    pub vietnamese: bool,
    /// Từ tiếng Anh giữ nguyên khi gõ đúng (có sẵn + của người dùng).
    pub english: EnglishWords,
    /// Tự viết hoa chữ đầu câu (sau ". " "? " "! " hoặc xuống dòng).
    pub auto_capitalize: bool,
    sentence: Sentence,
    keys: Vec<char>,
    out: String,
    /// Sau khi xoá lùi không dựng lại được dãy phím: chỉ nối thêm phím thô.
    literal_only: bool,
    last: Option<Transformed>,
    /// Các từ vừa kết thúc (mỗi dấu ngắt từ một mục): xoá lùi qua dấu ngắt thì lấy lại từ trước để gõ tiếp.
    history: Vec<WordState>,
    /// Dạng còn lại của từ đang gõ (sau khi bấm phím khôi phục).
    alt: Option<String>,
    /// Dấu ngắt vừa gõ (nếu chưa gõ gì thêm) — phím khôi phục chỉ sửa từ trước khi đó là một dấu cách.
    last_sep: Option<char>,
}

/// Trạng thái đầy đủ của một từ, để khôi phục khi người dùng xoá lùi qua dấu cách.
#[derive(Debug, Clone, Default)]
struct WordState {
    keys: Vec<char>,
    out: String,
    literal_only: bool,
    last: Option<Transformed>,
    /// Dạng còn lại của từ để phím khôi phục đổi qua lại (vd "ít" ⇄ "its", "test" ⇄ "tét").
    alt: Option<String>,
}

/// Giới hạn số từ nhớ được để xoá lùi về.
const HISTORY_MAX: usize = 32;

impl Engine {
    pub fn new(opts: Options) -> Engine {
        Engine { opts, vietnamese: true, english: EnglishWords::builtin(), ..Default::default() }
    }

    /// Phím này có thuộc về từ đang gõ không? (Ngược lại là dấu ngắt từ.)
    pub fn is_word_char(&self, c: char) -> bool {
        if c.is_ascii_alphabetic() {
            return true;
        }
        if !self.vietnamese {
            return false;
        }
        // Phím dấu không phải chữ (số VNI, dấu câu VIQR...) chỉ thuộc về từ khi đã có chữ trước đó;
        // riêng Telex [ ] luôn là chữ (ơ ư).
        is_mark_char(self.opts.method, self.opts.km(), c)
            && (!self.keys.is_empty() || matches!(self.opts.method, InputMethod::Telex | InputMethod::Custom))
    }

    /// Chữ đang hiển thị của từ hiện tại.
    pub fn current(&self) -> &str {
        &self.out
    }

    pub fn key(&mut self, typed: char) -> KeyOutcome {
        // Chữ đầu tiên của câu: tự viết hoa.
        let c = if self.auto_capitalize && self.sentence.cap_next && self.keys.is_empty() && typed.is_ascii_lowercase() {
            typed.to_ascii_uppercase()
        } else {
            typed
        };
        self.sentence.word_started();
        self.last_sep = None;
        self.alt = None;
        self.keys.push(c);
        let new = if !self.vietnamese || self.literal_only {
            let mut s = self.out.clone();
            s.push(c);
            s
        } else {
            let t = transform(&self.keys, &self.opts);
            let text = t.text.clone();
            self.last = Some(t);
            text
        };
        let edit = diff(&self.out, &new);
        self.out = new;
        if edit.delete.is_empty() && edit.insert.chars().eq(std::iter::once(typed)) {
            KeyOutcome::Pass
        } else {
            KeyOutcome::Replace(edit)
        }
    }

    /// Người dùng bấm Backspace (phím được cho đi qua; engine chỉ cập nhật trạng thái).
    pub fn backspace(&mut self) {
        // Xoá lùi: không còn chắc đang ở đầu câu.
        self.sentence.word_started();
        self.last_sep = None;
        self.alt = None;
        if self.out.is_empty() {
            // Từ hiện tại đã rỗng: phím xoá này xoá dấu ngắt từ (dấu cách, dấu câu) trước đó →
            // con trỏ quay về cuối từ trước: lấy lại trạng thái của từ đó để gõ tiếp / sửa dấu.
            match self.history.pop() {
                Some(w) => self.restore_word(w),
                None => self.reset_word(),
            }
            return;
        }
        self.out.pop();
        if self.out.is_empty() {
            self.reset_word();
            return;
        }
        let fallback = self.literal_only
            || !self.vietnamese
            || self.last.as_ref().is_some_and(|t| t.raw_fallback || t.literal);
        let rebuilt = if fallback { None } else { encode_with(&self.out, self.opts.method, self.opts.km()) };
        match rebuilt {
            Some(keys) if transform(&keys, &self.opts).text == self.out => {
                self.keys = keys;
                self.literal_only = false;
            }
            _ => {
                self.keys = self.out.chars().collect();
                self.literal_only = true;
            }
        }
    }

    /// Kết thúc từ (gặp dấu cách, dấu câu, Enter...). Trả về phép sửa nếu cần
    /// khôi phục phím gốc hoặc thay bằng gõ tắt. Sau đó bắt đầu từ mới.
    pub fn end_word(&mut self, macros: Option<&MacroTable>) -> Option<Edit> {
        let mut text = self.out.clone();
        if self.vietnamese && !self.literal_only {
            if let Some(t) = &self.last {
                let raw: String = self.keys.iter().collect();
                // Dãy phím trùng đúng một từ tiếng Anh trong danh sách (test, error, password...) →
                // giữ tiếng Anh. Phím lặp để bỏ dấu mà không thành từ tiếng Anh ("tesst") vẫn ra "test".
                let english = self.opts.english_words && t.text != raw && self.english.contains(&raw);
                let invalid = self.opts.spell_check && t.should_restore();
                if english || invalid {
                    text = raw;
                }
            }
        }
        if let Some(m) = macros {
            if let Some(exp) = m.expand(&text) {
                text = exp;
            }
        }
        let edit = diff(&self.out, &text);
        // Nhớ từ vừa xong (đúng như đang hiển thị) để có thể xoá lùi về sửa tiếp.
        self.sentence.prev_word = text.to_lowercase();
        let snapshot = if text == self.out {
            WordState {
                keys: std::mem::take(&mut self.keys),
                out: std::mem::take(&mut self.out),
                literal_only: self.literal_only,
                last: self.last.take(),
                alt: self.alt.take(),
            }
        } else {
            // Đã đổi thành từ khác (trả phím gốc, gõ tắt): gõ tiếp chỉ nối thêm chữ thường.
            // Nhớ dạng cũ để phím khôi phục đổi lại được ("test" → "tét", "việt nam" → "vn").
            let alt = Some(self.out.clone());
            WordState { keys: text.chars().collect(), out: text, literal_only: true, last: None, alt }
        };
        self.reset_word();
        if self.history.len() >= HISTORY_MAX {
            self.history.remove(0);
        }
        self.history.push(snapshot);
        if edit.is_empty() { None } else { Some(edit) }
    }

    /// Kết thúc từ bằng một dấu ngắt "cứng" (Enter, Tab): sau đó không xoá lùi về từ trước được
    /// (Enter có thể đã gửi tin nhắn, Tab chuyển ô...).
    pub fn forget_history(&mut self) {
        self.history.clear();
    }

    /// Bỏ từ đang gõ và quên các từ trước (di chuyển con trỏ, bấm chuột, đổi cửa sổ...).
    pub fn reset(&mut self) {
        self.reset_word();
        self.history.clear();
        self.sentence.clear();
        self.last_sep = None;
    }

    /// Báo dấu ngắt từ vừa gõ (gọi sau `end_word`): ' ', '.', '?', '!', '\n' (Enter)... — để biết đầu câu.
    pub fn separator(&mut self, c: char) {
        self.sentence.separator(c);
        self.last_sep = Some(c);
    }

    /// Vừa kết thúc một câu (". " "? " "! "): chữ gõ tiếp theo là đầu câu mới.
    pub fn at_sentence_start(&self) -> bool {
        self.sentence.cap_next
    }

    /// Bỏ từ đang gõ, vẫn nhớ các từ trước.
    fn reset_word(&mut self) {
        self.keys.clear();
        self.out.clear();
        self.literal_only = false;
        self.last = None;
        self.alt = None;
    }

    fn restore_word(&mut self, w: WordState) {
        self.keys = w.keys;
        self.out = w.out;
        self.literal_only = w.literal_only;
        self.last = w.last;
        self.alt = w.alt;
    }

    /// Phím khôi phục: đổi từ đang gõ (hoặc từ vừa gõ xong, ngay sau một dấu cách) qua lại giữa
    /// dạng tiếng Việt và đúng các phím đã bấm: "its" → "ít" → khôi phục → "its"; "test" (đã giữ
    /// tiếng Anh) → khôi phục → "tét". Trả về phép sửa, hoặc `None` nếu không có gì để đổi.
    pub fn restore(&mut self) -> Option<Edit> {
        if !self.out.is_empty() {
            let target = match self.alt.take() {
                Some(a) => a,
                None => {
                    let raw: String = self.keys.iter().collect();
                    if raw == self.out {
                        return None;
                    }
                    raw
                }
            };
            let edit = diff(&self.out, &target);
            self.alt = Some(std::mem::replace(&mut self.out, target));
            self.keys = self.out.chars().collect();
            self.literal_only = true;
            self.last = None;
            return Some(edit);
        }
        if self.last_sep != Some(' ') {
            return None;
        }
        let w = self.history.last_mut()?;
        if w.out.is_empty() {
            return None;
        }
        let target = match w.alt.take() {
            Some(a) => a,
            None => {
                let raw: String = w.keys.iter().collect();
                if raw == w.out {
                    return None;
                }
                raw
            }
        };
        let edit = Edit { delete: format!("{} ", w.out), insert: format!("{target} ") };
        w.alt = Some(std::mem::replace(&mut w.out, target));
        w.keys = w.out.chars().collect();
        w.literal_only = true;
        w.last = None;
        Some(edit)
    }
}
