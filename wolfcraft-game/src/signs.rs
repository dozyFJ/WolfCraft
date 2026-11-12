//! Sign text drawn on the sign itself, in the world: the text is laid out with the game's UI
//! font, sampled into a grid of "ink" cells and turned into thin dark strips just in front of
//! the board, on both sides (each side reads left to right for whoever stands there).

use std::collections::HashMap;
use wolfcraft_engine::block::Block;
use wolfcraft_engine::cubesphere::{Cell, Grid};
use wolfcraft_engine::renderer::model::ModelVertex;

/// Ink cells across and down the writable part of the board (2:1 like the board).
const COLS: usize = 96;
const ROWS: usize = 48;
/// Signs further away than this show no text (blocks).
pub const TEXT_DISTANCE: f64 = 14.0;

/// Ink of one sign: for every row the runs of inked cells (x0, x1 exclusive).
type Ink = Vec<Vec<(u16, u16)>>;

#[derive(Default)]
pub struct SignTexts {
    cache: HashMap<String, Ink>,
}

impl SignTexts {
    fn ink(&mut self, ctx: &egui::Context, text: &str) -> &Ink {
        if !self.cache.contains_key(text) {
            let ink = rasterize(ctx, text);
            if self.cache.len() > 256 {
                self.cache.clear();
            }
            self.cache.insert(text.to_string(), ink);
        }
        &self.cache[text]
    }

    /// Model triangles for the text of every sign near `camera` (positions relative to it).
    pub fn build(&mut self, ctx: &egui::Context, grid: &Grid, signs: &[(Cell, Block, String)], camera: glam::DVec3) -> Vec<ModelVertex> {
        let mut out = Vec::new();
        for (cell, block, text) in signs {
            if text.trim().is_empty() {
                continue;
            }
            let centre = grid.cell_center(*cell);
            if (centre - camera).length() > TEXT_DISTANCE {
                continue;
            }
            let ink = self.ink(ctx, text.trim()).clone();
            emit(&mut out, grid, *cell, *block, &ink, camera);
        }
        out
    }
}

/// Lays the text out (wrapped, centred, shrunk until it fits) and samples the glyph coverage
/// into the ink grid.
fn rasterize(ctx: &egui::Context, text: &str) -> Ink {
    let (box_w, box_h) = (120.0f32, 60.0f32);
    let mut chosen = None;
    for size in [24.0f32, 20.0, 17.0, 14.0, 12.0, 10.0, 8.0] {
        let g = ctx.fonts(|f| f.layout(text.to_string(), egui::FontId::proportional(size), egui::Color32::WHITE, box_w));
        let fits = g.rect.height() <= box_h;
        chosen = Some(g);
        if fits {
            break;
        }
    }
    let Some(galley) = chosen else { return vec![Vec::new(); ROWS] };
    let image = ctx.fonts(|f| f.image());
    let (iw, ih) = (image.size[0], image.size[1]);
    // Centre the text block in the board area, every line on its own.
    let oy = ((box_h - galley.rect.height()) * 0.5).max(0.0) - galley.rect.min.y;
    let glyphs: Vec<(egui::Rect, epaint_uv::Uv)> = galley
        .rows
        .iter()
        .flat_map(|r| {
            let used = r.glyphs.iter().filter(|g| g.chr != ' ').fold((f32::MAX, f32::MIN), |(a, b), g| (a.min(g.pos.x), b.max(g.max_x())));
            let ox = if used.0 < used.1 { (box_w - (used.1 - used.0)) * 0.5 - used.0 } else { 0.0 };
            r.glyphs.iter().map(move |g| (g, egui::vec2(ox, oy)))
        })
        .filter(|(g, _)| !g.uv_rect.is_nothing())
        .map(|(g, off)| {
            let lt = g.pos + g.uv_rect.offset + off;
            (egui::Rect::from_min_size(lt, g.uv_rect.size), epaint_uv::Uv { min: g.uv_rect.min, max: g.uv_rect.max })
        })
        .collect();
    let coverage = |p: egui::Pos2| -> f32 {
        glyphs
            .iter()
            .filter(|(r, _)| r.contains(p))
            .map(|(r, uv)| {
                let fx = (p.x - r.min.x) / r.width().max(1e-3);
                let fy = (p.y - r.min.y) / r.height().max(1e-3);
                let tx = (uv.min[0] as f32 + fx * (uv.max[0] - uv.min[0]) as f32) as usize;
                let ty = (uv.min[1] as f32 + fy * (uv.max[1] - uv.min[1]) as f32) as usize;
                if tx < iw && ty < ih { image.pixels[ty * iw + tx] } else { 0.0 }
            })
            .fold(0.0, f32::max)
    };
    let mut ink: Ink = vec![Vec::new(); ROWS];
    for (ry, row) in ink.iter_mut().enumerate() {
        let mut run: Option<u16> = None;
        for cx in 0..=COLS {
            // 3×3 samples per cell: thin strokes are not lost between cell centres.
            let on = cx < COLS && {
                let (cw, ch) = (box_w / COLS as f32, box_h / ROWS as f32);
                let mut sum = 0.0;
                for sy in 0..3 {
                    for sx in 0..3 {
                        let p = egui::pos2((cx as f32 + (sx as f32 + 0.5) / 3.0) * cw, (ry as f32 + (sy as f32 + 0.5) / 3.0) * ch);
                        sum += coverage(p);
                    }
                }
                sum / 9.0 > 0.28
            };
            match (on, run) {
                (true, None) => run = Some(cx as u16),
                (false, Some(s)) => {
                    row.push((s, cx as u16));
                    run = None;
                }
                _ => {}
            }
        }
    }
    ink
}

