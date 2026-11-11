//! Deterministic structures: villages, abandoned settlements, ruins, temples, fortresses,
//! watchtowers, monoliths (rare), mines and dungeons.
//!
//! Structures live in the index space `(i, j, k)` of one cube face, so they are perfectly
//! aligned with the local up and the terrain below. The grid of each face is divided into coarse
//! cells; each cell tries a few jittered candidate columns. A surface candidate is accepted only
//! on gentle dry land away from the face edges; its footprint is flattened and foundations are
//! filled, so buildings sit on a level terrace.

use crate::biome::Biome;
use crate::block::Block;
use crate::chunk::{CHUNK_SIZE, Chunk};
use crate::cubesphere::{ChunkKey, local_idx};
use crate::planet::{Planet, hash3, smoothstep, splitmix, to01};
use glam::IVec3;

const CELL: i32 = 96;
const SEED: u64 = 0x57_2C7;
const MARGIN: i32 = 60;
/// Structures keep this many columns away from the face edges.
const EDGE_KEEP: i32 = 50;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StructureKind {
    Village,
    Abandoned,
    Ruins,
    Temple,
    Fortress,
    Watchtower,
    Monolith,
    Mine,
    Dungeon,
}

impl StructureKind {
    pub fn label(self) -> &'static str {
        match self {
            StructureKind::Village => "Деревня",
            StructureKind::Abandoned => "Заброшенное поселение",
            StructureKind::Ruins => "Руины",
            StructureKind::Temple => "Храм",
            StructureKind::Fortress => "Крепость",
            StructureKind::Watchtower => "Сторожевая башня",
            StructureKind::Monolith => "Монолит",
            StructureKind::Mine => "Заброшенная шахта",
            StructureKind::Dungeon => "Подземелье",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Always overwrite.
    Set,
    /// Only replace air, water, fluids and plants (used for foundations).
    Fill,
}

/// A box of cells in face index space `(i, j, k)`.
struct BoxOp {
    min: IVec3,
    max: IVec3,
    block: Block,
    mode: Mode,
}

/// Maps structure-local `(u, w, v)` to face indices: `w` is always the radial `k` axis.
#[derive(Clone, Copy)]
struct Frame {
    o: IVec3,
    u: IVec3,
    v: IVec3,
}

impl Frame {
    fn map(&self, u: i32, w: i32, v: i32) -> IVec3 {
        self.o + self.u * u + self.v * v + IVec3::Z * w
    }
}

/// A placed structure (its operations are generated on demand).
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub kind: StructureKind,
    pub face: u8,
    /// Anchor cell `(i, j, k)`: the surface block the structure stands on (or the centre of an
    /// underground one).
    pub anchor: IVec3,
    pub seed: u64,
    pub biome: Biome,
}

struct Builder {
    f: Frame,
    ops: Vec<BoxOp>,
    seed: u64,
}

impl Builder {
    fn new(f: Frame, seed: u64) -> Self {
        Self { f, ops: Vec::new(), seed }
    }

    fn bx(&mut self, u0: i32, u1: i32, w0: i32, w1: i32, v0: i32, v1: i32, block: Block, mode: Mode) {
        let a = self.f.map(u0, w0, v0);
        let b = self.f.map(u1, w1, v1);
        self.ops.push(BoxOp { min: a.min(b), max: a.max(b), block, mode });
    }
    fn set(&mut self, u: i32, w: i32, v: i32, block: Block) {
        self.bx(u, u, w, w, v, v, block, Mode::Set);
    }
    fn rnd(&self, u: i32, w: i32, v: i32) -> f64 {
        to01(hash3(self.seed, u, w + 1000, v))
    }

    /// Flattens a rectangular footprint: clears everything above the floor, builds the terrace.
    fn terrace(&mut self, u0: i32, u1: i32, v0: i32, v1: i32, height: i32, top: Block, filler: Block) {
        self.bx(u0, u1, 1, height, v0, v1, Block::Air, Mode::Set);
        self.bx(u0, u1, 0, 0, v0, v1, top, Mode::Set);
        self.bx(u0, u1, -10, -1, v0, v1, filler, Mode::Fill);
    }

