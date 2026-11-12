//! Entity skins (entities atlas). Mob simulation comes later; the skins are packed and ready
//! for an entity renderer.

use super::{Painter, Rgb, SpriteList, rnd};
use crate::image::Image;

pub mod entities {
    pub const WOLF: u16 = 0;
    pub const PLAYER: u16 = 1;
}

pub(crate) fn build() -> Vec<(String, Image)> {
    let mut l = SpriteList::new();
    // Wolf skin 64x32: grey fur, lighter belly, amber eyes.
    let mut q = Painter::new(64, 32);
    for y in 0..32 {
        for x in 0..64 {
            let n = (rnd(x, y, 200) - 0.5) * 0.1;
            let belly = if y > 20 { 0.15 } else { 0.0 };
            q.put(x, y, Rgb(0.45, 0.47, 0.52).add(n + belly), 1.0);
        }
    }
    for (x, y) in [(4, 4), (8, 4)] {
        q.put(x, y, Rgb(1.0, 0.7, 0.1), 1.0);
    }
    l.add(entities::WOLF, "wolf", q.finish());
    // Player skin 64x64, horizontal bands (the model maps each body part to one band):
    // 0..4 hair, 4..16 skin, 16..30 jacket, 30..32 belt, 32..48 trousers, 48..64 boots.
    let mut q = Painter::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            let n = (rnd(x, y, 201) - 0.5) * 0.08;
            let c = if y < 4 {
                Rgb(0.20, 0.13, 0.08).add(if (x + y) % 3 == 0 { 0.04 } else { 0.0 })
            } else if y < 16 {
                Rgb(0.86, 0.68, 0.55).add(if y == 4 { -0.06 } else { 0.0 })
            } else if y < 30 {
                // Jacket with a darker seam every 8 px and a zip line.
                let seam = if x % 8 == 0 || y == 16 { -0.04 } else { 0.0 };
                let zip = if x % 32 == 16 { 0.08 } else { 0.0 };
                Rgb(0.10, 0.50, 0.55).add(seam + zip)
            } else if y < 32 {
                Rgb(0.95, 0.60, 0.15)
            } else if y < 48 {
                Rgb(0.18, 0.20, 0.30).add(if x % 16 == 0 { -0.03 } else { 0.0 })
            } else {
                Rgb(0.26, 0.17, 0.11).add(if y < 51 { 0.05 } else { 0.0 })
            };
            q.put(x, y, c.add(n), 1.0);
        }
    }
    l.add(entities::PLAYER, "player", q.finish());
    l.0
}
