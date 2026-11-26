//! Giao diện dòng C: nền tối, thẻ bo góc, màu nhấn xanh ngọc lục bảo. Các widget dùng chung.

use std::sync::Arc;

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Layout, Margin,
    Response, RichText, Sense, Stroke, StrokeKind, TextStyle, Ui, Vec2, vec2,
};

pub const BG: Color32 = Color32::from_rgb(0x10, 0x10, 0x12);
pub const SIDEBAR: Color32 = Color32::from_rgb(0x0A, 0x0A, 0x0B);
pub const CARD: Color32 = Color32::from_rgb(0x18, 0x18, 0x1B);
pub const RAISED: Color32 = Color32::from_rgb(0x21, 0x21, 0x25);
pub const INPUT: Color32 = Color32::from_rgb(0x0D, 0x0D, 0x0F);
pub const BORDER: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x30);
pub const TEXT: Color32 = Color32::from_rgb(0xEE, 0xEE, 0xF0);
pub const MUTED: Color32 = Color32::from_rgb(0x9C, 0x9C, 0xA6);
pub const FAINT: Color32 = Color32::from_rgb(0x5E, 0x5E, 0x68);
pub const ACCENT: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
pub const ACCENT_HI: Color32 = Color32::from_rgb(0x34, 0xD3, 0x99);
pub const ACCENT_BG: Color32 = Color32::from_rgba_premultiplied(4, 46, 32, 70);
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x03, 0x1A, 0x12);
pub const WARN: Color32 = Color32::from_rgb(0xFB, 0xBF, 0x24);
pub const DANGER: Color32 = Color32::from_rgb(0xF8, 0x71, 0x71);

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

fn icons_family() -> FontFamily {
    FontFamily::Name("icons".into())
}

/// Biểu tượng Segoe MDL2 Assets (có sẵn trên Windows 10/11).
pub mod icon {
    pub const KEYBOARD: &str = "\u{E765}";
    pub const MACRO: &str = "\u{E8C8}";
    pub const CONVERT: &str = "\u{E8AB}";
    pub const APPS: &str = "\u{E71D}";
    pub const INFO: &str = "\u{E946}";
    pub const GLOBE: &str = "\u{E774}";
    pub const DELETE: &str = "\u{E74D}";
    pub const SEARCH: &str = "\u{E721}";
    pub const FOLDER: &str = "\u{E838}";
    pub const REFRESH: &str = "\u{E72C}";
    pub const SWAP: &str = "\u{E8AB}";
    pub const CHECK: &str = "\u{E73E}";
    pub const WARNING: &str = "\u{E7BA}";
    pub const PLAY: &str = "\u{E768}";
    pub const PASTE: &str = "\u{E77F}";
    pub const COPY: &str = "\u{E8C8}";
}

pub fn setup(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fonts_dir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into())
        .join("Fonts");
    let mut load = |name: &str, file: &str| -> bool {
        match std::fs::read(fonts_dir.join(file)) {
            Ok(bytes) => {
                fonts.font_data.insert(name.into(), Arc::new(FontData::from_owned(bytes)));
                true
            }
            Err(_) => false,
        }
    };
    let has_segoe = load("segoe", "segoeui.ttf");
    let has_sb = load("segoe-sb", "seguisb.ttf");
    let has_icons = load("mdl2", "segmdl2.ttf");

    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    if has_segoe {
        prop.insert(0, "segoe".into());
    }
    let mut sb = Vec::new();
    if has_sb {
        sb.push("segoe-sb".to_string());
    }
    sb.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
    fonts.families.insert(semibold(), sb);
    let mut ic = Vec::new();
    if has_icons {
        ic.push("mdl2".to_string());
    }
    ic.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
    fonts.families.insert(icons_family(), ic);
    if has_segoe {
        fonts.families.entry(FontFamily::Monospace).or_default().push("segoe".into());
    }
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (TextStyle::Heading, FontId::new(24.0, semibold())),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(14.0, 7.0);
        s.spacing.interact_size.y = 32.0;
        s.spacing.combo_width = 210.0;
        s.spacing.scroll.bar_width = 6.0;
        s.spacing.scroll.floating = true;

        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = BG;
        v.window_fill = CARD;
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = INPUT;
        v.faint_bg_color = CARD;
        v.override_text_color = Some(TEXT);
        v.hyperlink_color = ACCENT_HI;
        v.selection.bg_fill = Color32::from_rgba_premultiplied(8, 92, 64, 140);
        v.selection.stroke = Stroke::new(1.0, ACCENT_HI);
        v.text_cursor.stroke = Stroke::new(2.0, ACCENT_HI);
        let r = CornerRadius::same(8);
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = r;
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.inactive.bg_fill = RAISED;
        v.widgets.inactive.weak_bg_fill = RAISED;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.hovered.bg_fill = Color32::from_rgb(0x2A, 0x2A, 0x2F);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2A, 0x2A, 0x2F);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x3A, 0x3A, 0x42));
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.active.bg_fill = Color32::from_rgb(0x33, 0x33, 0x39);
        v.widgets.active.weak_bg_fill = Color32::from_rgb(0x33, 0x33, 0x39);
        v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.open.weak_bg_fill = RAISED;
        v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);
    });
}

