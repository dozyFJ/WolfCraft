//! Block textures (16x16, blocks atlas).

use super::{Painter, Rgb, SpriteList, hash, rnd};
use crate::image::Image;

/// Sprite ids inside the blocks atlas.
pub mod tiles {
    pub const GRASS_TOP: u16 = 0;
    pub const GRASS_SIDE: u16 = 1;
    pub const DIRT: u16 = 2;
    pub const STONE: u16 = 3;
    pub const COBBLESTONE: u16 = 4;
    pub const SAND: u16 = 5;
    pub const GRAVEL: u16 = 6;
    pub const LOG_SIDE: u16 = 7;
    pub const LOG_TOP: u16 = 8;
    pub const PLANKS: u16 = 9;
    pub const LEAVES: u16 = 10;
    pub const WATER: u16 = 11;
    pub const GLASS: u16 = 12;
    pub const SNOW: u16 = 13;
    pub const SNOW_SIDE: u16 = 14;
    pub const COAL_ORE: u16 = 15;
    pub const IRON_ORE: u16 = 16;
    pub const GOLD_ORE: u16 = 17;
    pub const DIAMOND_ORE: u16 = 18;
    pub const BEDROCK: u16 = 19;
    pub const SANDSTONE: u16 = 20;
    pub const TABLE_TOP: u16 = 21;
    pub const TABLE_SIDE: u16 = 22;
    pub const CACTUS_SIDE: u16 = 23;
    pub const CACTUS_TOP: u16 = 24;
    pub const TALL_GRASS: u16 = 25;
    pub const FLOWER_RED: u16 = 26;
    pub const FLOWER_YELLOW: u16 = 27;
    pub const ICE: u16 = 28;
    pub const CLAY: u16 = 29;
    pub const BRICKS: u16 = 30;
    pub const LAMP: u16 = 31;
    pub const SPRUCE_LOG_SIDE: u16 = 32;
    pub const SPRUCE_LEAVES: u16 = 33;
    pub const DEAD_BUSH: u16 = 34;
    pub const STONE_BRICKS: u16 = 35;
    pub const LAVA: u16 = 36;
    pub const BASALT_SIDE: u16 = 37;
    pub const BASALT_TOP: u16 = 38;
    pub const OBSIDIAN: u16 = 39;
    pub const MUD: u16 = 40;
    pub const MOSSY_COBBLESTONE: u16 = 41;
    pub const BIRCH_LOG_SIDE: u16 = 42;
    pub const BIRCH_LEAVES: u16 = 43;
    pub const ACACIA_LOG_SIDE: u16 = 44;
    pub const ACACIA_LEAVES: u16 = 45;
    pub const MUSHROOM_RED: u16 = 46;
    pub const MUSHROOM_BROWN: u16 = 47;
    pub const SEAGRASS: u16 = 48;
    pub const KELP: u16 = 49;
    pub const SHRUB: u16 = 50;
    pub const DRY_GRASS_TOP: u16 = 51;
    pub const DRY_GRASS_SIDE: u16 = 52;
    pub const MOSSY_STONE_BRICKS: u16 = 53;
    pub const CRACKED_STONE_BRICKS: u16 = 54;
    pub const CHEST_SIDE: u16 = 55;
    pub const CHEST_TOP: u16 = 56;
    pub const CHEST_FRONT: u16 = 57;
    pub const ASH: u16 = 58;
    pub const PACKED_ICE: u16 = 59;
    pub const GOLD_BLOCK: u16 = 60;
    pub const RED_SAND: u16 = 61;
    pub const TERRACOTTA: u16 = 62;
    pub const SWAMP_GRASS_TOP: u16 = 63;
    pub const SWAMP_GRASS_SIDE: u16 = 64;
    pub const MAGMA: u16 = 65;
    // Added: stones, metals, tech (see docs/MODDING.md).
    pub const GRANITE: u16 = 66;
    pub const MARBLE: u16 = 67;
    pub const SLATE: u16 = 68;
    pub const LIMESTONE: u16 = 69;
    pub const COPPER_ORE: u16 = 70;
    pub const TIN_ORE: u16 = 71;
    pub const SILVER_ORE: u16 = 72;
    pub const COBALT_ORE: u16 = 73;
    pub const COPPER_BLOCK: u16 = 74;
    pub const IRON_BLOCK: u16 = 75;
    pub const DIAMOND_BLOCK: u16 = 76;
    pub const COAL_BLOCK: u16 = 77;
    pub const BRONZE_BLOCK: u16 = 78;
    pub const SALT: u16 = 79;
    pub const PEAT: u16 = 80;
    pub const GLOWSTONE: u16 = 81;
    pub const MACHINE_CASING: u16 = 82;
    pub const GENERATOR_FRONT: u16 = 83;
    pub const BONES: u16 = 84;
    pub const BATTERY_SIDE: u16 = 85;
    pub const BATTERY_TOP: u16 = 86;
    pub const ELECTRIC_LAMP: u16 = 87;
    pub const FURNACE_FRONT: u16 = 88;
    pub const ELECTRIC_LAMP_ON: u16 = 89;
    pub const SWITCH_OFF: u16 = 90;
    pub const SWITCH_ON: u16 = 91;
    pub const COMMAND_BLOCK: u16 = 92;
    pub const TERMINAL: u16 = 95;
    pub const NETHERRACK: u16 = 93;
    pub const PORTAL: u16 = 94;
    // Added: biome and building blocks.
    pub const WOOL: u16 = 96;
    pub const BIRCH_PLANKS: u16 = 97;
    pub const SPRUCE_PLANKS: u16 = 98;
    pub const ACACIA_PLANKS: u16 = 99;
    pub const TORCH: u16 = 100;
    pub const PUMPKIN_SIDE: u16 = 101;
    pub const PUMPKIN_TOP: u16 = 102;
    pub const MELON_SIDE: u16 = 103;
    pub const MELON_TOP: u16 = 104;
    pub const SUGAR_CANE: u16 = 105;
    pub const FERN: u16 = 106;
    pub const FLOWER_BLUE: u16 = 107;
    pub const PODZOL_TOP: u16 = 108;
    pub const PODZOL_SIDE: u16 = 109;
    pub const MOSS: u16 = 110;
    pub const POLISHED_GRANITE: u16 = 111;
    pub const POLISHED_MARBLE: u16 = 112;
    pub const SMOOTH_STONE: u16 = 113;
    pub const SANDSTONE_BRICKS: u16 = 114;
    pub const BOOKSHELF: u16 = 115;
    pub const LANTERN: u16 = 116;
    pub const WOOL_RED: u16 = 117;
    pub const WOOL_BLUE: u16 = 118;
    pub const WOOL_YELLOW: u16 = 119;
    pub const GLASS_RED: u16 = 120;
    pub const GLASS_BLUE: u16 = 121;
    pub const GLASS_GREEN: u16 = 122;
    pub const GLASS_YELLOW: u16 = 123;
    // Iskrit (signal network).
    pub const ISKRIT_ORE: u16 = 124;
    pub const ISKRIT_WIRE: u16 = 125;
    pub const ISKRIT_WIRE_ON: u16 = 126;
    pub const ISKRIT_BLOCK: u16 = 127;
    pub const ISKRIT_TORCH: u16 = 128;
    pub const ISKRIT_LAMP: u16 = 129;
    pub const ISKRIT_LAMP_ON: u16 = 130;
    pub const BUTTON: u16 = 131;
    pub const DOOR_OAK: u16 = 132;
    pub const DOOR_BIRCH: u16 = 133;
    pub const DOOR_SPRUCE: u16 = 134;
    pub const DOOR_IRON: u16 = 135;
    pub const DOOR_OAK_TOP: u16 = 136;
    pub const DOOR_BIRCH_TOP: u16 = 137;
    pub const DOOR_SPRUCE_TOP: u16 = 138;
    pub const DOOR_IRON_TOP: u16 = 139;
    pub const URANIUM_ORE: u16 = 140;
    pub const URANIUM_BLOCK: u16 = 141;
    pub const PISTON_SIDE: u16 = 142;
    pub const PISTON_FRONT: u16 = 143;
    pub const PISTON_BACK: u16 = 144;
    pub const PISTON_INNER: u16 = 145;
    pub const STICKY_FRONT: u16 = 146;
    /// Repeater / comparator tops, one per direction (0 +i, 1 -i, 2 +j, 3 -j), off and on.
    pub const REPEATER: u16 = 147;
    pub const REPEATER_ON: u16 = 151;
    pub const COMPARATOR: u16 = 155;
    pub const COMPARATOR_ON: u16 = 159;
    pub const BASEPLATE: u16 = 163;
    /// Farming: tilled soil and two stages of every crop (young, ripe).
    pub const FARMLAND: u16 = 164;
    pub const WHEAT_YOUNG: u16 = 165;
    pub const WHEAT_RIPE: u16 = 166;
    pub const CARROT_YOUNG: u16 = 167;
    pub const CARROT_RIPE: u16 = 168;
    pub const POTATO_YOUNG: u16 = 169;
    pub const POTATO_RIPE: u16 = 170;
    pub const ENCHANT_TOP: u16 = 171;
    pub const ENCHANT_SIDE: u16 = 172;
    pub const BUILD_GLASS: u16 = 173;
    pub const BUILD_GLASS_RED: u16 = 174;
    pub const BUILD_GLASS_BLUE: u16 = 175;
    pub const BUILD_GLASS_GREEN: u16 = 176;
    pub const BUILD_FABRIC: u16 = 177;
    pub const BUILD_MIRROR: u16 = 178;
    pub const BUILD_CERAMIC: u16 = 179;
    pub const BUILD_PICTURE_FOREST: u16 = 180;
    pub const BUILD_PICTURE_SKY: u16 = 181;
    pub const BUILD_PICTURE_ISKRIT: u16 = 182;
}

/// Top of a repeater or comparator: a stone slab with an arrow pointing along `dir`
/// (image x = +i, image y = +j), two torches (repeater) or three (comparator), lit when on.
fn gate_tile(dir: u8, on: bool, comparator: bool) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let g = 0.60 + (rnd(x, y, 230) - 0.5) * 0.06;
                q.put(x, y, Rgb(g, g, g * 1.02), 1.0);
            }
        }
        let (fx, fy): (i32, i32) = match dir { 0 => (1, 0), 1 => (-1, 0), 2 => (0, 1), _ => (0, -1) };
        let at = |a: i32, b: i32| -> (u32, u32) {
            let x = 7.5 + a as f32 * fx as f32 + b as f32 * (-fy) as f32;
            let y = 7.5 + a as f32 * fy as f32 + b as f32 * fx as f32;
            (x.round().clamp(0.0, 15.0) as u32, y.round().clamp(0.0, 15.0) as u32)
        };
        let dust = if on { Rgb(1.0, 0.32, 0.72) } else { Rgb(0.45, 0.12, 0.30) };
        for a in -6..=6 {
            let (x, y) = at(a, 0);
            q.put(x, y, dust, 1.0);
        }
        for k in 1..=3 {
            for s2 in [-k, k] {
                let (x, y) = at(6 - k, s2);
                q.put(x, y, dust, 1.0);
            }
        }
        // Torches seen from above: a wooden ring round the head; the head glows when on.
        let (head, ring) = if on { (Rgb(1.0, 0.70, 0.92), Rgb(0.95, 0.25, 0.65)) } else { (Rgb(0.42, 0.12, 0.28), Rgb(0.40, 0.27, 0.15)) };
        let spots: &[(i32, i32)] = if comparator { &[(-4, -3), (-4, 3), (3, 0)] } else { &[(-3, 0), (2, 0)] };
        for &(a, b) in spots {
            for da in -1..=2 {
                for db in -1..=2 {
                    let inner = (0..=1).contains(&da) && (0..=1).contains(&db);
                    let corner = (da == -1 || da == 2) && (db == -1 || db == 2);
                    if corner {
                        continue;
                    }
                    let (x, y) = at(a + da, b + db);
                    q.put(x, y, if inner { head } else { ring }, 1.0);
                }
            }
        }
        bevel(q, 0, 0, 15, 15, Rgb(0.78, 0.78, 0.80), Rgb(0.36, 0.36, 0.38));
    })
}

