//! In-game chat: a translucent panel at the top-left with "Ник: текст" lines and an input
//! field with a send button; closed, the last messages show there and fade.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{ANIM_TIME, palette};
use egui::{Align2, Color32, Context, FontId, Frame, Id, Key, LayerId, Margin, Order, Pos2, Rect, Rounding, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatKind {
    Player,
    System,
    Error,
}

#[derive(Clone, Debug)]
pub struct ChatLine {
    pub text: String,
    pub kind: ChatKind,
    pub born: f64,
}

#[derive(Default)]
pub struct ChatState {
    pub lines: Vec<ChatLine>,
    pub open: bool,
    pub input: String,
    history: Vec<String>,
    hist_pos: Option<usize>,
    just_opened: bool,
    prefill: String,
}

pub const MAX_LINES: usize = 200;
/// Seconds a message stays visible while the chat is closed.
pub const VISIBLE_SECS: f64 = 9.0;

impl ChatState {
    pub fn push(&mut self, kind: ChatKind, text: impl Into<String>, now: f64) {
        for l in text.into().lines() {
            self.lines.push(ChatLine { text: l.to_string(), kind, born: now });
        }
        if self.lines.len() > MAX_LINES {
            let extra = self.lines.len() - MAX_LINES;
            self.lines.drain(..extra);
        }
    }

    pub fn open(&mut self, prefill: &str) {
        self.open = true;
        self.just_opened = true;
        self.prefill = prefill.to_string();
        self.input = prefill.to_string();
        self.hist_pos = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.input.clear();
        self.hist_pos = None;
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    fn remember(&mut self, line: &str) {
        if self.history.last().map(|l| l.as_str()) != Some(line) {
            self.history.push(line.to_string());
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
    }

    /// Moves through the sent-message history (-1 = older, +1 = newer).
    pub fn browse_history(&mut self, dir: i32) {
        if self.history.is_empty() {
            return;
        }
        let pos = match (self.hist_pos, dir) {
            (None, d) if d < 0 => Some(self.history.len() - 1),
            (None, _) => None,
            (Some(p), d) if d < 0 => Some(p.saturating_sub(1)),
            (Some(p), _) if p + 1 < self.history.len() => Some(p + 1),
            (Some(_), _) => None,
        };
        self.hist_pos = pos;
        self.input = pos.map(|p| self.history[p].clone()).unwrap_or_default();
    }

    /// How many lines are visible right now.
    pub fn visible_lines(&self, now: f64) -> usize {
        if self.open { self.lines.len().min(16) } else { self.lines.iter().rev().take(8).filter(|l| now - l.born < VISIBLE_SECS).count() }
    }
}

/// Colour of a nickname: picked from a small bright palette by the name's hash.
fn nick_color(name: &str) -> Color32 {
    const COLS: [Color32; 8] = [
        Color32::from_rgb(253, 41, 67),
        Color32::from_rgb(1, 162, 255),
        Color32::from_rgb(2, 184, 87),
        Color32::from_rgb(180, 128, 255),
        Color32::from_rgb(218, 133, 65),
        Color32::from_rgb(245, 205, 48),
        Color32::from_rgb(232, 186, 200),
        Color32::from_rgb(215, 197, 154),
    ];
    let h = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    COLS[h as usize % COLS.len()]
}

/// One chat line as rich text: "Ник: текст" with a coloured nick, system lines in grey-green,
/// errors in red.
fn line_job(l: &ChatLine, size: f32, alpha: f32, wrap: f32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let fmt = |c: Color32| TextFormat { font_id: FontId::proportional(size), color: c.gamma_multiply(alpha), ..Default::default() };
    match l.kind {
        ChatKind::Player => {
            // Stored as "<Ник> текст".
            if let Some((name, text)) = l.text.strip_prefix('<').and_then(|t| t.split_once("> ")) {
                job.append(&format!("{name}:"), 0.0, fmt(nick_color(name)));
                job.append(text, 6.0, fmt(Color32::from_rgb(240, 240, 240)));
            } else {
                job.append(&l.text, 0.0, fmt(Color32::from_rgb(240, 240, 240)));
            }
        }
        ChatKind::System => job.append(&l.text, 0.0, fmt(Color32::from_rgb(200, 220, 205))),
        ChatKind::Error => job.append(&l.text, 0.0, fmt(Color32::from_rgb(255, 120, 110))),
    }
    job
}

/// Panel rectangle: top-left, under the pause/chat island.
fn panel_rect(screen: Rect) -> Rect {
    let w = (screen.width() * 0.40).clamp(290.0, 470.0);
    let h = (screen.height() * 0.42).clamp(150.0, 300.0);
    Rect::from_min_size(Pos2::new(screen.left() + 14.0, screen.top() + 92.0), Vec2::new(w, h))
}

const PANEL: Color32 = Color32::from_rgba_premultiplied(22, 25, 30, 190);

/// Open chat: rounded translucent panel with the log and an input field with a send button.
fn open_chat(ctx: &Context, st: &mut ChatState, screen: Rect) -> Option<String> {
    let mut submitted = None;
    let mut close = false;
    let r = panel_rect(screen);
    egui::Area::new(Id::new("chat-open")).order(Order::Foreground).fixed_pos(r.min).show(ctx, |ui| {
        Frame::none().fill(PANEL).rounding(Rounding::same(10.0)).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            ui.set_width(r.width() - 20.0);
            // Log, newest at the bottom; a small close button in the corner.
            let top = ui.cursor().min;
            let log_h = r.height() - 20.0 - 52.0;
            egui::ScrollArea::vertical().max_height(log_h).min_scrolled_height(log_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                if st.lines.is_empty() {
                    ui.label(egui::RichText::new("Напишите сообщение или /help — список команд").size(14.0).color(Color32::from_gray(170)));
                }
                for l in st.lines.iter().rev().take(100).collect::<Vec<_>>().into_iter().rev() {
                    ui.label(line_job(l, 15.0, 1.0, r.width() - 40.0));
                }
            });
            let x = Rect::from_min_size(Pos2::new(top.x + r.width() - 20.0 - 26.0, top.y - 4.0), Vec2::splat(26.0));
            let xr = ui.interact(x, Id::new("chat-close"), egui::Sense::click());
            let c = x.center();
            let col = if xr.hovered() { Color32::WHITE } else { Color32::from_gray(170) };
            for (a, b) in [(Vec2::new(-5.0, -5.0), Vec2::new(5.0, 5.0)), (Vec2::new(-5.0, 5.0), Vec2::new(5.0, -5.0))] {
                ui.painter().line_segment([c + a, c + b], Stroke::new(2.0_f32, col));
            }
            if xr.clicked() {
                close = true;
            }
            ui.add_space(8.0);
            // Input field with a send arrow.
            let (row, _) = ui.allocate_exact_size(Vec2::new(r.width() - 20.0, 40.0), egui::Sense::hover());
            ui.painter().rect_filled(row, Rounding::same(8.0), Color32::from_rgba_premultiplied(8, 10, 12, 200));
            ui.painter().rect_stroke(row, Rounding::same(8.0), Stroke::new(1.0_f32, Color32::from_white_alpha(40)));
            let send = Rect::from_min_size(Pos2::new(row.right() - 40.0, row.top()), Vec2::new(40.0, 40.0));
            let field = Rect::from_min_max(row.min + Vec2::new(12.0, 0.0), Pos2::new(send.left() - 4.0, row.bottom()));
            let id = Id::new("chat-input-edit");
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let resp = child.add(
                egui::TextEdit::singleline(&mut st.input)
                    .id(id)
                    .frame(false)
                    .desired_width(field.width())
                    .font(FontId::proportional(15.0))
                    .text_color(Color32::WHITE)
                    .hint_text(egui::RichText::new("Нажмите сюда, чтобы написать").color(Color32::from_gray(140)))
                    .cursor_at_end(true),
            );
            if st.just_opened {
                // Drop the character of the key that opened the chat (T) but keep the "/" prefill.
                st.input = st.prefill.clone();
                st.just_opened = false;
                resp.request_focus();
                if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
                    let end = egui::text::CCursor::new(st.input.chars().count());
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
                    state.store(ctx, id);
                }
            }
            if resp.has_focus() {
                if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                    st.browse_history(-1);
                }
                if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                    st.browse_history(1);
                }
                if ctx.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
            }
            let sr = ui.interact(send, Id::new("chat-send"), egui::Sense::click());
            let c = send.center();
            let col = if st.input.trim().is_empty() { Color32::from_gray(120) } else { Color32::WHITE };
            ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(-8.0, -8.0), c + Vec2::new(9.0, 0.0), c + Vec2::new(-8.0, 8.0), c + Vec2::new(-4.0, 0.0)], col, Stroke::NONE));
            let enter = resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter));
            if enter || sr.clicked() {
                let line = st.input.trim().to_string();
                if !line.is_empty() {
                    st.remember(&line);
                    submitted = Some(line);
                }
                close = true;
            }
        });
    });
    if close {
        st.close();
    }
    submitted
}