    /// Four walls of a rectangle between heights w0..=w1.
    fn walls(&mut self, u0: i32, u1: i32, v0: i32, v1: i32, w0: i32, w1: i32, block: Block) {
        self.bx(u0, u1, w0, w1, v0, v0, block, Mode::Set);
        self.bx(u0, u1, w0, w1, v1, v1, block, Mode::Set);
        self.bx(u0, u0, w0, w1, v0, v1, block, Mode::Set);
        self.bx(u1, u1, w0, w1, v0, v1, block, Mode::Set);
    }
}

#[derive(Clone, Copy)]
struct Materials {
    wall: Block,
    corner: Block,
    floor: Block,
    roof: Block,
    path: Block,
    top: Block,
    filler: Block,
}

fn materials(b: Biome, ruined: bool) -> Materials {
    let mut m = match b {
        Biome::Desert | Biome::Badlands => Materials { wall: Block::Sandstone, corner: Block::StoneBricks, floor: Block::Sandstone, roof: Block::Sandstone, path: Block::Sandstone, top: Block::Sand, filler: Block::Sand },
        Biome::Taiga | Biome::Tundra | Biome::SnowyPlains => Materials { wall: Block::OakPlanks, corner: Block::SpruceLog, floor: Block::OakPlanks, roof: Block::StoneBricks, path: Block::Gravel, top: Block::Grass, filler: Block::Dirt },
        Biome::Savanna => Materials { wall: Block::OakPlanks, corner: Block::AcaciaLog, floor: Block::Cobblestone, roof: Block::OakPlanks, path: Block::Gravel, top: Block::DryGrass, filler: Block::Dirt },
        _ => Materials { wall: Block::OakPlanks, corner: Block::OakLog, floor: Block::Cobblestone, roof: Block::OakPlanks, path: Block::Gravel, top: Block::Grass, filler: Block::Dirt },
    };
    if ruined {
        m.wall = Block::MossyCobblestone;
        m.floor = Block::MossyCobblestone;
        m.roof = Block::CrackedStoneBricks;
    }
    m
}

fn house(b: &mut Builder, m: &Materials, u0: i32, v0: i32, su: i32, sv: i32, door_side: i32, ruined: bool, kind: i32) {
    let (u1, v1) = (u0 + su - 1, v0 + sv - 1);
    b.bx(u0, u1, 0, 0, v0, v1, m.floor, Mode::Set);
    for h in 1..=3 {
        b.walls(u0, u1, v0, v1, h, h, m.wall);
    }
    // Corner posts.
    for (cu, cv) in [(u0, v0), (u0, v1), (u1, v0), (u1, v1)] {
        b.bx(cu, cu, 1, 3, cv, cv, m.corner, Mode::Set);
    }
    // Windows.
    let (mu, mv) = ((u0 + u1) / 2, (v0 + v1) / 2);
    for (wu, wv) in [(mu, v0), (mu, v1), (u0, mv), (u1, mv)] {
        b.set(wu, 2, wv, Block::Glass);
    }
    // Door (side 0:-v, 1:+v, 2:-u, 3:+u).
    let (du, dv) = match door_side {
        0 => (mu, v0),
        1 => (mu, v1),
        2 => (u0, mv),
        _ => (u1, mv),
    };
    b.bx(du, du, 1, 2, dv, dv, Block::Air, Mode::Set);
    if !ruined {
        b.bx(u0 - 1, u1 + 1, 4, 4, v0 - 1, v1 + 1, b_roof(m), Mode::Set);
        b.bx(u0, u1, 5, 5, v0, v1, b_roof(m), Mode::Set);
        if su > 4 && sv > 4 {
            b.bx(u0 + 1, u1 - 1, 6, 6, v0 + 1, v1 - 1, b_roof(m), Mode::Set);
        }
    }
    // Interior.
    b.set(u0 + 1, 1, v0 + 1, if kind % 2 == 0 { Block::CraftingTable } else { Block::Chest });
    b.set(u1 - 1, 3, v1 - 1, Block::Lamp);
    if ruined {
        // Knock random holes into walls and the floor.
        for h in 1..=3 {
            for u in u0..=u1 {
                for v in v0..=v1 {
                    let edge = u == u0 || u == u1 || v == v0 || v == v1;
                    if edge && b.rnd(u, h, v) < 0.28 + 0.1 * h as f64 {
                        b.set(u, h, v, Block::Air);
                    }
                }
            }
        }
        for _ in 0..4 {
            let uu = u0 + (b.rnd(u0, 90, v0) * su as f64) as i32;
            let vv = v0 + (b.rnd(v0, 91, u0 + uu) * sv as f64) as i32;
            b.set(uu, 0, vv, Block::CrackedStoneBricks);
        }
    }
}