mod epaint_uv {
    #[derive(Clone, Copy)]
    pub struct Uv {
        pub min: [u16; 2],
        pub max: [u16; 2],
    }
}

/// Strips for the ink on both faces of the board.
fn emit(out: &mut Vec<ModelVertex>, grid: &Grid, cell: Cell, block: Block, ink: &Ink, camera: glam::DVec3) {
    // Board: Sign spans i (faces ±j), SignB spans j (faces ±i); writable part of it.
    let along_i = block == Block::Sign;
    let (u0, u1, k0, k1) = (0.10f64, 0.90f64, 0.53f64, 0.92f64);
    let (t0, t1) = (0.44f64 - 0.004, 0.56f64 + 0.004);
    let at = |u: f64, t: f64, k: f64| {
        let (i, j) = if along_i { (u, t) } else { (t, u) };
        grid.point(cell.f, cell.i as f64 + i, cell.j as f64 + j, cell.k as f64 + k)
    };
    let up = (at(0.5, 0.5, 1.0) - at(0.5, 0.5, 0.0)).normalize();
    let e_u = (at(1.0, 0.5, 0.5) - at(0.0, 0.5, 0.5)).normalize();
    let e_t = (at(0.5, 1.0, 0.5) - at(0.5, 0.0, 0.5)).normalize();
    let ink_c = [0.10f32, 0.07, 0.05];
    for (t, out_dir) in [(t1, e_t), (t0, -e_t)] {
        // Viewer looks along -out_dir: screen-right = forward × up.
        let right = (-out_dir).cross(up);
        let flip = right.dot(e_u) < 0.0;
        let n = out_dir.as_vec3().to_array();
        for (ry, row) in ink.iter().enumerate() {
            let ka = k1 - (ry as f64) * (k1 - k0) / ROWS as f64;
            let kb = k1 - (ry as f64 + 1.0) * (k1 - k0) / ROWS as f64;
            for &(x0, x1) in row {
                let mut ua = u0 + x0 as f64 * (u1 - u0) / COLS as f64;
                let mut ub = u0 + x1 as f64 * (u1 - u0) / COLS as f64;
                if flip {
                    (ua, ub) = (1.0 - ub, 1.0 - ua);
                }
                let q = [at(ua, t, kb), at(ub, t, kb), at(ub, t, ka), at(ua, t, ka)].map(|p| (p - camera).as_vec3().to_array());
                for i in [0usize, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] {
                    out.push(ModelVertex { pos: q[i], normal: n, color: ink_c, uv: [0.0, 0.0], tex: 0.0 });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_text_turns_into_ink_on_the_board() {
        let ctx = egui::Context::default();
        wolfcraft_ui::theme_manager::install(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let ink = rasterize(&ctx, "Привет! Ферма тут");
        let cells: usize = ink.iter().flatten().map(|(a, b)| (b - a) as usize).sum();
        if std::env::var("SHOW_INK").is_ok() {
            for row in &ink {
                let mut line = vec![' '; COLS];
                for &(a, b) in row {
                    for x in a..b {
                        line[x as usize] = '#';
                    }
                }
                println!("{}", line.into_iter().collect::<String>());
            }
        }
        assert!(cells > 80, "text has ink ({cells})");
        assert!(ink.iter().filter(|r| !r.is_empty()).count() >= 6, "several rows tall");
    }
}
