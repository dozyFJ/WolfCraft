//! In-game chat: a translucent panel at the top-left with "Ник: текст" lines and an input
//! field with a send button; closed, the last messages show there and fade.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{ANIM_TIME, palette};
use egui::{Align2, Color32, Context, FontId, Id, Key, LayerId, Order, Pos2, Rect, Stroke, Vec2};

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

/// One chat line as rich text, Bedrock style: "<Ник> текст" in white, system lines in light
/// grey-yellow, errors in red. `nick_color` tints the name when `colored` is set.
fn line_job(l: &ChatLine, size: f32, alpha: f32, wrap: f32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let fmt = |c: Color32| TextFormat { font_id: FontId::proportional(size), color: c.gamma_multiply(alpha), ..Default::default() };
    match l.kind {
        ChatKind::Player => {
            // Stored as "<Ник> текст".
            if let Some((name, text)) = l.text.strip_prefix('<').and_then(|t| t.split_once("> ")) {
                job.append("<", 0.0, fmt(Color32::WHITE));
                job.append(name, 0.0, fmt(nick_color(name).lerp_to_gamma(Color32::WHITE, 0.55)));
                job.append(">", 0.0, fmt(Color32::WHITE));
                job.append(text, 6.0, fmt(Color32::WHITE));
            } else {
                job.append(&l.text, 0.0, fmt(Color32::WHITE));
            }
        }
        ChatKind::System => job.append(&l.text, 0.0, fmt(Color32::from_rgb(255, 255, 160))),
        ChatKind::Error => job.append(&l.text, 0.0, fmt(Color32::from_rgb(255, 95, 85))),
    }
    job
}

/// Quick commands in the open chat: (button text, command). A command ending in a space
/// needs an argument and goes into the field; the rest run at once.
const QUICK: [(&str, &str); 22] = [
    ("Творческий", "/gamemode creative"),
    ("Выживание", "/gamemode survival"),
    ("День", "/time set day"),
    ("Ночь", "/time set night"),
    ("Полёт", "/fly"),
    ("Лечение", "/heal"),
    ("Накормить", "/feed"),
    ("На спавн", "/spawn"),
    ("Координаты", "/pos"),
    ("Биом", "/biome"),
    ("Сохранить", "/save"),
    ("Демо искрита", "/demo"),
    ("Выдать…", "/give "),
    ("Телепорт…", "/tp "),
    ("Сложность…", "/difficulty "),
    ("Сезон…", "/season "),
    ("Арена босса…", "/arena "),
    ("Арены", "/arena"),
    ("Дата", "/month"),
    ("Месяц…", "/month "),
    ("WorldEdit", "//help"),
    ("Все команды", "/help"),
];

/// Width of the message column (left side of the screen).
fn log_width(screen: Rect) -> f32 {
    (screen.width() * 0.46).clamp(300.0, 560.0)
}

/// Background of one message line.
const LINE_BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 128);
const BAR_BG: Color32 = Color32::from_rgba_premultiplied(18, 18, 20, 235);
const FIELD_BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 200);
const BUTTON_BG: Color32 = Color32::from_rgb(72, 72, 76);
const BUTTON_HI: Color32 = Color32::from_rgb(98, 98, 104);
const TEXT: f32 = 16.0;

/// Paints messages bottom-up from `bottom` at `left`, each on its own dark strip, as many as
/// fit above `top`. Returns nothing: purely visual.
fn paint_lines(painter: &egui::Painter, lines: &[(&ChatLine, f32)], left: f32, top: f32, bottom: f32, width: f32) {
    let mut y = bottom;
    for (l, alpha) in lines.iter().rev() {
        let g = painter.layout_job(line_job(l, TEXT, *alpha, width - 16.0));
        let h = g.size().y + 6.0;
        if y - h < top {
            break;
        }
        y -= h;
        let bg = Rect::from_min_size(Pos2::new(left, y), Vec2::new(width, h));
        painter.rect_filled(bg, 0.0, LINE_BG.gamma_multiply(*alpha));
        painter.galley(Pos2::new(left + 8.0, y + 3.0), g, Color32::WHITE);
    }
}

