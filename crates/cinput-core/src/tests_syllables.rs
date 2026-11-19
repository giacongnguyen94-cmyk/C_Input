//! Kiểm thử trên toàn bộ âm tiết tiếng Việt sinh tự động:
//! phụ âm đầu × vần × phụ âm cuối × 6 thanh, với Telex/VNI và đặt dấu kiểu cũ/mới.

use crate::engine::{encode, render, transform};
use crate::spell::is_valid_word;
use crate::types::{Letter, Mark, Tone, decompose};
use crate::{InputMethod, Options};

const INITIALS: [&str; 27] = [
    "", "b", "c", "ch", "d", "đ", "g", "gh", "gi", "h", "k", "kh", "l", "m", "n", "ng", "ngh", "nh", "p",
    "ph", "qu", "r", "s", "t", "th", "tr", "v",
];
const FINALS: [&str; 9] = ["", "c", "ch", "m", "n", "ng", "nh", "p", "t"];
const NUCLEI: [&str; 55] = [
    "a", "ă", "â", "e", "ê", "i", "o", "ô", "ơ", "u", "ư", "y", "ai", "ao", "au", "ay", "âu", "ây", "eo", "êu",
    "ia", "iu", "iê", "yê", "oa", "oă", "oe", "oi", "ôi", "ơi", "oo", "ua", "uâ", "uê", "ui", "uô", "uơ", "uy",
    "ưa", "ưi", "ươ", "ưu", "iêu", "yêu", "oai", "oay", "oeo", "uây", "uôi", "ươi", "ươu", "uyê", "uya", "uyu",
    "oao",
];
const TONES: [Tone; 6] = [Tone::None, Tone::Acute, Tone::Grave, Tone::Hook, Tone::Tilde, Tone::Dot];

fn letters_of(s: &str) -> Vec<Letter> {
    s.chars().map(|c| decompose(c).unwrap().0).collect()
}

/// Mọi âm tiết hợp lệ theo luật chính tả của engine (chữ cái + dấu thanh).
fn all_syllables() -> Vec<(Vec<Letter>, Tone)> {
    let mut out = Vec::new();
    for init in INITIALS {
        for nuc in NUCLEI {
            // Loại các ghép vô nghĩa: "gi" + i/y..., "qu" + u/ư/o...
            if init == "gi" && (nuc.starts_with('i') || nuc.starts_with('y')) {
                continue;
            }
            if init == "qu" && (nuc.starts_with(['u', 'ư', 'o'])) {
                continue;
            }
            // "y" đứng sau phụ âm đầu thay cho "i" chỉ hợp lệ trong vài vần (yê/yêu sau phụ âm là sai chính tả).
            if !init.is_empty() && init != "qu" && nuc.starts_with('y') {
                continue;
            }
            for fin in FINALS {
                let letters = letters_of(&format!("{init}{nuc}{fin}"));
                for tone in TONES {
                    if is_valid_word(&letters, tone) {
                        out.push((letters.clone(), tone));
                    }
                }
            }
        }
    }
    out
}

/// Gõ dấu tự do: gõ hết chữ gốc, rồi dấu thanh, rồi các dấu phụ.
fn free_order_keys(word: &str, method: InputMethod) -> Vec<char> {
    let mut base = Vec::new();
    let mut marks = Vec::new();
    let mut tone = Tone::None;
    let mut horn_done = false;
    for c in word.chars() {
        let (l, t) = decompose(c).unwrap();
        if t != Tone::None {
            tone = t;
        }
        base.push(if l.upper { l.base.to_ascii_uppercase() } else { l.base });
        let telex = method.is_telex();
        match l.mark {
            Mark::Hat => marks.push(if telex { l.base } else { '6' }),
            Mark::Horn | Mark::Breve if !horn_done => {
                marks.push(if telex { 'w' } else if l.mark == Mark::Horn { '7' } else { '8' });
                horn_done = l.mark == Mark::Horn; // "ươ" chỉ cần một phím w / 7
            }
            Mark::Stroke => marks.push(if telex { 'd' } else { '9' }),
            _ => {}
        }
    }
    let mut keys = base;
    if let Some(k) = crate::method::tone_key(method, tone) {
        keys.push(k);
    }
    keys.extend(marks);
    keys
}

