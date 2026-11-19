//! Tiến trình bộ gõ chạy nền: hook bàn phím cấp thấp, gửi phím đã biến đổi, biểu tượng khay hệ thống,
//! phím tắt toàn cục, chuyển mã clipboard, nhớ trạng thái Anh/Việt theo ứng dụng.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use cinput_charset::{Charset, convert, encode};
use cinput_core::{Edit, Engine, KeyOutcome, MacroTable};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::config::{self, Config, Method, SwitchKey};
use crate::password;
use crate::ai;
use crate::i18n::{fill, t};
use crate::win::{
    self, WM_CINPUT_CONVERT_CLIPBOARD, WM_CINPUT_PASSWORD, WM_CINPUT_RELOAD, WM_CINPUT_TRANSLATED, WM_CINPUT_UPDATE, wide,
};

/// Đánh dấu phím do chính C_Input gửi để hook bỏ qua.
const MAGIC: usize = 0x4349_4E50; // "CINP"
const WM_TRAY: u32 = WM_APP + 1;
const HOTKEY_CLIPBOARD: i32 = 1;
const HOTKEY_SETTINGS: i32 = 2;

const ID_TOGGLE: usize = 1;
const ID_METHOD: usize = 10; // + chỉ số
const ID_CHARSET: usize = 20; // + chỉ số
const ID_CLIPBOARD: usize = 30;
const ID_MACROS: usize = 31;
const ID_SETTINGS: usize = 32;
const ID_ELEVATE: usize = 40;
const ID_TRANSLATE: usize = 41;
const ID_READ: usize = 42;
const ID_EXIT: usize = 99;
const TIMER_WATCHDOG: usize = 1;

struct Service {
    hwnd: HWND,
    cfg: Config,
    macros: MacroTable,
    engine: Engine,
    icon_on: HICON,
    icon_off: HICON,
    cur_exe: String,
    app_states: HashMap<String, bool>,
    /// Đang giữ Ctrl + Shift mà chưa bấm phím nào khác.
    switch_armed: bool,
    taskbar_created: u32,
    kb_hook: HHOOK,
    mouse_hook: HHOOK,
    /// C_Input đang chạy với quyền Administrator.
    elevated_self: bool,
    /// Cửa sổ hiện tại thuộc ứng dụng quyền Admin mà C_Input không có quyền: không gõ được.
    cur_blocked: bool,
    /// Ứng dụng đã được cảnh báo (mỗi ứng dụng một lần).
    warned: HashSet<String>,
    // ---- Dịch AI theo câu
    /// Văn bản gõ liên tục kể từ lần cuối mất dấu con trỏ (bấm chuột, mũi tên, đổi cửa sổ...).
    typed: Vec<char>,
    /// Vị trí (trong `typed`) bắt đầu câu đang gõ.
    sent_start: usize,
    /// Các câu đang chờ bản dịch.
    pending: Vec<PendingTranslation>,
    next_id: u64,
    api_key: Option<String>,
    /// Lần báo lỗi dịch gần nhất (GetTickCount) — tránh báo dồn dập.
    last_tr_error: u32,
    /// Lần bấm Ctrl+C gần nhất (GetTickCount) — bấm hai lần liền thì mở cửa sổ dịch.
    last_ctrl_c: u32,
    /// Phím C đang được giữ (bỏ qua phím tự lặp).
    c_held: bool,
}

/// Một câu đã gửi đi dịch: vị trí trong `typed` và nội dung lúc gửi (để chắc chắn còn nguyên khi thay).
struct PendingTranslation {
    id: u64,
    start: usize,
    end: usize,
    original: String,
}

thread_local! {
    static STATE: RefCell<Option<Service>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut Service) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

fn key_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}

impl Service {
    fn active(&self) -> bool {
        self.cfg.vietnamese && !self.cfg.is_excluded(&self.cur_exe) && !self.cur_blocked && !self.in_password()
    }

    /// Con trỏ đang ở ô nhập mật khẩu (và người dùng bật tự tắt ở ô mật khẩu).
    fn in_password(&self) -> bool {
        self.cfg.password_off && password::in_password()
    }

    fn macros_for_word(&self) -> Option<&MacroTable> {
        let on = self.cfg.macro_enabled && (self.active() || self.cfg.macro_always);
        on.then_some(&self.macros)
    }

    fn apply_config(&mut self) {
        self.engine.opts = self.cfg.engine_options();
        self.reset_all();
        self.macros = config::load_macros(self.cfg.macro_auto_case);
        self.engine.english = config::load_english();
        self.engine.auto_capitalize = self.cfg.auto_capitalize;
        self.api_key = ai::unprotect_key(&self.cfg.gemini_key);
        crate::i18n::set_lang(&self.cfg.ui_lang);
        self.register_hotkeys();
        self.update_tray(false);
    }

    fn reload(&mut self) {
        self.cfg = Config::load();
        self.apply_config();
    }

    fn save(&self) {
        let _ = self.cfg.save();
    }

    fn toggle(&mut self) {
        self.cfg.vietnamese = !self.cfg.vietnamese;
        self.reset_all();
        if self.cfg.per_app && !self.cur_exe.is_empty() {
            self.app_states.insert(self.cur_exe.clone(), self.cfg.vietnamese);
        }
        if self.cfg.beep {
            unsafe { MessageBeep(if self.cfg.vietnamese { MB_ICONASTERISK } else { MB_OK }) };
        }
        self.update_tray(false);
        self.save();
    }

