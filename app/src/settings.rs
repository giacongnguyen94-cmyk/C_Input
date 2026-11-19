//! Cửa sổ cài đặt C_Input (chạy như một tiến trình riêng: `C_Input.exe --settings`).
//! Mọi thay đổi được lưu ngay và báo cho tiến trình bộ gõ đọc lại.

use std::time::{Duration, Instant, SystemTime};

use cinput_charset::{Charset, decode, encode, remove_tones};
use cinput_core::MacroTable;
use eframe::egui::{
    self, Align, ColorImage, CornerRadius, FontId, Frame, Layout, Margin, RichText, ScrollArea, Stroke, TextEdit,
    TextureHandle, TextureOptions, Ui, vec2,
};

use crate::config::{self, Config, Method, SwitchKey};
use crate::ui::{self, icon};
use crate::win;

const LOGO_PNG: &[u8] = include_bytes!("../res/logo.png");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Typing,
    Macros,
    Convert,
    Apps,
    Ai,
    About,
}

/// Kết quả các thao tác mạng chạy nền của trang Dịch AI.
enum AiMsg {
    Models(Result<Vec<String>, String>),
    Translated(Result<String, String>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaseMode {
    Keep,
    Upper,
    Lower,
}

struct SettingsApp {
    cfg: Config,
    saved: Config,
    cfg_mtime: Option<SystemTime>,
    macros: MacroTable,
    page: Page,
    logo: Option<TextureHandle>,
    service_running: bool,
    last_poll: Instant,
    toast: Option<(String, Instant)>,
    // Gõ thử
    playground: String,
    // Gõ tắt
    new_key: String,
    new_value: String,
    macro_filter: String,
    // Chuyển mã
    conv_input: String,
    conv_output: String,
    conv_remove_tones: bool,
    conv_case: CaseMode,
    // Ứng dụng
    new_app: String,
    running_apps: Vec<String>,
    dark_titlebar_done: bool,
    english_builtin: usize,
    english_user: usize,
    // Dịch AI
    ai_tx: std::sync::mpsc::Sender<AiMsg>,
    ai_rx: std::sync::mpsc::Receiver<AiMsg>,
    key_input: String,
    ai_busy: bool,
    ai_status: Option<(bool, String)>,
    models: Vec<String>,
    try_input: String,
    try_output: String,
    lang_search: String,
    read_search: String,
}

impl SettingsApp {
    fn new(cc: &eframe::CreationContext<'_>, page: Option<String>) -> SettingsApp {
        ui::setup(&cc.egui_ctx);
        let logo = image::load_from_memory(LOGO_PNG).ok().map(|img| {
            let img = img.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            cc.egui_ctx.load_texture("logo", ColorImage::from_rgba_unmultiplied(size, &img), TextureOptions::LINEAR)
        });
        let cfg = Config::load();
        let (ai_tx, ai_rx) = std::sync::mpsc::channel();
        let page = match page.as_deref() {
            Some("macros") => Page::Macros,
            Some("convert") => Page::Convert,
            Some("apps") => Page::Apps,
            Some("about") => Page::About,
            Some("ai") => Page::Ai,
            _ => Page::Typing,
        };
        SettingsApp {
            saved: cfg.clone(),
            macros: config::load_macros(cfg.macro_auto_case),
            cfg,
            cfg_mtime: config::modified(&config::config_path()),
            page,
            logo,
            service_running: !win::find_service_window().is_null(),
            last_poll: Instant::now(),
            toast: None,
            playground: String::new(),
            new_key: String::new(),
            new_value: String::new(),
            macro_filter: String::new(),
            conv_input: String::new(),
            conv_output: String::new(),
            conv_remove_tones: false,
            conv_case: CaseMode::Keep,
            new_app: String::new(),
            running_apps: win::running_apps(),
            dark_titlebar_done: false,
            english_builtin: cinput_core::EnglishWords::builtin().len(),
            english_user: config::user_english_count(),
            ai_tx,
            ai_rx,
            key_input: String::new(),
            ai_busy: false,
            ai_status: None,
            models: Vec::new(),
            try_input: String::new(),
            try_output: String::new(),
            lang_search: String::new(),
            read_search: String::new(),
        }
    }

    fn toast(&mut self, text: &str) {
        self.toast = Some((text.to_string(), Instant::now()));
    }

    /// Lưu nếu có thay đổi và báo bộ gõ.
    fn persist(&mut self) {
        if self.cfg == self.saved {
            return;
        }
        if self.cfg.autostart != self.saved.autostart {
            win::set_autostart(self.cfg.autostart);
        }
        if self.cfg.macro_auto_case != self.saved.macro_auto_case {
            self.macros.auto_case = self.cfg.macro_auto_case;
        }
        match self.cfg.save() {
            Ok(()) => {
                self.saved = self.cfg.clone();
                self.cfg_mtime = config::modified(&config::config_path());
                win::notify_service(win::WM_CINPUT_RELOAD);
                self.toast("Đã lưu thay đổi");
            }
            Err(e) => self.toast(&format!("Không lưu được: {e}")),
        }
    }

    fn save_macros(&mut self) {
        match config::save_macros(&self.macros) {
            Ok(()) => {
                win::notify_service(win::WM_CINPUT_RELOAD);
                self.toast("Đã lưu bảng gõ tắt");
            }
            Err(e) => self.toast(&format!("Không lưu được: {e}")),
        }
    }

    /// Đồng bộ khi bộ gõ đổi cấu hình (bật/tắt từ khay hệ thống...).
    fn poll(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(700) {
            return;
        }
        self.last_poll = Instant::now();
        self.service_running = !win::find_service_window().is_null();
        let m = config::modified(&config::config_path());
        if m != self.cfg_mtime {
            self.cfg_mtime = m;
            let c = Config::load();
            self.cfg = c.clone();
            self.saved = c;
        }
    }

    // ------------------------------------------------------------------ khung

    fn sidebar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("status")
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::NONE)
            .show(ui, |ui| self.status_card(ui));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if let Some(t) = &self.logo {
                ui.add(egui::Image::from_texture(t).fit_to_exact_size(vec2(40.0, 40.0)));
            }
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(2.0);
                ui.label(ui::title("C_Input", 19.0));
                ui.label(ui::muted("Bộ gõ tiếng Việt"));
            });
        });
        ui.add_space(22.0);
        ui.spacing_mut().item_spacing.y = 4.0;
        let items = [
            (Page::Typing, icon::KEYBOARD, "Bộ gõ"),
            (Page::Macros, icon::MACRO, "Gõ tắt"),
            (Page::Convert, icon::CONVERT, "Chuyển mã"),
            (Page::Apps, icon::APPS, "Ứng dụng"),
            (Page::Ai, icon::GLOBE, "Dịch AI"),
            (Page::About, icon::INFO, "Giới thiệu"),
        ];
        for (page, glyph, label) in items {
            if ui::nav_item(ui, glyph, label, self.page == page).clicked() {
                self.page = page;
                if page == Page::Apps {
                    self.running_apps = win::running_apps();
                }
            }
        }
    }

    fn status_card(&mut self, ui: &mut Ui) {
        let on = self.cfg.vietnamese;
        Frame::new()
            .fill(if on { ui::ACCENT_BG } else { ui::CARD })
            .stroke(Stroke::new(1.0, if on { egui::Color32::from_rgb(0x0E, 0x5C, 0x43) } else { ui::BORDER }))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                if !self.service_running {
                    ui.horizontal(|ui| {
                        ui.label(ui::icon_text(icon::WARNING, 14.0, ui::WARN));
                        ui.label(RichText::new("Bộ gõ chưa chạy").color(ui::WARN).font(FontId::new(13.5, ui::semibold())));
                    });
                    ui.label(ui::muted("Bấm để khởi động bộ gõ nền."));
                    ui.add_space(4.0);
                    let btn = egui::Button::new(
                        RichText::new("Khởi động bộ gõ").font(FontId::new(14.0, ui::semibold())).color(ui::ON_ACCENT),
                    )
                    .fill(ui::ACCENT)
                    .corner_radius(CornerRadius::same(8))
                    .min_size(vec2(ui.available_width(), 34.0));
                    if ui.add(btn).clicked() {
                        win::spawn_self(&[]);
                        self.last_poll = Instant::now() - Duration::from_secs(1);
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(ui::title(if on { "Tiếng Việt" } else { "Tiếng Anh" }, 15.0));
                        let detail = if on {
                            format!("{} · {}", self.cfg.method.label(), self.cfg.charset.short())
                        } else {
                            "Đang tắt gõ dấu".to_string()
                        };
                        ui.label(ui::muted(&detail));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui::toggle(ui, &mut self.cfg.vietnamese).on_hover_text("Bật/tắt gõ tiếng Việt");
                    });
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for k in self.cfg.switch_key.label().split(" + ") {
                        ui::keycap(ui, k);
                    }
                    ui.label(ui::muted("để chuyển"));
                });
            }));
    }

    fn header(ui: &mut Ui, title: &str, subtitle: &str) {
        ui.label(ui::title(title, 26.0));
        ui.label(RichText::new(subtitle).size(14.0).color(ui::MUTED));
        ui.add_space(14.0);
    }

    // ------------------------------------------------------------------ trang Bộ gõ

    fn page_typing(&mut self, ui: &mut Ui) {
        Self::header(ui, "Bộ gõ", "Kiểu gõ, bảng mã và cách đặt dấu.");
        let (english_builtin, english_user) = (self.english_builtin, self.english_user);
        let (mut english_open, mut english_reload) = (false, false);
        let cfg = &mut self.cfg;
        ui::card(ui, Some("Kiểu gõ"), |ui| {
            ui::row(ui, "Kiểu gõ", "Telex gõ dấu bằng chữ (s f r x j), VNI gõ bằng phím số (1–9).", |ui| {
                let opts: Vec<(Method, &str)> = Method::ALL.iter().map(|m| (*m, m.label())).collect();
                ui::segmented(ui, &mut cfg.method, &opts);
            });
            ui::row_sep(ui);
            ui::row(ui, "Bảng mã", "Bảng mã của chữ gửi vào ứng dụng. Dùng Unicode cho hầu hết trường hợp.", |ui| {
                egui::ComboBox::from_id_salt("charset")
                    .selected_text(cfg.charset.label())
                    .width(210.0)
                    .show_ui(ui, |ui| {
                        for c in Charset::TYPING {
                            ui.selectable_value(&mut cfg.charset, c, c.label());
                        }
                    });
            });
            ui::row_sep(ui);
            ui::row(ui, "Phím chuyển Anh/Việt", "Phím tắt bật/tắt gõ tiếng Việt ở bất cứ đâu.", |ui| {
                ui::segmented(
                    ui,
                    &mut cfg.switch_key,
                    &[(SwitchKey::CtrlShift, "Ctrl + Shift"), (SwitchKey::AltZ, "Alt + Z")],
                );
            });
        });

        ui::card(ui, Some("Cách đặt dấu"), |ui| {
            ui::row(ui, "Đặt dấu kiểu mới", "hoà, khoẻ, thuý — thay cho hòa, khỏe, thúy.", |ui| {
                ui::toggle(ui, &mut cfg.modern_tone);
            });
            ui::row_sep(ui);
            ui::row(
                ui,
                "Kiểm tra chính tả",
                "Không bỏ dấu vào từ sai chính tả, tự trả lại phím gốc cho từ tiếng Anh (windows, text…).",
                |ui| {
                    ui::toggle(ui, &mut cfg.spell_check);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                "Từ tiếng Anh giữ nguyên",
                &format!(
                    "Gõ đúng các từ như test, error, password, data… thì giữ tiếng Anh. \
                     {english_builtin} từ có sẵn + {english_user} từ của bạn. Muốn ra \"tét\" thì gõ \"tets\".",
                ),
                |ui| {
                    ui::toggle(ui, &mut cfg.english_words);
                },
            );
            ui.horizontal_wrapped(|ui| {
                if ui::button(ui, Some(icon::FOLDER), "Sửa danh sách của tôi").clicked() {
                    english_open = true;
                }
                if ui::button(ui, Some(icon::REFRESH), "Nạp lại").on_hover_text("Đọc lại danh sách sau khi sửa").clicked() {
                    english_reload = true;
                }
            });
            ui.add_space(8.0);
        });
        if english_open {
            let _ = config::ensure_english_file();
            win::shell_open(&config::english_path());
        }
        if english_reload {
            self.english_user = config::user_english_count();
            win::notify_service(win::WM_CINPUT_RELOAD);
            self.toast("Đã nạp lại danh sách từ tiếng Anh");
        }

        let cfg = &mut self.cfg;
        ui::card(ui, Some("Tự động"), |ui| {
            ui::row(
                ui,
                "Tự viết hoa đầu câu",
                "Chữ đầu tiên sau dấu . ? ! và khi xuống dòng tự viết hoa. Bỏ qua viết tắt (v.v., TP.), \
                 dấu ba chấm và địa chỉ web.",
                |ui| {
                    ui::toggle(ui, &mut cfg.auto_capitalize);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                "Tắt tiếng Việt trong ô mật khẩu",
                "Khi đang nhập mật khẩu, C_Input gõ ra đúng phím đã bấm (không bỏ dấu, không gõ tắt) \
                 và biểu tượng khay chuyển sang màu xám.",
                |ui| {
                    ui::toggle(ui, &mut cfg.password_off);
                },
            );
        });

        let method = self.cfg.method;
        ui::card(ui, Some("Gõ thử"), |ui| {
            ui.add_space(8.0);
            ui::input_frame(ui, |ui| {
                ui.add(
                    TextEdit::singleline(&mut self.playground)
                        .frame(egui::Frame::NONE)
                        .hint_text(if method == Method::Vni {
                            "Gõ thử ở đây, ví dụ: Tie61ng Vie65t"
                        } else {
                            "Gõ thử ở đây, ví dụ: Tieesng Vieejt"
                        })
                        .font(FontId::proportional(16.0))
                        .desired_width(f32::INFINITY),
                );
            });
            ui.add_space(4.0);
            Self::cheatsheet(ui, method);
            ui.add_space(6.0);
        });

        let cfg = &mut self.cfg;
        ui::card(ui, Some("Hệ thống"), |ui| {
            ui::row(ui, "Khởi động cùng Windows", "Tự chạy C_Input khi đăng nhập Windows.", |ui| {
                ui::toggle(ui, &mut cfg.autostart);
            });
            ui::row_sep(ui);
            ui::row(ui, "Tiếng bíp khi chuyển chế độ", "Phát âm thanh ngắn khi bật/tắt tiếng Việt.", |ui| {
                ui::toggle(ui, &mut cfg.beep);
            });
        });
    }

    fn cheatsheet(ui: &mut Ui, method: Method) {
        let pairs: &[(&str, &str)] = if method == Method::Vni {
            &[
                ("1", "sắc"), ("2", "huyền"), ("3", "hỏi"), ("4", "ngã"), ("5", "nặng"), ("0", "xoá dấu"),
                ("6", "â ê ô"), ("7", "ơ ư"), ("8", "ă"), ("9", "đ"),
            ]
        } else {
            let mut v: Vec<(&str, &str)> = vec![
                ("s", "sắc"), ("f", "huyền"), ("r", "hỏi"), ("x", "ngã"), ("j", "nặng"), ("z", "xoá dấu"),
                ("aa", "â"), ("aw", "ă"), ("ee", "ê"), ("oo", "ô"), ("ow", "ơ"), ("uw", "ư"), ("dd", "đ"),
            ];
            if method == Method::Telex {
                v.push(("w", "ư"));
                v.push(("[ ]", "ơ ư"));
            }
            return Self::chips(ui, &v);
        };
        Self::chips(ui, pairs);
    }

    fn chips(ui: &mut Ui, pairs: &[(&str, &str)]) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (k, v) in pairs {
                ui::key_chip(ui, k, v);
            }
        });
    }

    // ------------------------------------------------------------------ trang Gõ tắt

    fn page_macros(&mut self, ui: &mut Ui) {
        Self::header(ui, "Gõ tắt", "Gõ một từ ngắn rồi bấm cách để thay bằng cụm từ đầy đủ.");
        let cfg = &mut self.cfg;
        ui::card(ui, Some("Tuỳ chọn"), |ui| {
            ui::row(ui, "Bật gõ tắt", "Thay từ tắt khi gõ dấu cách, dấu câu hoặc Enter.", |ui| {
                ui::toggle(ui, &mut cfg.macro_enabled);
            });
            ui::row_sep(ui);
            ui::row(ui, "Gõ tắt cả khi tắt tiếng Việt", "Vẫn thay từ tắt khi đang ở chế độ tiếng Anh.", |ui| {
                ui::toggle(ui, &mut cfg.macro_always);
            });
            ui::row_sep(ui);
            ui::row(ui, "Tự viết hoa theo chữ gõ", "vn → việt nam, Vn → Việt nam, VN → VIỆT NAM.", |ui| {
                ui::toggle(ui, &mut cfg.macro_auto_case);
            });
        });

        let mut add = false;
        ui::card(ui, Some("Thêm gõ tắt"), |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.allocate_ui(vec2(150.0, 34.0), |ui| {
                    ui::input_frame(ui, |ui| {
                        let r = ui.add(
                            TextEdit::singleline(&mut self.new_key)
                                .frame(egui::Frame::NONE)
                                .hint_text("Từ tắt, vd: vn")
                                .desired_width(f32::INFINITY),
                        );
                        add |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    });
                });
                ui.label(RichText::new("→").color(ui::FAINT).size(16.0));
                let w = ui.available_width() - 100.0;
                ui.allocate_ui(vec2(w, 34.0), |ui| {
                    ui::input_frame(ui, |ui| {
                        let r = ui.add(
                            TextEdit::singleline(&mut self.new_value)
                                .frame(egui::Frame::NONE)
                                .hint_text("Nội dung thay thế, vd: Việt Nam")
                                .desired_width(f32::INFINITY),
                        );
                        add |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    });
                });
                add |= ui::primary_button(ui, "  Thêm  ").clicked();
            });
            ui.add_space(8.0);
        });
        if add {
            let key = self.new_key.trim().to_string();
            if key.is_empty() || self.new_value.is_empty() {
                self.toast("Cần nhập cả từ tắt và nội dung");
            } else if key.contains(':') || key.contains(' ') {
                self.toast("Từ tắt không được chứa dấu cách hoặc dấu ':'");
            } else {
                self.macros.insert(&key, &self.new_value);
                self.new_key.clear();
                self.new_value.clear();
                self.save_macros();
            }
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Danh sách ({})", self.macros.len()))
                    .font(FontId::new(13.0, ui::semibold()))
                    .color(ui::MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.allocate_ui(vec2(220.0, 30.0), |ui| {
                    ui::input_frame(ui, |ui| {
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.label(ui::icon_text(icon::SEARCH, 12.0, ui::FAINT));
                            ui.add(
                                TextEdit::singleline(&mut self.macro_filter)
                                    .frame(egui::Frame::NONE)
                                    .hint_text("Tìm…")
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    });
                });
            });
        });
        ui.add_space(2.0);
        let filter = self.macro_filter.to_lowercase();
        let items: Vec<(String, String)> = self
            .macros
            .iter()
            .filter(|(k, v)| filter.is_empty() || k.to_lowercase().contains(&filter) || v.to_lowercase().contains(&filter))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut remove = None;
        ui::card(ui, None, |ui| {
            if items.is_empty() {
                ui.add_space(14.0);
                ui.vertical_centered(|ui| {
                    ui.label(ui::muted(if self.macros.is_empty() { "Chưa có từ gõ tắt nào." } else { "Không tìm thấy." }));
                });
                ui.add_space(14.0);
            }
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    ui::row_sep(ui);
                }
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.allocate_ui_with_layout(vec2(150.0, 36.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_min_size(vec2(150.0, 36.0));
                        ui.label(RichText::new(k).font(FontId::new(14.0, ui::semibold())).color(ui::ACCENT_HI));
                    });
                    ui.label(RichText::new(v).size(14.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::icon_button(ui, icon::DELETE, ui::DANGER).on_hover_text("Xoá").clicked() {
                            remove = Some(k.clone());
                        }
                    });
                });
            }
        });
        if let Some(k) = remove {
            self.macros.remove(&k);
            self.save_macros();
        }
        ui.horizontal(|ui| {
            if ui::button(ui, Some(icon::FOLDER), "Mở file gõ tắt").clicked() {
                if !config::macros_path().exists() {
                    let _ = config::save_macros(&self.macros);
                }
                win::shell_open(&config::macros_path());
            }
            if ui::button(ui, Some(icon::REFRESH), "Nạp lại từ file").clicked() {
                self.macros = config::load_macros(self.cfg.macro_auto_case);
                win::notify_service(win::WM_CINPUT_RELOAD);
                self.toast("Đã nạp lại bảng gõ tắt");
            }
        });
    }

    // ------------------------------------------------------------------ trang Chuyển mã

    fn convert_text(&self, text: &str) -> String {
        let mut s = decode(text, self.cfg.clip_from);
        if self.conv_remove_tones {
            s = remove_tones(&s);
        }
        s = match self.conv_case {
            CaseMode::Keep => s,
            CaseMode::Upper => s.to_uppercase(),
            CaseMode::Lower => s.to_lowercase(),
        };
        encode(&s, self.cfg.clip_to)
    }

    fn page_convert(&mut self, ui: &mut Ui) {
        Self::header(ui, "Chuyển mã", "Chuyển văn bản giữa các bảng mã tiếng Việt: TCVN3, VNI, Unicode…");
        let cfg = &mut self.cfg;
        ui::card(ui, Some("Bảng mã"), |ui| {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(ui::muted("Từ"));
                egui::ComboBox::from_id_salt("from").selected_text(cfg.clip_from.label()).width(190.0).show_ui(ui, |ui| {
                    for c in Charset::ALL {
                        ui.selectable_value(&mut cfg.clip_from, c, c.label());
                    }
                });
                if ui::icon_button(ui, icon::SWAP, ui::ACCENT_HI).on_hover_text("Đảo chiều").clicked() {
                    std::mem::swap(&mut cfg.clip_from, &mut cfg.clip_to);
                }
                ui.label(ui::muted("Sang"));
                egui::ComboBox::from_id_salt("to").selected_text(cfg.clip_to.label()).width(190.0).show_ui(ui, |ui| {
                    for c in Charset::ALL {
                        ui.selectable_value(&mut cfg.clip_to, c, c.label());
                    }
                });
            });
            ui.add_space(4.0);
            ui::row_sep(ui);
            ui::row(ui, "Bỏ dấu", "Tiếng Việt → Tieng Viet", |ui| {
                ui::toggle(ui, &mut self.conv_remove_tones);
            });
            ui::row_sep(ui);
            ui::row(ui, "Chữ hoa / thường", "", |ui| {
                ui::segmented(
                    ui,
                    &mut self.conv_case,
                    &[(CaseMode::Keep, "Giữ nguyên"), (CaseMode::Upper, "HOA"), (CaseMode::Lower, "thường")],
                );
            });
        });

        ui::card(ui, Some("Văn bản"), |ui| {
            ui.add_space(8.0);
            let w = (ui.available_width() - 12.0) / 2.0;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(w, 220.0), Layout::top_down(Align::Min), |ui| {
                    ui.label(ui::muted("Nguồn"));
                    ui::input_frame(ui, |ui| {
                        ScrollArea::vertical().id_salt("in").max_height(190.0).show(ui, |ui| {
                            ui.add(
                                TextEdit::multiline(&mut self.conv_input)
                                    .frame(egui::Frame::NONE)
                                    .hint_text("Dán văn bản cần chuyển vào đây…")
                                    .desired_rows(9)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    });
                });
                ui.allocate_ui_with_layout(vec2(w, 220.0), Layout::top_down(Align::Min), |ui| {
                    ui.label(ui::muted("Kết quả"));
                    ui::input_frame(ui, |ui| {
                        ScrollArea::vertical().id_salt("out").max_height(190.0).show(ui, |ui| {
                            ui.add(
                                TextEdit::multiline(&mut self.conv_output.as_str())
                                    .frame(egui::Frame::NONE)
                                    .desired_rows(9)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    });
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui::button(ui, Some(icon::PASTE), "Dán từ clipboard").clicked() {
                    if let Some(t) = win::clipboard_text(std::ptr::null_mut()) {
                        self.conv_input = t;
                    }
                }
                if ui::primary_button(ui, "  Chuyển mã  ").clicked() {
                    self.conv_output = self.convert_text(&self.conv_input);
                }
                if ui::button(ui, Some(icon::COPY), "Chép kết quả").clicked()
                    && win::set_clipboard_text(std::ptr::null_mut(), &self.conv_output)
                {
                    self.toast("Đã chép kết quả vào clipboard");
                }
            });
            ui.add_space(8.0);
        });

        ui::card(ui, Some("Chuyển nhanh clipboard"), |ui| {
            ui::row(
                ui,
                "Ctrl + Shift + F9",
                "Bấm ở bất cứ đâu để chuyển mã nội dung clipboard theo hai bảng mã ở trên.",
                |ui| {
                    if ui::button(ui, Some(icon::PLAY), "Chuyển clipboard ngay").clicked() {
                        match win::clipboard_text(std::ptr::null_mut()) {
                            Some(t) => {
                                let out = self.convert_text(&t);
                                win::set_clipboard_text(std::ptr::null_mut(), &out);
                                self.toast("Đã chuyển mã clipboard");
                            }
                            None => self.toast("Clipboard không có văn bản"),
                        }
                    }
                },
            );
        });
    }

    // ------------------------------------------------------------------ trang Ứng dụng

    fn page_apps(&mut self, ui: &mut Ui) {
        Self::header(ui, "Ứng dụng", "Chế độ gõ riêng cho từng ứng dụng.");
        let cfg = &mut self.cfg;
        ui::card(ui, Some("Theo từng ứng dụng"), |ui| {
            ui::row(
                ui,
                "Nhớ chế độ Anh/Việt cho từng ứng dụng",
                "Mỗi cửa sổ giữ chế độ gõ riêng khi chuyển qua lại.",
                |ui| {
                    ui::toggle(ui, &mut cfg.per_app);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                "Sửa lỗi gợi ý tự động",
                "Thanh địa chỉ trình duyệt và Excel hay bôi đen phần gợi ý, làm mất chữ khi bỏ dấu. \
                 Áp dụng cho Chrome, Edge, Firefox, Cốc Cốc, Brave, Opera, Excel.",
                |ui| {
                    ui::toggle(ui, &mut cfg.browser_fix);
                },
            );
        });

        let svc = win::find_service_window();
        let svc_elevated = !svc.is_null() && win::window_elevated(svc);
        ui::card(ui, Some("Quyền Administrator"), |ui| {
            let desc = if svc_elevated {
                "C_Input đang chạy với quyền Administrator: gõ được trong mọi ứng dụng, kể cả ứng dụng quyền Admin."
            } else {
                "Windows không cho chương trình thường gõ vào ứng dụng chạy quyền Admin (Task Manager, \
                 cmd quyền Admin, trình cài đặt…). C_Input sẽ tự tạm ngừng bỏ dấu ở đó và báo cho bạn."
            };
            ui::row(ui, if svc_elevated { "Đang chạy quyền Admin" } else { "Chạy thường" }, desc, |ui| {
                if !svc_elevated && !svc.is_null() && ui::button(ui, None, "Chạy với quyền Admin").clicked() {
                    // Bản mới (quyền Admin, qua hộp thoại UAC) chờ bản hiện tại thoát rồi thay thế.
                    if win::run_elevated("--replace") {
                        unsafe {
                            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                                svc,
                                windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                                0,
                                0,
                            );
                        }
                    }
                }
            });
        });

        let mut add: Option<String> = None;
        ui::card(ui, Some("Luôn gõ tiếng Anh trong"), |ui| {
            ui.add_space(8.0);
            ui.add(egui::Label::new(ui::muted(
                "Trong các ứng dụng này C_Input không bỏ dấu — hữu ích cho trình soạn code, game, terminal…",
            )));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("running")
                    .selected_text("Chọn ứng dụng đang mở…")
                    .width(230.0)
                    .show_ui(ui, |ui| {
                        for a in &self.running_apps {
                            if ui.selectable_label(false, a).clicked() {
                                add = Some(a.clone());
                            }
                        }
                    });
                ui.label(ui::muted("hoặc"));
                let w = ui.available_width() - 100.0;
                ui.allocate_ui(vec2(w, 34.0), |ui| {
                    ui::input_frame(ui, |ui| {
                        let r = ui.add(
                            TextEdit::singleline(&mut self.new_app)
                                .frame(egui::Frame::NONE)
                                .hint_text("tên file, vd: code.exe")
                                .desired_width(f32::INFINITY),
                        );
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            add = Some(self.new_app.clone());
                        }
                    });
                });
                if ui::primary_button(ui, "  Thêm  ").clicked() {
                    add = Some(self.new_app.clone());
                }
            });
            ui.add_space(8.0);
            let mut remove = None;
            if cfg.excluded_apps.is_empty() {
                ui::row_sep(ui);
                ui.add_space(12.0);
                ui.vertical_centered(|ui| ui.label(ui::muted("Chưa có ứng dụng nào.")));
                ui.add_space(12.0);
            }
            for (i, a) in cfg.excluded_apps.iter().enumerate() {
                ui::row_sep(ui);
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(vec2(28.0, 36.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_min_size(vec2(28.0, 36.0));
                        ui.label(ui::icon_text(icon::APPS, 14.0, ui::MUTED));
                    });
                    ui.label(RichText::new(a).size(14.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::icon_button(ui, icon::DELETE, ui::DANGER).on_hover_text("Bỏ khỏi danh sách").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                cfg.excluded_apps.remove(i);
            }
        });
        if let Some(a) = add {
            let mut a = a.trim().to_lowercase();
            if !a.is_empty() {
                if !a.ends_with(".exe") {
                    a.push_str(".exe");
                }
                if !self.cfg.is_excluded(&a) {
                    self.cfg.excluded_apps.push(a);
                }
                self.new_app.clear();
            }
        }
    }

    // ------------------------------------------------------------------ trang Giới thiệu

    // ------------------------------------------------------------------ trang Dịch AI

    fn saved_key(&self) -> Option<String> {
        crate::ai::unprotect_key(&self.cfg.gemini_key)
    }

    fn check_key(&mut self, key: String) {
        self.ai_busy = true;
        self.ai_status = None;
        let tx = self.ai_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(AiMsg::Models(crate::ai::list_models(&key)));
        });
    }

    fn page_ai(&mut self, ui: &mut Ui) {
        Self::header(ui, "Dịch AI", "Gõ xong một câu, C_Input tự dịch câu đó sang ngôn ngữ bạn chọn bằng Google Gemini.");
        let search = &mut self.lang_search;
        let cfg = &mut self.cfg;
        ui::card(ui, Some("Dịch tự động"), |ui| {
            ui::row(
                ui,
                "Dịch câu vừa gõ",
                "Kết thúc câu bằng . ? hoặc ! rồi dấu cách: câu đó được thay bằng bản dịch, bạn gõ tiếp câu sau \
                 ngay, không phải chờ. Không dịch khi bấm Enter, trong ô mật khẩu và ứng dụng luôn gõ tiếng Anh.",
                |ui| {
                    ui::toggle(ui, &mut cfg.translate_enabled);
                },
            );
            if cfg.translate_enabled {
                ui::row_sep(ui);
                ui::row(ui, "Dịch sang", "Ngôn ngữ đích của bản dịch. Gõ để tìm nhanh, không cần dấu.", |ui| {
                    language_picker(ui, "tr_lang", &mut cfg.translate_lang, search);
                });
            }
        });
        let read_search = &mut self.read_search;
        let cfg = &mut self.cfg;
        ui::card(ui, Some("Dịch đoạn bôi đen"), |ui| {
            ui::row(
                ui,
                "Bấm Ctrl+C hai lần để dịch",
                "Bôi đen một đoạn chữ ở bất kỳ đâu (tin nhắn đến, trang web, tài liệu) rồi bấm Ctrl+C hai lần liền: \
                 bản dịch hiện trong cửa sổ nhỏ cạnh con trỏ, đổi ngôn ngữ ngay trong đó. Văn bản gốc không bị sửa.",
                |ui| {
                    ui::toggle(ui, &mut cfg.read_enabled);
                },
            );
            if cfg.read_enabled {
                ui::row_sep(ui);
                ui::row(ui, "Dịch sang", "Ngôn ngữ bạn đọc — thường là tiếng Việt.", |ui| {
                    language_picker(ui, "read_lang_settings", &mut cfg.read_lang, read_search);
                });
            }
        });
        if !self.cfg.translate_enabled && !self.cfg.read_enabled {
            ui.add(egui::Label::new(ui::muted(
                "Bật một trong hai tính năng trên để gắn API key Gemini. Khi tắt cả hai, C_Input không gửi gì lên mạng.",
            )));
            return;
        }

        let saved = self.saved_key();
        let mut save = false;
        let mut remove = false;
        let mut check = false;
        ui::card(ui, Some("API key Gemini"), |ui| {
            ui.add_space(8.0);
            match &saved {
                Some(k) => {
                    let tail: String = k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
                    ui.horizontal(|ui| {
                        ui.label(ui::icon_text(icon::CHECK, 13.0, ui::ACCENT_HI));
                        ui.label(RichText::new(format!("Đã lưu key ••••{tail}")).size(14.0));
                        ui.label(ui::muted("(mã hoá bằng Windows, chỉ tài khoản của bạn đọc được)"));
                    });
                }
                None => {
                    ui.horizontal(|ui| {
                        ui.label(ui::icon_text(icon::WARNING, 13.0, ui::WARN));
                        ui.label(RichText::new("Chưa có API key").color(ui::WARN));
                    });
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 290.0).max(160.0);
                ui.allocate_ui(vec2(w, 34.0), |ui| {
                    ui::input_frame(ui, |ui| {
                        let r = ui.add(
                            TextEdit::singleline(&mut self.key_input)
                                .password(true)
                                .frame(egui::Frame::NONE)
                                .hint_text(if saved.is_some() { "Dán key mới để thay" } else { "Dán API key Gemini vào đây" })
                                .desired_width(f32::INFINITY),
                        );
                        save |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    });
                });
                save |= ui::primary_button(ui, "  Lưu key  ").clicked();
                if saved.is_some() {
                    check |= ui::button(ui, Some(icon::REFRESH), "Kiểm tra").clicked();
                    remove |= ui::icon_button(ui, icon::DELETE, ui::DANGER).on_hover_text("Xoá key").clicked();
                }
            });
            ui.horizontal(|ui| {
                ui.label(ui::muted("Chưa có key? Tạo miễn phí tại"));
                ui.hyperlink_to("aistudio.google.com", crate::ai::KEY_URL);
            });
            if self.ai_busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(ui::muted("Đang liên hệ Gemini…"));
                });
            } else if let Some((ok, msg)) = &self.ai_status {
                ui.horizontal(|ui| {
                    ui.label(ui::icon_text(if *ok { icon::CHECK } else { icon::WARNING }, 13.0, if *ok { ui::ACCENT_HI } else { ui::DANGER }));
                    ui.add(egui::Label::new(RichText::new(msg).color(if *ok { ui::MUTED } else { ui::DANGER })).wrap());
                });
            }
            ui::row_sep(ui);
            let cfg = &mut self.cfg;
            let models = &self.models;
            ui::row(ui, "Model", "Model \"flash\" nhanh và rẻ, hợp để dịch từng câu. Bấm Kiểm tra để lấy danh sách.", |ui| {
                egui::ComboBox::from_id_salt("tr_model").selected_text(cfg.gemini_model.as_str()).width(210.0).show_ui(ui, |ui| {
                    if models.is_empty() {
                        ui.selectable_value(&mut cfg.gemini_model, crate::ai::DEFAULT_MODEL.to_string(), crate::ai::DEFAULT_MODEL);
                    }
                    for m in models {
                        ui.selectable_value(&mut cfg.gemini_model, m.clone(), m);
                    }
                });
            });
        });
        if save {
            let key = self.key_input.trim().to_string();
            if key.is_empty() {
                self.toast("Hãy dán API key trước khi lưu");
            } else if let Some(enc) = crate::ai::protect_key(&key) {
                self.cfg.gemini_key = enc;
                self.key_input.clear();
                self.toast("Đã lưu API key (đã mã hoá)");
                self.check_key(key);
            }
        }
        if remove {
            self.cfg.gemini_key.clear();
            self.models.clear();
            self.ai_status = None;
            self.toast("Đã xoá API key");
        }
        if check {
            if let Some(k) = saved.clone() {
                self.check_key(k);
            }
        }

        let mut run_try = false;
        ui::card(ui, Some("Dịch thử"), |ui| {
            ui.add_space(8.0);
            ui::input_frame(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut self.try_input)
                        .frame(egui::Frame::NONE)
                        .hint_text("Gõ một câu để thử, ví dụ: Hôm nay trời đẹp quá.")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                run_try = ui::primary_button(ui, "  Dịch thử  ").clicked();
                ui.label(ui::muted(&format!("→ {}", crate::ai::language_label(&self.cfg.translate_lang))));
            });
            if !self.try_output.is_empty() {
                ui.add_space(4.0);
                ui::input_frame(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.try_output.as_str())
                            .frame(egui::Frame::NONE)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY),
                    );
                });
            }
            ui.add_space(8.0);
        });
        if run_try {
            match (saved, self.try_input.trim().is_empty()) {
                (None, _) => self.toast("Cần lưu API key trước"),
                (_, true) => self.toast("Hãy gõ câu cần dịch"),
                (Some(key), false) => {
                    self.ai_busy = true;
                    self.ai_status = None;
                    self.try_output.clear();
                    let (tx, text) = (self.ai_tx.clone(), self.try_input.trim().to_string());
                    let (model, lang) = (self.cfg.gemini_model.clone(), self.cfg.translate_lang.clone());
                    std::thread::spawn(move || {
                        let _ = tx.send(AiMsg::Translated(crate::ai::translate(&key, &model, &lang, &text)));
                    });
                }
            }
        }
    }

    fn page_about(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            if let Some(t) = &self.logo {
                ui.add(egui::Image::from_texture(t).fit_to_exact_size(vec2(96.0, 96.0)));
            }
            ui.add_space(6.0);
            ui.label(ui::title("C_Input", 28.0));
            ui.label(ui::muted(&format!("Phiên bản {}", env!("CARGO_PKG_VERSION"))));
            ui.add_space(4.0);
            ui.label(RichText::new("Bộ gõ tiếng Việt cho Windows — nhẹ, nhanh, chạy hoàn toàn trên máy.").color(ui::MUTED));
        });
        ui.add_space(14.0);
        ui::card(ui, Some("Phím tắt"), |ui| {
            let rows: [(&[&str], &str); 3] = [
                (&[self.cfg.switch_key.label()], "Bật/tắt gõ tiếng Việt"),
                (&["Ctrl", "Shift", "F5"], "Mở cửa sổ cài đặt"),
                (&["Ctrl", "Shift", "F9"], "Chuyển mã clipboard"),
            ];
            for (i, (keys, what)) in rows.iter().enumerate() {
                if i > 0 {
                    ui::row_sep(ui);
                }
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(vec2(170.0, 38.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_min_size(vec2(170.0, 38.0));
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for k in keys.iter().flat_map(|k| k.split(" + ")) {
                            ui::keycap(ui, k);
                        }
                    });
                    ui.label(*what);
                });
            }
        });
        ui::card(ui, Some("Thông tin"), |ui| {
            ui.add_space(8.0);
            for line in [
                "Lõi gõ viết bằng Rust: Telex, VNI, đặt dấu tự do, kiểm tra chính tả, tự khôi phục từ tiếng Anh.",
                "Không gửi dữ liệu ra ngoài, không cần mạng.",
                "Bảng mã TCVN3 / VNI Windows tham khảo từ mã nguồn UniKey 3.62 của Phạm Kim Long (GPL).",
                "Cấu hình lưu tại %APPDATA%\\C_Input.",
            ] {
                ui.horizontal(|ui| {
                    ui.label(ui::icon_text(icon::CHECK, 12.0, ui::ACCENT_HI));
                    ui.add(egui::Label::new(RichText::new(line).color(ui::MUTED)).wrap());
                });
            }
            ui.add_space(4.0);
            if ui::button(ui, Some(icon::FOLDER), "Mở thư mục cấu hình").clicked() {
                let _ = std::fs::create_dir_all(config::data_dir());
                win::shell_open(&config::data_dir());
            }
            ui.add_space(8.0);
        });
    }

    fn toast_overlay(&mut self, ui: &mut Ui) {
        let Some((text, at)) = &self.toast else { return };
        let age = at.elapsed().as_secs_f32();
        if age > 2.2 {
            self.toast = None;
            return;
        }
        let alpha = ((2.2 - age) / 0.4).clamp(0.0, 1.0);
        let rect = ui.max_rect();
        let font = FontId::new(13.5, ui::semibold());
        let galley = ui.painter().layout_no_wrap(text.clone(), font, ui::TEXT.gamma_multiply(alpha));
        let size = galley.size() + vec2(48.0, 20.0);
        let pos = egui::pos2(rect.right() - size.x - 24.0, rect.bottom() - size.y - 20.0);
        let r = egui::Rect::from_min_size(pos, size);
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::same(10), ui::RAISED.gamma_multiply(alpha));
        p.rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, ui::BORDER.gamma_multiply(alpha)), egui::StrokeKind::Inside);
        p.text(
            r.left_center() + vec2(16.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon::CHECK,
            FontId::new(12.0, egui::FontFamily::Name("icons".into())),
            ui::ACCENT_HI.gamma_multiply(alpha),
        );
        p.galley(r.left_top() + vec2(36.0, 10.0), galley, ui::TEXT);
        ui.ctx().request_repaint();
    }
}

