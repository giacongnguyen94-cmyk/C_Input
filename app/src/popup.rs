//! Cửa sổ "Dịch đoạn bôi đen": bôi đen chữ ở bất kỳ đâu rồi bấm Ctrl+C hai lần → cửa sổ nhỏ hiện cạnh
//! con trỏ với bản dịch sang ngôn ngữ đọc (mặc định tiếng Việt); đổi ngôn ngữ để dịch lại ngay, chép bản dịch.
//! Văn bản gốc không bị sửa. Chạy như một tiến trình riêng: `C_Input.exe --popup <file> <x> <y>`.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, CornerRadius, FontId, Frame, Layout, Margin, RichText, ScrollArea, Stroke, TextEdit, Ui, vec2};

use crate::config::Config;
use crate::ui::{self, icon};
use crate::{ai, win};

pub const POPUP_TITLE: &str = "C_Input — Dịch";
pub const POPUP_SIZE: [f32; 2] = [460.0, 340.0];
/// Độ dài tối đa gửi đi dịch.
const MAX_CHARS: usize = 6000;

struct PopupApp {
    source: String,
    cfg: Config,
    key: Option<String>,
    search: String,
    result: Option<Result<String, String>>,
    busy: bool,
    tx: Sender<(String, Result<String, String>)>,
    rx: Receiver<(String, Result<String, String>)>,
    /// Ngôn ngữ của bản dịch đang hiển thị / đang chờ.
    current_lang: String,
    focused_once: bool,
    opened: Instant,
    copied_at: Option<Instant>,
}

impl PopupApp {
    fn translate(&mut self) {
        let Some(key) = self.key.clone() else {
            if !fake() {
                self.result = Some(Err(crate::i18n::t("Chưa có API key Gemini — mở Cài đặt → Dịch AI để thêm.").to_string()));
                return;
            }
            return self.spawn(String::new());
        };
        self.spawn(key);
    }

    fn spawn(&mut self, key: String) {
        self.busy = true;
        self.result = None;
        self.current_lang = self.cfg.read_lang.clone();
        let (tx, lang, model, text) =
            (self.tx.clone(), self.cfg.read_lang.clone(), self.cfg.gemini_model.clone(), self.source.clone());
        std::thread::spawn(move || {
            let r = if fake() {
                std::thread::sleep(Duration::from_millis(400));
                Ok(format!("[{lang}] {}", text.to_uppercase()))
            } else {
                ai::translate(&key, &model, &lang, &text)
            };
            let _ = tx.send((lang, r));
        });
    }

    /// Ghi nhớ ngôn ngữ đọc vừa chọn cho lần sau.
    fn remember_language(&self) {
        let mut c = Config::load();
        if c.read_lang != self.cfg.read_lang {
            c.read_lang = self.cfg.read_lang.clone();
            let _ = c.save();
            win::notify_service(win::WM_CINPUT_RELOAD);
        }
    }
}

/// Bộ dịch giả cho kiểm thử tự động (không cần API key).
fn fake() -> bool {
    std::env::var_os("CINPUT_TEST_FAKE_TRANSLATE").is_some()
}

