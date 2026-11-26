//! Các hàm Win32 dùng chung cho tiến trình bộ gõ và cửa sổ cài đặt.

use std::mem::size_of;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{CloseHandle, GlobalFree, HWND, LPARAM, MAX_PATH};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, GW_OWNER, IsWindowVisible,
    PostMessageW, SW_SHOWNORMAL, WM_APP,
};

pub const SERVICE_CLASS: &str = "C_Input.Service";
pub const SETTINGS_TITLE: &str = "C_Input — Cài đặt";
/// Cửa sổ cài đặt báo tiến trình bộ gõ đọc lại cấu hình.
pub const WM_CINPUT_RELOAD: u32 = WM_APP + 10;
/// Cửa sổ cài đặt yêu cầu chuyển mã clipboard.
pub const WM_CINPUT_CONVERT_CLIPBOARD: u32 = WM_APP + 11;
/// Luồng theo dõi ô mật khẩu báo trạng thái đổi (wParam = 1: đang ở ô mật khẩu).
pub const WM_CINPUT_PASSWORD: u32 = WM_APP + 12;
/// Luồng dịch trả kết quả (lParam = Box<(u64, Result<String, String>)>).
pub const WM_CINPUT_TRANSLATED: u32 = WM_APP + 13;
/// Luồng kiểm tra cập nhật báo có bản mới (lParam = Box<String> phiên bản).
pub const WM_CINPUT_UPDATE: u32 = WM_APP + 14;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "C_Input";

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn find_service_window() -> HWND {
    let class = wide(SERVICE_CLASS);
    unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) }
}

pub fn notify_service(msg: u32) -> bool {
    let h = find_service_window();
    !h.is_null() && unsafe { PostMessageW(h, msg, 0, 0) } != 0
}

/// Tên file .exe (viết thường) của tiến trình sở hữu cửa sổ.
pub fn exe_name_of_window(hwnd: HWND) -> Option<String> {
    if hwnd.is_null() {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 {
        return None;
    }
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; MAX_PATH as usize * 2];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(|s| s.to_lowercase())
    }
}

/// Các ứng dụng đang có cửa sổ hiển thị (tên .exe, đã bỏ trùng).
pub fn running_apps() -> Vec<String> {
    unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> i32 {
        unsafe {
            let list = &mut *(lp as *mut Vec<String>);
            if IsWindowVisible(hwnd) != 0
                && GetWindow(hwnd, GW_OWNER).is_null()
                && GetWindowTextLengthW(hwnd) > 0
            {
                if let Some(n) = exe_name_of_window(hwnd) {
                    if !list.contains(&n) {
                        list.push(n);
                    }
                }
            }
        }
        1
    }
    let mut list: Vec<String> = Vec::new();
    unsafe { EnumWindows(Some(cb), &mut list as *mut _ as LPARAM) };
    let me = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_lowercase()));
    list.retain(|n| Some(n) != me.as_ref() && n != "explorer.exe" && n != "applicationframehost.exe");
    list.sort();
    list
}

pub fn clipboard_text(owner: HWND) -> Option<String> {
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT as u32) == 0 || OpenClipboard(owner) == 0 {
            return None;
        }
        let h = GetClipboardData(CF_UNICODETEXT as u32);
        let mut result = None;
        if !h.is_null() {
            let p = GlobalLock(h) as *const u16;
            if !p.is_null() {
                let mut n = 0;
                while *p.add(n) != 0 {
                    n += 1;
                }
                result = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)));
                GlobalUnlock(h);
            }
        }
        CloseClipboard();
        result
    }
}

pub fn set_clipboard_text(owner: HWND, text: &str) -> bool {
    let w = wide(text);
    unsafe {
        if OpenClipboard(owner) == 0 {
            return false;
        }
        EmptyClipboard();
        let mem = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2);
        let mut ok = false;
        if !mem.is_null() {
            let p = GlobalLock(mem) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(w.as_ptr(), p, w.len());
                GlobalUnlock(mem);
                ok = !SetClipboardData(CF_UNICODETEXT as u32, mem).is_null();
            }
            if !ok {
                GlobalFree(mem);
            }
        }
        CloseClipboard();
        ok
    }
}

/// Bật/tắt khởi động cùng Windows (khoá Run của người dùng hiện tại).
pub fn set_autostart(enable: bool) {
    let key = wide(RUN_KEY);
    let name = wide(RUN_VALUE);
    unsafe {
        if enable {
            let Ok(exe) = std::env::current_exe() else { return };
            let val = wide(&format!("\"{}\"", exe.display()));
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                val.as_ptr() as *const _,
                (val.len() * 2) as u32,
            );
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr());
        }
    }
}