fn b_roof(m: &Materials) -> Block {
    m.roof
}

fn build_village(b: &mut Builder, biome: Biome, ruined: bool) {
    let m = materials(biome, ruined);
    b.terrace(-26, 26, -26, 26, 10, m.top, m.filler);
    // Cross-shaped paths.
    b.bx(-24, 24, 0, 0, -1, 1, m.path, Mode::Set);
    b.bx(-1, 1, 0, 0, -24, 24, m.path, Mode::Set);
    // Well.
    b.bx(-2, 2, 0, 1, -2, 2, Block::Cobblestone, Mode::Set);
    b.bx(-1, 1, 1, 1, -1, 1, Block::Air, Mode::Set);
    b.bx(-1, 1, 0, 0, -1, 1, if ruined { Block::Gravel } else { Block::Water }, Mode::Set);
    for (cu, cv) in [(-2, -2), (-2, 2), (2, -2), (2, 2)] {
        b.bx(cu, cu, 2, 3, cv, cv, Block::OakLog, Mode::Set);
    }
    b.bx(-2, 2, 4, 4, -2, 2, Block::OakPlanks, Mode::Set);
    // Houses around the cross.
    let spots = [(-18, -17, 7, 6, 1), (-18, 9, 6, 7, 1), (10, -18, 7, 6, 2), (10, 10, 7, 7, 0), (-8, -22, 5, 5, 1), (4, 18, 6, 5, 0)];
    for (i, (u, v, su, sv, door)) in spots.iter().enumerate() {
        if b.rnd(i as i32, 5, 5) < 0.85 {
            house(b, &m, *u, *v, *su, *sv, *door, ruined, i as i32);
        }
    }
    // Street lamps.
    if !ruined {
        for (u, v) in [(-10, -3), (10, 3), (3, -10), (-3, 10)] {
            b.bx(u, u, 1, 3, v, v, Block::OakLog, Mode::Set);
            b.set(u, 4, v, Block::Lamp);
        }
    }
}

fn build_ruins(b: &mut Builder, biome: Biome) {
    let m = materials(biome, true);
    b.terrace(-9, 9, -9, 9, 9, m.top, m.filler);
    b.bx(-8, 8, 0, 0, -8, 8, Block::MossyStoneBricks, Mode::Set);
    for u in -8i32..=8 {
        for v in -8i32..=8 {
            let edge = u.abs() == 8 || v.abs() == 8;
            let h = (b.rnd(u, 3, v) * 5.0) as i32;
            if edge && h > 0 && b.rnd(u, 4, v) < 0.7 {
                b.bx(u, u, 1, h, v, v, if b.rnd(u, 9, v) < 0.5 { Block::MossyStoneBricks } else { Block::CrackedStoneBricks }, Mode::Set);
            }
        }
    }
    for (u, v) in [(-4, -4), (4, 4), (-4, 4), (4, -4)] {
        let h = 3 + (b.rnd(u, 1, v) * 4.0) as i32;
        b.bx(u, u, 1, h, v, v, Block::StoneBricks, Mode::Set);
    }
    b.set(0, 1, 0, Block::Chest);
}

