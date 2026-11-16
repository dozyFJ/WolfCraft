//! Menu screens: main menu, world selection, world creation, settings, loading, pause.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{ANIM_TIME, Palette, bold_font, palette};
use crate::ui_components::*;
use egui::{Align2, Color32, FontId, Frame, Margin, Pos2, Rect, Rounding, Sense, Stroke, TextureHandle, Ui, Vec2};
use wolfcraft_assets::gui as icon;
use wolfcraft_engine::config::*;

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    None,
    OpenWorldSelect,
    /// Enter the main menu of the installed game version.
    LaunchVersion(String),
    OpenCreate,
    OpenSettings,
    CheckUpdates,
    Quit,
    Back,
    PlayWorld(String),
    /// Connect to a WolfServer at this address.
    JoinServer(String),
    /// Start a server on this device and join it: with one of the player's worlds
    /// (private world for friends) or a fresh network world (`None`).
    HostServer(Option<String>),
    CreateWorld(Box<WorldConfig>),
    DeleteWorld(String),
    RenameWorld(String, String),
    ToggleTheme,
    Resume,
    SaveAndQuit,
    SaveGame,
    /// Pick a skin preset (launcher "Скины" tab).
    SetSkin(u8),
    /// New nickname from the launcher profile.
    SetNickname(String),
    /// Studio: create a server (place) in the menu without opening the editor.
    /// `template`: 0 baseplate, 1 flat, 2 planet.
    CreateStudioProject { name: String, template: u8 },
    /// Open this world in the studio (editor).
    OpenStudio(String),
    /// Join a server's world (address) with the studio tools: edits go straight to the server.
    EditServerInStudio(String),
    /// Studio: a new empty project (a baseplate world) opened right away.
    NewStudioProject,
    /// World list opened straight on "Игра по сети → Рядом".
    OpenNetwork,
}

pub struct WorldEntry {
    pub cfg: WorldConfig,
    pub preview: Option<TextureHandle>,
}

// ---------------------------------------------------------------------------------------------
// Shared chrome

/// Former top bar: the header is gone, its title and version are shown in the centre of the
/// bottom bar instead. Always returns false (settings open from the main menu).
pub fn top_bar(ctx: &egui::Context, _icons: &GuiIcons, title: &str, version: &str, _show_settings: bool) -> bool {
    let text = if title.is_empty() { format!("WolfCraft · v{version}") } else { format!("{title} · v{version}") };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("menu-bar-title"), text));
    false
}

/// Scrollable launcher-style tabs keep all sections reachable on narrow phones.
fn menu_tabs(ui: &mut Ui, id: &str, labels: &[&str], selected: &mut usize) {
    egui::ScrollArea::horizontal().id_salt(id).auto_shrink([false, true]).show(ui, |ui| {
        tab_bar(ui, id, labels, selected);
    });
}

fn menu_header_frame(p: &Palette) -> Frame {
    Frame::none().fill(crate::launcher::glass_tint(p))
        .stroke(Stroke::new(1.0, crate::launcher::glass_rim(p)))
        .inner_margin(Margin::symmetric(14.0, 10.0))
}

fn menu_card_frame(p: &Palette, margin: f32) -> Frame {
    Frame::none().fill(crate::launcher::glass_tint(p)).rounding(Rounding::same(18.0))
        .stroke(Stroke::new(1.0, crate::launcher::glass_rim(p)))
        .inner_margin(Margin::same(margin))
}

// ---------------------------------------------------------------------------------------------
// Main menu

/// Hints shown under the splash loading bar.
const TIPS: [&str; 9] = [
    "Планета — это куб: дойдите до ребра и шагните на соседнюю грань",
    "В творческом режиме дважды нажмите прыжок, чтобы полететь",
    "Чем выше летите, тем быстрее: в космосе скорость растёт",
    "Верстак открывает сетку крафта 3×3",
    "Свои текстуры: Настройки → Текстуры → загрузить пак по ссылке",
    "Пауза и чат — в островке вверху экрана",
    "Корзина в творческом инвентаре удаляет предмет из выбранного слота",
    "Сервер на телефоне: Играть → Сеть → Создать сервер",
    "Ночью появляются монстры — постройте укрытие",
];

/// Studio splash: logo, name, a loading bar and rotating tips. Returns true when tapped.
/// Loading and splash: stars over a dark sky, the name large, one short line of what is
/// happening, a slim bar that glides to its value, and a tip. No counters or technical text.
fn loading_panel(ctx: &egui::Context, icons: &GuiIcons, title: &str, subtitle: &str, fraction: f32, tip: &str, version: &str, interactive: bool) -> bool {
    let _ = icons;
    let p = palette(ctx);
    let mut clicked = false;
    // The bar moves smoothly instead of jumping between chunk batches.
    let shown = ctx.animate_value_with_time(egui::Id::new("loading-bar"), fraction.clamp(0.0, 1.0), 0.35);
    egui::CentralPanel::default().frame(Frame::none()).show(ctx, |ui| {
        let rect = ui.max_rect();
        let t = ui.input(|i| i.time) as f32;
        world_backdrop(ui.painter(), rect, t);
        if interactive {
            clicked = ui.interact(rect, egui::Id::new("splash-skip"), Sense::click()).clicked();
        }
        let scale = (rect.height() / 540.0).clamp(0.6, 1.0);
        let painter = ui.painter();
        let cx = rect.center().x;
        let mid = rect.top() + rect.height() * 0.40;
        // A soft dark glow behind the text so it reads on the moving world.
        let glow = Rect::from_center_size(Pos2::new(cx, mid), Vec2::new((rect.width() * 0.9).min(520.0), 190.0 * scale));
        for k in 0..6 {
            let g = glow.shrink2(Vec2::new(k as f32 * 18.0, k as f32 * 12.0) * scale);
            painter.rect_filled(g, Rounding::same(80.0), Color32::from_black_alpha(14));
        }
        wave_title(painter, Pos2::new(cx, mid - 34.0 * scale), title, 44.0 * scale, t);
        // Status line: letters come in one by one, then the dots run.
        let now = ui.input(|i| i.time);
        let key = egui::Id::new(("loading-sub", subtitle));
        let start = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(key, || now));
        let since = (now - start) as f32;
        let n = subtitle.chars().count();
        let shown_chars = ((since * 28.0) as usize).min(n);
        let dots = if shown_chars == n && !subtitle.ends_with('…') { [".", "..", "...", ""][((t * 2.5) as usize) % 4] } else { "" };
        let line: String = subtitle.chars().take(shown_chars).collect::<String>() + dots;
        let pulse = 0.75 + 0.25 * (t * 2.2).sin().abs();
        painter.text(Pos2::new(cx + 1.0, mid + 1.0), Align2::CENTER_CENTER, &line, FontId::proportional(16.0), Color32::from_black_alpha(150));
        painter.text(Pos2::new(cx, mid), Align2::CENTER_CENTER, &line, FontId::proportional(16.0), Color32::from_white_alpha((230.0 * pulse) as u8));
        let w = (rect.width() * 0.62).clamp(200.0, 440.0);
        let bar = Rect::from_center_size(Pos2::new(cx, mid + 40.0 * scale), Vec2::new(w, 22.0));
        liquid_bar(painter, bar, shown, t, p.accent);
        painter.text(Pos2::new(bar.right(), bar.bottom() + 8.0), Align2::RIGHT_TOP, format!("{:.0}%", shown * 100.0), FontId::proportional(13.0), Color32::from_white_alpha(200));
        // The tip near the bottom on a dark strip, wrapped; it slides in when it changes.
        let g = painter.layout(tip.to_string(), FontId::proportional(13.5), Color32::from_white_alpha(225), (rect.width() - 64.0).min(560.0));
        let phase = (t / 5.0).fract();
        let slide = (1.0 - (phase * 8.0).min(1.0)).powi(2) * 24.0;
        let alpha = ((phase * 8.0).min(1.0) * ((1.0 - phase) * 12.0).min(1.0)).clamp(0.0, 1.0);
        let tip_pos = Pos2::new(cx - g.size().x / 2.0, rect.bottom() - 46.0 * scale - g.size().y + slide);
        let back = Rect::from_min_size(tip_pos, g.size()).expand2(Vec2::new(14.0, 8.0));
        painter.rect_filled(back, Rounding::same(12.0), Color32::from_black_alpha((120.0 * alpha) as u8));
        painter.galley(tip_pos, g, Color32::from_white_alpha((225.0 * alpha) as u8));
        painter.text(Pos2::new(rect.right() - 12.0, rect.bottom() - 8.0), Align2::RIGHT_BOTTOM, format!("v{version}"), FontId::proportional(11.0), Color32::from_white_alpha(170));
    });
    ctx.request_repaint_after(std::time::Duration::from_millis(16));
    clicked
}

/// The title with each letter riding a wave and a light sweeping across it.
fn wave_title(painter: &egui::Painter, center_bottom: Pos2, text: &str, size: f32, t: f32) {
    let font = bold_font(size);
    let glyphs: Vec<(String, f32)> = text.chars().map(|c| {
        let s = c.to_string();
        let w = painter.layout_no_wrap(s.clone(), font.clone(), Color32::WHITE).size().x;
        (s, w)
    }).collect();
    let total: f32 = glyphs.iter().map(|g| g.1).sum();
    let mut x = center_bottom.x - total / 2.0;
    let n = glyphs.len().max(1) as f32;
    // The sweep runs across every 3 s.
    let sweep = (t / 3.0).fract() * (n + 6.0) - 3.0;
    for (i, (s, w)) in glyphs.iter().enumerate() {
        let fi = i as f32;
        let dy = (t * 3.0 - fi * 0.55).sin() * size * 0.08;
        let shine = (1.0 - ((fi - sweep).abs() / 2.0)).clamp(0.0, 1.0);
        let pos = Pos2::new(x, center_bottom.y + dy);
        painter.text(pos + Vec2::new(2.0, 3.0), Align2::LEFT_BOTTOM, s, font.clone(), Color32::from_black_alpha(140));
        let base = Color32::from_rgb(235, 245, 255);
        let hot = Color32::from_rgb(140, 220, 255);
        let c = lerp_color(base, hot, shine);
        painter.text(pos, Align2::LEFT_BOTTOM, s, font.clone(), c);
        x += w;
    }
}

