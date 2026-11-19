//! Tự động cập nhật từ GitHub Releases.
//!
//! - Kiểm tra: hỏi API GitHub bản phát hành mới nhất, so với phiên bản đang chạy. Chỉ thông báo.
//! - Cập nhật (khi người dùng bấm): tải C_Input.exe và SHA256SUMS.txt của bản đó, so mã SHA-256 —
//!   không khớp thì từ chối. Đổi tên file đang chạy thành .old (Windows cho phép với file đang chạy),
//!   đặt bản mới vào chỗ cũ; lần khởi động sau xoá file .old.

use serde_json::Value;

use crate::ai::https_request;

pub const REPO: &str = "giacongnguyen94-cmyk/C_Input";
pub const RELEASES_URL: &str = "https://github.com/giacongnguyen94-cmyk/C_Input/releases";

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    exe_url: String,
    sums_url: Option<String>,
}

/// So hai phiên bản dạng "1.2.3" (bỏ chữ v đầu). `true` nếu `remote` mới hơn `current`.
pub fn is_newer(remote: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim().trim_start_matches(['v', 'V']).split('.').map(|p| p.trim().parse::<u64>().unwrap_or(0)).collect()
    };
    let (r, c) = (parse(remote), parse(current));
    for i in 0..r.len().max(c.len()) {
        let (a, b) = (r.get(i).copied().unwrap_or(0), c.get(i).copied().unwrap_or(0));
        if a != b {
            return a > b;
        }
    }
    false
}

/// Tách "https://host/path" thành (host, path).
fn split_url(url: &str) -> Option<(&str, &str)> {
    let rest = url.strip_prefix("https://")?;
    let slash = rest.find('/').unwrap_or(rest.len());
    Some((&rest[..slash], if slash < rest.len() { &rest[slash..] } else { "/" }))
}

fn get(url: &str, accept: &str) -> Result<Vec<u8>, String> {
    let (host, path) = split_url(url).ok_or("Địa chỉ tải không hợp lệ")?;
    let headers = format!("Accept: {accept}\r\n");
    let (status, body) = https_request(host, "GET", path, &headers, None)?;
    if status != 200 {
        return Err(format!("GitHub trả lỗi {status}"));
    }
    Ok(body)
}

pub fn parse_release(body: &str) -> Result<Release, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("Phản hồi không đọc được: {e}"))?;
    let version = v["tag_name"].as_str().ok_or("Không có bản phát hành nào")?.trim_start_matches('v').to_string();
    let assets = v["assets"].as_array().cloned().unwrap_or_default();
    let find = |name: &str| {
        assets
            .iter()
            .find(|a| a["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
            .and_then(|a| a["browser_download_url"].as_str())
            .map(str::to_string)
    };
    Ok(Release {
        version,
        notes: v["body"].as_str().unwrap_or("").to_string(),
        exe_url: find("C_Input.exe").ok_or("Bản phát hành không có file C_Input.exe")?,
        sums_url: find("SHA256SUMS.txt"),
    })
}

/// Bản phát hành mới nhất trên GitHub.
pub fn latest() -> Result<Release, String> {
    let body = get(&format!("https://api.github.com/repos/{REPO}/releases/latest"), "application/vnd.github+json")?;
    parse_release(&String::from_utf8_lossy(&body))
}

/// Có bản mới hơn bản đang chạy không?
pub fn check() -> Result<Option<Release>, String> {
    let r = latest()?;
    Ok(is_newer(&r.version, env!("CARGO_PKG_VERSION")).then_some(r))
}

fn sha256_hex(data: &[u8]) -> Option<String> {
    use windows_sys::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};
    let mut out = [0u8; 32];
    let status = unsafe {
        BCryptHash(BCRYPT_SHA256_ALG_HANDLE, std::ptr::null(), 0, data.as_ptr(), data.len() as u32, out.as_mut_ptr(), 32)
    };
    (status == 0).then(|| out.iter().map(|b| format!("{b:02x}")).collect())
}

