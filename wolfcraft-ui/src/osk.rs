//! The game's own on-screen keyboard for phones.
//!
//! The game window is a NativeActivity: the system keyboard (Gboard etc.) cannot type into
//! it, and a system text dialog on top of the game is clumsy. So whenever a text field has
//! focus, this keyboard slides up at the bottom of the screen: Russian and English letters,
//! digits and symbols, shift, backspace, space and enter. Its keys become ordinary egui
//! input events for the focused field on the next frame.

use egui::{Align2, Color32, Context, Event, FontId, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Stroke, Vec2};

/// Height of the keyboard on screen (points), for screens that must lift their own input
/// bar above it (the chat). 0 when hidden.
pub fn height(ctx: &Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(Id::new("osk-height"))).unwrap_or(0.0)
}

#[derive(Default)]
pub struct OskState {
    /// The text field the keyboard types into.
    target: Option<Id>,
    /// Where the keyboard was drawn last frame (a press there must not unfocus the field).
    rect: Option<Rect>,
    layout: Layout,
    shift: bool,
    /// Caps lock (shift tapped twice quickly).
    caps: bool,
    /// When shift was last tapped (for the double tap).
    shift_at: f64,
    /// How long backspace has been held, and the repeat clock.
    back_held: f32,
    back_rep: f32,
    /// Key events for the next frame.
    pub pending: Vec<Event>,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Layout {
    #[default]
    Ru,
    En,
    Sym,
}

const RU: [&str; 3] = ["йцукенгшщзх", "фывапролджэ", "ячсмитьбюъ"];
const EN: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
const SYM: [&str; 3] = ["1234567890", "/-:;()\"@&!?", ".,_+=*#%'<>"];

enum K {
    Ch(char),
    Shift,
    Back,
    Mode,
    Lang,
    Space,
    Enter,
    Left,
    Right,
    Hide,
}

pub fn draw(ctx: &Context, st: &mut OskState) {
    let screen = ctx.screen_rect();
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    let touching = ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_down() || i.pointer.any_released());
    let on_keys = touching && pointer.zip(st.rect).is_some_and(|(p, r)| r.contains(p));
    // A text field has focus this frame (it asked for the IME), or the finger is on our keys
    // (egui unfocuses the field on a press elsewhere: give the focus back).
    let field = ctx.output(|o| o.ime.is_some());
    if field {
        st.target = ctx.memory(|m| m.focused());
    } else if on_keys && st.target.is_some() {
        if let Some(id) = st.target {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    } else {
        st.target = None;
        st.rect = None;
        ctx.data_mut(|d| d.insert_temp(Id::new("osk-height"), 0.0f32));
        return;
    }
    // Lying phone: a lower keyboard, so the field above stays visible.
    let landscape = screen.width() > screen.height();
    let h = if landscape { (screen.height() * 0.50).clamp(150.0, 230.0) } else { (screen.height() * 0.40).clamp(190.0, 300.0) };
    let rect = Rect::from_min_max(Pos2::new(screen.left(), screen.bottom() - h), screen.max);
    st.rect = Some(rect);
    ctx.data_mut(|d| d.insert_temp(Id::new("osk-height"), h));

    let rows: [&str; 3] = match st.layout {
        Layout::Ru => RU,
        Layout::En => EN,
        Layout::Sym => SYM,
    };
    let mut pressed: Vec<K> = Vec::new();
    let mut back_down = false;
    let press_now = ctx.input(|i| i.pointer.any_pressed());
    let shift_on = st.shift || st.caps;
    let caps = st.caps;
    egui::Area::new(Id::new("osk")).order(Order::Tooltip).fixed_pos(rect.min).interactable(true).show(ctx, |ui| {
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, Color32::from_rgb(24, 26, 32));
        painter.line_segment([rect.left_top(), rect.right_top()], Stroke::new(2.0_f32, Color32::from_rgb(60, 64, 76)));
        // The whole panel is one layer on top, so nothing behind it reacts to taps.
        // (Allocated, so the area has the keyboard's size and gets the hits.)
        ui.allocate_rect(rect, Sense::hover());
        let pad = 6.0;
        let gap = 5.0;
        let row_h = (h - pad * 2.0 - gap * 3.0) / 4.0;
        let inner_w = rect.width() - pad * 2.0;
        // Unit width: the longest letter row fills the width.
        let longest = rows.iter().map(|r| r.chars().count()).max().unwrap_or(10).max(10) as f32;
        let unit = (inner_w - gap * (longest - 1.0)) / longest;
        let font = (row_h * 0.42).clamp(14.0, 22.0);
        let mut key = |ui: &mut egui::Ui, r: Rect, label: &str, k: K, accent: bool, id: String| {
            // A key reacts the moment the finger lands (not on release): fast typing does not
            // lose letters, and a finger sliding off the key does not cancel it.
            let resp = ui.interact(r.expand(gap / 2.0), Id::new(id), Sense::click());
            let down = resp.is_pointer_button_down_on();
            let fill = if down {
                Color32::from_rgb(38, 198, 218)
            } else if accent {
                Color32::from_rgb(58, 62, 74)
            } else {
                Color32::from_rgb(44, 47, 56)
            };
            ui.painter().rect_filled(r, 7.0, fill);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(font), Color32::WHITE);
            if matches!(k, K::Back) && down {
                back_down = true;
            }
            if down && matches!(k, K::Ch(_)) {
                // The pressed letter pops up above the finger, big.
                let bubble = Rect::from_center_size(r.center_top() - Vec2::new(0.0, r.height() * 0.7), Vec2::new(r.width().max(34.0) * 1.3, r.height() * 1.25));
                let layer = ui.ctx().layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("osk-bubble")));
                layer.rect_filled(bubble, 9.0, Color32::from_rgb(60, 66, 82));
                layer.rect_stroke(bubble, 9.0, Stroke::new(1.5_f32, Color32::from_rgb(38, 198, 218)));
                layer.text(bubble.center(), Align2::CENTER_CENTER, label, FontId::proportional(font * 1.5), Color32::WHITE);
            }
            if down && press_now {
                pressed.push(k);
            }
        };
        for (ri, row) in rows.iter().enumerate() {
            let y = rect.top() + pad + ri as f32 * (row_h + gap);
            let n = row.chars().count() as f32;
            let mut x = rect.left() + pad;
            // The last letter row: shift on the left, backspace on the right.
            let side = if ri == 2 { unit * 1.4 } else { 0.0 };
            let row_w = n * unit + (n - 1.0) * gap;
            if ri == 2 {
                key(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(side, row_h)), if caps { "⇪" } else if shift_on { "⇧" } else { "⇧" }, K::Shift, shift_on, "osk-shift".into());
                let back = Rect::from_min_size(Pos2::new(rect.right() - pad - side, y), Vec2::new(side, row_h));
                key(ui, back, "⌫", K::Back, true, "osk-back".into());
                x = rect.left() + pad + side + gap + ((inner_w - 2.0 * (side + gap)) - row_w) / 2.0;
            } else {
                x += (inner_w - row_w) / 2.0;
            }
            for (ci, ch) in row.chars().enumerate() {
                let c = if shift_on { ch.to_uppercase().next().unwrap_or(ch) } else { ch };
                let r = Rect::from_min_size(Pos2::new(x + ci as f32 * (unit + gap), y), Vec2::new(unit, row_h));
                key(ui, r, &c.to_string(), K::Ch(c), false, format!("osk-{ri}-{ci}"));
            }
        }
        // Bottom row: symbols, language, space, enter.
        let y = rect.top() + pad + 3.0 * (row_h + gap);
        let mut x = rect.left() + pad;
        let w_mode = unit * 1.3;
        let w_small = unit;
        key(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w_small, row_h)), "⌄", K::Hide, true, "osk-hide".into());
        x += w_small + gap;
        key(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w_mode, row_h)), if st.layout == Layout::Sym { "АБВ" } else { "?123" }, K::Mode, true, "osk-mode".into());
        x += w_mode + gap;
        key(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w_mode, row_h)), if st.layout == Layout::En { "RU" } else { "EN" }, K::Lang, true, "osk-lang".into());
        x += w_mode + gap;
        let w_enter = unit * 1.8;
        let w_space = rect.right() - pad - x - w_enter - 3.0 * gap - 2.0 * w_small;
        key(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w_space, row_h)), "пробел", K::Space, false, "osk-space".into());
        x += w_space + gap;
        key(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w_small, row_h)), "‹", K::Left, true, "osk-left".into());
        x += w_small + gap;
        key(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w_small, row_h)), "›", K::Right, true, "osk-right".into());
        x += w_small + gap;
        key(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w_enter, row_h)), "Ввод", K::Enter, true, "osk-enter".into());
    });

    let key_ev = |key: Key, pressed: bool| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    for k in pressed {
        match k {
            K::Ch(c) => {
                st.pending.push(Event::Text(c.to_string()));
                if st.shift && !st.caps {
                    st.shift = false;
                }
            }
            K::Shift => {
                // One tap: the next letter big. Two quick taps: caps lock. A tap in caps: off.
                let now = ctx.input(|i| i.time);
                if st.caps {
                    st.caps = false;
                    st.shift = false;
                } else if st.shift && now - st.shift_at < 0.4 {
                    st.caps = true;
                } else {
                    st.shift = !st.shift;
                }
                st.shift_at = now;
            }
            K::Left => {
                st.pending.push(key_ev(Key::ArrowLeft, true));
                st.pending.push(key_ev(Key::ArrowLeft, false));
            }
            K::Right => {
                st.pending.push(key_ev(Key::ArrowRight, true));
                st.pending.push(key_ev(Key::ArrowRight, false));
            }
            K::Hide => {
                // Down with the keyboard: the field lets go of the focus.
                if let Some(id) = st.target.take() {
                    ctx.memory_mut(|m| m.surrender_focus(id));
                }
                st.rect = None;
                ctx.data_mut(|d| d.insert_temp(Id::new("osk-height"), 0.0f32));
                return;
            }
            K::Back => {
                st.pending.push(key_ev(Key::Backspace, true));
                st.pending.push(key_ev(Key::Backspace, false));
            }
            K::Mode => st.layout = if st.layout == Layout::Sym { Layout::Ru } else { Layout::Sym },
            K::Lang => st.layout = if st.layout == Layout::En { Layout::Ru } else { Layout::En },
            K::Space => st.pending.push(Event::Text(" ".into())),
            K::Enter => {
                st.pending.push(key_ev(Key::Enter, true));
                st.pending.push(key_ev(Key::Enter, false));
            }
        }
    }
    // Holding backspace: after a moment it keeps erasing, faster and faster.
    if back_down {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        st.back_held += dt;
        if st.back_held > 0.45 {
            st.back_rep += dt;
            let every = if st.back_held > 1.5 { 0.03 } else { 0.07 };
            while st.back_rep >= every {
                st.back_rep -= every;
                st.pending.push(key_ev(Key::Backspace, true));
                st.pending.push(key_ev(Key::Backspace, false));
            }
        }
        ctx.request_repaint();
    } else {
        st.back_held = 0.0;
        st.back_rep = 0.0;
    }
    // Keep the field focused while typing on our keys.
    if on_keys {
        if let Some(id) = st.target {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &Context, st: &mut OskState, text: &mut String, events: Vec<Event>, focus: bool) {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 400.0))), events, ..Default::default() };
        let _ = ctx.run(input, |c| {
            egui::CentralPanel::default().show(c, |ui| {
                let r = ui.add(egui::TextEdit::singleline(text).id(Id::new("field")));
                if focus {
                    r.request_focus();
                }
            });
            draw(c, st);
        });
    }

    fn tap(p: Pos2) -> Vec<Vec<Event>> {
        let b = |pressed| Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        vec![vec![Event::PointerMoved(p)], vec![b(true)], vec![b(false)]]
    }

    #[test]
    fn keys_type_into_the_focused_field_and_it_stays_focused() {
        let ctx = Context::default();
        let mut st = OskState::default();
        let mut text = String::new();
        frame(&ctx, &mut st, &mut text, vec![], true);
        frame(&ctx, &mut st, &mut text, vec![], false);
        let r = st.rect.expect("keyboard shown under a focused field");
        // The first key of the top row (й), then backspace-free check of a second letter.
        let first = Pos2::new(r.left() + 30.0, r.top() + 20.0);
        for ev in tap(first) {
            let pending = std::mem::take(&mut st.pending);
            let mut all = pending;
            all.extend(ev);
            frame(&ctx, &mut st, &mut text, all, false);
        }
        let pending = std::mem::take(&mut st.pending);
        frame(&ctx, &mut st, &mut text, pending, false);
        assert_eq!(text, "й");
        assert!(st.rect.is_some(), "still up after typing");
    }
}