    fn register_hotkeys(&self) {
        unsafe {
            UnregisterHotKey(self.hwnd, HOTKEY_CLIPBOARD);
            UnregisterHotKey(self.hwnd, HOTKEY_SETTINGS);
            let mods = MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT;
            RegisterHotKey(self.hwnd, HOTKEY_CLIPBOARD, mods, VK_F9 as u32);
            RegisterHotKey(self.hwnd, HOTKEY_SETTINGS, mods, VK_F5 as u32);
        }
    }

    // ------------------------------------------------------------ khay hệ thống

    fn tray_data(&self) -> NOTIFYICONDATAW {
        let mut nid: NOTIFYICONDATAW = unsafe { zeroed() };
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;
        nid
    }

    fn tooltip(&self) -> String {
        if self.in_password() && self.cfg.vietnamese {
            return t("C_Input — đang nhập mật khẩu: tạm gõ tiếng Anh").to_string();
        }
        if self.cur_blocked && self.cfg.vietnamese {
            return fill(t("C_Input — không gõ được vào {} (ứng dụng chạy quyền Admin)"), &[&self.cur_exe]);
        }
        if self.cfg.is_excluded(&self.cur_exe) {
            return fill(t("C_Input — Tiếng Anh ({} luôn gõ tiếng Anh)"), &[&self.cur_exe]);
        }
        if self.cfg.vietnamese {
            fill(t("C_Input — Tiếng Việt\n{} · {}"), &[&t(self.cfg.method.label()), &t(self.cfg.charset.short())])
        } else {
            fill(t("C_Input — Tiếng Anh\nBấm {} để chuyển"), &[&self.cfg.switch_key.label()])
        }
    }

    fn update_tray(&self, add: bool) {
        let mut nid = self.tray_data();
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = if self.active() { self.icon_on } else { self.icon_off };
        copy_wstr(&mut nid.szTip, &self.tooltip());
        unsafe { Shell_NotifyIconW(if add { NIM_ADD } else { NIM_MODIFY }, &nid) };
    }

    fn notify(&self, title: &str, text: &str) {
        let mut nid = self.tray_data();
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = NIIF_INFO;
        copy_wstr(&mut nid.szInfoTitle, t(title));
        copy_wstr(&mut nid.szInfo, t(text));
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
    }

    fn convert_clipboard(&self) {
        let (from, to) = (self.cfg.clip_from, self.cfg.clip_to);
        match win::clipboard_text(self.hwnd) {
            Some(text) => {
                let out = convert(&text, from, to);
                if win::set_clipboard_text(self.hwnd, &out) {
                    self.notify("Đã chuyển mã clipboard", &format!("{} → {}", t(from.label()), t(to.label())));
                }
            }
            None => self.notify("Clipboard trống", "Hãy chép (Ctrl+C) đoạn văn bản cần chuyển mã trước."),
        }
    }

    // ------------------------------------------------------------ ứng dụng đang dùng

    fn on_foreground(&mut self, hwnd: HWND) {
        let exe = win::exe_name_of_window(hwnd).unwrap_or_default();
        let blocked = !self.elevated_self && win::window_elevated(hwnd);
        if exe == self.cur_exe && blocked == self.cur_blocked {
            return;
        }
        self.reset_all();
        self.cur_blocked = blocked;
        test_log(&format!("foreground: {exe} blocked={blocked}"));
        if blocked && self.cfg.vietnamese && !exe.is_empty() && self.warned.insert(exe.clone()) {
            self.notify(
                &fill(t("Không gõ được tiếng Việt trong {}"), &[&exe]),
                "Ứng dụng này chạy với quyền Administrator. Chuột phải biểu tượng C_Input → \
                 \"Chạy với quyền Administrator\" để gõ được.",
            );
        }
        if self.cfg.per_app {
            if !self.cur_exe.is_empty() {
                self.app_states.insert(self.cur_exe.clone(), self.cfg.vietnamese);
            }
            if let Some(&v) = self.app_states.get(&exe) {
                self.cfg.vietnamese = v;
            }
        }
        self.cur_exe = exe;
        self.update_tray(false);
    }

    // ------------------------------------------------------------ bàn phím