impl eframe::App for PopupApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok((lang, r)) = self.rx.try_recv() {
            // Bỏ kết quả của ngôn ngữ cũ nếu người dùng đã đổi sang ngôn ngữ khác.
            if lang == self.current_lang {
                self.busy = false;
                self.result = Some(r);
            }
        }
        // Đóng: Esc, hoặc bấm ra ngoài (mất focus sau khi đã có focus).
        let focused = ctx.input(|i| i.viewport().focused);
        if focused == Some(true) {
            self.focused_once = true;
        }
        let lost = self.focused_once && focused == Some(false) && self.opened.elapsed() > Duration::from_millis(400);
        if lost || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.busy || self.copied_at.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(ui::CARD)
                    .stroke(Stroke::new(1.0, ui::ACCENT))
                    .corner_radius(CornerRadius::same(0))
                    .inner_margin(Margin::same(14)),
            )
            .show(ui, |ui| {
                // Thanh tiêu đề: kéo để di chuyển cửa sổ.
                let header = ui.horizontal(|ui| {
                    ui.label(ui::icon_text(icon::GLOBE, 15.0, ui::ACCENT_HI));
                    ui.label(ui::title(crate::i18n::t("Dịch sang"), 14.5));
                    let before = self.cfg.read_lang.clone();
                    crate::settings::language_picker(ui, "read_lang", &mut self.cfg.read_lang, &mut self.search);
                    if self.cfg.read_lang != before {
                        self.remember_language();
                        self.translate();
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::icon_button(ui, "\u{E8BB}", ui::DANGER).on_hover_text(crate::i18n::t("Đóng (Esc)")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
                let drag = ui.interact(header.response.rect, ui.id().with("drag"), egui::Sense::drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.add_space(6.0);
                // Văn bản gốc (thu gọn).
                ui::input_frame(ui, |ui| {
                    ScrollArea::vertical().id_salt("src").max_height(54.0).show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(&self.source).size(12.5).color(ui::MUTED)).wrap());
                    });
                });
                ui.add_space(8.0);

                // Bản dịch.
                let avail = ui.available_height() - 44.0;
                ui.allocate_ui(vec2(ui.available_width(), avail.max(60.0)), |ui| match &self.result {
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(ui::muted(&crate::i18n::fill(crate::i18n::t("Đang dịch sang {}…"), &[&ai::language_label(&self.cfg.read_lang)])));
                        });
                    }
                    Some(Ok(text)) => {
                        ScrollArea::vertical().id_salt("out").max_height(avail.max(60.0)).show(ui, |ui| {
                            ui.add(
                                TextEdit::multiline(&mut text.as_str())
                                    .frame(egui::Frame::NONE)
                                    .font(FontId::proportional(15.0))
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    }
                    Some(Err(e)) => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(ui::icon_text(icon::WARNING, 13.0, ui::DANGER));
                            ui.add(egui::Label::new(RichText::new(e).color(ui::DANGER)).wrap());
                        });
                        if self.key.is_none() && ui::button(ui, Some(icon::GLOBE), crate::i18n::t("Mở cài đặt Dịch AI")).clicked() {
                            win::spawn_self(&["--settings", "ai"]);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });

                // Nút dưới cùng.
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.horizontal(|ui| {
                        let ok = matches!(self.result, Some(Ok(_)));
                        let copied = self.copied_at.is_some_and(|t| t.elapsed() < Duration::from_millis(1500));
                        if ui
                            .add_enabled(ok, egui::Button::new(if copied { crate::i18n::t("✓ Đã chép") } else { crate::i18n::t("Chép bản dịch") }))
                            .clicked()
                        {
                            if let Some(Ok(t)) = &self.result {
                                win::set_clipboard_text(std::ptr::null_mut(), t);
                                self.copied_at = Some(Instant::now());
                            }
                        }
                        if ui.add_enabled(!self.busy, egui::Button::new(crate::i18n::t("Dịch lại"))).clicked() {
                            self.translate();
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(ui::muted(crate::i18n::t("Esc để đóng")));
                        });
                    });
                });
            });
    }
}

/// `args`: [đường dẫn file chứa đoạn chữ, x, y] (toạ độ màn hình, góc trên trái của cửa sổ).
pub fn run(args: &[String]) {
    let Some(path) = args.first() else { return };
    let source = std::fs::read_to_string(path).unwrap_or_default();
    let _ = std::fs::remove_file(path);
    let source: String = source.trim().chars().take(MAX_CHARS).collect();
    if source.is_empty() {
        return;
    }
    let x = args.get(1).and_then(|v| v.parse::<f32>().ok()).unwrap_or(200.0);
    let y = args.get(2).and_then(|v| v.parse::<f32>().ok()).unwrap_or(200.0);

    let viewport = egui::ViewportBuilder::default()
        .with_title(POPUP_TITLE)
        .with_inner_size(POPUP_SIZE)
        .with_position([x, y])
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_always_on_top()
        .with_active(true);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    let _ = eframe::run_native(
        POPUP_TITLE,
        options,
        Box::new(move |cc| {
            ui::setup(&cc.egui_ctx);
            let cfg = Config::load();
            crate::i18n::set_lang(&cfg.ui_lang);
            let (tx, rx) = channel();
            let mut app = PopupApp {
                key: ai::unprotect_key(&cfg.gemini_key),
                current_lang: cfg.read_lang.clone(),
                cfg,
                source,
                search: String::new(),
                result: None,
                busy: false,
                tx,
                rx,
                focused_once: false,
                opened: Instant::now(),
                copied_at: None,
            };
            app.translate();
            Ok(Box::new(app))
        }),
    );
}