fn lerp_color(a: Color32, b: Color32, k: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// A glass capsule that fills with liquid: a wobbling front, waves on top, bubbles and a gloss.
fn liquid_bar(painter: &egui::Painter, bar: Rect, fill: f32, t: f32, accent: Color32) {
    let r = bar.height() / 2.0;
    painter.rect_filled(bar.expand(2.0), Rounding::same(r + 2.0), Color32::from_black_alpha(110));
    painter.rect_filled(bar, Rounding::same(r), Color32::from_white_alpha(18));
    // Half-height of the capsule at x: round ends.
    let half = |x: f32| -> f32 {
        let dl = bar.left() + r - x;
        let dr = x - (bar.right() - r);
        let d = dl.max(dr).max(0.0);
        (r * r - d * d).max(0.0).sqrt()
    };
    let front = bar.left() + bar.width() * fill;
    let deep = Color32::from_rgb(20, 90, 200);
    let top = lerp_color(accent, Color32::from_rgb(120, 230, 255), 0.4);
    let back_c = Color32::from_rgba_unmultiplied(top.r(), top.g(), top.b(), 110);
    if fill > 0.002 {
        let step = 2.0;
        for layer in 0..2 {
            let mut m = egui::Mesh::default();
            let (amp, k, sp, off) = if layer == 0 { (2.4, 0.09, 2.6, 1.3) } else { (2.0, 0.12, -3.4, 0.0) };
            let mut x = bar.left();
            loop {
                // The front edge wobbles like a pushed wave.
                let wob = (t * 4.0).sin() * 3.0 + (t * 6.3 + 1.0).sin() * 1.5;
                let end = front + wob;
                if x > end || x > bar.right() {
                    break;
                }
                let h = half(x);
                if h > 0.0 {
                    let near = ((end - x) / 10.0).clamp(0.0, 1.0);
                    let level = bar.center().y - h * 0.55 + ((x * k + t * sp + off).sin() * amp + (x * k * 2.3 - t * sp * 0.7).sin() * amp * 0.4) * near.max(0.4);
                    let y0 = level.max(bar.center().y - h + 0.5);
                    let y1 = bar.center().y + h - 0.5;
                    let (ct, cb) = if layer == 0 { (back_c, back_c) } else { (top, deep) };
                    let i = m.vertices.len() as u32;
                    m.colored_vertex(Pos2::new(x, y0), ct);
                    m.colored_vertex(Pos2::new(x, y1.max(y0)), cb);
                    if i >= 2 {
                        m.add_triangle(i - 2, i - 1, i);
                        m.add_triangle(i - 1, i + 1, i);
                    }
                }
                x += step;
            }
            painter.add(egui::Shape::mesh(m));
        }
        // Bubbles rising through the liquid.
        for b in 0..10u32 {
            let h = b.wrapping_mul(2654435761) ^ 0x5bd1_e995;
            let bx = bar.left() + r + ((h % 1000) as f32 / 1000.0) * (bar.width() - 2.0 * r);
            if bx > front - 4.0 {
                continue;
            }
            let speed = 0.5 + ((h >> 10) % 100) as f32 / 120.0;
            let ph = (t * speed + (h % 97) as f32 / 97.0).fract();
            let hh = half(bx);
            let by = bar.center().y + hh * 0.8 - ph * hh * 1.4;
            let wiggle = (t * 5.0 + b as f32).sin() * 1.2;
            painter.circle_stroke(Pos2::new(bx + wiggle, by), 1.0 + ((h >> 4) % 3) as f32 * 0.5, Stroke::new(1.0_f32, Color32::from_white_alpha((150.0 * (1.0 - ph)) as u8)));
        }
    }
    // Glass gloss on top and the rim.
    let gloss = Rect::from_min_max(Pos2::new(bar.left() + r * 0.7, bar.top() + 3.0), Pos2::new(bar.right() - r * 0.7, bar.top() + 6.0));
    painter.rect_filled(gloss, Rounding::same(2.0), Color32::from_white_alpha(40));
    painter.rect_stroke(bar, Rounding::same(r), Stroke::new(1.5_f32, Color32::from_white_alpha(120)));
}

/// A living world behind the loading text: dusk sky, drifting clouds, blocky hills scrolling
/// past at two depths, and a lake with moving waves and glints.
pub(crate) fn world_backdrop(painter: &egui::Painter, rect: Rect, t: f32) {
    let vgrad = |r: Rect, a: Color32, b: Color32| {
        let mut m = egui::Mesh::default();
        m.colored_vertex(r.left_top(), a);
        m.colored_vertex(r.right_top(), a);
        m.colored_vertex(r.right_bottom(), b);
        m.colored_vertex(r.left_bottom(), b);
        m.add_triangle(0, 1, 2);
        m.add_triangle(0, 2, 3);
        egui::Shape::mesh(m)
    };
    let horizon = rect.top() + rect.height() * 0.70;
    painter.add(vgrad(Rect::from_min_max(rect.min, Pos2::new(rect.right(), horizon)), Color32::from_rgb(10, 18, 44), Color32::from_rgb(232, 138, 92)));
    // Sun low on the horizon.
    let sun = Pos2::new(rect.left() + rect.width() * 0.72, horizon - rect.height() * 0.10);
    for k in (1..7).rev() {
        painter.circle_filled(sun, 18.0 + k as f32 * 14.0, Color32::from_rgba_unmultiplied(255, 190, 120, 10));
    }
    painter.circle_filled(sun, 22.0, Color32::from_rgb(255, 226, 170));
    // Stars fading towards the horizon.
    for i in 0..70u32 {
        let h = i.wrapping_mul(2654435761).rotate_left(7) ^ 0x9e37_79b9;
        let x = rect.left() + (h % 10_000) as f32 / 10_000.0 * rect.width();
        let yk = ((h / 10_000) % 10_000) as f32 / 10_000.0;
        let y = rect.top() + yk * (horizon - rect.top()) * 0.6;
        let tw = 0.5 + 0.5 * (t * (1.0 + (h % 5) as f32 * 0.4) + i as f32).sin();
        painter.circle_filled(Pos2::new(x, y), 1.0, Color32::from_white_alpha(((1.0 - yk) * 160.0 * tw) as u8));
    }
    // Clouds drifting.
    for i in 0..6u32 {
        let h = i.wrapping_mul(0x27d4_eb2d) ^ 0x1656_67b1;
        let sp = 6.0 + (h % 10) as f32;
        let span = rect.width() + 260.0;
        let x = rect.left() - 130.0 + ((h % 1000) as f32 / 1000.0 * span + t * sp).rem_euclid(span);
        let y = rect.top() + rect.height() * (0.12 + ((h >> 8) % 100) as f32 / 100.0 * 0.40);
        let c = Color32::from_rgba_unmultiplied(255, 220, 210, 26);
        for (dx, dy, rr) in [(-34.0, 6.0, 20.0), (0.0, 0.0, 28.0), (34.0, 5.0, 22.0), (14.0, 12.0, 20.0), (-14.0, 12.0, 18.0)] {
            painter.circle_filled(Pos2::new(x + dx, y + dy), rr, c);
        }
    }
    // Blocky hills: column heights from smooth value noise, in whole blocks.
    let noise = |i: i64, seed: u32| -> f32 {
        let hv = |n: i64| ((n as u32).wrapping_mul(0x9E37_79B1) ^ seed).wrapping_mul(0x85EB_CA6B).rotate_left(13) as f32 / u32::MAX as f32;
        let a = i.div_euclid(6);
        let f = i.rem_euclid(6) as f32 / 6.0;
        let s = f * f * (3.0 - 2.0 * f);
        hv(a) * (1.0 - s) + hv(a + 1) * s
    };
    let water_y = rect.top() + rect.height() * 0.80;
    for (bs, speed, seed, base, amp, grass, dirt) in [
        (12.0f32, 8.0f32, 7u32, horizon - 10.0, 7.0f32, Color32::from_rgb(52, 70, 92), Color32::from_rgb(40, 52, 74)),
        (20.0, 22.0, 31, water_y - 6.0, 6.0, Color32::from_rgb(74, 150, 70), Color32::from_rgb(96, 66, 44)),
    ] {
        let scroll = t * speed;
        let first = ((rect.left() + scroll) / bs).floor() as i64;
        let cols = (rect.width() / bs) as i64 + 2;
        for c in first..first + cols {
            let x = c as f32 * bs - scroll;
            let hb = (noise(c, seed) * amp).round() + 2.0;
            let ytop = base - hb * bs;
            let col = Rect::from_min_max(Pos2::new(x, ytop), Pos2::new(x + bs + 0.5, water_y + 2.0));
            painter.rect_filled(col, Rounding::ZERO, dirt);
            painter.rect_filled(Rect::from_min_size(col.min, Vec2::new(bs + 0.5, bs * 0.35)), Rounding::ZERO, grass);
            // Block seams.
            let mut y = ytop + bs;
            while y < water_y {
                painter.line_segment([Pos2::new(x, y), Pos2::new(x + bs, y)], Stroke::new(1.0_f32, Color32::from_black_alpha(28)));
                y += bs;
            }
            // A tree now and then on the near hills.
            if bs > 15.0 && (c.wrapping_mul(2654435761) as u32) % 9 == 0 {
                let trunk = Rect::from_min_size(Pos2::new(x + bs * 0.35, ytop - bs * 1.6), Vec2::new(bs * 0.3, bs * 1.6));
                painter.rect_filled(trunk, Rounding::ZERO, Color32::from_rgb(90, 60, 36));
                painter.rect_filled(Rect::from_center_size(Pos2::new(x + bs * 0.5, ytop - bs * 2.0), Vec2::splat(bs * 1.4)), Rounding::ZERO, Color32::from_rgb(52, 120, 56));
            }
        }
    }
    // The lake: layered waves with a gradient, sun glints running on top.
    for layer in 0..2 {
        let mut m = egui::Mesh::default();
        let (amp, k, sp, a_top, a_bot) = if layer == 0 {
            (4.0, 0.025, 1.2, Color32::from_rgba_unmultiplied(90, 160, 220, 150), Color32::from_rgba_unmultiplied(20, 50, 110, 200))
        } else {
            (3.0, 0.04, -1.8, Color32::from_rgb(60, 140, 210), Color32::from_rgb(8, 26, 60))
        };
        let mut x = rect.left();
        let base = water_y + layer as f32 * 6.0;
        while x <= rect.right() + 6.0 {
            let y = base + (x * k + t * sp).sin() * amp + (x * k * 2.7 - t * sp * 1.3).sin() * amp * 0.35;
            let i = m.vertices.len() as u32;
            m.colored_vertex(Pos2::new(x, y), a_top);
            m.colored_vertex(Pos2::new(x, rect.bottom()), a_bot);
            if i >= 2 {
                m.add_triangle(i - 2, i - 1, i);
                m.add_triangle(i - 1, i + 1, i);
            }
            x += 6.0;
        }
        painter.add(egui::Shape::mesh(m));
    }
    // Sun path on the water.
    for j in 0..14u32 {
        let fy = j as f32 / 14.0;
        let y = water_y + 10.0 + fy * (rect.bottom() - water_y - 10.0);
        let w = 10.0 + fy * 60.0;
        let shift = (t * 2.0 + j as f32 * 1.7).sin() * 8.0;
        let a = (0.5 + 0.5 * (t * 3.0 + j as f32).sin()) * (1.0 - fy) * 120.0;
        painter.line_segment([Pos2::new(sun.x - w / 2.0 + shift, y), Pos2::new(sun.x + w / 2.0 + shift, y)], Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 220, 170, a as u8)));
    }
    // Darken a bit so the text stays readable.
    painter.add(vgrad(rect, Color32::from_black_alpha(90), Color32::from_black_alpha(40)));
}

pub fn splash_screen(ctx:&egui::Context,icons:&GuiIcons,progress:f32,version:&str)->bool {
    let tip=TIPS[((ctx.input(|i|i.time)/5.0) as usize)%TIPS.len()];
    loading_panel(ctx,icons,"WolfCraft","Подготовка игры",progress,tip,version,true)
}

/// Space backdrop with a big planet rising from the bottom edge (the planet of the last played world).
#[allow(dead_code)]
fn planet_backdrop(ui: &Ui, planet: Option<&TextureHandle>, p: &Palette) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(2, 7, 13));
    // Twinkling stars at fixed pseudo-random spots.
    let t = ui.input(|i| i.time) as f32;
    for i in 0..140u32 {
        let h = i.wrapping_mul(2654435761).rotate_left(7) ^ 0x9e37_79b9;
        let x = (h % 10_000) as f32 / 10_000.0;
        let y = ((h / 10_000) % 10_000) as f32 / 10_000.0;
        // Static stars: no twinkling, nothing in the menu moves (no shimmer on phones).
        let tw = 0.55 + 0.45 * ((h % 7) as f32 / 6.0);
        let r = if h % 11 == 0 { 1.6 } else { 1.0 };
        painter.circle_filled(Pos2::new(rect.left() + x * rect.width(), rect.top() + y * rect.height()), r, Color32::from_white_alpha((150.0 * tw) as u8));
    }
    if let Some(tex) = planet {
        // Slow drift up and down so the screen feels alive.
        let d = rect.width().max(rect.height() * 1.6);
        // Low on the screen: only the top of the planet shows, under the buttons.
        let c = Pos2::new(rect.center().x.round(), (rect.bottom() + d * 0.30).round());
        painter.image(tex.id(), Rect::from_center_size(c, Vec2::splat(d)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
    // Soft shade so the menu stays readable, darker from the top down to the middle where the
    // logo and title sit.
    painter.rect_filled(rect, Rounding::ZERO, p.bg.gamma_multiply(if p.dark { 0.25 } else { 0.15 }));
    let mut m = egui::Mesh::default();
    let dark = Color32::from_black_alpha(200);
    let clear = Color32::from_black_alpha(0);
    let mid = rect.top() + rect.height() * 0.62;
    m.colored_vertex(rect.left_top(), dark);
    m.colored_vertex(rect.right_top(), dark);
    m.colored_vertex(Pos2::new(rect.right(), mid), clear);
    m.colored_vertex(Pos2::new(rect.left(), mid), clear);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(m));
    let _ = t;
}

/// Classic game main menu, entered after launching from the launcher.
pub fn game_menu(ctx: &egui::Context, icons: &GuiIcons, version: &str, planet: Option<&TextureHandle>) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    egui::CentralPanel::default().frame(Frame::none()).show(ctx, |ui| {
        let _ = planet;
        let rect = ui.max_rect();
        // The same living world as the loading screen behind the menu.
        let t = ui.input(|i| i.time) as f32;
        world_backdrop(ui.painter(), rect, t);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        // One list of buttons (no dock): Играть first, then the rest in pairs, Выход, and a small
        // version badge at the end. Above it the logo: icon on the left, WOLFCRAFT in capitals,
        // a line under it. Lying phone: logo on the left half, buttons on the right.
        let wide = rect.width() > rect.height() * 1.2;
        // Upright phone: the title and one column of buttons, all of it in the middle of the
        // screen.
        let list_h = 3.0 * 60.0 + 34.0;
        let (logo_area, list_area) = if wide {
            (Rect::from_min_max(rect.min, Pos2::new(rect.center().x, rect.bottom())), Rect::from_min_max(Pos2::new(rect.center().x, rect.top()), rect.max))
        } else {
            let top = (rect.center().y - (100.0 + list_h) / 2.0).max(rect.top() + 8.0);
            (Rect::from_min_max(Pos2::new(rect.left(), top), Pos2::new(rect.right(), top + 100.0)), Rect::from_min_max(Pos2::new(rect.left(), top + 100.0), rect.max))
        };
        let width = (list_area.width() - 32.0).clamp(200.0, 380.0);
        // Aero: one glass pane behind the title and the buttons (like the launcher's cards).
        {
            let pane = if wide {
                Rect::from_center_size(list_area.center(), Vec2::new(width + 36.0, list_h + 36.0))
            } else {
                Rect::from_min_max(Pos2::new(list_area.center().x - width / 2.0 - 18.0, logo_area.top() + 6.0), Pos2::new(list_area.center().x + width / 2.0 + 18.0, list_area.top() + list_h + 18.0))
            };
            aero_pane(ui.painter(), pane, &p);
        }
        // Title: just the name in capitals and a line under it, centred (no icon).
        {
            let painter = ui.painter();
            let size = (logo_area.width() * 0.105).clamp(26.0, 40.0);
            let cy = logo_area.center().y + if wide { -10.0 } else { 0.0 };
            // The loading screen's logo: «WolfCraft» with waving letters.
            wave_title(painter, Pos2::new(logo_area.center().x, cy - 4.0), "WolfCraft", size * 1.15, t);
            painter.text(Pos2::new(logo_area.center().x + 1.0, cy + 5.0), Align2::CENTER_TOP, "Целая планета из блоков", FontId::proportional((size * 0.40).max(13.0)), Color32::from_black_alpha(150));
            painter.text(Pos2::new(logo_area.center().x, cy + 4.0), Align2::CENTER_TOP, "Целая планета из блоков", FontId::proportional((size * 0.40).max(13.0)), Color32::from_white_alpha(220));
        }
        let height = (list_area.height() - 16.0).max(150.0);
        let col = if wide { Rect::from_center_size(list_area.center(), Vec2::new(width, height)) } else { Rect::from_min_size(Pos2::new(list_area.center().x - width / 2.0, list_area.top()), Vec2::new(width, height)) };
        let sheet_id = egui::Id::new("game-menu-sheet");
        let open: Option<Sheet> = ui.ctx().data(|d| d.get_temp(sheet_id)).flatten();
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(col), |ui| {
            egui::ScrollArea::vertical().id_salt("game-main-menu").show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    let compact = wide && rect.height() < 420.0;
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, if compact { 6.0 } else { 10.0 });
                    let w = width.min(380.0);
                    let half = (w - 10.0) / 2.0;
                    if aero_menu_button(ui, icons, &p, icon::PLAY, "Играть", Some(Color32::from_rgb(62, 172, 56)), w).clicked() {
                        action = MenuAction::OpenWorldSelect;
                    }
                    let mut sheet = |ui: &mut Ui, want: Sheet| {
                        let next = if open == Some(want) { None } else { Some(want) };
                        ui.ctx().data_mut(|d| d.insert_temp(sheet_id, next));
                    };
                    // Just three buttons: Играть, Настройки, Выход.
                    let rows: [[(u16, &str, u8); 1]; 1] = [[(icon::GEAR, "Настройки", 5)]];
                    // Lying phone: pairs side by side; upright: one button under another.
                    let rows: Vec<Vec<(u16, &str, u8)>> = if wide { rows.iter().map(|r| r.to_vec()).collect() } else { rows.iter().flatten().map(|b| vec![*b]).collect() };
                    for row in rows {
                        let bw = if row.len() == 1 { w } else { half };
                        ui.horizontal(|ui| {
                            ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                            for (ic, label, k) in row {
                                let on = (k == 3 && open == Some(Sheet::News)) || (k == 4 && open == Some(Sheet::More));
                                if aero_menu_button(ui, icons, &p, ic, label, on.then_some(p.accent), bw).clicked() {
                                    match k {
                                        0 => action = MenuAction::OpenNetwork,
                                        1 => action = open_settings_at(ui.ctx(), 0),
                                        2 => action = open_settings_at(ui.ctx(), 2),
                                        3 => sheet(ui, Sheet::News),
                                        4 => sheet(ui, Sheet::More),
                                        _ => action = MenuAction::OpenSettings,
                                    }
                                }
                            }
                        });
                    }
                    if aero_menu_button(ui, icons, &p, icon::EXIT, "Выход", None, w)
                        .on_hover_text("Вернуться в лаунчер").clicked() {
                        action = MenuAction::Back;
                    }
                    // The version: a small badge at the end of the list.
                    let short: String = version.split('.').take(2).collect::<Vec<_>>().join(".");
                    let g = ui.painter().layout_no_wrap(format!("v{short}"), FontId::proportional(12.0), Color32::from_white_alpha(200));
                    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 22.0, 24.0), Sense::hover());
                    ui.painter().rect_filled(r, Rounding::same(12.0), crate::launcher::glass_tint(&p));
                    ui.painter().add(crate::launcher::grad_shape(Rect::from_min_max(r.min + Vec2::splat(1.0), Pos2::new(r.right() - 1.0, r.center().y)), 11.0, 0.0, crate::launcher::glass_gloss(&p), Color32::TRANSPARENT));
                    ui.painter().rect_stroke(r, Rounding::same(12.0), Stroke::new(1.0_f32, crate::launcher::glass_rim(&p)));
                    ui.painter().galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
                });
            });
        });
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("game-dock-h"), 24.0f32));
    });
    if let Some(a) = game_menu_sheet(ctx, icons, &p) {
        action = a;
    }
    action
}

