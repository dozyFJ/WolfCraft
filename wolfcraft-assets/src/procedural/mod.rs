//! Procedural painters for every sprite. All art is original and generated deterministically.

mod blocks;
mod entities_paint;
mod gui_paint;
mod items_paint;
mod particles_paint;

use crate::atlas_manager::AtlasKind;
use crate::image::Image;

pub use blocks::{tiles, water_frame};
pub use entities_paint::entities;
pub use gui_paint::gui;
pub use items_paint::items;
pub use particles_paint::particles;

/// Sprites of a category in id order.
pub fn sprites(kind: AtlasKind) -> Vec<(String, Image)> {
    match kind {
        AtlasKind::Blocks => blocks::build(),
        AtlasKind::Items => items_paint::build(),
        AtlasKind::Entities => entities_paint::build(),
        AtlasKind::Gui => gui_paint::build(),
        AtlasKind::Particles => particles_paint::build(),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rgb(pub f32, pub f32, pub f32);

impl Rgb {
    pub fn scale(self, s: f32) -> Rgb {
        Rgb(self.0 * s, self.1 * s, self.2 * s)
    }
    pub fn add(self, n: f32) -> Rgb {
        Rgb(self.0 + n, self.1 + n, self.2 + n)
    }
    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        Rgb(self.0 + (o.0 - self.0) * t, self.1 + (o.1 - self.1) * t, self.2 + (o.2 - self.2) * t)
    }
}

pub fn hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ seed.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

pub fn rnd(x: u32, y: u32, seed: u32) -> f32 {
    (hash(x, y, seed) & 0xFFFF) as f32 / 65535.0
}

/// Pixel painter over a sprite image.
pub struct Painter {
    pub img: Image,
}

impl Painter {
    pub fn new(w: u32, h: u32) -> Self {
        Self { img: Image::new(w, h) }
    }
    pub fn w(&self) -> u32 {
        self.img.width
    }
    pub fn h(&self) -> u32 {
        self.img.height
    }

    pub fn put(&mut self, x: u32, y: u32, c: Rgb, a: f32) {
        if x >= self.img.width || y >= self.img.height {
            return;
        }
        let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        self.img.set(x, y, [f(c.0), f(c.1), f(c.2), f(a)]);
    }

    /// Alpha-composites `c` with coverage `a` over the existing pixel.
    pub fn blend(&mut self, x: u32, y: u32, c: Rgb, a: f32) {
        if x >= self.img.width || y >= self.img.height || a <= 0.0 {
            return;
        }
        let d = self.img.get(x, y);
        let da = d[3] as f32 / 255.0;
        let oa = a + da * (1.0 - a);
        let mix = |s: f32, dc: u8| {
            if oa <= 0.0 { 0.0 } else { (s * a + dc as f32 / 255.0 * da * (1.0 - a)) / oa }
        };
        let col = Rgb(mix(c.0, d[0]), mix(c.1, d[1]), mix(c.2, d[2]));
        self.put(x, y, col, oa);
    }

    pub fn noisy(&mut self, base: Rgb, amount: f32, seed: u32) {
        for y in 0..self.h() {
            for x in 0..self.w() {
                let n = (rnd(x, y, seed) - 0.5) * amount;
                self.put(x, y, base.add(n), 1.0);
            }
        }
    }

    pub fn speckle(&mut self, color: Rgb, density: f32, seed: u32) {
        for y in 0..self.h() {
            for x in 0..self.w() {
                if rnd(x / 2, y / 2, seed) < density && rnd(x, y, seed ^ 77) < 0.75 {
                    let n = (rnd(x, y, seed ^ 5) - 0.5) * 0.15;
                    self.put(x, y, color.add(n), 1.0);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.img.pixels.fill(0);
    }

    /// Paints an ASCII template (one char per pixel).
    pub fn template(&mut self, rows: &[&str], palette: &dyn Fn(char) -> Option<Rgb>) {
        self.clear();
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if let Some(c) = palette(ch) {
                    self.put(x as u32, y as u32, c, 1.0);
                }
            }
        }
    }

    /// Anti-aliased shape fill: `inside(u, v)` gets coordinates in [0,1]² (y down).
    /// Coverage is computed with 4x4 supersampling.
    pub fn shape(&mut self, color: Rgb, inside: &dyn Fn(f32, f32) -> bool) {
        let (w, h) = (self.w() as f32, self.h() as f32);
        for y in 0..self.h() {
            for x in 0..self.w() {
                let mut cov = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let u = (x as f32 + (sx as f32 + 0.5) / 4.0) / w;
                        let v = (y as f32 + (sy as f32 + 0.5) / 4.0) / h;
                        if inside(u, v) {
                            cov += 1;
                        }
                    }
                }
                if cov > 0 {
                    self.blend(x, y, color, cov as f32 / 16.0);
                }
            }
        }
    }

    /// Like `shape`, but erases (sets alpha to 0) where inside.
    pub fn erase(&mut self, inside: &dyn Fn(f32, f32) -> bool) {
        let (w, h) = (self.w() as f32, self.h() as f32);
        for y in 0..self.h() {
            for x in 0..self.w() {
                let mut cov = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let u = (x as f32 + (sx as f32 + 0.5) / 4.0) / w;
                        let v = (y as f32 + (sy as f32 + 0.5) / 4.0) / h;
                        if inside(u, v) {
                            cov += 1;
                        }
                    }
                }
                if cov > 0 {
                    let mut p = self.img.get(x, y);
                    p[3] = (p[3] as f32 * (1.0 - cov as f32 / 16.0)) as u8;
                    self.img.set(x, y, p);
                }
            }
        }
    }

    pub fn finish(self) -> Image {
        self.img
    }
}