pub fn icon_text(glyph: &str, size: f32, color: Color32) -> RichText {
    RichText::new(glyph).font(FontId::new(size, icons_family())).color(color)
}

pub fn title(text: &str, size: f32) -> RichText {
    RichText::new(crate::i18n::t(text)).font(FontId::new(size, semibold())).color(TEXT)
}

pub fn muted(text: &str) -> RichText {
    RichText::new(crate::i18n::t(text)).size(12.5).color(MUTED)
}

/// Công tắc bật/tắt.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = vec2(44.0, 24.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    if ui.is_rect_visible(rect) {
        let off_bg = if resp.hovered() { Color32::from_rgb(0x3A, 0x3A, 0x42) } else { Color32::from_rgb(0x30, 0x30, 0x37) };
        let bg = lerp_color(off_bg, ACCENT, t);
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(12), bg);
        let r = 9.0;
        let x = egui::lerp((rect.left() + 12.0)..=(rect.right() - 12.0), t);
        p.circle_filled(egui::pos2(x, rect.center().y), r, Color32::WHITE);
    }
    resp
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Nhóm nút chọn một (segmented control). Trả về `true` nếu đổi lựa chọn.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    // Đo trước để cấp phát đúng một khối, vẽ tay từng ô (không phụ thuộc hướng layout của dòng chứa nó).
    let (pad, gap, seg_h) = (3.0, 2.0, 28.0);
    let galleys: Vec<_> = options
        .iter()
        .map(|(opt, label)| {
            let selected = *value == *opt;
            let font = FontId::new(13.5, if selected { semibold() } else { FontFamily::Proportional });
            ui.painter().layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER)
        })
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 26.0).collect();
    let total = widths.iter().sum::<f32>() + gap * (widths.len().saturating_sub(1)) as f32 + pad * 2.0;
    let (outer, _) = ui.allocate_exact_size(vec2(total, seg_h + pad * 2.0), Sense::hover());
    ui.painter().rect_filled(outer, CornerRadius::same(9), INPUT);
    ui.painter().rect_stroke(outer, CornerRadius::same(9), Stroke::new(1.0, BORDER), StrokeKind::Inside);

    let mut changed = false;
    let mut x = outer.left() + pad;
    for (i, ((opt, label), galley)) in options.iter().zip(galleys).enumerate() {
        let rect = egui::Rect::from_min_size(egui::pos2(x, outer.top() + pad), vec2(widths[i], seg_h));
        x += widths[i] + gap;
        let resp = ui.interact(rect, ui.id().with(("seg", *label)), Sense::click());
        let selected = *value == *opt;
        if resp.clicked() && !selected {
            *value = *opt;
            changed = true;
        }
        let fill = if selected {
            ACCENT
        } else if resp.hovered() {
            RAISED
        } else {
            Color32::TRANSPARENT
        };
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(7), fill);
        let color = if selected { ON_ACCENT } else if resp.hovered() { TEXT } else { MUTED };
        p.galley(rect.center() - galley.size() / 2.0, galley, color);
        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    changed
}