impl eframe::App for SettingsApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.dark_titlebar_done {
            self.dark_titlebar_done = win::dark_titlebar(win::SETTINGS_TITLE);
        }
        self.poll();
        while let Ok(msg) = self.ai_rx.try_recv() {
            self.ai_busy = false;
            match msg {
                AiMsg::Models(Ok(list)) => {
                    let n = list.len();
                    if !list.is_empty() && !list.contains(&self.cfg.gemini_model) {
                        // Model đang chọn không còn: chọn model "flash" mới nhất có trong danh sách.
                        if let Some(m) = list.iter().rev().find(|m| m.contains("flash") && !m.contains("image")) {
                            self.cfg.gemini_model = m.clone();
                        }
                    }
                    self.models = list;
                    self.ai_status = Some((true, format!("API key hoạt động — {n} model dùng được.")));
                }
                AiMsg::Models(Err(e)) => self.ai_status = Some((false, e)),
                AiMsg::Translated(Ok(t)) => self.try_output = t,
                AiMsg::Translated(Err(e)) => self.ai_status = Some((false, e)),
            }
        }
        if self.ai_busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        ctx.request_repaint_after(Duration::from_millis(800));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("nav")
            .exact_size(236.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(ui::SIDEBAR).inner_margin(Margin::same(16)))
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(Frame::new().fill(ui::BG).inner_margin(Margin { left: 34, right: 18, top: 28, bottom: 8 }))
            .show(ui, |ui| {
                let mut area = ScrollArea::vertical().auto_shrink([false, false]);
                if let Some(y) = std::env::var("CINPUT_TEST_SCROLL").ok().and_then(|v| v.parse::<f32>().ok()) {
                    // Chỉ dùng khi kiểm thử giao diện: mở trang ở vị trí cuộn cho trước.
                    area = area.vertical_scroll_offset(y);
                }
                area.show(ui, |ui| {
                    ui.set_max_width(ui.available_width() - 16.0);
                    match self.page {
                        Page::Typing => self.page_typing(ui),
                        Page::Macros => self.page_macros(ui),
                        Page::Convert => self.page_convert(ui),
                        Page::Apps => self.page_apps(ui),
                        Page::Ai => self.page_ai(ui),
                        Page::About => self.page_about(ui),
                    }
                    ui.add_space(16.0);
                });
                self.toast_overlay(ui);
            });
        self.persist();
    }
}

