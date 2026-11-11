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
    pub const CABLE: u16 = 84;
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
}

const S: u32 = 16;

fn p(f: impl FnOnce(&mut Painter)) -> Image {
    let mut q = Painter::new(S, S);
    f(&mut q);
    q.finish()
}

fn side_with_top(base: Rgb, top: Rgb, seed: u32) -> Image {
    p(|q| {
        q.noisy(Rgb(0.47, 0.33, 0.22), 0.14, 3);
        q.speckle(Rgb(0.38, 0.26, 0.17), 0.25, 4);
        let _ = base;
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
        for y in 0..S {
            for x in 0..S {
                // Gaps are a darker shade, not holes: holes let the bright sky shine through
                // as white dots.
                let n = (rnd(x, y, seed + 1) - 0.5) * 0.12;
                let c = if rnd(x, y, seed) < 0.15 { base.scale(0.72) } else { base.add(n) };
                q.put(x, y, c, 1.0);
            }
        }
    })
}

fn ore(c: Rgb, seed: u32) -> Image {
    p(|q| {
        q.noisy(Rgb(0.50, 0.50, 0.52), 0.12, 5);
        q.speckle(Rgb(0.42, 0.42, 0.44), 0.25, 6);
        q.speckle(c, 0.22, seed);
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
                let edge = x == 0 || y == 0 || x == 15 || y == 15;
                let ring = ((d * 1.3) as u32 % 2) as f32 * 0.06;
                let c = if edge { Rgb(0.40, 0.30, 0.18) } else { Rgb(0.68 - ring, 0.54 - ring, 0.33 - ring) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
    l.add(PLANKS, "planks", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let board = y / 4;
                let seam = y % 4 == 3 || (x + board * 5) % 16 == 0;
                let n = (rnd(x, y, 15) - 0.5) * 0.06 + (rnd(board, 0, 16) - 0.5) * 0.06;
                let c = if seam { Rgb(0.45, 0.33, 0.19) } else { Rgb(0.66 + n, 0.50 + n, 0.30 + n) };
                q.put(x, y, c, 1.0);
            }
        }
    }));
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
    l.add(GOLD_BLOCK, "gold_block", p(|q| {
        q.noisy(Rgb(0.95, 0.80, 0.25), 0.06, 110);
        for i in 0..S {
            q.put(i, 0, Rgb(1.0, 0.92, 0.5), 1.0);
            q.put(0, i, Rgb(1.0, 0.92, 0.5), 1.0);
            q.put(i, 15, Rgb(0.75, 0.6, 0.15), 1.0);
            q.put(15, i, Rgb(0.75, 0.6, 0.15), 1.0);
        }
    }));
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
    let metal = |base: Rgb, hi: Rgb, lo: Rgb, seed: u32| {
        p(|q| {
            q.noisy(base, 0.05, seed);
            for i in 0..S {
                q.put(i, 0, hi, 1.0);
                q.put(0, i, hi, 1.0);
                q.put(i, 15, lo, 1.0);
                q.put(15, i, lo, 1.0);
            }
        })
    };
    l.add(COPPER_BLOCK, "copper_block", metal(Rgb(0.80, 0.46, 0.30), Rgb(0.95, 0.65, 0.45), Rgb(0.55, 0.30, 0.20), 133));
    l.add(IRON_BLOCK, "iron_block", metal(Rgb(0.82, 0.83, 0.85), Rgb(0.95, 0.96, 0.98), Rgb(0.58, 0.59, 0.62), 134));
    l.add(DIAMOND_BLOCK, "diamond_block", metal(Rgb(0.42, 0.88, 0.86), Rgb(0.75, 1.0, 0.98), Rgb(0.25, 0.60, 0.62), 135));
    l.add(COAL_BLOCK, "coal_block", metal(Rgb(0.12, 0.12, 0.14), Rgb(0.22, 0.22, 0.25), Rgb(0.06, 0.06, 0.07), 136));
    l.add(BRONZE_BLOCK, "bronze_block", metal(Rgb(0.70, 0.50, 0.22), Rgb(0.88, 0.70, 0.38), Rgb(0.46, 0.32, 0.14), 137));
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
    let steel = Rgb(0.46, 0.50, 0.55);
    let plate = |seed: u32| {
        p(|q| {
            q.noisy(steel, 0.04, seed);
            for i in 0..S {
                q.put(i, 0, Rgb(0.30, 0.33, 0.38), 1.0);
                q.put(i, 15, Rgb(0.30, 0.33, 0.38), 1.0);
                q.put(0, i, Rgb(0.30, 0.33, 0.38), 1.0);
                q.put(15, i, Rgb(0.30, 0.33, 0.38), 1.0);
            }
            for (x, y) in [(2, 2), (13, 2), (2, 13), (13, 13)] {
                q.put(x, y, Rgb(0.72, 0.74, 0.78), 1.0);
            }
        })
    };
    l.add(MACHINE_CASING, "machine_casing", plate(144));
    l.add(GENERATOR_FRONT, "generator_front", {
        let mut img = plate(145);
        for y in 5..11u32 {
            for x in 4..12u32 {
                img.set(x, y, [255, (170 + (x * 7 + y * 5) % 50) as u8, 40, 255]);
            }
        }
        img
    });
    l.add(CABLE, "cable", p(|q| {
        q.noisy(Rgb(0.20, 0.20, 0.22), 0.04, 146);
        for y in 0..S {
            for x in 6..10u32 {
                q.put(x, y, Rgb(0.80, 0.46, 0.28).add(if x == 6 { 0.1 } else { 0.0 }), 1.0);
            }
        }
        for x in 0..S {
            for y in 6..10u32 {
                q.put(x, y, Rgb(0.80, 0.46, 0.28).add(if y == 6 { 0.1 } else { 0.0 }), 1.0);
            }
        }
    }));
    l.add(BATTERY_SIDE, "battery_side", p(|q| {
        q.noisy(Rgb(0.18, 0.20, 0.24), 0.04, 147);
        for y in 3..13u32 {
            let on = y > 6;
            for x in 5..11u32 {
                q.put(x, y, if on { Rgb(0.25, 0.85, 0.35) } else { Rgb(0.12, 0.30, 0.15) }, 1.0);
            }
        }
    }));
    l.add(BATTERY_TOP, "battery_top", p(|q| {
        q.noisy(steel, 0.04, 148);
        for y in 5..11u32 {
            for x in 5..11u32 {
                q.put(x, y, Rgb(0.80, 0.46, 0.28), 1.0);
            }
        }
    }));
    l.add(ELECTRIC_LAMP, "electric_lamp", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let frame = x < 2 || y < 2 || x > 13 || y > 13;
                let n = (rnd(x, y, 149) - 0.5) * 0.06;
                q.put(x, y, if frame { Rgb(0.35, 0.38, 0.43) } else { Rgb(0.85, 0.96, 1.0).add(n) }, 1.0);
            }
        }
    }));
    l.add(FURNACE_FRONT, "electric_furnace_front", {
        let mut img = plate(150);
        for y in 6..12u32 {
            for x in 4..12u32 {
                img.set(x, y, if y > 8 { [255, 120, 30, 255] } else { [30, 20, 20, 255] });
            }
        }
        img
    });
    l.add(ELECTRIC_LAMP_ON, "electric_lamp_on", p(|q| {
        for y in 0..S {
            for x in 0..S {
                let frame = x < 2 || y < 2 || x > 13 || y > 13;
                let n = (rnd(x, y, 151) - 0.5) * 0.06;
                q.put(x, y, if frame { Rgb(0.55, 0.58, 0.62) } else { Rgb(1.0, 0.98, 0.70).add(n) }, 1.0);
            }
        }
    }));
    let switch = |on: bool, seed: u32| {
        p(|q| {
            q.noisy(Rgb(0.30, 0.32, 0.36), 0.04, seed);
            for y in 3..13u32 {
                for x in 5..11u32 {
                    q.put(x, y, Rgb(0.14, 0.15, 0.18), 1.0);
                }
            }
            // Handle: up = on, down = off.
            let (y0, y1, c) = if on { (3u32, 8u32, Rgb(0.30, 0.95, 0.40)) } else { (8, 13, Rgb(0.85, 0.25, 0.22)) };
            for y in y0..y1 {
                for x in 7..9u32 {
                    q.put(x, y, c, 1.0);
                }
            }
        })
    };
    l.add(SWITCH_OFF, "switch_off", switch(false, 152));
    l.add(SWITCH_ON, "switch_on", switch(true, 153));
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
    // Pixel-art finish for every opaque pixel: a touch more contrast and a limited palette
    // (14 levels per channel) so textures look hand-painted instead of smooth noise.
    for (_, img) in l.0.iter_mut() {
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