/// Door leaf, one half of a two-block door. Lower half: a frame, two tall raised panels and
/// the handle. Upper half: a frame and a four-pane window.
fn door(wood: Rgb, iron: bool, seed: u32, top: bool) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let grain = (rnd(x / 3, y, seed) - 0.5) * 0.08 + if x % 4 == 0 { -0.04 } else { 0.0 };
                let mut c = if iron { wood.add((rnd(x, y, seed) - 0.5) * 0.04) } else { wood.add(grain) };
                // The frame runs round the whole door: no bar where the two halves meet.
                let frame = x <= 1 || x >= 14 || if top { y <= 1 } else { y >= 14 };
                let window = top && (3..=12).contains(&x) && (3..=13).contains(&y);
                let mullion = x == 7 || x == 8 || y == 8;
                let (py0, py1) = if top { (99, 99) } else { (2, 11) };
                let panel_l = (3..=6).contains(&x) && (py0..=py1).contains(&y);
                let panel_r = (9..=12).contains(&x) && (py0..=py1).contains(&y);
                if frame {
                    c = c.scale(0.68);
                } else if window {
                    c = if mullion { wood.scale(0.7) } else { Rgb(0.62, 0.78, 0.86).add((y as f32 - 4.5) * -0.03) };
                } else if panel_l || panel_r {
                    // Raised panel: light top-left edge, dark bottom-right edge.
                    let px0 = if panel_l { 3 } else { 9 };
                    let edge_lt = x == px0 || y == py0;
                    let edge_rb = x == px0 + 3 || y == py1;
                    c = if edge_lt { c.scale(1.35) } else if edge_rb { c.scale(0.55) } else { c.scale(0.92) };
                }
                q.put(x, y, c, 1.0);
            }
        }
        if !top {
            // Handle on the right, at the height of a hand.
            for y in 1..=3 {
                q.put(12, y, Rgb(0.20, 0.18, 0.16), 1.0);
                q.put(13, y, Rgb(0.78, 0.70, 0.40), 1.0);
            }
        }
    })
}

const S: u32 = 16;

fn p(f: impl FnOnce(&mut Painter)) -> Image {
    let mut q = Painter::new(S, S);
    f(&mut q);
    q.finish()
}

fn side_with_top(base: Rgb, top: Rgb, seed: u32) -> Image {
    p(|q| {
        q.noisy(base, 0.09, 3);
        q.speckle(base.scale(0.88), 0.2, 4);
        for x in 0..S {
            let depth = 3 + (hash(x, 0, seed) % 3);
            for y in 0..depth {
                let n = (rnd(x, y, seed + 1) - 0.5) * 0.12;
                q.put(x, y, top.add(n), 1.0);
            }
        }
    })
}

fn log_side(base: Rgb, seed: u32, birch: bool) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let stripe = (rnd(x, 0, seed) - 0.5) * 0.12;
                let n = (rnd(x, y, seed + 1) - 0.5) * 0.06;
                let groove = if hash(x, y / 3, seed) % 9 == 0 { -0.08 } else { 0.0 };
                let mut c = base.add(stripe + n + groove);
                if birch && hash(x / 3, y, seed + 2) % 11 == 0 {
                    c = Rgb(0.15, 0.15, 0.14);
                }
                q.put(x, y, c, 1.0);
            }
        }
    })
}

fn leaves(base: Rgb, seed: u32) -> Image {
    p(|q| {
        // Dark shade between the leaves (not holes: holes let the bright sky shine through as
        // white dots), then small leaf clumps lit from the top-left. Wraps around, so it tiles.
        let mut px = vec![base.scale(0.55); (S * S) as usize];
        for y in 0..S {
            for x in 0..S {
                px[(y * S + x) as usize] = base.scale(0.50 + rnd(x, y, seed + 3) * 0.12);
            }
        }
        for cy in 0..4u32 {
            for cx in 0..4u32 {
                // One clump per 4x4 cell, jittered; every other row shifted (no visible grid).
                let ox = cx * 4 + (cy % 2) * 2 + (rnd(cx, cy, seed) * 2.0) as u32;
                let oy = cy * 4 + (rnd(cx, cy, seed + 1) * 2.0) as u32;
                let tone = (rnd(cx, cy, seed + 2) - 0.5) * 0.14;
                for (dx, dy, f) in [(1i32, 0i32, 0.08), (0, 1, 0.08), (1, 1, 0.0), (2, 1, -0.06), (1, 2, -0.08), (2, 2, -0.14), (0, 0, 0.16)] {
                    let x = (ox as i32 + dx).rem_euclid(S as i32) as u32;
                    let y = (oy as i32 + dy).rem_euclid(S as i32) as u32;
                    px[(y * S + x) as usize] = base.add(tone + f + (rnd(x, y, seed + 4) - 0.5) * 0.05);
                }
            }
        }
        for y in 0..S {
            for x in 0..S {
                q.put(x, y, px[(y * S + x) as usize], 1.0);
            }
        }
    })
}

fn ore(c: Rgb, seed: u32) -> Image {
    p(|q| {
        q.noisy(Rgb(0.50, 0.50, 0.52), 0.08, 5);
        // Larger mineral chips stay readable after the cartoon filter.
        for y in 0..S { for x in 0..S {
            let cluster = hash(x / 4, y / 4, seed) % 3 == 0;
            if cluster && x % 4 > 0 && y % 4 > 0 && hash(x, y, seed + 1) % 7 != 0 {
                let facet = if x % 4 == 1 || y % 4 == 1 { 0.10 } else { -0.05 };
                q.put(x, y, c.add(facet), 1.0);
            }
        }}
    })
}

fn bricks_pattern(base: Rgb, mortar: Rgb, rows: u32, cols: u32, seed: u32, moss: f32, cracks: bool) -> Image {
    p(|q| {
        let bh = S / rows;
        let bw = S / cols;
        for y in 0..S {
            for x in 0..S {
                let off = if (y / bh) % 2 == 0 { 0 } else { bw / 2 };
                let is_mortar = y % bh == bh - 1 || (x + off) % bw == bw - 1;
                let n = (rnd(x, y, seed) - 0.5) * 0.1;
                let mut c = if is_mortar { mortar } else { base.add(n) };
                if moss > 0.0 && rnd(x / 2, y / 2, seed + 9) < moss {
                    c = Rgb(0.30, 0.48, 0.22).add(n);
                }
                if cracks && !is_mortar && hash(x + y, x, seed + 3) % 13 == 0 {
                    c = mortar.scale(0.8);
                }
                q.put(x, y, c, 1.0);
            }
        }
    })
}

fn flower(petal: Rgb) -> Image {
    p(|q| {
        let stem = Rgb(0.25, 0.55, 0.20);
        for y in 7..S {
            q.put(7, y, stem, 1.0);
        }
        q.put(6, 12, stem, 1.0);
        q.put(8, 10, stem, 1.0);
        for (dx, dy) in [(0i32, -1i32), (-1, 0), (1, 0), (0, 1), (-1, -1), (1, 1), (1, -1), (-1, 1)] {
            q.put((7 + dx) as u32, (5 + dy) as u32, petal, 1.0);
        }
        q.put(7, 5, Rgb(0.95, 0.85, 0.30), 1.0);
    })
}

fn mushroom(cap: Rgb, spots: bool) -> Image {
    p(|q| {
        for y in 9..S {
            for x in 6..10 {
                q.put(x, y, Rgb(0.90, 0.86, 0.78).add((rnd(x, y, 70) - 0.5) * 0.06), 1.0);
            }
        }
        for y in 4..10u32 {
            let half = match y {
                4 => 3,
                5 => 5,
                _ => 6,
            };
            for x in (8 - half)..(8 + half) {
                let mut c = cap.add((rnd(x, y, 71) - 0.5) * 0.08);
                if spots && hash(x, y, 72) % 6 == 0 {
                    c = Rgb(0.95, 0.95, 0.92);
                }
                q.put(x, y, c, 1.0);
            }
        }
    })
}

/// One frame of the animated water texture.
pub fn water_frame(frame: u32) -> Image {
    p(|q| {
        let ph = frame as f32 / 16.0 * std::f32::consts::TAU;
        for y in 0..S {
            for x in 0..S {
                let w = ((x as f32 * 0.8 + y as f32 * 0.4 + ph).sin() * 0.05)
                    + ((y as f32 * 0.9 - ph * 2.0).cos() * 0.03)
                    + (rnd(x, y, 18) - 0.5) * 0.03;
                q.put(x, y, Rgb(0.18 + w, 0.36 + w, 0.78 + w), 0.72);
            }
        }
    })
}

fn planks(base: Rgb, seed: u32) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let board = y / 4;
                let seam = y % 4 == 3 || (x + board * 5) % 16 == 0;
                let n = (rnd(x, y, seed) - 0.5) * 0.04 + (rnd(board, 0, seed + 1) - 0.5) * 0.07;
                // Wood grain: dark streaks along the board, lighter edge on top of each board.
                let grain = (rnd(x / 3 + board * 7, y, seed + 2) < 0.22) as u32 as f32 * -0.06;
                let edge = if y % 4 == 0 { 0.05 } else { 0.0 };
                q.put(x, y, if seam { base.scale(0.62) } else { base.add(n + grain + edge) }, 1.0);
            }
        }
    })
}

fn wool(base: Rgb, seed: u32) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                // Soft knitted rows.
                let row = if (x + (y / 2) % 2) % 2 == 0 { 0.02 } else { -0.02 };
                q.put(x, y, base.add(row + (rnd(x, y, seed) - 0.5) * 0.06), 1.0);
            }
        }
    })
}

fn polished(base: Rgb, seed: u32) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let edge = x == 0 || y == 0 || x == 15 || y == 15;
                let c = if edge { base.scale(0.82) } else { base.add((rnd(x, y, seed) - 0.5) * 0.05 + if x + y < 8 { 0.03 } else { 0.0 }) };
                q.put(x, y, c, 1.0);
            }
        }
    })
}

fn stained_glass(c: Rgb) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let edge = x == 0 || y == 0 || x == 15 || y == 15;
                if edge {
                    q.put(x, y, c.scale(0.7), 1.0);
                } else if (x + y) % 9 == 0 && x > 2 && x < 10 {
                    q.put(x, y, c.add(0.25), 0.9);
                } else {
                    q.put(x, y, c, 0.45);
                }
            }
        }
    })
}

/// 1-pixel raised edge: light top/left, dark bottom/right (inclusive corners).
fn bevel(q: &mut Painter, x0: u32, y0: u32, x1: u32, y1: u32, hi: Rgb, lo: Rgb) {
    for x in x0..=x1 {
        q.put(x, y0, hi, 1.0);
        q.put(x, y1, lo, 1.0);
    }
    for y in y0..=y1 {
        q.put(x0, y, hi, 1.0);
        q.put(x1, y, lo, 1.0);
    }
}

/// Recessed window: dark top/left edge, light bottom/right edge.
fn inset(q: &mut Painter, x0: u32, y0: u32, x1: u32, y1: u32) {
    bevel(q, x0, y0, x1, y1, Rgb(0.14, 0.15, 0.18), Rgb(0.78, 0.80, 0.84));
}