/// Lấy mã SHA-256 của C_Input.exe trong nội dung SHA256SUMS.txt ("<hex>  C_Input.exe" hoặc "<hex> *C_Input.exe").
pub fn expected_hash(sums: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut parts = l.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        (name.eq_ignore_ascii_case("C_Input.exe") && hash.len() == 64).then(|| hash.to_ascii_lowercase())
    })
}

/// Tải, kiểm tra SHA-256 và thay file chạy. Trả về đường dẫn file chạy (đã là bản mới).
pub fn install(release: &Release) -> Result<std::path::PathBuf, String> {
    let sums_url = release.sums_url.as_deref().ok_or("Bản phát hành thiếu SHA256SUMS.txt — không thể kiểm tra, không cài")?;
    let sums = String::from_utf8_lossy(&get(sums_url, "application/octet-stream")?).into_owned();
    let expected = expected_hash(&sums).ok_or("SHA256SUMS.txt không có mã của C_Input.exe")?;
    let exe = get(&release.exe_url, "application/octet-stream")?;
    let actual = sha256_hex(&exe).ok_or("Không tính được mã SHA-256")?;
    if actual != expected {
        return Err("Mã SHA-256 của file tải về không khớp — đã huỷ cập nhật để an toàn".into());
    }
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let new_path = current.with_extension("new.exe");
    let old_path = current.with_extension("old.exe");
    std::fs::write(&new_path, &exe).map_err(|e| format!("Không ghi được file mới (thư mục cần quyền ghi): {e}"))?;
    let _ = std::fs::remove_file(&old_path);
    std::fs::rename(&current, &old_path).map_err(|e| {
        let _ = std::fs::remove_file(&new_path);
        format!("Không thay được file đang chạy: {e}")
    })?;
    if let Err(e) = std::fs::rename(&new_path, &current) {
        // Trả lại như cũ.
        let _ = std::fs::rename(&old_path, &current);
        return Err(format!("Không đặt được bản mới: {e}"));
    }
    Ok(current)
}

/// Xoá file .old còn lại sau lần cập nhật trước (gọi khi khởi động).
pub fn cleanup_old() {
    if let Ok(cur) = std::env::current_exe() {
        let _ = std::fs::remove_file(cur.with_extension("old.exe"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("1.2.0", "1.1.0"));
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(!is_newer("1.1.0", "1.1.0"));
        assert!(!is_newer("1.0.9", "1.1.0"));
        assert!(is_newer("1.1.1", "1.1"));
    }

    #[test]
    fn release_json_and_sums() {
        let body = r#"{"tag_name":"v1.2.0","body":"Mới","assets":[
            {"name":"C_Input.exe","browser_download_url":"https://github.com/x/y/releases/download/v1.2.0/C_Input.exe"},
            {"name":"SHA256SUMS.txt","browser_download_url":"https://github.com/x/y/releases/download/v1.2.0/SHA256SUMS.txt"}]}"#;
        let r = parse_release(body).unwrap();
        assert_eq!(r.version, "1.2.0");
        assert!(r.sums_url.is_some());
        let h = "ea0b21634b53536a153f990f6c1fd99b61741165df950713f4b29e2a3c1b827b";
        assert_eq!(expected_hash(&format!("{h}  C_Input.exe\n")).as_deref(), Some(h));
        assert_eq!(expected_hash(&format!("{} *C_Input.exe", h.to_uppercase())).as_deref(), Some(h));
        assert_eq!(split_url("https://api.github.com/repos/a/b"), Some(("api.github.com", "/repos/a/b")));
    }

    #[test]
    fn sha256_known_value() {
        assert_eq!(
            sha256_hex(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// Gọi thật API GitHub (repo công khai, không gửi dữ liệu người dùng).
    /// cargo test -p c_input -- --ignored github
    #[test]
    #[ignore]
    fn github_latest_release() {
        let r = latest().unwrap();
        eprintln!("bản mới nhất trên GitHub: {} ({} ký tự ghi chú)", r.version, r.notes.len());
        assert!(!r.version.is_empty());
    }
}
