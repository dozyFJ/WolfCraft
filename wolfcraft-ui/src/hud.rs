//! In-game HUD: top bar, crosshair, hints, hotbar, health/hunger/XP, F3 debug overlay.

use crate::gui_renderer::{GuiIcons, centered_square};
use crate::theme_manager::{Palette, bold_font, palette};
use egui::{Align2, Color32, Context, FontId, LayerId, Order, Painter, Pos2, Rect, Rounding, Stroke, Vec2};
use wolfcraft_assets::gui as icon;
use wolfcraft_engine::inventory::{HOTBAR, Inventory};
use wolfcraft_engine::items::ItemStack;

#[derive(Clone, Debug, Default)]
pub struct DebugInfo {
    pub fps: f32,
    pub frame_ms_avg: f32,
    pub frame_ms_max: f32,
    pub cpu_percent: f32,
    pub cpu_cores: usize,
    pub ram_mb: f32,
    pub gpu_name: String,
    /// GPU validation errors so far, with the last message.
    pub gpu_errors: String,
    pub backend: String,
    pub pos: [f64; 3],
    pub block: [i32; 3],
    pub chunk: [i32; 3],
    pub altitude: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub biome: String,
    pub layer: String,
    pub facing: String,
    pub loaded_chunks: usize,
    pub meshed_chunks: usize,
    pub gen_inflight: usize,
    pub mesh_inflight: usize,
    pub visible_chunks: usize,
    pub gpu_chunks: usize,
    pub draw_calls: u32,
    pub triangles: u32,
    pub gpu_buffer_mb: f32,
    pub gui_vertices: usize,
    pub vsync: bool,
    pub view_distance: u32,
    pub clock: String,
    pub seed: u64,
    pub world_name: String,
    pub update_ms: f32,
    pub chunks_ms: f32,
    pub render_ms: f32,
    pub ui_ms: f32,
}

pub struct HudState<'a> {
    pub coords: [f64; 3],
    pub fps: f32,
    pub clock: String,
    pub biome: String,
    pub inventory: &'a Inventory,
    pub health: f32,
    pub hunger: f32,
    pub xp_level: u32,
    pub xp_progress: f32,
    pub air: f32,
    pub underwater: bool,
    pub hurt: f32,
    pub creative: bool,
    pub flying: bool,
    pub hint: Option<String>,
    pub mining: f32,
    /// Bow draw 0..1 (a bar under the crosshair while drawing).
    pub bow_charge: f32,
    /// Text of the sign under the crosshair.
    pub sign_text: Option<String>,
    pub item_name: Option<(String, f32)>,
    pub show_fps: bool,
    pub debug: Option<&'a DebugInfo>,
    /// Facing, degrees clockwise from north (compass strip above the hotbar).
    pub compass: Option<f32>,
    /// Heading-up map of the surroundings (bottom-right corner).
    pub minimap: Option<egui::TextureId>,
    /// Phone layout: no top bar, minimap in the top-right corner.
    pub compact: bool,
    /// Draw the crosshair (hidden when blocks are picked with the finger).
    pub crosshair: bool,
}

fn pill(painter: &Painter, rect: Rect, p: &Palette) {
    painter.rect_filled(rect, Rounding::ZERO, p.surface.gamma_multiply(0.82));
    painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
}

pub const SLOT: f32 = 46.0;
pub const SLOT_GAP: f32 = 5.0;

/// Draws an item icon with count and durability inside `rect`.
pub fn draw_stack(painter: &Painter, icons: &GuiIcons, rect: Rect, s: &ItemStack, p: &Palette) {
    icons.paint_item(painter, rect.shrink(rect.width() * 0.14), s.id);
    if s.count > 1 {
        let pos = Pos2::new(rect.right() - 4.0, rect.bottom() - 3.0);
        painter.text(pos + Vec2::splat(1.0), Align2::RIGHT_BOTTOM, s.count.to_string(), FontId::proportional(13.0), Color32::from_black_alpha(200));
        painter.text(pos, Align2::RIGHT_BOTTOM, s.count.to_string(), FontId::proportional(13.0), Color32::WHITE);
    }
    let dur = s.info().durability;
    if dur > 0 && s.damage > 0 {
        let f = 1.0 - s.damage as f32 / dur as f32;
        let bar = Rect::from_min_size(Pos2::new(rect.left() + 6.0, rect.bottom() - 7.0), Vec2::new(rect.width() - 12.0, 3.0));
        painter.rect_filled(bar, Rounding::ZERO, Color32::from_black_alpha(160));
        let col = Color32::from_rgb(((1.0 - f) * 255.0) as u8, (f * 220.0) as u8, 60);
        painter.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * f, 3.0)), Rounding::ZERO, col);
    }
    let _ = p;
}