// ---- small geometry helpers for vector-style icons ----

pub fn circle(cx: f32, cy: f32, r: f32) -> impl Fn(f32, f32) -> bool {
    move |u, v| (u - cx).powi(2) + (v - cy).powi(2) <= r * r
}

pub fn ring(cx: f32, cy: f32, r: f32, thickness: f32) -> impl Fn(f32, f32) -> bool {
    move |u, v| {
        let d = ((u - cx).powi(2) + (v - cy).powi(2)).sqrt();
        (d - r).abs() <= thickness * 0.5
    }
}

pub fn rrect(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> impl Fn(f32, f32) -> bool {
    move |u, v| {
        if u < x0 || u > x1 || v < y0 || v > y1 {
            return false;
        }
        let cx = u.clamp(x0 + r, x1 - r);
        let cy = v.clamp(y0 + r, y1 - r);
        (u - cx).powi(2) + (v - cy).powi(2) <= r * r
    }
}

/// Thick line segment with round caps.
pub fn segment(ax: f32, ay: f32, bx: f32, by: f32, thickness: f32) -> impl Fn(f32, f32) -> bool {
    move |u, v| {
        let (dx, dy) = (bx - ax, by - ay);
        let t = (((u - ax) * dx + (v - ay) * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
        let (px, py) = (ax + dx * t, ay + dy * t);
        (u - px).powi(2) + (v - py).powi(2) <= (thickness * 0.5).powi(2)
    }
}

/// Point-in-polygon (even-odd).
pub fn polygon(pts: Vec<(f32, f32)>) -> impl Fn(f32, f32) -> bool {
    move |u, v| {
        let mut inside = false;
        let n = pts.len();
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = pts[i];
            let (xj, yj) = pts[j];
            if (yi > v) != (yj > v) && u < (xj - xi) * (v - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

/// Helper that pushes sprites and checks ids match the declared constants.
pub(crate) struct SpriteList(pub Vec<(String, Image)>);

impl SpriteList {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn add(&mut self, id: u16, name: &str, img: Image) {
        assert_eq!(self.0.len(), id as usize, "sprite '{name}' declared with id {id} but pushed at {}", self.0.len());
        self.0.push((name.to_string(), img));
    }
}