/// A glass pane in the launcher's Aero style: shadow, tint, a sheen fading downwards, rims.
fn aero_pane(pt: &egui::Painter, r: Rect, p: &crate::theme_manager::Palette) {
    use crate::launcher::{glass_gloss, glass_rim, glass_tint, grad_shape};
    pt.rect_filled(r.translate(Vec2::new(0.0, 5.0)).expand(2.0), Rounding::same(24.0), Color32::from_black_alpha(55));
    pt.rect_filled(r, Rounding::same(22.0), glass_tint(p));
    pt.add(grad_shape(Rect::from_min_max(r.min + Vec2::splat(1.0), Pos2::new(r.right() - 1.0, r.top() + (r.height() * 0.45).min(110.0))), 21.0, 0.0, glass_gloss(p).gamma_multiply(1.3), Color32::TRANSPARENT));
    pt.rect_stroke(r.shrink(1.0), Rounding::same(21.0), Stroke::new(1.0_f32, glass_rim(p).gamma_multiply(0.7)));
    pt.rect_stroke(r, Rounding::same(22.0), Stroke::new(1.0_f32, Color32::from_black_alpha(70)));
}

/// A glossy Aero menu button: a coloured capsule (`fill`) or clear glass (None).
fn aero_menu_button(ui: &mut Ui, icons: &GuiIcons, p: &crate::theme_manager::Palette, ic: u16, label: &str, fill: Option<Color32>, w: f32) -> egui::Response {
    use crate::launcher::{glass_gloss, glass_rim, gloss_capsule, grad_shape, shadow_text};
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 50.0), Sense::click());
    let press = resp.is_pointer_button_down_on();
    let hot = resp.hovered() || press;
    match fill {
        Some(c) => gloss_capsule(ui, r, if press { crate::launcher::mix(c, Color32::BLACK, 0.15) } else { c }),
        None => {
            let pt = ui.painter();
            pt.rect_filled(r, Rounding::same(25.0), Color32::from_black_alpha(if p.dark { 70 } else { 22 }));
            let shine = Rect::from_min_max(r.min + Vec2::new(16.0, 2.5), Pos2::new(r.right() - 16.0, r.center().y));
            pt.add(grad_shape(shine, shine.height() * 0.7, shine.height() * 0.3, glass_gloss(p).gamma_multiply(if hot { 2.2 } else { 1.4 }), Color32::TRANSPARENT));
            pt.rect_stroke(r, Rounding::same(25.0), Stroke::new(1.0_f32, glass_rim(p).gamma_multiply(if hot { 1.0 } else { 0.7 })));
        }
    }
    let col = if fill.is_some() { Color32::WHITE } else { p.text };
    let galley = ui.painter().layout_no_wrap(label.to_string(), bold_font(16.0), col);
    let total = galley.size().x + 28.0;
    let x = r.center().x - total / 2.0;
    icons.paint(ui.painter(), Rect::from_center_size(Pos2::new(x + 10.0, r.center().y), Vec2::splat(20.0)), ic, col);
    let pos = Pos2::new(x + 28.0, r.center().y);
    if fill.is_some() {
        shadow_text(ui.painter(), pos, Align2::LEFT_CENTER, label, bold_font(16.0), col);
    } else {
        ui.painter().text(pos, Align2::LEFT_CENTER, label, bold_font(16.0), col);
    }
    resp
}

/// What the dock's «Новости» and «Ещё» open over the menu.
#[derive(Clone, Copy, PartialEq)]
enum Sheet {
    News,
    More,
}

fn open_settings_at(ctx: &egui::Context, tab: usize) -> MenuAction {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("settings-tab"), tab));
    MenuAction::OpenSettings
}

/// The dock under the game menu: Скины, Магазин, Сеть, Новости, Ещё on a half-circle, each a
/// see-through round button with a soft icon and a caption; in the middle of the arc a small
/// version badge.
#[allow(dead_code)]
fn game_dock(ui: &mut Ui, icons: &GuiIcons, p: &Palette, rect: Rect, cx: f32, arc_r: f32, version: &str) -> Option<MenuAction> {
    const ITEMS: [(u16, &str); 5] = [(icon::BRUSH, "Скины"), (icon::BACKPACK, "Магазин"), (icon::GLOBE, "Сеть"), (icon::STAR, "Новости"), (icon::PROPS, "Ещё")];
    const BTN: f32 = 27.0;
    let c = Pos2::new(cx, rect.bottom() - 30.0);
    let painter = ui.painter().clone();
    // The arc itself: a faint line through the buttons.
    let n = 48;
    let pts: Vec<Pos2> = (0..=n).map(|k| {
        let a = std::f32::consts::PI * (1.0 + k as f32 / n as f32);
        c + Vec2::new(a.cos(), a.sin()) * arc_r
    }).collect();
    painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, Color32::from_white_alpha(28))));
    // Version badge in the middle.
    let short: String = version.split('.').take(2).collect::<Vec<_>>().join(".");
    painter.circle_filled(c, 26.0, Color32::from_white_alpha(14));
    painter.circle_stroke(c, 26.0, Stroke::new(1.0_f32, Color32::from_white_alpha(60)));
    painter.text(c - Vec2::new(0.0, 4.0), Align2::CENTER_CENTER, format!("v{short}"), bold_font(14.0), Color32::from_white_alpha(220));
    painter.text(c + Vec2::new(0.0, 10.0), Align2::CENTER_CENTER, "версия", FontId::proportional(9.0), Color32::from_white_alpha(130));
    let sheet_id = egui::Id::new("game-menu-sheet");
    let open: Option<Sheet> = ui.ctx().data(|d| d.get_temp(sheet_id)).flatten();
    let mut out = None;
    for (i, (ic, label)) in ITEMS.iter().enumerate() {
        let a = std::f32::consts::PI * (1.0 + (i as f32 + 0.5) / ITEMS.len() as f32);
        let at = c + Vec2::new(a.cos(), a.sin()) * arc_r;
        // Button and caption together are the tap area.
        let r = Rect::from_min_max(at - Vec2::new(BTN + 6.0, BTN), at + Vec2::new(BTN + 6.0, BTN + 22.0));
        let resp = ui.interact(r, egui::Id::new(("game-dock", i)), Sense::click());
        let on = (i == 3 && open == Some(Sheet::News)) || (i == 4 && open == Some(Sheet::More));
        let pressed = resp.is_pointer_button_down_on();
        let fill = if on { p.accent.gamma_multiply(0.35) } else if pressed || resp.hovered() { Color32::from_white_alpha(40) } else { Color32::from_white_alpha(16) };
        ui.painter().circle_filled(at, BTN, fill);
        ui.painter().circle_stroke(at, BTN, Stroke::new(1.2_f32, if on { p.accent } else { Color32::from_white_alpha(70) }));
        let col = if on { p.accent } else { Color32::from_white_alpha(200) };
        icons.paint(ui.painter(), Rect::from_center_size(at, Vec2::splat(24.0)), *ic, col);
        ui.painter().text(Pos2::new(at.x, at.y + BTN + 11.0), Align2::CENTER_CENTER, *label, FontId::proportional(11.5), if on { p.accent } else { Color32::from_white_alpha(170) });
        if resp.clicked() {
            match i {
                0 => out = Some(open_settings_at(ui.ctx(), 0)),
                1 => out = Some(open_settings_at(ui.ctx(), 2)),
                2 => out = Some(MenuAction::OpenNetwork),
                _ => {
                    let want = if i == 3 { Sheet::News } else { Sheet::More };
                    let next = if open == Some(want) { None } else { Some(want) };
                    ui.ctx().data_mut(|d| d.insert_temp(sheet_id, next));
                }
            }
        }
    }
    out
}

