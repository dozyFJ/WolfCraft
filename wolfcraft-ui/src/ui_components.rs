//! Reusable Modern Solid components: pill buttons, switches, segmented controls, dropdowns,
//! cards, progress bars, modals. All animate over `ANIM_TIME` and use icons from the GUI atlas.

use crate::gui_renderer::{GuiIcons, centered_square};
use crate::theme_manager::{ANIM_TIME, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, bold_font, palette};
use egui::{Align2, Color32, CursorIcon, FontId, Frame, Id, Margin, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2};
use wolfcraft_assets::gui as icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Danger,
    Ghost,
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

pub const BUTTON_HEIGHT: f32 = 38.0;

/// Pill-shaped button: icon on the left, label on the right.
pub fn pill_button(ui: &mut Ui, icons: &GuiIcons, icon_id: Option<u16>, text: &str, kind: ButtonKind, min_width: f32, enabled: bool) -> Response {
    let p = palette(ui.ctx());
    let font = FontId::proportional(15.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let icon_w = if icon_id.is_some() { 20.0 + 8.0 } else { 0.0 };
    let width = (galley.size().x + icon_w + 28.0).max(min_width);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, BUTTON_HEIGHT), if enabled { Sense::click() } else { Sense::hover() });
    let hover_t = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered() && enabled, ANIM_TIME);
    let pressed = resp.is_pointer_button_down_on() && enabled;
    let rect = if pressed { rect.shrink(1.0) } else { rect };

    let (fill, stroke, fg) = match kind {
        ButtonKind::Primary => (mix(p.accent, p.accent_hover, hover_t), Color32::TRANSPARENT, p.on_accent),
        ButtonKind::Secondary => (mix(p.surface_hi, p.surface_hover, hover_t), mix(p.outline, p.accent, hover_t), p.text),
        ButtonKind::Danger => (mix(p.danger.gamma_multiply(0.16), p.danger.gamma_multiply(0.30), hover_t), p.danger.gamma_multiply(0.7), p.danger),
        ButtonKind::Ghost => (mix(Color32::TRANSPARENT, p.surface_hover, hover_t), Color32::TRANSPARENT, p.text),
    };
    let (fill, fg) = if enabled { (fill, fg) } else { (p.surface_hi.gamma_multiply(0.6), p.text_muted.gamma_multiply(0.7)) };
    let painter = ui.painter();
    let r = Rounding::ZERO;
    painter.rect_filled(rect, r, fill);
    if stroke != Color32::TRANSPARENT {
        painter.rect_stroke(rect, r, Stroke::new(2.0_f32, stroke));
    }
    let content_w = galley.size().x + icon_w;
    let mut x = rect.center().x - content_w / 2.0;
    if let Some(ic) = icon_id {
        icons.paint(painter, Rect::from_min_size(Pos2::new(x, rect.center().y - 10.0), Vec2::splat(20.0)), ic, fg);
        x += 28.0;
    }
    painter.galley_with_override_text_color(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Round icon-only button.
pub fn icon_button(ui: &mut Ui, icons: &GuiIcons, icon_id: u16, size: f32, kind: ButtonKind) -> Response {
    let p = palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), ANIM_TIME);
    let (fill, fg) = match kind {
        ButtonKind::Danger => (mix(Color32::TRANSPARENT, p.danger, t), mix(p.text_muted, Color32::WHITE, t)),
        ButtonKind::Primary => (mix(p.accent, p.accent_hover, t), p.on_accent),
        _ => (mix(Color32::TRANSPARENT, p.surface_hover, t), mix(p.text_muted, p.text, t)),
    };
    ui.painter().rect_filled(rect, Rounding::ZERO, fill);
    icons.paint(ui.painter(), centered_square(rect, size * 0.58), icon_id, fg);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Pill-shaped on/off switch.
