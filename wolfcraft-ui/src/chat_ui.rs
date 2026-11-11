//! In-game chat: fading message log, command line with history.

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

fn kind_color(k: ChatKind, _p: &crate::theme_manager::Palette) -> Color32 {
    match k {
        ChatKind::Player => Color32::from_rgb(230, 232, 228),
        ChatKind::System => Color32::from_rgb(110, 230, 140),
        ChatKind::Error => Color32::from_rgb(255, 110, 100),
    }
}

/// Quick commands shown as terminal keys: (label, command).
const QUICK: [(&str, &str); 10] = [
    ("day", "/time set день"),
    ("night", "/time set ночь"),
    ("creative", "/gamemode творческий"),
    ("survival", "/gamemode выживание"),
    ("heal", "/heal"),
    ("feed", "/feed"),
    ("fly", "/fly"),
    ("spawn", "/spawn"),
    ("pos", "/pos"),
    ("help", "/help"),
];

/// One terminal key (Termux "extra keys" style). Returns true when tapped.
fn term_key(ui: &mut egui::Ui, label: &str, w: f32, accent: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), egui::Sense::click());
    let down = resp.is_pointer_button_down_on();
    let fill = if down { Color32::from_rgb(60, 66, 62) } else { Color32::from_rgb(28, 31, 30) };
    ui.painter().rect_filled(rect, 4.0, fill);
    let col = if accent { Color32::from_rgb(110, 230, 140) } else { Color32::from_rgb(225, 228, 224) };
    match label {
        "UP" | "DOWN" => {
            let c = rect.center();
            let s = if label == "UP" { -1.0 } else { 1.0 };
            ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, 6.0 * s), c + Vec2::new(-7.0, -5.0 * s), c + Vec2::new(7.0, -5.0 * s)], col, Stroke::NONE));
        }
        _ => {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::monospace(13.0), col);
        }
    }
    resp.clicked()
}

/// Open chat as a terminal: log and prompt in a dark monospace window at the top-left,
/// a row of extra keys and a row of quick commands under it (clear of the phone keyboard).
fn terminal(ctx: &Context, st: &mut ChatState, screen: Rect) -> Option<String> {
    let mut submitted = None;
    let w = (screen.width() * 0.58).clamp(320.0, 680.0);
    let h = (screen.height() * 0.46).clamp(150.0, 360.0);
    let green = Color32::from_rgb(110, 230, 140);
    let mut close = false;
    egui::Area::new(Id::new("chat-terminal")).order(Order::Foreground).fixed_pos(Pos2::new(screen.left() + 12.0, screen.top() + 10.0)).show(ctx, |ui| {
        Frame::none().fill(Color32::from_rgba_unmultiplied(6, 9, 8, 238)).rounding(Rounding::same(8.0)).stroke(Stroke::new(1.5_f32, Color32::from_rgb(40, 46, 43))).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            ui.set_width(w);
            // Title bar with the three dots.
            ui.horizontal(|ui| {
                for c in [Color32::from_rgb(255, 95, 86), Color32::from_rgb(255, 189, 46), Color32::from_rgb(39, 201, 63)] {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 5.0, c);
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("wolfcraft — терминал").font(FontId::monospace(12.0)).color(Color32::from_gray(150)));
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                for l in st.lines.iter().rev().take(80).collect::<Vec<_>>().into_iter().rev() {
                    ui.label(egui::RichText::new(&l.text).font(FontId::monospace(13.0)).color(kind_color(l.kind, &crate::theme_manager::palette(ctx))));
                }
                if st.lines.is_empty() {
                    ui.label(egui::RichText::new("Введите /help — список команд").font(FontId::monospace(13.0)).color(Color32::from_gray(140)));
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("wolfcraft:~$").font(FontId::monospace(14.0)).color(green));
                let id = Id::new("chat-input-edit");
                let resp = ui.add(egui::TextEdit::singleline(&mut st.input).id(id).frame(false).desired_width(f32::INFINITY).font(FontId::monospace(14.0)).text_color(Color32::WHITE).cursor_at_end(true));
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
                } else if !resp.has_focus() && !resp.lost_focus() {
                    resp.request_focus();
                }
                if resp.has_focus() {
                    if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                        st.browse_history(-1);
                    }
                    if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                        st.browse_history(1);
                    }
                }
                if resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                    let line = st.input.trim().to_string();
                    if !line.is_empty() {
                        st.remember(&line);
                        submitted = Some(line);
                    }
                    close = true;
                }
            });
        });
        ui.add_space(6.0);
        // Extra keys, like Termux.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let kw = (w + 20.0 - 4.0 * 5.0) / 6.0;
            if term_key(ui, "ESC", kw, false) {
                close = true;
            }
            if term_key(ui, "/", kw, false) && !st.input.starts_with('/') {
                st.input.insert(0, '/');
            }
            if term_key(ui, "TAB", kw, false) {
                // Complete a command from the quick list.
                let q = st.input.trim_start_matches('/').to_lowercase();
                if let Some((_, cmd)) = QUICK.iter().find(|(l, c)| !q.is_empty() && (l.starts_with(&q) || c.trim_start_matches('/').starts_with(&q))) {
                    st.input = cmd.to_string();
                }
            }
            if term_key(ui, "UP", kw, false) {
                st.browse_history(-1);
            }
            if term_key(ui, "DOWN", kw, false) {
                st.browse_history(1);
            }
            if term_key(ui, "ENTER", kw, true) {
                let line = st.input.trim().to_string();
                if !line.is_empty() {
                    st.remember(&line);
                    submitted = Some(line);
                }
                close = true;
            }
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(w + 20.0);
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for (label, cmd) in QUICK {
                let kw = 9.0 * label.len() as f32 + 22.0;
                if term_key(ui, label, kw, true) {
                    st.remember(cmd);
                    submitted = Some(cmd.to_string());
                    close = true;
                }
            }
        });
    });
    if close {
        st.close();
    }
    submitted
}

