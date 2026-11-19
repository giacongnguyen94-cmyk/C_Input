//! Dịch AI bằng Google Gemini: gọi API qua WinHTTP (TLS của Windows), lưu API key mã hoá bằng DPAPI.

use std::ptr::{null, null_mut};

use serde_json::{Value, json};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Networking::WinHttp::*;
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

use crate::win::wide;

const HOST: &str = "generativelanguage.googleapis.com";
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";
pub const KEY_URL: &str = "https://aistudio.google.com/app/apikey";

/// Ngôn ngữ hay dùng, luôn đứng đầu danh sách.
const POPULAR: &[(&str, &str)] = &[
    ("English", "Tiếng Anh"),
    ("Vietnamese", "Tiếng Việt"),
    ("Simplified Chinese", "Tiếng Trung giản thể (Trung Quốc)"),
    ("Traditional Chinese (Taiwan)", "Tiếng Trung phồn thể (Đài Loan)"),
    ("Japanese", "Tiếng Nhật"),
    ("Korean", "Tiếng Hàn"),
];

/// Các ngôn ngữ khác (tự xếp theo vần khi hiển thị). (Tên tiếng Anh gửi cho Gemini, nhãn tiếng Việt.)
const OTHERS: &[(&str, &str)] = &[
    // Trung Quốc, Đài Loan và các phương ngữ Hán
    ("Traditional Chinese (Hong Kong)", "Tiếng Trung phồn thể (Hồng Kông)"),
    ("Taiwanese Hokkien", "Tiếng Đài Loan (Mân Nam, Tâi-gí)"),
    ("Cantonese", "Tiếng Quảng Đông"),
    ("Hakka Chinese", "Tiếng Khách Gia (Hakka)"),
    ("Teochew", "Tiếng Triều Châu"),
    ("Hokkien", "Tiếng Phúc Kiến (Mân Nam)"),
    ("Shanghainese", "Tiếng Thượng Hải (Ngô)"),
    ("Classical Chinese", "Tiếng Hán cổ (văn ngôn)"),
    // Đông Nam Á và Việt Nam
    ("Thai", "Tiếng Thái"),
    ("Lao", "Tiếng Lào"),
    ("Khmer", "Tiếng Khmer (Campuchia)"),
    ("Burmese", "Tiếng Myanmar (Miến Điện)"),
    ("Malay", "Tiếng Mã Lai"),
    ("Indonesian", "Tiếng Indonesia"),
    ("Filipino", "Tiếng Filipino (Tagalog)"),
    ("Cebuano", "Tiếng Cebuano"),
    ("Ilocano", "Tiếng Ilocano"),
    ("Javanese", "Tiếng Java"),
    ("Sundanese", "Tiếng Sunda"),
    ("Tetum", "Tiếng Tetum (Đông Timor)"),
    ("Hmong", "Tiếng H'Mông"),
    ("Tay", "Tiếng Tày"),
    ("Cham", "Tiếng Chăm"),
    ("Rade (Ede)", "Tiếng Ê Đê"),
    ("Jarai", "Tiếng Gia Rai"),
    ("Shan", "Tiếng Shan"),
    ("Karen", "Tiếng Karen"),
    ("Mon", "Tiếng Môn"),
    // Nam Á
    ("Hindi", "Tiếng Hindi"),
    ("Bengali", "Tiếng Bengal"),
    ("Urdu", "Tiếng Urdu"),
    ("Punjabi", "Tiếng Punjab"),
    ("Gujarati", "Tiếng Gujarat"),
    ("Marathi", "Tiếng Marathi"),
    ("Tamil", "Tiếng Tamil"),
    ("Telugu", "Tiếng Telugu"),
    ("Kannada", "Tiếng Kannada"),
    ("Malayalam", "Tiếng Malayalam"),
    ("Odia", "Tiếng Odia"),
    ("Assamese", "Tiếng Assam"),
    ("Nepali", "Tiếng Nepal"),
    ("Sinhala", "Tiếng Sinhala"),
    ("Sanskrit", "Tiếng Phạn (Sanskrit)"),
    ("Pali", "Tiếng Pali"),
    ("Dhivehi", "Tiếng Dhivehi (Maldives)"),
    ("Dzongkha", "Tiếng Dzongkha (Bhutan)"),
    ("Tibetan", "Tiếng Tây Tạng"),
    ("Sindhi", "Tiếng Sindh"),
    ("Konkani", "Tiếng Konkan"),
    ("Maithili", "Tiếng Maithili"),
    ("Bhojpuri", "Tiếng Bhojpuri"),
    // Trung Á, Tây Á, Trung Đông
    ("Arabic", "Tiếng Ả Rập"),
    ("Egyptian Arabic", "Tiếng Ả Rập (Ai Cập)"),
    ("Persian", "Tiếng Ba Tư (Farsi)"),
    ("Dari", "Tiếng Dari (Afghanistan)"),
    ("Pashto", "Tiếng Pashto"),
    ("Turkish", "Tiếng Thổ Nhĩ Kỳ"),
    ("Hebrew", "Tiếng Do Thái (Hebrew)"),
    ("Yiddish", "Tiếng Yiddish"),
    ("Kurdish (Kurmanji)", "Tiếng Kurd (Kurmanji)"),
    ("Kurdish (Sorani)", "Tiếng Kurd (Sorani)"),
    ("Armenian", "Tiếng Armenia"),
    ("Georgian", "Tiếng Gruzia"),
    ("Azerbaijani", "Tiếng Azerbaijan"),
    ("Kazakh", "Tiếng Kazakh"),
    ("Uzbek", "Tiếng Uzbek"),
    ("Kyrgyz", "Tiếng Kyrgyz"),
    ("Tajik", "Tiếng Tajik"),
    ("Turkmen", "Tiếng Turkmen"),
    ("Uyghur", "Tiếng Duy Ngô Nhĩ (Uyghur)"),
    ("Mongolian", "Tiếng Mông Cổ"),
    ("Tatar", "Tiếng Tatar"),
    ("Bashkir", "Tiếng Bashkir"),
    ("Chechen", "Tiếng Chechen"),
    ("Assyrian Neo-Aramaic", "Tiếng Aram (Assyria)"),
    // Châu Âu
    ("French", "Tiếng Pháp"),
    ("German", "Tiếng Đức"),
    ("Spanish", "Tiếng Tây Ban Nha"),
    ("Spanish (Latin America)", "Tiếng Tây Ban Nha (Mỹ Latinh)"),
    ("Portuguese (Brazil)", "Tiếng Bồ Đào Nha (Brazil)"),
    ("Portuguese (Portugal)", "Tiếng Bồ Đào Nha (Bồ Đào Nha)"),
    ("Italian", "Tiếng Ý"),
    ("Russian", "Tiếng Nga"),
    ("Ukrainian", "Tiếng Ukraina"),
    ("Belarusian", "Tiếng Belarus"),
    ("Polish", "Tiếng Ba Lan"),
    ("Czech", "Tiếng Séc"),
    ("Slovak", "Tiếng Slovakia"),
    ("Slovenian", "Tiếng Slovenia"),
    ("Croatian", "Tiếng Croatia"),
    ("Serbian", "Tiếng Serbia"),
    ("Bosnian", "Tiếng Bosnia"),
    ("Montenegrin", "Tiếng Montenegro"),
    ("Macedonian", "Tiếng Macedonia"),
    ("Bulgarian", "Tiếng Bulgaria"),
    ("Romanian", "Tiếng Romania"),
    ("Hungarian", "Tiếng Hungary"),
    ("Greek", "Tiếng Hy Lạp"),
    ("Ancient Greek", "Tiếng Hy Lạp cổ"),
    ("Albanian", "Tiếng Albania"),
    ("Dutch", "Tiếng Hà Lan"),
    ("Flemish", "Tiếng Flemish (Bỉ)"),
    ("Afrikaans", "Tiếng Afrikaans"),
    ("Danish", "Tiếng Đan Mạch"),
    ("Swedish", "Tiếng Thuỵ Điển"),
    ("Norwegian", "Tiếng Na Uy"),
    ("Icelandic", "Tiếng Iceland"),
    ("Faroese", "Tiếng Faroe"),
    ("Finnish", "Tiếng Phần Lan"),
    ("Estonian", "Tiếng Estonia"),
    ("Latvian", "Tiếng Latvia"),
    ("Lithuanian", "Tiếng Litva"),
    ("Irish", "Tiếng Ireland (Gaelic)"),
    ("Scottish Gaelic", "Tiếng Gaelic Scotland"),
    ("Welsh", "Tiếng Wales"),
    ("Breton", "Tiếng Breton"),
    ("Basque", "Tiếng Basque"),
    ("Catalan", "Tiếng Catalan"),
    ("Galician", "Tiếng Galicia"),
    ("Occitan", "Tiếng Occitan"),
    ("Corsican", "Tiếng Corse"),
    ("Luxembourgish", "Tiếng Luxembourg"),
    ("Swiss German", "Tiếng Đức (Thuỵ Sĩ)"),
    ("Maltese", "Tiếng Malta"),
    ("Romansh", "Tiếng Romansh"),
    ("Sardinian", "Tiếng Sardinia"),
    ("Sicilian", "Tiếng Sicily"),
    ("Frisian", "Tiếng Frisia"),
    ("Sami (Northern)", "Tiếng Sami (Bắc Âu)"),
    ("Latin", "Tiếng Latinh"),
    ("Esperanto", "Tiếng Esperanto (Quốc tế ngữ)"),
    ("English (British)", "Tiếng Anh (Anh)"),
    ("English (American)", "Tiếng Anh (Mỹ)"),
    ("French (Canada)", "Tiếng Pháp (Canada)"),
    // Châu Phi
    ("Swahili", "Tiếng Swahili"),
    ("Amharic", "Tiếng Amharic (Ethiopia)"),
    ("Tigrinya", "Tiếng Tigrinya"),
    ("Oromo", "Tiếng Oromo"),
    ("Somali", "Tiếng Somali"),
    ("Hausa", "Tiếng Hausa"),
    ("Yoruba", "Tiếng Yoruba"),
    ("Igbo", "Tiếng Igbo"),
    ("Fula", "Tiếng Fula"),
    ("Wolof", "Tiếng Wolof"),
    ("Zulu", "Tiếng Zulu"),
    ("Xhosa", "Tiếng Xhosa"),
    ("Sesotho", "Tiếng Sesotho"),
    ("Setswana", "Tiếng Tswana"),
    ("Shona", "Tiếng Shona"),
    ("Kinyarwanda", "Tiếng Kinyarwanda"),
    ("Kirundi", "Tiếng Kirundi"),
    ("Luganda", "Tiếng Luganda"),
    ("Lingala", "Tiếng Lingala"),
    ("Malagasy", "Tiếng Malagasy (Madagascar)"),
    ("Chichewa", "Tiếng Chichewa"),
    ("Twi", "Tiếng Twi (Akan)"),
    ("Ewe", "Tiếng Ewe"),
    ("Berber (Tamazight)", "Tiếng Berber (Tamazight)"),
    ("Moroccan Arabic", "Tiếng Ả Rập (Ma-rốc)"),
    ("Tsonga", "Tiếng Tsonga"),
    // Châu Mỹ và Thái Bình Dương
    ("Haitian Creole", "Tiếng Creole Haiti"),
    ("Quechua", "Tiếng Quechua"),
    ("Guarani", "Tiếng Guarani"),
    ("Aymara", "Tiếng Aymara"),
    ("Nahuatl", "Tiếng Nahuatl"),
    ("Maya (Yucatec)", "Tiếng Maya (Yucatec)"),
    ("Navajo", "Tiếng Navajo"),
    ("Cherokee", "Tiếng Cherokee"),
    ("Inuktitut", "Tiếng Inuktitut"),
    ("Greenlandic", "Tiếng Greenland"),
    ("Hawaiian", "Tiếng Hawaii"),
    ("Maori", "Tiếng Maori"),
    ("Samoan", "Tiếng Samoa"),
    ("Tongan", "Tiếng Tonga"),
    ("Fijian", "Tiếng Fiji"),
    ("Tok Pisin", "Tiếng Tok Pisin (Papua New Guinea)"),
    ("Bislama", "Tiếng Bislama"),
    ("Tahitian", "Tiếng Tahiti"),
    ("Chamorro", "Tiếng Chamorro"),
    ("Papiamento", "Tiếng Papiamento"),
    // Khác
    ("Javanese (Krama)", "Tiếng Java (Krama)"),
    ("Ainu", "Tiếng Ainu"),
    ("Okinawan", "Tiếng Okinawa"),
    ("Korean (North)", "Tiếng Triều Tiên (Bắc Triều Tiên)"),
];

