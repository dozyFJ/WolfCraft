//! Real high-resolution block textures for the HD packs (32×, 64×, 128×).
//!
//! These are painted directly at the target size from resolution-independent, seamlessly
//! tiling noise (value noise, fractal noise, Voronoi cells), so a 128× texture has 128× detail:
//! individual pebbles, wood grain, bark crevices, leaf shapes, mortar joints. Tiles without an
//! HD painter fall back to the smooth upscale of the 16× art.

use super::{Rgb, hash};
use crate::image::Image;

/// Seamless value noise on a `period`×`period` lattice; `u`, `v` in lattice units.
fn vnoise(u: f32, v: f32, period: u32, seed: u32) -> f32 {
    let (x0, y0) = (u.floor(), v.floor());
    let (fx, fy) = (u - x0, v - y0);
    let p = period.max(1) as i64;
    let w = |x: f32, y: f32| {
        let xi = (x as i64).rem_euclid(p) as u32;
        let yi = (y as i64).rem_euclid(p) as u32;
        (hash(xi, yi, seed) & 0xFFFF) as f32 / 65535.0
    };
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let a = w(x0, y0) + (w(x0 + 1.0, y0) - w(x0, y0)) * sx;
    let b = w(x0, y0 + 1.0) + (w(x0 + 1.0, y0 + 1.0) - w(x0, y0 + 1.0)) * sx;
    a + (b - a) * sy
}

/// Fractal noise 0..1 over the tile (`x`, `y` in 0..1), starting at `base` cells per tile.
fn fbm(x: f32, y: f32, base: u32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut p) = (0.0, 1.0, 0.0, base);
    for o in 0..octaves {
        sum += vnoise(x * p as f32, y * p as f32, p, seed.wrapping_add(o * 1013)) * amp;
        norm += amp;
        amp *= 0.5;
        p *= 2;
    }
    sum / norm
}

/// Seamless Voronoi over `cells`×`cells` cells: (nearest distance, second distance, cell id),
/// distances in cell units.
fn worley(x: f32, y: f32, cells: u32, seed: u32) -> (f32, f32, u32) {
    let (u, v) = (x * cells as f32, y * cells as f32);
    let (cx, cy) = (u.floor() as i64, v.floor() as i64);
    let n = cells as i64;
    let (mut d1, mut d2, mut id) = (9.0f32, 9.0f32, 0u32);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (gx, gy) = (cx + dx, cy + dy);
            let (wx, wy) = (gx.rem_euclid(n) as u32, gy.rem_euclid(n) as u32);
            let h = hash(wx, wy, seed);
            let px = gx as f32 + 0.15 + 0.7 * ((h & 0xFFFF) as f32 / 65535.0);
            let py = gy as f32 + 0.15 + 0.7 * ((h >> 16) as f32 / 65535.0);
            let d = ((px - u).powi(2) + (py - v).powi(2)).sqrt();
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = h;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    (d1, d2, id)
}

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

fn unit(h: u32) -> f32 {
    (h & 0xFFFF) as f32 / 65535.0
}

/// Paints a `size`×`size` tile from a function of the tile coordinates (0..1) returning a colour
/// and alpha.
fn paint(size: u32, f: impl Fn(f32, f32) -> (Rgb, f32)) -> Image {
    let mut img = Image::new(size, size);
    let px = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    for y in 0..size {
        for x in 0..size {
            let (c, a) = f((x as f32 + 0.5) / size as f32, (y as f32 + 0.5) / size as f32);
            img.set(x, y, [px(c.0), px(c.1), px(c.2), px(a)]);
        }
    }
    img
}

fn solid(size: u32, f: impl Fn(f32, f32) -> Rgb) -> Image {
    paint(size, |x, y| (f(x, y), 1.0))
}

// ---- natural surfaces ----