fn build_temple(b: &mut Builder, biome: Biome) {
    let (block, accent) = match biome {
        Biome::Desert | Biome::Badlands => (Block::Sandstone, Block::StoneBricks),
        Biome::Jungle => (Block::MossyStoneBricks, Block::MossyCobblestone),
        _ => (Block::StoneBricks, Block::CrackedStoneBricks),
    };
    let top = if matches!(biome, Biome::Desert | Biome::Badlands) { Block::Sand } else { Block::Grass };
    b.terrace(-13, 13, -13, 13, 20, top, if top == Block::Sand { Block::Sand } else { Block::Dirt });
    // Stepped pyramid.
    for tier in 0..5 {
        let r = 11 - tier * 2;
        b.bx(-r, r, 1 + tier * 2, 2 + tier * 2, -r, r, if tier % 2 == 0 { block } else { accent }, Mode::Set);
    }
    b.bx(-11, 11, 0, 0, -11, 11, block, Mode::Set);
    // Hall inside the lowest tier, entrance on +u.
    b.bx(-5, 5, 1, 3, -5, 5, Block::Air, Mode::Set);
    b.bx(6, 11, 1, 2, -1, 1, Block::Air, Mode::Set);
    b.bx(-5, 5, 0, 0, -5, 5, accent, Mode::Set);
    b.set(0, 1, 0, Block::GoldBlock);
    b.set(-4, 1, -4, Block::Chest);
    b.set(-4, 1, 4, Block::Chest);
    for (u, v) in [(4, 4), (4, -4), (-4, -4), (-4, 4)] {
        b.set(u, 3, v, Block::Lamp);
    }
    // Entrance pillars.
    b.bx(12, 12, 1, 4, -2, -2, accent, Mode::Set);
    b.bx(12, 12, 1, 4, 2, 2, accent, Mode::Set);
}

fn build_fortress(b: &mut Builder) {
    b.terrace(-16, 16, -16, 16, 18, Block::Cobblestone, Block::Stone);
    b.bx(-14, 14, 0, 0, -14, 14, Block::StoneBricks, Mode::Set);
    // Curtain walls with battlements.
    for h in 1..=6 {
        b.walls(-14, 14, -14, 14, h, h, if h % 3 == 0 { Block::MossyStoneBricks } else { Block::StoneBricks });
    }
    for i in -14..=14 {
        if i % 2 == 0 {
            for (u, v) in [(i, -14), (i, 14), (-14, i), (14, i)] {
                b.set(u, 7, v, Block::StoneBricks);
            }
        }
    }
    // Corner towers.
    for (cu, cv) in [(-14, -14), (-14, 14), (14, -14), (14, 14)] {
        b.bx(cu - 2, cu + 2, 1, 12, cv - 2, cv + 2, Block::StoneBricks, Mode::Set);
        b.bx(cu - 1, cu + 1, 1, 11, cv - 1, cv + 1, Block::Air, Mode::Set);
        b.bx(cu - 2, cu + 2, 12, 12, cv - 2, cv + 2, Block::CrackedStoneBricks, Mode::Set);
        b.set(cu, 3, cv, Block::Lamp);
        for (du, dv) in [(-2, -2), (2, 2), (-2, 2), (2, -2), (0, -2), (0, 2), (-2, 0), (2, 0)] {
            b.set(cu + du, 13, cv + dv, Block::StoneBricks);
        }
    }
    // Gate on the -v side.
    b.bx(-1, 1, 1, 4, -14, -14, Block::Air, Mode::Set);
    // Keep.
    b.bx(-5, 5, 1, 9, -5, 5, Block::StoneBricks, Mode::Set);
    b.bx(-4, 4, 1, 8, -4, 4, Block::Air, Mode::Set);
    b.bx(-1, 1, 1, 2, -5, -5, Block::Air, Mode::Set);
    b.bx(-5, 5, 10, 10, -5, 5, Block::CrackedStoneBricks, Mode::Set);
    b.set(0, 1, 3, Block::Chest);
    b.set(-3, 1, 3, Block::Chest);
    b.set(0, 7, 0, Block::Lamp);
    b.set(3, 4, -3, Block::Lamp);
    b.set(-3, 4, -3, Block::Lamp);
}