/// «Новости» and «Ещё»: a card above the dock.
fn game_menu_sheet(ctx: &egui::Context, icons: &GuiIcons, p: &Palette) -> Option<MenuAction> {
    let sheet_id = egui::Id::new("game-menu-sheet");
    let open: Sheet = ctx.data(|d| d.get_temp::<Option<Sheet>>(sheet_id)).flatten()?;
    let screen = ctx.screen_rect();
    let w = (screen.width() - 32.0).min(420.0);
    let mut out = None;
    let mut close = false;
    egui::Area::new(egui::Id::new("game-menu-sheet-area")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -ctx.data(|d| d.get_temp::<f32>(egui::Id::new("game-dock-h"))).unwrap_or(96.0) - 8.0)).show(ctx, |ui| {
        Frame::none().fill(p.surface).rounding(Rounding::same(20.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
            ui.set_width(w - 32.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(if open == Sheet::News { "Что нового" } else { "Ещё" }).font(bold_font(18.0)).color(p.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if square_button(ui, icons, icon::CLOSE, false).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(6.0);
            match open {
                Sheet::News => {
                    egui::ScrollArea::vertical().max_height((screen.height() * 0.45).max(120.0)).show(ui, |ui| {
                        for (title, text) in NEWS {
                            ui.label(egui::RichText::new(title).font(bold_font(14.5)).color(p.text));
                            ui.label(egui::RichText::new(text).size(13.0).color(p.text_muted));
                            ui.add_space(6.0);
                        }
                    });
                }
                Sheet::More => {
                    let bw = ui.available_width();
                    for (ic, label, tab) in [(icon::REFRESH, "Обновления", 6usize), (icon::CROSSHAIR, "Управление", 5), (icon::EDIT, "Интерфейс", 4), (icon::SUN, "Графика", 1), (icon::HAMMER, "Рука и вид", 3)] {
                        if pill_button(ui, icons, Some(ic), label, ButtonKind::Secondary, bw, true).clicked() {
                            out = Some(open_settings_at(ctx, tab));
                            close = true;
                        }
                    }
                    if pill_button(ui, icons, Some(icon::ARROW_LEFT), "В лаунчер", ButtonKind::Ghost, bw, true).clicked() {
                        out = Some(MenuAction::Back);
                        close = true;
                    }
                }
            }
        });
    });
    if close {
        ctx.data_mut(|d| d.insert_temp::<Option<Sheet>>(sheet_id, None));
    }
    out
}

/// «Новости» in the game menu: what came in recent updates.
const NEWS: [(&str, &str); 6] = [
    ("Оружие", "Пистолет, автомат, дробовик и снайперская винтовка с 3D-моделями, прицелами и отдачей. Патроны и дробь — в «Вооружении»."),
    ("Разрушаемость", "Постройки без опоры рушатся, блоки падают и разбиваются, взрывы раскидывают обломки."),
    ("Игра по сети", "Короткие коды входа: введите код сервера во вкладке «По коду»."),
    ("WolfStudio", "Свои серверы и мини-игры: редактор, свободная камера, «Играть», скрипты блоков на Luau."),
    ("Мини-игры", "«Питомник» и «Симулятор кнопок» — шаблоны для серверов в Тулбоксе студии."),
    ("Взрывчатка", "Динамит с зажигалкой, гранаты, гранатомёт и базука."),
];

/// Home screen: the mobile launcher (toolbar, pages, bottom navigation).
pub fn main_menu(ctx: &egui::Context, icons: &GuiIcons, version: &str, data_dir: &str, dark: bool, planet: Option<&TextureHandle>, skin: u8, nickname: &str, worlds: &[WorldEntry]) -> MenuAction {
    let _ = data_dir;
    crate::launcher::show(ctx, icons, version, dark, planet, skin, nickname, worlds)
}

// ---------------------------------------------------------------------------------------------
// World selection

#[derive(Default)]
pub struct WorldSelectState {
    /// 0 = single player (saved worlds), 1 = play over the network.
    pub mode: usize,
    /// Address typed for a network game.
    pub address: String,
    /// Header tabs in "Игра по сети": 0 nearby (found on the local network), 1 host a
    /// server, 2 by code or address, 3 saved servers.
    pub net_tab: usize,
    /// Servers heard on the local network (Wi-Fi, hotspot): (name, address, players, max).
    pub nearby: Vec<(String, String, usize, usize)>,
    /// Saved servers (name, address); `servers_changed` tells the app to save them.
    pub servers: Vec<(String, String)>,
    pub servers_changed: bool,
    pub new_server_name: String,
    pub new_server_addr: String,
    /// Why the last connection attempt failed.
    pub net_error: Option<String>,
    /// «Вставить» pressed: the app puts the clipboard into `address` (phones cannot paste
    /// into the game's own text fields).
    pub paste_request: bool,
    pub search: String,
    /// Search field shown in the header (opened with the magnifier button).
    pub search_open: bool,
    /// Focus the search field on the next frame (just opened).
    pub search_focus: bool,
    /// Sort menu under the header sort button.
    pub sort_open: bool,
    pub sort: usize,
    pub selected: Option<String>,
    pub rename: Option<(String, String)>,
    pub confirm_delete: Option<String>,
}

/// "Игра по сети": content of the tab chosen in the right sidebar.
fn online_tab(ui: &mut egui::Ui, icons: &GuiIcons, p: &crate::theme_manager::Palette, st: &mut WorldSelectState, worlds: &[WorldEntry]) -> Option<MenuAction> {
    let mut act = None;
    if let Some(e) = &st.net_error {
        ui.label(egui::RichText::new(e).size(14.0).color(p.danger));
        ui.add_space(8.0);
    }
    match st.net_tab {
        0 => {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Рядом").font(bold_font(18.0)).color(p.text));
                // Little radar: the list refreshes by itself.
                let t = ui.ctx().input(|i| i.time) as f32;
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                ui.painter().circle_stroke(r.center(), 4.0 + (t * 1.5).fract() * 7.0, Stroke::new(1.5_f32, p.accent.gamma_multiply(1.0 - (t * 1.5).fract())));
                ui.painter().circle_filled(r.center(), 3.0, p.accent);
                ui.label(egui::RichText::new("ищем игры в вашей сети Wi-Fi и точке доступа").size(13.0).color(p.text_muted));
            });
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().id_salt("nearby").max_height(ui.available_height() - 10.0).show(ui, |ui| {
                for (name, addr, players, max) in &st.nearby {
                    // A tap anywhere on the row joins too (not only on «Играть»).
                    let row = row_card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            icons.paint(ui.painter(), r, icon::GLOBE, p.success);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(name).font(bold_font(16.0)).color(p.text));
                                ui.label(egui::RichText::new(format!("игроков {players}/{max} · {addr}")).size(12.5).color(p.text_muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let b = pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 0.0, players < max);
                                // Where the button is and what it saw (read by the tap test).
                                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("dbg_play"), (b.rect, b.hovered(), b.contains_pointer(), b.is_pointer_button_down_on(), b.enabled())));
                                if b.clicked() {
                                    st.net_error = None;
                                    act = Some(MenuAction::JoinServer(addr.clone()));
                                }
                            });
                        });
                    });
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("dbg_row"), (row.rect, row.hovered(), row.contains_pointer(), row.is_pointer_button_down_on(), row.sense.click)));
                    if row.clicked() && players < max && act.is_none() {
                        st.net_error = None;
                        act = Some(MenuAction::JoinServer(addr.clone()));
                    }
                    ui.add_space(6.0);
                }
                if st.nearby.is_empty() {
                    ui.label(egui::RichText::new("Пока никого. Сервер появится здесь сам, как только друг его откроет.").size(13.5).color(p.text_muted));
                }
                ui.add_space(10.0);
                for (title, text) in [
                    ("Одна сеть Wi-Fi", "Один игрок открывает мир во вкладке «Создать сервер» — у остальных он сразу появляется в этом списке."),
                    ("Только мобильный интернет", "Операторы не пропускают входящие подключения, поэтому напрямую через мобильную сеть сыграть нельзя. Включите точку доступа на телефоне с сервером и подключите к ней друзей: сервер появится здесь, а интернет на игру не тратится."),
                    ("Код приглашения", "Сервер показывает короткий код вида K7QX2 (в чате, в WolfStudio и в WolfServer). Введите его во вкладке «По коду»."),
                ] {
                    row_card(ui, false, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new(title).font(bold_font(15.0)).color(p.text));
                        ui.label(egui::RichText::new(text).size(13.0).color(p.text_muted));
                    });
                    ui.add_space(6.0);
                }
            });
        }
        3 => {
            ui.label(egui::RichText::new("Мои серверы").font(bold_font(18.0)).color(p.text));
            ui.add_space(6.0);
            let mut remove = None;
            egui::ScrollArea::vertical().id_salt("servers").max_height(ui.available_height() - 70.0).show(ui, |ui| {
                if st.servers.is_empty() {
                    ui.label(egui::RichText::new("Список пуст. Добавьте сервер ниже: имя и адрес (например 192.168.0.10:25570).").size(13.5).color(p.text_muted));
                }
                for (i, (name, addr)) in st.servers.iter().enumerate() {
                    let mut trash = Rect::NOTHING;
                    let row = row_card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            icons.paint(ui.painter(), r, icon::GLOBE, p.accent);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(name).font(bold_font(16.0)).color(p.text));
                                ui.label(egui::RichText::new(addr).size(12.5).color(p.text_muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let t = icon_button(ui, icons, icon::TRASH, 34.0, ButtonKind::Danger);
                                trash = t.rect;
                                if t.clicked() {
                                    remove = Some(i);
                                }
                                if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 0.0, true).clicked() {
                                    act = Some(MenuAction::JoinServer(addr.clone()));
                                }
                            });
                        });
                    });
                    if row.clicked() && act.is_none() && remove.is_none() {
                        let on_trash = row.interact_pointer_pos().is_some_and(|p| trash.expand(4.0).contains(p));
                        if on_trash {
                            remove = Some(i);
                        } else {
                            act = Some(MenuAction::JoinServer(addr.clone()));
                        }
                    }
                    ui.add_space(6.0);
                }
            });
            if let Some(i) = remove {
                st.servers.remove(i);
                st.servers_changed = true;
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                text_input(ui, icons, &mut st.new_server_name, "Имя", 150.0, Some(icon::EDIT));
                text_input(ui, icons, &mut st.new_server_addr, "Адрес:порт", 220.0, Some(icon::GLOBE));
                let ok = !st.new_server_addr.trim().is_empty();
                if pill_button(ui, icons, Some(icon::PLUS), "Добавить", ButtonKind::Secondary, 0.0, ok).clicked() {
                    let name = if st.new_server_name.trim().is_empty() { st.new_server_addr.trim().to_string() } else { st.new_server_name.trim().to_string() };
                    st.servers.push((name, st.new_server_addr.trim().to_string()));
                    st.new_server_name.clear();
                    st.new_server_addr.clear();
                    st.servers_changed = true;
                }
            });
        }
        1 => {
            ui.label(egui::RichText::new("Создать сервер").font(bold_font(18.0)).color(p.text));
            ui.label(egui::RichText::new("Выберите мир — телефон станет сервером. Друзья в той же сети Wi-Fi или подключённые к вашей точке доступа увидят его во вкладке «Рядом»; код приглашения появится в чате.").size(13.0).color(p.text_muted));
            ui.add_space(8.0);
            egui::ScrollArea::vertical().id_salt("private-worlds").max_height(ui.available_height() - 60.0).show(ui, |ui| {
                for w in worlds {
                    row_card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
                            ui.painter().rect_filled(r, Rounding::same(10.0), Color32::from_rgb(8, 12, 22));
                            if let Some(tex) = &w.preview {
                                ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                            }
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&w.cfg.name).font(bold_font(16.0)).color(p.text));
                                ui.label(egui::RichText::new(format!("{} · {}", if w.cfg.mode == GameMode::Creative { "Творческий" } else { "Выживание" }, w.cfg.size.label())).size(12.5).color(p.text_muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if pill_button(ui, icons, Some(icon::GLOBE), "Открыть для друзей", ButtonKind::Primary, 0.0, true).clicked() {
                                    st.net_error = None;
                                    act = Some(MenuAction::HostServer(Some(w.cfg.id.clone())));
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                }
                if worlds.is_empty() {
                    ui.label(egui::RichText::new("У вас пока нет миров.").color(p.text_muted));
                }
            });
            ui.add_space(6.0);
            if pill_button(ui, icons, Some(icon::PLUS), "Новый сетевой мир", ButtonKind::Secondary, 0.0, true).clicked() {
                st.net_error = None;
                act = Some(MenuAction::HostServer(None));
            }
        }
        _ => {
            ui.label(egui::RichText::new("По коду или адресу").font(bold_font(18.0)).color(p.text));
            ui.add_space(6.0);
            let full = ui.available_width().min(520.0);
            text_input(ui, icons, &mut st.address, "Код K7QX2 или адрес 192.168.0.10", full, Some(icon::GLOBE));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let half = (full - 8.0) / 2.0;
                if pill_button(ui, icons, Some(icon::PASTE), "Вставить", ButtonKind::Secondary, half, true).clicked() {
                    st.paste_request = true;
                }
                let ready = !st.address.trim().is_empty();
                if pill_button(ui, icons, Some(icon::PLAY), "Подключиться", ButtonKind::Primary, half, ready).clicked() {
                    st.net_error = None;
                    act = Some(MenuAction::JoinServer(st.address.trim().to_string()));
                }
            });

            ui.add_space(14.0);
            row_card(ui, false, |ui| {
                ui.label(egui::RichText::new("Как играть вместе").font(bold_font(16.0)).color(p.text));
                ui.label(egui::RichText::new("Один игрок открывает мир («Создать сервер») — в чате появятся код приглашения и адрес. Остальные вводят код здесь или просто выбирают сервер во вкладке «Рядом». Вы окажетесь в одном мире: видите друг друга, общие постройки и чат.").size(13.5).color(p.text_muted));
            });
        }
    }
    act
}

pub fn sorted_filtered<'a>(worlds: &'a [WorldEntry], search: &str, sort: usize) -> Vec<&'a WorldEntry> {
    let q = search.trim().to_lowercase();
    // Studio projects live in the Studio, not among the game worlds.
    let mut v: Vec<&WorldEntry> = worlds.iter().filter(|w| !w.cfg.studio_project && (q.is_empty() || w.cfg.name.to_lowercase().contains(&q))).collect();
    match sort {
        1 => v.sort_by(|a, b| a.cfg.name.to_lowercase().cmp(&b.cfg.name.to_lowercase())),
        2 => v.sort_by(|a, b| b.cfg.created.cmp(&a.cfg.created)),
        _ => v.sort_by(|a, b| b.cfg.last_played.cmp(&a.cfg.last_played)),
    }
    v
}