/// Hotbar rectangle (min x, min y, max x, max y) in points, published for the touch buttons.
pub static HOTBAR_RECT: std::sync::RwLock<[f32; 4]> = std::sync::RwLock::new([0.0; 4]);
/// Inventory tile next to the hotbar (phones), in points.
pub static INV_RECT: std::sync::RwLock<[f32; 4]> = std::sync::RwLock::new([0.0; 4]);
/// Minimap with its compass frame, in points: tapping it opens the world map.
pub static MINIMAP_RECT: std::sync::RwLock<[f32; 4]> = std::sync::RwLock::new([0.0; 4]);

/// Rows of the 9×9 heart: `o` outline, `r` red, `w` highlight, `.` empty.
const HEART: [&str; 9] = [".oo...oo.", "orro.orro", "orwrorrro", "orrrrrrro", "orrrrrrro", ".orrrrro.", "..orrro..", "...oro...", "....o...."];

/// Pseudo-random 0..2 per heart and time step (the low-health shake).
pub(crate) fn shake_seed(i: usize, step: u64) -> u64 {
    let mut h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ step.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 32)
}

/// One heart of `px`-sized pixels at `origin` (top-left); `fill` 1 = full, 0.5 = half.
fn pixel_heart(painter: &Painter, origin: Pos2, px: f32, fill: f32, flash: bool) {
    let outline = if flash { Color32::from_rgb(255, 255, 255) } else { Color32::from_rgb(20, 8, 10) };
    let empty = Color32::from_rgb(52, 18, 22);
    let red = Color32::from_rgb(222, 30, 38);
    let dark = Color32::from_rgb(160, 18, 28);
    let hi = Color32::from_rgb(255, 170, 170);
    // Filled columns: a half heart covers the left 4.5 of 9.
    let filled_cols = if fill >= 0.99 { 9.0 } else if fill > 0.01 { 4.5 } else { 0.0 };
    for (y, row) in HEART.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let col = match ch {
                'o' => outline,
                'r' | 'w' => {
                    if (x as f32) < filled_cols {
                        if ch == 'w' { hi } else if y >= 5 || x >= 6 { dark } else { red }
                    } else {
                        empty
                    }
                }
                _ => continue,
            };
            let r = Rect::from_min_size(Pos2::new(origin.x + x as f32 * px, origin.y + y as f32 * px), Vec2::splat(px));
            painter.rect_filled(r, Rounding::ZERO, col);
        }
    }
}