fn build_tower(b: &mut Builder) {
    b.terrace(-5, 5, -5, 5, 20, Block::Cobblestone, Block::Stone);
    b.bx(-3, 3, 0, 0, -3, 3, Block::StoneBricks, Mode::Set);
    b.bx(-3, 3, 1, 14, -3, 3, Block::StoneBricks, Mode::Set);
    b.bx(-2, 2, 1, 14, -2, 2, Block::Air, Mode::Set);
    b.bx(-1, 1, 1, 2, -3, -3, Block::Air, Mode::Set);
    // Spiral stair around the inner wall, one block per step.
    let ring = [(-2, -2), (-1, -2), (0, -2), (1, -2), (2, -2), (2, -1), (2, 0), (2, 1), (2, 2), (1, 2), (0, 2), (-1, 2), (-2, 2), (-2, 1), (-2, 0), (-2, -1)];
    for h in 1..=13 {
        let (u, v) = ring[(h as usize - 1) % ring.len()];
        b.bx(u, u, 1, h, v, v, Block::OakPlanks, Mode::Set);
    }
    b.bx(-4, 4, 15, 15, -4, 4, Block::StoneBricks, Mode::Set);
    b.bx(-3, 3, 15, 15, -3, 3, Block::Air, Mode::Set);
    b.bx(-3, 3, 14, 14, -3, 3, Block::OakPlanks, Mode::Set);
    b.bx(-2, 2, 14, 14, -2, 2, Block::OakPlanks, Mode::Set);
    for (u, v) in [(-4, -4), (4, 4), (-4, 4), (4, -4)] {
        b.bx(u, u, 16, 16, v, v, Block::StoneBricks, Mode::Set);
    }
    b.set(0, 16, 0, Block::Lamp);
    b.set(0, 15, 2, Block::Chest);
}

fn build_monolith(b: &mut Builder) {
    b.terrace(-6, 6, -6, 6, 20, Block::StoneBricks, Block::Stone);
    b.bx(-5, 5, 0, 0, -5, 5, Block::StoneBricks, Mode::Set);
    b.bx(-4, 4, 0, 0, -4, 4, Block::MossyStoneBricks, Mode::Set);
    b.bx(-1, 1, 1, 1, -1, 1, Block::Obsidian, Mode::Set);
    b.bx(0, 0, 2, 12, 0, 0, Block::Obsidian, Mode::Set);
    b.set(0, 13, 0, Block::GoldBlock);
    for (u, v) in [(3, 3), (-3, -3), (3, -3), (-3, 3)] {
        b.bx(u, u, 1, 2, v, v, Block::Obsidian, Mode::Set);
        b.set(u, 3, v, Block::Lamp);
    }
}

fn build_mine(b: &mut Builder) {
    // Long tunnel along u with supports, side chambers and a chest at the end.
    b.bx(0, 52, 0, 0, -1, 1, Block::OakPlanks, Mode::Set);
    b.bx(0, 52, 1, 3, -1, 1, Block::Air, Mode::Set);
    for u in (3..52).step_by(6) {
        b.bx(u, u, 1, 3, -2, -2, Block::OakLog, Mode::Set);
        b.bx(u, u, 1, 3, 2, 2, Block::OakLog, Mode::Set);
        b.bx(u, u, 4, 4, -2, 2, Block::OakLog, Mode::Set);
        b.bx(u, u, 1, 3, -1, -1, Block::OakPlanks, Mode::Set); // cross-brace
        b.bx(u, u, 2, 3, -1, -1, Block::Air, Mode::Set);
        b.bx(u, u, 1, 3, 1, 1, Block::OakPlanks, Mode::Set);
        b.bx(u, u, 2, 3, 1, 1, Block::Air, Mode::Set);
        if (u / 6) % 3 == 0 {
            b.set(u, 3, 0, Block::Lamp);
        }
    }
    for (u, side) in [(12, 1), (30, -1), (42, 1)] {
        b.bx(u - 2, u + 2, 1, 3, if side > 0 { 2 } else { -6 }, if side > 0 { 6 } else { -2 }, Block::Air, Mode::Set);
        b.set(u, 1, if side > 0 { 5 } else { -5 }, Block::Chest);
    }
    b.bx(53, 56, 1, 3, -2, 2, Block::Air, Mode::Set);
    b.set(55, 1, 0, Block::Chest);
    b.bx(53, 56, 0, 0, -2, 2, Block::Cobblestone, Mode::Set);
}