/// Thẻ nhóm cài đặt, có tiêu đề nhỏ phía trên.
pub fn card<R>(ui: &mut Ui, heading: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    if let Some(h) = heading {
        ui.add_space(6.0);
        ui.label(RichText::new(crate::i18n::t(h)).font(FontId::new(13.0, semibold())).color(MUTED));
        ui.add_space(2.0);
    }
    let r = Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(18, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner;
    ui.add_space(10.0);
    r
}

/// Một dòng cài đặt: tên + mô tả bên trái, điều khiển bên phải.
/// Dưới độ rộng này, dòng cài đặt xếp điều khiển xuống dưới phần mô tả (cửa sổ hẹp).
const ROW_STACK_WIDTH: f32 = 640.0;

/// Một dòng cài đặt: tên + mô tả bên trái, điều khiển bên phải.
/// Cửa sổ hẹp thì tự xếp điều khiển xuống dưới, không bao giờ đẩy trang rộng quá khung nhìn.
pub fn row(ui: &mut Ui, name: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    let (name, desc) = (crate::i18n::t(name), crate::i18n::t(desc));
    ui.add_space(6.0);
    let avail = ui.available_width();
    let labels = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.add(egui::Label::new(RichText::new(name).size(14.5).color(TEXT)).wrap());
        if !desc.is_empty() {
            ui.add(egui::Label::new(muted(desc)).wrap());
        }
    };
    if avail < ROW_STACK_WIDTH {
        ui.vertical(|ui| {
            ui.set_max_width(avail);
            labels(ui);
            ui.add_space(6.0);
            ui.horizontal_wrapped(control);
        });
    } else {
        ui.horizontal(|ui| {
            let control_w = (avail * 0.42).clamp(220.0, 340.0);
            let left_w = avail - control_w - 16.0;
            ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), labels);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), Layout::right_to_left(Align::Center), control);
        });
    }
    ui.add_space(6.0);
}

/// Ô nhỏ "phím → kết quả" (vd: "aa â"). Đo trước kích thước rồi mới vẽ, nên khi xuống dòng
/// trong `horizontal_wrapped` luôn đi nguyên khối, không bị bóp dọc.
pub fn key_chip(ui: &mut Ui, key: &str, result: &str) {
    let k = ui.painter().layout_no_wrap(key.to_string(), FontId::new(13.0, semibold()), ACCENT_HI);
    let v = ui.painter().layout_no_wrap(result.to_string(), FontId::proportional(13.0), MUTED);
    let (pad_x, gap, h) = (8.0, 5.0, 24.0);
    let size = vec2(pad_x * 2.0 + k.size().x + gap + v.size().x, h);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(7), INPUT);
    p.rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.0, BORDER), StrokeKind::Inside);
    let y = rect.center().y;
    let kx = rect.left() + pad_x;
    p.galley(egui::pos2(kx, y - k.size().y / 2.0), k.clone(), ACCENT_HI);
    p.galley(egui::pos2(kx + k.size().x + gap, y - v.size().y / 2.0), v, MUTED);
}

pub fn row_sep(ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    let y = rect.top();
    ui.painter().hline(rect.left()..=rect.right(), y, Stroke::new(1.0, BORDER));
    ui.add_space(1.0);
}

pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(crate::i18n::t(text)).font(FontId::new(14.0, semibold())).color(ON_ACCENT))
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(8))
            .min_size(vec2(0.0, 34.0)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn button(ui: &mut Ui, glyph: Option<&str>, text: &str) -> Response {
    let text = crate::i18n::t(text);
    let mut job = egui::text::LayoutJob::default();
    if let Some(g) = glyph {
        job.append(g, 0.0, egui::TextFormat { font_id: FontId::new(13.0, icons_family()), color: TEXT, valign: Align::Center, ..Default::default() });
        job.append(text, 8.0, egui::TextFormat { font_id: FontId::proportional(14.0), color: TEXT, valign: Align::Center, ..Default::default() });
    } else {
        job.append(text, 0.0, egui::TextFormat { font_id: FontId::proportional(14.0), color: TEXT, ..Default::default() });
    }
    ui.add(egui::Button::new(job).min_size(vec2(0.0, 34.0)))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Nút chỉ có biểu tượng (vd: xoá).
pub fn icon_button(ui: &mut Ui, glyph: &str, hover_color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.painter().rect_filled(rect, CornerRadius::same(7), RAISED);
    }
    let color = if hovered { hover_color } else { FAINT };
    ui.painter().text(rect.center(), Align2C, glyph, FontId::new(13.0, icons_family()), color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[allow(non_upper_case_globals)]
const Align2C: egui::Align2 = egui::Align2::CENTER_CENTER;

/// Phím bấm hiển thị kiểu nắp phím.
pub fn keycap(ui: &mut Ui, text: &str) {
    Frame::new()
        .fill(RAISED)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).font(FontId::new(12.5, semibold())).color(TEXT));
        });
}

/// Mục điều hướng ở thanh bên.
pub fn nav_item(ui: &mut Ui, glyph: &str, label: &str, selected: bool) -> Response {
    let label = crate::i18n::t(label);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(9), ACCENT_BG);
        let bar = egui::Rect::from_min_size(rect.left_top() + vec2(0.0, 10.0), vec2(3.0, 20.0));
        p.rect_filled(bar, CornerRadius::same(2), ACCENT);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(9), Color32::from_rgb(0x17, 0x17, 0x1A));
    }
    let color = if selected { TEXT } else if resp.hovered() { TEXT } else { MUTED };
    let icon_color = if selected { ACCENT_HI } else { color };
    p.text(rect.left_center() + vec2(18.0, 0.0), egui::Align2::LEFT_CENTER, glyph, FontId::new(15.0, icons_family()), icon_color);
    let font = if selected { FontId::new(14.5, semibold()) } else { FontId::proportional(14.5) };
    p.text(rect.left_center() + vec2(46.0, 0.0), egui::Align2::LEFT_CENTER, label, font, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Viền mảnh cho ô nhập (dùng quanh TextEdit để đồng bộ phong cách).
pub fn input_frame<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, add)
        .inner
}

/// Mở một cửa sổ egui. Thử OpenGL trước; máy không có driver OpenGL dùng được (máy ảo, Remote Desktop,
/// card đồ hoạ cũ / "Microsoft Basic Display Adapter") thì chuyển sang DirectX 12 (wgpu, có WARP chạy bằng CPU).
/// Lỗi được ghi vào log.txt và báo bằng hộp thoại thay vì im lặng không mở.
pub fn run_window<A: eframe::App + 'static>(
    title: &str,
    options: impl Fn() -> eframe::NativeOptions,
    make: impl Fn(&eframe::CreationContext<'_>) -> A + Clone + 'static,
) {
    let mut last_err = String::new();
    let force_wgpu = std::env::var_os("CINPUT_FORCE_WGPU").is_some();
    for renderer in [eframe::Renderer::Glow, eframe::Renderer::Wgpu] {
        if force_wgpu && renderer == eframe::Renderer::Glow {
            continue;
        }
        let mut o = options();
        o.renderer = renderer;
        let m = make.clone();
        match eframe::run_native(title, o, Box::new(move |cc| Ok(Box::new(m(cc))))) {
            Ok(()) => return,
            Err(e) => {
                last_err = format!("{e}");
                crate::log::log(&format!("Không mở được cửa sổ bằng {renderer:?}: {last_err}"));
            }
        }
    }
    crate::win::message_box(
        "C_Input",
        &crate::i18n::fill(
            crate::i18n::t("Không mở được cửa sổ — máy chưa có driver đồ hoạ phù hợp.\n\n{}\n\nChi tiết trong %APPDATA%\\C_Input\\log.txt"),
            &[&last_err],
        ),
    );
}