pub fn toggle_switch(ui: &mut Ui, on: &mut bool) -> Response {
    let p = palette(ui.ctx());
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(46.0, 26.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id.with("on"), *on, ANIM_TIME);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hv"), resp.hovered(), ANIM_TIME);
    let track = mix(mix(p.surface_hover, p.outline, 0.5), p.accent, t);
    ui.painter().rect_filled(rect, Rounding::ZERO, track);
    ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, mix(p.outline, p.accent, hover.max(t))));
    let knob_x = egui::lerp((rect.left() + 13.0)..=(rect.right() - 13.0), t);
    let knob = mix(p.text_muted, p.on_accent, t);
    ui.painter().rect_filled(Rect::from_center_size(Pos2::new(knob_x, rect.center().y), Vec2::splat(16.0 + hover * 2.0)), Rounding::ZERO, if p.dark { knob } else { Color32::WHITE });
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Segmented pill control. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, id: &str, options: &[(T, &str)], value: &mut T) -> bool {
    let p = palette(ui.ctx());
    let font = FontId::proportional(14.0);
    let widths: Vec<f32> = options.iter().map(|(_, l)| ui.painter().layout_no_wrap(l.to_string(), font.clone(), Color32::WHITE).size().x + 26.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 34.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::ZERO, p.surface_hi);
    ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
    let mut changed = false;
    let mut x = rect.left() + 4.0;
    // Animated highlight position.
    let sel_idx = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let mut sel_x = rect.left() + 4.0;
    for w in widths.iter().take(sel_idx) {
        sel_x += w;
    }
    let ax = ui.ctx().animate_value_with_time(Id::new(id).with("hx"), sel_x, ANIM_TIME);
    let aw = ui.ctx().animate_value_with_time(Id::new(id).with("hw"), widths[sel_idx], ANIM_TIME);
    ui.painter().rect_filled(Rect::from_min_size(Pos2::new(ax, rect.top() + 4.0), Vec2::new(aw, 26.0)), Rounding::ZERO, p.accent);
    for (i, (v, label)) in options.iter().enumerate() {
        let seg = Rect::from_min_size(Pos2::new(x, rect.top() + 4.0), Vec2::new(widths[i], 26.0));
        let resp = ui.interact(seg, Id::new(id).with(i), Sense::click());
        if resp.clicked() && *value != *v {
            *value = *v;
            changed = true;
        }
        let selected = *value == *v;
        let col = if selected { p.on_accent } else if resp.hovered() { p.text } else { p.text_muted };
        ui.painter().text(seg.center(), Align2::CENTER_CENTER, label, font.clone(), col);
        resp.on_hover_cursor(CursorIcon::PointingHand);
        x += widths[i];
    }
    changed
}