fn build_dungeon(b: &mut Builder) {
    b.bx(-5, 5, 0, 6, -5, 5, Block::MossyCobblestone, Mode::Set);
    b.bx(-4, 4, 1, 5, -4, 4, Block::Air, Mode::Set);
    b.bx(-4, 4, 0, 0, -4, 4, Block::Cobblestone, Mode::Set);
    for i in -4..=4 {
        for j in -4..=4 {
            if b.rnd(i, 7, j) < 0.35 {
                b.set(i, 0, j, Block::MossyCobblestone);
            }
        }
    }
    for (u, v) in [(-3, -3), (3, 3), (-3, 3), (3, -3)] {
        b.bx(u, u, 1, 5, v, v, Block::StoneBricks, Mode::Set);
    }
    b.set(-4, 1, 0, Block::Chest);
    b.set(4, 1, 0, Block::Chest);
    b.set(0, 1, 0, Block::Obsidian);
    // Corridor out of the +u side.
    b.bx(5, 14, 1, 3, -1, 1, Block::Air, Mode::Set);
    b.bx(5, 14, 0, 0, -1, 1, Block::Cobblestone, Mode::Set);
}

/// Structure placements whose anchor lies in the coarse cells overlapping the index box
/// `[lo, hi]` of face `f`.
pub fn placements_near(planet: &Planet, f: u8, lo: IVec3, hi: IVec3) -> Vec<Placement> {
    let mut out = Vec::new();
    let n = planet.grid.n;
    let s0 = (lo.x.div_euclid(CELL).max(0), lo.y.div_euclid(CELL).max(0));
    let s1 = (hi.x.div_euclid(CELL).min((n - 1) / CELL), hi.y.div_euclid(CELL).min((n - 1) / CELL));
    for sj in s0.1..=s1.1 {
        for si in s0.0..=s1.0 {
            let h = hash3(planet.seed ^ SEED ^ ((f as u64 + 1) << 56), si, sj, 0);
            if let Some(p) = candidate(planet, f, (si, sj), h) {
                out.push(p);
            }
        }
    }
    out
}

fn candidate(planet: &Planet, f: u8, cell: (i32, i32), h: u64) -> Option<Placement> {
    let g = &planet.grid;
    let n = g.n;
    let underground = to01(splitmix(h ^ 0xAA)) < 0.30;
    for attempt in 0..4u64 {
        let hh = splitmix(h ^ (attempt + 1).wrapping_mul(0x9E37));
        let i = cell.0 * CELL + (to01(splitmix(hh ^ 1)) * CELL as f64) as i32;
        let j = cell.1 * CELL + (to01(splitmix(hh ^ 2)) * CELL as f64) as i32;
        if i < EDGE_KEEP || j < EDGE_KEEP || i >= n - EDGE_KEEP || j >= n - EDGE_KEEP {
            continue;
        }
        let col = planet.column(g.cell_dir(f, i, j));
        let k_top = planet.top_k(&col);
        let roll = to01(splitmix(hh ^ 4));
        if underground {
            let depth = to01(splitmix(hh ^ 6)) * 69.0 + 16.0;
            let k = k_top - depth as i32;
            if (k as f64) < planet.layers.deep_crust_top + 20.0 || roll > 0.6 {
                continue;
            }
            let kind = if to01(splitmix(hh ^ 5)) < 0.5 { StructureKind::Mine } else { StructureKind::Dungeon };
            return Some(Placement { kind, face: f, anchor: IVec3::new(i, j, k), seed: hh, biome: col.biome });
        }
        // Surface structure.
        if roll > 0.75 || col.elev < 2.0 || col.river {
            continue;
        }
        let Some(kind) = pick_kind(col.biome, col.elev, to01(splitmix(hh ^ 6))) else { continue };
        // Flatness and dryness check on a ring of sample columns.
        let reach = if matches!(kind, StructureKind::Fortress | StructureKind::Temple) { 18 } else { 28 };
        let mut ok = true;
        for (a, b) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
            let c2 = planet.column(g.cell_dir(f, i + a * reach, j + b * reach));
            if c2.elev < 1.0 || c2.river || (c2.elev - col.elev).abs() > 9.0 || smoothstep(0.0, 1.0, c2.volcanic as f64) > 0.5 {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }
        return Some(Placement { kind, face: f, anchor: IVec3::new(i, j, k_top), seed: hh, biome: col.biome });
    }
    None
}

