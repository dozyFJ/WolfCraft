//! GUI renderer: egui tessellation batched into a few draw calls and rendered with wgpu,
//! plus the sprite atlases (GUI icons, item icons, block icons) registered as egui textures.
//!
//! Atlas textures are uploaded once at startup (never per frame). Geometry is cached by egui's
//! tessellator between frames when nothing changes.

use egui::{Color32, Rect, TextureHandle, TextureId, TextureOptions, Vec2, epaint};
use std::sync::Arc;
use winit::window::Window;
use wolfcraft_assets::{AtlasKind, AtlasManager};
use wolfcraft_engine::block::{Block, RenderKind};
use wolfcraft_engine::items::{ITEM_BASE, Icon, ItemId};

/// Handles to atlas textures inside egui plus sprite UV tables.
pub struct GuiIcons {
    gui: TextureHandle,
    items: TextureHandle,
    blocks: TextureHandle,
    gui_uv: Vec<Rect>,
    item_uv: Vec<Rect>,
    block_uv: Vec<Rect>,
}

fn uv_table(a: &wolfcraft_assets::TextureAtlas) -> Vec<Rect> {
    (0..a.len())
        .map(|i| {
            let uv = a.uv(i);
            Rect::from_min_max(egui::pos2(uv.min[0], uv.min[1]), egui::pos2(uv.max[0], uv.max[1]))
        })
        .collect()
}

fn upload(ctx: &egui::Context, name: &str, a: &wolfcraft_assets::TextureAtlas, opts: TextureOptions) -> TextureHandle {
    let img = a.image();
    ctx.load_texture(name, egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.pixels), opts)
}

impl GuiIcons {
    pub fn new(ctx: &egui::Context, atlases: &AtlasManager) -> Self {
        let gui = atlases.get(AtlasKind::Gui);
        let items = atlases.get(AtlasKind::Items);
        let blocks = atlases.get(AtlasKind::Blocks);
        // GUI icons are anti-aliased vectors в†’ linear; item/block art is pixel art в†’ nearest.
        Self {
            gui: upload(ctx, "gui_atlas", gui, TextureOptions::NEAREST),
            items: upload(ctx, "items_atlas", items, TextureOptions::NEAREST),
            blocks: upload(ctx, "blocks_atlas", blocks, TextureOptions::NEAREST),
            gui_uv: uv_table(gui),
            item_uv: uv_table(items),
            block_uv: uv_table(blocks),
        }
    }

    /// Draws a GUI atlas icon tinted with `tint`.
    pub fn paint(&self, painter: &egui::Painter, rect: Rect, icon: u16, tint: Color32) {
        if let Some(uv) = self.gui_uv.get(icon as usize) {
            painter.image(self.gui.id(), rect, *uv, tint);
        }
    }

    /// Draws only the left `fraction` (0..1) of an icon, e.g. a half heart.
    pub fn paint_partial(&self, painter: &egui::Painter, rect: Rect, icon: u16, tint: Color32, fraction: f32) {
        let f = fraction.clamp(0.0, 1.0);
        if f <= 0.0 {
            return;
        }
        if let Some(uv) = self.gui_uv.get(icon as usize) {
            let r = Rect::from_min_max(rect.min, egui::pos2(rect.min.x + rect.width() * f, rect.max.y));
            let u = Rect::from_min_max(uv.min, egui::pos2(uv.min.x + uv.width() * f, uv.max.y));
            painter.image(self.gui.id(), r, u, tint);
        }
    }

    /// Draws an item or block icon from the proper atlas (pixel-perfect, nearest filtering).
    pub fn paint_item(&self, painter: &egui::Painter, rect: Rect, item: ItemId) {
        // Full blocks are drawn as a small 3D cube; plants and items stay flat sprites.
        if item < ITEM_BASE {
            let bi = Block::from_u8(item as u8).info();
            if !matches!(bi.render, RenderKind::Cross | RenderKind::Invisible) {
                let block = Block::from_u8(item as u8);
                let boxes = block.boxes();
                if boxes.is_empty() {
                    self.paint_cube(painter, rect, bi.tiles[2], bi.tiles[4], bi.tiles[0]);
                } else {
                    // Stairs, slabs, doors, panes, fences: their real shape.
                    self.paint_boxes(painter, rect, boxes, bi.tiles[2], bi.tiles[4], bi.tiles[0]);
                }
                return;
            }
        }
        match wolfcraft_engine::items::info(item).icon {
            Icon::Block(t) => {
                if let Some(uv) = self.block_uv.get(t as usize) {
                    painter.image(self.blocks.id(), rect, *uv, Color32::WHITE);
                }
            }
            Icon::Item(t) => {
                if let Some(uv) = self.item_uv.get(t as usize) {
                    painter.image(self.items.id(), rect, *uv, Color32::WHITE);
                }
            }
        }
    }