    /// Trả về `true` nếu chặn phím này.
    fn on_key(&mut self, kb: &KBDLLHOOKSTRUCT, down: bool) -> bool {
        let vk = kb.vkCode as VIRTUAL_KEY;
        if accept_injected() && vk == VK_F24 && down {
            // Kiểm thử: giả lập Windows gỡ mất hook.
            test_log("test: gỡ hook");
            unsafe {
                UnhookWindowsHookEx(self.kb_hook);
                UnhookWindowsHookEx(self.mouse_hook);
            }
            self.kb_hook = null_mut();
            self.mouse_hook = null_mut();
            return true;
        }
        let is_ctrl = matches!(vk, VK_CONTROL | VK_LCONTROL | VK_RCONTROL);
        let is_shift = matches!(vk, VK_SHIFT | VK_LSHIFT | VK_RSHIFT);

        // Phím chuyển Ctrl + Shift: bấm rồi nhả mà không kèm phím nào khác.
        if self.cfg.switch_key == SwitchKey::CtrlShift {
            if down {
                if is_ctrl || is_shift {
                    let other = if is_ctrl { key_down(VK_SHIFT) } else { key_down(VK_CONTROL) };
                    if other && !key_down(VK_MENU) && !key_down(VK_LWIN) && !key_down(VK_RWIN) {
                        self.switch_armed = true;
                    }
                } else {
                    self.switch_armed = false;
                }
            } else if (is_ctrl || is_shift) && self.switch_armed {
                self.switch_armed = false;
                self.toggle();
            }
        }
        if !down && vk == VK_C {
            self.c_held = false;
        }
        if !down {
            return false;
        }
        let ctrl = key_down(VK_CONTROL);
        let alt = key_down(VK_MENU);
        if self.cfg.switch_key == SwitchKey::AltZ && vk == VK_Z && alt && !ctrl {
            self.toggle();
            return true;
        }
        // Bôi đen rồi bấm Ctrl+C hai lần (trong 0,5 giây) → cửa sổ dịch đoạn vừa chép.
        if vk == VK_C && ctrl && !alt && self.cfg.read_enabled {
            if !self.c_held {
                let now = unsafe { GetTickCount() };
                if self.last_ctrl_c != 0 && now.wrapping_sub(self.last_ctrl_c) < 500 {
                    self.last_ctrl_c = 0;
                    self.open_reader(true);
                } else {
                    self.last_ctrl_c = now;
                }
            }
            self.c_held = true;
        }
        if ctrl || alt || key_down(VK_LWIN) || key_down(VK_RWIN) {
            if !is_ctrl && !is_shift && !matches!(vk, VK_MENU | VK_LMENU | VK_RMENU) {
                self.reset_all();
            }
            return false;
        }

        if self.in_password() || self.cfg.is_excluded(&self.cur_exe) || self.cur_blocked {
            // Ô mật khẩu / ứng dụng luôn gõ tiếng Anh / không gửi phím được: không bỏ dấu, không gõ tắt,
            // không tự viết hoa — gõ ra đúng phím đã bấm.
            self.reset_all();
            return false;
        }
        let active = self.active();
        if !active && self.macros_for_word().is_none() && !self.cfg.auto_capitalize && !self.cfg.translate_enabled {
            return false;
        }
        if self.engine.vietnamese != active {
            self.engine.vietnamese = active;
            self.reset_all();
        }

        let shift = key_down(VK_SHIFT);
        let caps = unsafe { GetKeyState(VK_CAPITAL as i32) } & 1 != 0;
        let ch: Option<char> = match vk {
            0x41..=0x5A => {
                let c = (b'a' + (vk - 0x41) as u8) as char;
                Some(if shift ^ caps { c.to_ascii_uppercase() } else { c })
            }
            0x30..=0x39 if !shift => Some((b'0' + (vk - 0x30) as u8) as char),
            VK_OEM_4 => Some(if shift { '{' } else { '[' }),
            VK_OEM_6 => Some(if shift { '}' } else { ']' }),
            // Dấu câu: phím dấu của VIQR / kiểu gõ tự định nghĩa (nếu không thì là dấu ngắt từ).
            VK_SPACE | VK_RETURN | VK_TAB => None,
            _ => typed_char(vk, shift),
        };
        if let Some(c) = ch {
            if self.engine.is_word_char(c) {
                return match self.engine.key(c) {
                    KeyOutcome::Pass => {
                        self.typed.push(c);
                        false
                    }
                    KeyOutcome::Replace(edit) => {
                        self.typed_apply(&edit);
                        self.send(&edit, None);
                        true
                    }
                };
            }
        }
        match vk {
            VK_BACK => {
                self.engine.backspace();
                self.typed.pop();
                self.typed_changed();
                false
            }
            VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CAPITAL => false,
            _ if ch.is_some() || is_separator(vk) => {
                let use_macros = self.macros_for_word().is_some();
                let macros = if use_macros { Some(&self.macros) } else { None };
                let before = self.engine.current().to_string();
                let edit = self.engine.end_word(macros);
                if let Some(e) = &edit {
                    self.typed_apply(e);
                }
                test_log(&format!("end_word {before:?} -> {edit:?}"));
                if matches!(vk, VK_RETURN | VK_TAB) {
                    // Enter có thể đã gửi tin nhắn, Tab chuyển ô: không xoá lùi về từ trước được nữa.
                    self.engine.forget_history();
                }
                let sep = separator_char(vk, shift);
                self.engine.separator(sep);
                match typed_char(vk, shift) {
                    // Enter / Tab / phím lạ: không theo dõi tiếp được (Enter có thể đã gửi tin nhắn).
                    Some(c) if !matches!(vk, VK_RETURN | VK_TAB) => self.typed.push(c),
                    _ => self.forget_typed(),
                }
                if sep == ' ' && self.engine.at_sentence_start() {
                    self.sentence_finished();
                }
                match edit {
                    Some(edit) => {
                        self.send(&edit, Some(kb));
                        true
                    }
                    None => false,
                }
            }
            VK_ESCAPE if self.cfg.restore_key => match self.engine.restore() {
                Some(edit) => {
                    self.typed_apply(&edit);
                    self.send(&edit, None);
                    true
                }
                None => {
                    self.reset_all();
                    false
                }
            },
            _ => {
                self.reset_all();
                false
            }
        }
    }