const SORT_NAMES: [&str; 3] = ["Недавние", "По названию", "По созданию"];

/// Square outlined icon button for the header; `active` paints it in the accent colour.
fn square_button(ui: &mut Ui, icons: &GuiIcons, ic: u16, active: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
    let (fill, stroke, fg) = if active {
        (p.accent, p.accent, p.on_accent)
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        (p.surface_hover, p.accent, p.text)
    } else {
        (p.surface_hi, p.outline, p.text)
    };
    ui.painter().rect_filled(rect, Rounding::same(10.0), fill);
    ui.painter().rect_stroke(rect, Rounding::same(10.0), Stroke::new(1.0_f32, stroke));
    icons.paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(22.0)), ic, fg);
    resp
}

pub fn world_select(ctx: &egui::Context, icons: &GuiIcons, st: &mut WorldSelectState, worlds: &[WorldEntry], _version: &str) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
        egui::TopBottomPanel::top("worlds-phone-header")
            .frame(menu_header_frame(&p))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if square_button(ui, icons, icon::ARROW_LEFT, false).clicked() { action = MenuAction::Back; }
                    ui.label(egui::RichText::new("Миры").font(bold_font(21.0)).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if square_button(ui, icons, icon::GEAR, false).clicked() { action = MenuAction::OpenSettings; }
                    });
                });
                ui.add_space(8.0);
                menu_tabs(ui, "world-mode-tabs", &["Игра", "Сеть"], &mut st.mode);
            });
    {
        // Shared layout: sections one under another in a centred column — «Одиночная» (create, search,
        // sort and the worlds) and «По сети» (nearby, create a server, by code, my servers).
        egui::CentralPanel::default().frame(Frame::none().fill(p.bg).inner_margin(Margin::symmetric(12.0, 12.0))).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("worlds-phone-body").auto_shrink([false, false]).show(ui, |ui| {
                let full = ui.available_width();
                let col_w = full.min(620.0);
                ui.horizontal(|ui| {
                    ui.add_space(((full - col_w) / 2.0).max(0.0));
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        let section = |ui: &mut Ui, _ic: u16, _title: &str, add: &mut dyn FnMut(&mut Ui)| {
                            menu_card_frame(&p, 14.0).show(ui, |ui| {
                                ui.set_width(col_w - 28.0);
                                add(ui);
                            });
                        };
                        if st.mode == 0 { section(ui, icon::PLAY, "Игра", &mut |ui| {
                            ui.horizontal(|ui| {
                                let w = (ui.available_width() - 100.0).max(100.0);
                                if pill_button(ui, icons, Some(icon::PLUS), "Создать мир", ButtonKind::Primary, w, true).clicked() { action = MenuAction::OpenCreate; }
                                if square_button(ui, icons, icon::SEARCH, st.search_open).clicked() {
                                    st.search_open = !st.search_open;
                                    st.search_focus = st.search_open;
                                    if !st.search_open { st.search.clear(); }
                                }
                                if square_button(ui, icons, icon::SORT, st.sort_open).clicked() { st.sort_open = !st.sort_open; }
                            });
                            if st.search_open {
                                let r = text_input(ui, icons, &mut st.search, "Название…", ui.available_width(), None);
                                if st.search_focus { st.search_focus = false; r.request_focus(); }
                            }
                            if st.sort_open {
                                ui.horizontal_wrapped(|ui| {
                                    for (i, label) in SORT_NAMES.iter().enumerate() {
                                        if pill_button(ui, icons, None, label, if st.sort == i { ButtonKind::Primary } else { ButtonKind::Secondary }, 0.0, true).clicked() {
                                            st.sort = i; st.sort_open = false;
                                        }
                                    }
                                });
                            }
                            ui.add_space(8.0);
                            let list = sorted_filtered(worlds, &st.search, st.sort);
                            if list.is_empty() {
                                let none = st.search.trim().is_empty() || !worlds.iter().any(|w| !w.cfg.studio_project);
                                ui.label(egui::RichText::new(if none { "Миров пока нет — нажмите «Создать мир»" } else { "Ничего не найдено" }).color(p.text_muted));
                            }
                            let inner = ui.available_width();
                            for w in &list {
                                let selected = st.selected.as_deref() == Some(w.cfg.id.as_str());
                                let resp = row_card(ui, selected, |ui| {
                                    ui.set_width(inner - 26.0);
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                                        if let Some(tex) = &w.preview {
                                            ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                        } else { ui.painter().rect_filled(r, Rounding::same(8.0), p.surface); }
                                        ui.vertical(|ui| {
                                            ui.add(egui::Label::new(egui::RichText::new(&w.cfg.name).font(bold_font(17.0)).color(p.text)).truncate());
                                            ui.label(egui::RichText::new(format!("{} · {}", if w.cfg.mode == GameMode::Creative { "Творческий" } else { "Выживание" }, w.cfg.size.label())).size(12.0).color(p.text_muted));
                                        });
                                    });
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        let play_w = (ui.available_width() - 100.0).max(100.0);
                                        if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, play_w, true).clicked() { action = MenuAction::PlayWorld(w.cfg.id.clone()); }
                                        if icon_button(ui, icons, icon::EDIT, 40.0, ButtonKind::Ghost).clicked() { st.rename = Some((w.cfg.id.clone(), w.cfg.name.clone())); }
                                        if icon_button(ui, icons, icon::TRASH, 40.0, ButtonKind::Danger).clicked() { st.confirm_delete = Some(w.cfg.id.clone()); }
                                    });
                                });
                                if resp.clicked() { st.selected = Some(w.cfg.id.clone()); }
                                ui.add_space(8.0);
                            }
                        });
                        }
                        if st.mode == 1 { section(ui, icon::GLOBE, "Сеть", &mut |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for (i, label) in [(0usize, "Рядом"), (1, "Создать сервер"), (2, "По коду"), (3, "Мои серверы")] {
                                    if pill_button(ui, icons, None, label, if st.net_tab == i { ButtonKind::Primary } else { ButtonKind::Secondary }, 0.0, true).clicked() { st.net_tab = i; }
                                }
                            });
                            ui.add_space(8.0);
                            if let Some(a) = online_tab(ui, icons, &p, st, worlds) {
                                action = a;
                            }
                        });
                        }
                    });
                });
            });
        });
    }
    if let Some((id, name)) = &mut st.rename {
        let mut close = false;
        let mut apply = false;
        let dismissed = modal(ctx, "rename-world", "Переименовать мир", 380.0, |ui| {
            text_input(ui, icons, name, "Название мира", 360.0, Some(icon::EDIT));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if pill_button(ui, icons, Some(icon::CHECK), "Сохранить", ButtonKind::Primary, 0.0, !name.trim().is_empty()).clicked() {
                    apply = true;
                }
                if pill_button(ui, icons, Some(icon::CLOSE), "Отмена", ButtonKind::Secondary, 0.0, true).clicked() {
                    close = true;
                }
            });
        });
        if apply {
            action = MenuAction::RenameWorld(id.clone(), name.trim().to_string());
        }
        if apply || close || dismissed {
            st.rename = None;
        }
    }
    if let Some(id) = st.confirm_delete.clone() {
        let name = worlds.iter().find(|w| w.cfg.id == id).map(|w| w.cfg.name.clone()).unwrap_or_default();
        let mut close = false;
        let dismissed = modal(ctx, "delete-world", "Удалить мир?", 400.0, |ui| {
            ui.label(egui::RichText::new(format!("«{name}» будет перемещён в корзину Windows. Его можно восстановить оттуда.")).color(p.text));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if pill_button(ui, icons, Some(icon::TRASH), "Удалить", ButtonKind::Danger, 0.0, true).clicked() {
                    action = MenuAction::DeleteWorld(id.clone());
                    close = true;
                }
                if pill_button(ui, icons, Some(icon::CLOSE), "Отмена", ButtonKind::Secondary, 0.0, true).clicked() {
                    close = true;
                }
            });
        });
        if close || dismissed {
            st.confirm_delete = None;
            if st.selected.as_deref() == Some(id.as_str()) {
                st.selected = None;
            }
        }
    }
    action
}

// ---------------------------------------------------------------------------------------------
// Create world

pub struct CreateWorldState {
    pub name: String,
    pub seed_text: String,
    pub mode: GameMode,
    pub size: PlanetSize,
    pub world_type: WorldType,
    pub shape: PlanetShape,
    pub continents: ContinentCount,
    pub structures: bool,
    pub difficulty: Difficulty,
    pub error: Option<String>,
    /// Set whenever a setting that changes the planet changed; the owner re-renders the preview.
    pub preview_dirty: bool,
    pub preview_seed: u64,
    /// Selected section tab (0 = Основное, 1 = Планета).
    pub tab: usize,
}

impl Default for CreateWorldState {
    fn default() -> Self {
        Self {
            name: "Новый мир".into(),
            seed_text: String::new(),
            mode: GameMode::Survival,
            size: PlanetSize::Medium,
            world_type: WorldType::Standard,
            shape: PlanetShape::Cube,
            continents: ContinentCount::Auto,
            structures: true,
            difficulty: Difficulty::Normal,
            error: None,
            preview_dirty: true,
            preview_seed: 0,
            tab: 0,
        }
    }
}

impl CreateWorldState {
    /// Builds the config; `random_seed` is used when the seed field is empty.
    pub fn to_config(&self, random_seed: u64) -> Result<WorldConfig, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Введите название мира".into());
        }
        if name.chars().count() > 40 {
            return Err("Название слишком длинное (максимум 40 символов)".into());
        }
        let seed = parse_seed(&self.seed_text, random_seed);
        let mut cfg = WorldConfig::new(name, seed);
        cfg.mode = self.mode;
        cfg.size = self.size;
        cfg.world_type = self.world_type;
        cfg.shape = self.shape;
        cfg.continents = self.continents;
        cfg.structures = self.structures;
        cfg.difficulty = self.difficulty;
        Ok(cfg)
    }

    /// The seed currently shown in the preview (explicit seed or the preview's random one).
    pub fn effective_seed(&self) -> u64 {
        parse_seed(&self.seed_text, self.preview_seed)
    }
}

pub fn create_world(ctx: &egui::Context, icons: &GuiIcons, st: &mut CreateWorldState, preview: Option<&TextureHandle>, version: &str, random_seed: u64) -> MenuAction {
    create_world_phone(ctx, icons, st, preview, version, random_seed)
}


fn create_world_form(ui: &mut Ui, icons: &GuiIcons, st: &mut CreateWorldState, random_seed: u64) {
            ui.spacing_mut().item_spacing.y = 6.0;
            if st.tab == 0 {
                stacked_row(ui, "Название мира", None, |ui| {
                    text_input(ui, icons, &mut st.name, "Новый мир", ui.available_width().min(300.0), Some(icon::EDIT));
                });
                ui.add_space(10.0);
                stacked_row(ui, "Seed", Some("Пусто — случайный. Можно ввести число или текст"), |ui| {
                    if text_input(ui, icons, &mut st.seed_text, "Случайный", (ui.available_width() - 48.0).clamp(80.0, 250.0), Some(icon::GLOBE)).changed() {
                        st.preview_dirty = true;
                    }
                    if square_button(ui, icons, icon::REFRESH, false).clicked() {
                        let t = (ui.ctx().input(|i| i.time) * 1.0e6) as u64;
                        st.seed_text = (wolfcraft_engine::planet::splitmix(random_seed ^ t) % 1_000_000_000).to_string();
                        st.preview_dirty = true;
                    }
                });
                ui.add_space(10.0);
                stacked_row(ui, "Режим", None, |ui| {
                    segmented(ui, "cw-mode", &[(GameMode::Survival, "Выживание"), (GameMode::Creative, "Творческий")], &mut st.mode);
                });
                ui.add_space(10.0);
                stacked_row(ui, "Сложность", None, |ui| {
                    let opts: Vec<(Difficulty, &str)> = Difficulty::ALL.iter().map(|d| (*d, d.label())).collect();
                    segmented(ui, "cw-diff", &opts, &mut st.difficulty);
                });
            } else {
                stacked_row(ui, "Форма планеты", Some("Куб — плоские грани со скруглёнными рёбрами"), |ui| {
                    let opts: Vec<(PlanetShape, &str)> = PlanetShape::ALL.iter().map(|d| (*d, d.label())).collect();
                    if segmented(ui, "cw-shape", &opts, &mut st.shape) {
                        st.preview_dirty = true;
                    }
                });
                ui.add_space(10.0);
                stacked_row(ui, "Размер планеты", Some(&format!("Окружность ≈ {} блоков", st.size.circumference())), |ui| {
                    let opts: Vec<(PlanetSize, &str)> = PlanetSize::ALL.iter().map(|d| (*d, d.label())).collect();
                    if segmented(ui, "cw-size", &opts, &mut st.size) {
                        st.preview_dirty = true;
                    }
                });
                ui.add_space(10.0);
                stacked_row(ui, "Тип мира", None, |ui| {
                    let opts: Vec<(WorldType, &str)> = WorldType::ALL.iter().map(|d| (*d, d.label())).collect();
                    if segmented(ui, "cw-type", &opts, &mut st.world_type) {
                        st.preview_dirty = true;
                    }
                });
                ui.add_space(10.0);
                let uses = st.world_type == WorldType::Standard;
                stacked_row(ui, "Количество материков", Some(if uses { "Центры материков выбираются по seed" } else { "Не используется для этого типа мира" }), |ui| {
                    ui.add_enabled_ui(uses, |ui| {
                        let opts: Vec<(ContinentCount, &str)> = ContinentCount::ALL.iter().map(|d| (*d, d.label())).collect();
                        if segmented(ui, "cw-cont", &opts, &mut st.continents) {
                            st.preview_dirty = true;
                        }
                    });
                });
                ui.add_space(10.0);
                stacked_row(ui, "Генерация структур", Some("Деревни, руины, храмы, крепости, шахты"), |ui| {
                    toggle_switch(ui, &mut st.structures);
                });
            }

}