fn pick_kind(biome: Biome, elev: f32, roll: f64) -> Option<StructureKind> {
    let flat_land = matches!(biome, Biome::Plains | Biome::Savanna | Biome::Taiga | Biome::Desert | Biome::Forest | Biome::BirchForest | Biome::Tundra | Biome::Badlands | Biome::Jungle);
    if !flat_land && biome != Biome::Mountains && biome != Biome::Swamp {
        return None;
    }
    if roll < 0.03 {
        return Some(StructureKind::Monolith);
    }
    if biome == Biome::Mountains || (elev > 10.0 && roll < 0.40) {
        return Some(StructureKind::Fortress);
    }
    if matches!(biome, Biome::Desert | Biome::Jungle | Biome::Badlands) && roll < 0.35 {
        return Some(StructureKind::Temple);
    }
    Some(match roll {
        r if r < 0.40 => StructureKind::Village,
        r if r < 0.55 => StructureKind::Abandoned,
        r if r < 0.75 => StructureKind::Ruins,
        _ => StructureKind::Watchtower,
    })
}

fn ops_for(p: &Placement) -> Vec<BoxOp> {
    // Orientation variety: swap the horizontal axes and mirror them, chosen by the seed.
    let su = if p.seed & 2 == 0 { 1 } else { -1 };
    let sv = if p.seed & 4 == 0 { 1 } else { -1 };
    let (u, v) = if p.seed & 8 == 0 { (IVec3::new(su, 0, 0), IVec3::new(0, sv, 0)) } else { (IVec3::new(0, su, 0), IVec3::new(sv, 0, 0)) };
    let f = Frame { o: p.anchor, u, v };
    let mut b = Builder::new(f, p.seed);
    match p.kind {
        StructureKind::Village => build_village(&mut b, p.biome, false),
        StructureKind::Abandoned => build_village(&mut b, p.biome, true),
        StructureKind::Ruins => build_ruins(&mut b, p.biome),
        StructureKind::Temple => build_temple(&mut b, p.biome),
        StructureKind::Fortress => build_fortress(&mut b),
        StructureKind::Watchtower => build_tower(&mut b),
        StructureKind::Monolith => build_monolith(&mut b),
        StructureKind::Mine => build_mine(&mut b),
        StructureKind::Dungeon => build_dungeon(&mut b),
    }
    b.ops
}

/// Bounding box `(min, max)` of a placement in index space (tests/tools).
pub fn placement_bounds(p: &Placement) -> (IVec3, IVec3) {
    let ops = ops_for(p);
    let mut lo = IVec3::splat(i32::MAX);
    let mut hi = IVec3::splat(i32::MIN);
    for o in &ops {
        lo = lo.min(o.min);
        hi = hi.max(o.max);
    }
    (lo, hi)
}