/// Draws the chat. Returns a submitted line (message or `/command`) when it was sent.
pub fn draw(ctx: &Context, _icons: &GuiIcons, st: &mut ChatState, now: f64) -> Option<String> {
    let _ = (palette(ctx), ANIM_TIME, Align2::LEFT_TOP);
    let screen = ctx.screen_rect();
    if st.open {
        return open_chat(ctx, st, screen);
    }
    // Closed: the last few messages in the same place, fading out.
    let r = panel_rect(screen);
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("chat-log")));
    let mut blocks = Vec::new();
    for l in st.lines.iter().rev().take(7) {
        let age = now - l.born;
        if age >= VISIBLE_SECS {
            continue;
        }
        let alpha = ((VISIBLE_SECS - age) / 1.5).clamp(0.0, 1.0) as f32;
        blocks.push((painter.layout_job(line_job(l, 15.0, alpha, r.width() - 24.0)), alpha));
    }
    if blocks.is_empty() {
        return None;
    }
    blocks.reverse();
    let total: f32 = blocks.iter().map(|(g, _)| g.size().y + 3.0).sum::<f32>() + 16.0;
    let max_alpha = blocks.iter().map(|(_, a)| *a).fold(0.0, f32::max);
    let bg = Rect::from_min_size(r.min, Vec2::new(r.width(), total));
    painter.rect_filled(bg, 10.0, Color32::from_rgba_premultiplied(22, 25, 30, (150.0 * max_alpha) as u8));
    let mut y = bg.top() + 8.0;
    for (g, _) in blocks {
        let h = g.size().y;
        painter.galley(Pos2::new(bg.left() + 12.0, y), g, Color32::WHITE);
        y += h + 3.0;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_fade_and_cap() {
        let mut c = ChatState::default();
        c.push(ChatKind::System, "a", 0.0);
        c.push(ChatKind::Player, "b\nc", 1.0);
        assert_eq!(c.lines.len(), 3, "multi-line messages split");
        assert_eq!(c.visible_lines(2.0), 3);
        assert_eq!(c.visible_lines(100.0), 0, "old messages hide when chat is closed");
        c.open("");
        assert_eq!(c.visible_lines(100.0), 3, "opening the chat shows the history");
        for i in 0..300 {
            c.push(ChatKind::System, format!("m{i}"), 0.0);
        }
        assert_eq!(c.lines.len(), MAX_LINES);
        assert_eq!(c.lines.last().unwrap().text, "m299");
    }

    #[test]
    fn history_navigation() {
        let mut c = ChatState::default();
        c.remember("/help");
        c.remember("привет");
        c.remember("привет");
        assert_eq!(c.history().len(), 2, "duplicates are collapsed");
        c.browse_history(-1);
        assert_eq!(c.input, "привет");
        c.browse_history(-1);
        assert_eq!(c.input, "/help");
        c.browse_history(-1);
        assert_eq!(c.input, "/help", "stays at the oldest");
        c.browse_history(1);
        assert_eq!(c.input, "привет");
        c.browse_history(1);
        assert_eq!(c.input, "", "past the newest = empty line");
    }

    #[test]
    fn draws_open_and_closed() {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, wolfcraft_engine::config::ThemeMode::Dark);
        let atlases = wolfcraft_assets::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        let mut c = ChatState::default();
        c.push(ChatKind::Error, "Неизвестная команда: /ой", 0.0);
        for open in [false, true] {
            if open {
                c.open("/");
            }
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0)));
            let _ = ctx.run(raw, |ctx| {
                draw(ctx, &icons, &mut c, 1.0);
            });
        }
        assert!(c.open);
        c.close();
        assert!(!c.open && c.input.is_empty());
    }
}