fn create_world_phone(ctx: &egui::Context, icons: &GuiIcons, st: &mut CreateWorldState, preview: Option<&TextureHandle>, version: &str, random_seed: u64) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    egui::TopBottomPanel::top("create-phone-header")
        .frame(menu_header_frame(&p))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if square_button(ui, icons, icon::ARROW_LEFT, false).clicked() { action=MenuAction::Back; }
                ui.label(egui::RichText::new("Создать мир").font(bold_font(21.0)).color(p.text));
                ui.label(egui::RichText::new(format!("v{version}")).size(11.0).color(p.text_muted));
            });
            ui.add_space(8.0);
            menu_tabs(ui, "create-sections", &["Основное", "Планета"], &mut st.tab);
        });
    egui::TopBottomPanel::bottom("create-phone-actions")
        .frame(menu_header_frame(&p))
        .show(ctx, |ui| {
            if let Some(error) = &st.error { ui.label(egui::RichText::new(error).color(p.warn)); }
            ui.horizontal(|ui| {
                let create_w = (ui.available_width() - 108.0).max(140.0);
                if pill_button(ui, icons, Some(icon::ARROW_LEFT), "Назад", ButtonKind::Secondary, 100.0, true).clicked() { action = MenuAction::Back; }
                if pill_button(ui, icons, Some(icon::CUBE), "Создать мир", ButtonKind::Primary, create_w, true).clicked() {
                    match st.to_config(random_seed) {
                        Ok(cfg) => { st.error = None; action = MenuAction::CreateWorld(Box::new(cfg)); }
                        Err(error) => st.error = Some(error),
                    }
                }
            });
        });
    egui::CentralPanel::default().frame(Frame::none().fill(p.bg).inner_margin(Margin::same(16.0))).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt("create-phone-body").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let globe = ui.available_width().min(200.0);
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(globe), Sense::hover());
                ui.painter().rect_filled(r, Rounding::same(12.0), Color32::from_rgb(6, 9, 18));
                if let Some(tex) = preview { ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
                else { ui.painter().text(r.center(), Align2::CENTER_CENTER, "Генерация…", FontId::proportional(15.0), p.text_muted); }
                ui.label(egui::RichText::new(format!("Seed: {}", st.effective_seed() as i64)).size(12.0).color(p.text_muted));
            });
            ui.add_space(12.0);
            create_world_form(ui, icons, st, random_seed);
        });
    });
    action
}

// ---------------------------------------------------------------------------------------------
// Settings

#[derive(Default, Debug, PartialEq, Eq)]
pub struct SettingsResult {
    pub changed: bool,
    pub back: bool,
    pub open_updates: bool,
    /// Re-scan the resource pack folders.
    pub refresh_packs: bool,
    /// Download a resource pack (.zip) from this link.
    pub download: Option<String>,
}

/// What the "Текстуры" tab shows.
pub struct PacksView<'a> {
    pub list: &'a [wolfcraft_assets::packs::PackInfo],
    /// Download / scan status line.
    pub status: &'a str,
    /// Folder where the player can drop packs.
    pub folder: &'a str,
}

const FPS_LIMITS: [u32; 7] = [0, 30, 60, 90, 120, 144, 240];

const SETTINGS_TABS: [(u16, &str); 7] = [(icon::HEART, "Профиль"), (icon::SUN, "Графика"), (icon::CUBE, "Текстуры"), (icon::HAMMER, "Рука"), (icon::EDIT, "Интерфейс"), (icon::CROSSHAIR, "Управление"), (icon::REFRESH, "Обновления")];

/// Resource packs: built-in ones and the player's own, plus import from a link.
fn packs_tab(ui: &mut Ui, icons: &GuiIcons, p: &Palette, s: &mut Settings, packs: &PacksView, res: &mut SettingsResult) {
    // Shaders: a preset, then every effect on its own (changing one switches to "Свои").
    setting_row(ui, "Шейдеры", Some("Лёгкие: рельеф, ветер, вода. Ультра: ещё глубина швов (параллакс). Мульт: мягкий яркий свет и сочные цвета, как в топовых симуляторах. RTX: всё сразу и настоящие тени от солнца (для мощных устройств)"), |ui| {
        let mut preset = s.shaders;
        if segmented(ui, "shaders", &[(0u8, "Выкл"), (1, "Лёгкие"), (2, "Ультра"), (5, "RTX"), (4, "Мульт"), (3, "Свои")], &mut preset) {
            if preset != 3 {
                s.apply_shader_preset(preset);
            } else {
                s.shaders = 3;
            }
            res.changed = true;
        }
    });
    ui.add_space(8.0);
    let before = (s.relief_mode, s.relief_strength, s.fx_wind, s.fx_water, s.fx_grading, s.fx_toon);
    setting_row(ui, "Рельеф текстур", Some("Объём: свет и тень на каждом камне. Глубина: швы и щели уходят внутрь блока. В HD-паках подробнее"), |ui| {
        res.changed |= segmented(ui, "relief", &[(0u8, "Нет"), (1, "Объём"), (2, "Глубина")], &mut s.relief_mode);
    });
    if s.relief_mode > 0 {
        ui.add_space(6.0);
        setting_row(ui, "Сила рельефа", None, |ui| {
            res.changed |= ui.add(egui::Slider::new(&mut s.relief_strength, 0.3..=2.0).fixed_decimals(1)).changed();
        });
    }
    ui.add_space(6.0);
    setting_row(ui, "Ветер", Some("Качаются листва и трава"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_wind).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Вода", Some("Волны, блики солнца и отражение неба"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_water).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Цветокоррекция", Some("Контраст и сочные цвета"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_grading).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Мультяшный свет", Some("Мягкие светлые тени, яркие краски, контур-подсветка мобов"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_toon).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Быстрая листва", Some("Кроны без просветов — сильно быстрее на слабых телефонах"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fast_leaves).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Свечение", Some("Как в шейдерах на ПК: лава, факелы и солнце светятся ореолом (чуть медленнее)"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_bloom).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Лучи солнца", Some("Солнечные лучи пробиваются сквозь листву и края гор (чуть медленнее)"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_rays).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Мокрые блоки", Some("В дождь блоки под небом темнеют и блестят"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_wet).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Тени от солнца", Some("Как RTX: каждый блок, дерево и дом отбрасывают настоящую тень (заметно медленнее)"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_shadows).changed();
    });
    if (s.relief_mode, s.relief_strength, s.fx_wind, s.fx_water, s.fx_grading, s.fx_toon) != before {
        s.shaders = 3;
    }
    ui.add_space(10.0);
    for pack in packs.list {
        let sel = s.texture_pack == pack.id;
        let resp = row_card(ui, sel, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                icons.paint(ui.painter(), r, if sel { icon::CHECK } else if pack.path.is_some() { icon::FOLDER } else { icon::CUBE }, if sel { p.accent } else { p.text_muted });
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&pack.name).font(bold_font(15.0)).color(if sel { p.accent } else { p.text }));
                    if !pack.description.is_empty() {
                        ui.label(egui::RichText::new(&pack.description).size(12.5).color(p.text_muted));
                    }
                });
            });
        });
        if resp.clicked() && !sel {
            s.texture_pack = pack.id.clone();
            // The cartoon pack looks its best with the cartoon light.
            if wolfcraft_assets::packs::is_toon(&pack.id) && s.shaders != 4 {
                s.apply_shader_preset(4);
            }
            res.changed = true;
        }
        ui.add_space(6.0);
    }
    ui.add_space(8.0);
    section_label(ui, "Свой пак");
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Пак — папка или .zip с картинками blocks/, items/, entities/, gui/, particles/ (имя_спрайта.png, любой размер) и pack.json {\"name\": \"...\"}. Положите его в папку:").size(12.5).color(p.text_muted));
    ui.label(egui::RichText::new(packs.folder).size(12.5).color(p.text));
    ui.add_space(6.0);
    let url_id = egui::Id::new("pack-url");
    let mut url: String = ui.ctx().data(|d| d.get_temp(url_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        text_input(ui, icons, &mut url, "Ссылка на .zip", 300.0, Some(icon::GLOBE));
        if pill_button(ui, icons, Some(icon::DOWNLOAD), "Загрузить", ButtonKind::Primary, 0.0, !url.trim().is_empty()).clicked() {
            res.download = Some(url.trim().to_string());
        }
        if square_button(ui, icons, icon::REFRESH, false).on_hover_text("Обновить список").clicked() {
            res.refresh_packs = true;
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(url_id, url));
    if !packs.status.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(packs.status).size(13.0).color(p.text_muted));
    }
}

