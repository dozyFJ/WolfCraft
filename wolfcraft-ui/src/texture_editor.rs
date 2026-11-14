//! «Текстуры»: a pixel-art editor inside WolfCraft Studio.
//!
//! * Top: a header (name, size, undo/redo, convert, save, close) and under it a dock bar
//!   with the tool's options (brush size, mirror, grid, fill shapes) and the colours.
//! * Left: the tools (pencil, brush, eraser, fill, pipette, line, rectangle, circle, and the
//!   special ones: dither, noise, shading lighter / darker).
//! * Right: layer slots (show/hide, pick, add, delete, move up/down, merge down).
//! * Middle: the canvas.
//!
//! A finished texture can be turned into blocks (a pixel-art wall in front of the camera) or
//! saved as a block or item texture of the place.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{Palette, bold_font};
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

pub type Rgba = [u8; 4];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PTool {
    #[default]
    Pencil,
    Brush,
    Eraser,
    Fill,
    Picker,
    Line,
    Rect,
    Circle,
    Dither,
    Noise,
    Lighten,
    Darken,
}

const TOOLS: [(PTool, &str, &str); 12] = [
    (PTool::Pencil, "✏", "Карандаш"),
    (PTool::Brush, "🖌", "Кисть"),
    (PTool::Eraser, "⌫", "Ластик"),
    (PTool::Fill, "◧", "Заливка"),
    (PTool::Picker, "💧", "Пипетка"),
    (PTool::Line, "╱", "Линия"),
    (PTool::Rect, "▭", "Прямоуг."),
    (PTool::Circle, "◯", "Круг"),
    (PTool::Dither, "▦", "Сетка"),
    (PTool::Noise, "░", "Шум"),
    (PTool::Lighten, "☀", "Светлее"),
    (PTool::Darken, "☾", "Темнее"),
];

/// A colour scheme that suits blocks: greys, earth, wood, greens, water, ores, bright.
const SWATCHES: [Rgba; 24] = [
    [0, 0, 0, 255],
    [60, 60, 64, 255],
    [125, 125, 125, 255],
    [190, 190, 190, 255],
    [245, 245, 245, 255],
    [120, 85, 60, 255],
    [90, 60, 40, 255],
    [170, 135, 85, 255],
    [220, 205, 150, 255],
    [95, 150, 60, 255],
    [55, 100, 40, 255],
    [150, 200, 90, 255],
    [50, 90, 190, 255],
    [100, 170, 235, 255],
    [30, 45, 110, 255],
    [190, 40, 40, 255],
    [240, 120, 40, 255],
    [240, 205, 60, 255],
    [100, 220, 215, 255],
    [150, 70, 190, 255],
    [235, 120, 170, 255],
    [190, 110, 75, 255],
    [215, 215, 215, 255],
    [40, 25, 50, 255],
];

#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub px: Vec<Rgba>,
    pub visible: bool,
}

#[derive(Clone, Debug)]
pub struct PaintState {
    pub open: bool,
    pub name: String,
    pub size: u32,
    pub layers: Vec<Layer>,
    pub active: usize,
    pub color: Rgba,
    pub tool: PTool,
    pub brush: i32,
    pub mirror: bool,
    pub grid: bool,
    pub shape_fill: bool,
    undo: Vec<Vec<Layer>>,
    redo: Vec<Vec<Layer>>,
    drag_from: Option<(i32, i32)>,
    last: Option<(i32, i32)>,
    seed: u32,
}

impl Default for PaintState {
    fn default() -> Self {
        let size = 16;
        Self {
            open: false,
            name: "текстура".into(),
            size,
            layers: vec![Layer { name: "Слой 1".into(), px: vec![[0; 4]; (size * size) as usize], visible: true }],
            active: 0,
            color: [95, 150, 60, 255],
            tool: PTool::Pencil,
            brush: 2,
            mirror: false,
            grid: true,
            shape_fill: false,
            undo: Vec::new(),
            redo: Vec::new(),
            drag_from: None,
            last: None,
            seed: 1,
        }
    }
}

/// What the editor asks the game to do.
#[derive(Clone, PartialEq, Debug)]
pub enum PaintAction {
    /// Build the picture out of blocks in front of the camera.
    ToBlocks { size: u32, rgba: Vec<Rgba> },
    /// Save as a texture of the place (`textures/blocks` or `textures/items`).
    Save { name: String, size: u32, rgba: Vec<Rgba>, item: bool },
}