#[test]
fn every_syllable_types_correctly() {
    let sylls = all_syllables();
    assert!(sylls.len() > 15_000, "chỉ sinh được {} âm tiết", sylls.len());
    let mut failures = Vec::new();
    let mut checked = 0;
    for modern in [true, false] {
        for method in [InputMethod::Telex, InputMethod::Vni] {
            let opt = Options { method, modern_tone: modern, spell_check: true, english_words: true };
            for (letters, tone) in &sylls {
                let word = render(letters, *tone, modern);
                // Telex: "oo" luôn thành "ô" (xoong phải gõ "xooong") — giới hạn của kiểu gõ, như UniKey.
                let plain_oo = letters
                    .windows(2)
                    .any(|p| p[0].base == 'o' && p[1].base == 'o' && p[0].mark == Mark::None && p[1].mark == Mark::None);
                if method.is_telex() && plain_oo {
                    continue;
                }
                for case in 0..3 {
                    let w = match case {
                        0 => word.clone(),
                        1 => {
                            let mut cs = word.chars();
                            let f = cs.next().unwrap();
                            f.to_uppercase().chain(cs).collect::<String>()
                        }
                        _ => word.to_uppercase(),
                    };
                    let mut variants = vec![("chuẩn", encode(&w, method).unwrap())];
                    variants.push(("tự do", free_order_keys(&w, method)));
                    for (kind, keys) in variants {
                        checked += 1;
                        let t = transform(&keys, &opt);
                        let restored = t.should_restore();
                        if t.text != w || t.raw_fallback || restored {
                            failures.push(format!(
                                "{method:?} {} {kind}: gõ {:?} → {:?}{}{} (cần {w})",
                                if modern { "mới" } else { "cũ" },
                                keys.iter().collect::<String>(),
                                t.text,
                                if t.raw_fallback { " [trả phím thô]" } else { "" },
                                if restored { " [sẽ bị khôi phục]" } else { "" },
                            ));
                        }
                    }
                }
            }
        }
    }
    if !failures.is_empty() {
        let sample: Vec<_> = failures.iter().take(60).collect();
        panic!("{}/{} ca sai. Ví dụ:\n{}", failures.len(), checked, sample.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    }
    eprintln!("đã kiểm tra {checked} ca trên {} âm tiết", sylls.len());
}

/// Từ tiếng Anh thông dụng: in ra những từ bị engine biến đổi (để theo dõi, không bắt lỗi).
#[test]
fn english_words_report() {
    let words = "the be to of and a in that have it for not on with he as you do at this but his by from they we \
        say her she or an will my one all would there their what so up out if about who get which go me when make \
        can like time no just him know take people into year your good some could them see other than then now look \
        only come its over think also back after use two how our work first well way even new want because any these \
        give day most us is was are has had were said did windows text user email file folder google facebook \
        string class public return print code server data error update login password chrome excel word";
    let opt = Options { method: InputMethod::Telex, modern_tone: true, spell_check: true, english_words: true };
    let mut changed = Vec::new();
    for w in words.split_whitespace() {
        // Đi đúng đường của bộ gõ: từng phím qua Engine rồi kết thúc từ (có danh sách tiếng Anh).
        let mut e = crate::Engine::new(opt.clone());
        for c in w.chars() {
            let _ = e.key(c);
        }
        let mut final_text = e.current().to_string();
        if let Some(edit) = e.end_word(None) {
            for _ in edit.delete.chars() {
                final_text.pop();
            }
            final_text.push_str(&edit.insert);
        }
        if final_text != w {
            changed.push(format!("{w}→{final_text}"));
        }
    }
    eprintln!("{} từ bị đổi: {}", changed.len(), changed.join(", "));
}

/// Âm tiết hợp lệ về chính tả nhưng không phải từ tiếng Việt có thật — từ tiếng Anh được phép trùng cách gõ.
const NOT_REAL_VI: [&str; 15] = [
    "hẻ", "dổ", "dơn", "pải", "pẻ", "rôt", "lôp", "kêp", "mêt", "dêp", "dóe", "doé", "góe", "goé", "quêu",
];

/// Danh sách từ tiếng Anh không được "cướp" cách gõ chuẩn (dấu phụ ngay sau nguyên âm, dấu thanh cuối từ)
/// của âm tiết tiếng Việt có thật. Trùng với cách gõ dấu tự do (vd "did" ~ "đi", "data" ~ "dât") thì chấp nhận:
/// người gõ tiếng Việt gần như không gõ theo thứ tự đó.
#[test]
fn english_list_never_blocks_vietnamese() {
    let english = crate::EnglishWords::builtin();
    let opt = Options { method: InputMethod::Telex, modern_tone: true, spell_check: true, english_words: true };
    let mut clashes = std::collections::BTreeSet::new();
    let mut free_order = std::collections::BTreeSet::new();
    for (letters, tone) in all_syllables() {
        for modern in [true, false] {
            let word = render(&letters, tone, modern);
            let canonical: String = encode(&word, InputMethod::Telex).unwrap().iter().collect();
            let free: String = free_order_keys(&word, InputMethod::Telex).iter().collect();
            for (raw, is_canonical) in [(canonical, true), (free, false)] {
                let keys: Vec<char> = raw.chars().collect();
                // Gõ ra y hệt dãy phím (am, at, but...) thì danh sách không làm gì.
                // Vần "oo" (loop, root) vốn không gõ được bằng dãy phím trùng chữ: không phải trùng thật.
                if !english.contains(&raw) || transform(&keys, &opt).text == raw || word == raw {
                    continue;
                }
                if is_canonical && !NOT_REAL_VI.contains(&word.as_str()) {
                    clashes.insert(format!("{raw} (= {word})"));
                } else {
                    free_order.insert(format!("{raw}~{word}"));
                }
            }
        }
    }
    eprintln!("trùng chấp nhận được: {}", free_order.into_iter().collect::<Vec<_>>().join(", "));
    assert!(clashes.is_empty(), "{} từ trùng cách gõ chuẩn của âm tiết tiếng Việt, cần bỏ khỏi english_words.txt:
{}",
        clashes.len(), clashes.into_iter().collect::<Vec<_>>().join(", "));
}
