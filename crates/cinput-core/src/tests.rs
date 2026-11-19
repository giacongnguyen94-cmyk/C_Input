//! Kiểm thử gõ: mô phỏng một ô văn bản nhận phím qua Engine.

use crate::{Edit, Engine, InputMethod, KeyOutcome, MacroTable, Options};

/// Ô văn bản giả: áp dụng phép sửa của engine như lớp Windows sẽ làm.
struct Doc {
    text: String,
    engine: Engine,
    macros: Option<MacroTable>,
}

impl Doc {
    fn new(opts: Options) -> Doc {
        Doc { text: String::new(), engine: Engine::new(opts), macros: None }
    }

    fn apply(&mut self, e: &Edit) {
        for _ in e.delete.chars() {
            self.text.pop();
        }
        self.text.push_str(&e.insert);
    }

    fn type_str(&mut self, s: &str) -> &str {
        for c in s.chars() {
            if c == '\u{8}' {
                self.engine.backspace();
                self.text.pop();
            } else if c == '\u{1b}' {
                // Phím khôi phục (Esc).
                if let Some(e) = self.engine.restore() {
                    self.apply(&e);
                }
            } else if self.engine.is_word_char(c) {
                match self.engine.key(c) {
                    KeyOutcome::Pass => self.text.push(c),
                    KeyOutcome::Replace(e) => self.apply(&e),
                }
            } else {
                if let Some(e) = self.engine.end_word(self.macros.as_ref()) {
                    self.apply(&e);
                }
                self.engine.separator(c);
                self.text.push(c);
            }
        }
        if let Some(e) = self.engine.end_word(self.macros.as_ref()) {
            self.apply(&e);
        }
        &self.text
    }
}

fn opts(method: InputMethod, modern: bool) -> Options {
    Options { method, modern_tone: modern, spell_check: true, english_words: true, ..Default::default() }
}

fn check(method: InputMethod, modern: bool, cases: &[(&str, &str)]) {
    for (input, expected) in cases {
        let mut d = Doc::new(opts(method, modern));
        assert_eq!(d.type_str(input), *expected, "{method:?} gõ {input:?}");
    }
}

#[test]
fn telex_basic() {
    check(
        InputMethod::Telex,
        true,
        &[
            ("tieesng", "tiếng"),
            ("vieejt", "việt"),
            ("dduwowcj", "được"),
            ("nguwowif", "người"),
            ("quoocs", "quốc"),
            ("giuwax", "giữa"),
            ("gifn", "gìn"),
            ("khuyar", "khuỷa"),
            ("nghieeng", "nghiêng"),
            ("thuowr", "thuở"),
            ("Vieejt Nam", "Việt Nam"),
            ("DDaij hocj", "Đại học"),
            ("tw", "tư"),
            ("nhw", "như"),
            ("muwa", "mưa"),
            ("hoawcj", "hoặc"),
            ("gaauj", "gậu"),
            ("tooi", "tôi"),
            ("khoong", "không"),
            ("xin chaof", "xin chào"),
            ("cuwus", "cứu"),
            ("guwir", "gửi"),
            ("chuyeenj", "chuyện"),
            ("quyeets", "quyết"),
            ("giuwowngf", "giường"),
            ("uoongs", "uống"),
            ("yeeu", "yêu"),
            ("ddi", "đi"),
            ("[", "ơ"),
        ],
    );
}

#[test]
fn telex_free_marking() {
    check(
        InputMethod::Telex,
        true,
        &[
            ("tienges", "tiếng"),
            ("tiengse", "tiếng"),
            ("thuongw", "thương"),
            ("nguoiwf", "người"),
            ("vietje", "việt"),
            ("duongwf", "dường"),
            ("trangw", "trăng"),
            ("hoanf", "hoàn"),
            ("dod", "đo"),
        ],
    );
}

#[test]
fn tone_style() {
    check(InputMethod::Telex, true, &[("hoaf", "hoà"), ("khoer", "khoẻ"), ("thuys", "thuý")]);
    check(InputMethod::Telex, false, &[("hoaf", "hòa"), ("khoer", "khỏe"), ("thuys", "thúy")]);
    // Có phụ âm cuối thì kiểu nào cũng đặt ở nguyên âm sau.
    check(InputMethod::Telex, false, &[("toans", "toán"), ("huyfnh", "huỳnh")]);
}

