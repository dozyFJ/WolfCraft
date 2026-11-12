//! Particles and effects (particles atlas): block-break cracks, dust, smoke, sparks, bubbles.

use super::{Painter, Rgb, SpriteList, circle, hash, ring, rnd};
use crate::image::Image;

pub mod particles {
    /// 8 crack stages: CRACK_BASE + stage.
    pub const CRACK_BASE: u16 = 0;
    pub const DUST: u16 = 8;
    pub const SMOKE: u16 = 9;
    pub const SPARK: u16 = 10;
    pub const BUBBLE: u16 = 11;
}

pub(crate) fn build() -> Vec<(String, Image)> {
    use particles::*;
    let mut l = SpriteList::new();
    for stage in 0..8u16 {
        let mut q = Painter::new(16, 16);
        let threshold = (stage as f32 + 1.0) / 9.0;
        for y in 0..16 {
            for x in 0..16 {
                let line = hash(x / 2, y, 60) % 4 == 0 || hash(x, y / 2, 61) % 5 == 0;
                if line && rnd(x, y, 62) < threshold {
                    q.put(x, y, Rgb(0.0, 0.0, 0.0), 0.65);
                }
            }
        }
        l.add(CRACK_BASE + stage, &format!("crack_{stage}"), q.finish());
    }
    let mut q = Painter::new(8, 8);
    q.shape(Rgb(0.6, 0.55, 0.5), &circle(0.5, 0.5, 0.35));
    l.add(DUST, "dust", q.finish());
    let mut q = Painter::new(8, 8);
    q.shape(Rgb(0.5, 0.5, 0.5), &circle(0.5, 0.5, 0.45));
    q.shape(Rgb(0.7, 0.7, 0.7), &circle(0.4, 0.4, 0.2));
    l.add(SMOKE, "smoke", q.finish());
    let mut q = Painter::new(8, 8);
    q.shape(Rgb(1.0, 0.8, 0.3), &|u, v| (u - 0.5).abs() + (v - 0.5).abs() < 0.3);
    l.add(SPARK, "spark", q.finish());
    let mut q = Painter::new(8, 8);
    q.shape(Rgb(0.8, 0.9, 1.0), &ring(0.5, 0.5, 0.33, 0.15));
    l.add(BUBBLE, "bubble", q.finish());
    l.0
}