fn stone(x: f32, y: f32, base: Rgb, seed: u32) -> Rgb {
    let m = fbm(x, y, 4, 5, seed);
    let fine = fbm(x, y, 32, 2, seed + 7);
    let (d1, d2, _) = worley(x, y, 5, seed + 3);
    // Thin dark fissures along some cell borders.
    let crack = if d2 - d1 < 0.035 && fbm(x, y, 3, 2, seed + 9) > 0.55 { 0.72 } else { 1.0 };
    base.scale((0.78 + 0.36 * m + 0.12 * (fine - 0.5)) * crack)
}

fn dirt(x: f32, y: f32, seed: u32) -> Rgb {
    let base = rgb(121, 85, 58);
    let m = fbm(x, y, 4, 5, seed);
    let mut c = base.scale(0.75 + 0.45 * m);
    // Pebbles and grit.
    let (d1, _, id) = worley(x, y, 9, seed + 1);
    if d1 < 0.22 && unit(id) < 0.35 {
        let k = 1.0 - d1 / 0.22;
        c = c.lerp(rgb(150, 132, 112).scale(0.8 + 0.3 * unit(id >> 3)), 0.8).scale(0.85 + 0.25 * k);
    }
    if fbm(x, y, 48, 1, seed + 5) > 0.72 {
        c = c.scale(0.8);
    }
    c
}

fn grass(x: f32, y: f32, seed: u32) -> Rgb {
    grass_c(x, y, rgb(98, 160, 62), seed)
}

fn grass_c(x: f32, y: f32, base: Rgb, seed: u32) -> Rgb {
    let m = fbm(x, y, 3, 4, seed);
    // Blades: thin streaks in many directions, from hashed short strokes.
    let (_, _, id) = worley(x, y, 40, seed + 2);
    let fine = fbm(x, y, 64, 2, seed + 3);
    let tone = 0.82 + 0.12 * unit(id) + 0.35 * (fine - 0.5) + 0.1;
    let mut c = base.scale(tone * (0.85 + 0.3 * m));
    if unit(id >> 5) < 0.06 {
        c = c.lerp(rgb(180, 210, 90), 0.5); // sunlit tips
    }
    c
}

fn sand(x: f32, y: f32, base: Rgb, seed: u32) -> Rgb {
    let m = fbm(x, y, 4, 4, seed);
    let grain = fbm(x, y, 64, 1, seed + 3);
    let ripple = ((y * 6.0 + fbm(x, y, 2, 3, seed + 4) * 2.5) * std::f32::consts::TAU).sin();
    base.scale(0.88 + 0.16 * m + 0.12 * (grain - 0.5) + 0.03 * ripple)
}

fn gravel(x: f32, y: f32, seed: u32) -> Rgb {
    let (d1, d2, id) = worley(x, y, 7, seed);
    let palette = [rgb(138, 132, 128), rgb(112, 106, 102), rgb(160, 150, 140), rgb(98, 94, 96), rgb(130, 116, 100)];
    let c = palette[(id % 5) as usize];
    let edge = ((d2 - d1) / 0.18).min(1.0);
    let shade = 1.0 - d1 * 0.45; // rounded pebbles
    c.scale((0.55 + 0.45 * edge) * shade * (0.9 + 0.2 * fbm(x, y, 16, 2, seed + 1)))
}

fn cobble(x: f32, y: f32, base: Rgb, seed: u32, moss: bool) -> Rgb {
    let (d1, d2, id) = worley(x, y, 4, seed);
    let edge = d2 - d1;
    if edge < 0.06 {
        return base.scale(0.38 + 2.0 * edge);
    }
    // Bevel: lighter towards the top-left of each stone, darker at the bottom-right.
    let (dx, dy) = (worley(x - 0.004, y - 0.004, 4, seed).0 - d1, 0.0);
    let _ = dy;
    let bevel = (edge.min(0.25) / 0.25).sqrt();
    let tone = 0.78 + 0.3 * unit(id) + 0.25 * fbm(x, y, 16, 3, seed + 2) - 0.12;
    let mut c = base.scale(tone * (0.7 + 0.3 * bevel) * (1.0 - dx * 8.0).clamp(0.85, 1.15));
    if moss && fbm(x, y, 4, 4, seed + 11) > 0.56 {
        c = c.lerp(rgb(84, 122, 58), 0.75);
    }
    c
}