/// Brushed steel plate: lighter at the top, raised edge, dark outline, corner rivets and (for a
/// casing) a seam across the middle.
fn steel_plate(seed: u32, seam: bool) -> Image {
    p(|q| {
        for y in 0..S {
            let streak = (rnd(0, y, seed) - 0.5) * 0.05;
            for x in 0..S {
                let g = 0.60 - y as f32 * 0.012 + streak + (rnd(x, y, seed ^ 3) - 0.5) * 0.025;
                q.put(x, y, Rgb(g * 0.92, g * 0.96, g * 1.04), 1.0);
            }
        }
        bevel(q, 0, 0, 15, 15, Rgb(0.20, 0.22, 0.26), Rgb(0.16, 0.17, 0.20));
        bevel(q, 1, 1, 14, 14, Rgb(0.80, 0.83, 0.88), Rgb(0.34, 0.36, 0.41));
        if seam {
            for x in 2..14u32 {
                q.put(x, 7, Rgb(0.30, 0.32, 0.37), 1.0);
                q.put(x, 8, Rgb(0.74, 0.77, 0.82), 1.0);
            }
            for y in 3..13u32 {
                if y != 7 && y != 8 && y % 2 == 1 {
                    q.put(7, y, Rgb(0.40, 0.42, 0.47), 1.0);
                }
            }
        }
        for (x, y) in [(3u32, 3u32), (12, 3), (3, 12), (12, 12)] {
            q.put(x, y, Rgb(0.90, 0.92, 0.95), 1.0);
            q.put(x + 1, y + 1, Rgb(0.26, 0.28, 0.32), 1.0);
            q.put(x + 1, y, Rgb(0.62, 0.64, 0.68), 1.0);
            q.put(x, y + 1, Rgb(0.62, 0.64, 0.68), 1.0);
        }
    })
}

/// Dark machine casing (battery body).
fn dark_casing(seed: u32) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let g = 0.22 - y as f32 * 0.006 + (rnd(x, y, seed) - 0.5) * 0.025;
                q.put(x, y, Rgb(g * 0.95, g, g * 1.10), 1.0);
            }
        }
        bevel(q, 0, 0, 15, 15, Rgb(0.10, 0.11, 0.13), Rgb(0.06, 0.06, 0.08));
        bevel(q, 1, 1, 14, 14, Rgb(0.40, 0.43, 0.48), Rgb(0.12, 0.13, 0.15));
    })
}

/// Polished metal block: diagonal sheen, raised edge and a pressed inner frame.
fn metal_block(base: Rgb, seed: u32) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let sheen = (1.0 - ((x as f32 - y as f32 + 3.0).abs() / 6.0)).max(0.0) * 0.16;
                let t = (x + y) as f32 / 30.0;
                let c = base.scale(1.08 - t * 0.22).add(sheen + (rnd(x, y, seed) - 0.5) * 0.03);
                q.put(x, y, c, 1.0);
            }
        }
        // Four plates with a seam between them and a rivet in each corner (no "button"
        // frame-in-a-frame).
        for i in 0..S {
            q.put(i, 7, base.scale(0.62), 1.0);
            q.put(7, i, base.scale(0.62), 1.0);
            q.put(i, 8, base.scale(1.18).add(0.04), 1.0);
            q.put(8, i, base.scale(1.18).add(0.04), 1.0);
        }
        for (x, y) in [(2u32, 2u32), (13, 2), (2, 13), (13, 13)] {
            q.put(x, y, base.scale(1.35).add(0.06), 1.0);
            q.put(x + 1, y + 1, base.scale(0.6), 1.0);
        }
        bevel(q, 0, 0, 15, 15, base.scale(1.2).add(0.04), base.scale(0.55));
    })
}

/// Gem block: a grid of faceted gems.
fn gem_block(base: Rgb, seed: u32) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let (lx, ly) = (x % 4, y % 4);
                let c = match (lx, ly) {
                    (0, _) | (_, 0) => base.scale(0.62),
                    (1, 1) => base.scale(1.35).add(0.12),
                    (3, 3) => base.scale(0.82),
                    (1, _) | (_, 1) => base.scale(1.15),
                    _ => base,
                };
                q.put(x, y, c.add((rnd(x, y, seed) - 0.5) * 0.03), 1.0);
            }
        }
        bevel(q, 0, 0, 15, 15, base.scale(1.30).add(0.05), base.scale(0.45));
    })
}

/// Electric lamp: steel frame, glass with a cross grid; lit = warm glow from the centre.
fn lamp(on: bool, seed: u32) -> Image {
    p(|q| {
        for y in 0..S {
            for x in 0..S {
                let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt() / 7.5;
                let c = if on {
                    Rgb(1.0, 0.98, 0.85).lerp(Rgb(1.0, 0.78, 0.32), d.min(1.0))
                } else {
                    Rgb(0.78, 0.86, 0.92).lerp(Rgb(0.52, 0.60, 0.68), d.min(1.0))
                };
                q.put(x, y, c.add((rnd(x, y, seed) - 0.5) * 0.03), 1.0);
            }
        }
        let grid = if on { Rgb(0.85, 0.62, 0.25) } else { Rgb(0.40, 0.46, 0.52) };
        for i in 2..14u32 {
            q.put(i, 7, grid, 1.0);
            q.put(7, i, grid, 1.0);
        }
        let frame = if on { Rgb(0.62, 0.62, 0.66) } else { Rgb(0.40, 0.42, 0.47) };
        bevel(q, 0, 0, 15, 15, frame.scale(0.5), frame.scale(0.4));
        bevel(q, 1, 1, 14, 14, frame.scale(1.4), frame.scale(0.75));
    })
}

/// Smooth value noise in 0..1 with cells of `cell` pixels, tiling over a 16 px sprite.
fn value_noise(x: u32, y: u32, cell: u32, seed: u32) -> f32 {
    let n = (S / cell).max(1);
    let (gx, gy) = (x / cell, y / cell);
    let (fx, fy) = ((x % cell) as f32 / cell as f32, (y % cell) as f32 / cell as f32);
    let v = |a: u32, b: u32| rnd(a % n, b % n, seed);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let top = v(gx, gy) + (v(gx + 1, gy) - v(gx, gy)) * sx;
    let bot = v(gx, gy + 1) + (v(gx + 1, gy + 1) - v(gx, gy + 1)) * sx;
    top + (bot - top) * sy
}

/// Turns per-pixel static into clumps with a few tones: pixels lean towards their 2×2 block,
/// a soft large-scale shading is added, then brightness snaps to five bands around the mean.
fn painterly(img: &mut Image, seed: u32) {
    let w = img.width;
    let h = img.height;
    let get = |img: &Image, x: u32, y: u32| {
        let p = img.get(x % w, y % h);
        [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]
    };
    let lum = |c: [f32; 3]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    let mut mean = 0.0;
    for y in 0..h {
        for x in 0..w {
            mean += lum(get(img, x, y));
        }
    }
    mean /= (w * h) as f32;
    if mean <= 0.01 {
        return;
    }
    let src = img.clone();
    for y in 0..h {
        for x in 0..w {
            if src.get(x, y)[3] != 255 {
                continue;
            }
            let c = get(&src, x, y);
            // 2×2 clump (offset per row of clumps so they do not form a grid).
            let ox = x / 2 * 2 + (y / 2 % 2);
            let oy = y / 2 * 2;
            let mut avg = [0.0f32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let q = get(&src, ox + dx, oy + dy);
                for k in 0..3 {
                    avg[k] += q[k] * 0.25;
                }
            }
            let mut m = [0.0f32; 3];
            for k in 0..3 {
                m[k] = c[k] * 0.55 + avg[k] * 0.45;
            }
            // Large soft patches.
            let shade = (value_noise(x, y, 8, seed) - 0.5) * 0.10 + (value_noise(x, y, 4, seed ^ 0x55) - 0.5) * 0.06;
            let l = lum(m).max(0.001);
            // Five bands: ±11 % steps around the tile's mean.
            let rel = (l / mean - 1.0 + shade) * 1.15;
            let band = (rel / 0.11).round().clamp(-2.0, 2.0) * 0.11;
            let target = mean * (1.0 + band);
            let k = target / l;
            let to = |v: f32| ((v * k).clamp(0.0, 1.0) * 255.0) as u8;
            img.set(x, y, [to(m[0]), to(m[1]), to(m[2]), 255]);
        }
    }
}