/// Dropdown list with a down-arrow. Returns true when the selection changed.
pub fn dropdown(ui: &mut Ui, icons: &GuiIcons, id: &str, options: &[String], selected: &mut usize, width: f32) -> bool {
    let p = palette(ui.ctx());
    let popup_id = Id::new(id).with("popup");
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::click());
    let open = ui.memory(|m| m.is_popup_open(popup_id));
    let t = ui.ctx().animate_bool_with_time(resp.id.with("hv"), resp.hovered() || open, ANIM_TIME);
    let r = Rounding::ZERO;
    ui.painter().rect_filled(rect, r, mix(p.surface_hi, p.surface_hover, t));
    ui.painter().rect_stroke(rect, r, Stroke::new(2.0_f32, mix(p.outline, p.accent, t)));
    let label = options.get(*selected).map(String::as_str).unwrap_or("—");
    ui.painter().text(Pos2::new(rect.left() + 14.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(15.0), p.text);
    // Arrow rotates when open.
    let rot = ui.ctx().animate_bool_with_time(resp.id.with("rot"), open, ANIM_TIME);
    let arrow_rect = Rect::from_center_size(Pos2::new(rect.right() - 20.0, rect.center().y), Vec2::splat(20.0));
    icons.paint(ui.painter(), arrow_rect.translate(Vec2::new(0.0, -rot * 2.0)), icon::CHEVRON_DOWN, mix(p.text_muted, p.accent, t));
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    let mut changed = false;
    egui::popup::popup_below_widget(ui, popup_id, &resp, egui::popup::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(width - 16.0);
        for (i, o) in options.iter().enumerate() {
            let sel = i == *selected;
            let (r, rr) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(width - 16.0), 32.0), Sense::click());
            let hv = ui.ctx().animate_bool_with_time(rr.id.with("hv"), rr.hovered(), ANIM_TIME);
            ui.painter().rect_filled(r, Rounding::ZERO, mix(if sel { p.accent.gamma_multiply(0.18) } else { Color32::TRANSPARENT }, p.surface_hover, hv));
            ui.painter().text(Pos2::new(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, o, FontId::proportional(15.0), if sel { p.accent } else { p.text });
            if sel {
                icons.paint(ui.painter(), Rect::from_center_size(Pos2::new(r.right() - 16.0, r.center().y), Vec2::splat(16.0)), icon::CHECK, p.accent);
            }
            if rr.clicked() && !sel {
                *selected = i;
                changed = true;
            }
            rr.on_hover_cursor(CursorIcon::PointingHand);
        }
    });
    resp.on_hover_cursor(CursorIcon::PointingHand);
    changed
}

/// Single-line text field in a rounded frame with an optional leading icon.
pub fn text_input(ui: &mut Ui, icons: &GuiIcons, text: &mut String, hint: &str, width: f32, icon_id: Option<u16>) -> Response {
    let p = palette(ui.ctx());
    // A stable id (from the hint): an automatic one changes when the layout around the field
    // changes, e.g. when the phone keyboard opens and shrinks the screen, and the field then
    // loses focus after every letter.
    let id = ui.make_persistent_id(("text_input", hint));
    let focused = ui.memory(|m| m.has_focus(id.with("edit")));
    let t = ui.ctx().animate_bool_with_time(id.with("focus"), focused, ANIM_TIME);
    let frame = Frame::none().fill(p.surface_hi).rounding(Rounding::ZERO).stroke(Stroke::new(2.0_f32, mix(p.outline, p.accent, t))).inner_margin(Margin::symmetric(12.0, 7.0));
    let inner = frame.show(ui, |ui| {
        ui.set_width(width - 26.0);
        ui.horizontal(|ui| {
            if let Some(ic) = icon_id {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                icons.paint(ui.painter(), r, ic, mix(p.text_muted, p.accent, t));
            }
            ui.add(egui::TextEdit::singleline(text).id(id.with("edit")).hint_text(hint).frame(false).desired_width(f32::INFINITY).text_color(p.text))
        })
        .inner
    });
    inner.inner
}

/// Rounded surface container.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = palette(ui.ctx());
    Frame::none().fill(p.surface).rounding(Rounding::ZERO).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::same(14.0)).show(ui, add).inner
}

/// Hoverable card (used for list rows). Returns the response of the whole row.
pub fn row_card(ui: &mut Ui, selected: bool, add: impl FnOnce(&mut Ui)) -> Response {
    let p = palette(ui.ctx());
    let id = ui.next_auto_id();
    let hover = ui.ctx().animate_bool_with_time(id.with("hv"), ui.rect_contains_pointer(ui.available_rect_before_wrap()), ANIM_TIME);
    let fill = mix(p.surface, p.surface_hi, hover);
    let stroke = if selected { p.accent } else { mix(p.outline, p.accent.gamma_multiply(0.6), hover) };
    let out = Frame::none().fill(fill).rounding(Rounding::ZERO).stroke(Stroke::new(if selected { 1.5_f32 } else { 1.0 }, stroke)).inner_margin(Margin::same(12.0)).show(ui, add);
    ui.interact(out.response.rect, id, Sense::click())
}