#[test]
fn telex_undo() {
    check(
        InputMethod::Telex,
        true,
        &[
            ("ass", "as"),
            ("aaa", "aa"),
            ("ddd", "dd"),
            ("muww", "muw"),
            ("tooo", "too"),
            // Bỏ dấu bằng phím lặp rồi gõ tiếp: giữ đúng ý người dùng.
            ("tesst", "test"),
            ("texxt", "text"),
            ("Tesst Casse", "Test Case"),
        ],
    );
}

#[test]
fn english_dictionary() {
    check(
        InputMethod::Telex,
        true,
        &[
            // Gõ đúng một từ tiếng Anh thông dụng: giữ nguyên dù ra được âm tiết tiếng Việt hợp lệ.
            ("test", "test"),
            ("Test", "Test"),
            ("TEST", "TEST"),
            ("best", "best"),
            ("most", "most"),
            ("data", "data"),
            ("did", "did"),
            ("fast", "fast"),
            ("unit test", "unit test"),
            ("unit test passed", "unit test passed"),
            ("tests files using stopped copied", "tests files using stopped copied"),
            ("songs mays", "sóng máy"),
            // Người gõ tiếng Việt gõ dấu ở cuối: vẫn ra tiếng Việt.
            ("tets", "tét"),
            ("bets", "bét"),
            ("ddi", "đi"),
            ("its", "ít"),
            ("has", "há"),
            ("banhs tets", "bánh tét"),
        ],
    );
}

#[test]
fn auto_restore_english() {
    check(
        InputMethod::Telex,
        true,
        &[
            ("windows", "windows"),
            ("text", "text"),
            ("user", "user"),
            ("for", "for"),
            ("string", "string"),
            ("class", "class"),
            ("Microsoft", "Microsoft"),
            ("email", "email"),
            ("error", "error"),
            ("password", "password"),
            ("address", "address"),
            ("success", "success"),
        ],
    );
}

#[test]
fn simple_telex() {
    check(InputMethod::SimpleTelex, true, &[("w", "w"), ("tieesng", "tiếng"), ("[", "[")]);
}

#[test]
fn vni() {
    check(
        InputMethod::Vni,
        true,
        &[
            ("tie61ng", "tiếng"),
            ("vie65t", "việt"),
            ("d9u7o7c5", "được"),
            ("ngu7o72i", "người"),
            ("hoa2", "hoà"),
            ("ca8n", "căn"),
            ("tua6n", "tuân"),
            ("a11", "a1"),
            ("2024", "2024"),
            ("Vie65t Nam 2", "Việt Nam 2"),
        ],
    );
}

#[test]
fn backspace_keeps_state() {
    let mut d = Doc::new(opts(InputMethod::Telex, true));
    // Gõ "tieesng", xoá "g", gõ lại "g" rồi đổi dấu sang huyền.
    assert_eq!(d.type_str("tieesng\u{8}gf"), "tiềng");
    let mut d = Doc::new(opts(InputMethod::Telex, true));
    assert_eq!(d.type_str("hoaf\u{8}\u{8}oaf"), "hoà");
}

#[test]
fn backspace_into_previous_word() {
    let cases = [
        // Gõ từ, dấu cách, xoá dấu cách rồi gõ tiếp: từ trước được sửa tiếp như chưa từng kết thúc.
        ("tieeng \u{8}s", "tiếng"),
        ("tieeng \u{8}s ", "tiếng "),
        ("a \u{8}s", "á"),
        ("as \u{8}s", "as"),              // "á" + s = bỏ dấu, như khi gõ liền "ass"
        ("as \u{8}f", "à"),               // đổi dấu
        ("vieet nam \u{8}\u{8}\u{8}\u{8}\u{8}j", "việt"), // xoá cả từ sau rồi dấu cách
        ("vieet nam \u{8}\u{8}\u{8}\u{8}j", "viêt j"),     // chưa xoá dấu cách giữa hai từ: từ mới
        ("tieeng  \u{8}\u{8}s", "tiếng"),  // hai dấu cách
        ("tieeng, \u{8}\u{8}s", "tiếng"),  // dấu phẩy + cách
        ("Vieet \u{8}j", "Việt"),
        ("windows \u{8}", "windows"),
        ("test \u{8}s", "tests"),         // từ tiếng Anh đã giữ nguyên: gõ tiếp vẫn là chữ thường
    ];
    for (input, expected) in cases {
        let mut d = Doc::new(opts(InputMethod::Telex, true));
        assert_eq!(d.type_str(input), expected, "gõ {input:?}");
    }
}