/// Số ngôn ngữ hay dùng đứng đầu danh sách.
pub const POPULAR_LANGUAGES: usize = POPULAR.len();

/// Toàn bộ ngôn ngữ: nhóm hay dùng trước, phần còn lại xếp theo vần tên tiếng Việt (không dấu).
pub fn languages() -> &'static [(&'static str, &'static str)] {
    static L: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();
    L.get_or_init(|| {
        let mut rest: Vec<_> = OTHERS.to_vec();
        rest.sort_by_key(|(_, label)| cinput_charset::remove_tones(label).to_lowercase());
        POPULAR.iter().copied().chain(rest).collect()
    })
}

/// Tìm ngôn ngữ: không phân biệt hoa thường, không cần dấu, khớp cả tên tiếng Anh
/// ("nhat", "japan" → Tiếng Nhật).
pub fn language_matches(name: &str, label: &str, query: &str) -> bool {
    let norm = |s: &str| cinput_charset::remove_tones(s).to_lowercase();
    let q = norm(query.trim());
    q.is_empty() || norm(label).contains(&q) || name.to_lowercase().contains(&q)
}

pub fn language_label(name: &str) -> &str {
    languages().iter().find(|(n, _)| *n == name).map(|(_, l)| *l).unwrap_or(name)
}