pub fn section_label(ui: &mut Ui, text: &str) {
    let p = palette(ui.ctx());
    ui.label(egui::RichText::new(text.to_uppercase()).font(FontId::proportional(11.5)).color(p.text_muted).extra_letter_spacing(1.0));
}

/// Label on the left, control on the right.
pub fn setting_row(ui: &mut Ui, label: &str, hint: Option<&str>, add: impl FnOnce(&mut Ui)) {
    let p = palette(ui.ctx());
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(label).font(FontId::proportional(15.0)).color(p.text));
            if let Some(h) = hint {
                ui.label(egui::RichText::new(h).font(FontId::proportional(12.5)).color(p.text_muted));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
}

/// Label (and hint) on top, control below — for narrow columns on phones.
pub fn stacked_row(ui: &mut Ui, label: &str, hint: Option<&str>, add: impl FnOnce(&mut Ui)) {
    let p = palette(ui.ctx());
    ui.label(egui::RichText::new(label).font(FontId::proportional(15.0)).color(p.text));
    if let Some(h) = hint {
        ui.label(egui::RichText::new(h).font(FontId::proportional(12.5)).color(p.text_muted));
    }
    ui.add_space(4.0);
    ui.horizontal(add);
}

/// Horizontal progress pill with a centred label.
pub fn progress_bar(ui: &mut Ui, fraction: f32, label: &str, width: f32) {
    let p = palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());
    let f = ui.ctx().animate_value_with_time(resp.id.with("p"), fraction.clamp(0.0, 1.0), ANIM_TIME);
    ui.painter().rect_filled(rect, Rounding::ZERO, p.surface_hi);
    ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
    if f > 0.0 {
        let w = (rect.width() * f).max(24.0);
        ui.painter().rect_filled(Rect::from_min_size(rect.min, Vec2::new(w, rect.height())), Rounding::ZERO, p.accent);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(13.0), if f > 0.55 { p.on_accent } else { p.text });
}

/// Underline tab bar. Returns true when the tab changed.
pub fn tab_bar(ui: &mut Ui, id: &str, tabs: &[&str], selected: &mut usize) -> bool {
    let p = palette(ui.ctx());
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, t) in tabs.iter().enumerate() {
            let font = FontId::proportional(14.5);
            let galley = ui.painter().layout_no_wrap(t.to_string(), font, Color32::WHITE);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 24.0, 34.0), Sense::click());
            let sel = *selected == i;
            let a = ui.ctx().animate_bool_with_time(Id::new(id).with(i), sel, ANIM_TIME);
            let hv = ui.ctx().animate_bool_with_time(Id::new(id).with(("h", i)), resp.hovered(), ANIM_TIME);
            ui.painter().rect_filled(rect, Rounding::ZERO, mix(Color32::TRANSPARENT, p.surface_hover, hv * (1.0 - a)));
            let col = mix(p.text_muted, p.accent, a).lerp_to_gamma(p.text, hv * (1.0 - a));
            ui.painter().galley_with_override_text_color(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, col);
            let w = (rect.width() - 16.0) * a;
            ui.painter().rect_filled(Rect::from_center_size(Pos2::new(rect.center().x, rect.bottom() - 2.0), Vec2::new(w, 3.0)), Rounding::ZERO, p.accent);
            if resp.clicked() && !sel {
                *selected = i;
                changed = true;
            }
            resp.on_hover_cursor(CursorIcon::PointingHand);
        }
    });
    changed
}

/// Small coloured label.
pub fn badge(ui: &mut Ui, text: &str, color: Color32) {
    let font = FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x + 16.0, 22.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::ZERO, color.gamma_multiply(0.16));
    ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, color.gamma_multiply(0.5)));
    ui.painter().galley_with_override_text_color(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, color);
}

/// Centered modal dialog over a dimmed screen. Returns true if the user clicked outside / Esc.
pub fn modal(ctx: &egui::Context, id: &str, title: &str, width: f32, add: impl FnOnce(&mut Ui)) -> bool {
    modal_opts(ctx, id, title, width, true, add)
}