#[test]
fn reset_forgets_previous_word() {
    // Bấm chuột / phím mũi tên giữa chừng: không được sửa nhầm một từ ở chỗ khác.
    let mut d = Doc::new(opts(InputMethod::Telex, true));
    d.type_str("tieeng ");
    d.engine.reset();
    assert_eq!(d.type_str("\u{8}s"), "tiêngs");
}

#[test]
fn encode_roundtrip() {
    let words = "tiếng việt được người quốc giữa gìn khuya nghiêng thuở hoà khoẻ thuý toán huỳnh \
                 chuyện quyết giường uống yêu đi mưa hoặc cứu gửi trăng Đại Học Việt";
    for method in [InputMethod::Telex, InputMethod::Vni] {
        let o = opts(method, true);
        for w in words.split_whitespace() {
            let keys = crate::engine::encode(w, method).unwrap();
            assert_eq!(crate::transform(&keys, &o).text, w, "{method:?} {w}");
        }
    }
}

#[test]
fn macros() {
    let mut d = Doc::new(opts(InputMethod::Telex, true));
    d.macros = Some(MacroTable::parse(";comment\nvn:việt nam\nko:không\n", true));
    assert_eq!(d.type_str("vn ko VN Vn"), "việt nam không VIỆT NAM Việt nam");
}

#[test]
fn english_dictionary_old_tone_style() {
    check(InputMethod::Telex, false, &[("test", "test"), ("tesst", "test"), ("test tesst", "test test")]);
}

#[test]
fn english_list_independent_of_spell_check() {
    // Tắt kiểm tra chính tả vẫn giữ được "test"; tắt danh sách thì ra "tét".
    let o = Options { method: InputMethod::Telex, modern_tone: false, spell_check: false, english_words: true, ..Default::default() };
    let mut d = Doc::new(o.clone());
    // "windows" có trong danh sách; "wiki" thì không, và không còn tự trả phím vì đã tắt chính tả.
    assert_eq!(d.type_str("test windows wiki "), "test windows ưiki ");
    let mut d = Doc::new(Options { english_words: false, ..o });
    assert_eq!(d.type_str("test "), "tét ");
}

#[test]
fn standalone_w_undo() {
    check(
        InputMethod::Telex,
        true,
        &[
            ("w", "ư"),
            ("ww", "w"),
            ("W", "Ư"),
            ("WW", "W"),
            ("tww", "tw"),
            ("nhww", "nhw"),
            ("uww", "uw"),      // u có sẵn + w (móc) + w (bỏ móc): giữ chữ u
            ("[", "ơ"),
            ("[[", "["),
            ("]]", "]"),
            ("tw ww", "tư w"),
            ("www", "ww"),
        ],
    );
}

#[test]
fn auto_capitalize_sentences() {
    let cases = [
        ("xin chaof. tooi laf an", "xin chào. Tôi là an"),
        ("ddi ddaau? ddi hocj! vaang", "đi đâu? Đi học! Vâng"),
        ("dongf 1\ndongf 2", "dòng 1\nDòng 2"),
        ("ddau. \"vaang.\" dduowcj", "đau. \"Vâng.\" Được"), // câu mới mở ngoặc kép vẫn viết hoa
        ("chowf... ddax", "chờ... đã"),                      // ba chấm: không viết hoa
        ("v.v. vaf", "v.v. và"),                              // viết tắt
        ("TP. hoof chis minh", "TP. hồ chí minh"),
        ("google.com laf", "google.com là"),                  // không có dấu cách sau dấu chấm
        ("so 1.5 laf", "so 1.5 là"),
        ("heets. vn", "hết. Việt nam"),                       // gõ tắt ở đầu câu: viết hoa theo chữ gõ
        ("xong. Test", "xong. Test"),                         // đã viết hoa sẵn
        ("xong.  hai", "xong.  Hai"),                         // hai dấu cách
        ("xong. \u{8}\u{8}. ba", "xong. Ba"),                 // xoá rồi gõ lại
    ];
    for (input, expected) in cases {
        let mut d = Doc::new(opts(InputMethod::Telex, true));
        d.engine.auto_capitalize = true;
        d.macros = Some(MacroTable::parse("vn:việt nam\n", true));
        assert_eq!(d.type_str(input), expected, "gõ {input:?}");
    }
    // Tắt tính năng: giữ nguyên.
    let mut d = Doc::new(opts(InputMethod::Telex, true));
    assert_eq!(d.type_str("xin chaof. tooi"), "xin chào. tôi");
    // Bấm chuột / di chuyển con trỏ: không chắc đang ở đầu câu.
    let mut d = Doc::new(opts(InputMethod::Telex, true));
    d.engine.auto_capitalize = true;
    d.type_str("xong. ");
    d.engine.reset();
    assert_eq!(d.type_str("tieeps"), "xong. tiếp");
}