// ---------------------------------------------------------------- API key (DPAPI)

/// Mã hoá API key bằng DPAPI (chỉ tài khoản Windows hiện tại giải mã được), trả về chuỗi hex.
pub fn protect_key(key: &str) -> Option<String> {
    let mut data = key.as_bytes().to_vec();
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_mut_ptr() };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
    let desc = wide("C_Input Gemini API key");
    unsafe {
        if CryptProtectData(&input, desc.as_ptr(), null(), null(), null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) == 0 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize);
        let hex = bytes.iter().map(|b| format!("{b:02x}")).collect();
        LocalFree(out.pbData as _);
        Some(hex)
    }
}

/// Giải mã API key đã lưu.
pub fn unprotect_key(hex: &str) -> Option<String> {
    if hex.is_empty() || hex.len() % 2 != 0 {
        return None;
    }
    let mut data: Vec<u8> =
        (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16)).collect::<Result<_, _>>().ok()?;
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_mut_ptr() };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
    unsafe {
        if CryptUnprotectData(&input, null_mut(), null(), null(), null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) == 0 {
            return None;
        }
        let key = String::from_utf8_lossy(std::slice::from_raw_parts(out.pbData, out.cbData as usize)).into_owned();
        LocalFree(out.pbData as _);
        Some(key)
    }
}