    /// Gửi phép sửa: Backspace rồi chuỗi ký tự (theo bảng mã đang chọn); nếu `then_key`
    /// thì gửi lại phím vừa bị chặn (dấu cách, Enter...) ngay sau đó.
    fn send(&mut self, edit: &Edit, then_key: Option<&KBDLLHOOKSTRUCT>) {
        let cs = self.cfg.charset;
        let mut deletes = encode(&edit.delete, cs).encode_utf16().count();
        let text = encode(&edit.insert, cs);
        let mut inputs: Vec<INPUT> = Vec::with_capacity(deletes * 2 + text.len() * 2 + 6);
        if deletes > 0 && self.cfg.needs_browser_fix(&self.cur_exe) {
            // Ô có gợi ý tự động (thanh địa chỉ, Excel) đang bôi đen phần gợi ý: phím xoá đầu tiên
            // sẽ chỉ xoá phần bôi đen. Gõ trước một ký tự vô hình để thay phần bôi đen, rồi xoá thêm nó.
            inputs.push(key_input(0, 0x202F, KEYEVENTF_UNICODE));
            inputs.push(key_input(0, 0x202F, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
            deletes += 1;
        }
        for _ in 0..deletes {
            inputs.push(key_input(VK_BACK, 0x0E, 0));
            inputs.push(key_input(VK_BACK, 0x0E, KEYEVENTF_KEYUP));
        }
        let via_clipboard = !text.is_empty() && self.cfg.uses_clipboard(&self.cur_exe);
        if via_clipboard {
            // Phần mềm cũ không nhận ký tự Unicode từ phím ảo: dán chữ qua clipboard rồi trả lại
            // nội dung clipboard cũ của người dùng.
            let saved = win::clipboard_text(self.hwnd);
            if win::set_clipboard_text(self.hwnd, &text) {
                inputs.push(key_input(VK_CONTROL, 0, 0));
                inputs.push(key_input(VK_V, 0, 0));
                inputs.push(key_input(VK_V, 0, KEYEVENTF_KEYUP));
                inputs.push(key_input(VK_CONTROL, 0, KEYEVENTF_KEYUP));
                let hwnd = self.hwnd as isize;
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    if let Some(old) = saved {
                        win::set_clipboard_text(hwnd as HWND, &old);
                    }
                });
            }
        } else {
            for unit in text.encode_utf16() {
                inputs.push(key_input(0, unit, KEYEVENTF_UNICODE));
                inputs.push(key_input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
            }
        }
        if let Some(kb) = then_key {
            let ext = if kb.flags & LLKHF_EXTENDED != 0 { KEYEVENTF_EXTENDEDKEY } else { 0 };
            inputs.push(key_input(kb.vkCode as VIRTUAL_KEY, kb.scanCode as u16, ext));
        }
        let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
        if (sent as usize) < inputs.len() {
            // Windows chặn gửi phím (cửa sổ quyền cao hơn, màn hình UAC...): ngừng bỏ dấu ở đây.
            self.cur_blocked = true;
            self.reset_all();
            self.update_tray(false);
        }
    }

    // ------------------------------------------------------------ dịch AI theo câu

    /// Bỏ từ đang gõ và mọi thứ đang theo dõi (con trỏ có thể đã sang chỗ khác).
    fn reset_all(&mut self) {
        self.engine.reset();
        self.forget_typed();
    }

    fn forget_typed(&mut self) {
        self.typed.clear();
        self.sent_start = 0;
        self.pending.clear();
    }

    /// Áp dụng một phép sửa vào bản theo dõi `typed`.
    fn typed_apply(&mut self, e: &Edit) {
        for _ in e.delete.chars() {
            self.typed.pop();
        }
        self.typed.extend(e.insert.chars());
        self.typed_changed();
    }

    /// `typed` vừa ngắn đi: bỏ các câu chờ dịch không còn nguyên vẹn.
    fn typed_changed(&mut self) {
        let len = self.typed.len();
        self.pending.retain(|p| p.end <= len);
        self.sent_start = self.sent_start.min(len);
        if self.typed.len() > 20_000 {
            self.forget_typed();
        }
    }

    /// Vừa gõ xong một câu: gửi đi dịch (luồng nền), câu sau gõ tiếp bình thường.
    fn sentence_finished(&mut self) {
        let (start, end) = (self.sent_start, self.typed.len());
        self.sent_start = end;
        test_log(&format!(
            "sentence_finished {start}..{end} enabled={} active={} text={:?}",
            self.cfg.translate_enabled,
            self.active(),
            self.typed.iter().collect::<String>()
        ));
        if !self.cfg.translate_enabled || !self.active() || start >= end {
            return;
        }
        let original: String = self.typed[start..end].iter().collect();
        let text = original.trim().to_string();
        if !text.chars().any(char::is_alphabetic) {
            return;
        }
        // Kiểm thử tự động: bộ dịch giả (chờ như gọi mạng, trả "<CÂU VIẾT HOA>") — không cần API key.
        let fake = accept_injected() && std::env::var_os("CINPUT_TEST_FAKE_TRANSLATE").is_some();
        let key = match self.api_key.clone() {
            Some(k) => k,
            None if fake => String::new(),
            None => {
                self.translate_error("Chưa có API key Gemini — mở Cài đặt → Dịch AI để thêm.");
                return;
            }
        };
        let id = self.next_id;
        self.next_id += 1;
        self.pending.push(PendingTranslation { id, start, end, original });
        let (model, lang, hwnd) = (self.cfg.gemini_model.clone(), self.cfg.translate_lang.clone(), self.hwnd as isize);
        std::thread::spawn(move || {
            let result = if fake {
                std::thread::sleep(std::time::Duration::from_millis(800));
                Ok(format!("<{}>", text.to_uppercase()))
            } else {
                ai::translate(&key, &model, &lang, &text)
            };
            let boxed = Box::into_raw(Box::new((id, result)));
            unsafe {
                if PostMessageW(hwnd as HWND, WM_CINPUT_TRANSLATED, 0, boxed as isize) == 0 {
                    drop(Box::from_raw(boxed));
                }
            }
        });
    }

    /// Bản dịch về: thay đúng câu cũ (nếu còn nguyên ở chỗ cũ), giữ phần đã gõ tiếp sau đó.
    fn on_translated(&mut self, id: u64, result: Result<String, String>) {
        let Some(i) = self.pending.iter().position(|p| p.id == id) else { return };
        let p = self.pending.remove(i);
        let translation = match result {
            Ok(t) => t,
            Err(e) => {
                self.translate_error(&e);
                return;
            }
        };
        let intact = p.end <= self.typed.len() && self.typed[p.start..p.end].iter().copied().eq(p.original.chars());
        if !intact || !self.active() {
            return;
        }
        // Giữ khoảng trắng đầu / cuối câu như người dùng đã gõ; bản dịch nằm một dòng.
        let lead: String = p.original.chars().take_while(|c| c.is_whitespace()).collect();
        let trail: String = {
            let t: Vec<char> = p.original.chars().rev().take_while(|c| c.is_whitespace()).collect();
            t.into_iter().rev().collect()
        };
        let body = translation.split_whitespace().collect::<Vec<_>>().join(" ");
        let replacement: Vec<char> = format!("{lead}{body}{trail}").chars().collect();
        let tail: String = self.typed[p.end..].iter().collect();
        let edit = Edit {
            delete: self.typed[p.start..].iter().collect(),
            insert: replacement.iter().collect::<String>() + &tail,
        };
        self.send(&edit, None);
        // Cập nhật bản theo dõi và vị trí các câu khác đang chờ.
        let old_len = p.end - p.start;
        self.typed.splice(p.start..p.end, replacement.iter().copied());
        let shift = replacement.len() as isize - old_len as isize;
        for q in self.pending.iter_mut().filter(|q| q.start >= p.end) {
            q.start = (q.start as isize + shift) as usize;
            q.end = (q.end as isize + shift) as usize;
        }
        if self.sent_start >= p.end {
            self.sent_start = (self.sent_start as isize + shift) as usize;
        }
        // Các từ đã nhớ để xoá lùi về không còn trên màn hình.
        self.engine.forget_history();
    }

    /// Mở cửa sổ dịch cho nội dung clipboard, đặt cạnh con trỏ chuột.
    /// `after_copy`: vừa bấm Ctrl+C — chờ ứng dụng chép xong vào clipboard.
    fn open_reader(&mut self, after_copy: bool) {
        if self.in_password() {
            return;
        }
        // Chỉ giữ một cửa sổ dịch.
        let title = wide(crate::popup::POPUP_TITLE);
        unsafe {
            let old = FindWindowW(null(), title.as_ptr());
            if !old.is_null() {
                PostMessageW(old, WM_CLOSE, 0, 0);
            }
        }
        let (x, y) = reader_position();
        std::thread::spawn(move || {
            if after_copy {
                std::thread::sleep(std::time::Duration::from_millis(180));
            }
            let Some(text) = win::clipboard_text(null_mut()) else { return };
            if text.trim().is_empty() {
                return;
            }
            let path = std::env::temp_dir().join(format!("c_input_sel_{}.txt", unsafe { GetTickCount() }));
            if std::fs::write(&path, text).is_err() {
                return;
            }
            unsafe { AllowSetForegroundWindow(u32::MAX) }; // cho cửa sổ dịch được lên trước
            win::spawn_self(&["--popup", &path.to_string_lossy(), &x.to_string(), &y.to_string()]);
        });
    }

    fn translate_error(&mut self, msg: &str) {
        let now = unsafe { GetTickCount() };
        if self.last_tr_error == 0 || now.wrapping_sub(self.last_tr_error) > 30_000 {
            self.last_tr_error = now.max(1);
            self.notify("Dịch AI không thành công", msg);
        }
    }

    // ------------------------------------------------------------ hook

    fn install_hooks(&mut self) {
        unsafe {
            if !self.kb_hook.is_null() {
                UnhookWindowsHookEx(self.kb_hook);
            }
            if !self.mouse_hook.is_null() {
                UnhookWindowsHookEx(self.mouse_hook);
            }
            let hinst = GetModuleHandleW(null());
            self.kb_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hinst, 0);
            self.mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hinst, 0);
            touch_hook();
        }
    }

    /// Windows âm thầm gỡ hook cấp thấp nếu chương trình phản hồi chậm (máy treo, ngủ, khoá màn hình...).
    /// Mỗi giây so thời điểm có thao tác gần nhất với lần cuối hook nhận sự kiện: bình thường hook luôn nhận
    /// sự kiện ngay sau thao tác, nên thao tác mới hơn 250ms mà hook không thấy nghĩa là hook đã mất → cài lại.
    fn watchdog(&mut self) {
        let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return;
        }
        let last = LAST_HOOK_TICK.load(std::sync::atomic::Ordering::Relaxed);
        if info.dwTime.wrapping_sub(last) as i32 > 250 {
            // Chỉ cài lại hook; không xoá từ đang gõ / câu đang chờ dịch (có thể chỉ là báo nhầm).
            test_log(&format!("watchdog: cài lại hook (thao tác lúc {}, hook lúc {last})", info.dwTime));
            self.install_hooks();
        }
    }
}

