//! Nhận diện ô nhập mật khẩu đang được gõ, để tạm tắt tiếng Việt (gõ ra đúng phím đã bấm).
//!
//! Chạy trên một luồng riêng: mỗi khi ô nhập / cửa sổ được chọn thay đổi (WinEvent), kiểm tra:
//! 1. Ô Edit kiểu Win32 có kiểu ES_PASSWORD (hộp thoại Windows, WinForms, ứng dụng cũ) — tức thời.
//! 2. UI Automation `IsPassword` của phần tử đang được chọn (trình duyệt, ứng dụng hiện đại).
//! Kết quả ghi vào một cờ; hook bàn phím chỉ đọc cờ này nên không bao giờ bị chậm vì UI Automation.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EVENT_OBJECT_FOCUS, EVENT_SYSTEM_FOREGROUND, GUITHREADINFO, GWL_STYLE, GetClassNameW,
    GetForegroundWindow, GetGUIThreadInfo, GetMessageW, GetWindowLongW, GetWindowThreadProcessId, MSG,
    PostMessageW, TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

use crate::win::WM_CINPUT_PASSWORD;

static IN_PASSWORD: AtomicBool = AtomicBool::new(false);
/// Cửa sổ ẩn của bộ gõ — báo khi trạng thái đổi để cập nhật biểu tượng khay.
static NOTIFY_HWND: AtomicIsize = AtomicIsize::new(0);

const ES_PASSWORD: i32 = 0x20;

thread_local! {
    static UIA: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
}

/// Ô đang được nhập có phải ô mật khẩu?
pub fn in_password() -> bool {
    IN_PASSWORD.load(Ordering::Relaxed)
}

/// Bắt đầu theo dõi. `notify` nhận thông điệp `WM_CINPUT_PASSWORD` (wParam = 1/0) khi trạng thái đổi.
pub fn start(notify: HWND) {
    NOTIFY_HWND.store(notify as isize, Ordering::Relaxed);
    std::thread::Builder::new()
        .name("c_input-password".into())
        .spawn(worker)
        .ok();
}

fn worker() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let uia: Option<IUIAutomation> = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok();
        UIA.with(|u| *u.borrow_mut() = uia);

        for ev in [EVENT_OBJECT_FOCUS, EVENT_SYSTEM_FOREGROUND] {
            SetWinEventHook(ev, ev, std::ptr::null_mut(), Some(on_event), 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
        }
        check();

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    check();
}

fn check() {
    let now = win32_password_focus() || uia_password_focus();
    let before = IN_PASSWORD.swap(now, Ordering::Relaxed);
    if before != now {
        let h = NOTIFY_HWND.load(Ordering::Relaxed) as HWND;
        if !h.is_null() {
            unsafe { PostMessageW(h, WM_CINPUT_PASSWORD, now as usize, 0) };
        }
    }
}

/// Ô Edit kiểu Win32 đang có con trỏ, mang kiểu ES_PASSWORD.
fn win32_password_focus() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return false;
        }
        let thread = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(thread, &mut info) == 0 || info.hwndFocus.is_null() {
            return false;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(info.hwndFocus, class.as_mut_ptr(), class.len() as i32);
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]).to_ascii_lowercase();
        // ES_PASSWORD chỉ có nghĩa với ô Edit (Edit, RichEdit, WindowsForms10.EDIT...).
        class.contains("edit") && GetWindowLongW(info.hwndFocus, GWL_STYLE) & ES_PASSWORD != 0
    }
}

/// Phần tử đang được chọn (theo UI Automation) là ô mật khẩu.
fn uia_password_focus() -> bool {
    UIA.with(|u| {
        let u = u.borrow();
        let Some(uia) = u.as_ref() else { return false };
        unsafe {
            uia.GetFocusedElement()
                .and_then(|e| e.CurrentIsPassword())
                .map(|b| b.as_bool())
                .unwrap_or(false)
        }
    })
}