/// A flat square button like the Bedrock UI: grey with a darker bottom edge.
fn flat_button(ui: &mut egui::Ui, rect: Rect, id: &str) -> egui::Response {
    let r = ui.interact(rect, Id::new(id), egui::Sense::click());
    let fill = if r.is_pointer_button_down_on() || r.hovered() { BUTTON_HI } else { BUTTON_BG };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 4.0), rect.max), 0.0, Color32::from_rgb(40, 40, 44));
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(20, 20, 22)));
    r
}

/// Open chat (Bedrock layout): the screen dims, a header bar with a back arrow and the title,
/// the log on the left, and a full-width input bar at the bottom with a "/" (commands) button,
/// the text field and a send button.
fn open_chat(ctx: &Context, st: &mut ChatState, screen: Rect) -> Option<String> {
    let mut submitted = None;
    let mut close = false;
    egui::Area::new(Id::new("chat-open")).order(Order::Foreground).fixed_pos(screen.min).fade_in(false).show(ctx, |ui| {
        let (all, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
        let painter = ui.painter().clone();
        painter.rect_filled(all, 0.0, Color32::from_black_alpha(110));
        // Header.
        let head = Rect::from_min_size(all.min, Vec2::new(all.width(), 52.0));
        painter.rect_filled(head, 0.0, BAR_BG);
        painter.text(head.center(), Align2::CENTER_CENTER, "Чат", FontId::proportional(20.0), Color32::WHITE);
        let back = Rect::from_min_size(head.min + Vec2::new(10.0, 6.0), Vec2::splat(40.0));
        if flat_button(ui, back, "chat-back").clicked() {
            close = true;
        }
        let c = back.center();
        for (a, b) in [(Vec2::new(5.0, -9.0), Vec2::new(-5.0, 0.0)), (Vec2::new(-5.0, 0.0), Vec2::new(5.0, 9.0))] {
            painter.line_segment([c + a, c + b], Stroke::new(3.0_f32, Color32::WHITE));
        }
        // Input bar.
        let bar_h = 58.0;
        // Above the on-screen keyboard when it is up.
        let kb = crate::osk::height(ctx);
        let bar = Rect::from_min_max(Pos2::new(all.left(), all.bottom() - kb - bar_h), Pos2::new(all.right(), all.bottom() - kb));
        painter.rect_filled(bar, 0.0, BAR_BG);
        let pad = 8.0;
        let bh = bar_h - pad * 2.0;
        let slash = Rect::from_min_size(Pos2::new(bar.left() + pad, bar.top() + pad), Vec2::splat(bh));
        let send = Rect::from_min_size(Pos2::new(bar.right() - pad - 120.0, bar.top() + pad), Vec2::new(120.0, bh));
        let field = Rect::from_min_max(Pos2::new(slash.right() + pad, bar.top() + pad), Pos2::new(send.left() - pad, bar.bottom() - pad));
        if flat_button(ui, slash, "chat-slash").clicked() && !st.input.starts_with('/') {
            st.input.insert(0, '/');
        }
        painter.text(slash.center() - Vec2::new(0.0, 2.0), Align2::CENTER_CENTER, "/", FontId::proportional(22.0), Color32::WHITE);
        let can_send = !st.input.trim().is_empty();
        let sr = flat_button(ui, send, "chat-send");
        painter.text(send.center() - Vec2::new(0.0, 2.0), Align2::CENTER_CENTER, "Отправить", FontId::proportional(16.0), if can_send { Color32::WHITE } else { Color32::from_gray(150) });
        painter.rect_filled(field, 0.0, FIELD_BG);
        painter.rect_stroke(field, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(110, 110, 116)));
        let id = Id::new("chat-input-edit");
        let inner = field.shrink2(Vec2::new(10.0, 0.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let resp = child.add(
            egui::TextEdit::singleline(&mut st.input)
                .id(id)
                .frame(false)
                .desired_width(inner.width())
                .font(FontId::proportional(TEXT))
                .text_color(Color32::WHITE)
                .hint_text(egui::RichText::new("Нажмите здесь, чтобы написать в чат").color(Color32::from_gray(150)))
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
        // Quick commands on the right: a tap runs a ready command at once, or puts one that
        // needs an argument into the field.
        {
            let panel_w = (all.width() * 0.34).clamp(220.0, 360.0);
            let panel = Rect::from_min_max(Pos2::new(all.right() - panel_w - 10.0, head.bottom() + 8.0), Pos2::new(all.right() - 10.0, bar.top() - 8.0));
            painter.rect_filled(panel, 0.0, Color32::from_black_alpha(140));
            painter.text(Pos2::new(panel.left() + 10.0, panel.top() + 8.0), Align2::LEFT_TOP, "Быстрые команды", FontId::proportional(14.0), Color32::from_gray(200));
            let body = Rect::from_min_max(Pos2::new(panel.left() + 6.0, panel.top() + 30.0), Pos2::new(panel.right() - 6.0, panel.bottom() - 6.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
            egui::ScrollArea::vertical().id_salt("chat-quick").auto_shrink([false, false]).show(&mut child, |ui| {
                let gap = 6.0;
                let bw = (ui.available_width() - gap) / 2.0;
                ui.spacing_mut().item_spacing = Vec2::splat(gap);
                for row in QUICK.chunks(2) {
                    ui.horizontal(|ui| {
                        for (label, cmd) in row {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(bw, 40.0), egui::Sense::hover());
                            let r = flat_button(ui, rect, &format!("quick-{cmd}"));
                            let ready = !cmd.ends_with(' ');
                            ui.painter().text(rect.center() - Vec2::new(0.0, 2.0), Align2::CENTER_CENTER, *label, FontId::proportional(14.0), if ready { Color32::WHITE } else { Color32::from_rgb(255, 230, 140) });
                            if r.clicked() {
                                if ready {
                                    submitted = Some(cmd.to_string());
                                    close = true;
                                } else {
                                    st.input = cmd.to_string();
                                    st.prefill = st.input.clone();
                                    st.just_opened = true;
                                }
                            }
                        }
                    });
                }
            });
        }
        // Log: all recent messages on the left, newest just above the input bar.
        let w = log_width(screen);
        if st.lines.is_empty() {
            painter.text(Pos2::new(all.left() + 14.0, bar.top() - 14.0), Align2::LEFT_BOTTOM, "Сообщений пока нет. /help — список команд", FontId::proportional(TEXT), Color32::from_gray(190));
        }
        let lines: Vec<(&ChatLine, f32)> = st.lines.iter().rev().take(60).collect::<Vec<_>>().into_iter().rev().map(|l| (l, 1.0)).collect();
        paint_lines(&painter, &lines, all.left() + 10.0, head.bottom() + 8.0, bar.top() - 8.0, w);
        let enter = resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter));
        if enter || (sr.clicked() && can_send) {
            let line = st.input.trim().to_string();
            if !line.is_empty() {
                st.remember(&line);
                submitted = Some(line);
            }
            close = true;
        }
    });
    if close {
        st.close();
    }
    submitted
}

/// Draws the chat. Returns a submitted line (message or `/command`) when it was sent.
pub fn draw(ctx: &Context, _icons: &GuiIcons, st: &mut ChatState, now: f64) -> Option<String> {
    let _ = (palette(ctx), ANIM_TIME);
    let screen = ctx.screen_rect();
    if st.open {
        return open_chat(ctx, st, screen);
    }
    // Closed: the last messages on the left (under the top buttons), each on its own dark
    // strip, fading out after a few seconds.
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("chat-log")));
    let lines: Vec<(&ChatLine, f32)> = st
        .lines
        .iter()
        .rev()
        .take(6)
        .filter(|l| now - l.born < VISIBLE_SECS)
        .map(|l| (l, ((VISIBLE_SECS - (now - l.born)) / 1.5).clamp(0.0, 1.0) as f32))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if lines.is_empty() {
        return None;
    }
    let top = screen.top() + 92.0;
    let bottom = top + (screen.height() * 0.42).clamp(150.0, 300.0);
    // Bottom-up layout, but the block hugs the top: measure first, then paint from the top.
    let w = log_width(screen);
    let heights: f32 = lines.iter().map(|(l, a)| painter.layout_job(line_job(l, TEXT, *a, w - 16.0)).size().y + 6.0).sum();
    paint_lines(&painter, &lines, screen.left() + 14.0, top, (top + heights).min(bottom), w);
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