fn token_elevated(process: windows_sys::Win32::Foundation::HANDLE) -> Option<bool> {
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
    use windows_sys::Win32::System::Threading::OpenProcessToken;
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut el = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut el as *mut _ as *mut _,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        (ok != 0).then_some(el.TokenIsElevated != 0)
    }
}

/// C_Input đang chạy với quyền Administrator?
pub fn self_elevated() -> bool {
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    token_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false)
}

/// Cửa sổ thuộc tiến trình chạy quyền Administrator (hoặc không đọc được quyền — coi như có)?
/// Windows chặn chương trình thường gửi phím vào những cửa sổ này.
pub fn window_elevated(hwnd: HWND) -> bool {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return true;
        }
        let r = token_elevated(h).unwrap_or(true);
        CloseHandle(h);
        r
    }
}

/// Chạy lại chương trình với quyền Administrator (hiện hộp thoại UAC). `true` nếu đã khởi chạy.
pub fn run_elevated(args: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let op = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params = wide(args);
    let r = unsafe { ShellExecuteW(null_mut(), op.as_ptr(), file.as_ptr(), params.as_ptr(), std::ptr::null(), SW_SHOWNORMAL) };
    r as usize > 32
}

/// Thanh tiêu đề tối (Windows 10 20H1+ / 11) cho cửa sổ có tiêu đề này. `true` nếu đã tìm thấy cửa sổ.
pub fn dark_titlebar(title: &str) -> bool {
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
    let t = wide(title);
    let h = unsafe { FindWindowW(std::ptr::null(), t.as_ptr()) };
    if h.is_null() {
        return false;
    }
    let on: i32 = 1;
    unsafe {
        DwmSetWindowAttribute(h, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, &on as *const i32 as *const _, 4);
    }
    true
}

/// Chạy lại chính chương trình này với tham số (vd: "--settings").
pub fn spawn_self(args: &[&str]) {
    if let Ok(exe) = std::env::current_exe() {
        if let Err(e) = std::process::Command::new(&exe).args(args).spawn() {
            crate::log::log(&format!("Không chạy được {} {:?}: {e}", exe.display(), args.first()));
        }
    }
}

/// Mở file/thư mục bằng ứng dụng mặc định.
pub fn shell_open(path: &std::path::Path) {
    let op = wide("open");
    let p = wide(&path.to_string_lossy());
    unsafe {
        ShellExecuteW(null_mut(), op.as_ptr(), p.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
    }
}

/// Hộp thoại chọn file của Windows. `save = true`: hộp thoại "Lưu thành" (hỏi trước khi ghi đè).
/// `filter`: các cặp (mô tả, mẫu), vd [("Văn bản (*.txt)", "*.txt")].
pub fn file_dialog(save: bool, title: &str, filter: &[(&str, &str)], default_name: &str) -> Option<std::path::PathBuf> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let mut f: Vec<u16> = Vec::new();
    for (desc, pat) in filter {
        f.extend(desc.encode_utf16());
        f.push(0);
        f.extend(pat.encode_utf16());
        f.push(0);
    }
    f.push(0);
    let title = wide(title);
    let ext = wide("txt");
    let mut buf = vec![0u16; 1024];
    for (i, u) in default_name.encode_utf16().take(1000).enumerate() {
        buf[i] = u;
    }
    unsafe {
        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = size_of::<OPENFILENAMEW>() as u32;
        ofn.lpstrFilter = f.as_ptr();
        ofn.lpstrFile = buf.as_mut_ptr();
        ofn.nMaxFile = buf.len() as u32;
        ofn.lpstrTitle = title.as_ptr();
        ofn.lpstrDefExt = ext.as_ptr();
        ofn.Flags = OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save { OFN_OVERWRITEPROMPT } else { OFN_FILEMUSTEXIST };
        let ok = if save { GetSaveFileNameW(&mut ofn) } else { GetOpenFileNameW(&mut ofn) };
        if ok == 0 {
            return None;
        }
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

/// Hộp thoại thông báo đơn giản.
pub fn message_box(title: &str, text: &str) {
    let (t, m) = (wide(title), wide(text));
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            std::ptr::null_mut(),
            m.as_ptr(),
            t.as_ptr(),
            windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONWARNING,
        );
    }
}