pub fn settings_screen(ctx: &egui::Context, icons: &GuiIcons, s: &mut Settings, version: &str, packs: &PacksView) -> SettingsResult {
    let p = palette(ctx);
    let mut res = SettingsResult::default();
    // Other screens may ask to open a section (e.g. «Скины» → Профиль): scroll to it once.
    let tab_id = egui::Id::new("settings-tab");
    let jump: Option<usize> = ctx.data_mut(|d| d.remove_temp::<usize>(tab_id)).map(|t| t.min(SETTINGS_TABS.len() - 1));
    let selected_id = egui::Id::new("settings-selected-section");
    let mut selected = jump.unwrap_or_else(|| ctx.data(|d| d.get_temp::<usize>(selected_id).unwrap_or(0))).min(SETTINGS_TABS.len()-1);
    egui::TopBottomPanel::top("settings-header")
        .frame(menu_header_frame(&p))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if square_button(ui, icons, icon::ARROW_LEFT, false).on_hover_text("Назад").clicked() {
                    res.back = true;
                }
                ui.label(egui::RichText::new("Настройки").font(bold_font(21.0)).color(p.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("v{version}")).size(12.0).color(p.text_muted));
                    if ui.available_width() > 270.0 {
                        ui.label(egui::RichText::new("Сохраняется автоматически").size(12.0).color(p.text_muted));
                    }
                });
            });
            ui.add_space(8.0);
            let names: Vec<&str> = SETTINGS_TABS.iter().map(|(_,name)| *name).collect();
            menu_tabs(ui, "settings-sections", &names, &mut selected);
        });
    // Show one selected section in a centred card beneath the shared tab bar.
    egui::CentralPanel::default().frame(Frame::none().fill(p.bg).inner_margin(Margin::symmetric(12.0, 14.0))).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt(("settings-body", selected)).auto_shrink([false, false]).show(ui, |ui| {
            let full = ui.available_width();
            let col_w = full.min(620.0);
            ui.horizontal(|ui| {
            ui.add_space(((full - col_w) / 2.0).max(0.0));
            ui.vertical(|ui| {
            ui.set_width(col_w);
            for tab in std::iter::once(selected) {
            let (sec_icon, sec_name) = SETTINGS_TABS[tab];
            let card = menu_card_frame(&p, 16.0).show(ui, |ui| {
            ui.set_width(col_w - 32.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                icons.paint(ui.painter(), r, sec_icon, p.accent);
                ui.label(egui::RichText::new(sec_name).font(bold_font(18.0)).color(p.text));
            });
            ui.add_space(4.0);
            let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0.0, p.outline);
            ui.add_space(10.0);
            match tab {
                6 => {
                    ui.label(format!("Установлена WolfCraft {version}"));
                    ui.label(if cfg!(target_os="android") { "Обновления устанавливаются внутри приложения. Если понадобится новый загрузчик, приложение сообщит об этом." } else { "Новая версия устанавливается поверх текущей. Не удаляйте папку миров перед обновлением." });
                    ui.label(if cfg!(target_os="android") { "Для закрытого репозитория ключ чтения вводится один раз в окне обновления." } else { "Приватный GitHub потребует входа в ваш аккаунт." });
                    if pill_button(ui, icons, Some(icon::REFRESH), if cfg!(target_os="android") {"Проверить обновления"} else {"Обновления на GitHub"}, ButtonKind::Primary, 250.0, true).clicked() {
                        res.open_updates = true;
                    }
                    ui.add_space(12.0);
                    ui.label("Текстуры обновляются отдельно: раздел «Текстуры» → загрузка ZIP-пака по ссылке.");
                }
                0 => {
                    setting_row(ui, "Никнейм", Some("Показывается в чате"), |ui| {
                        res.changed |= ui.add(egui::TextEdit::singleline(&mut s.nickname).desired_width(200.0).char_limit(16)).changed();
                    });
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Скин").font(FontId::proportional(15.0)).color(p.text));
                    ui.label(egui::RichText::new("Видно в виде от третьего лица").font(FontId::proportional(12.5)).color(p.text_muted));
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for (i, name) in wolfcraft_engine::renderer::model::SKIN_NAMES.iter().enumerate() {
                            let kind = if s.skin as usize == i { ButtonKind::Primary } else { ButtonKind::Secondary };
                            if pill_button(ui, icons, None, name, kind, 0.0, true).clicked() {
                                s.skin = i as u8;
                                res.changed = true;
                            }
                        }
                    });
                }
                1 => {
                    setting_row(ui, "Дальность прорисовки", Some("В чанках по 16 блоков. Больше — красивее, но тяжелее"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.render_distance, 3..=16).suffix(" чанк.")).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Разрешение", Some("Чем меньше, тем быстрее. 75% на телефоне почти не отличить, а FPS выше"), |ui| {
                        let mut pct = (s.render_scale * 100.0).round() as u8;
                        if segmented(ui, "render-scale", &[(50u8, "50%"), (67, "67%"), (75, "75%"), (100, "100%")], &mut pct) {
                            s.render_scale = pct as f32 / 100.0;
                            res.changed = true;
                        }
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Поле зрения", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.fov, 50.0..=110.0).suffix("°").integer()).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Частицы блоков", Some("0 — выключены"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.particles, 0.0..=2.0)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Вертикальная синхронизация", Some("Убирает разрывы изображения"), |ui| {
                        res.changed |= toggle_switch(ui, &mut s.vsync).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Ограничение FPS", Some("Работает без VSync"), |ui| {
                        let labels: Vec<String> = FPS_LIMITS.iter().map(|f| if *f == 0 { "Без ограничения".to_string() } else { format!("{f} FPS") }).collect();
                        let mut idx = FPS_LIMITS.iter().position(|f| *f == s.max_fps).unwrap_or(0);
                        if dropdown(ui, icons, "fps-limit", &labels, &mut idx, 200.0) {
                            s.max_fps = FPS_LIMITS[idx];
                            res.changed = true;
                        }
                    });
                    ui.add_space(8.0);
                    if !cfg!(target_os = "android") {
                        setting_row(ui, "Полноэкранный режим", None, |ui| {
                            res.changed |= toggle_switch(ui, &mut s.fullscreen).changed();
                        });
                        ui.add_space(8.0);
                    }
                    setting_row(ui, "Частота экрана", Some("Плавнее на экранах 90/120 Гц, но тратит больше батареи"), |ui| {
                        res.changed |= segmented(ui, "refresh", &[(60u32, "60 Гц"), (90, "90 Гц"), (120, "120 Гц")], &mut s.refresh_rate);
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Показывать FPS", None, |ui| {
                        res.changed |= toggle_switch(ui, &mut s.show_fps).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "«Что это?»", Some("Как мод WAILA: плашка сверху по центру — что за блок или существо перед вами, чем добыть, энергия машин, здоровье"), |ui| {
                        res.changed |= toggle_switch(ui, &mut s.show_look).changed();
                    });
                }
                2 => {
                    packs_tab(ui, icons, &p, s, packs, &mut res);
                }
                3 => {
                    setting_row(ui, "Поле зрения руки", Some("Отдельно от основной камеры"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.vm_fov, 40.0..=100.0).suffix("°").integer()).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Масштаб", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.vm_scale, 0.6..=1.6)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Сдвиг по горизонтали", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.vm_x, -0.3..=0.3)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Сдвиг по вертикали", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.vm_y, -0.3..=0.3)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Размах анимации", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.hand_anim, 0.4..=1.0)).changed();
                    });
                }
                4 => {
                    setting_row(ui, "Тема", None, |ui| {
                        res.changed |= segmented(ui, "theme", &[(ThemeMode::Dark, "Тёмная"), (ThemeMode::Light, "Светлая")], &mut s.theme);
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Масштаб интерфейса", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.ui_scale, 0.75..=1.75).fixed_decimals(2)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Громкость", Some("Звуки блоков, шагов и урона"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.volume, 0.0..=1.0)).changed();
                    });
                }
                _ => {
                    setting_row(ui, "Чувствительность", Some("Скорость поворота камеры"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.mouse_sensitivity, 0.2..=3.0).fixed_decimals(2)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Выбор касанием", Some("Ломать и ставить там, где палец, без прицела"), |ui| {
                        res.changed |= toggle_switch(ui, &mut s.tap_aim).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Плавность камеры", Some("0 — поворот точно за пальцем, 1 — мягко"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.camera_smooth, 0.0..=1.0).fixed_decimals(2)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Инверсия по вертикали", None, |ui| {
                        res.changed |= toggle_switch(ui, &mut s.invert_y).changed();
                    });
                }
            }
            });
            if jump == Some(tab) {
                card.response.scroll_to_me(Some(egui::Align::TOP));
            }
            ui.add_space(12.0);
            }
            });
            });
        });
    });
    ctx.data_mut(|d| d.insert_temp(selected_id, selected));
    res
}

// ---------------------------------------------------------------------------------------------
// Loading

pub struct LoadingView<'a> {
    pub title: &'a str,
    pub stages: &'a [String],
    /// Index of the stage currently running (== stages.len() when everything is done).
    pub current: usize,
    /// Progress of the whole operation in 0..1.
    pub fraction: f32,
    pub detail: &'a str,
    /// Chunk map around the player, row by row (0 none, 1 generated, 2 ready); empty = no map yet.
    pub grid: &'a [u8],
}

/// World loading, in the style of the studio splash: the same dark backdrop, an outlined
/// ring in the middle that fills with small outlined circles as the world gets ready, the
/// percentage in its centre, and the splash's loading bar with the current step below.
pub fn loading_screen(ctx:&egui::Context,icons:&GuiIcons,v:&LoadingView) {
    // A friendly line instead of the stage name and chunk counters.
    let f = v.fraction;
    let subtitle = if f < 0.2 { "Готовим планету…" } else if f < 0.4 { "Рисуем материки и океаны…" } else if f < 0.75 { "Строим землю вокруг вас…" } else { "Почти готово…" };
    let _ = (v.stages, v.current, v.detail, v.grid);
    let tip = TIPS[((ctx.input(|i| i.time) / 6.0) as usize) % TIPS.len()];
    let title = v.title.trim_start_matches("Мир ").trim_matches(|c| c == '«' || c == '»');
    loading_panel(ctx, icons, title, subtitle, f, tip, wolfcraft_common::VERSION, false);
}

// ---------------------------------------------------------------------------------------------
// Pause

/// What the pause card shows about the running game.
pub struct PauseInfo {
    pub world_name: String,
    pub mode: String,
    pub clock: String,
    pub biome: String,
    pub seed: i64,
    pub coords: [f64; 3],
    pub camera: String,
}

/// Centered pause screen shown on Escape: dims the world and presents one custom card.
pub fn pause_menu(ctx: &egui::Context, icons: &GuiIcons, info: &PauseInfo) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    let screen = ctx.screen_rect();
    // Scrim over the frozen world.
    ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("pause-scrim"))).rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(4, 8, 14, 168));
    let t = ctx.animate_bool_with_time(egui::Id::new("pause-in"), true, ANIM_TIME);
    egui::Area::new(egui::Id::new("pause-card")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::new(0.0, (1.0 - t) * 18.0)).show(ctx, |ui| {
        Frame::none()
            .fill(p.surface)
            .rounding(Rounding::same(10.0))
            .stroke(Stroke::new(1.0_f32, p.outline))
            .shadow(egui::Shadow { offset: Vec2::new(6.0, 6.0), blur: 0.0, spread: 0.0, color: Color32::from_black_alpha(150) })
            .inner_margin(Margin::same(22.0))
            .show(ui, |ui| {
                ui.set_width(380.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
                    icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Пауза").font(bold_font(28.0)).color(p.text));
                        ui.label(egui::RichText::new(&info.world_name).size(14.5).color(p.text_muted));
                    });
                });
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    badge(ui, &info.mode, p.accent);
                    badge(ui, &info.clock, p.secondary);
                    badge(ui, &info.biome, p.text_muted);
                });
                ui.add_space(12.0);
                divider(ui);
                ui.add_space(12.0);
                ui.spacing_mut().item_spacing.y = 9.0;
                ui.vertical_centered(|ui| {
                    if pill_button(ui, icons, Some(icon::PLAY), "Продолжить игру", ButtonKind::Primary, 380.0, true).clicked() {
                        action = MenuAction::Resume;
                    }
                    if pill_button(ui, icons, Some(icon::GEAR), "Настройки", ButtonKind::Secondary, 380.0, true).clicked() {
                        action = MenuAction::OpenSettings;
                    }
                    if pill_button(ui, icons, Some(icon::DOWNLOAD), "Сохранить игру", ButtonKind::Secondary, 380.0, true).clicked() {
                        action = MenuAction::SaveGame;
                    }
                    if pill_button(ui, icons, Some(icon::ARROW_LEFT), "Сохранить и выйти в меню", ButtonKind::Secondary, 380.0, true).clicked() {
                        action = MenuAction::SaveAndQuit;
                    }
                });
            });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && action == MenuAction::None {
        // Handled by the app too; this keeps the card responsive if egui sees the key first.
    }
    action
}

/// Death screen.
/// Death screen in a cinematic style: the world fades to dark, a big title slides in,
/// then respawn / menu buttons; respawns by itself after a few seconds.
pub fn death_screen(ctx: &egui::Context, icons: &GuiIcons) -> MenuAction {
    let mut action = MenuAction::None;
    let now = ctx.input(|i| i.time);
    // Start time: reset when the screen was not shown for a moment (a new death).
    let (t0_id, last_id) = (egui::Id::new("death-t0"), egui::Id::new("death-last"));
    let last: f64 = ctx.data(|d| d.get_temp(last_id)).unwrap_or(-10.0);
    let mut t0: f64 = ctx.data(|d| d.get_temp(t0_id)).unwrap_or(now);
    if now - last > 0.5 {
        t0 = now;
    }
    ctx.data_mut(|d| {
        d.insert_temp(t0_id, t0);
        d.insert_temp(last_id, now);
    });
    let t = (now - t0) as f32;
    ctx.request_repaint();
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("death-fx")));
    let fade = (t / 1.2).clamp(0.0, 1.0);
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(20, 8, 8, (170.0 * fade) as u8));
    // Letterbox bars slide in.
    let bar = screen.height() * 0.12 * fade;
    painter.rect_filled(Rect::from_min_size(screen.min, Vec2::new(screen.width(), bar)), 0.0, Color32::BLACK);
    painter.rect_filled(Rect::from_min_max(Pos2::new(screen.left(), screen.bottom() - bar), screen.max), 0.0, Color32::BLACK);
    // Title: grows from large to normal and fades in.
    let ta = ((t - 0.4) / 0.8).clamp(0.0, 1.0);
    let size = 64.0 + (1.0 - ta) * 30.0;
    let c = screen.center() - Vec2::new(0.0, 40.0);
    painter.text(c + Vec2::splat(4.0), Align2::CENTER_CENTER, "ВЫ ПОГИБЛИ", bold_font(size), Color32::from_black_alpha((220.0 * ta) as u8));
    painter.text(c, Align2::CENTER_CENTER, "ВЫ ПОГИБЛИ", bold_font(size), Color32::from_rgba_unmultiplied(200, 30, 35, (255.0 * ta) as u8));
    const AUTO: f32 = 6.0;
    let left = (AUTO - t).ceil().max(0.0);
    if t > 1.4 {
        painter.text(c + Vec2::new(0.0, 52.0), Align2::CENTER_CENTER, format!("Возрождение через {left:.0}…"), FontId::proportional(16.0), Color32::from_gray(210));
        egui::Area::new(egui::Id::new("death-buttons")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 110.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if pill_button(ui, icons, Some(icon::REFRESH), "Возродиться", ButtonKind::Primary, 200.0, true).clicked() {
                    action = MenuAction::Resume;
                }
                ui.add_space(12.0);
                if pill_button(ui, icons, Some(icon::FOLDER), "В меню", ButtonKind::Secondary, 160.0, true).clicked() {
                    action = MenuAction::SaveAndQuit;
                }
            });
        });
    }
    if t >= AUTO && action == MenuAction::None {
        action = MenuAction::Resume;
    }
    action
}