    /// Isometric cube: lit top, medium left (front) side, dark right side.
    pub fn paint_cube(&self, painter: &egui::Painter, rect: Rect, top: u16, left: u16, right: u16) {
        let s = rect.width().min(rect.height());
        let c = rect.center();
        let (w, v) = (s * 0.44, s * 0.48);
        let h = w * 0.55;
        let y0 = c.y - (2.0 * h + v) / 2.0;
        let n = egui::pos2(c.x, y0);
        let e = egui::pos2(c.x + w, y0 + h);
        let so = egui::pos2(c.x, y0 + 2.0 * h);
        let we = egui::pos2(c.x - w, y0 + h);
        let d = Vec2::new(0.0, v);
        let mut mesh = egui::Mesh::with_texture(self.blocks.id());
        let mut quad = |p: [egui::Pos2; 4], tile: u16, col: Color32| {
            let Some(uv) = self.block_uv.get(tile as usize) else { return };
            let i = mesh.vertices.len() as u32;
            for (pos, uv) in p.into_iter().zip([uv.left_top(), uv.right_top(), uv.right_bottom(), uv.left_bottom()]) {
                mesh.vertices.push(epaint::Vertex { pos, uv, color: col });
            }
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        };
        quad([n, e, so, we], top, Color32::WHITE);
        quad([we, so, so + d, we + d], left, Color32::from_gray(190));
        quad([so, e, e + d, so + d], right, Color32::from_gray(125));
        painter.add(egui::Shape::mesh(mesh));
        // Dark outline and a bright top edge: reads as a crisp pixel-art cube.
        let ink = egui::Stroke::new((s / 24.0).max(1.0).round(), Color32::from_rgb(24, 20, 28));
        painter.add(egui::Shape::closed_line(vec![n, e, e + d, so + d, we + d, we], ink));
        let thin = egui::Stroke::new((ink.width * 0.6).max(1.0), Color32::from_black_alpha(140));
        painter.add(egui::Shape::line_segment([we, so], thin));
        painter.add(egui::Shape::line_segment([so, e], thin));
        painter.add(egui::Shape::line_segment([so, so + d], thin));
        painter.add(egui::Shape::line_segment([n, e], egui::Stroke::new(thin.width, Color32::from_white_alpha(120))));
    }

    /// Isometric view of a shaped block made of boxes `[i0, j0, k0, i1, j1, k1]` (k is up), in
    /// the same projection as [`Self::paint_cube`]; boxes are drawn back to front.
    pub fn paint_boxes(&self, painter: &egui::Painter, rect: Rect, boxes: &[[f32; 6]], top: u16, left: u16, right: u16) {
        let s = rect.width().min(rect.height());
        let c = rect.center();
        let (w, v) = (s * 0.44, s * 0.48);
        let h = w * 0.55;
        let y0 = c.y - (2.0 * h + v) / 2.0;
        let pt = |i: f32, j: f32, k: f32| egui::pos2(c.x + (i - j) * w, y0 + (i + j) * h + (1.0 - k) * v);
        let mut order: Vec<&[f32; 6]> = boxes.iter().collect();
        order.sort_by(|a, b| (a[0] + a[1] + a[3] + a[4] - a[2] * 0.01).partial_cmp(&(b[0] + b[1] + b[3] + b[4] - b[2] * 0.01)).unwrap_or(std::cmp::Ordering::Equal));
        let ink = egui::Stroke::new((s / 32.0).max(1.0), Color32::from_rgb(24, 20, 28));
        for b in order {
            let [i0, j0, k0, i1, j1, k1] = *b;
            let mut mesh = egui::Mesh::with_texture(self.blocks.id());
            let mut quad = |p: [egui::Pos2; 4], tile: u16, u: (f32, f32), vv: (f32, f32), col: Color32| {
                let Some(uv) = self.block_uv.get(tile as usize) else { return };
                let at = |a: f32, b: f32| egui::pos2(uv.min.x + uv.width() * a, uv.min.y + uv.height() * b);
                let i = mesh.vertices.len() as u32;
                for (pos, t) in p.into_iter().zip([at(u.0, vv.0), at(u.1, vv.0), at(u.1, vv.1), at(u.0, vv.1)]) {
                    mesh.vertices.push(epaint::Vertex { pos, uv: t, color: col });
                }
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i, i + 2, i + 3);
            };
            quad([pt(i0, j0, k1), pt(i1, j0, k1), pt(i1, j1, k1), pt(i0, j1, k1)], top, (i0, i1), (j0, j1), Color32::WHITE);
            quad([pt(i0, j1, k1), pt(i1, j1, k1), pt(i1, j1, k0), pt(i0, j1, k0)], left, (i0, i1), (1.0 - k1, 1.0 - k0), Color32::from_gray(190));
            quad([pt(i1, j1, k1), pt(i1, j0, k1), pt(i1, j0, k0), pt(i1, j1, k0)], right, (1.0 - j1, 1.0 - j0), (1.0 - k1, 1.0 - k0), Color32::from_gray(125));
            painter.add(egui::Shape::mesh(mesh));
            painter.add(egui::Shape::closed_line(vec![pt(i0, j0, k1), pt(i1, j0, k1), pt(i1, j0, k0), pt(i1, j1, k0), pt(i0, j1, k0), pt(i0, j1, k1)], ink));
        }
    }

    /// Draws a sprite from the blocks atlas by id (used for textured block previews).
    pub fn paint_block_tile(&self, painter: &egui::Painter, rect: Rect, tile: u16) {
        if let Some(uv) = self.block_uv.get(tile as usize) {
            painter.image(self.blocks.id(), rect, *uv, Color32::WHITE);
        }
    }

    pub fn gui_texture(&self) -> TextureId {
        self.gui.id()
    }
}