impl PaintState {
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        let n = self.size as i32;
        (x >= 0 && y >= 0 && x < n && y < n).then(|| (y * n + x) as usize)
    }

    fn snapshot(&mut self) {
        self.undo.push(self.layers.clone());
        if self.undo.len() > 40 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        if let Some(l) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.layers, l));
            self.active = self.active.min(self.layers.len() - 1);
        }
    }

    pub fn redo(&mut self) {
        if let Some(l) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.layers, l));
            self.active = self.active.min(self.layers.len() - 1);
        }
    }

    /// All visible layers blended, bottom first.
    pub fn composite(&self) -> Vec<Rgba> {
        let mut out = vec![[0u8; 4]; (self.size * self.size) as usize];
        for l in self.layers.iter().filter(|l| l.visible) {
            for (o, s) in out.iter_mut().zip(&l.px) {
                *o = blend(*o, *s);
            }
        }
        out
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed % 10_000) as f32 / 10_000.0
    }

    /// One pixel with the current tool (and its mirror image).
    fn apply(&mut self, x: i32, y: i32) {
        let n = self.size as i32;
        let mut spots = vec![(x, y)];
        if self.mirror && n - 1 - x != x {
            spots.push((n - 1 - x, y));
        }
        for (x, y) in spots {
            let Some(i) = self.idx(x, y) else { continue };
            let r = self.rand();
            let tool = self.tool;
            let color = self.color;
            let px = &mut self.layers[self.active].px[i];
            match tool {
                PTool::Eraser => *px = [0; 4],
                PTool::Dither => {
                    if (x + y) % 2 == 0 {
                        *px = color;
                    }
                }
                PTool::Noise => {
                    let k = 0.85 + r * 0.3;
                    *px = [scale(color[0], k), scale(color[1], k), scale(color[2], k), color[3]];
                }
                PTool::Lighten | PTool::Darken => {
                    if px[3] > 0 {
                        let k = if tool == PTool::Lighten { 1.12 } else { 0.88 };
                        *px = [scale(px[0], k), scale(px[1], k), scale(px[2], k), px[3]];
                    }
                }
                _ => *px = color,
            }
        }
    }

    /// The current tool at a point, with the brush's size where it has one.
    fn stamp(&mut self, x: i32, y: i32) {
        let r = if matches!(self.tool, PTool::Pencil) { 0 } else { self.brush - 1 };
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r + r / 2 {
                    self.apply(x + dx, y + dy);
                }
            }
        }
    }

    fn stroke(&mut self, a: (i32, i32), b: (i32, i32)) {
        for (x, y) in line(a, b) {
            self.stamp(x, y);
        }
    }

    fn fill(&mut self, x: i32, y: i32) {
        let Some(start) = self.idx(x, y) else { return };
        let from = self.layers[self.active].px[start];
        if from == self.color {
            return;
        }
        let mut stack = vec![(x, y)];
        while let Some((x, y)) = stack.pop() {
            let Some(i) = self.idx(x, y) else { continue };
            if self.layers[self.active].px[i] != from {
                continue;
            }
            self.layers[self.active].px[i] = self.color;
            stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
    }

    /// Pixels of the shape being dragged (line, rectangle, circle).
    fn shape(&self, a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        match self.tool {
            PTool::Line => line(a, b),
            PTool::Rect => {
                let mut v = Vec::new();
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        if self.shape_fill || x == x0 || x == x1 || y == y0 || y == y1 {
                            v.push((x, y));
                        }
                    }
                }
                v
            }
            PTool::Circle => {
                let cx = (x0 + x1) as f32 / 2.0;
                let cy = (y0 + y1) as f32 / 2.0;
                let rx = ((x1 - x0) as f32 / 2.0).max(0.5);
                let ry = ((y1 - y0) as f32 / 2.0).max(0.5);
                let mut v = Vec::new();
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let d = ((x as f32 - cx) / (rx + 0.5)).powi(2) + ((y as f32 - cy) / (ry + 0.5)).powi(2);
                        let inner = ((x as f32 - cx) / (rx - 0.5).max(0.01)).powi(2) + ((y as f32 - cy) / (ry - 0.5).max(0.01)).powi(2);
                        if d <= 1.0 && (self.shape_fill || inner >= 1.0) {
                            v.push((x, y));
                        }
                    }
                }
                v
            }
            _ => Vec::new(),
        }
    }

    fn resize(&mut self, size: u32) {
        if size == self.size {
            return;
        }
        self.snapshot();
        let old = self.size;
        for l in &mut self.layers {
            let mut px = vec![[0u8; 4]; (size * size) as usize];
            for y in 0..size {
                for x in 0..size {
                    px[(y * size + x) as usize] = l.px[((y * old / size) * old + x * old / size) as usize];
                }
            }
            l.px = px;
        }
        self.size = size;
    }
}