#[test]
fn restore_key() {
    let cases = [
        // Từ đang gõ: về đúng phím đã bấm, bấm lần nữa thì quay lại.
        ("its\u{1b}", "its"),
        ("its\u{1b}\u{1b}", "ít"),
        ("tieesng\u{1b}", "tieesng"),
        // Từ vừa xong, ngay sau một dấu cách.
        ("its \u{1b}", "its "),
        ("has \u{1b}is \u{1b}", "has is "),
        ("this is \u{1b}a test", "thí is a test"),
        // Từ đã tự giữ tiếng Anh → đổi sang tiếng Việt.
        ("test \u{1b}", "tét "),
        // Không có gì để đổi: không làm gì.
        ("abc \u{1b}", "abc "),
        ("its  \u{1b}", "ít  "), // hai dấu cách: không sửa từ trước
        // Gõ tiếp sau khi khôi phục: nối chữ thường.
        ("its\u{1b}y", "itsy"),
    ];
    for (input, expected) in cases {
        let mut d = Doc::new(opts(InputMethod::Telex, true));
        assert_eq!(d.type_str(input), expected, "gõ {input:?}");
    }
    // Hoàn tác gõ tắt.
    let mut d = Doc::new(opts(InputMethod::Telex, true));
    d.macros = Some(MacroTable::parse("vn:việt nam\n", true));
    assert_eq!(d.type_str("vn \u{1b}"), "vn ");
}

#[test]
fn viqr_method() {
    check(
        InputMethod::Viqr,
        true,
        &[
            ("Vie^.t Nam", "Việt Nam"),
            ("tie^'ng", "tiếng"),
            ("ddu+o+`ng", "đường"),
            ("a(n", "ăn"),
            ("ho?i", "hỏi"),
            ("ho+?i", "hởi"),
            ("nguo+`i", "người"),
            ("bo'-", "bo"),
        ],
    );
}

#[test]
fn custom_keymap_method() {
    use crate::method::{KeyAction, KeyMap};
    // Bảng phím lấy từ keymap.txt của UniKey (giống Telex) + phím [ ] chèn ơ ư + 6 = mũ.
    let mut km = KeyMap::telex_like();
    km.set('[', Some(KeyAction::Letter('ơ')));
    km.set('6', Some(KeyAction::RoofAll));
    km.set('w', Some(KeyAction::TelexW));
    let o = Options { method: InputMethod::Custom, keymap: Some(std::sync::Arc::new(km)), ..Default::default() };
    for (input, expected) in [
        ("tieesng", "tiếng"),
        ("dduwowcj", "được"),
        ("w", "ư"),
        ("ww", "w"),
        ("tie6ng", "tiêng"),
        ("[", "ơ"),
        ("windows", "windows"),
    ] {
        let mut d = Doc::new(o.clone());
        assert_eq!(d.type_str(input), expected, "gõ {input:?}");
    }
    // Xoá lùi vẫn dựng lại được dãy phím với bảng phím riêng.
    let mut d = Doc::new(o);
    assert_eq!(d.type_str("tieesng\u{8}gf"), "tiềng");
}

#[test]
fn strict_marking() {
    // Tắt gõ dấu tự do: dấu phụ chỉ áp cho chữ vừa gõ; dấu thanh vẫn tự do.
    let o = Options { free_marking: false, ..Default::default() };
    for (input, expected) in [
        ("tieengs", "tiếng"),
        ("tienge", "tienge"), // e ở cuối không còn thành mũ cho chữ e phía trước
        ("dod", "dod"),
        ("ddo", "đo"),
        ("thuongw", "thuongw"),
        ("thuowng", "thương"),
        ("hoaf", "hoà"),
        ("tiengs", "tiéngs"),
    ] {
        let mut d = Doc::new(o.clone());
        let got = d.type_str(input).to_string();
        // "tiengs": ie không có mũ nên không phải tiếng Việt hợp lệ → trả phím gốc.
        let expected = if input == "tiengs" { "tiengs" } else { expected };
        assert_eq!(got, expected, "gõ {input:?}");
    }
}
