//! Cấu hình người dùng: %APPDATA%\C_Input\config.toml và macros.txt.

use std::path::PathBuf;
use std::time::SystemTime;

use cinput_charset::Charset;
use cinput_core::{InputMethod, MacroTable, Options};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Method {
    #[default]
    Telex,
    Vni,
    SimpleTelex,
}

impl Method {
    pub const ALL: [Method; 3] = [Method::Telex, Method::Vni, Method::SimpleTelex];

    pub fn to_core(self) -> InputMethod {
        match self {
            Method::Telex => InputMethod::Telex,
            Method::Vni => InputMethod::Vni,
            Method::SimpleTelex => InputMethod::SimpleTelex,
        }
    }

    pub fn label(self) -> &'static str {
        self.to_core().label()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SwitchKey {
    #[default]
    CtrlShift,
    AltZ,
}

impl SwitchKey {
    pub fn label(self) -> &'static str {
        match self {
            SwitchKey::CtrlShift => "Ctrl + Shift",
            SwitchKey::AltZ => "Alt + Z",
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Đang gõ tiếng Việt hay tiếng Anh.
    pub vietnamese: bool,
    pub method: Method,
    pub charset: Charset,
    pub modern_tone: bool,
    pub spell_check: bool,
    /// Giữ nguyên từ tiếng Anh trong danh sách (test, error...).
    pub english_words: bool,
    /// Tự viết hoa chữ đầu câu.
    pub auto_capitalize: bool,
    /// Tự tắt tiếng Việt khi đang nhập ô mật khẩu.
    pub password_off: bool,
    /// Dịch AI: gõ xong một câu thì tự dịch câu đó sang ngôn ngữ đích.
    pub translate_enabled: bool,
    /// Ngôn ngữ đích (tên tiếng Anh, vd "English").
    pub translate_lang: String,
    /// API key Gemini đã mã hoá DPAPI (hex) — không lưu dạng chữ thường.
    pub gemini_key: String,
    pub gemini_model: String,
    pub switch_key: SwitchKey,
    pub beep: bool,
    pub macro_enabled: bool,
    pub macro_always: bool,
    pub macro_auto_case: bool,
    pub per_app: bool,
    /// Tên file .exe (viết thường) luôn gõ tiếng Anh.
    pub excluded_apps: Vec<String>,
    pub autostart: bool,
    /// Sửa lỗi gợi ý tự động (ô địa chỉ trình duyệt, Excel) nuốt mất phím xoá khi bỏ dấu.
    pub browser_fix: bool,
    /// Ứng dụng áp dụng cách sửa trên (tên .exe viết thường).
    pub browser_apps: Vec<String>,
    pub clip_from: Charset,
    pub clip_to: Charset,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            vietnamese: true,
            method: Method::Telex,
            charset: Charset::Unicode,
            modern_tone: true,
            spell_check: true,
            english_words: true,
            auto_capitalize: true,
            password_off: true,
            translate_enabled: false,
            translate_lang: "English".to_string(),
            gemini_key: String::new(),
            gemini_model: crate::ai::DEFAULT_MODEL.to_string(),
            switch_key: SwitchKey::CtrlShift,
            beep: false,
            macro_enabled: true,
            macro_always: false,
            macro_auto_case: true,
            per_app: false,
            excluded_apps: Vec::new(),
            autostart: false,
            browser_fix: true,
            browser_apps: DEFAULT_BROWSER_APPS.iter().map(|s| s.to_string()).collect(),
            clip_from: Charset::Tcvn3,
            clip_to: Charset::Unicode,
        }
    }
}

pub const DEFAULT_BROWSER_APPS: [&str; 9] = [
    "chrome.exe", "msedge.exe", "firefox.exe", "browser.exe", "brave.exe", "opera.exe", "vivaldi.exe",
    "coccoc.exe", "excel.exe",
];

impl Config {
    pub fn needs_browser_fix(&self, exe: &str) -> bool {
        self.browser_fix && self.browser_apps.iter().any(|a| a.eq_ignore_ascii_case(exe))
    }

    pub fn engine_options(&self) -> Options {
        Options {
            method: self.method.to_core(),
            modern_tone: self.modern_tone,
            spell_check: self.spell_check,
            english_words: self.english_words,
        }
    }

    pub fn load() -> Config {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(data_dir())?;
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = config_path().with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, config_path())
    }

    pub fn is_excluded(&self, exe: &str) -> bool {
        self.excluded_apps.iter().any(|a| a.eq_ignore_ascii_case(exe))
    }
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("C_Input")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.toml")
}

pub fn macros_path() -> PathBuf {
    data_dir().join("macros.txt")
}

pub fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

const DEFAULT_MACROS: &str = ";C_Input macro file — mỗi dòng: từ_tắt:nội dung
vn:việt nam
ko:không
dc:được
nc:nước
hn:hà nội
hcm:hồ chí minh
";

pub fn load_macros(auto_case: bool) -> MacroTable {
    match std::fs::read_to_string(macros_path()) {
        Ok(s) => MacroTable::parse(&s, auto_case),
        Err(_) => MacroTable::parse(DEFAULT_MACROS, auto_case),
    }
}

pub fn save_macros(t: &MacroTable) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir())?;
    std::fs::write(macros_path(), t.serialize())
}

pub fn english_path() -> PathBuf {
    data_dir().join("english.txt")
}

const ENGLISH_HEADER: &str = "# Từ tiếng Anh của bạn: gõ đúng các từ này (Telex) thì C_Input giữ nguyên tiếng Anh.
# Mỗi dòng một hoặc nhiều từ, cách nhau bằng dấu cách. Dòng bắt đầu bằng # là chú thích.
# Lưu ý: đừng thêm từ trùng cách gõ một từ tiếng Việt bạn hay dùng (vd \"its\" là cách gõ \"ít\").
";

/// Danh sách có sẵn + danh sách riêng của người dùng.
pub fn load_english() -> cinput_core::EnglishWords {
    let mut e = cinput_core::EnglishWords::builtin();
    if let Ok(text) = std::fs::read_to_string(english_path()) {
        e.extend_from_text(&text);
    }
    e
}

/// Số từ trong danh sách riêng của người dùng.
pub fn user_english_count() -> usize {
    let mut e = cinput_core::EnglishWords::default();
    if let Ok(text) = std::fs::read_to_string(english_path()) {
        e.extend_from_text(&text);
    }
    e.len()
}

/// Tạo file danh sách riêng (nếu chưa có) để người dùng mở ra sửa.
pub fn ensure_english_file() -> std::io::Result<()> {
    if !english_path().exists() {
        std::fs::create_dir_all(data_dir())?;
        std::fs::write(english_path(), ENGLISH_HEADER)?;
    }
    Ok(())
}