/// Ký tự của dấu ngắt từ (bàn phím US) — để nhận ra ranh giới câu khi tự viết hoa.
fn separator_char(vk: VIRTUAL_KEY, shift: bool) -> char {
    match (vk, shift) {
        (VK_SPACE, _) => ' ',
        (VK_RETURN, _) => '\n',
        (VK_OEM_PERIOD, false) | (VK_DECIMAL, _) => '.',
        (0x31, true) => '!',
        (VK_OEM_2, true) => '?',
        (VK_OEM_7, false) => '\'',
        (VK_OEM_7, true) => '"',
        (0x30, true) => ')',
        (VK_OEM_6, false) => ']',
        _ => '\0',
    }
}

/// Tự kiểm tra bản mới trên GitHub (nếu người dùng bật): 30 giây sau khi khởi động, rồi mỗi ngày.
/// Chỉ báo một lần cho mỗi phiên bản; không tự cài.
fn start_update_checker(hwnd: HWND) {
    let hwnd = hwnd as isize;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(30));
        let mut told = String::new();
        loop {
            if Config::load().auto_update {
                if let Ok(Some(r)) = crate::update::check() {
                    if r.version != told {
                        told = r.version.clone();
                        let boxed = Box::into_raw(Box::new(r.version));
                        unsafe {
                            if PostMessageW(hwnd as HWND, crate::win::WM_CINPUT_UPDATE, 0, boxed as isize) == 0 {
                                drop(Box::from_raw(boxed));
                            }
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(24 * 3600));
        }
    });
}