fn leaves(x: f32, y: f32, base: Rgb, seed: u32) -> Rgb {
    // Overlapping leaves: two Voronoi layers, each leaf lit from the top-left.
    let mut c = base.scale(0.42);
    for (layer, cells) in [(0u32, 7u32), (1, 11)] {
        let (d1, _, id) = worley(x + 0.03 * layer as f32, y, cells, seed + layer * 31);
        let r = 0.42 + 0.1 * unit(id);
        if d1 < r {
            let k = 1.0 - d1 / r;
            let tone = 0.7 + 0.35 * unit(id >> 4) + 0.25 * k;
            c = base.scale(tone);
        }
    }
    c.scale(0.9 + 0.2 * fbm(x, y, 8, 2, seed + 5))
}

fn ore(x: f32, y: f32, color: Rgb, seed: u32) -> Rgb {
    let s = stone(x, y, rgb(128, 128, 132), seed);
    let (d1, _, id) = worley(x, y, 6, seed + 50);
    let r = 0.28 + 0.12 * unit(id);
    let d1 = d1 + 0.22 * (fbm(x, y, 12, 2, seed + 52) - 0.5);
    if unit(id >> 8) < 0.55 && d1 < r {
        let k = 1.0 - d1 / r;
        // Crystal facets: bright core, dark rim.
        let facet = 0.7 + 0.5 * k + 0.2 * (fbm(x, y, 24, 1, seed + 51) - 0.5);
        return color.scale(facet);
    }
    s
}

// ---- wood ----

