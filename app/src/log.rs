//! Nhật ký lỗi `%APPDATA%\C_Input\log.txt` — để biết vì sao một tính năng không chạy trên máy người dùng
//! (mạng, API key, đồ hoạ, clipboard…). Không bao giờ ghi API key hay nội dung văn bản người dùng gõ.

use std::io::Write;

const MAX_BYTES: u64 = 256 * 1024;

pub fn path() -> std::path::PathBuf {
    crate::config::data_dir().join("log.txt")
}

/// Ghi một dòng kèm thời điểm và tên tiến trình (service / settings / popup).
pub fn log(line: &str) {
    if cfg!(test) {
        return; // bài kiểm thử không ghi vào nhật ký thật của người dùng
    }
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&p).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&p, p.with_file_name("log.old.txt"));
    }
    let t = unsafe {
        let mut st = std::mem::zeroed::<windows_sys::Win32::Foundation::SYSTEMTIME>();
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut st);
        format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond)
    };
    let role = std::env::args().nth(1).unwrap_or_else(|| "service".into());
    let role = role.trim_start_matches('-');
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{t} [{role}] {line}");
    }
}
