//! C_Input — bộ gõ tiếng Việt cho Windows.
//!
//! `C_Input.exe`            chạy bộ gõ nền (khay hệ thống).
//! `C_Input.exe --settings` mở cửa sổ cài đặt (tiến trình riêng).

#![windows_subsystem = "windows"]

mod ai;
mod config;
mod i18n;
mod password;
mod popup;
mod update;
mod service;
mod settings;
mod ui;
mod win;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--settings") => settings::run(args.get(1).cloned()),
        Some("--replace") => service::run(true),
        Some("--popup") => popup::run(&args[1..]),
        _ => service::run(false),
    }
}