/// In-game window (inventory, chest, furnace, workbench): no dimming behind it, the world stays
/// fully visible; a tap outside still closes it.
pub fn game_window(ctx: &egui::Context, id: &str, width: f32, add: impl FnOnce(&mut Ui)) -> bool {
    modal_opts(ctx, id, "", width, false, add)
}

/// Height of the header bar of [`game_screen`].
pub const SCREEN_BAR: f32 = 52.0;

/// Bedrock-style game screen, the same look as the chat: the world dims, a dark header bar
/// across the top carries a back arrow and the title, and the content sits in a panel centred
/// below it. Returns true when it should close (back arrow, a tap outside, Escape).
/// A window over the whole screen with no title bar (the content has its own back button).
/// Returns true when Escape was pressed.
pub fn full_screen(ctx: &egui::Context, id: &str, add: impl FnOnce(&mut Ui)) -> bool {
    let p: Palette = palette(ctx);
    let screen = ctx.screen_rect();
    let ins = crate::gui_renderer::SAFE_INSETS.read().map(|i| *i).unwrap_or([0.0; 4]);
    let rect = Rect::from_min_max(screen.min + Vec2::new(ins[0] + 6.0, ins[1] + 6.0), screen.max - Vec2::new(ins[2] + 6.0, ins[3] + 6.0));
    egui::Area::new(Id::new(id).with("full")).order(egui::Order::Foreground).fixed_pos(screen.min).fade_in(false).show(ctx, |ui| {
        let (bg, _) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(bg, 0.0, Color32::from_black_alpha(140));
    });
    egui::Area::new(Id::new(id).with("full-body")).order(egui::Order::Tooltip).fixed_pos(rect.min).fade_in(false).show(ctx, |ui| {
        Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, Color32::from_rgb(20, 20, 22))).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            ui.set_width(rect.width() - 20.0);
            ui.set_height(rect.height() - 20.0);
            add(ui);
        });
    });
    ctx.input(|i| i.key_pressed(egui::Key::Escape))
}

pub fn game_screen(ctx: &egui::Context, id: &str, title: &str, width: f32, add: impl FnOnce(&mut Ui)) -> bool {
    let p: Palette = palette(ctx);
    let screen = ctx.screen_rect();
    let mut dismissed = false;
    egui::Area::new(Id::new(id).with("screen")).order(egui::Order::Foreground).fixed_pos(screen.min).fade_in(false).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(110));
        let bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), SCREEN_BAR));
        painter.rect_filled(bar, 0.0, Color32::from_rgba_premultiplied(18, 18, 20, 235));
        painter.text(bar.center(), Align2::CENTER_CENTER, title, FontId::proportional(20.0), Color32::WHITE);
        // Back arrow: a flat grey square button.
        let back = Rect::from_min_size(bar.min + Vec2::new(10.0, 6.0), Vec2::splat(40.0));
        let br = ui.interact(back, Id::new(id).with("back"), Sense::click());
        let fill = if br.hovered() || br.is_pointer_button_down_on() { Color32::from_rgb(98, 98, 104) } else { Color32::from_rgb(72, 72, 76) };
        painter.rect_filled(back, 0.0, fill);
        painter.rect_filled(Rect::from_min_max(Pos2::new(back.left(), back.bottom() - 4.0), back.max), 0.0, Color32::from_rgb(40, 40, 44));
        painter.rect_stroke(back, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(20, 20, 22)));
        let c = back.center();
        for (a, b) in [(Vec2::new(5.0, -9.0), Vec2::new(-5.0, 0.0)), (Vec2::new(-5.0, 0.0), Vec2::new(5.0, 9.0))] {
            painter.line_segment([c + a, c + b], Stroke::new(3.0_f32, Color32::WHITE));
        }
        if br.clicked() || (resp.clicked() && !bar.contains(resp.interact_pointer_pos().unwrap_or(Pos2::ZERO))) {
            dismissed = true;
        }
    });
    let body_centre = Vec2::new(0.0, SCREEN_BAR * 0.5);
    let fit_area = Rect::from_min_max(screen.min + Vec2::new(8.0, SCREEN_BAR + 6.0), screen.max - Vec2::splat(8.0));
    let body = egui::Area::new(Id::new(id).with("body")).order(egui::Order::Tooltip).anchor(Align2::CENTER_CENTER, body_centre).fade_in(false).show(ctx, |ui| {
        Frame::none()
            .fill(p.surface)
            .rounding(Rounding::ZERO)
            .stroke(Stroke::new(2.0_f32, Color32::from_rgb(20, 20, 22)))
            .inner_margin(Margin::same(14.0))
            .show(ui, |ui| {
                ui.set_width(width);
                // On a short phone screen the content scrolls instead of running under the
                // header bar or off the bottom.
                // The window is scaled down to fit the screen (see `fit_body`); it scrolls
                // only when it would still not fit at the smallest scale.
                let max_h = (screen.height() - SCREEN_BAR - 2.0 * 14.0 - 16.0) / MIN_FIT;
                egui::ScrollArea::vertical().id_salt(Id::new(id).with("scroll")).max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                    add(ui);
                });
            });
    });
    fit_body(ctx, id, &body.response, fit_area);
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        dismissed = true;
    }
    dismissed
}