/// Ô chọn ngôn ngữ có tìm kiếm: mở ra là gõ được ngay, lọc không dấu, khớp cả tên tiếng Anh;
/// Enter chọn kết quả đầu tiên.
pub(crate) fn language_picker(ui: &mut Ui, id: &str, selected: &mut String, search: &mut String) {
    use crate::ai::{POPULAR_LANGUAGES, language_label, language_matches, languages};
    let all = languages();
    egui::ComboBox::from_id_salt(id)
        .selected_text(language_label(selected))
        .width(240.0)
        .height(400.0)
        // Bấm vào ô tìm kiếm không được đóng danh sách.
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            ui.set_min_width(300.0);
            let r = ui::input_frame(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(ui::icon_text(icon::SEARCH, 12.0, ui::FAINT));
                    ui.add(
                        TextEdit::singleline(search)
                            .frame(egui::Frame::NONE)
                            .hint_text(format!("Tìm trong {} ngôn ngữ… (vd: dai loan, japan)", all.len()))
                            .desired_width(f32::INFINITY),
                    )
                })
                .inner
            });
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if !r.has_focus() && !enter {
                r.request_focus();
            }
            ui.add_space(4.0);
            let query = search.trim().to_string();
            let matches: Vec<usize> = (0..all.len()).filter(|&i| language_matches(all[i].0, all[i].1, &query)).collect();
            // Tên gõ vào không trùng hẳn ngôn ngữ nào trong danh sách → cho dùng nguyên tên đó (Gemini hiểu tên tự do).
            let exact = all.iter().any(|(n, l)| {
                n.eq_ignore_ascii_case(&query) || l.to_lowercase() == query.to_lowercase()
            });
            let custom = (query.chars().count() >= 2 && !exact).then(|| query.clone());

            let mut picked: Option<String> = None;
            if enter {
                picked = matches.first().map(|&i| all[i].0.to_string()).or_else(|| custom.clone());
            }
            if let Some(c) = &custom {
                let text = format!("Dùng ngôn ngữ «{c}»");
                if ui.selectable_label(selected.as_str() == c, RichText::new(text).color(ui::ACCENT_HI))
                    .on_hover_text("Không có trong danh sách: gửi nguyên tên này cho Gemini")
                    .clicked()
                {
                    picked = Some(c.clone());
                }
                ui.separator();
            }
            if matches.is_empty() && custom.is_none() {
                ui.add_space(8.0);
                ui.label(ui::muted("Không tìm thấy ngôn ngữ phù hợp."));
            }
            for &i in &matches {
                let (name, label) = all[i];
                if query.is_empty() && i == POPULAR_LANGUAGES {
                    ui.separator();
                }
                let is_sel = selected.as_str() == name;
                if ui.selectable_label(is_sel, label).on_hover_text(name).clicked() {
                    picked = Some(name.to_string());
                }
            }
            if let Some(name) = picked {
                *selected = name;
                search.clear();
                ui.close();
            }
        });
}

pub fn run(page: Option<String>) {
    // Đã có cửa sổ cài đặt: đưa lên trước thay vì mở thêm.
    let title = win::wide(win::SETTINGS_TITLE);
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow};
        let h = FindWindowW(std::ptr::null(), title.as_ptr());
        if !h.is_null() {
            ShowWindow(h, SW_RESTORE);
            SetForegroundWindow(h);
            return;
        }
    }
    let icon = image::load_from_memory(LOGO_PNG).ok().map(|img| {
        let img = img.resize(64, 64, image::imageops::FilterType::Lanczos3).to_rgba8();
        egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(win::SETTINGS_TITLE)
        .with_inner_size([920.0, 660.0])
        .with_min_inner_size([820.0, 560.0]);
    if let Some(i) = icon {
        viewport = viewport.with_icon(i);
    }
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    let _ = eframe::run_native(
        win::SETTINGS_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(SettingsApp::new(cc, page)))),
    );
}