fn planks(x: f32, y: f32, wood: Rgb, seed: u32) -> Rgb {
    let boards = 4.0;
    let row = (y * boards).floor();
    let fy = y * boards - row;
    let shift = unit(hash(row as u32, 0, seed));
    // Board ends: one joint per board at a different place.
    let joint = ((x + shift).fract() - 0.5).abs() < 0.008;
    if fy < 0.06 || joint {
        return wood.scale(0.45);
    }
    let grain = fbm(x, y, 2, 3, seed + row as u32);
    let lines = (((fy + grain * 0.6) * 9.0) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    let tone = 0.82 + 0.14 * lines + 0.12 * (unit(hash(row as u32, 1, seed)) - 0.5) + 0.1 * fbm(x, y, 32, 2, seed + 3);
    // Top edge of each board catches the light.
    let edge = if fy < 0.12 { 1.1 } else if fy > 0.94 { 0.85 } else { 1.0 };
    wood.scale(tone * edge)
}

fn bark(x: f32, y: f32, base: Rgb, seed: u32, birch: bool) -> Rgb {
    if birch {
        let m = fbm(x, y, 4, 3, seed);
        let mut c = rgb(222, 218, 205).scale(0.88 + 0.15 * m);
        // Dark horizontal lenticels.
        let (d1, _, id) = worley(x, y * 3.0, 5, seed + 1);
        if d1 < 0.22 && unit(id) < 0.6 {
            c = rgb(45, 42, 40);
        }
        return c;
    }
    // Vertical ridges with dark crevices.
    let ridge = fbm(x * 3.0, y, 2, 4, seed);
    let crev = ((x * 9.0 + ridge * 3.0) * std::f32::consts::TAU).sin();
    let k = if crev > 0.6 { 0.55 } else { 0.85 + 0.25 * (crev * 0.5 + 0.5) };
    base.scale(k * (0.85 + 0.3 * fbm(x, y, 16, 2, seed + 4)))
}

fn log_top(x: f32, y: f32, wood: Rgb, bark_c: Rgb, seed: u32) -> Rgb {
    let (dx, dy) = (x - 0.5, y - 0.5);
    let r = (dx * dx + dy * dy).sqrt();
    let edge = dx.abs().max(dy.abs());
    if edge > 0.44 {
        return bark_c.scale(0.8 + 0.3 * fbm(x, y, 16, 2, seed));
    }
    let rings = ((r * 11.0 + fbm(x, y, 4, 3, seed + 1) * 0.8) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    wood.scale(0.8 + 0.25 * rings).scale(0.95 + 0.1 * fbm(x, y, 32, 1, seed + 2))
}

// ---- masonry ----

fn bricks(x: f32, y: f32, seed: u32) -> Rgb {
    let rows = 4.0;
    let row = (y * rows).floor();
    let fy = y * rows - row;
    let off = if row as i32 % 2 == 0 { 0.0 } else { 0.25 };
    let bx = ((x + off) * 2.0).fract();
    let col = ((x + off) * 2.0).floor();
    let mortar = rgb(190, 182, 168);
    if fy < 0.1 || bx < 0.05 {
        return mortar.scale(0.85 + 0.2 * fbm(x, y, 32, 1, seed));
    }
    let tone = unit(hash(col as u32, row as u32, seed));
    let base = rgb(150, 66, 50).lerp(rgb(120, 52, 44), tone);
    let bevel = if fy < 0.18 { 1.12 } else if fy > 0.93 { 0.85 } else { 1.0 };
    base.scale(bevel * (0.85 + 0.3 * fbm(x, y, 16, 3, seed + 2)))
}

fn stone_bricks(x: f32, y: f32, seed: u32, mossy: bool, cracked: bool) -> Rgb {
    let row = (y * 2.0).floor();
    let fy = y * 2.0 - row;
    let off = if row as i32 % 2 == 0 { 0.0 } else { 0.5 };
    let bx = (x + off).fract();
    let base = rgb(128, 128, 128);
    if fy < 0.05 || bx < 0.025 {
        return base.scale(0.45);
    }
    let tone = unit(hash(((x + off).floor()) as u32, row as u32, seed));
    let bevel = if fy < 0.1 || bx < 0.06 { 1.15 } else if fy > 0.95 || bx > 0.975 { 0.75 } else { 1.0 };
    let mut c = stone(x, y, base, seed).scale((0.92 + 0.12 * tone) * bevel);
    if cracked {
        let (d1, d2, _) = worley(x, y, 3, seed + 8);
        if d2 - d1 < 0.025 && fbm(x, y, 2, 2, seed + 9) > 0.45 {
            c = c.scale(0.45);
        }
    }
    if mossy && fbm(x, y, 5, 4, seed + 4) > 0.55 {
        c = c.lerp(rgb(80, 118, 56), 0.7);
    }
    c
}

fn sandstone(x: f32, y: f32, seed: u32) -> Rgb {
    let layer = ((y * 7.0 + fbm(x, y, 2, 2, seed) * 0.6) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    sand(x, y, rgb(222, 205, 150), seed).scale(0.9 + 0.12 * layer)
}

/// HD painting of a block sprite, or `None` to upscale the 16× art.
pub fn block(name: &str, size: u32) -> Option<Image> {
    let s = size;
    let grass_side = |x: f32, y: f32| {
        let depth = 0.2 + 0.12 * fbm(x, 0.0, 6, 3, 41) + 0.08 * fbm(x, 0.5, 24, 1, 42);
        if y < depth { grass(x, y, 40).scale(0.95) } else if y < depth + 0.03 { dirt(x, y, 43).scale(0.7) } else { dirt(x, y, 43) }
    };
    Some(match name {
        "stone" => solid(s, |x, y| stone(x, y, rgb(128, 128, 130), 1)),
        "dirt" => solid(s, |x, y| dirt(x, y, 2)),
        "grass_top" => solid(s, |x, y| grass(x, y, 40)),
        "grass_side" => solid(s, grass_side),
        "swamp_grass_top" => solid(s, |x, y| grass_c(x, y, rgb(76, 108, 56), 80)),
        "dry_grass_top" => solid(s, |x, y| grass_c(x, y, rgb(140, 158, 82), 81)),
        "moss" => solid(s, |x, y| grass_c(x, y, rgb(84, 130, 52), 82)),
        "swamp_grass_side" | "dry_grass_side" | "podzol_side" => {
            let top = match name {
                "swamp_grass_side" => rgb(76, 108, 56),
                "dry_grass_side" => rgb(140, 158, 82),
                _ => rgb(110, 76, 38),
            };
            solid(s, move |x, y| {
                let depth = 0.2 + 0.12 * fbm(x, 0.0, 6, 3, 41) + 0.08 * fbm(x, 0.5, 24, 1, 42);
                if y < depth { grass_c(x, y, top, 83).scale(0.95) } else if y < depth + 0.03 { dirt(x, y, 43).scale(0.7) } else { dirt(x, y, 43) }
            })
        }
        "snow" => solid(s, |x, y| rgb(240, 245, 250).scale(0.93 + 0.08 * fbm(x, y, 4, 4, 5))),
        "snow_side" => solid(s, |x, y| {
            let depth = 0.22 + 0.1 * fbm(x, 0.0, 6, 3, 44);
            if y < depth { rgb(240, 245, 250).scale(0.95 + 0.05 * fbm(x, y, 8, 2, 45)) } else { dirt(x, y, 43) }
        }),
        "sand" => solid(s, |x, y| sand(x, y, rgb(219, 207, 160), 6)),
        "red_sand" => solid(s, |x, y| sand(x, y, rgb(190, 104, 50), 7)),
        "gravel" => solid(s, |x, y| gravel(x, y, 8)),
        "cobblestone" => solid(s, |x, y| cobble(x, y, rgb(122, 122, 122), 9, false)),
        "mossy_cobblestone" => solid(s, |x, y| cobble(x, y, rgb(122, 122, 122), 9, true)),
        "bedrock" => solid(s, |x, y| {
            let (d1, d2, id) = worley(x, y, 6, 10);
            rgb(70, 70, 72).scale(0.4 + 0.9 * unit(id) * ((d2 - d1) * 4.0).min(1.0) + 0.1 * d1)
        }),
        "clay" => solid(s, |x, y| rgb(160, 166, 178).scale(0.9 + 0.15 * fbm(x, y, 4, 4, 11))),
        "obsidian" => solid(s, |x, y| {
            let m = fbm(x, y, 3, 5, 12);
            let streak = if fbm(x, y * 2.0, 4, 3, 13) > 0.62 { 1.8 } else { 1.0 };
            rgb(28, 18, 42).scale((0.7 + 0.6 * m) * streak)
        }),
        "netherrack" => solid(s, |x, y| {
            let (d1, d2, _) = worley(x, y, 6, 14);
            rgb(110, 38, 38).scale((0.65 + 0.5 * fbm(x, y, 4, 5, 15)) * (0.7 + 0.3 * ((d2 - d1) * 5.0).min(1.0)))
        }),
        "sandstone" => solid(s, |x, y| sandstone(x, y, 16)),
        "coal_ore" => solid(s, |x, y| ore(x, y, rgb(40, 40, 42), 20)),
        "iron_ore" => solid(s, |x, y| ore(x, y, rgb(214, 170, 136), 21)),
        "gold_ore" => solid(s, |x, y| ore(x, y, rgb(250, 214, 70), 22)),
        "diamond_ore" => solid(s, |x, y| ore(x, y, rgb(100, 236, 226), 23)),
        "copper_ore" => solid(s, |x, y| ore(x, y, rgb(220, 128, 80), 24)),
        "tin_ore" => solid(s, |x, y| ore(x, y, rgb(200, 210, 214), 25)),
        "silver_ore" => solid(s, |x, y| ore(x, y, rgb(235, 238, 250), 26)),
        "cobalt_ore" => solid(s, |x, y| ore(x, y, rgb(60, 100, 230), 27)),
        "iskrit_ore" => solid(s, |x, y| ore(x, y, rgb(240, 60, 170), 28)),
        "uranium_ore" => solid(s, |x, y| ore(x, y, rgb(120, 245, 80), 29)),
        "planks" => solid(s, |x, y| planks(x, y, rgb(166, 124, 76), 30)),
        "birch_planks" => solid(s, |x, y| planks(x, y, rgb(214, 192, 140), 31)),
        "spruce_planks" => solid(s, |x, y| planks(x, y, rgb(118, 84, 52), 32)),
        "acacia_planks" => solid(s, |x, y| planks(x, y, rgb(186, 98, 52), 33)),
        "oak_log_side" => solid(s, |x, y| bark(x, y, rgb(104, 80, 50), 34, false)),
        "spruce_log_side" => solid(s, |x, y| bark(x, y, rgb(70, 50, 32), 35, false)),
        "acacia_log_side" => solid(s, |x, y| bark(x, y, rgb(110, 100, 90), 36, false)),
        "birch_log_side" => solid(s, |x, y| bark(x, y, rgb(220, 216, 204), 37, true)),
        "log_top" => solid(s, |x, y| log_top(x, y, rgb(180, 142, 90), rgb(104, 80, 50), 38)),
        "oak_leaves" => solid(s, |x, y| leaves(x, y, rgb(70, 132, 48), 50)),
        "spruce_leaves" => solid(s, |x, y| leaves(x, y, rgb(48, 98, 58), 51)),
        "birch_leaves" => solid(s, |x, y| leaves(x, y, rgb(104, 146, 62), 52)),
        "acacia_leaves" => solid(s, |x, y| leaves(x, y, rgb(102, 140, 52), 53)),
        "bricks" => solid(s, |x, y| bricks(x, y, 60)),
        "stone_bricks" => solid(s, |x, y| stone_bricks(x, y, 61, false, false)),
        "mossy_stone_bricks" => solid(s, |x, y| stone_bricks(x, y, 61, true, false)),
        "cracked_stone_bricks" => solid(s, |x, y| stone_bricks(x, y, 61, false, true)),
        "smooth_stone" => solid(s, |x, y| rgb(160, 160, 162).scale(0.92 + 0.1 * fbm(x, y, 4, 4, 62))),
        "granite" => solid(s, |x, y| stone(x, y, rgb(160, 110, 90), 63)),
        "marble" => solid(s, |x, y| {
            let vein = (((x + y) * 3.0 + fbm(x, y, 3, 5, 64) * 2.5) * std::f32::consts::TAU).sin().abs();
            rgb(236, 234, 228).scale(if vein < 0.06 { 0.7 } else { 0.95 + 0.05 * fbm(x, y, 8, 2, 65) })
        }),
        "slate" => solid(s, |x, y| stone(x, y, rgb(78, 82, 92), 66).scale(0.9 + 0.15 * (y * 8.0 * std::f32::consts::TAU).sin().abs())),
        "limestone" => solid(s, |x, y| stone(x, y, rgb(200, 194, 170), 67)),
        "mud" => solid(s, |x, y| rgb(76, 60, 50).scale(0.75 + 0.4 * fbm(x, y, 4, 5, 68))),
        "podzol_top" => solid(s, |x, y| dirt(x, y, 69).lerp(rgb(92, 64, 30), 0.4 + 0.3 * fbm(x, y, 6, 3, 70))),
        "basalt_side" => solid(s, |x, y| rgb(72, 72, 78).scale(0.75 + 0.35 * fbm(x * 4.0, y, 2, 4, 71))),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hd_tiles_have_the_size_asked_and_tile_seamlessly() {
        for name in ["stone", "grass_top", "planks", "oak_leaves", "cobblestone", "iron_ore"] {
            let img = block(name, 64).expect(name);
            assert_eq!((img.width, img.height), (64, 64));
            // Opposite edges match closely (seamless tiling).
            let diff: f32 = (0..64)
                .map(|i| {
                    let a = img.get(0, i);
                    let b = img.get(63, i);
                    (0..3).map(|c| (a[c] as f32 - b[c] as f32).abs()).sum::<f32>()
                })
                .sum::<f32>()
                / 64.0;
            assert!(diff < 120.0, "{name}: left and right edges differ by {diff}");
        }
        assert!(block("water", 64).is_none(), "animated water keeps the upscale");
    }
}
