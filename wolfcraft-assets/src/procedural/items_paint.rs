//! Item icons (16x16, items atlas).

use super::{Painter, Rgb, SpriteList, rnd};
use crate::image::Image;

pub mod items {
    pub const STICK: u16 = 0;
    pub const APPLE: u16 = 1;
    pub const COAL: u16 = 2;
    pub const RAW_IRON: u16 = 3;
    pub const IRON_INGOT: u16 = 4;
    pub const DIAMOND: u16 = 5;
    pub const BREAD: u16 = 6;
    pub const GOLD_INGOT: u16 = 7;
    pub const RAW_GOLD: u16 = 8;
    pub const MUSHROOM_STEW: u16 = 9;
    /// Tools: TOOLS_BASE + material * 4 + kind. material: 0 wood, 1 stone, 2 iron, 3 diamond;
    /// kind: 0 pickaxe, 1 axe, 2 shovel, 3 sword.
    pub const TOOLS_BASE: u16 = 10;
    pub const SHIELD: u16 = 26;
    pub const RAW_COPPER: u16 = 27;
    pub const RAW_TIN: u16 = 28;
    pub const RAW_SILVER: u16 = 29;
    pub const COPPER_INGOT: u16 = 30;
    pub const TIN_INGOT: u16 = 31;
    pub const BRONZE_INGOT: u16 = 32;
    pub const SILVER_INGOT: u16 = 33;
    pub const RAW_MEAT: u16 = 34;
    pub const COOKED_MEAT: u16 = 35;
    pub const ROTTEN_FLESH: u16 = 36;
    /// Armor: ARMOR_BASE + material * 4 + slot (helmet, chestplate, leggings, boots).
    pub const ARMOR_BASE: u16 = 37;
    pub const COUNT: u16 = 53;
}

const PICKAXE: [&str; 16] = [
    "................", "...xxxxxxxxx....", "..xXXXXXXXXXx...", "..xx...hh..xx...", "..x....hh...x...", ".......hh.......",
    "........hh......", "........hh......", ".........hh.....", ".........hh.....", "..........hh....", "..........hh....",
    "...........hh...", "...........hh...", "............h...", "................",
];
const AXE: [&str; 16] = [
    "................", "......xxx.......", ".....xXXXxhh....", "....xXXXXxhh....", "....xXXXXhh.....", ".....xxx.hh.....",
    ".........hh.....", "..........hh....", "..........hh....", "...........hh...", "...........hh...", "............hh..",
    "............hh..", ".............h..", "................", "................",
];
const SHOVEL: [&str; 16] = [
    "................", "...........xxx..", "..........xXXXx.", "..........xXXXx.", "...........xXx..", "..........hh....",
    ".........hh.....", "........hh......", ".......hh.......", "......hh........", ".....hh.........", "....hh..........",
    "...hh...........", "..hh............", "................", "................",
];
const SWORD: [&str; 16] = [
    "................", ".............xx.", "............xXx.", "...........xXx..", "..........xXx...", ".........xXx....",
    "........xXx.....", ".......xXx......", "..ww..xXx.......", "...wwxXx........", "....wwx.........", "...hhww.........",
    "..hh..ww........", ".hh.............", "................", "................",
];
const MATERIALS: [Rgb; 4] = [Rgb(0.62, 0.45, 0.25), Rgb(0.55, 0.55, 0.55), Rgb(0.85, 0.85, 0.88), Rgb(0.35, 0.90, 0.85)];

fn p(f: impl FnOnce(&mut Painter)) -> Image {
    let mut q = Painter::new(16, 16);
    f(&mut q);
    q.finish()
}

fn blob(c: Rgb, r: f32, seed: u32) -> Image {
    p(|q| {
        for y in 0..16 {
            for x in 0..16 {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 8.0;
                let d = (dx * dx + dy * dy).sqrt() + (rnd(x, y, seed) - 0.5) * 1.5;
                if d < r {
                    q.put(x, y, c.scale(1.0 - (dx + dy) * 0.025), 1.0);
                }
            }
        }
    })
}

const HELMET: [&str; 16] = [
    "................", "................", "....xxxxxxxx....", "...xXXXXXXXXx...", "..xXXhhhhhhXXx..", "..xXhXXXXXXhXx..",
    "..xXXXXXXXXXXx..", "..xXXXXXXXXXXx..", "..xXXx....xXXx..", "..xXx......xXx..", "..xx........xx..", "................",
    "................", "................", "................", "................",
];
const CHEST: [&str; 16] = [
    "................", "..xxx......xxx..", ".xXXXx....xXXXx.", ".xXXXXxxxxXXXXx.", ".xXXXXXXXXXXXXx.", ".xXhXXXXXXXXhXx.",
    ".xXXXXXXXXXXXXx.", "..xXXXXXXXXXXx..", "..xXXXhhhhXXXx..", "..xXXXXXXXXXXx..", "..xXXXXXXXXXXx..", "..xXXXXXXXXXXx..",
    "..xXXXXXXXXXXx..", "..xxxxxxxxxxxx..", "................", "................",
];
const LEGS: [&str; 16] = [
    "................", "...xxxxxxxxxx...", "..xXXXXXXXXXXx..", "..xXhhhhhhhhXx..", "..xXXXXXXXXXXx..", "..xXXXXXXXXXXx..",
    "..xXXXxxxxXXXx..", "..xXXXx..xXXXx..", "..xXXXx..xXXXx..", "..xXXXx..xXXXx..", "..xXXXx..xXXXx..", "..xXXXx..xXXXx..",
    "..xXXXx..xXXXx..", "..xxxxx..xxxxx..", "................", "................",
];
const BOOTS: [&str; 16] = [
    "................", "................", "................", "................", "..xxxx....xxxx..", "..xXXx....xXXx..",
    "..xXXx....xXXx..", "..xXXx....xXXx..", "..xXXx....xXXx..", ".xXXXXx..xXXXXx.", ".xXhhXXx.xXhhXXx", ".xXXXXXx.xXXXXXx",
    ".xxxxxxx.xxxxxxx", "................", "................", "................",
];