/// Smallest scale a window is shrunk to before it starts to scroll instead.
const MIN_FIT: f32 = 0.55;

/// Auto-fit of a window to the screen, by width and by height: when the body `resp` is
/// larger than `avail`, its layer is scaled down about the body's centre (never up, never
/// below `MIN_FIT`). The scale is remembered under `id` for `to_body`. Returns the
/// transform in use this frame.
fn fit_body(ctx: &egui::Context, id: &str, resp: &egui::Response, avail: Rect) -> egui::emath::TSTransform {
    let r = resp.rect;
    let s = (avail.width() / r.width().max(1.0)).min(avail.height() / r.height().max(1.0)).clamp(MIN_FIT, 1.0);
    // Keep the centre in place, then pull the scaled window inside the free area.
    let c = r.center();
    let mut t = c.to_vec2() * (1.0 - s);
    let top = c.y - r.height() * s / 2.0;
    if top < avail.top() {
        t.y += avail.top() - top;
    }
    let tf = egui::emath::TSTransform::new(t, s);
    ctx.set_transform_layer(resp.layer_id, tf);
    ctx.data_mut(|d| d.insert_temp(Id::new(id).with("fit"), (s, t)));
    tf
}

/// A screen position (the finger) in the coordinates of the auto-fitted window `id`: use it
/// to test slot rectangles recorded inside the window.
pub fn to_body(ctx: &egui::Context, id: &str, pos: Pos2) -> Pos2 {
    match ctx.data(|d| d.get_temp::<(f32, Vec2)>(Id::new(id).with("fit"))) {
        Some((s, t)) if s > 0.0 => ((pos.to_vec2() - t) / s).to_pos2(),
        _ => pos,
    }
}