// ---------------------------------------------------------------- HTTPS (WinHTTP)

struct Handle(*mut core::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

/// Gửi một yêu cầu HTTPS tới máy chủ Gemini. Trả về (mã trạng thái, nội dung).
fn https(method: &str, path: &str, api_key: &str, body: Option<&str>) -> Result<(u32, String), String> {
    let err = |what: &str| format!("{what} (lỗi Windows {})", unsafe { windows_sys::Win32::Foundation::GetLastError() });
    unsafe {
        let agent = wide("C_Input/1.0");
        let session = Handle(WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null(), null(), 0));
        if session.0.is_null() {
            return Err(err("Không mở được kết nối mạng"));
        }
        WinHttpSetTimeouts(session.0, 10_000, 10_000, 15_000, 30_000);
        let host = wide(HOST);
        let connect = Handle(WinHttpConnect(session.0, host.as_ptr(), 443, 0));
        if connect.0.is_null() {
            return Err(err("Không kết nối được máy chủ Gemini"));
        }
        let verb = wide(method);
        let obj = wide(path);
        let req = Handle(WinHttpOpenRequest(connect.0, verb.as_ptr(), obj.as_ptr(), null(), null(), null(), WINHTTP_FLAG_SECURE));
        if req.0.is_null() {
            return Err(err("Không tạo được yêu cầu"));
        }
        // API key gửi trong header (không đặt trong URL).
        let headers = wide(&format!("Content-Type: application/json; charset=utf-8\r\nx-goog-api-key: {api_key}\r\n"));
        let payload = body.unwrap_or("").as_bytes();
        let ok = WinHttpSendRequest(
            req.0,
            headers.as_ptr(),
            u32::MAX, // -1: chuỗi header kết thúc bằng 0
            if payload.is_empty() { null() } else { payload.as_ptr() as *const _ },
            payload.len() as u32,
            payload.len() as u32,
            0,
        );
        if ok == 0 || WinHttpReceiveResponse(req.0, null_mut()) == 0 {
            return Err(err("Không gửi được yêu cầu tới Gemini — kiểm tra kết nối mạng"));
        }
        let mut status: u32 = 0;
        let mut len = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            &mut status as *mut u32 as *mut _,
            &mut len,
            null_mut(),
        );
        let mut buf = Vec::new();
        loop {
            let mut avail = 0u32;
            if WinHttpQueryDataAvailable(req.0, &mut avail) == 0 || avail == 0 {
                break;
            }
            let start = buf.len();
            buf.resize(start + avail as usize, 0);
            let mut read = 0u32;
            if WinHttpReadData(req.0, buf[start..].as_mut_ptr() as *mut _, avail, &mut read) == 0 {
                break;
            }
            buf.truncate(start + read as usize);
        }
        Ok((status, String::from_utf8_lossy(&buf).into_owned()))
    }
}