fn meat(c: Rgb, bone: Rgb) -> Image {
    p(|q| {
        for y in 4..13u32 {
            for x in 3..13u32 {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 8.5;
                if (dx / 4.8).powi(2) + (dy / 4.0).powi(2) < 1.0 {
                    q.put(x, y, c.add((rnd(x, y, 170) - 0.5) * 0.1 - dy * 0.02), 1.0);
                }
            }
        }
        for i in 0..4u32 {
            q.put(11 + i / 2, 3 - i / 2 + 1, bone, 1.0);
            q.put(12 + i / 2, 2 + i / 2, bone, 1.0);
        }
    })
}

fn ingot(c: Rgb) -> Image {
    p(|q| {
        for y in 6..11u32 {
            let inset = 10 - y;
            for x in (2 + inset)..(14 - inset) {
                q.put(x, y, c.add((10 - y) as f32 * 0.04), 1.0);
            }
        }
    })
}

pub(crate) fn build() -> Vec<(String, Image)> {
    use items::*;
    let mut l = SpriteList::new();
    l.add(
        STICK,
        "stick",
        p(|q| {
            for i in 2..14u32 {
                q.put(i, 15 - i, Rgb(0.50, 0.36, 0.20), 1.0);
                q.put(i + 1, 15 - i, Rgb(0.40, 0.28, 0.15), 1.0);
            }
        }),
    );
    l.add(APPLE, "apple", {
        let mut img = blob(Rgb(0.85, 0.12, 0.12), 5.5, 50);
        img.set(8, 2, [102, 71, 38, 255]);
        img.set(8, 1, [102, 71, 38, 255]);
        img.set(9, 2, [64, 153, 51, 255]);
        img
    });
    l.add(COAL, "coal", blob(Rgb(0.15, 0.15, 0.17), 5.0, 51));
    l.add(RAW_IRON, "raw_iron", blob(Rgb(0.78, 0.60, 0.46), 5.0, 52));
    l.add(IRON_INGOT, "iron_ingot", ingot(Rgb(0.78, 0.78, 0.80)));
    l.add(DIAMOND, "diamond", blob(Rgb(0.45, 0.95, 0.90), 4.5, 53));
    l.add(
        BREAD,
        "bread",
        p(|q| {
            for y in 6..12u32 {
                for x in 2..14u32 {
                    q.put(x, y, Rgb(0.80, 0.58, 0.30).add((rnd(x, y, 54) - 0.5) * 0.08), 1.0);
                }
            }
        }),
    );
    l.add(GOLD_INGOT, "gold_ingot", ingot(Rgb(0.95, 0.80, 0.25)));
    l.add(RAW_GOLD, "raw_gold", blob(Rgb(0.92, 0.75, 0.25), 5.0, 55));
    l.add(
        MUSHROOM_STEW,
        "mushroom_stew",
        p(|q| {
            q.shape(Rgb(0.45, 0.30, 0.18), &|u, v| v > 0.5 && ((u - 0.5) / 0.42).powi(2) + ((v - 0.5) / 0.35).powi(2) < 1.0);
            q.shape(Rgb(0.70, 0.50, 0.32), &|u, v| ((u - 0.5) / 0.38).powi(2) + ((v - 0.52) / 0.08).powi(2) < 1.0);
        }),
    );
    let templates: [&[&str; 16]; 4] = [&PICKAXE, &AXE, &SHOVEL, &SWORD];
    let mat_names = ["wood", "stone", "iron", "diamond"];
    let kind_names = ["pickaxe", "axe", "shovel", "sword"];
    for (mi, m) in MATERIALS.iter().enumerate() {
        for (ki, t) in templates.iter().enumerate() {
            let m = *m;
            let img = p(|q| {
                q.template(&t[..], &|ch| match ch {
                    'x' => Some(m.scale(0.75)),
                    'X' => Some(m),
                    'h' => Some(Rgb(0.45, 0.32, 0.18)),
                    'w' => Some(Rgb(0.35, 0.25, 0.15)),
                    _ => None,
                })
            });
            l.add(TOOLS_BASE + (mi * 4 + ki) as u16, &format!("{}_{}", mat_names[mi], kind_names[ki]), img);
        }
    }
    l.add(
        SHIELD,
        "shield",
        p(|q| {
            for y in 1..15u32 {
                for x in 3..13u32 {
                    let rim = x == 3 || x == 12 || y == 1 || y == 14 || (y == 13 && (x == 4 || x == 11));
                    let c = if rim { Rgb(0.60, 0.62, 0.66) } else { Rgb(0.62, 0.45, 0.25).add((rnd(x, y, 77) - 0.5) * 0.06 + if x % 3 == 0 { -0.05 } else { 0.0 }) };
                    if !(y == 14 && (x == 3 || x == 12)) {
                        q.put(x, y, c, 1.0);
                    }
                }
            }
            for y in 6..9u32 {
                for x in 7..9u32 {
                    q.put(x, y, Rgb(0.72, 0.74, 0.78), 1.0);
                }
            }
        }),
    );
    l.add(RAW_COPPER, "raw_copper", blob(Rgb(0.80, 0.48, 0.30), 5.0, 160));
    l.add(RAW_TIN, "raw_tin", blob(Rgb(0.74, 0.78, 0.80), 5.0, 161));
    l.add(RAW_SILVER, "raw_silver", blob(Rgb(0.86, 0.88, 0.94), 5.0, 162));
    l.add(COPPER_INGOT, "copper_ingot", ingot(Rgb(0.85, 0.50, 0.32)));
    l.add(TIN_INGOT, "tin_ingot", ingot(Rgb(0.78, 0.82, 0.84)));
    l.add(BRONZE_INGOT, "bronze_ingot", ingot(Rgb(0.74, 0.54, 0.24)));
    l.add(SILVER_INGOT, "silver_ingot", ingot(Rgb(0.90, 0.92, 0.98)));
    l.add(RAW_MEAT, "raw_meat", meat(Rgb(0.82, 0.36, 0.38), Rgb(0.95, 0.75, 0.75)));
    l.add(COOKED_MEAT, "cooked_meat", meat(Rgb(0.55, 0.30, 0.16), Rgb(0.80, 0.55, 0.30)));
    l.add(ROTTEN_FLESH, "rotten_flesh", meat(Rgb(0.45, 0.50, 0.30), Rgb(0.60, 0.65, 0.40)));
    let armor_mats = [Rgb(0.55, 0.36, 0.20), Rgb(0.74, 0.54, 0.24), Rgb(0.82, 0.83, 0.86), Rgb(0.40, 0.90, 0.88)];
    let armor_tpl: [&[&str; 16]; 4] = [&HELMET, &CHEST, &LEGS, &BOOTS];
    let armor_names = ["helmet", "chestplate", "leggings", "boots"];
    for (mi, m) in armor_mats.iter().enumerate() {
        for (si, t) in armor_tpl.iter().enumerate() {
            let m = *m;
            let img = p(|q| {
                q.template(&t[..], &|ch| match ch {
                    'x' => Some(m.scale(0.72)),
                    'X' => Some(m),
                    'h' => Some(m.add(0.12)),
                    _ => None,
                })
            });
            l.add(ARMOR_BASE + (mi * 4 + si) as u16, &format!("armor_{}_{}", mi, armor_names[si]), img);
        }
    }
    for (_, img) in l.0.iter_mut() {
        outline(img);
        shade_edges(img);
    }
    l.0
}