pub struct GuiRenderer {
    pub ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    pub icons: Arc<GuiIcons>,
    ui_scale: f32,
    pub last_vertices: usize,
    pub last_meshes: usize,
    /// Last non-empty frame, shown again if egui ever hands back an empty one (no blinking).
    last_jobs: Vec<egui::ClippedPrimitive>,
    /// The game has captured the mouse: keep the OS cursor hidden (egui would show it again).
    pub hide_cursor: bool,
}

pub struct GuiFrame {
    paint_jobs: Vec<egui::ClippedPrimitive>,
    textures_delta: egui::TexturesDelta,
    pixels_per_point: f32,
}

/// Screen margins in points (left, top, right, bottom) kept free of system bars on phones.
pub static SAFE_INSETS: std::sync::RwLock<[f32; 4]> = std::sync::RwLock::new([0.0; 4]);

impl GuiRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat, window: &Window, atlases: &AtlasManager) -> Self {
        let ctx = egui::Context::default();
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, window, Some(window.scale_factor() as f32), None, None);
        let renderer = egui_wgpu::Renderer::new(device, color_format, None, 1, false);
        let icons = Arc::new(GuiIcons::new(&ctx, atlases));
        Self { ctx, state, renderer, icons, ui_scale: 1.0, last_vertices: 0, last_meshes: 0, last_jobs: Vec::new(), hide_cursor: false }
    }

    /// Feeds a window event to egui; returns true if egui wants it (UI consumed the input).
    pub fn on_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> egui_winit::EventResponse {
        self.state.on_window_event(window, event)
    }

    pub fn set_ui_scale(&mut self, scale: f32) {
        let s = scale.clamp(0.6, 2.5);
        if (s - self.ui_scale).abs() > f32::EPSILON {
            self.ui_scale = s;
            self.ctx.set_zoom_factor(s);
        }
    }

    pub fn wants_pointer(&self) -> bool {
        self.ctx.wants_pointer_input()
    }
    pub fn wants_keyboard(&self) -> bool {
        self.ctx.wants_keyboard_input()
    }

    /// Runs the UI closure and tessellates the result.
    pub fn run(&mut self, window: &Window, mut add_ui: impl FnMut(&egui::Context)) -> GuiFrame {
        let mut input = self.state.take_egui_input(window);
        if let (Ok(i), Some(r)) = (SAFE_INSETS.read(), input.screen_rect.as_mut()) {
            *r = egui::Rect::from_min_max(r.min + egui::vec2(i[0], i[1]), r.max - egui::vec2(i[2], i[3]));
        }
        // egui may run the closure several times per frame (a pass can be discarded when layout
        // changes). Every pass must draw the UI, otherwise the frame comes out empty and the
        // whole interface blinks.
        let output = self.ctx.run(input, |c| add_ui(c));
        let mut platform = output.platform_output;
        if self.hide_cursor {
            platform.cursor_icon = egui::CursorIcon::None;
        }
        self.state.handle_platform_output(window, platform);
        let ppp = output.pixels_per_point;
        let mut paint_jobs = self.ctx.tessellate(output.shapes, ppp);
        if paint_jobs.is_empty() {
            log::warn!("empty gui frame (ppp {ppp}): showing the previous one");
            paint_jobs = self.last_jobs.clone();
        } else {
            self.last_jobs = paint_jobs.clone();
        }
        self.last_meshes = paint_jobs.len();
        self.last_vertices = paint_jobs
            .iter()
            .map(|p| match &p.primitive {
                epaint::Primitive::Mesh(m) => m.vertices.len(),
                _ => 0,
            })
            .sum();
        GuiFrame { paint_jobs, textures_delta: output.textures_delta, pixels_per_point: ppp }
    }

    /// Records the UI pass on top of whatever is in `view`. Returns extra command buffers that
    /// must be submitted before the main encoder.
    pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size_px: [u32; 2], frame: GuiFrame) -> Vec<wgpu::CommandBuffer> {
        for (id, delta) in &frame.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size_px, pixels_per_point: frame.pixels_per_point };
        let extra = self.renderer.update_buffers(device, queue, encoder, &frame.paint_jobs, &screen);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gui-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &frame.paint_jobs, &screen);
        }
        for id in &frame.textures_delta.free {
            self.renderer.free_texture(id);
        }
        extra
    }
}

/// Convenience: a square rect of `size` centred in `rect`.
pub fn centered_square(rect: Rect, size: f32) -> Rect {
    Rect::from_center_size(rect.center(), Vec2::splat(size))
}