#[allow(dead_code)]
const _ANIM: f32 = ANIM_TIME;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_game_menu_buttons_open_worlds_settings_and_launcher() {
        for size in [Vec2::new(1280.0, 720.0), Vec2::new(800.0, 360.0)] {
            let (ctx, icons) = setup();
            for (label, expected) in [("Играть", MenuAction::OpenWorldSelect), ("Настройки", MenuAction::OpenSettings), ("Выход", MenuAction::Back)] {
                let mut action = MenuAction::None;
                let mut frame = |events| ctx.run(egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default()
                }, |ctx| { action = game_menu(ctx, &icons, "0.2.1", None); });
                let out = frame(vec![]);
                let text = out.shapes.iter().find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t), _ => None
                }).expect("classic menu button");
                let pos = text.pos + text.galley.size() * 0.5;
                frame(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
                frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
                assert_eq!(action, expected, "{label}, {size:?}");
            }
            // Only three buttons now: the old dock items are gone.
            for (label, expected) in [] as [(&str, MenuAction); 0] {
                let mut action = MenuAction::None;
                let mut frame = |events| ctx.run(egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default()
                }, |ctx| { action = game_menu(ctx, &icons, "0.2.1", None); });
                let out = frame(vec![]);
                let text = out.shapes.iter().find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t), _ => None
                }).expect("dock item");
                let pos = text.pos + text.galley.size() * 0.5;
                frame(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
                frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
                assert_eq!(action, expected, "{label}, {size:?}");
            }
        }
    }


    #[test]
    fn portrait_worlds_put_controls_above_full_width_cards_and_play_works() {
        let (ctx, icons) = setup();
        let worlds = vec![WorldEntry { cfg: WorldConfig::new("Телефон", 42), preview: None }];
        let mut st = WorldSelectState::default();
        let render = |st: &mut WorldSelectState, events| {
            let mut action = MenuAction::None;
            let out = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0))),
                events, ..Default::default()
            }, |ctx| { action = world_select(ctx, &icons, st, &worlds, "0.3.0"); });
            (out, action)
        };
        render(&mut st, vec![]);
        let (out, _) = render(&mut st, vec![]);
        let text = |label: &str| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t.pos + t.galley.size() * 0.5), _ => None
        }).expect(label);
        let single = text("Игра");
        let network = text("Сеть");
        let create = text("Создать мир");
        let world = text("Телефон");
        let play = text("Играть");
        // Mode tabs sit together above the game list.
        assert!((single.y-network.y).abs()<2.0 && single.y < create.y && create.y < world.y && world.y < play.y);
        assert!(play.x > 0.0 && play.x < 390.0 && play.y < 844.0);
        render(&mut st, vec![egui::Event::PointerMoved(play), egui::Event::PointerButton { pos: play, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        let (_, action) = render(&mut st, vec![egui::Event::PointerButton { pos: play, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert_eq!(action, MenuAction::PlayWorld(worlds[0].cfg.id.clone()));
    }

    #[test]
    fn portrait_game_menu_keeps_buttons_and_dock_inside_phone_screen() {
        for width in [320.0, 390.0] {
            let (ctx, icons) = setup();
            let mut render = || ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 740.0))),
                ..Default::default()
            }, |ctx| { game_menu(ctx, &icons, "0.3.0", None); });
            render();
            let out = render();
            for label in ["Играть", "Настройки", "Выход"] {
                let r = out.shapes.iter().find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t.galley.rect.translate(t.pos.to_vec2())), _ => None
                }).expect(label);
                assert!(r.left() >= 0.0 && r.right() <= width + 1.0 && r.top() >= 0.0 && r.bottom() <= 740.0, "{label}: {r:?}");
            }
        }
    }

    #[test]
    fn portrait_settings_stack_nickname_and_wrap_skins() {
        let (ctx, icons) = setup();
        let mut settings = Settings::default();
        let packs = PacksView { list: &[], status: "", folder: "" };
        let mut render = || ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0))),
            ..Default::default()
        }, |ctx| { settings_screen(ctx, &icons, &mut settings, "0.3.0", &packs); });
        render();
        let out = render();
        let text = |label: &str| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(Rect::from_min_size(t.pos, t.galley.size())), _ => None
        }).expect(label);
        assert!(text("Никнейм").bottom() < text("Игрок").top());
        for label in wolfcraft_engine::renderer::model::SKIN_NAMES {
            assert!(text(label).right() <= 390.0);
        }
    }

    #[test]
    fn world_selection_has_mode_tabs_below_header() {
        let (ctx, icons) = setup();
        let worlds = vec![WorldEntry { cfg: WorldConfig::new("Side test world", 42), preview: None }];
        let mut st = WorldSelectState::default();
        let render = |st: &mut WorldSelectState, events| ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 540.0))),
            events, ..Default::default()
        }, |ctx| { let _ = world_select(ctx, &icons, st, &worlds, "0.2.1"); });
        render(&mut st, vec![]);
        let out = render(&mut st, vec![]);
        let text = |out: &egui::FullOutput, label: &str| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t.pos + t.galley.size() * 0.5), _ => None
        }).expect(label);
        let single = text(&out, "Игра");
        let network = text(&out, "Сеть");
        let create = text(&out, "Создать мир");
        assert!((single.y-network.y).abs()<2.0 && network.x>single.x);
        assert!(create.y>single.y);
        assert!(text(&out,"Side test world").y>create.y);
        render(&mut st, vec![egui::Event::PointerMoved(network), egui::Event::PointerButton { pos: network, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        render(&mut st, vec![egui::Event::PointerButton { pos: network, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert_eq!(st.mode, 1);
        render(&mut st, vec![]);
        let out = render(&mut st, vec![]);
        assert!(text(&out, "По коду").y > network.y);
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Text(t) if t.galley.job.text == "Side test world")), "game list is hidden on the network tab");
    }

    #[test]
    fn settings_tabs_show_only_the_selected_section() {
        let (ctx, icons) = setup();
        let mut settings = Settings::default();
        let packs = PacksView { list: &[], status: "", folder: "" };
        let mut render = |events| ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 540.0))),
            events, ..Default::default()
        }, |ctx| { settings_screen(ctx, &icons, &mut settings, "0.2.1", &packs); });
        render(vec![]);
        let out = render(vec![]);
        let text = |label: &str| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t), _ => None
        }).expect("settings label");
        let title = text("Настройки");
        // Tabs share one row; only the selected section fills the centred card.
        let profile = text("Профиль");
        let graphics = text("Графика");
        assert!(profile.pos.y > title.pos.y && (graphics.pos.y-profile.pos.y).abs()<2.0);
        let nick = text("Никнейм");
        assert!(nick.pos.y > profile.pos.y);
        assert!(nick.pos.x > (960.0 - 620.0) / 2.0 - 1.0, "content sits in the centred column");
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Text(t) if t.galley.job.text == "Дальность прорисовки")));
        let at=graphics.pos+graphics.galley.size()*0.5;
        render(vec![egui::Event::PointerMoved(at),egui::Event::PointerButton {pos:at,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
        render(vec![egui::Event::PointerButton {pos:at,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
        let out=render(vec![]);
        assert!(out.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Text(t) if t.galley.job.text == "Дальность прорисовки")));
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Text(t) if t.galley.job.text == "Никнейм")));
    }

    fn setup() -> (egui::Context, GuiIcons) {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, ThemeMode::Dark);
        let atlases = wolfcraft_assets::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        (ctx, icons)
    }

    fn frame(ctx: &egui::Context, mut f: impl FnMut(&egui::Context)) {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 760.0)));
        let _ = ctx.run(raw, |c| f(c));
    }

    /// A tap = finger down in one frame, up in the next (like a phone). Returns the action of
    /// the frame where the finger lifts.
    fn tap(ctx: &egui::Context, icons: &GuiIcons, st: &mut WorldSelectState, at: Pos2) -> MenuAction {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 720.0));
        let mut act = MenuAction::None;
        for pressed in [true, false] {
            let mut raw = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            raw.events.push(egui::Event::PointerMoved(at));
            raw.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
            let _ = ctx.run(raw, |c| {
                let a = world_select(c, icons, st, &[], "0.3.0");
                if a != MenuAction::None {
                    act = a;
                }
            });
            if std::env::var("TAP_TRACE").is_ok() {
                let b: Option<(Rect, bool, bool, bool, bool)> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_play")));
                let r: Option<(Rect, bool, bool, bool, bool)> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_row")));
                println!("  {} at {at:?}: play {b:?} row {r:?} act {act:?}", if pressed { "down" } else { "up  " });
            }
        }
        // Pointer leaves the screen between taps.
        let mut raw = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        raw.events.push(egui::Event::PointerGone);
        let _ = ctx.run(raw, |c| {
            world_select(c, icons, st, &[], "0.3.0");
        });
        act
    }

    #[test]
    fn tap_on_nearby_server_joins_it() {
        let (ctx, icons) = setup();
        let mut st = WorldSelectState { mode: 1, net_tab: 0, ..Default::default() };
        st.nearby = vec![("Сервер".into(), "10.8.181.7:25570".into(), 0, 16)];
        // A few frames to lay the screen out (the rows learn their size).
        for _ in 0..3 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 720.0))), ..Default::default() };
            let _ = ctx.run(raw, |c| {
                world_select(c, &icons, &mut st, &[], "0.3.0");
            });
        }
        let want = MenuAction::JoinServer("10.8.181.7:25570".into());
        type Dbg = (Rect, bool, bool, bool, bool);
        let play: Option<Dbg> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_play")));
        let row: Option<Dbg> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_row")));
        println!("play button: {play:?}\nrow: {row:?}");
        if let Some((r, ..)) = play {
            // Press frame and release frame, reporting what the button saw each time.
            unsafe { std::env::set_var("TAP_TRACE", "1") };
            let a = tap(&ctx, &icons, &mut st, r.center());
            unsafe { std::env::remove_var("TAP_TRACE") };
            let after: Option<Dbg> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_play")));
            println!("tap at play centre {:?} -> {a:?}; button now {after:?}", r.center());
            assert_eq!(a, want, "tap in the middle of «Играть»");
        }
        // And a tap on the row itself (on the server's name, left side).
        if let Some((r, ..)) = row {
            let a = tap(&ctx, &icons, &mut st, Pos2::new(r.left() + 120.0, r.center().y));
            assert_eq!(a, want, "tap on the server row");
        }
        assert!(play.is_some() && row.is_some(), "the nearby server row is drawn");
    }

    #[test]
    fn all_screens_render_in_both_themes() {
        let (ctx, icons) = setup();
        let worlds = vec![WorldEntry { cfg: WorldConfig::new("Тест", 5), preview: None }];
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            crate::theme_manager::apply(&ctx, mode);
            frame(&ctx, |c| {
                main_menu(c, &icons, "0.1.0", "C:\\data", mode == ThemeMode::Dark, None, 0, "Игрок", &[]);
            });
            let mut ws = WorldSelectState::default();
            frame(&ctx, |c| {
                world_select(c, &icons, &mut ws, &worlds, "0.1.0");
            });
            let mut cw = CreateWorldState::default();
            frame(&ctx, |c| {
                create_world(c, &icons, &mut cw, None, "0.1.0", 1);
            });
            let mut s = Settings::default();
            frame(&ctx, |c| {
                settings_screen(c, &icons, &mut s, "0.1.0", &PacksView { list: &[], status: "", folder: "" });
            });
            let stages = vec!["A".to_string(), "B".to_string()];
            frame(&ctx, |c| loading_screen(c, &icons, &LoadingView { title: "t", stages: &stages, current: 1, fraction: 0.4, detail: "d", grid: &[] }));
            frame(&ctx, |c| {
                pause_menu(c, &icons, &PauseInfo { world_name: "Мир".into(), mode: "Выживание".into(), clock: "12:00".into(), biome: "Лес".into(), seed: 5, coords: [1.0, 2.0, 3.0], camera: "Вид от первого лица".into() });
            });
        }
    }


    #[test]
    fn portrait_create_world_has_top_tabs_and_visible_working_create_button() {
        let (ctx, icons) = setup();
        let mut st = CreateWorldState::default();
        st.name = "Мобильный мир".into();
        let render = |st: &mut CreateWorldState, events| {
            let mut action = MenuAction::None;
            let out = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0))),
                events, ..Default::default()
            }, |ctx| { action = create_world(ctx, &icons, st, None, "0.3.0", 42); });
            (out, action)
        };
        render(&mut st, vec![]);
        let (out, _) = render(&mut st, vec![]);
        let text = |label: &str, below: f32| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label && t.pos.y > below => Some(t.galley.rect.translate(t.pos.to_vec2())), _ => None
        }).expect(label);
        let base = text("Основное", 0.0);
        let planet = text("Планета", 0.0);
        assert!((base.center().y - planet.center().y).abs() < 2.0);
        let create = text("Создать мир", 700.0);
        assert!(create.right() <= 390.0 && create.bottom() < 844.0);
        let at = create.center();
        render(&mut st, vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        let (_, action) = render(&mut st, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert!(matches!(action, MenuAction::CreateWorld(ref cfg) if cfg.name == "Мобильный мир"));
    }

    #[test]
    fn create_world_validation_and_config() {
        let mut st = CreateWorldState::default();
        st.name = "  ".into();
        assert!(st.to_config(1).is_err());
        st.name = "x".repeat(41);
        assert!(st.to_config(1).is_err());
        st.name = "Мир".into();
        st.size = PlanetSize::Huge;
        st.world_type = WorldType::Islands;
        st.structures = false;
        st.mode = GameMode::Creative;
        st.seed_text = "12345".into();
        let c = st.to_config(99).unwrap();
        assert_eq!((c.seed, c.size, c.world_type, c.structures, c.mode), (12345, PlanetSize::Huge, WorldType::Islands, false, GameMode::Creative));
        st.seed_text.clear();
        assert_eq!(st.to_config(99).unwrap().seed, 99);
    }

    #[test]
    fn world_list_sorting_and_search() {
        let mk = |name: &str, created: u64, played: u64| {
            let mut c = WorldConfig::new(name, 1);
            c.id = name.to_lowercase();
            c.created = created;
            c.last_played = played;
            WorldEntry { cfg: c, preview: None }
        };
        let w = vec![mk("Бета", 1, 30), mk("Альфа", 3, 10), mk("Гамма", 2, 20)];
        let names = |v: Vec<&WorldEntry>| v.iter().map(|w| w.cfg.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(sorted_filtered(&w, "", 0)), vec!["Бета", "Гамма", "Альфа"]);
        assert_eq!(names(sorted_filtered(&w, "", 1)), vec!["Альфа", "Бета", "Гамма"]);
        assert_eq!(names(sorted_filtered(&w, "", 2)), vec!["Альфа", "Гамма", "Бета"]);
        assert_eq!(names(sorted_filtered(&w, "ГАМ", 0)), vec!["Гамма"]);
    }
}