pub fn apply(planet: &Planet, key: ChunkKey, chunk: &mut Chunk) {
    let o = key.origin();
    let end = o + IVec3::splat(CHUNK_SIZE - 1);
    // Structures only exist near the surface band.
    let k_lo = planet.radius - 230.0;
    let k_hi = planet.max_surface + 130.0;
    if (end.z as f64) < k_lo || (o.z as f64) > k_hi {
        return;
    }
    for pl in placements_near(planet, key.f, o - IVec3::new(MARGIN, MARGIN, 0), end + IVec3::new(MARGIN, MARGIN, 0)) {
        for op in ops_for(&pl) {
            let lo = op.min.max(o);
            let hi = op.max.min(end);
            if lo.cmpgt(hi).any() {
                continue;
            }
            for k in lo.z..=hi.z {
                for j in lo.y..=hi.y {
                    for i in lo.x..=hi.x {
                        let idx = local_idx(i & 15, j & 15, k & 15);
                        match op.mode {
                            Mode::Set => chunk.set_index(idx, op.block),
                            Mode::Fill => {
                                let cur = chunk.get_index(idx);
                                if cur.is_replaceable() || cur.is_fluid() {
                                    chunk.set_index(idx, op.block);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    fn planet(seed: u64) -> Planet {
        let mut c = WorldConfig::new("t", seed);
        c.size = PlanetSize::Small;
        Planet::new(&c)
    }

    fn all_placements(p: &Planet) -> Vec<Placement> {
        let n = p.grid.n;
        (0..6u8).flat_map(|f| placements_near(p, f, IVec3::new(0, 0, 0), IVec3::new(n, n, 0))).collect()
    }

    #[test]
    fn placements_are_deterministic_and_varied() {
        let p = planet(21);
        let a = all_placements(&p);
        let b = all_placements(&p);
        assert_eq!(a.len(), b.len());
        assert!(!a.is_empty(), "a planet must have structures");
        let kinds: std::collections::HashSet<_> = a.iter().map(|x| x.kind).collect();
        assert!(kinds.len() >= 3, "expected several structure kinds, got {kinds:?}");
        let faces: std::collections::HashSet<_> = a.iter().map(|x| x.face).collect();
        assert!(faces.len() >= 4, "structures on only {} faces", faces.len());
    }

    #[test]
    fn surface_structures_stand_on_dry_gentle_land_away_from_edges() {
        for seed in [1u64, 2, 3] {
            let p = planet(seed);
            let n = p.grid.n;
            for pl in all_placements(&p) {
                assert!(pl.anchor.x >= EDGE_KEEP && pl.anchor.y >= EDGE_KEEP && pl.anchor.x < n - EDGE_KEEP && pl.anchor.y < n - EDGE_KEEP);
                let col = p.column(p.grid.cell_dir(pl.face, pl.anchor.x, pl.anchor.y));
                if matches!(pl.kind, StructureKind::Mine | StructureKind::Dungeon) {
                    assert!(pl.anchor.z < p.top_k(&col) - 10, "underground structure too shallow");
                    continue;
                }
                assert!(col.elev >= 2.0, "{:?} on water/beach: {}", pl.kind, col.elev);
                assert!(!col.river);
                assert_eq!(pl.anchor.z, p.top_k(&col));
            }
        }
    }

    #[test]
    fn structure_blocks_are_written_into_chunks() {
        let p = planet(21);
        let pl = all_placements(&p).into_iter().find(|x| !matches!(x.kind, StructureKind::Mine | StructureKind::Dungeon)).expect("surface structure");
        let key = ChunkKey::new(pl.face, pl.anchor.x >> 4, pl.anchor.y >> 4, (pl.anchor.z + 2) >> 4);
        let with = crate::worldgen::generate_chunk(&p, key);
        let mut cfg = WorldConfig::new("t", 21);
        cfg.size = PlanetSize::Small;
        cfg.structures = false;
        let p2 = Planet::new(&cfg);
        let without = crate::worldgen::generate_chunk(&p2, key);
        let differ = (0..4096).filter(|&i| with.get_index(i) != without.get_index(i)).count();
        assert!(differ > 20, "structure left no trace in its chunk ({differ})");
    }

    #[test]
    fn bounds_are_reasonable_and_floor_is_level() {
        let p = planet(21);
        for pl in all_placements(&p).into_iter().take(12) {
            let (lo, hi) = placement_bounds(&pl);
            let size = hi - lo;
            assert!(size.max_element() < 120, "{:?} too large: {size}", pl.kind);
            // Buildings never extend below their anchor by more than the foundation depth.
            assert!(lo.z >= pl.anchor.z - 70, "{:?}", pl.kind);
        }
    }
}