/// A block's own window (chest, furnace, forge…): no top bar, just a slightly rounded card
/// in the middle of the dimmed world. A tap outside the card or the round ✕ on its corner
/// closes it. Returns true when dismissed.
pub fn block_screen(ctx: &egui::Context, id: &str, width: f32, add: impl FnOnce(&mut Ui)) -> bool {
    let p: Palette = palette(ctx);
    let screen = ctx.screen_rect();
    let mut dismissed = false;
    let scrim = egui::Area::new(Id::new(id).with("screen")).order(egui::Order::Foreground).fixed_pos(screen.min).fade_in(false).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(110));
        resp.clicked()
    });
    let body = egui::Area::new(Id::new(id).with("body")).order(egui::Order::Tooltip).anchor(Align2::CENTER_CENTER, Vec2::ZERO).fade_in(false).show(ctx, |ui| {
        Frame::none()
            .fill(p.surface)
            .rounding(Rounding::same(10.0))
            .stroke(Stroke::new(2.0_f32, Color32::from_rgb(20, 20, 22)))
            .inner_margin(Margin::same(14.0))
            .show(ui, |ui| {
                ui.set_width(width);
                let max_h = (screen.height() - 2.0 * 14.0 - 20.0) / MIN_FIT;
                egui::ScrollArea::vertical().id_salt(Id::new(id).with("scroll")).max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                    add(ui);
                });
            });
    });
    // Sized to the screen: scaled down when the window is taller or wider than it.
    let fit = fit_body(ctx, id, &body.response, screen.shrink(10.0));
    let card = Rect::from_min_max(fit * body.response.rect.min, fit * body.response.rect.max);
    // Round close button riding on the card's top-right corner.
    let c = Pos2::new((card.right() + 4.0).min(screen.right() - 20.0), (card.top() - 4.0).max(screen.top() + 20.0));
    egui::Area::new(Id::new(id).with("close")).order(egui::Order::Tooltip).fixed_pos(c - Vec2::splat(17.0)).fade_in(false).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
        let fill = if resp.hovered() || resp.is_pointer_button_down_on() { Color32::from_rgb(200, 70, 60) } else { Color32::from_rgb(72, 72, 76) };
        ui.painter().circle_filled(r.center(), 16.0, fill);
        ui.painter().circle_stroke(r.center(), 16.0, Stroke::new(2.0_f32, Color32::from_rgb(20, 20, 22)));
        for (a, b) in [(Vec2::new(-5.5, -5.5), Vec2::new(5.5, 5.5)), (Vec2::new(-5.5, 5.5), Vec2::new(5.5, -5.5))] {
            ui.painter().line_segment([r.center() + a, r.center() + b], Stroke::new(2.5_f32, Color32::WHITE));
        }
        if resp.clicked() {
            dismissed = true;
        }
    });
    // A tap on the dimmed world (not on the card) closes too.
    if scrim.inner && !ctx.input(|i| i.pointer.interact_pos()).is_some_and(|q| card.expand(8.0).contains(q)) {
        dismissed = true;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        dismissed = true;
    }
    dismissed
}

fn modal_opts(ctx: &egui::Context, id: &str, title: &str, width: f32, scrim: bool, add: impl FnOnce(&mut Ui)) -> bool {
    let p: Palette = palette(ctx);
    let screen = ctx.screen_rect();
    let mut dismissed = false;
    egui::Area::new(Id::new(id).with("scrim")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        let a = ctx.animate_bool_with_time(Id::new(id).with("fade"), true, ANIM_TIME);
        if scrim {
            ui.painter().rect_filled(rect, 0.0, p.scrim.gamma_multiply(a));
        }
        if resp.clicked() {
            dismissed = true;
        }
    });
    egui::Area::new(Id::new(id).with("body")).order(egui::Order::Tooltip).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
        Frame::none()
            .fill(p.surface)
            .rounding(Rounding::ZERO)
            .stroke(Stroke::new(2.0_f32, p.outline))
            .shadow(egui::Shadow { offset: Vec2::new(6.0, 6.0), blur: 0.0, spread: 0.0, color: Color32::from_black_alpha(120) })
            .inner_margin(Margin::same(18.0))
            .show(ui, |ui| {
                ui.set_width(width);
                // An empty title lets the caller draw its own header bar.
                if !title.is_empty() {
                    ui.label(egui::RichText::new(title).font(bold_font(20.0)).color(p.text));
                    ui.add_space(8.0);
                }
                add(ui);
            });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        dismissed = true;
    }
    dismissed
}

/// Thin separator.
pub fn divider(ui: &mut Ui) {
    let p = palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, p.outline);
}