/// Lỗi trả về từ API: {"error": {"message": "..."}}.
fn api_error(status: u32, body: &str) -> String {
    let msg = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(200).collect());
    match status {
        400 if msg.to_lowercase().contains("api key") => "API key không hợp lệ".to_string(),
        403 => format!("API key không có quyền dùng Gemini: {msg}"),
        404 => format!("Không tìm thấy model — bấm \"Kiểm tra\" để chọn lại. ({msg})"),
        429 => "Đã vượt giới hạn số lần gọi của API key — thử lại sau ít phút".to_string(),
        _ => format!("Gemini báo lỗi {status}: {msg}"),
    }
}

// ---------------------------------------------------------------- Gemini

pub fn build_prompt(text: &str, language: &str) -> String {
    format!(
        "Translate the following text into {language}. Output ONLY the translation, with no quotes, notes or \
         explanations. Keep the meaning, tone, punctuation, emoji, names and numbers. If the text is already in \
         {language}, return it unchanged.\n\nText:\n{text}"
    )
}

fn request_body(text: &str, language: &str) -> String {
    json!({
        "contents": [{ "role": "user", "parts": [{ "text": build_prompt(text, language) }] }],
        "generationConfig": { "temperature": 0.2 }
    })
    .to_string()
}

/// Lấy văn bản trả lời từ phản hồi generateContent.
pub fn parse_reply(body: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("Phản hồi không đọc được: {e}"))?;
    let parts = v["candidates"][0]["content"]["parts"].as_array().ok_or_else(|| {
        let reason = v["promptFeedback"]["blockReason"].as_str().or(v["candidates"][0]["finishReason"].as_str());
        format!("Gemini không trả bản dịch{}", reason.map(|r| format!(" ({r})")).unwrap_or_default())
    })?;
    let text: String = parts.iter().filter_map(|p| p["text"].as_str()).collect();
    let text = text.trim().to_string();
    if text.is_empty() { Err("Gemini trả về bản dịch rỗng".to_string()) } else { Ok(text) }
}

/// Dịch một đoạn văn bản sang ngôn ngữ đích.
pub fn translate(api_key: &str, model: &str, language: &str, text: &str) -> Result<String, String> {
    let model = if model.trim().is_empty() { DEFAULT_MODEL } else { model.trim() };
    let path = format!("/v1beta/models/{model}:generateContent");
    let (status, body) = https("POST", &path, api_key, Some(&request_body(text, language)))?;
    if status != 200 {
        return Err(api_error(status, &body));
    }
    parse_reply(&body)
}

/// Danh sách model dùng được để dịch (kiểm tra luôn API key).
pub fn parse_models(body: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let mut out: Vec<String> = v["models"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|m| {
            m["supportedGenerationMethods"].as_array().is_some_and(|g| g.iter().any(|x| x == "generateContent"))
        })
        .filter_map(|m| m["name"].as_str())
        .map(|n| n.trim_start_matches("models/").to_string())
        .filter(|n| n.starts_with("gemini"))
        .collect();
    out.sort();
    out
}