/// Draws the chat. Returns a submitted line (message or `/command`) when Enter was pressed.
pub fn draw(ctx: &Context, _icons: &GuiIcons, st: &mut ChatState, now: f64) -> Option<String> {
    let p = palette(ctx);
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("chat-log")));
    let width = 560.0f32.min(screen.width() - 40.0);
    let input_h = if st.open { 112.0 } else { 0.0 };
    let bottom = screen.bottom() - 16.0 - input_h;
    let n = st.visible_lines(now);
    let mut y = bottom;
    let font = FontId::monospace(14.0);

    // Newest at the bottom.
    let take = if st.open { 16 } else { 8 };
    let shown: Vec<&ChatLine> = st.lines.iter().rev().take(take).collect();
    let mut blocks = Vec::new();
    for l in shown.iter() {
        let age = now - l.born;
        let alpha = if st.open { 1.0 } else if age >= VISIBLE_SECS { continue } else { ((VISIBLE_SECS - age) / 1.5).clamp(0.0, 1.0) as f32 };
        let galley = painter.layout(l.text.clone(), font.clone(), kind_color(l.kind, &p).gamma_multiply(alpha), width - 24.0);
        blocks.push((galley, alpha));
    }
    if !st.open && !blocks.is_empty() {
        let total: f32 = blocks.iter().map(|(g, _)| g.size().y + 3.0).sum::<f32>() + 14.0;
        let max_alpha = blocks.iter().map(|(_, a)| *a).fold(0.0, f32::max);
        let rect = Rect::from_min_size(Pos2::new(14.0, bottom - total), Vec2::new(width, total));
        painter.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(6, 9, 8, (200.0 * max_alpha) as u8));
        for (g, _) in blocks {
            y -= g.size().y + 3.0;
            let h = g.size().y;
            painter.galley(Pos2::new(26.0, y - 4.0 + (h - h)), g, Color32::WHITE);
        }
    }
    let _ = n;

    let mut submitted = None;
    if st.open {
        submitted = terminal(ctx, st, screen);
    }
    submitted
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
