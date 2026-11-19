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
    UpdateChecked(Result<Option<crate::update::Release>, String>),
    UpdateInstalled(Result<std::path::PathBuf, String>),
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
    new_clip_app: String,
    update_busy: bool,
    update_status: Option<(bool, String)>,
    update_release: Option<crate::update::Release>,
    keymap: cinput_core::KeyMap,
    key_fields: Vec<String>,
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
        crate::i18n::set_lang(&cfg.ui_lang);
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
            new_clip_app: String::new(),
            update_busy: false,
            update_status: None,
            update_release: None,
            key_fields: key_fields_of(&config::load_keymap()),
            keymap: config::load_keymap(),
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
            Err(e) => self.toast(&crate::i18n::fill(crate::i18n::t("Không lưu được: {}"), &[&e])),
        }
    }

    fn save_macros(&mut self) {
        match config::save_macros(&self.macros) {
            Ok(()) => {
                win::notify_service(win::WM_CINPUT_RELOAD);
                self.toast("Đã lưu bảng gõ tắt");
            }
            Err(e) => self.toast(&crate::i18n::fill(crate::i18n::t("Không lưu được: {}"), &[&e])),
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
                ui.label(ui::muted(crate::i18n::t("Bộ gõ tiếng Việt")));
            });
        });
        ui.add_space(22.0);
        ui.spacing_mut().item_spacing.y = 4.0;
        let items = [
            (Page::Typing, icon::KEYBOARD, crate::i18n::t("Bộ gõ")),
            (Page::Macros, icon::MACRO, crate::i18n::t("Gõ tắt")),
            (Page::Convert, icon::CONVERT, crate::i18n::t("Chuyển mã")),
            (Page::Apps, icon::APPS, crate::i18n::t("Ứng dụng")),
            (Page::Ai, icon::GLOBE, crate::i18n::t("Dịch AI")),
            (Page::About, icon::INFO, crate::i18n::t("Giới thiệu")),
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
                        ui.label(RichText::new(crate::i18n::t("Bộ gõ chưa chạy")).color(ui::WARN).font(FontId::new(13.5, ui::semibold())));
                    });
                    ui.label(ui::muted(crate::i18n::t("Bấm để khởi động bộ gõ nền.")));
                    ui.add_space(4.0);
                    let btn = egui::Button::new(
                        RichText::new(crate::i18n::t("Khởi động bộ gõ")).font(FontId::new(14.0, ui::semibold())).color(ui::ON_ACCENT),
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
                        ui.label(ui::title(if on { crate::i18n::t("Tiếng Việt") } else { crate::i18n::t("Tiếng Anh") }, 15.0));
                        let detail = if on {
                            format!("{} · {}", crate::i18n::t(self.cfg.method.label()), self.cfg.charset.short())
                        } else {
                            crate::i18n::t("Đang tắt gõ dấu").to_string()
                        };
                        ui.label(ui::muted(&detail));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui::toggle(ui, &mut self.cfg.vietnamese).on_hover_text(crate::i18n::t("Bật/tắt gõ tiếng Việt"));
                    });
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for k in self.cfg.switch_key.label().split(" + ") {
                        ui::keycap(ui, k);
                    }
                    ui.label(ui::muted(crate::i18n::t("để chuyển")));
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
        Self::header(ui, crate::i18n::t("Bộ gõ"), crate::i18n::t("Kiểu gõ, bảng mã và cách đặt dấu."));
        let (english_builtin, english_user) = (self.english_builtin, self.english_user);
        let (mut english_open, mut english_reload) = (false, false);
        let cfg = &mut self.cfg;
        ui::card(ui, Some(crate::i18n::t("Kiểu gõ")), |ui| {
            ui::row(
                ui,
                crate::i18n::t("Kiểu gõ"),
                crate::i18n::t("Telex gõ dấu bằng chữ (s f r x j), VNI bằng phím số (1–9), VIQR bằng dấu câu (a' a` a? a~ a.). \
                 Tự định nghĩa: tự gán phím, đọc được file keymap.txt của UniKey."),
                |ui| {
                    egui::ComboBox::from_id_salt("method").selected_text(crate::i18n::t(cfg.method.label())).width(210.0).show_ui(ui, |ui| {
                        for m in Method::ALL {
                            ui.selectable_value(&mut cfg.method, m, m.label());
                        }
                    });
                },
            );
            ui::row_sep(ui);
            ui::row(ui, crate::i18n::t("Bảng mã"), crate::i18n::t("Bảng mã của chữ gửi vào ứng dụng. Dùng Unicode cho hầu hết trường hợp."), |ui| {
                egui::ComboBox::from_id_salt("charset")
                    .selected_text(crate::i18n::t(cfg.charset.label()))
                    .width(210.0)
                    .show_ui(ui, |ui| {
                        for c in Charset::TYPING {
                            ui.selectable_value(&mut cfg.charset, c, c.label());
                        }
                    });
            });
            ui::row_sep(ui);
            ui::row(ui, crate::i18n::t("Phím chuyển Anh/Việt"), crate::i18n::t("Phím tắt bật/tắt gõ tiếng Việt ở bất cứ đâu."), |ui| {
                ui::segmented(
                    ui,
                    &mut cfg.switch_key,
                    &[(SwitchKey::CtrlShift, "Ctrl + Shift"), (SwitchKey::AltZ, "Alt + Z")],
                );
            });
        });

        ui::card(ui, Some(crate::i18n::t("Cách đặt dấu")), |ui| {
            ui::row(ui, crate::i18n::t("Đặt dấu kiểu mới"), crate::i18n::t("hoà, khoẻ, thuý — thay cho hòa, khỏe, thúy."), |ui| {
                ui::toggle(ui, &mut cfg.modern_tone);
            });
            ui::row_sep(ui);
            ui::row(
                ui,
                crate::i18n::t("Gõ dấu tự do"),
                crate::i18n::t("Gõ dấu ở bất kỳ đâu trong từ (tiengs → tiếng). Tắt: mũ, móc, trăng, đ phải gõ ngay sau chữ cần dấu \
                 (tieengs); dấu thanh vẫn gõ cuối từ được."),
                |ui| {
                    ui::toggle(ui, &mut cfg.free_marking);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                crate::i18n::t("Phím Esc khôi phục từ"),
                crate::i18n::t("Bấm Esc ngay sau một từ để đổi qua lại giữa tiếng Việt và đúng các phím đã gõ: its → ít → Esc → its; \
                 test → Esc → tét. Hoàn tác được cả gõ tắt."),
                |ui| {
                    ui::toggle(ui, &mut cfg.restore_key);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                crate::i18n::t("Kiểm tra chính tả"),
                crate::i18n::t("Không bỏ dấu vào từ sai chính tả, tự trả lại phím gốc cho từ tiếng Anh (windows, text…)."),
                |ui| {
                    ui::toggle(ui, &mut cfg.spell_check);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                crate::i18n::t("Từ tiếng Anh giữ nguyên"),
                &crate::i18n::fill(crate::i18n::t("Gõ đúng các từ như test, error, password, data… thì giữ tiếng Anh. \
                     {} từ có sẵn + {} từ của bạn. Muốn ra \"tét\" thì gõ \"tets\"."), &[&english_builtin, &english_user]),
                |ui| {
                    ui::toggle(ui, &mut cfg.english_words);
                },
            );
            ui.horizontal_wrapped(|ui| {
                if ui::button(ui, Some(icon::FOLDER), crate::i18n::t("Sửa danh sách của tôi")).clicked() {
                    english_open = true;
                }
                if ui::button(ui, Some(icon::REFRESH), crate::i18n::t("Nạp lại")).on_hover_text(crate::i18n::t("Đọc lại danh sách sau khi sửa")).clicked() {
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
        ui::card(ui, Some(crate::i18n::t("Tự động")), |ui| {
            ui::row(
                ui,
                crate::i18n::t("Tự viết hoa đầu câu"),
                crate::i18n::t("Chữ đầu tiên sau dấu . ? ! và khi xuống dòng tự viết hoa. Bỏ qua viết tắt (v.v., TP.), \
                 dấu ba chấm và địa chỉ web."),
                |ui| {
                    ui::toggle(ui, &mut cfg.auto_capitalize);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                crate::i18n::t("Tắt tiếng Việt trong ô mật khẩu"),
                crate::i18n::t("Khi đang nhập mật khẩu, C_Input gõ ra đúng phím đã bấm (không bỏ dấu, không gõ tắt) \
                 và biểu tượng khay chuyển sang màu xám."),
                |ui| {
                    ui::toggle(ui, &mut cfg.password_off);
                },
            );
        });

        if self.cfg.method == Method::Custom {
            self.keymap_card(ui);
        }

        let method = self.cfg.method;
        ui::card(ui, Some(crate::i18n::t("Gõ thử")), |ui| {
            ui.add_space(8.0);
            ui::input_frame(ui, |ui| {
                ui.add(
                    TextEdit::singleline(&mut self.playground)
                        .frame(egui::Frame::NONE)
                        .hint_text(if method == Method::Vni {
                            crate::i18n::t("Gõ thử ở đây, ví dụ: Tie61ng Vie65t")
                        } else {
                            crate::i18n::t("Gõ thử ở đây, ví dụ: Tieesng Vieejt")
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
        ui::card(ui, Some(crate::i18n::t("Hệ thống")), |ui| {
            ui::row(ui, crate::i18n::t("Khởi động cùng Windows"), crate::i18n::t("Tự chạy C_Input khi đăng nhập Windows."), |ui| {
                ui::toggle(ui, &mut cfg.autostart);
            });
            ui::row_sep(ui);
            ui::row(ui, crate::i18n::t("Tiếng bíp khi chuyển chế độ"), crate::i18n::t("Phát âm thanh ngắn khi bật/tắt tiếng Việt."), |ui| {
                ui::toggle(ui, &mut cfg.beep);
            });
            ui::row_sep(ui);
            ui::row(ui, "Ngôn ngữ giao diện", "Interface language", |ui| {
                let mut lang: &str = if cfg.ui_lang == "en" { "en" } else { "vi" };
                ui::segmented(ui, &mut lang, &[("vi", "Tiếng Việt"), ("en", "English")]);
                if lang != cfg.ui_lang {
                    cfg.ui_lang = lang.to_string();
                    crate::i18n::set_lang(lang);
                }
            });
        });
    }

    fn cheatsheet(ui: &mut Ui, method: Method) {
        let pairs: &[(&str, &str)] = if method == Method::Vni {
            &[
                ("1", crate::i18n::t("sắc")), ("2", crate::i18n::t("huyền")), ("3", crate::i18n::t("hỏi")), ("4", crate::i18n::t("ngã")), ("5", crate::i18n::t("nặng")), ("0", crate::i18n::t("xoá dấu")),
                ("6", crate::i18n::t("â ê ô")), ("7", crate::i18n::t("ơ ư")), ("8", crate::i18n::t("ă")), ("9", crate::i18n::t("đ")),
            ]
        } else {
            let mut v: Vec<(&str, &str)> = vec![
                ("s", crate::i18n::t("sắc")), ("f", crate::i18n::t("huyền")), ("r", crate::i18n::t("hỏi")), ("x", crate::i18n::t("ngã")), ("j", crate::i18n::t("nặng")), ("z", crate::i18n::t("xoá dấu")),
                ("aa", crate::i18n::t("â")), ("aw", crate::i18n::t("ă")), ("ee", crate::i18n::t("ê")), ("oo", crate::i18n::t("ô")), ("ow", crate::i18n::t("ơ")), ("uw", crate::i18n::t("ư")), ("dd", crate::i18n::t("đ")),
            ];
            if method == Method::Telex {
                v.push(("w", crate::i18n::t("ư")));
                v.push(("[ ]", crate::i18n::t("ơ ư")));
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
        Self::header(ui, crate::i18n::t("Gõ tắt"), crate::i18n::t("Gõ một từ ngắn rồi bấm cách để thay bằng cụm từ đầy đủ."));
        let cfg = &mut self.cfg;
        ui::card(ui, Some(crate::i18n::t("Tuỳ chọn")), |ui| {
            ui::row(ui, crate::i18n::t("Bật gõ tắt"), crate::i18n::t("Thay từ tắt khi gõ dấu cách, dấu câu hoặc Enter."), |ui| {
                ui::toggle(ui, &mut cfg.macro_enabled);
            });
            ui::row_sep(ui);
            ui::row(ui, crate::i18n::t("Gõ tắt cả khi tắt tiếng Việt"), crate::i18n::t("Vẫn thay từ tắt khi đang ở chế độ tiếng Anh."), |ui| {
                ui::toggle(ui, &mut cfg.macro_always);
            });
            ui::row_sep(ui);
            ui::row(ui, crate::i18n::t("Tự viết hoa theo chữ gõ"), crate::i18n::t("vn → việt nam, Vn → Việt nam, VN → VIỆT NAM."), |ui| {
                ui::toggle(ui, &mut cfg.macro_auto_case);
            });
        });

        let mut add = false;
        ui::card(ui, Some(crate::i18n::t("Thêm gõ tắt")), |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.allocate_ui(vec2(150.0, 34.0), |ui| {
                    ui::input_frame(ui, |ui| {
                        let r = ui.add(
                            TextEdit::singleline(&mut self.new_key)
                                .frame(egui::Frame::NONE)
                                .hint_text(crate::i18n::t("Từ tắt, vd: vn"))
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
                                .hint_text(crate::i18n::t("Nội dung thay thế, vd: Việt Nam"))
                                .desired_width(f32::INFINITY),
                        );
                        add |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    });
                });
                add |= ui::primary_button(ui, crate::i18n::t("  Thêm  ")).clicked();
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
                RichText::new(crate::i18n::fill(crate::i18n::t("Danh sách ({})"), &[&self.macros.len()]))
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
                                    .hint_text(crate::i18n::t("Tìm…"))
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
                    ui.label(ui::muted(if self.macros.is_empty() { crate::i18n::t("Chưa có từ gõ tắt nào.") } else { crate::i18n::t("Không tìm thấy.") }));
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
                        if ui::icon_button(ui, icon::DELETE, ui::DANGER).on_hover_text(crate::i18n::t("Xoá")).clicked() {
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
            if ui::button(ui, Some(icon::FOLDER), crate::i18n::t("Mở file gõ tắt")).clicked() {
                if !config::macros_path().exists() {
                    let _ = config::save_macros(&self.macros);
                }
                win::shell_open(&config::macros_path());
            }
            if ui::button(ui, Some(icon::REFRESH), crate::i18n::t("Nạp lại từ file")).clicked() {
                self.macros = config::load_macros(self.cfg.macro_auto_case);
                win::notify_service(win::WM_CINPUT_RELOAD);
                self.toast("Đã nạp lại bảng gõ tắt");
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui::button(ui, Some(icon::PASTE), crate::i18n::t("Nhập từ file…")).on_hover_text(crate::i18n::t("Thêm gõ tắt từ file UniKey / C_Input")).clicked() {
                let filter = [(crate::i18n::t("Bảng gõ tắt (*.txt)"), "*.txt"), (crate::i18n::t("Mọi file"), "*.*")];
                if let Some(path) = win::file_dialog(false, crate::i18n::t("Nhập bảng gõ tắt"), &filter, "") {
                    match read_text_file(&path) {
                        Some(text) => {
                            let imported = cinput_core::MacroTable::parse(&text, self.cfg.macro_auto_case);
                            let n = imported.len();
                            for (k, v) in imported.iter() {
                                self.macros.insert(k, v);
                            }
                            self.save_macros();
                            self.toast(&crate::i18n::fill(crate::i18n::t("Đã nhập {} gõ tắt"), &[&n]));
                        }
                        None => self.toast("Không đọc được file"),
                    }
                }
            }
            if ui::button(ui, Some(icon::COPY), crate::i18n::t("Xuất ra file…")).clicked() {
                let filter = [(crate::i18n::t("Bảng gõ tắt (*.txt)"), "*.txt")];
                if let Some(path) = win::file_dialog(true, crate::i18n::t("Xuất bảng gõ tắt"), &filter, "c_input_macros.txt") {
                    match std::fs::write(&path, self.macros.serialize()) {
                        Ok(()) => self.toast("Đã xuất bảng gõ tắt"),
                        Err(e) => self.toast(&crate::i18n::fill(crate::i18n::t("Không ghi được file: {}"), &[&e])),
                    }
                }
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
        Self::header(ui, crate::i18n::t("Chuyển mã"), crate::i18n::t("Chuyển văn bản giữa các bảng mã tiếng Việt: TCVN3, VNI, Unicode…"));
        let cfg = &mut self.cfg;
        ui::card(ui, Some(crate::i18n::t("Bảng mã")), |ui| {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(ui::muted(crate::i18n::t("Từ")));
                egui::ComboBox::from_id_salt("from").selected_text(crate::i18n::t(cfg.clip_from.label())).width(190.0).show_ui(ui, |ui| {
                    for c in Charset::ALL {
                        ui.selectable_value(&mut cfg.clip_from, c, c.label());
                    }
                });
                if ui::icon_button(ui, icon::SWAP, ui::ACCENT_HI).on_hover_text(crate::i18n::t("Đảo chiều")).clicked() {
                    std::mem::swap(&mut cfg.clip_from, &mut cfg.clip_to);
                }
                ui.label(ui::muted("Sang"));
                egui::ComboBox::from_id_salt("to").selected_text(crate::i18n::t(cfg.clip_to.label())).width(190.0).show_ui(ui, |ui| {
                    for c in Charset::ALL {
                        ui.selectable_value(&mut cfg.clip_to, c, c.label());
                    }
                });
            });
            ui.add_space(4.0);
            ui::row_sep(ui);
            ui::row(ui, crate::i18n::t("Bỏ dấu"), crate::i18n::t("Tiếng Việt → Tieng Viet"), |ui| {
                ui::toggle(ui, &mut self.conv_remove_tones);
            });
            ui::row_sep(ui);
            ui::row(ui, crate::i18n::t("Chữ hoa / thường"), "", |ui| {
                ui::segmented(
                    ui,
                    &mut self.conv_case,
                    &[(CaseMode::Keep, crate::i18n::t("Giữ nguyên")), (CaseMode::Upper, crate::i18n::t("HOA")), (CaseMode::Lower, crate::i18n::t("thường"))],
                );
            });
        });

        ui::card(ui, Some(crate::i18n::t("Văn bản")), |ui| {
            ui.add_space(8.0);
            let w = (ui.available_width() - 12.0) / 2.0;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(w, 220.0), Layout::top_down(Align::Min), |ui| {
                    ui.label(ui::muted(crate::i18n::t("Nguồn")));
                    ui::input_frame(ui, |ui| {
                        ScrollArea::vertical().id_salt("in").max_height(190.0).show(ui, |ui| {
                            ui.add(
                                TextEdit::multiline(&mut self.conv_input)
                                    .frame(egui::Frame::NONE)
                                    .hint_text(crate::i18n::t("Dán văn bản cần chuyển vào đây…"))
                                    .desired_rows(9)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    });
                });
                ui.allocate_ui_with_layout(vec2(w, 220.0), Layout::top_down(Align::Min), |ui| {
                    ui.label(ui::muted(crate::i18n::t("Kết quả")));
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
                if ui::button(ui, Some(icon::PASTE), crate::i18n::t("Dán từ clipboard")).clicked() {
                    if let Some(t) = win::clipboard_text(std::ptr::null_mut()) {
                        self.conv_input = t;
                    }
                }
                if ui::primary_button(ui, crate::i18n::t("  Chuyển mã  ")).clicked() {
                    self.conv_output = self.convert_text(&self.conv_input);
                }
                if ui::button(ui, Some(icon::COPY), crate::i18n::t("Chép kết quả")).clicked()
                    && win::set_clipboard_text(std::ptr::null_mut(), &self.conv_output)
                {
                    self.toast("Đã chép kết quả vào clipboard");
                }
            });
            ui.add_space(8.0);
        });

        ui::card(ui, Some(crate::i18n::t("Chuyển nhanh clipboard")), |ui| {
            ui::row(
                ui,
                "Ctrl + Shift + F9",
                crate::i18n::t("Bấm ở bất cứ đâu để chuyển mã nội dung clipboard theo hai bảng mã ở trên."),
                |ui| {
                    if ui::button(ui, Some(icon::PLAY), crate::i18n::t("Chuyển clipboard ngay")).clicked() {
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
        Self::header(ui, crate::i18n::t("Ứng dụng"), crate::i18n::t("Chế độ gõ riêng cho từng ứng dụng."));
        let cfg = &mut self.cfg;
        ui::card(ui, Some(crate::i18n::t("Theo từng ứng dụng")), |ui| {
            ui::row(
                ui,
                crate::i18n::t("Nhớ chế độ Anh/Việt cho từng ứng dụng"),
                crate::i18n::t("Mỗi cửa sổ giữ chế độ gõ riêng khi chuyển qua lại."),
                |ui| {
                    ui::toggle(ui, &mut cfg.per_app);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                crate::i18n::t("Sửa lỗi gợi ý tự động"),
                crate::i18n::t("Thanh địa chỉ trình duyệt và Excel hay bôi đen phần gợi ý, làm mất chữ khi bỏ dấu. \
                 Áp dụng cho Chrome, Edge, Firefox, Cốc Cốc, Brave, Opera, Excel."),
                |ui| {
                    ui::toggle(ui, &mut cfg.browser_fix);
                },
            );
            ui::row_sep(ui);
            ui::row(
                ui,
                "Gõ qua phần mềm điều khiển từ xa",
                "Bỏ dấu cả với phím gửi từ UltraViewer, TeamViewer, AnyDesk, bàn phím ảo… Tắt nếu dùng phần mềm                  tự gõ chữ (AutoHotkey, macro) mà không muốn C_Input bỏ dấu vào.",
                |ui| {
                    ui::toggle(ui, &mut cfg.remote_keys);
                },
            );
        });

        let svc = win::find_service_window();
        let svc_elevated = !svc.is_null() && win::window_elevated(svc);
        ui::card(ui, Some(crate::i18n::t("Quyền Administrator")), |ui| {
            let desc = if svc_elevated {
                crate::i18n::t("C_Input đang chạy với quyền Administrator: gõ được trong mọi ứng dụng, kể cả ứng dụng quyền Admin.")
            } else {
                crate::i18n::t("Windows không cho chương trình thường gõ vào ứng dụng chạy quyền Admin (Task Manager, \
                 cmd quyền Admin, trình cài đặt…). C_Input sẽ tự tạm ngừng bỏ dấu ở đó và báo cho bạn.")
            };
            ui::row(ui, if svc_elevated { crate::i18n::t("Đang chạy quyền Admin") } else { crate::i18n::t("Chạy thường") }, desc, |ui| {
                if !svc_elevated && !svc.is_null() && ui::button(ui, None, crate::i18n::t("Chạy với quyền Admin")).clicked() {
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

        app_list_card(
            ui,
            "excluded",
            crate::i18n::t("Luôn gõ tiếng Anh trong"),
            crate::i18n::t("Trong các ứng dụng này C_Input không bỏ dấu — hữu ích cho trình soạn code, game, terminal…"),
            &mut self.cfg.excluded_apps,
            &self.running_apps,
            &mut self.new_app,
        );
        app_list_card(
            ui,
            "clipboard",
            crate::i18n::t("Gửi chữ qua clipboard"),
            crate::i18n::t("Phần mềm cũ không nhận chữ tiếng Việt từ bộ gõ (ra dấu hỏi, ô vuông)? Thêm vào đây: C_Input sẽ dán chữ \
             qua clipboard rồi trả lại nội dung clipboard của bạn."),
            &mut self.cfg.clipboard_apps,
            &self.running_apps,
            &mut self.new_clip_app,
        );
    }

    // ------------------------------------------------------------------ bảng phím tự định nghĩa

    fn keymap_card(&mut self, ui: &mut Ui) {
        let actions = cinput_core::KeyAction::all();
        let mut changed = false;
        let mut action_msg: Option<String> = None;
        ui::card(ui, Some(crate::i18n::t("Bảng phím tự định nghĩa")), |ui| {
            ui.add_space(8.0);
            ui.add(egui::Label::new(ui::muted(
                crate::i18n::t("Gõ một phím cho mỗi chức năng (để trống nếu không dùng). Lưu ngay khi sửa, dùng chung định dạng \
                 file keymap.txt với UniKey."),
            )));
            ui.add_space(6.0);
            egui::Grid::new("keymap_grid").num_columns(4).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                for (n, (a, field)) in actions.iter().zip(self.key_fields.iter_mut()).enumerate() {
                    ui.allocate_ui_with_layout(vec2(46.0, 28.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_min_size(vec2(46.0, 28.0));
                        ui::input_frame(ui, |ui| {
                            let r = ui.add(
                                TextEdit::singleline(field)
                                    .frame(egui::Frame::NONE)
                                    .char_limit(1)
                                    .horizontal_align(Align::Center)
                                    .desired_width(18.0),
                            );
                            if r.changed() {
                                let key = field.chars().next().map(|c| c.to_ascii_uppercase());
                                *field = key.map(String::from).unwrap_or_default();
                                self.keymap.set_action_key(*a, key);
                                changed = true;
                            }
                        });
                    });
                    ui.label(RichText::new(a.describe()).size(13.5));
                    if n % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui::button(ui, Some(icon::PASTE), crate::i18n::t("Nạp từ file keymap.txt…")).clicked() {
                    let filter = [(crate::i18n::t("Bảng phím UniKey (keymap.txt)"), "*.txt"), (crate::i18n::t("Mọi file"), "*.*")];
                    if let Some(path) = win::file_dialog(false, crate::i18n::t("Nạp bảng phím"), &filter, "keymap.txt") {
                        match read_text_file(&path).map(|t| cinput_core::KeyMap::parse(&t)).filter(|m| !m.is_empty()) {
                            Some(m) => {
                                self.keymap = m;
                                changed = true;
                                action_msg = Some(crate::i18n::t("Đã nạp bảng phím").into());
                            }
                            None => action_msg = Some(crate::i18n::t("File không có bảng phím hợp lệ").into()),
                        }
                    }
                }
                if ui::button(ui, Some(icon::COPY), crate::i18n::t("Lưu ra file…")).clicked() {
                    let filter = [(crate::i18n::t("Bảng phím (keymap.txt)"), "*.txt")];
                    if let Some(path) = win::file_dialog(true, crate::i18n::t("Lưu bảng phím"), &filter, "keymap.txt") {
                        action_msg = Some(match std::fs::write(&path, self.keymap.serialize()) {
                            Ok(()) => crate::i18n::t("Đã lưu bảng phím").into(),
                            Err(e) => crate::i18n::fill(crate::i18n::t("Không ghi được file: {}"), &[&e]),
                        });
                    }
                }
                if ui::button(ui, Some(icon::REFRESH), crate::i18n::t("Đặt lại giống Telex")).clicked() {
                    self.keymap = cinput_core::KeyMap::telex_like();
                    changed = true;
                }
            });
            ui.add_space(8.0);
        });
        if changed {
            self.key_fields = key_fields_of(&self.keymap);
            match config::save_keymap(&self.keymap) {
                Ok(()) => {
                    win::notify_service(win::WM_CINPUT_RELOAD);
                    if action_msg.is_none() {
                        action_msg = Some(crate::i18n::t("Đã lưu bảng phím").into());
                    }
                }
                Err(e) => action_msg = Some(crate::i18n::fill(crate::i18n::t("Không lưu được: {}"), &[&e])),
            }
        }
        if let Some(m) = action_msg {
            self.toast(&m);
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
        Self::header(ui, crate::i18n::t("Dịch AI"), crate::i18n::t("Gõ xong một câu, C_Input tự dịch câu đó sang ngôn ngữ bạn chọn bằng Google Gemini."));
        let search = &mut self.lang_search;
        let cfg = &mut self.cfg;
        ui::card(ui, Some(crate::i18n::t("Dịch tự động")), |ui| {
            ui::row(
                ui,
                crate::i18n::t("Dịch câu vừa gõ"),
                crate::i18n::t("Kết thúc câu bằng . ? hoặc ! rồi dấu cách: câu đó được thay bằng bản dịch, bạn gõ tiếp câu sau \
                 ngay, không phải chờ. Không dịch khi bấm Enter, trong ô mật khẩu và ứng dụng luôn gõ tiếng Anh."),
                |ui| {
                    ui::toggle(ui, &mut cfg.translate_enabled);
                },
            );
            if cfg.translate_enabled {
                ui::row_sep(ui);
                ui::row(ui, crate::i18n::t("Dịch sang"), crate::i18n::t("Ngôn ngữ đích của bản dịch. Gõ để tìm nhanh, không cần dấu."), |ui| {
                    language_picker(ui, "tr_lang", &mut cfg.translate_lang, search);
                });
            }
        });
        let read_search = &mut self.read_search;
        let cfg = &mut self.cfg;
        ui::card(ui, Some(crate::i18n::t("Dịch đoạn bôi đen")), |ui| {
            ui::row(
                ui,
                crate::i18n::t("Bấm Ctrl+C hai lần để dịch"),
                crate::i18n::t("Bôi đen một đoạn chữ ở bất kỳ đâu (tin nhắn đến, trang web, tài liệu) rồi bấm Ctrl+C hai lần liền: \
                 bản dịch hiện trong cửa sổ nhỏ cạnh con trỏ, đổi ngôn ngữ ngay trong đó. Văn bản gốc không bị sửa."),
                |ui| {
                    ui::toggle(ui, &mut cfg.read_enabled);
                },
            );
            if cfg.read_enabled {
                ui::row_sep(ui);
                ui::row(ui, crate::i18n::t("Dịch sang"), crate::i18n::t("Ngôn ngữ bạn đọc — thường là tiếng Việt."), |ui| {
                    language_picker(ui, "read_lang_settings", &mut cfg.read_lang, read_search);
                });
            }
        });
        if !self.cfg.translate_enabled && !self.cfg.read_enabled {
            ui.add(egui::Label::new(ui::muted(
                crate::i18n::t("Bật một trong hai tính năng trên để gắn API key Gemini. Khi tắt cả hai, C_Input không gửi gì lên mạng."),
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
                        ui.label(RichText::new(crate::i18n::fill(crate::i18n::t("Đã lưu key ••••{}"), &[&tail])).size(14.0));
                        ui.label(ui::muted(crate::i18n::t("(mã hoá bằng Windows, chỉ tài khoản của bạn đọc được)")));
                    });
                }
                None => {
                    ui.horizontal(|ui| {
                        ui.label(ui::icon_text(icon::WARNING, 13.0, ui::WARN));
                        ui.label(RichText::new(crate::i18n::t("Chưa có API key")).color(ui::WARN));
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
                                .hint_text(if saved.is_some() { crate::i18n::t("Dán key mới để thay") } else { crate::i18n::t("Dán API key Gemini vào đây") })
                                .desired_width(f32::INFINITY),
                        );
                        save |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    });
                });
                save |= ui::primary_button(ui, crate::i18n::t("  Lưu key  ")).clicked();
                if saved.is_some() {
                    check |= ui::button(ui, Some(icon::REFRESH), crate::i18n::t("Kiểm tra")).clicked();
                    remove |= ui::icon_button(ui, icon::DELETE, ui::DANGER).on_hover_text(crate::i18n::t("Xoá key")).clicked();
                }
            });
            ui.horizontal(|ui| {
                ui.label(ui::muted(crate::i18n::t("Chưa có key? Tạo miễn phí tại")));
                ui.hyperlink_to("aistudio.google.com", crate::ai::KEY_URL);
            });
            if self.ai_busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(ui::muted(crate::i18n::t("Đang liên hệ Gemini…")));
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
            ui::row(ui, "Model", crate::i18n::t("Model \"flash\" nhanh và rẻ, hợp để dịch từng câu. Bấm Kiểm tra để lấy danh sách."), |ui| {
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
        ui::card(ui, Some(crate::i18n::t("Dịch thử")), |ui| {
            ui.add_space(8.0);
            ui::input_frame(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut self.try_input)
                        .frame(egui::Frame::NONE)
                        .hint_text(crate::i18n::t("Gõ một câu để thử, ví dụ: Hôm nay trời đẹp quá."))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                run_try = ui::primary_button(ui, crate::i18n::t("  Dịch thử  ")).clicked();
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
            ui.label(ui::muted(&crate::i18n::fill(crate::i18n::t("Phiên bản {}"), &[&env!("CARGO_PKG_VERSION")])));
            ui.add_space(4.0);
            ui.label(RichText::new(crate::i18n::t("Bộ gõ tiếng Việt cho Windows — nhẹ, nhanh, chạy hoàn toàn trên máy.")).color(ui::MUTED));
        });
        ui.add_space(14.0);
        ui::card(ui, Some(crate::i18n::t("Phím tắt")), |ui| {
            let rows: [(&[&str], &str); 3] = [
                (&[self.cfg.switch_key.label()], crate::i18n::t("Bật/tắt gõ tiếng Việt")),
                (&["Ctrl", "Shift", "F5"], crate::i18n::t("Mở cửa sổ cài đặt")),
                (&["Ctrl", "Shift", "F9"], crate::i18n::t("Chuyển mã clipboard")),
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
        let mut check = false;
        let mut install = false;
        let cfg = &mut self.cfg;
        ui::card(ui, Some(crate::i18n::t("Cập nhật")), |ui| {
            ui::row(
                ui,
                crate::i18n::t("Tự kiểm tra bản mới"),
                crate::i18n::t("Mỗi ngày hỏi GitHub một lần, có bản mới thì báo ở khay hệ thống. Không tự cài."),
                |ui| {
                    ui::toggle(ui, &mut cfg.auto_update);
                },
            );
            ui::row_sep(ui);
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!self.update_busy, egui::Button::new(crate::i18n::t("Kiểm tra cập nhật"))).clicked() {
                    check = true;
                }
                if let Some(rel) = &self.update_release {
                    let label = crate::i18n::fill(crate::i18n::t("Cập nhật lên {}"), &[&rel.version]);
                    if ui.add_enabled(!self.update_busy, egui::Button::new(RichText::new(label).color(ui::ACCENT_HI))).clicked() {
                        install = true;
                    }
                }
                ui.hyperlink_to(crate::i18n::t("Xem các bản phát hành"), crate::update::RELEASES_URL);
            });
            if self.update_busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(ui::muted(crate::i18n::t("Đang liên hệ GitHub…")));
                });
            } else if let Some((ok, msg)) = &self.update_status {
                ui.add(egui::Label::new(RichText::new(msg).color(if *ok { ui::MUTED } else { ui::DANGER })).wrap());
            }
            if let Some(rel) = &self.update_release {
                if !rel.notes.trim().is_empty() {
                    ui::input_frame(ui, |ui| {
                        ScrollArea::vertical().id_salt("notes").max_height(140.0).show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(rel.notes.trim()).size(12.5).color(ui::MUTED)).wrap());
                        });
                    });
                }
                ui.label(ui::muted(crate::i18n::t("Khi cập nhật: tải bản mới, kiểm tra mã SHA-256, thay file rồi khởi động lại C_Input.")));
            }
            ui.add_space(8.0);
        });
        if check {
            self.update_busy = true;
            self.update_status = None;
            let tx = self.ai_tx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(AiMsg::UpdateChecked(crate::update::check()));
            });
        }
        if install {
            if let Some(rel) = self.update_release.clone() {
                self.update_busy = true;
                self.update_status = None;
                let tx = self.ai_tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(AiMsg::UpdateInstalled(crate::update::install(&rel)));
                });
            }
        }

        ui::card(ui, Some(crate::i18n::t("Thông tin")), |ui| {
            ui.add_space(8.0);
            for line in [
                crate::i18n::t("Lõi gõ viết bằng Rust: Telex, VNI, đặt dấu tự do, kiểm tra chính tả, tự khôi phục từ tiếng Anh."),
                crate::i18n::t("Không gửi dữ liệu ra ngoài, không cần mạng."),
                crate::i18n::t("Bảng mã TCVN3 / VNI Windows tham khảo từ mã nguồn UniKey 3.62 của Phạm Kim Long (GPL)."),
                crate::i18n::t("Cấu hình lưu tại %APPDATA%\\C_Input."),
            ] {
                ui.horizontal(|ui| {
                    ui.label(ui::icon_text(icon::CHECK, 12.0, ui::ACCENT_HI));
                    ui.add(egui::Label::new(RichText::new(line).color(ui::MUTED)).wrap());
                });
            }
            ui.add_space(4.0);
            if ui::button(ui, Some(icon::FOLDER), crate::i18n::t("Mở thư mục cấu hình")).clicked() {
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
                    self.ai_status = Some((true, crate::i18n::fill(crate::i18n::t("API key hoạt động — {} model dùng được."), &[&n])));
                }
                AiMsg::Models(Err(e)) => self.ai_status = Some((false, e)),
                AiMsg::Translated(Ok(t)) => self.try_output = t,
                AiMsg::Translated(Err(e)) => self.ai_status = Some((false, e)),
                AiMsg::UpdateChecked(r) => {
                    self.update_busy = false;
                    match r {
                        Ok(Some(rel)) => {
                            self.update_status = Some((true, crate::i18n::fill(crate::i18n::t("Đã có bản {} (bạn đang dùng {})."), &[&rel.version, &env!("CARGO_PKG_VERSION")])));
                            self.update_release = Some(rel);
                        }
                        Ok(None) => {
                            self.update_status = Some((true, crate::i18n::fill(crate::i18n::t("Bạn đang dùng bản mới nhất ({})."), &[&env!("CARGO_PKG_VERSION")])));
                            self.update_release = None;
                        }
                        Err(e) => self.update_status = Some((false, e)),
                    }
                }
                AiMsg::UpdateInstalled(r) => {
                    self.update_busy = false;
                    match r {
                        Ok(exe) => {
                            // Khởi động lại bộ gõ bằng bản mới rồi đóng cửa sổ cài đặt (bản cũ).
                            let svc = win::find_service_window();
                            let _ = std::process::Command::new(&exe).arg("--replace").spawn();
                            if !svc.is_null() {
                                unsafe {
                                    windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                                        svc,
                                        windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                                        0,
                                        0,
                                    );
                                }
                            }
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(e) => self.update_status = Some((false, e)),
                    }
                }
            }
        }
        if self.ai_busy || self.update_busy {
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
                            .hint_text(crate::i18n::fill(crate::i18n::t("Tìm trong {} ngôn ngữ… (vd: dai loan, japan)"), &[&all.len()]))
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
                let text = crate::i18n::fill(crate::i18n::t("Dùng ngôn ngữ «{}»"), &[&c]);
                if ui.selectable_label(selected.as_str() == c, RichText::new(text).color(ui::ACCENT_HI))
                    .on_hover_text(crate::i18n::t("Không có trong danh sách: gửi nguyên tên này cho Gemini"))
                    .clicked()
                {
                    picked = Some(c.clone());
                }
                ui.separator();
            }
            if matches.is_empty() && custom.is_none() {
                ui.add_space(8.0);
                ui.label(ui::muted(crate::i18n::t("Không tìm thấy ngôn ngữ phù hợp.")));
            }
            for &i in &matches {
                let (name, label) = all[i];
                let label = if crate::i18n::is_en() { name } else { label };
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

/// Ô phím cho từng chức năng của bảng phím (theo thứ tự `KeyAction::all()`).
fn key_fields_of(m: &cinput_core::KeyMap) -> Vec<String> {
    cinput_core::KeyAction::all()
        .into_iter()
        .map(|a| m.key_for(a).map(|c| c.to_ascii_uppercase().to_string()).unwrap_or_default())
        .collect()
}

/// Đọc file văn bản: UTF-8 (có/không BOM) hoặc UTF-16 LE (file UniKey cũ).
fn read_text_file(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return Some(String::from_utf16_lossy(&units));
    }
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    Some(String::from_utf8_lossy(b).into_owned())
}

/// Thẻ danh sách ứng dụng (tên .exe): chọn từ ứng dụng đang mở hoặc gõ tên, xoá từng mục.
fn app_list_card(
    ui: &mut Ui,
    id: &str,
    heading: &str,
    desc: &str,
    list: &mut Vec<String>,
    running: &[String],
    new_text: &mut String,
) {
    let mut add: Option<String> = None;
    let mut remove = None;
    ui::card(ui, Some(heading), |ui| {
        ui.add_space(8.0);
        ui.add(egui::Label::new(ui::muted(desc)).wrap());
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt(("running", id)).selected_text(crate::i18n::t("Chọn ứng dụng đang mở…")).width(230.0).show_ui(
                ui,
                |ui| {
                    for a in running {
                        if ui.selectable_label(false, a).clicked() {
                            add = Some(a.clone());
                        }
                    }
                },
            );
            ui.label(ui::muted(crate::i18n::t("hoặc")));
            ui.allocate_ui(vec2(190.0, 34.0), |ui| {
                ui::input_frame(ui, |ui| {
                    let r = ui.add(
                        TextEdit::singleline(new_text)
                            .id_salt(("app_input", id))
                            .frame(egui::Frame::NONE)
                            .hint_text(crate::i18n::t("tên file, vd: code.exe"))
                            .desired_width(f32::INFINITY),
                    );
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        add = Some(new_text.clone());
                    }
                });
            });
            if ui::primary_button(ui, crate::i18n::t("  Thêm  ")).clicked() {
                add = Some(new_text.clone());
            }
        });
        ui.add_space(8.0);
        if list.is_empty() {
            ui::row_sep(ui);
            ui.add_space(12.0);
            ui.vertical_centered(|ui| ui.label(ui::muted(crate::i18n::t("Chưa có ứng dụng nào."))));
            ui.add_space(12.0);
        }
        for (i, a) in list.iter().enumerate() {
            ui::row_sep(ui);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(vec2(28.0, 36.0), Layout::left_to_right(Align::Center), |ui| {
                    ui.set_min_size(vec2(28.0, 36.0));
                    ui.label(ui::icon_text(icon::APPS, 14.0, ui::MUTED));
                });
                ui.label(RichText::new(a).size(14.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::icon_button(ui, icon::DELETE, ui::DANGER).on_hover_text(crate::i18n::t("Bỏ khỏi danh sách")).clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
    });
    if let Some(i) = remove {
        list.remove(i);
    }
    if let Some(a) = add {
        let mut a = a.trim().to_lowercase();
        if !a.is_empty() {
            if !a.ends_with(".exe") {
                a.push_str(".exe");
            }
            if !list.iter().any(|x| x.eq_ignore_ascii_case(&a)) {
                list.push(a);
            }
            new_text.clear();
        }
    }
}

pub fn run(page: Option<String>) {
    // Đã có cửa sổ cài đặt: đưa lên trước thay vì mở thêm.
    let title = win::wide(win::SETTINGS_TITLE);
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow};
        let h = FindWindowW(std::ptr::null(), title.as_ptr());
        if !h.is_null() && std::env::var_os("CINPUT_TEST_MULTI").is_none() {
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