fn scale(c: u8, k: f32) -> u8 {
    (c as f32 * k).round().clamp(0.0, 255.0) as u8
}

fn blend(dst: Rgba, src: Rgba) -> Rgba {
    let a = src[3] as f32 / 255.0;
    if a <= 0.0 {
        return dst;
    }
    let da = dst[3] as f32 / 255.0;
    let oa = a + da * (1.0 - a);
    let mix = |s: u8, d: u8| ((s as f32 * a + d as f32 * da * (1.0 - a)) / oa.max(1e-4)).round() as u8;
    [mix(src[0], dst[0]), mix(src[1], dst[1]), mix(src[2], dst[2]), (oa * 255.0).round() as u8]
}

fn line(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (mut x, mut y) = a;
    let dx = (b.0 - a.0).abs();
    let dy = -(b.1 - a.1).abs();
    let sx = if a.0 < b.0 { 1 } else { -1 };
    let sy = if a.1 < b.1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut v = Vec::new();
    loop {
        v.push((x, y));
        if (x, y) == b || v.len() > 4096 {
            return v;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

fn small_button(ui: &mut Ui, p: &Palette, label: &str, on: bool, w: f32, h: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let fill = if on {
        p.accent.gamma_multiply(0.28)
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        p.surface_hover
    } else {
        p.bg
    };
    ui.painter().rect_filled(r, Rounding::same(6.0), fill);
    if on {
        ui.painter().rect_stroke(r, Rounding::same(6.0), Stroke::new(1.0_f32, p.accent));
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), if on { p.accent } else { p.text });
    resp
}

/// Draws the editor over the whole screen. Returns what the game should do.
pub fn draw(ctx: &egui::Context, _icons: &GuiIcons, p: &Palette, st: &mut PaintState) -> Vec<PaintAction> {
    let mut out = Vec::new();
    let screen = ctx.screen_rect();
    let landscape = screen.width() > screen.height();
    const HEAD_H: f32 = 34.0;
    const DOCK_H: f32 = 40.0;
    let left_w = if landscape { 74.0 } else { 58.0 };
    let right_w = if landscape { 130.0 } else { 104.0 };
    let head = Rect::from_min_size(screen.min, Vec2::new(screen.width(), HEAD_H));
    let dock = Rect::from_min_size(screen.min + Vec2::new(0.0, HEAD_H), Vec2::new(screen.width(), DOCK_H));
    let body_top = dock.bottom();
    let left = Rect::from_min_max(Pos2::new(screen.left(), body_top), Pos2::new(screen.left() + left_w, screen.bottom()));
    let right = Rect::from_min_max(Pos2::new(screen.right() - right_w, body_top), screen.max);
    let middle = Rect::from_min_max(Pos2::new(left.right(), body_top), Pos2::new(right.left(), screen.bottom()));

    egui::Area::new(Id::new("paint-editor")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        ui.set_width(screen.width());
        ui.set_height(screen.height());
        let painter = ui.painter().clone();
        painter.rect_filled(screen, 0.0, p.bg);
        painter.rect_filled(head, 0.0, p.surface);
        painter.rect_filled(dock, 0.0, p.surface.gamma_multiply(0.85));
        painter.rect_filled(left, 0.0, p.surface);
        painter.rect_filled(right, 0.0, p.surface);
        painter.line_segment([dock.left_bottom(), dock.right_bottom()], Stroke::new(1.0_f32, p.outline));

        // ---- Header ----
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(head.shrink2(Vec2::new(6.0, 3.0))), |ui| {
            egui::ScrollArea::horizontal().id_salt("paint-head").show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(egui::RichText::new("Текстура").font(bold_font(14.0)).color(p.text));
                    ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(90.0));
                    for s in [16u32, 32] {
                        if small_button(ui, p, &format!("{s}×{s}"), st.size == s, 50.0, 26.0).clicked() {
                            st.resize(s);
                        }
                    }
                    if small_button(ui, p, "↶", false, 30.0, 26.0).clicked() {
                        st.undo();
                    }
                    if small_button(ui, p, "↷", false, 30.0, 26.0).clicked() {
                        st.redo();
                    }
                    if small_button(ui, p, "Очистить", false, 70.0, 26.0).clicked() {
                        st.snapshot();
                        let a = st.active;
                        st.layers[a].px.iter_mut().for_each(|c| *c = [0; 4]);
                    }
                    if small_button(ui, p, "▦ В блоки", false, 80.0, 26.0).clicked() {
                        out.push(PaintAction::ToBlocks { size: st.size, rgba: st.composite() });
                    }
                    if small_button(ui, p, "▣ Как блок", false, 80.0, 26.0).clicked() {
                        out.push(PaintAction::Save { name: st.name.clone(), size: st.size, rgba: st.composite(), item: false });
                    }
                    if small_button(ui, p, "⚔ Как предмет", false, 96.0, 26.0).clicked() {
                        out.push(PaintAction::Save { name: st.name.clone(), size: st.size, rgba: st.composite(), item: true });
                    }
                    if small_button(ui, p, "✕ Закрыть", false, 80.0, 26.0).clicked() {
                        st.open = false;
                    }
                });
            });
        });

        // ---- Options dock: tool settings and colours ----
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(dock.shrink2(Vec2::new(6.0, 4.0))), |ui| {
            egui::ScrollArea::horizontal().id_salt("paint-dock").show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let mut c = c32(st.color);
                    if egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::OnlyBlend).changed() {
                        let [r, g, b, a] = c.to_srgba_unmultiplied();
                        st.color = [r, g, b, a];
                    }
                    for s in SWATCHES {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                        ui.painter().rect_filled(r, Rounding::same(4.0), c32(s));
                        if s == st.color {
                            ui.painter().rect_stroke(r.expand(1.0), Rounding::same(4.0), Stroke::new(2.0_f32, p.accent));
                        }
                        if resp.clicked() {
                            st.color = s;
                        }
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Размер").color(p.text_muted).size(11.0));
                    ui.add(egui::Slider::new(&mut st.brush, 1..=6).show_value(true));
                    if small_button(ui, p, "⇋ Зеркало", st.mirror, 78.0, 26.0).clicked() {
                        st.mirror = !st.mirror;
                    }
                    if small_button(ui, p, "# Сетка", st.grid, 64.0, 26.0).clicked() {
                        st.grid = !st.grid;
                    }
                    if small_button(ui, p, "■ Залить фигуру", st.shape_fill, 110.0, 26.0).clicked() {
                        st.shape_fill = !st.shape_fill;
                    }
                });
            });
        });

        // ---- Tools (left) ----
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left.shrink(4.0)), |ui| {
            egui::ScrollArea::vertical().id_salt("paint-tools").show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    for (t, ic, name) in TOOLS {
                        let on = st.tool == t;
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(left.width() - 8.0, 42.0), Sense::click());
                        ui.painter().rect_filled(r, Rounding::same(6.0), if on { p.accent.gamma_multiply(0.28) } else if resp.hovered() { p.surface_hover } else { Color32::TRANSPARENT });
                        let col = if on { p.accent } else { p.text };
                        ui.painter().text(r.center() - Vec2::new(0.0, 7.0), Align2::CENTER_CENTER, ic, FontId::proportional(17.0), col);
                        ui.painter().text(r.center() + Vec2::new(0.0, 12.0), Align2::CENTER_CENTER, name, FontId::proportional(9.5), col);
                        if resp.clicked() {
                            st.tool = t;
                        }
                    }
                });
            });
        });

        // ---- Layer slots (right) ----
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right.shrink(4.0)), |ui| {
            ui.label(egui::RichText::new("Слои").font(bold_font(13.0)).color(p.text));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
                let w = (right.width() - 20.0) / 3.0;
                if small_button(ui, p, "+", false, w, 24.0).clicked() {
                    st.snapshot();
                    let n = st.layers.len() + 1;
                    st.layers.insert(st.active + 1, Layer { name: format!("Слой {n}"), px: vec![[0; 4]; (st.size * st.size) as usize], visible: true });
                    st.active += 1;
                }
                if small_button(ui, p, "🗑", false, w, 24.0).clicked() && st.layers.len() > 1 {
                    st.snapshot();
                    st.layers.remove(st.active);
                    st.active = st.active.min(st.layers.len() - 1);
                }
                if small_button(ui, p, "⤓", false, w, 24.0).on_hover_text("Слить с нижним").clicked() && st.active > 0 {
                    st.snapshot();
                    let top = st.layers.remove(st.active);
                    st.active -= 1;
                    let a = st.active;
                    for (d, s) in st.layers[a].px.iter_mut().zip(top.px) {
                        *d = blend(*d, s);
                    }
                }
                if small_button(ui, p, "▲", false, w, 24.0).clicked() && st.active + 1 < st.layers.len() {
                    st.snapshot();
                    st.layers.swap(st.active, st.active + 1);
                    st.active += 1;
                }
                if small_button(ui, p, "▼", false, w, 24.0).clicked() && st.active > 0 {
                    st.snapshot();
                    st.layers.swap(st.active, st.active - 1);
                    st.active -= 1;
                }
                if small_button(ui, p, "⧉", false, w, 24.0).on_hover_text("Копия слоя").clicked() {
                    st.snapshot();
                    let mut l = st.layers[st.active].clone();
                    l.name += " копия";
                    st.layers.insert(st.active + 1, l);
                    st.active += 1;
                }
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical().id_salt("paint-layers").show(ui, |ui| {
                // Top layer first, like in drawing apps.
                for i in (0..st.layers.len()).rev() {
                    let on = st.active == i;
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(right.width() - 8.0, 40.0), Sense::click());
                    ui.painter().rect_filled(r, Rounding::same(6.0), if on { p.accent.gamma_multiply(0.25) } else { p.bg });
                    // Thumbnail.
                    let th = Rect::from_min_size(r.min + Vec2::new(4.0, 4.0), Vec2::splat(32.0));
                    ui.painter().rect_filled(th, 0.0, Color32::from_gray(200));
                    let n = st.size as f32;
                    for (k, c) in st.layers[i].px.iter().enumerate() {
                        if c[3] > 0 {
                            let x = (k as u32 % st.size) as f32;
                            let y = (k as u32 / st.size) as f32;
                            ui.painter().rect_filled(Rect::from_min_size(th.min + Vec2::new(x, y) * (32.0 / n), Vec2::splat(32.0 / n + 0.3)), 0.0, c32(*c));
                        }
                    }
                    ui.painter().text(Pos2::new(th.right() + 6.0, r.center().y), Align2::LEFT_CENTER, &st.layers[i].name, FontId::proportional(11.0), p.text);
                    let eye = Rect::from_center_size(Pos2::new(r.right() - 14.0, r.center().y), Vec2::splat(22.0));
                    ui.painter().text(eye.center(), Align2::CENTER_CENTER, if st.layers[i].visible { "👁" } else { "–" }, FontId::proportional(13.0), p.text_muted);
                    if resp.clicked() {
                        let tap = resp.interact_pointer_pos().unwrap_or(r.center());
                        if eye.expand(4.0).contains(tap) {
                            st.layers[i].visible = !st.layers[i].visible;
                        } else {
                            st.active = i;
                        }
                    }
                    ui.add_space(3.0);
                }
            });
        });

        // ---- Canvas ----
        let side = (middle.width().min(middle.height()) - 16.0).max(64.0);
        let canvas = Rect::from_center_size(middle.center(), Vec2::splat(side));
        let n = st.size as i32;
        let cell = side / n as f32;
        let resp = ui.allocate_rect(canvas, Sense::click_and_drag());
        let painter = ui.painter_at(canvas);
        // Checkerboard under see-through pixels.
        for y in 0..n {
            for x in 0..n {
                let g = if (x + y) % 2 == 0 { 205 } else { 170 };
                painter.rect_filled(Rect::from_min_size(canvas.min + Vec2::new(x as f32, y as f32) * cell, Vec2::splat(cell)), 0.0, Color32::from_gray(g));
            }
        }
        for (k, c) in st.composite().iter().enumerate() {
            if c[3] > 0 {
                let x = (k as i32 % n) as f32;
                let y = (k as i32 / n) as f32;
                painter.rect_filled(Rect::from_min_size(canvas.min + Vec2::new(x, y) * cell, Vec2::splat(cell + 0.4)), 0.0, c32(*c));
            }
        }
        if st.grid {
            let s = Stroke::new(1.0_f32, Color32::from_black_alpha(40));
            for i in 1..n {
                let t = i as f32 * cell;
                painter.line_segment([Pos2::new(canvas.left() + t, canvas.top()), Pos2::new(canvas.left() + t, canvas.bottom())], s);
                painter.line_segment([Pos2::new(canvas.left(), canvas.top() + t), Pos2::new(canvas.right(), canvas.top() + t)], s);
            }
        }
        painter.rect_stroke(canvas, 0.0, Stroke::new(1.5_f32, p.outline));
        if st.mirror {
            painter.line_segment([Pos2::new(canvas.center().x, canvas.top()), Pos2::new(canvas.center().x, canvas.bottom())], Stroke::new(1.0_f32, p.accent.gamma_multiply(0.6)));
        }

        let to_px = |pos: Pos2| -> (i32, i32) { (((pos.x - canvas.left()) / cell).floor() as i32, ((pos.y - canvas.top()) / cell).floor() as i32) };
        let shape_tool = matches!(st.tool, PTool::Line | PTool::Rect | PTool::Circle);
        if let Some(pos) = resp.interact_pointer_pos() {
            let at = to_px(pos);
            let started = resp.drag_started() || (resp.clicked() && st.drag_from.is_none());
            match st.tool {
                PTool::Picker => {
                    if started || resp.dragged() {
                        if let Some(i) = st.idx(at.0, at.1) {
                            let c = st.composite()[i];
                            if c[3] > 0 {
                                st.color = c;
                            }
                        }
                    }
                    if resp.clicked() || resp.drag_stopped() {
                        st.tool = PTool::Pencil;
                    }
                }
                PTool::Fill => {
                    if started {
                        st.snapshot();
                        st.fill(at.0, at.1);
                    }
                }
                _ if shape_tool => {
                    if started {
                        st.drag_from = Some(at);
                    }
                    if let Some(a) = st.drag_from {
                        if resp.drag_stopped() || resp.clicked() {
                            st.snapshot();
                            for (x, y) in st.shape(a, at) {
                                st.apply(x, y);
                            }
                            st.drag_from = None;
                        } else {
                            for (x, y) in st.shape(a, at) {
                                if st.idx(x, y).is_some() {
                                    painter.rect_filled(Rect::from_min_size(canvas.min + Vec2::new(x as f32, y as f32) * cell, Vec2::splat(cell)), 0.0, c32(st.color).gamma_multiply(0.7));
                                }
                            }
                        }
                    }
                }
                _ => {
                    if started {
                        st.snapshot();
                        st.last = None;
                    }
                    if started || resp.dragged() || resp.clicked() {
                        let from = st.last.unwrap_or(at);
                        st.stroke(from, at);
                        st.last = Some(at);
                    }
                    if resp.drag_stopped() || resp.clicked() {
                        st.last = None;
                    }
                }
            }
        } else if !resp.dragged() {
            st.last = None;
            if !shape_tool {
                st.drag_from = None;
            }
        }
        // Hint under the canvas.
        let tool_name = TOOLS.iter().find(|t| t.0 == st.tool).map(|t| t.2).unwrap_or("");
        ui.painter().text(Pos2::new(middle.center().x, canvas.bottom() + 2.0), Align2::CENTER_TOP, format!("{tool_name} · {}×{} · слой «{}»", st.size, st.size, st.layers[st.active].name), FontId::proportional(10.5), p.text_muted);
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_fill_undo_and_layers() {
        let mut st = PaintState::default();
        st.snapshot();
        st.stroke((0, 0), (15, 0));
        assert!(st.layers[0].px[..16].iter().all(|c| *c == st.color));
        st.color = [255, 0, 0, 255];
        st.snapshot();
        st.fill(5, 5);
        assert_eq!(st.layers[0].px[16 * 8 + 8], [255, 0, 0, 255]);
        st.undo();
        assert_eq!(st.layers[0].px[16 * 8 + 8], [0; 4]);
        // A half see-through top layer blends over the bottom one.
        st.layers.push(Layer { name: "2".into(), px: vec![[0, 0, 255, 128]; 256], visible: true });
        let c = st.composite()[0];
        assert!(c[2] > 100 && c[1] < 120);
        st.resize(32);
        assert_eq!(st.layers[0].px.len(), 32 * 32);
        st.tool = PTool::Circle;
        assert!(!st.shape((2, 2), (10, 10)).is_empty());
    }
}