pub(crate) fn build() -> Vec<(String, Image)> {
    use tiles::*;
    let mut l = SpriteList::new();
    // Calm palette: fresh green grass, warm brown earth, soft variation (little noise).
    let grass = Rgb(0.41, 0.66, 0.28);
    let dirt = Rgb(0.52, 0.37, 0.24);
    let stone = Rgb(0.50, 0.50, 0.52);

    l.add(GRASS_TOP, "grass_top", p(|q| {
        q.noisy(grass, 0.08, 1);
        q.speckle(Rgb(0.37, 0.61, 0.25), 0.18, 2);
    }));
    l.add(GRASS_SIDE, "grass_side", side_with_top(dirt, grass, 9));
    l.add(DIRT, "dirt", p(|q| {
        q.noisy(dirt, 0.09, 3);
        q.speckle(Rgb(0.46, 0.32, 0.21), 0.2, 4);
    }));
    l.add(STONE, "stone", p(|q| {
        q.noisy(stone, 0.12, 5);
        q.speckle(Rgb(0.42, 0.42, 0.44), 0.25, 6);
    }));
    l.add(COBBLESTONE, "cobblestone", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let cx = (x + (hash(y / 4, 0, 7) % 3)) % 5;
                let cy = (y + (hash(x / 5, 1, 7) % 2)) % 4;
                let mortar = cx == 0 || cy == 0;
                let n = (rnd(x, y, 8) - 0.5) * 0.12;
                let b = if mortar { 0.33 } else { 0.52 + (rnd(x / 5, y / 4, 9) - 0.5) * 0.12 };
                q.put(x, y, Rgb(b + n, b + n, b + n + 0.01), 1.0);
            }
        }
    }));
    l.add(SAND, "sand", p(|q| q.noisy(Rgb(0.86, 0.80, 0.58), 0.08, 10)));
    l.add(GRAVEL, "gravel", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let b = 0.38 + rnd(x / 2, y / 2, 12) * 0.3 + (rnd(x, y, 13) - 0.5) * 0.08;
                q.put(x, y, Rgb(b, b * 0.97, b * 0.95), 1.0);
            }
        }
    }));
    l.add(LOG_SIDE, "oak_log_side", log_side(Rgb(0.52, 0.38, 0.23), 14, false));
    l.add(LOG_TOP, "log_top", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 7.5;
                let d = (dx * dx + dy * dy).sqrt();
                // Bark round the edge, clear growth rings inside, a dark pith.
                let edge = x <= 1 || y <= 1 || x >= 14 || y >= 14;
                let ring = if (d * 0.9) as u32 % 2 == 1 { 0.14 } else { 0.0 };
                let c = if edge {
                    Rgb(0.40, 0.30, 0.18).add((rnd(x, y, 26) - 0.5) * 0.08)
                } else if d < 1.2 {
                    Rgb(0.45, 0.33, 0.20)
                } else {
                    Rgb(0.74 - ring, 0.58 - ring, 0.36 - ring * 0.8)
                };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(PLANKS, "planks", planks(Rgb(0.66, 0.50, 0.30), 15));
    l.add(LEAVES, "oak_leaves", leaves(Rgb(0.32, 0.58, 0.22), 17));
    l.add(WATER, "water", water_frame(0));
    l.add(GLASS, "glass", p(|q| {
        for i in 0..S {
            for (x, y) in [(i, 0), (i, 15), (0, i), (15, i)] {
                q.put(x, y, Rgb(0.80, 0.90, 0.95), 1.0);
            }
        }
        for i in 3..7 {
            q.put(i, i - 1, Rgb(0.95, 0.98, 1.0), 0.9);
            q.put(i + 6, i + 5, Rgb(0.95, 0.98, 1.0), 0.9);
        }
    }));
    l.add(SNOW, "snow", p(|q| q.noisy(Rgb(0.94, 0.96, 0.98), 0.05, 19)));
    l.add(SNOW_SIDE, "snow_side", side_with_top(dirt, Rgb(0.94, 0.96, 0.98), 19));
    l.add(COAL_ORE, "coal_ore", ore(Rgb(0.12, 0.12, 0.12), 20));
    l.add(IRON_ORE, "iron_ore", ore(Rgb(0.80, 0.62, 0.48), 21));
    l.add(GOLD_ORE, "gold_ore", ore(Rgb(0.95, 0.82, 0.25), 22));
    l.add(DIAMOND_ORE, "diamond_ore", ore(Rgb(0.40, 0.92, 0.88), 23));
    l.add(BEDROCK, "bedrock", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let b = 0.12 + rnd(x / 2, y / 2, 24) * 0.35;
                q.put(x, y, Rgb(b, b, b), 1.0);
            }
        }
    }));
    l.add(SANDSTONE, "sandstone", p(|q| {
        q.noisy(Rgb(0.84, 0.77, 0.55), 0.06, 11);
        for x in 0..S {
            q.put(x, 0, Rgb(0.76, 0.69, 0.48), 1.0);
            q.put(x, 11, Rgb(0.78, 0.71, 0.50), 1.0);
        }
    }));
    l.add(TABLE_TOP, "crafting_table_top", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let edge = x < 2 || y < 2 || x > 13 || y > 13;
                let grid = x % 5 == 2 || y % 5 == 2;
                let n = (rnd(x, y, 25) - 0.5) * 0.06;
                let c = if edge {
                    Rgb(0.45, 0.32, 0.18).add(n)
                } else if grid {
                    Rgb(0.38, 0.27, 0.15)
                } else {
                    Rgb(0.68, 0.52, 0.31).add(n)
                };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(TABLE_SIDE, "crafting_table_side", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let n = (rnd(x, y, 26) - 0.5) * 0.06;
                let c = if y < 3 {
                    Rgb(0.45, 0.32, 0.18).add(n)
                } else if (4..9).contains(&y) && (3..13).contains(&x) && x % 3 != 0 {
                    Rgb(0.58, 0.58, 0.60)
                } else {
                    Rgb(0.62, 0.47, 0.28).add(n)
                };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(CACTUS_SIDE, "cactus_side", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let n = (rnd(x, y, 27) - 0.5) * 0.08;
                let rib = if x % 4 == 1 { 0.06 } else { 0.0 };
                let c = if hash(x, y, 27) % 23 == 0 { Rgb(0.85, 0.85, 0.65) } else { Rgb(0.20 + n + rib, 0.50 + n + rib, 0.20 + n) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(CACTUS_TOP, "cactus_top", p(|q| q.noisy(Rgb(0.30, 0.58, 0.26), 0.08, 29)));
    l.add(TALL_GRASS, "tall_grass", p(|q| {
        for blade in 0..7u32 {
            let bx = 1 + blade * 2 + hash(blade, 0, 40) % 2;
            let h = 6 + hash(blade, 1, 40) % 9;
            for y in (S - h)..S {
                let lean = ((S - y) as i32 * (hash(blade, 2, 40) % 3) as i32 - 1) / 6;
                let x = (bx as i32 + lean).clamp(0, 15) as u32;
                q.put(x, y, Rgb(0.30, 0.58, 0.22).add((rnd(x, y, 41) - 0.5) * 0.12), 1.0);
            }
        }
    }));
    l.add(FLOWER_RED, "flower_red", flower(Rgb(0.85, 0.15, 0.15)));
    l.add(FLOWER_YELLOW, "flower_yellow", flower(Rgb(0.98, 0.85, 0.20)));
    l.add(ICE, "ice", p(|q| {
        q.noisy(Rgb(0.62, 0.78, 0.95), 0.06, 28);
        for i in 0..S {
            q.put(i, (i * 7 / 10 + 3) % 16, Rgb(0.80, 0.90, 1.0), 1.0);
        }
        for y in 0..S {
            for x in 0..S {
                let mut px = q.img.get(x, y);
                px[3] = 200;
                q.img.set(x, y, px);
            }
        }
    }));
    l.add(CLAY, "clay", p(|q| q.noisy(Rgb(0.62, 0.64, 0.70), 0.06, 30)));
    l.add(BRICKS, "bricks", bricks_pattern(Rgb(0.60, 0.28, 0.22), Rgb(0.65, 0.62, 0.58), 4, 2, 32, 0.0, false));
    l.add(LAMP, "lamp", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let frame = x == 0 || y == 0 || x == 15 || y == 15 || x == 7 || y == 7;
                let n = (rnd(x, y, 33) - 0.5) * 0.1;
                let c = if frame { Rgb(0.35, 0.30, 0.22) } else { Rgb(0.98, 0.85, 0.50).add(n) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(SPRUCE_LOG_SIDE, "spruce_log_side", log_side(Rgb(0.28, 0.20, 0.12), 34, false));
    l.add(SPRUCE_LEAVES, "spruce_leaves", leaves(Rgb(0.22, 0.44, 0.26), 37));
    l.add(DEAD_BUSH, "dead_bush", p(|q| {
        let c = Rgb(0.50, 0.36, 0.20);
        for y in 6..S {
            q.put(7, y, c, 1.0);
        }
        for i in 0..5u32 {
            q.put(7 - i, 10 - i, c, 1.0);
            q.put(8 + i, 11 - i, c, 1.0);
            q.put(8 + i / 2, 8 - i, c, 1.0);
        }
    }));
    let sb = Rgb(0.50, 0.50, 0.50);
    let sm = Rgb(0.32, 0.32, 0.32);
    l.add(STONE_BRICKS, "stone_bricks", bricks_pattern(sb, sm, 2, 1, 31, 0.0, false));
    l.add(LAVA, "lava", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let t = rnd(x / 2, y / 2, 80) * 0.6 + rnd(x, y, 81) * 0.4;
                let c = Rgb(0.85, 0.30, 0.05).lerp(Rgb(1.0, 0.75, 0.20), t);
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(BASALT_SIDE, "basalt_side", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let col = (rnd(x / 3, 0, 82) - 0.5) * 0.08;
                let n = (rnd(x, y, 83) - 0.5) * 0.06;
                let line = if x % 4 == 0 { -0.05 } else { 0.0 };
                q.put(x, y, Rgb(0.24, 0.24, 0.27).add(col + n + line), 1.0);
            }
        }
    }));
    l.add(BASALT_TOP, "basalt_top", p(|q| {
        q.noisy(Rgb(0.26, 0.26, 0.29), 0.08, 84);
        for i in 0..S {
            q.put(i, i / 2 + 4, Rgb(0.18, 0.18, 0.2), 1.0);
            q.put(15 - i / 2, i, Rgb(0.18, 0.18, 0.2), 1.0);
        }
    }));
    l.add(OBSIDIAN, "obsidian", p(|q| {
        q.noisy(Rgb(0.10, 0.07, 0.16), 0.06, 85);
        q.speckle(Rgb(0.30, 0.18, 0.45), 0.12, 86);
    }));
    l.add(MUD, "mud", p(|q| {
        q.noisy(Rgb(0.30, 0.25, 0.22), 0.08, 87);
        q.speckle(Rgb(0.24, 0.20, 0.18), 0.3, 88);
    }));
    l.add(MOSSY_COBBLESTONE, "mossy_cobblestone", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let cx = (x + (hash(y / 4, 0, 7) % 3)) % 5;
                let cy = (y + (hash(x / 5, 1, 7) % 2)) % 4;
                let mortar = cx == 0 || cy == 0;
                let n = (rnd(x, y, 8) - 0.5) * 0.12;
                let mut c = if mortar { Rgb(0.33, 0.33, 0.33) } else { Rgb(0.52, 0.52, 0.53).add(n) };
                if rnd(x / 2, y / 2, 89) < 0.35 {
                    c = Rgb(0.32, 0.50, 0.24).add(n);
                }
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(BIRCH_LOG_SIDE, "birch_log_side", log_side(Rgb(0.88, 0.87, 0.82), 90, true));
    l.add(BIRCH_LEAVES, "birch_leaves", leaves(Rgb(0.42, 0.60, 0.26), 91));
    l.add(ACACIA_LOG_SIDE, "acacia_log_side", log_side(Rgb(0.55, 0.43, 0.32), 92, false));
    l.add(ACACIA_LEAVES, "acacia_leaves", leaves(Rgb(0.42, 0.60, 0.24), 93));
    l.add(MUSHROOM_RED, "mushroom_red", mushroom(Rgb(0.80, 0.15, 0.12), true));
    l.add(MUSHROOM_BROWN, "mushroom_brown", mushroom(Rgb(0.55, 0.40, 0.28), false));
    l.add(SEAGRASS, "seagrass", p(|q| {
        for blade in 0..5u32 {
            let bx = 2 + blade * 3;
            let h = 9 + hash(blade, 1, 94) % 7;
            for y in (S - h)..S {
                let wob = (((y as f32) * 0.7 + blade as f32).sin() * 1.2) as i32;
                let x = (bx as i32 + wob).clamp(0, 15) as u32;
                q.put(x, y, Rgb(0.20, 0.55, 0.30).add((rnd(x, y, 95) - 0.5) * 0.1), 1.0);
            }
        }
    }));
    l.add(KELP, "kelp", p(|q| {
        for y in 0..S {
            let wob = ((y as f32 * 0.5).sin() * 1.5) as i32;
            for dx in -1..=1 {
                let x = (7 + wob + dx) as u32;
                q.put(x, y, Rgb(0.25, 0.45, 0.15).add((rnd(x, y, 96) - 0.5) * 0.1), 1.0);
            }
            if y % 4 == 0 {
                q.put((9 + wob) as u32, y, Rgb(0.32, 0.55, 0.18), 1.0);
                q.put((10 + wob) as u32, y + 1, Rgb(0.32, 0.55, 0.18), 1.0);
            }
        }
    }));
    l.add(SHRUB, "shrub", p(|q| {
        let stem = Rgb(0.40, 0.28, 0.16);
        for y in 10..S {
            q.put(7, y, stem, 1.0);
            q.put(8, y, stem, 1.0);
        }
        q.shape(Rgb(0.22, 0.48, 0.18), &|u, v| ((u - 0.5) / 0.45).powi(2) + ((v - 0.45) / 0.32).powi(2) < 1.0);
        for y in 0..S {
            for x in 0..S {
                let px = q.img.get(x, y);
                if px[3] > 0 && rnd(x, y, 97) < 0.2 {
                    q.put(x, y, Rgb(0.16, 0.38, 0.14), 1.0);
                }
            }
        }
    }));
    let dry = Rgb(0.55, 0.62, 0.32);
    l.add(DRY_GRASS_TOP, "dry_grass_top", p(|q| {
        q.noisy(dry, 0.08, 98);
        q.speckle(Rgb(0.51, 0.58, 0.30), 0.18, 99);
    }));
    l.add(DRY_GRASS_SIDE, "dry_grass_side", side_with_top(dirt, dry, 100));
    l.add(MOSSY_STONE_BRICKS, "mossy_stone_bricks", bricks_pattern(sb, sm, 2, 1, 101, 0.3, false));
    l.add(CRACKED_STONE_BRICKS, "cracked_stone_bricks", bricks_pattern(sb, sm, 2, 1, 102, 0.0, true));
    let wood = Rgb(0.55, 0.38, 0.20);
    let band = Rgb(0.30, 0.22, 0.12);
    l.add(CHEST_SIDE, "chest_side", p(|q| {
        q.noisy(wood, 0.06, 103);
        for i in 0..S {
            q.put(i, 0, band, 1.0);
            q.put(i, 15, band, 1.0);
            q.put(0, i, band, 1.0);
            q.put(15, i, band, 1.0);
            q.put(i, 6, band, 1.0);
        }
    }));
    l.add(CHEST_TOP, "chest_top", p(|q| {
        q.noisy(wood, 0.06, 104);
        for i in 0..S {
            q.put(i, 0, band, 1.0);
            q.put(i, 15, band, 1.0);
            q.put(0, i, band, 1.0);
            q.put(15, i, band, 1.0);
        }
    }));
    l.add(CHEST_FRONT, "chest_front", p(|q| {
        q.noisy(wood, 0.06, 105);
        for i in 0..S {
            q.put(i, 0, band, 1.0);
            q.put(i, 15, band, 1.0);
            q.put(0, i, band, 1.0);
            q.put(15, i, band, 1.0);
            q.put(i, 6, band, 1.0);
        }
        for y in 5..9 {
            for x in 7..9 {
                q.put(x, y, Rgb(0.85, 0.80, 0.40), 1.0);
            }
        }
    }));
    l.add(ASH, "ash", p(|q| {
        q.noisy(Rgb(0.40, 0.39, 0.38), 0.1, 106);
        q.speckle(Rgb(0.28, 0.27, 0.27), 0.3, 107);
    }));
    l.add(PACKED_ICE, "packed_ice", p(|q| {
        q.noisy(Rgb(0.55, 0.70, 0.92), 0.05, 108);
        q.speckle(Rgb(0.72, 0.84, 0.98), 0.15, 109);
    }));
    l.add(GOLD_BLOCK, "gold_block", metal_block(Rgb(0.98, 0.80, 0.26), 110));
    l.add(RED_SAND, "red_sand", p(|q| q.noisy(Rgb(0.76, 0.42, 0.20), 0.08, 111)));
    l.add(TERRACOTTA, "terracotta", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let layer = (rnd(0, y / 3, 112) - 0.5) * 0.12;
                let n = (rnd(x, y, 113) - 0.5) * 0.05;
                q.put(x, y, Rgb(0.62, 0.38, 0.28).add(layer + n), 1.0);
            }
        }
    }));
    let swamp = Rgb(0.30, 0.42, 0.22);
    l.add(SWAMP_GRASS_TOP, "swamp_grass_top", p(|q| {
        q.noisy(swamp, 0.16, 114);
        q.speckle(Rgb(0.24, 0.35, 0.18), 0.3, 115);
    }));
    l.add(SWAMP_GRASS_SIDE, "swamp_grass_side", side_with_top(dirt, swamp, 116));
    l.add(MAGMA, "magma", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let crack = rnd(x / 3, y / 3, 117) < 0.3 && (x + y) % 3 == 0;
                let c = if crack { Rgb(1.0, 0.55, 0.10) } else { Rgb(0.35, 0.12, 0.06).add((rnd(x, y, 118) - 0.5) * 0.08) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    // ---- Stones ----
    l.add(GRANITE, "granite", p(|q| {
        q.noisy(Rgb(0.60, 0.40, 0.34), 0.10, 120);
        q.speckle(Rgb(0.45, 0.28, 0.24), 0.25, 121);
        q.speckle(Rgb(0.80, 0.66, 0.60), 0.12, 122);
    }));
    l.add(MARBLE, "marble", p(|q| {
        q.noisy(Rgb(0.90, 0.90, 0.92), 0.04, 123);
        for i in 0..S {
            let y = (i as f32 * 0.6 + (rnd(i, 0, 124) * 2.0)) as u32 % S;
            q.put(i, y, Rgb(0.68, 0.70, 0.76), 1.0);
        }
    }));
    l.add(SLATE, "slate", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let layer = (rnd(0, y / 2, 125) - 0.5) * 0.08;
                let n = (rnd(x, y, 126) - 0.5) * 0.05;
                q.put(x, y, Rgb(0.30, 0.34, 0.40).add(layer + n), 1.0);
            }
        }
    }));
    l.add(LIMESTONE, "limestone", p(|q| {
        q.noisy(Rgb(0.78, 0.76, 0.66), 0.07, 127);
        q.speckle(Rgb(0.66, 0.64, 0.55), 0.2, 128);
    }));
    // ---- Ores and metal blocks ----
    l.add(COPPER_ORE, "copper_ore", ore(Rgb(0.82, 0.48, 0.28), 129));
    l.add(TIN_ORE, "tin_ore", ore(Rgb(0.80, 0.84, 0.86), 130));
    l.add(SILVER_ORE, "silver_ore", ore(Rgb(0.88, 0.90, 0.96), 131));
    l.add(COBALT_ORE, "cobalt_ore", ore(Rgb(0.20, 0.35, 0.85), 132));
    l.add(COPPER_BLOCK, "copper_block", metal_block(Rgb(0.80, 0.46, 0.30), 133));
    l.add(IRON_BLOCK, "iron_block", metal_block(Rgb(0.80, 0.81, 0.84), 134));
    l.add(DIAMOND_BLOCK, "diamond_block", gem_block(Rgb(0.36, 0.86, 0.84), 135));
    l.add(COAL_BLOCK, "coal_block", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let lump = value_noise(x, y, 4, 136);
                let b = 0.07 + lump * 0.10 + (rnd(x, y, 137) - 0.5) * 0.03;
                q.put(x, y, Rgb(b, b, b * 1.06), 1.0);
                // Glints on the lumps.
                if lump > 0.72 && rnd(x, y, 138) < 0.25 {
                    q.put(x, y, Rgb(0.34, 0.34, 0.38), 1.0);
                }
            }
        }
        bevel(q, 0, 0, 15, 15, Rgb(0.24, 0.24, 0.27), Rgb(0.02, 0.02, 0.03));
    }));
    l.add(BRONZE_BLOCK, "bronze_block", metal_block(Rgb(0.72, 0.52, 0.24), 137));
    l.add(SALT, "salt_block", p(|q| {
        q.noisy(Rgb(0.93, 0.92, 0.90), 0.04, 138);
        q.speckle(Rgb(0.80, 0.80, 0.82), 0.15, 139);
    }));
    l.add(PEAT, "peat", p(|q| {
        q.noisy(Rgb(0.25, 0.17, 0.11), 0.08, 140);
        q.speckle(Rgb(0.15, 0.10, 0.07), 0.3, 141);
    }));
    l.add(GLOWSTONE, "glowstone", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let t = rnd(x / 2, y / 2, 142) * 0.7 + rnd(x, y, 143) * 0.3;
                q.put(x, y, Rgb(0.70, 0.50, 0.18).lerp(Rgb(1.0, 0.90, 0.50), t), 1.0);
            }
        }
    }));
    // ---- Tech ----
    l.add(MACHINE_CASING, "machine_casing", steel_plate(144, true));
    l.add(GENERATOR_FRONT, "generator_front", {
        let mut q = Painter { img: steel_plate(145, false) };
        inset(&mut q, 3, 4, 12, 12);
        for y in 5..12u32 {
            for x in 4..12u32 {
                // Fire: bright at the bottom, flickering tongues upwards.
                let h = (11 - y) as f32 / 6.0;
                let tongue = value_noise(x, 0, 2, 146) * 0.6 + 0.2;
                let heat = (1.0 - h / (tongue + 0.35)).clamp(0.0, 1.0);
                let c = Rgb(0.20, 0.05, 0.03).lerp(Rgb(0.95, 0.35, 0.05), heat).lerp(Rgb(1.0, 0.88, 0.40), (heat - 0.6).max(0.0) * 2.0);
                q.put(x, y, c, 1.0);
            }
        }
        for y in 5..12u32 {
            for x in [6u32, 9] {
                q.put(x, y, Rgb(0.16, 0.17, 0.20), 1.0);
            }
        }
        q.finish()
    });
    l.add(BONES, "bones", p(|q| {
        // Ivory bone surface: soft rings and darker pores, like a cut through bone.
        for y in 0..S {
            for x in 0..S {
                let n = (rnd(x, y, 146) - 0.5) * 0.06;
                let ring = (((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt() * 0.9).sin() * 0.025;
                let pore = rnd(x, y, 147) < 0.05;
                let c = if pore { Rgb(0.70, 0.66, 0.56) } else { Rgb(0.93, 0.90, 0.80).add(n + ring) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(BATTERY_SIDE, "battery_side", {
        let mut q = Painter { img: dark_casing(147) };
        inset(&mut q, 4, 2, 11, 13);
        for y in 3..13u32 {
            for x in 5..11u32 {
                let level = y >= 6;
                let seg_line = (y - 3) % 2 == 1 && y < 12;
                let shine = if x == 5 { 0.10 } else if x == 10 { -0.08 } else { 0.0 };
                let c = if seg_line {
                    Rgb(0.06, 0.08, 0.07)
                } else if level {
                    Rgb(0.18, 0.75, 0.30).lerp(Rgb(0.45, 0.98, 0.50), (12 - y) as f32 / 6.0)
                } else {
                    Rgb(0.10, 0.22, 0.13)
                };
                q.put(x, y, c.add(shine), 1.0);
            }
        }
        for y in [4u32, 7, 10] {
            q.put(12, y, Rgb(0.70, 0.74, 0.80), 1.0);
        }
        q.finish()
    });
    l.add(BATTERY_TOP, "battery_top", {
        let mut q = Painter { img: steel_plate(148, false) };
        for (cx, plus) in [(5u32, true), (10u32, false)] {
            for y in 6..10u32 {
                for x in cx - 2..cx + 2 {
                    let c = Rgb(0.88, 0.55, 0.34).lerp(Rgb(0.55, 0.30, 0.18), (y - 6) as f32 / 3.0);
                    q.put(x, y, c, 1.0);
                }
            }
            let mark = if plus { Rgb(0.90, 0.20, 0.18) } else { Rgb(0.15, 0.15, 0.18) };
            q.put(cx - 1, 3, mark, 1.0);
            q.put(cx, 3, mark, 1.0);
            if plus {
                q.put(cx - 1, 2, mark, 1.0);
                q.put(cx - 1, 4, mark, 1.0);
                q.put(cx - 2, 3, mark, 1.0);
            }
        }
        q.finish()
    });
    l.add(ELECTRIC_LAMP, "electric_lamp", lamp(false, 149));
    l.add(FURNACE_FRONT, "electric_furnace_front", {
        let mut q = Painter { img: steel_plate(150, false) };
        inset(&mut q, 3, 5, 12, 13);
        for y in 6..13u32 {
            for x in 4..12u32 {
                let coil = y >= 9 && (y - 9) % 2 == 0;
                let glow = (y as f32 - 6.0) / 6.0;
                let c = if coil { Rgb(1.0, 0.62, 0.18).lerp(Rgb(1.0, 0.90, 0.55), (rnd(x, y, 150) < 0.3) as u32 as f32) } else { Rgb(0.10, 0.07, 0.07).lerp(Rgb(0.55, 0.18, 0.06), glow * glow) };
                q.put(x, y, c, 1.0);
            }
        }
        // Little display above the chamber.
        for x in 5..11u32 {
            q.put(x, 2, Rgb(0.08, 0.10, 0.12), 1.0);
            q.put(x, 3, Rgb(0.08, 0.10, 0.12), 1.0);
        }
        q.put(6, 2, Rgb(1.0, 0.55, 0.15), 1.0);
        q.put(7, 2, Rgb(1.0, 0.55, 0.15), 1.0);
        q.finish()
    });
    l.add(ELECTRIC_LAMP_ON, "electric_lamp_on", lamp(true, 151));
    // Lever (the "switch" slots): side and top tiles of a 3D lever. The base boxes sit in the
    // bottom three rows of the side tile (cobblestone), the handle above (wood with a red tip);
    // on top the cobblestone base has a dark slot where the handle stands.
    let cobble_px = |x: u32, y: u32| {
        let cx = (x + (hash(y / 4, 0, 7) % 3)) % 5;
        let cy = (y + (hash(x / 5, 1, 7) % 2)) % 4;
        let b = if cx == 0 || cy == 0 { 0.36 } else { 0.56 + (rnd(x / 5, y / 4, 9) - 0.5) * 0.12 };
        Rgb(b, b, b + 0.01)
    };
    l.add(SWITCH_OFF, "switch_off", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let c = if y >= 13 {
                    cobble_px(x, y).scale(if y == 13 { 1.15 } else { 1.0 })
                } else if (3..7).contains(&y) {
                    // Red knob at the end of the handle.
                    Rgb(0.86, 0.16, 0.14).lerp(Rgb(1.0, 0.45, 0.40), if y == 3 { 0.6 } else { 0.0 }).scale(if y == 6 { 0.75 } else { 1.0 })
                } else {
                    let g = (rnd(x, y / 3, 152) - 0.5) * 0.08;
                    Rgb(0.55, 0.38, 0.20).add(g).scale(if x % 4 == 0 { 0.85 } else { 1.0 })
                };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(SWITCH_ON, "switch_on", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let slot = (6..10).contains(&x) && (3..13).contains(&y);
                let c = if slot { Rgb(0.30, 0.20, 0.11).add((rnd(x, y, 153) - 0.5) * 0.05) } else { cobble_px(x, y) };
                q.put(x, y, c, 1.0);
            }
        }
        bevel(q, 0, 0, 15, 15, Rgb(0.66, 0.66, 0.68), Rgb(0.30, 0.30, 0.32));
    }));
    l.add(COMMAND_BLOCK, "command_block", p(|q| {
        q.noisy(Rgb(0.78, 0.45, 0.18), 0.05, 154);
        for y in 0..S {
            for x in 0..S {
                if x < 2 || y < 2 || x > 13 || y > 13 {
                    q.put(x, y, Rgb(0.45, 0.24, 0.10), 1.0);
                }
            }
        }
        // ">_" prompt
        for (x, y) in [(4u32, 5u32), (5, 6), (6, 7), (5, 8), (4, 9), (8, 10), (9, 10), (10, 10), (11, 10)] {
            q.put(x, y, Rgb(1.0, 0.92, 0.55), 1.0);
        }
    }));
    l.add(NETHERRACK, "netherrack", p(|q| {
        q.noisy(Rgb(0.46, 0.17, 0.17), 0.09, 156);
        for y in 0..S {
            for x in 0..S {
                if rnd(x, y, 157) > 0.86 {
                    q.put(x, y, Rgb(0.30, 0.08, 0.10), 1.0);
                } else if rnd(x, y, 158) > 0.95 {
                    q.put(x, y, Rgb(0.66, 0.30, 0.22), 1.0);
                }
            }
        }
    }));
    l.add(PORTAL, "portal", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let w = ((x as f32 * 0.9 + y as f32 * 0.5).sin() * 0.5 + 0.5) * 0.35;
                let n = (rnd(x, y, 159) - 0.5) * 0.12;
                q.put(x, y, Rgb(0.45 + w, 0.10 + w * 0.4, 0.80 + n), 0.82);
            }
        }
    }));
    l.add(TERMINAL, "terminal", p(|q| {
        q.noisy(Rgb(0.16, 0.18, 0.22), 0.03, 155);
        for y in 2..14u32 {
            for x in 2..14u32 {
                q.put(x, y, Rgb(0.04, 0.10, 0.07), 1.0);
            }
        }
        for (x, y) in [(3u32, 4u32), (4, 4), (5, 4), (3, 6), (4, 6), (5, 6), (6, 6), (7, 6), (3, 8), (4, 8), (3, 10), (4, 10), (5, 10), (6, 10)] {
            q.put(x, y, Rgb(0.30, 0.95, 0.45), 1.0);
        }
    }));
    // ---- biome and building blocks ----
    l.add(WOOL, "wool", wool(Rgb(0.92, 0.92, 0.89), 160));
    l.add(BIRCH_PLANKS, "birch_planks", planks(Rgb(0.82, 0.74, 0.52), 161));
    l.add(SPRUCE_PLANKS, "spruce_planks", planks(Rgb(0.44, 0.31, 0.19), 162));
    l.add(ACACIA_PLANKS, "acacia_planks", planks(Rgb(0.74, 0.42, 0.24), 163));
    l.add(TORCH, "torch", p(|q| {
        // A short stick (10/16 of a block, like the classic torch) with a flame on top.
        for y in 9..16u32 {
            q.put(7, y, Rgb(0.50, 0.36, 0.20), 1.0);
            q.put(8, y, Rgb(0.40, 0.28, 0.15), 1.0);
        }
        for (x, y, c) in [(7u32, 7u32, Rgb(1.0, 0.85, 0.30)), (8, 7, Rgb(1.0, 0.75, 0.25)), (7, 8, Rgb(1.0, 0.60, 0.15)), (8, 8, Rgb(0.95, 0.50, 0.10)), (7, 6, Rgb(1.0, 0.95, 0.60)), (8, 6, Rgb(1.0, 0.90, 0.50))] {
            q.put(x, y, c, 1.0);
        }
    }));
    l.add(PUMPKIN_SIDE, "pumpkin_side", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let rib = if x % 4 == 0 { -0.10 } else { 0.0 };
                q.put(x, y, Rgb(0.88, 0.52, 0.12).add(rib + (rnd(x, y, 164) - 0.5) * 0.05), 1.0);
            }
        }
    }));
    l.add(PUMPKIN_TOP, "pumpkin_top", p(|q| {
        q.noisy(Rgb(0.86, 0.50, 0.12), 0.05, 165);
        for (x, y) in [(7u32, 7u32), (8, 7), (7, 8), (8, 8)] {
            q.put(x, y, Rgb(0.40, 0.45, 0.18), 1.0);
        }
    }));
    l.add(MELON_SIDE, "melon_side", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let stripe = if (x / 2) % 2 == 0 { 0.06 } else { -0.04 };
                q.put(x, y, Rgb(0.42, 0.62, 0.22).add(stripe + (rnd(x, y, 166) - 0.5) * 0.05), 1.0);
            }
        }
    }));
    l.add(MELON_TOP, "melon_top", p(|q| {
        q.noisy(Rgb(0.45, 0.64, 0.24), 0.06, 167);
        q.put(7, 7, Rgb(0.45, 0.35, 0.18), 1.0);
    }));
    l.add(SUGAR_CANE, "sugar_cane", p(|q| {
        for (x0, seed) in [(3u32, 168u32), (7, 169), (11, 170)] {
            for y in 0..S {
                let joint = (y + seed) % 5 == 0;
                let c = if joint { Rgb(0.45, 0.62, 0.30) } else { Rgb(0.58, 0.78, 0.40) };
                q.put(x0, y, c, 1.0);
                q.put(x0 + 1, y, c.scale(0.88), 1.0);
            }
            q.put(x0 + 2, 4 + seed % 5, Rgb(0.50, 0.72, 0.34), 1.0);
        }
    }));
    l.add(FERN, "fern", p(|q| {
        let g = Rgb(0.30, 0.56, 0.24);
        for y in 3..16u32 {
            q.put(7, y, g.scale(0.85), 1.0);
            let w = (16 - y) / 3;
            for d in 1..=w {
                if y % 2 == 0 {
                    q.put(7 - d, y - d / 2, g.add((rnd(d, y, 171) - 0.5) * 0.1), 1.0);
                    q.put(7 + d, y - d / 2, g.add((rnd(d, y, 172) - 0.5) * 0.1), 1.0);
                }
            }
        }
    }));
    l.add(FLOWER_BLUE, "flower_blue", flower(Rgb(0.30, 0.45, 0.92)));
    let podzol = Rgb(0.42, 0.30, 0.18);
    l.add(PODZOL_TOP, "podzol_top", p(|q| {
        q.noisy(podzol, 0.08, 173);
        q.speckle(Rgb(0.55, 0.40, 0.20), 0.2, 174);
        q.speckle(Rgb(0.28, 0.40, 0.20), 0.08, 175);
    }));
    l.add(PODZOL_SIDE, "podzol_side", side_with_top(dirt, podzol, 176));
    l.add(MOSS, "moss", p(|q| {
        q.noisy(Rgb(0.36, 0.52, 0.22), 0.08, 177);
        q.speckle(Rgb(0.44, 0.60, 0.26), 0.2, 178);
    }));
    l.add(POLISHED_GRANITE, "polished_granite", polished(Rgb(0.66, 0.44, 0.38), 179));
    l.add(POLISHED_MARBLE, "polished_marble", polished(Rgb(0.90, 0.89, 0.86), 180));
    l.add(SMOOTH_STONE, "smooth_stone", polished(Rgb(0.60, 0.60, 0.62), 181));
    l.add(SANDSTONE_BRICKS, "sandstone_bricks", bricks_pattern(Rgb(0.86, 0.79, 0.58), Rgb(0.70, 0.62, 0.44), 4, 2, 182, 0.0, false));
    l.add(BOOKSHELF, "bookshelf", p(|q| {
        let colors = [Rgb(0.62, 0.20, 0.18), Rgb(0.22, 0.36, 0.62), Rgb(0.30, 0.52, 0.26), Rgb(0.70, 0.58, 0.22), Rgb(0.45, 0.28, 0.50)];
        for y in 0..S {
            for x in 0..S {
                let shelf = y == 0 || y == 7 || y == 8 || y == 15 || x == 0 || x == 15;
                let c = if shelf {
                    Rgb(0.60, 0.45, 0.26).add((rnd(x, y, 183) - 0.5) * 0.06)
                } else {
                    let book = (x + (y / 8) * 3) / 2;
                    colors[(hash(book, y / 8, 184) % 5) as usize].add(if x % 2 == 0 { 0.04 } else { -0.04 })
                };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(LANTERN, "lantern", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let frame = x < 2 || x > 13 || y < 2 || y > 13 || x == 7 || x == 8;
                let c = if frame { Rgb(0.25, 0.25, 0.28) } else { Rgb(1.0, 0.82, 0.40).add((rnd(x, y, 185) - 0.5) * 0.08) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(WOOL_RED, "wool_red", wool(Rgb(0.74, 0.20, 0.20), 186));
    l.add(WOOL_BLUE, "wool_blue", wool(Rgb(0.22, 0.34, 0.70), 187));
    l.add(WOOL_YELLOW, "wool_yellow", wool(Rgb(0.92, 0.80, 0.25), 188));
    l.add(GLASS_RED, "glass_red", stained_glass(Rgb(0.80, 0.20, 0.18)));
    l.add(GLASS_BLUE, "glass_blue", stained_glass(Rgb(0.22, 0.36, 0.85)));
    l.add(GLASS_GREEN, "glass_green", stained_glass(Rgb(0.28, 0.70, 0.30)));
    l.add(GLASS_YELLOW, "glass_yellow", stained_glass(Rgb(0.95, 0.82, 0.22)));
    // ---- Iskrit: a glowing violet-pink crystal dust that carries signals ----
    let isk = Rgb(0.86, 0.22, 0.62);
    l.add(ISKRIT_ORE, "iskrit_ore", p(|q| {
        q.noisy(Rgb(0.50, 0.50, 0.52), 0.12, 190);
        q.speckle(Rgb(0.42, 0.42, 0.44), 0.25, 191);
        for y in 0..S {
            for x in 0..S {
                if rnd(x / 2, y / 2, 192) < 0.16 && rnd(x, y, 193) < 0.8 {
                    let hi = rnd(x, y, 194) < 0.3;
                    q.put(x, y, if hi { Rgb(1.0, 0.55, 0.85) } else { isk }, 1.0);
                }
            }
        }
    }));
    // Iskrit dust, like the classic dust: a speckled cross with a round blob in the middle.
    // Off it is a dark wine colour, on a bright glowing pink with light sparkles.
    let wire = |on: bool| {
        p(|q| {
            q.clear();
            let (core, mid, edge) = if on {
                (Rgb(1.0, 0.62, 0.90), Rgb(1.0, 0.30, 0.74), Rgb(0.78, 0.12, 0.52))
            } else {
                (Rgb(0.52, 0.10, 0.34), Rgb(0.40, 0.07, 0.26), Rgb(0.26, 0.04, 0.17))
            };
            for y in 0..S {
                for x in 0..S {
                    let (fx, fy) = (x as f32 - 7.5, y as f32 - 7.5);
                    let blob = (fx * fx + fy * fy).sqrt() < 3.4;
                    let arm = fx.abs() < 1.6 || fy.abs() < 1.6;
                    let fringe = (fx.abs() < 2.6 || fy.abs() < 2.6) && rnd(x, y, 195) < 0.45;
                    if !(blob || arm || fringe) {
                        continue;
                    }
                    let d = fx.abs().min(fy.abs()).min((fx * fx + fy * fy).sqrt() - 1.0).max(0.0);
                    let c = if d < 0.8 { core.lerp(mid, rnd(x, y, 196) * 0.6) } else if d < 1.6 || blob { mid } else { edge };
                    q.put(x, y, c, 1.0);
                }
            }
            if on {
                for (x, y) in [(7u32, 7u32), (8, 8), (2, 7), (13, 8), (7, 1), (8, 14), (10, 6)] {
                    q.put(x, y, Rgb(1.0, 0.92, 0.98), 1.0);
                }
            }
        })
    };
    l.add(ISKRIT_WIRE, "iskrit_wire", wire(false));
    l.add(ISKRIT_WIRE_ON, "iskrit_wire_on", wire(true));
    l.add(ISKRIT_BLOCK, "iskrit_block", p(|q| {
        // Solid iskrit: a glowing pink block with a lattice of crystal facets and sparks.
        for y in 0..S {
            for x in 0..S {
                let t = value_noise(x, y, 4, 196);
                let mut c = Rgb(0.70, 0.12, 0.46).lerp(Rgb(1.0, 0.42, 0.80), t);
                if (x + y) % 6 == 0 || (x + 16 - y) % 6 == 0 {
                    c = c.scale(0.78);
                }
                if rnd(x, y, 197) < 0.05 {
                    c = Rgb(1.0, 0.90, 0.97);
                }
                q.put(x, y, c, 1.0);
            }
        }
        bevel(q, 0, 0, 15, 15, Rgb(1.0, 0.66, 0.90), Rgb(0.42, 0.05, 0.27));
    }));
    l.add(ISKRIT_TORCH, "iskrit_torch", p(|q| {
        // Wooden stick with a glowing iskrit head and a soft halo round it.
        q.clear();
        for y in 9..16u32 {
            q.put(7, y, Rgb(0.50, 0.34, 0.18), 1.0);
            q.put(8, y, Rgb(0.38, 0.25, 0.13), 1.0);
        }
        for y in 4..10u32 {
            for x in 5..11u32 {
                let (fx, fy) = (x as f32 - 7.5, y as f32 - 7.0);
                let d = (fx * fx + fy * fy).sqrt();
                if d > 2.8 {
                    continue;
                }
                let c = if d < 1.0 { Rgb(1.0, 0.90, 0.97) } else if d < 1.9 { Rgb(1.0, 0.40, 0.80) } else { Rgb(0.80, 0.14, 0.52) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    // Iskrit lamp: a dark frame holding four crystal panes; lit, the crystals glow pink-white.
    let isk_lamp = |on: bool| {
        p(|q| {
            for y in 0..S {
                for x in 0..S {
                    let frame = x == 0 || y == 0 || x == 15 || y == 15 || x == 7 || x == 8 || y == 7 || y == 8;
                    let c = if frame {
                        if on { Rgb(0.62, 0.40, 0.30) } else { Rgb(0.34, 0.22, 0.16) }
                    } else {
                        // Each pane: brighter towards its centre, crystal grain.
                        let (cx, cy) = (if x < 8 { 3.5 } else { 11.5 }, if y < 8 { 3.5 } else { 11.5 });
                        let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / 4.0;
                        let grain = (rnd(x, y, 198) - 0.5) * 0.12;
                        if on {
                            Rgb(1.0, 0.95, 0.88).lerp(Rgb(1.0, 0.42, 0.78), d.min(1.0)).add(grain)
                        } else {
                            Rgb(0.46, 0.22, 0.34).lerp(Rgb(0.24, 0.10, 0.18), d.min(1.0)).add(grain)
                        }
                    };
                    q.put(x, y, c, 1.0);
                }
            }
            bevel(q, 0, 0, 15, 15, if on { Rgb(0.80, 0.56, 0.42) } else { Rgb(0.48, 0.32, 0.24) }, Rgb(0.20, 0.12, 0.09));
        })
    };
    l.add(ISKRIT_LAMP, "iskrit_lamp", isk_lamp(false));
    l.add(ISKRIT_LAMP_ON, "iskrit_lamp_on", isk_lamp(true));
    l.add(BUTTON, "button", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let g = 0.62 - y as f32 * 0.01 + (rnd(x, y, 198) - 0.5) * 0.04;
                q.put(x, y, Rgb(g, g, g * 1.02), 1.0);
            }
        }
        bevel(q, 0, 0, 15, 15, Rgb(0.82, 0.82, 0.84), Rgb(0.36, 0.36, 0.38));
    }));
    let doors = [(Rgb(0.62, 0.45, 0.25), false), (Rgb(0.84, 0.76, 0.55), false), (Rgb(0.42, 0.30, 0.18), false), (Rgb(0.74, 0.76, 0.80), true)];
    for (i, (name, (wood, iron))) in ["oak", "birch", "spruce", "iron"].iter().zip(doors).enumerate() {
        l.add(DOOR_OAK + i as u16, &format!("door_{name}"), door(wood, iron, 201 + i as u32, false));
    }
    for (i, (name, (wood, iron))) in ["oak", "birch", "spruce", "iron"].iter().zip(doors).enumerate() {
        l.add(DOOR_OAK_TOP + i as u16, &format!("door_{name}_top"), door(wood, iron, 201 + i as u32, true));
    }
    l.add(URANIUM_ORE, "uranium_ore", ore(Rgb(0.45, 0.95, 0.30), 205));
    l.add(URANIUM_BLOCK, "uranium_block", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let glow = 0.75 + 0.25 * rnd(x / 2, y / 2, 206);
                let rod = x % 5 == 2;
                let c = if rod { Rgb(0.55, 1.0, 0.35).scale(glow) } else { Rgb(0.20, 0.42, 0.16).scale(glow) };
                q.put(x, y, c, 1.0);
            }
        }
        bevel(q, 0, 0, 15, 15, Rgb(0.62, 0.66, 0.60), Rgb(0.22, 0.26, 0.22));
    }));
    // Pistons: planks face, cobblestone body with a planks band, iron back plate.
    let plank_c = Rgb(0.66, 0.50, 0.30);
    l.add(PISTON_SIDE, "piston_side", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let c = if y < 4 { plank_c.add((rnd(x / 3, y, 240) - 0.5) * 0.08) } else { Rgb(0.48, 0.48, 0.48).add((rnd(x / 2, y / 2, 241) - 0.5) * 0.14) };
                q.put(x, y, c, 1.0);
            }
        }
        for x in 0..S {
            q.put(x, 4, Rgb(0.30, 0.30, 0.30), 1.0);
        }
        for y in 5..12u32 {
            for x in 6..10u32 {
                q.put(x, y, Rgb(0.70, 0.62, 0.40), 1.0);
            }
        }
    }));
    l.add(PISTON_FRONT, "piston_front", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let c = plank_c.add((rnd(x / 3, y, 242) - 0.5) * 0.08 + if y % 4 == 0 { -0.08 } else { 0.0 });
                q.put(x, y, c, 1.0);
            }
        }
        for y in 6..10u32 {
            for x in 6..10u32 {
                q.put(x, y, Rgb(0.62, 0.64, 0.68), 1.0);
            }
        }
        bevel(q, 0, 0, 15, 15, plank_c.add(0.12), plank_c.scale(0.6));
    }));
    l.add(PISTON_BACK, "piston_back", p(|q| {
        for y in 0..S {
            for x in 0..S {
                q.put(x, y, Rgb(0.46, 0.46, 0.46).add((rnd(x / 2, y / 2, 243) - 0.5) * 0.14), 1.0);
            }
        }
        for y in 4..12u32 {
            for x in 4..12u32 {
                q.put(x, y, Rgb(0.62, 0.64, 0.68).add(if x == 4 || y == 4 { 0.1 } else { 0.0 }), 1.0);
            }
        }
    }));
    l.add(PISTON_INNER, "piston_inner", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let edge = x < 2 || y < 2 || x > 13 || y > 13;
                let c = if edge { Rgb(0.48, 0.48, 0.48).add((rnd(x, y, 244) - 0.5) * 0.1) } else { Rgb(0.20, 0.18, 0.16) };
                q.put(x, y, c, 1.0);
            }
        }
        for y in 6..10u32 {
            for x in 6..10u32 {
                q.put(x, y, Rgb(0.70, 0.62, 0.40), 1.0);
            }
        }
    }));
    l.add(STICKY_FRONT, "sticky_front", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let slime = x > 1 && x < 14 && y > 1 && y < 14;
                let c = if slime { Rgb(0.42, 0.78, 0.30).add((rnd(x / 2, y / 2, 245) - 0.5) * 0.16) } else { plank_c };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    for d in 0..4u8 {
        l.add(REPEATER + d as u16, &format!("repeater_{d}"), gate_tile(d, false, false));
    }
    for d in 0..4u8 {
        l.add(REPEATER_ON + d as u16, &format!("repeater_on_{d}"), gate_tile(d, true, false));
    }
    for d in 0..4u8 {
        l.add(COMPARATOR + d as u16, &format!("comparator_{d}"), gate_tile(d, false, true));
    }
    for d in 0..4u8 {
        l.add(COMPARATOR_ON + d as u16, &format!("comparator_on_{d}"), gate_tile(d, true, true));
    }
    // Studio baseplate: light grey plate with a darker grid line on two edges (tiles into a
    // one-block grid).
    l.add(BASEPLATE, "baseplate", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let line = x == 0 || y == 0;
                let g = if line { 0.50 } else { 0.70 + (rnd(x, y, 250) - 0.5) * 0.02 };
                q.put(x, y, Rgb(g, g, g * 1.02), 1.0);
            }
        }
    }));
    // ---- Furnace, forge, barrel (they took over the slots of the electric machines) ----
    let stone_px = |x: u32, y: u32, seed: u32| {
        let cx = (x + (hash(y / 4, 0, seed) % 3)) % 5;
        let cy = (y + (hash(x / 5, 1, seed) % 2)) % 4;
        let b = if cx == 0 || cy == 0 { 0.38 } else { 0.56 + (rnd(x / 5, y / 4, seed + 1) - 0.5) * 0.10 };
        Rgb(b, b, b + 0.01)
    };
    l.0[FURNACE_FRONT as usize].1 = p(|q| {
        // Stone front with a dark fire mouth glowing at the bottom and a vent above.
        for y in 0..S {
            for x in 0..S {
                let mouth = (4..12).contains(&x) && (8..14).contains(&y);
                let vent = (5..11).contains(&x) && (3..5).contains(&y);
                let c = if mouth {
                    let glow = ((y as f32 - 8.0) / 5.0).clamp(0.0, 1.0);
                    Rgb(0.10, 0.07, 0.06).lerp(Rgb(1.0, 0.55, 0.15), glow * glow * if rnd(x, y, 270) < 0.7 { 1.0 } else { 0.6 })
                } else if vent {
                    Rgb(0.16, 0.15, 0.15)
                } else {
                    stone_px(x, y, 271)
                };
                q.put(x, y, c, 1.0);
            }
        }
        bevel(q, 3, 7, 12, 14, Rgb(0.30, 0.30, 0.31), Rgb(0.70, 0.70, 0.72));
    });
    l.0[MACHINE_CASING as usize].1 = p(|q| {
        // Forge side: an iron rim on top of a stone-brick hearth with glowing coals.
        for y in 0..S {
            for x in 0..S {
                let c = if y < 3 {
                    Rgb(0.26, 0.27, 0.30).add(if y == 0 { 0.18 } else { 0.0 })
                } else if (6..11).contains(&y) && (3..13).contains(&x) {
                    let hot = rnd(x, y, 272);
                    if y == 6 { Rgb(0.12, 0.08, 0.06) } else { Rgb(0.95, 0.35, 0.08).lerp(Rgb(1.0, 0.85, 0.35), if hot < 0.3 { 1.0 } else { 0.0 }).scale(0.75 + 0.06 * y as f32 / 10.0) }
                } else {
                    let brick = (y - 3) % 4 == 0 || (x + if (y - 3) / 4 % 2 == 0 { 0 } else { 4 }) % 8 == 0;
                    if brick { Rgb(0.32, 0.30, 0.29) } else { Rgb(0.55, 0.40, 0.33).add((rnd(x / 4, y / 4, 273) - 0.5) * 0.08) }
                };
                q.put(x, y, c, 1.0);
            }
        }
    });
    l.0[GENERATOR_FRONT as usize].1 = p(|q| {
        // Forge top: the anvil's flat working face.
        for y in 0..S {
            for x in 0..S {
                let face = (2..14).contains(&y) && (1..15).contains(&x);
                let c = if face { Rgb(0.38, 0.39, 0.43).add((rnd(x, y, 274) - 0.5) * 0.05) } else { Rgb(0.22, 0.22, 0.25) };
                q.put(x, y, c, 1.0);
            }
        }
        bevel(q, 1, 2, 14, 13, Rgb(0.55, 0.56, 0.60), Rgb(0.20, 0.20, 0.23));
    });
    l.0[BATTERY_SIDE as usize].1 = p(|q| {
        // Barrel side: oak staves and two iron hoops.
        for y in 0..S {
            for x in 0..S {
                let hoop = y == 3 || y == 4 || y == 11 || y == 12;
                let stave = x % 4 == 0;
                let c = if hoop {
                    Rgb(0.30, 0.30, 0.33).add(if y == 3 || y == 11 { 0.08 } else { 0.0 })
                } else {
                    Rgb(0.58, 0.40, 0.22).add((rnd(x / 4, y, 275) - 0.5) * 0.06 - if stave { 0.10 } else { 0.0 })
                };
                q.put(x, y, c, 1.0);
            }
        }
    });
    l.0[BATTERY_TOP as usize].1 = p(|q| {
        // Barrel lid: round planks inside an iron ring.
        for y in 0..S {
            for x in 0..S {
                let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
                let c = if d > 7.0 {
                    Rgb(0.30, 0.30, 0.33)
                } else if d > 6.0 {
                    Rgb(0.44, 0.30, 0.17)
                } else {
                    Rgb(0.64, 0.46, 0.27).add((rnd(x, y / 4, 276) - 0.5) * 0.06 - if y % 4 == 0 { 0.08 } else { 0.0 })
                };
                q.put(x, y, c, 1.0);
            }
        }
    });
    // ---- Farming ----
    l.add(FARMLAND, "farmland", p(|q| {
        // Dark, moist tilled soil in furrows.
        for y in 0..S {
            for x in 0..S {
                let furrow = y % 4 == 0;
                let n = (rnd(x, y, 260) - 0.5) * 0.06 + (rnd(x / 3, y / 2, 261) - 0.5) * 0.05;
                let c = if furrow { Rgb(0.24, 0.15, 0.08) } else { Rgb(0.40, 0.26, 0.14).add(n - if y % 4 == 3 { 0.05 } else { 0.0 }) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    // Crops are crossed sprites: stems from the bottom edge, the crop near the top.
    let crop = |seed: u32, stalk: Rgb, ripe: bool, fruit: Option<Rgb>, ear: Option<Rgb>| {
        p(move |q| {
            q.clear();
            let height = if ripe { 13 } else { 6 };
            for s in 0..5u32 {
                let x = 2 + s * 3;
                let h = height - (rnd(s, 0, seed) * 3.0) as u32;
                for y in (S - h)..S {
                    let lean = if y < S - h / 2 && s % 2 == 0 { 1 } else { 0 };
                    let xx = (x + lean).min(S - 1);
                    q.put(xx, y, stalk.scale(0.85 + rnd(xx, y, seed + 1) * 0.3), 1.0);
                    if !ripe && y == S - h {
                        q.put((xx + 1).min(S - 1), y, stalk.add(0.08), 1.0);
                    }
                }
                if let Some(e) = ear {
                    for y in (S - h)..(S - h + 4).min(S) {
                        q.put(x, y, e.add(if y % 2 == 0 { 0.06 } else { -0.04 }), 1.0);
                        q.put((x + 1).min(S - 1), y, e.scale(0.85), 1.0);
                    }
                }
                if let Some(f) = fruit {
                    // Tops of carrots / potatoes peeking out of the soil.
                    for dx in 0..2u32 {
                        q.put((x + dx).min(S - 1), S - 1, f, 1.0);
                        q.put((x + dx).min(S - 1), S - 2, f.scale(0.9), 1.0);
                    }
                }
            }
        })
    };
    l.add(WHEAT_YOUNG, "wheat_young", crop(262, Rgb(0.36, 0.66, 0.22), false, None, None));
    l.add(WHEAT_RIPE, "wheat_ripe", crop(263, Rgb(0.74, 0.64, 0.30), true, None, Some(Rgb(0.90, 0.72, 0.28))));
    l.add(CARROT_YOUNG, "carrot_young", crop(264, Rgb(0.30, 0.66, 0.20), false, None, None));
    l.add(CARROT_RIPE, "carrot_ripe", crop(265, Rgb(0.28, 0.62, 0.18), true, Some(Rgb(0.98, 0.55, 0.12)), None));
    l.add(POTATO_YOUNG, "potato_young", crop(266, Rgb(0.34, 0.62, 0.24), false, None, None));
    l.add(POTATO_RIPE, "potato_ripe", crop(267, Rgb(0.30, 0.58, 0.22), true, Some(Rgb(0.78, 0.64, 0.38)), None));
    // The Чаровальня: a dark obsidian desk with a red glowing rune circle on top and red
    // runes running round the sides.
    l.add(ENCHANT_TOP, "enchant_top", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
                let d = (dx * dx + dy * dy).sqrt();
                let ring = (d - 5.5).abs() < 0.8 || (d - 2.5).abs() < 0.6;
                let spoke = d < 5.5 && d > 2.5 && ((dx.abs() < 0.6) || (dy.abs() < 0.6));
                let c = if ring || spoke { Rgb(1.0, 0.25, 0.22).add(-(d / 30.0)) } else { Rgb(0.16, 0.10, 0.20).add((rnd(x, y, 291) - 0.5) * 0.06) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(ENCHANT_SIDE, "enchant_side", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let edge = y < 2 || y > 13;
                let rune = (5..11).contains(&y) && x % 4 != 0 && rnd(x / 2, y, 292) < 0.45;
                let c = if edge { Rgb(0.30, 0.18, 0.12) } else if rune { Rgb(0.95, 0.22, 0.20) } else { Rgb(0.14, 0.09, 0.18).add((rnd(x, y, 293) - 0.5) * 0.05) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    // Pixel-art finish. Natural, noisy materials get clumps instead of per-pixel static, a
    // gentle large-scale variation and a few luminance bands (the hand-painted look); every
    // opaque pixel then gets a touch more contrast and a limited palette.
    const NATURAL: [&str; 27] = [
        "grass_top", "dirt", "stone", "sand", "snow", "granite", "limestone", "salt_block", "peat", "ash", "red_sand", "mud", "clay", "marble",
        "netherrack", "dry_grass_top", "swamp_grass_top", "podzol_top", "moss", "wool", "wool_red", "wool_blue", "wool_yellow", "terracotta", "packed_ice", "slate", "smooth_stone",
    ];
    l.add(BUILD_GLASS, "building_glass", p(|q| {
        for y in 0..S { for x in 0..S {
            let grain = if BUILD_GLASS == BUILD_FABRIC { if (x + y) % 3 == 0 { 0.025 } else { -0.015 } } else { 0.0 };
            q.put(x, y, Rgb(0.68, 0.84, 0.91).add(grain), 0.35);
        } }
    }));
    l.add(BUILD_GLASS_RED, "building_glass_red", p(|q| {
        for y in 0..S { for x in 0..S {
            let grain = if BUILD_GLASS_RED == BUILD_FABRIC { if (x + y) % 3 == 0 { 0.025 } else { -0.015 } } else { 0.0 };
            q.put(x, y, Rgb(0.88, 0.24, 0.3).add(grain), 0.4);
        } }
    }));
    l.add(BUILD_GLASS_BLUE, "building_glass_blue", p(|q| {
        for y in 0..S { for x in 0..S {
            let grain = if BUILD_GLASS_BLUE == BUILD_FABRIC { if (x + y) % 3 == 0 { 0.025 } else { -0.015 } } else { 0.0 };
            q.put(x, y, Rgb(0.18, 0.48, 0.9).add(grain), 0.4);
        } }
    }));
    l.add(BUILD_GLASS_GREEN, "building_glass_green", p(|q| {
        for y in 0..S { for x in 0..S {
            let grain = if BUILD_GLASS_GREEN == BUILD_FABRIC { if (x + y) % 3 == 0 { 0.025 } else { -0.015 } } else { 0.0 };
            q.put(x, y, Rgb(0.25, 0.75, 0.48).add(grain), 0.4);
        } }
    }));
    l.add(BUILD_FABRIC, "building_fabric", p(|q| {
        for y in 0..S { for x in 0..S {
            let grain = if BUILD_FABRIC == BUILD_FABRIC { if (x + y) % 3 == 0 { 0.025 } else { -0.015 } } else { 0.0 };
            q.put(x, y, Rgb(0.25, 0.53, 0.65).add(grain), 1.0);
        } }
    }));
    l.add(BUILD_MIRROR, "building_mirror", p(|q| {
        for y in 0..S { for x in 0..S {
            let grain = if BUILD_MIRROR == BUILD_FABRIC { if (x + y) % 3 == 0 { 0.025 } else { -0.015 } } else { 0.0 };
            q.put(x, y, Rgb(0.72, 0.82, 0.88).add(grain), 1.0);
        } }
    }));
    l.add(BUILD_CERAMIC, "building_ceramic", p(|q| {
        for y in 0..S { for x in 0..S {
            let grain = if BUILD_CERAMIC == BUILD_FABRIC { if (x + y) % 3 == 0 { 0.025 } else { -0.015 } } else { 0.0 };
            q.put(x, y, Rgb(0.87, 0.86, 0.79).add(grain), 1.0);
        } }
    }));
    l.add(BUILD_PICTURE_FOREST, "building_picture_forest", p(|q| {for y in 0..S {for x in 0..S {q.put(x,y,if y>11 {Rgb(0.23,0.43,0.19)}else if (x==4||x==11)&&y>5 {Rgb(0.37,0.25,0.15)}else if ((x as i32-4).abs()+(y as i32-5).abs()<5)||((x as i32-11).abs()+(y as i32-6).abs()<4) {Rgb(0.18,0.48,0.27)}else {Rgb(0.51,0.72,0.83)},1.0);}}}));
    l.add(BUILD_PICTURE_SKY, "building_picture_sky", p(|q| {for y in 0..S {for x in 0..S {q.put(x,y,if (x as i32-11).pow(2)+(y as i32-4).pow(2)<9 {Rgb(1.0,0.84,0.35)}else if y>10+x/4 {Rgb(0.32,0.32,0.58)}else {Rgb(0.30+y as f32*0.025,0.40,0.68)},1.0);}}}));
    l.add(BUILD_PICTURE_ISKRIT, "building_picture_iskrit", p(|q| {for y in 0..S {for x in 0..S {q.put(x,y,if (x+y)%5==0||x==8 {Rgb(0.95,0.61,0.17)}else {Rgb(0.19,0.14,0.33)},1.0);}}}));
    for (i, (name, img)) in l.0.iter_mut().enumerate() {
        if NATURAL.contains(&name.as_str()) {
            painterly(img, 900 + i as u32);
        }
        for y in 0..img.height {
            for x in 0..img.width {
                let mut px = img.get(x, y);
                if px[3] != 255 {
                    continue;
                }
                for c in px.iter_mut().take(3) {
                    let v = ((*c as f32 / 255.0 - 0.5) * 1.12 + 0.5).clamp(0.0, 1.0);
                    *c = ((v * 14.0).round() / 14.0 * 255.0) as u8;
                }
                img.set(x, y, px);
            }
        }
    }
    l.0
}