/// Vị trí (điểm logic, đã chia tỉ lệ DPI) cho cửa sổ dịch: cạnh con trỏ, không tràn khỏi màn hình.
fn reader_position() -> (i32, i32) {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint};
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = zeroed();
        mi.cbSize = size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(mon, &mut mi);
        let (mut dx, mut dy) = (96u32, 96u32);
        GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        let scale = dx.max(96) as f32 / 96.0;
        let w = (crate::popup::POPUP_SIZE[0] * scale) as i32;
        let h = (crate::popup::POPUP_SIZE[1] * scale) as i32;
        let wa = mi.rcWork;
        let mut x = (pt.x + 14).min(wa.right - w - 8).max(wa.left + 8);
        let mut y = pt.y + 20;
        if y + h > wa.bottom - 8 {
            y = pt.y - h - 14; // không đủ chỗ phía dưới: hiện phía trên con trỏ
        }
        y = y.max(wa.top + 8);
        x = (x as f32 / scale) as i32;
        y = (y as f32 / scale) as i32;
        (x, y)
    }
}

/// Ký tự mà một phím ngắt từ gõ ra (bàn phím US) — để theo dõi văn bản cho tính năng dịch.
fn typed_char(vk: VIRTUAL_KEY, shift: bool) -> Option<char> {
    const DIGITS_SHIFT: &[u8; 10] = b")!@#$%^&*(";
    Some(match (vk, shift) {
        (VK_SPACE, _) => ' ',
        (0x30..=0x39, false) => (b'0' + (vk - 0x30) as u8) as char,
        (0x30..=0x39, true) => DIGITS_SHIFT[(vk - 0x30) as usize] as char,
        (0x60..=0x69, _) => (b'0' + (vk - 0x60) as u8) as char,
        (VK_MULTIPLY, _) => '*',
        (VK_ADD, _) => '+',
        (VK_SUBTRACT, _) => '-',
        (VK_DECIMAL, _) => '.',
        (VK_DIVIDE, _) => '/',
        (VK_OEM_1, s) => if s { ':' } else { ';' },
        (VK_OEM_2, s) => if s { '?' } else { '/' },
        (VK_OEM_3, s) => if s { '~' } else { '`' },
        (VK_OEM_4, s) => if s { '{' } else { '[' },
        (VK_OEM_5, s) => if s { '|' } else { '\\' },
        (VK_OEM_6, s) => if s { '}' } else { ']' },
        (VK_OEM_7, s) => if s { '"' } else { '\'' },
        (VK_OEM_PLUS, s) => if s { '+' } else { '=' },
        (VK_OEM_COMMA, s) => if s { '<' } else { ',' },
        (VK_OEM_MINUS, s) => if s { '_' } else { '-' },
        (VK_OEM_PERIOD, s) => if s { '>' } else { '.' },
        _ => return None,
    })
}

fn is_separator(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_SPACE
            | VK_RETURN
            | VK_TAB
            | VK_OEM_1
            | VK_OEM_2
            | VK_OEM_3
            | VK_OEM_4
            | VK_OEM_5
            | VK_OEM_6
            | VK_OEM_7
            | VK_OEM_8
            | VK_OEM_102
            | VK_OEM_PLUS
            | VK_OEM_COMMA
            | VK_OEM_MINUS
            | VK_OEM_PERIOD
            | 0x30..=0x39
            | 0x60..=0x6F
    )
}

fn key_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: MAGIC } },
    }
}

fn copy_wstr(dst: &mut [u16], s: &str) {
    let n = dst.len() - 1;
    for (i, u) in s.encode_utf16().take(n).enumerate() {
        dst[i] = u;
    }
    dst[s.encode_utf16().count().min(n)] = 0;
}

// ---------------------------------------------------------------- hook

/// Chỉ dùng khi kiểm thử tự động (CINPUT_TEST_INJECTED=1): xử lý cả phím do chương trình khác gửi.
fn accept_injected() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("CINPUT_TEST_INJECTED").is_some())
}