pub fn list_models(api_key: &str) -> Result<Vec<String>, String> {
    let (status, body) = https("GET", "/v1beta/models?pageSize=200", api_key, None)?;
    if status != 200 {
        return Err(api_error(status, &body));
    }
    Ok(parse_models(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_roundtrip() {
        let hex = protect_key("khoa-thu-nghiem-123").unwrap();
        assert!(!hex.contains("khoa"));
        assert_eq!(unprotect_key(&hex).as_deref(), Some("khoa-thu-nghiem-123"));
        assert_eq!(unprotect_key("zz"), None);
    }

    #[test]
    fn parse_generate_content() {
        let body = r#"{"candidates":[{"content":{"parts":[{"text":"Hello, how are you?\n"}],"role":"model"},
            "finishReason":"STOP"}]}"#;
        assert_eq!(parse_reply(body).unwrap(), "Hello, how are you?");
        let blocked = r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#;
        assert!(parse_reply(blocked).unwrap_err().contains("SAFETY"));
    }

    #[test]
    fn parse_model_list() {
        let body = r#"{"models":[
            {"name":"models/gemini-2.5-flash","supportedGenerationMethods":["generateContent","countTokens"]},
            {"name":"models/text-embedding-004","supportedGenerationMethods":["embedContent"]},
            {"name":"models/gemini-2.5-pro","supportedGenerationMethods":["generateContent"]}]}"#;
        assert_eq!(parse_models(body), vec!["gemini-2.5-flash", "gemini-2.5-pro"]);
    }

    #[test]
    fn errors_are_readable() {
        let body = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}"#;
        assert_eq!(api_error(400, body), "API key không hợp lệ");
        assert!(api_error(429, "{}").contains("giới hạn"));
    }

    #[test]
    fn body_contains_text_and_language() {
        let b: Value = serde_json::from_str(&request_body("Xin chào", "Japanese")).unwrap();
        let p = b["contents"][0]["parts"][0]["text"].as_str().unwrap();
        assert!(p.contains("Japanese") && p.ends_with("Xin chào"));
    }
}

#[cfg(test)]
mod network_tests {
    /// Gọi thật tới máy chủ Gemini KHÔNG kèm API key: kiểm tra đường HTTPS/TLS và cách đọc lỗi.
    /// Chạy riêng: cargo test -p c_input -- --ignored network
    #[test]
    #[ignore]
    fn network_reaches_gemini_without_key() {
        let err = super::list_models("").unwrap_err();
        eprintln!("phản hồi khi không có key: {err}");
        assert!(!err.contains("Không kết nối") && !err.contains("Không gửi"), "{err}");
    }
}

#[cfg(test)]
mod language_tests {
    use super::*;

    #[test]
    fn list_is_clean() {
        let all = languages();
        assert!(all.len() >= 180, "chỉ có {} ngôn ngữ", all.len());
        let mut names: Vec<_> = all.iter().map(|l| l.0).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all.len(), "trùng tên ngôn ngữ");
        let mut labels: Vec<_> = all.iter().map(|l| l.1).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), all.len(), "trùng nhãn ngôn ngữ");
        assert_eq!(&all[..2], &[("English", "Tiếng Anh"), ("Vietnamese", "Tiếng Việt")]);
        // Phần sau nhóm hay dùng xếp theo vần.
        let keys: Vec<String> =
            all[POPULAR_LANGUAGES..].iter().map(|l| cinput_charset::remove_tones(l.1).to_lowercase()).collect();
        assert!(keys.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn search_without_accents() {
        let find = |q: &str| -> Vec<&str> {
            languages().iter().filter(|(n, l)| language_matches(n, l, q)).map(|l| l.1).collect()
        };
        assert_eq!(find("nhat"), vec!["Tiếng Nhật"]);
        assert_eq!(find("japan"), vec!["Tiếng Nhật"]);
        assert_eq!(find("Nhật"), vec!["Tiếng Nhật"]);
        assert!(find("trung").len() >= 3);
        let taiwan = find("taiwan");
        assert!(taiwan.contains(&"Tiếng Đài Loan (Mân Nam, Tâi-gí)") && taiwan.contains(&"Tiếng Trung phồn thể (Đài Loan)"));
        assert!(find("dai loan").len() >= 2);
        assert_eq!(language_label("Klingon"), "Klingon"); // ngôn ngữ nhập tay: hiển thị đúng tên
        assert!(find("bo dao").len() == 2);
        assert!(find("duc").contains(&"Tiếng Đức"));
        assert_eq!(find("").len(), languages().len());
        assert!(find("xyzxyz").is_empty());
    }
}