pub fn draw(ctx: &Context, icons: &GuiIcons, s: &HudState) {
    let p = palette(ctx);
    let painter = ctx.layer_painter(LayerId::new(Order::Background, egui::Id::new("hud")));
    let screen = ctx.screen_rect();

    // Damage flash.
    // Damage: a red glow along the screen edges (no full-screen tint).
    if s.hurt > 0.0 {
        let a = (s.hurt / 0.35).clamp(0.0, 1.0);
        let edge = Color32::from_rgba_unmultiplied(230, 20, 30, (190.0 * a) as u8);
        let clear = Color32::from_rgba_unmultiplied(230, 20, 30, 0);
        let w = (screen.width().min(screen.height()) * 0.16).max(40.0);
        let mut mesh = egui::Mesh::default();
        let mut strip = |o0: Pos2, o1: Pos2, i0: Pos2, i1: Pos2| {
            let b = mesh.vertices.len() as u32;
            mesh.colored_vertex(o0, edge);
            mesh.colored_vertex(o1, edge);
            mesh.colored_vertex(i1, clear);
            mesh.colored_vertex(i0, clear);
            mesh.add_triangle(b, b + 1, b + 2);
            mesh.add_triangle(b, b + 2, b + 3);
        };
        let r = screen;
        let ri = r.shrink(w);
        strip(r.left_top(), r.right_top(), ri.left_top(), ri.right_top());
        strip(r.right_top(), r.right_bottom(), ri.right_top(), ri.right_bottom());
        strip(r.right_bottom(), r.left_bottom(), ri.right_bottom(), ri.left_bottom());
        strip(r.left_bottom(), r.left_top(), ri.left_bottom(), ri.left_top());
        painter.add(egui::Shape::mesh(mesh));
    }

    // ---- Top bar: coordinates · FPS · time (not on phones) ----
    if !s.compact {
    let text = format!("X {:.0}   Y {:.0}   Z {:.0}", s.coords[0], s.coords[1], s.coords[2]);
    let fps = if s.show_fps { format!("{:.0} FPS", s.fps) } else { String::new() };
    let right = format!("{}  ·  {}", s.biome, s.clock);
    let f = FontId::proportional(14.0);
    let w1 = painter.layout_no_wrap(text.clone(), f.clone(), Color32::WHITE).size().x;
    let w2 = painter.layout_no_wrap(fps.clone(), f.clone(), Color32::WHITE).size().x;
    let w3 = painter.layout_no_wrap(right.clone(), f.clone(), Color32::WHITE).size().x;
    let total = w1 + w2 + w3 + 3.0 * 30.0 + if s.show_fps { 20.0 } else { 0.0 };
    let bar = Rect::from_center_size(Pos2::new(screen.center().x, if s.compact { 47.0 } else { 28.0 }), Vec2::new(total, 32.0));
    pill(&painter, bar, &p);
    let mut x = bar.left() + 18.0;
    painter.text(Pos2::new(x, bar.center().y), Align2::LEFT_CENTER, &text, f.clone(), p.text);
    x += w1 + 22.0;
    if s.show_fps {
        painter.line_segment([Pos2::new(x - 11.0, bar.top() + 8.0), Pos2::new(x - 11.0, bar.bottom() - 8.0)], Stroke::new(2.0_f32, p.outline));
        let col = if s.fps >= 55.0 { p.success } else if s.fps >= 30.0 { p.warn } else { p.danger };
        painter.text(Pos2::new(x, bar.center().y), Align2::LEFT_CENTER, &fps, f.clone(), col);
        x += w2 + 22.0;
    }
    painter.line_segment([Pos2::new(x - 11.0, bar.top() + 8.0), Pos2::new(x - 11.0, bar.bottom() - 8.0)], Stroke::new(2.0_f32, p.outline));
    painter.text(Pos2::new(x, bar.center().y), Align2::LEFT_CENTER, &right, f, p.text_muted);
    }

    // ---- Crosshair (hidden only when blocks are picked with the finger) ----
    let c = screen.center();
    for (a, b) in if !s.crosshair { &[][..] } else { &[((-9.0, 0.0), (-3.0, 0.0)), ((3.0, 0.0), (9.0, 0.0)), ((0.0, -9.0), (0.0, -3.0)), ((0.0, 3.0), (0.0, 9.0))][..] } {
        let (a, b): ((f32, f32), (f32, f32)) = (*a, *b);
        let (a, b) = (c + Vec2::new(a.0, a.1), c + Vec2::new(b.0, b.1));
        painter.line_segment([a, b], Stroke::new(4.0_f32, Color32::from_black_alpha(130)));
        painter.line_segment([a, b], Stroke::new(2.0_f32, Color32::from_white_alpha(235)));
    }
    // Bow draw: a small bar under the crosshair, turning gold when fully drawn.
    if s.bow_charge > 0.0 {
        let r = Rect::from_center_size(c + Vec2::new(0.0, 22.0), Vec2::new(44.0, 5.0));
        painter.rect_filled(r, Rounding::ZERO, Color32::from_black_alpha(150));
        let full = s.bow_charge >= 1.0;
        let col = if full { Color32::from_rgb(250, 200, 60) } else { Color32::from_white_alpha(230) };
        painter.rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width() * s.bow_charge.clamp(0.0, 1.0), r.height())), Rounding::ZERO, col);
    }
    if let Some(t) = &s.sign_text {
        let g = painter.layout(t.clone(), FontId::proportional(16.0), Color32::from_rgb(40, 28, 16), 360.0);
        let r = Rect::from_center_size(c - Vec2::new(0.0, 46.0), g.size() + Vec2::new(24.0, 14.0));
        painter.rect_filled(r, Rounding::same(4.0), Color32::from_rgb(214, 180, 128));
        painter.rect_stroke(r, Rounding::same(4.0), Stroke::new(2.0_f32, Color32::from_rgb(120, 84, 46)));
        painter.galley(r.min + Vec2::new(12.0, 7.0), g, Color32::from_rgb(40, 28, 16));
    }
    // No hint pill under the crosshair: the view stays clean.
    let _ = &s.hint;

    // ---- Bottom area: XP, health/hunger, hotbar ----
    // Separate slots, each on its own dark tile (no panel behind them); on phones the
    // inventory button is a tenth tile to the right of the hotbar.
    let slot_sz = if s.compact { 34.0 } else { SLOT };
    let gap = if s.compact { 6.0 } else { SLOT_GAP };
    let inv_gap = 14.0;
    let slots_w = HOTBAR as f32 * slot_sz + (HOTBAR as f32 - 1.0) * gap;
    let hb_w = slots_w;
    let total_w = slots_w + if s.compact { inv_gap + slot_sz } else { 0.0 };
    let left = screen.center().x - total_w / 2.0;
    // Phones: keep clear of the system gesture bar at the bottom edge.
    let bottom_gap = if s.compact { 30.0 } else { 14.0 };
    let hotbar = Rect::from_min_size(Pos2::new(left, screen.bottom() - bottom_gap - slot_sz), Vec2::new(slots_w, slot_sz));
    if let Ok(mut h) = HOTBAR_RECT.write() {
        *h = [hotbar.min.x, hotbar.min.y, hotbar.max.x, hotbar.max.y];
    }
    let tile = Color32::from_rgba_unmultiplied(14, 16, 22, 200);
    for i in 0..HOTBAR {
        let r = Rect::from_min_size(Pos2::new(hotbar.left() + i as f32 * (slot_sz + gap), hotbar.top()), Vec2::splat(slot_sz));
        let sel = i == s.inventory.selected;
        let t = ctx.animate_bool_with_time(egui::Id::new(("hb", i)), sel, 0.12);
        let rr = r.expand(t * 2.0);
        painter.rect_filled(rr, Rounding::ZERO, tile);
        painter.rect_stroke(rr, Rounding::ZERO, Stroke::new(1.5 + t * 1.5, if sel { p.accent } else { Color32::from_white_alpha(40) }));
        if let Some(st) = &s.inventory.slots[i] {
            draw_stack(&painter, icons, rr, st, &p);
        }
        painter.text(Pos2::new(r.left() + 4.0, r.top() + 2.0), Align2::LEFT_TOP, (i + 1).to_string(), FontId::proportional(9.5), Color32::from_white_alpha(110));
    }
    if s.compact {
        let r = Rect::from_min_size(Pos2::new(hotbar.right() + inv_gap, hotbar.top()), Vec2::splat(slot_sz));
        painter.rect_filled(r, Rounding::ZERO, tile);
        painter.rect_stroke(r, Rounding::ZERO, Stroke::new(1.5, Color32::from_white_alpha(70)));
        icons.paint(&painter, r.shrink(slot_sz * 0.22), icon::BACKPACK, Color32::WHITE);
        if let Ok(mut h) = INV_RECT.write() {
            *h = [r.min.x, r.min.y, r.max.x, r.max.y];
        }
    }
    // XP bar.
    let xp_y = hotbar.top() - 10.0;
    let xp = Rect::from_min_size(Pos2::new(hotbar.left() + 4.0, xp_y - 5.0), Vec2::new(hb_w - 8.0, 6.0));
    if !s.creative {
        painter.rect_filled(xp, Rounding::ZERO, p.surface_hi.gamma_multiply(0.9));
        painter.rect_filled(Rect::from_min_size(xp.min, Vec2::new(xp.width() * s.xp_progress.clamp(0.0, 1.0), 6.0)), Rounding::ZERO, Color32::from_rgb(150, 230, 60));
        if s.xp_level > 0 {
            painter.text(Pos2::new(xp.center().x, xp.top() - 5.0), Align2::CENTER_BOTTOM, s.xp_level.to_string(), bold_font(15.0), Color32::from_rgb(170, 245, 80));
        }
        // Health (left) and hunger (right).
        let row_y = xp.top() - 22.0 - if s.xp_level > 0 { 4.0 } else { -6.0 };
        // Minecraft-style pixel hearts: low health makes them shake, damage flashes the outline.
        let t = ctx.input(|i| i.time);
        let flash = s.hurt > 0.0 && ((t * 8.0) as i64) % 2 == 0;
        for i in 0..10 {
            let fill = ((s.health - i as f32 * 2.0) / 2.0).clamp(0.0, 1.0);
            let shake = if s.health <= 4.0 { ((crate::hud::shake_seed(i, (t * 12.0) as u64) % 3) as f32 - 1.0) * 1.5 } else { 0.0 };
            let origin = Pos2::new(hotbar.left() + 6.0 + i as f32 * 20.0, row_y + shake);
            pixel_heart(&painter, origin, 2.0, fill, flash);
        }
        for i in 0..10 {
            let r = Rect::from_min_size(Pos2::new(hotbar.right() - 6.0 - (i as f32 + 1.0) * 20.0 + 2.0, row_y), Vec2::splat(18.0));
            icons.paint(&painter, r, icon::HUNGER, Color32::from_black_alpha(110));
            let fill = ((s.hunger - (9 - i) as f32 * 2.0) / 2.0).clamp(0.0, 1.0);
            icons.paint_partial(&painter, r, icon::HUNGER, Color32::WHITE, fill);
        }
        // Air bubbles.
        if s.underwater || s.air < 14.9 {
            let n = (s.air / 15.0 * 10.0).ceil() as i32;
            for i in 0..n.clamp(0, 10) {
                let c = Pos2::new(hotbar.right() - 14.0 - i as f32 * 20.0, row_y - 14.0);
                painter.circle_stroke(c, 6.5, Stroke::new(2.0_f32, Color32::from_rgb(170, 220, 255)));
                painter.circle_filled(c + Vec2::new(-2.0, -2.0), 1.6, Color32::from_white_alpha(200));
            }
        }
    }
    if s.creative && s.flying {
        painter.text(Pos2::new(hotbar.center().x, xp_y - 8.0), Align2::CENTER_BOTTOM, "Полёт", FontId::proportional(13.0), p.accent);
    }
    // Compass strip above the hotbar (and above hearts / hunger in survival).
    let compass_bottom = hotbar.top() - if s.creative { 14.0 } else { 62.0 };
    if let (Some(b), false) = (s.compass, s.compact) {
        draw_compass(&painter, &p, Rect::from_center_size(Pos2::new(hotbar.center().x, compass_bottom - 12.0), Vec2::new(hb_w.min(420.0), 24.0)), b);
    }
    // Selected item name.
    if let Some((name, t)) = &s.item_name {
        let a = (t * 2.0).clamp(0.0, 1.0);
        let y = if s.compass.is_some() && !s.compact { compass_bottom - 32.0 } else { hotbar.top() - if s.creative { 24.0 } else { 70.0 } };
        painter.text(Pos2::new(hotbar.center().x, y), Align2::CENTER_BOTTOM, name, FontId::proportional(16.0), p.text.gamma_multiply(a));
    }
    // Minimap, bottom-right.
    if let Some(tex) = s.minimap {
        let size = if s.compact { 104.0 } else { 156.0 };
        let r = if s.compact {
            Rect::from_min_size(Pos2::new(screen.right() - size - 22.0, screen.top() + 20.0), Vec2::splat(size))
        } else {
            Rect::from_min_size(Pos2::new(screen.right() - size - 16.0, screen.bottom() - size - 16.0), Vec2::splat(size))
        };
        // Compass frame around the map: a wide border with the four cardinal letters riding
        // on it (the map turns with the view, so the letters move around the frame).
        let frame = r.expand(13.0);
        painter.rect_filled(frame, Rounding::ZERO, Color32::from_rgba_unmultiplied(14, 16, 22, 215));
        painter.image(tex, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        painter.rect_stroke(r, Rounding::ZERO, Stroke::new(1.0_f32, Color32::from_white_alpha(60)));
        painter.rect_stroke(frame, Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
        if let Some(b) = s.compass {
            let half = frame.width() / 2.0 - 6.5;
            for (deg, name) in [(0.0f32, "С"), (90.0, "В"), (180.0, "Ю"), (270.0, "З")] {
                let a = (deg - b).to_radians();
                let (dx, dy) = (a.sin(), -a.cos());
                let k = half / dx.abs().max(dy.abs()).max(1e-3);
                let pos = frame.center() + Vec2::new(dx * k, dy * k);
                let col = if deg == 0.0 { p.danger } else { Color32::WHITE };
                painter.text(pos + Vec2::splat(1.0), Align2::CENTER_CENTER, name, bold_font(12.0), Color32::from_black_alpha(200));
                painter.text(pos, Align2::CENTER_CENTER, name, bold_font(12.0), col);
            }
        }
        if let Ok(mut h) = MINIMAP_RECT.write() {
            *h = [frame.min.x, frame.min.y, frame.max.x, frame.max.y];
        }
        // Player arrow (the map turns with the view, so it always points up).
        let c = r.center();
        let pts = vec![c + Vec2::new(0.0, -8.0), c + Vec2::new(6.0, 7.0), c + Vec2::new(0.0, 3.5), c + Vec2::new(-6.0, 7.0)];
        painter.add(egui::Shape::convex_polygon(pts, p.accent, Stroke::new(1.5_f32, Color32::from_black_alpha(200))));
    }

    if let Some(d) = s.debug {
        draw_debug(&painter, &p, d);
    }
    let _ = centered_square;
}

/// Horizontal compass: ticks every 15°, letters every 45°, ±75° visible around `bearing`.
fn draw_compass(painter: &Painter, p: &Palette, r: Rect, bearing: f32) {
    pill(painter, r, p);
    let span = 75.0;
    let px = r.width() / (2.0 * span);
    let names = ["С", "СВ", "В", "ЮВ", "Ю", "ЮЗ", "З", "СЗ"];
    let first = ((bearing - span) / 15.0).ceil() as i32;
    let last = ((bearing + span) / 15.0).floor() as i32;
    let clip = painter.with_clip_rect(r.shrink2(Vec2::new(10.0, 0.0)));
    for k in first..=last {
        let d = k as f32 * 15.0;
        let x = r.center().x + (d - bearing) * px;
        let fade = 1.0 - ((d - bearing).abs() / span).powi(2) * 0.8;
        let deg = (d.rem_euclid(360.0)).round() as i32;
        if deg % 45 == 0 {
            let name = names[(deg / 45) as usize % 8];
            let col = if deg == 0 { p.danger } else { p.text };
            clip.text(Pos2::new(x, r.center().y), Align2::CENTER_CENTER, name, bold_font(if name.chars().count() == 1 { 14.0 } else { 12.0 }), col.gamma_multiply(fade));
        } else {
            clip.line_segment([Pos2::new(x, r.center().y - 4.0), Pos2::new(x, r.center().y + 4.0)], Stroke::new(1.5_f32, p.text_muted.gamma_multiply(fade)));
        }
    }
    // Centre marker.
    let c = Pos2::new(r.center().x, r.top() - 1.0);
    painter.add(egui::Shape::convex_polygon(vec![c + Vec2::new(-5.0, -5.0), c + Vec2::new(5.0, -5.0), c + Vec2::new(0.0, 2.0)], p.accent, Stroke::NONE));
}

fn draw_debug(painter: &Painter, p: &Palette, d: &DebugInfo) {
    // Minecraft-style F3: world info on the left, performance on the right; the game stays visible.
    let left = [
        format!("WolfCraft · {} · seed {}", d.world_name, d.seed),
        format!("{:.0} FPS ({:.2} ms avg, {:.2} ms max) VSync {}", d.fps, d.frame_ms_avg, d.frame_ms_max, if d.vsync { "вкл" } else { "выкл" }),
        String::new(),
        format!("XYZ: {:.2} / {:.2} / {:.2}", d.pos[0], d.pos[1], d.pos[2]),
        format!("Блок: {} {} {}", d.block[0], d.block[1], d.block[2]),
        format!("Чанк: {} {} {}", d.chunk[0], d.chunk[1], d.chunk[2]),
        format!("Высота над морем: {:.1}", d.altitude),
        format!("Шир.: {:.2}°  Долг.: {:.2}°", d.latitude, d.longitude),
        format!("Взгляд: {}", d.facing),
        format!("Биом: {}", d.biome),
        format!("Слой: {}", d.layer),
        format!("Время мира: {}", d.clock),
    ];
    let right = [
        format!("CPU {:.1}% ({} лог. ядер)", d.cpu_percent, d.cpu_cores),
        format!("RAM {:.0} МБ", d.ram_mb),
        String::new(),
        format!("GPU: {}", d.gpu_name),
        format!("API: {}", d.backend),
        d.gpu_errors.clone(),
        String::new(),
        format!("Кадр, мс: обн. {:.2} · чанки {:.2}", d.update_ms, d.chunks_ms),
        format!("рендер {:.2} · UI {:.2}", d.render_ms, d.ui_ms),
        String::new(),
        format!("Чанки: загружено {} · с мешем {}", d.loaded_chunks, d.meshed_chunks),
        format!("на GPU {} · видимых {}", d.gpu_chunks, d.visible_chunks),
        format!("Очередь: ген. {} · меш {}", d.gen_inflight, d.mesh_inflight),
        format!("Дальность: {} чанков", d.view_distance),
        format!("Draw calls: {} · треуг.: {}", d.draw_calls, d.triangles),
        format!("Буферы: {:.1} МБ · GUI: {} верш.", d.gpu_buffer_mb, d.gui_vertices),
    ];
    let font = FontId::monospace(13.0);
    let line_h = 16.0;
    let screen = painter.clip_rect();
    let draw = |lines: &[String], right_side: bool| {
        for (i, l) in lines.iter().enumerate() {
            if l.is_empty() {
                continue;
            }
            let w = painter.layout_no_wrap(l.clone(), font.clone(), Color32::WHITE).size().x;
            let y = screen.top() + 6.0 + i as f32 * line_h;
            let (x, anchor) = if right_side { (screen.right() - 8.0, Align2::RIGHT_TOP) } else { (screen.left() + 8.0, Align2::LEFT_TOP) };
            let bx = if right_side { x - w - 3.0 } else { x - 3.0 };
            painter.rect_filled(Rect::from_min_size(Pos2::new(bx, y - 1.0), Vec2::new(w + 6.0, line_h)), Rounding::ZERO, Color32::from_black_alpha(120));
            let col = if i == 0 && !right_side { p.accent } else { Color32::from_gray(235) };
            painter.text(Pos2::new(x, y), anchor, l, font.clone(), col);
        }
    };
    draw(&left, false);
    draw(&right, true);
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_draws_without_panicking_in_both_themes() {
        let ctx = Context::default();
        crate::theme_manager::install(&ctx);
        let atlases = wolfcraft_assets::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        let mut inv = Inventory::default();
        inv.add(ItemStack::of_block(wolfcraft_engine::block::Block::Lamp, 12));
        let dbg = DebugInfo::default();
        for mode in [wolfcraft_engine::config::ThemeMode::Dark, wolfcraft_engine::config::ThemeMode::Light] {
            crate::theme_manager::apply(&ctx, mode);
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0)));
            let out = ctx.run(raw, |ctx| {
                let s = HudState {
                    coords: [1.0, 2.0, 3.0],
                    fps: 120.0,
                    clock: "12:00".into(),
                    biome: "Равнины".into(),
                    inventory: &inv,
                    health: 13.0,
                    hunger: 7.0,
                    xp_level: 3,
                    xp_progress: 0.4,
                    air: 8.0,
                    underwater: true,
                    hurt: 0.2,
                    creative: false,
                    flying: false,
                    hint: Some("ЛКМ — сломать".into()),
                    mining: 0.5,
                    bow_charge: 0.5,
                    sign_text: Some("Привет".into()),
                    item_name: Some(("Лампа".into(), 1.0)),
                    show_fps: true,
                    debug: Some(&dbg),
                    compass: Some(35.0),
                    minimap: None,
                    compact: false,
                    crosshair: true,
                };
                draw(ctx, &icons, &s);
            });
            assert!(!out.shapes.is_empty());
        }
    }
}