/// Nhật ký kiểm thử (%TEMP%\cinput_test.log) — chỉ ghi ở chế độ kiểm thử.
fn test_log(line: &str) {
    if !accept_injected() {
        return;
    }
    use std::io::Write;
    let path = std::env::temp_dir().join("cinput_test.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

/// Thời điểm (GetTickCount) hook nhận sự kiện gần nhất — biến nguyên tử để luôn ghi được,
/// kể cả khi trạng thái bộ gõ đang bận (nếu không, watchdog sẽ tưởng nhầm hook đã mất).
static LAST_HOOK_TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn touch_hook() {
    LAST_HOOK_TICK.store(unsafe { GetTickCount() }, std::sync::atomic::Ordering::Relaxed);
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        touch_hook();
        let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        if kb.dwExtraInfo != MAGIC && (kb.flags & LLKHF_INJECTED == 0 || accept_injected()) {
            let msg = wparam as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            if with_state(|s| s.on_key(kb, down)).unwrap_or(false) {
                return 1;
            }
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        touch_hook();
        if matches!(wparam as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
            with_state(|s| s.reset_all());
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    with_state(|s| s.on_foreground(hwnd));
}

// ---------------------------------------------------------------- cửa sổ ẩn + menu

fn show_menu(hwnd: HWND) {
    let Some((cfg, active, elevated)) = with_state(|s| (s.cfg.clone(), s.active(), s.elevated_self)) else { return };
    unsafe {
        let menu = CreatePopupMenu();
        let item = |m: HMENU, id: usize, text: &str, checked: bool| {
            let w = wide(t(text));
            AppendMenuW(m, MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED }, id, w.as_ptr());
        };
        let sep = |m: HMENU| {
            AppendMenuW(m, MF_SEPARATOR, 0, null());
        };
        item(menu, ID_TOGGLE, &fill(t("Gõ tiếng Việt\t{}"), &[&cfg.switch_key.label()]), active);
        sep(menu);

        let methods = CreatePopupMenu();
        for (i, m) in Method::ALL.iter().enumerate() {
            item(methods, ID_METHOD + i, t(m.label()), *m == cfg.method);
        }
        let charsets = CreatePopupMenu();
        for (i, c) in Charset::TYPING.iter().enumerate() {
            item(charsets, ID_CHARSET + i, t(c.label()), *c == cfg.charset);
        }
        let label = wide(&fill(t("Kiểu gõ: {}"), &[&t(cfg.method.label())]));
        AppendMenuW(menu, MF_POPUP, methods as usize, label.as_ptr());
        let label = wide(&fill(t("Bảng mã: {}"), &[&t(cfg.charset.label())]));
        AppendMenuW(menu, MF_POPUP, charsets as usize, label.as_ptr());
        sep(menu);
        item(
            menu,
            ID_TRANSLATE,
            &fill(t("Dịch AI tự động → {}"), &[&ai::language_label(&cfg.translate_lang)]),
            cfg.translate_enabled,
        );
        item(menu, ID_READ, "Dịch nội dung clipboard…", false);
        item(menu, ID_CLIPBOARD, "Chuyển mã clipboard\tCtrl+Shift+F9", false);
        item(menu, ID_MACROS, "Bảng gõ tắt…", false);
        item(menu, ID_SETTINGS, "Cài đặt…\tCtrl+Shift+F5", false);
        sep(menu);
        if !elevated {
            item(menu, ID_ELEVATE, "Chạy với quyền Administrator", false);
        }
        item(menu, ID_EXIT, "Thoát C_Input", false);
        SetMenuDefaultItem(menu, ID_SETTINGS as u32, 0);

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RIGHTALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        if cmd > 0 {
            on_command(hwnd, cmd as usize);
        }
    }
}

fn open_settings(page: Option<&str>) {
    let title = wide(win::SETTINGS_TITLE);
    let existing = unsafe { FindWindowW(null(), title.as_ptr()) };
    if !existing.is_null() {
        unsafe {
            ShowWindow(existing, SW_RESTORE);
            SetForegroundWindow(existing);
        }
        return;
    }
    match page {
        Some(p) => win::spawn_self(&["--settings", p]),
        None => win::spawn_self(&["--settings"]),
    }
}

fn on_command(hwnd: HWND, id: usize) {
    match id {
        ID_TOGGLE => {
            with_state(|s| s.toggle());
        }
        ID_CLIPBOARD => {
            with_state(|s| s.convert_clipboard());
        }
        ID_MACROS => open_settings(Some("macros")),
        ID_SETTINGS => open_settings(None),
        ID_EXIT => unsafe {
            DestroyWindow(hwnd);
        },
        ID_READ => {
            with_state(|s| s.open_reader(false));
        }
        ID_TRANSLATE => {
            with_state(|s| {
                s.cfg.translate_enabled = !s.cfg.translate_enabled;
                s.forget_typed();
                if s.cfg.translate_enabled && s.api_key.is_none() {
                    s.notify("Chưa có API key Gemini", "Mở Cài đặt → Dịch AI để thêm API key.");
                }
                s.save();
            });
        }
        ID_ELEVATE => {
            // Bản mới (quyền Admin) sẽ chờ bản này thoát rồi mới chạy.
            if win::run_elevated("--replace") {
                unsafe { DestroyWindow(hwnd) };
            }
        }
        _ if (ID_METHOD..ID_METHOD + Method::ALL.len()).contains(&id) => {
            with_state(|s| {
                s.cfg.method = Method::ALL[id - ID_METHOD];
                s.engine.opts = s.cfg.engine_options();
                s.reset_all();
                s.update_tray(false);
                s.save();
            });
        }
        _ if (ID_CHARSET..ID_CHARSET + Charset::TYPING.len()).contains(&id) => {
            with_state(|s| {
                s.cfg.charset = Charset::TYPING[id - ID_CHARSET];
                s.reset_all();
                s.update_tray(false);
                s.save();
            });
        }
        _ => {}
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            match (lparam as u32) & 0xFFFF {
                WM_LBUTTONUP => {
                    with_state(|s| s.toggle());
                }
                WM_LBUTTONDBLCLK => open_settings(None),
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            on_command(hwnd, wparam & 0xFFFF);
            0
        }
        WM_HOTKEY => {
            match wparam as i32 {
                HOTKEY_CLIPBOARD => {
                    with_state(|s| s.convert_clipboard());
                }
                HOTKEY_SETTINGS => open_settings(None),
                _ => {}
            }
            0
        }
        WM_TIMER if wparam == TIMER_WATCHDOG => {
            with_state(|s| s.watchdog());
            0
        }
        WM_CINPUT_RELOAD => {
            with_state(|s| s.reload());
            0
        }
        WM_CINPUT_UPDATE => {
            let v = unsafe { Box::from_raw(lparam as *mut String) };
            with_state(|s| {
                s.notify(
                    &fill(t("Đã có C_Input {}"), &[&v]),
                    "Mở Cài đặt → Giới thiệu → Cập nhật để cài bản mới (kiểm tra SHA-256 trước khi cài).",
                )
            });
            0
        }
        WM_CINPUT_TRANSLATED => {
            let boxed = unsafe { Box::from_raw(lparam as *mut (u64, Result<String, String>)) };
            let (id, result) = *boxed;
            with_state(|s| s.on_translated(id, result));
            0
        }
        WM_CINPUT_PASSWORD => {
            // Vào / ra ô mật khẩu: đổi biểu tượng khay (xám khi đang nhập mật khẩu).
            with_state(|s| {
                s.reset_all();
                s.update_tray(false);
            });
            0
        }
        WM_CINPUT_CONVERT_CLIPBOARD => {
            with_state(|s| s.convert_clipboard());
            0
        }
        WM_DESTROY => {
            with_state(|s| unsafe {
                let nid = s.tray_data();
                Shell_NotifyIconW(NIM_DELETE, &nid);
                KillTimer(s.hwnd, TIMER_WATCHDOG);
                if !s.kb_hook.is_null() {
                    UnhookWindowsHookEx(s.kb_hook);
                }
                if !s.mouse_hook.is_null() {
                    UnhookWindowsHookEx(s.mouse_hook);
                }
                s.kb_hook = null_mut();
                s.mouse_hook = null_mut();
            });
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => {
            if with_state(|s| s.taskbar_created == msg && msg != 0) == Some(true) {
                with_state(|s| s.update_tray(true));
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

fn load_icon(id: u16) -> HICON {
    unsafe {
        let cx = GetSystemMetrics(SM_CXSMICON);
        let cy = GetSystemMetrics(SM_CYSMICON);
        LoadImageW(GetModuleHandleW(null()), id as usize as *const u16, IMAGE_ICON, cx, cy, LR_DEFAULTCOLOR)
    }
}

/// Chạy bộ gõ. Trả về khi người dùng chọn Thoát.
/// `replace = true`: bản này thay cho bản đang chạy (vd: chạy lại với quyền Admin) — chờ bản cũ thoát.
pub fn run(replace: bool) {
    let mutex_name = wide("Local\\C_Input.Service");
    unsafe {
        let mut tries = 0;
        loop {
            let m = CreateMutexW(null(), 1, mutex_name.as_ptr());
            if GetLastError() != ERROR_ALREADY_EXISTS {
                break;
            }
            CloseHandle(m);
            tries += 1;
            if !replace || tries > 40 {
                // Bộ gõ đang chạy: mở cửa sổ cài đặt thay vì chạy bản thứ hai.
                open_settings(None);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }

    let first_run = !config::config_path().exists();
    let cfg = Config::load();
    if first_run {
        let _ = cfg.save();
        let _ = config::save_macros(&config::load_macros(cfg.macro_auto_case));
    }

    unsafe {
        let hinst = GetModuleHandleW(null());
        let class = wide(win::SERVICE_CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst,
            lpszClassName: class.as_ptr(),
            hIcon: LoadIconW(hinst, 1 as *const u16),
            ..zeroed()
        };
        RegisterClassW(&wc);
        let title = wide("C_Input");
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), WS_OVERLAPPED, 0, 0, 0, 0, null_mut(), null_mut(), hinst, null());

        let taskbar = wide("TaskbarCreated");
        let mut engine = Engine::new(cfg.engine_options());
        engine.english = config::load_english();
        engine.auto_capitalize = cfg.auto_capitalize;
        let api_key = ai::unprotect_key(&cfg.gemini_key);
        crate::i18n::set_lang(&cfg.ui_lang);
        engine.vietnamese = cfg.vietnamese;
        let svc = Service {
            hwnd,
            macros: config::load_macros(cfg.macro_auto_case),
            engine,
            cfg,
            icon_on: load_icon(101),
            icon_off: load_icon(102),
            cur_exe: String::new(),
            app_states: HashMap::new(),
            switch_armed: false,
            taskbar_created: RegisterWindowMessageW(taskbar.as_ptr()),
            kb_hook: null_mut(),
            mouse_hook: null_mut(),
            elevated_self: win::self_elevated(),
            cur_blocked: false,
            warned: HashSet::new(),
            typed: Vec::new(),
            sent_start: 0,
            pending: Vec::new(),
            next_id: 1,
            api_key,
            last_tr_error: 0,
            last_ctrl_c: 0,
            c_held: false,
        };
        // Khi chạy quyền Admin, Windows chặn thông điệp từ chương trình thường (cửa sổ cài đặt,
        // Explorer báo dựng lại khay) — cho phép riêng các thông điệp này.
        for m in [WM_CINPUT_RELOAD, WM_CINPUT_CONVERT_CLIPBOARD, svc.taskbar_created] {
            ChangeWindowMessageFilterEx(hwnd, m, MSGFLT_ALLOW, null_mut());
        }
        svc.register_hotkeys();
        svc.update_tray(true);
        STATE.with(|s| *s.borrow_mut() = Some(svc));

        with_state(|s| s.install_hooks());
        crate::update::cleanup_old();
        start_update_checker(hwnd);
        password::start(hwnd);
        SetTimer(hwnd, TIMER_WATCHDOG, 1000, None);
        let fg_hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(foreground_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        with_state(|s| s.on_foreground(GetForegroundWindow()));

        if first_run {
            open_settings(None);
        }

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnhookWinEvent(fg_hook);
        STATE.with(|s| *s.borrow_mut() = None);
    }
}