/// Draws a 1 px dark outline around the opaque pixels (on transparent neighbours).
fn outline(img: &mut Image) {
    let (w, h) = (img.width as i32, img.height as i32);
    let mut edge = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if img.get(x as u32, y as u32)[3] != 0 {
                continue;
            }
            let touches = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0 && ny >= 0 && nx < w && ny < h && img.get(nx as u32, ny as u32)[3] > 200
            });
            if touches {
                edge.push((x as u32, y as u32));
            }
        }
    }
    for (x, y) in edge {
        img.set(x, y, [24, 20, 28, 255]);
    }
}

/// Light from the top-left: brighten pixels whose upper/left neighbour is empty, darken
/// those whose lower/right neighbour is empty. Gives sprites a little volume.
fn shade_edges(img: &mut Image) {
    let (w, h) = (img.width as i32, img.height as i32);
    let solid = |img: &Image, x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && img.get(x as u32, y as u32)[3] > 200 && img.get(x as u32, y as u32) != [24, 20, 28, 255];
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if !solid(img, x, y) {
                continue;
            }
            let mut px = img.get(x as u32, y as u32);
            let lit = !solid(img, x - 1, y) || !solid(img, x, y - 1);
            let dark = !solid(img, x + 1, y) || !solid(img, x, y + 1);
            let f = if lit && !dark { 1.18 } else if dark && !lit { 0.82 } else { 1.0 };
            if f != 1.0 {
                for c in px.iter_mut().take(3) {
                    *c = (*c as f32 * f).clamp(0.0, 255.0) as u8;
                }
                out.push((x as u32, y as u32, px));
            }
        }
    }
    for (x, y, px) in out {
        img.set(x, y, px);
    }
}
