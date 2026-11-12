//! Chunk generation: terrain from `Planet`, then flora and trees. Structures are applied last.
//!
//! Everything works in the face-local index space `(i, j, k)`, so a tree is a vertical column of
//! blocks and a flat meadow is really flat, on every part of the planet.

use crate::biome::Biome;
use crate::block::{Block, RenderKind};
use crate::chunk::{CHUNK_SIZE, CHUNK_VOL, Chunk};
use crate::cubesphere::{Cell, ChunkKey, local_idx};
use crate::planet::{Column, Planet, hash3, splitmix, to01};

const FLORA_SEED: u64 = 0xF10_4A;
const TREE_SEED: u64 = 0x7EE_5EED;
/// How far a tree can reach sideways from its trunk, in cells.
const TREE_REACH: i32 = 5;

/// Generates chunk `key`. Deterministic: depends only on the planet and the chunk key.
pub fn generate_chunk(planet: &Planet, key: ChunkKey) -> Chunk {
    let g = &planet.grid;
    let o = key.origin();
    let (i0, j0, k0) = (o.x, o.y, o.z);

    // Far above anything that can exist (terrain, trees, structures): all air.
    if k0 as f64 > planet.max_surface + 48.0 {
        return Chunk::empty();
    }

    // Studio baseplate: one layer of grid plate on bedrock at sea level, air everywhere else.
    if planet.world_type == crate::config::WorldType::Baseplate && !planet.hell {
        let top = planet.radius.round() as i32;
        if top - 1 < k0 || top - 1 >= k0 + CHUNK_SIZE {
            if !(k0..k0 + CHUNK_SIZE).contains(&top) {
                return Chunk::empty();
            }
        }
        let mut chunk = Chunk::empty();
        for lj in 0..CHUNK_SIZE {
            for li in 0..CHUNK_SIZE {
                for (k, b) in [(top, Block::Baseplate), (top - 1, Block::Bedrock)] {
                    if (k0..k0 + CHUNK_SIZE).contains(&k) {
                        chunk.set(li, lj, k - k0, b);
                    }
                }
            }
        }
        chunk.sky_open = true;
        return chunk;
    }
    let mut raw = Box::new([0u16; CHUNK_VOL]);
    let r_lo = k0 as f64;
    let r_hi = (k0 + CHUNK_SIZE) as f64;
    let band = r_hi >= planet.radius - 80.0 && r_lo <= planet.max_surface && r_hi >= planet.layers.deep_crust_top;
    for lj in 0..CHUNK_SIZE {
        for li in 0..CHUNK_SIZE {
            let (i, j) = (i0 + li, j0 + lj);
            let dir = g.cell_dir(key.f, i, j);
            let col = if band && !planet.hell { Some(planet.column(dir)) } else { None };
            for lk in 0..CHUNK_SIZE {
                let cell = Cell::new(key.f, i, j, k0 + lk);
                raw[local_idx(li, lj, lk)] = planet.cell_block(cell, dir, col.as_ref()) as u16;
            }
            if let Some(col) = &col {
                for (k, b) in flora(planet, key.f, i, j, col) {
                    if k >= k0 && k < k0 + CHUNK_SIZE {
                        let idx = local_idx(li, lj, k - k0);
                        let cur = Block::from_id(raw[idx]);
                        if matches!(cur, Block::Air | Block::Water) {
                            raw[idx] = b as u16;
                        }
                    }
                }
            }
        }
    }
    let mut chunk = Chunk::from_raw(raw);
    if !planet.hell {
        apply_trees(planet, key, &mut chunk);
    }
    if planet.structures {
        crate::structures::apply(planet, key, &mut chunk);
    }

    chunk.sky_open = sky_hint(planet, key);
    chunk.shrink_if_empty();
    chunk
}

/// Sky hint for a chunk: is its centre above the terrain? (Used where the chunk above is not
/// known; the mesher looks at the real blocks above when it can.)
pub fn sky_hint(planet: &Planet, key: ChunkKey) -> bool {
    let g = &planet.grid;
    let centre = g.chunk_center(key);
    let rc = g.radial(centre);
    if planet.hell {
        false
    } else if rc < planet.radius - 80.0 {
        false
    } else if rc > planet.max_surface {
        true
    } else {
        rc >= planet.surface_radius(planet.grid.col_dir(centre)) - 2.0
    }
}

fn is_grassy(b: Biome) -> bool {
    matches!(b, Biome::Plains | Biome::Forest | Biome::BirchForest | Biome::Jungle | Biome::Taiga | Biome::Savanna | Biome::Swamp | Biome::Tundra)
}

fn table(u: f64, t: &[(f64, Block)]) -> Option<Block> {
    t.iter().find(|(lim, _)| u < *lim).map(|(_, b)| *b)
}

/// Single-block vegetation of one column, as `(k, block)` pairs: grass, flowers, mushrooms,
/// shrubs, cacti (up to three high), seagrass and kelp.
pub fn flora(planet: &Planet, f: u8, i: i32, j: i32, col: &Column) -> Vec<(i32, Block)> {
    let s = planet.radius + col.elev as f64;
    let k_top = planet.top_k(col);
    let k_above = k_top + 1;
    let u = to01(hash3(planet.seed ^ FLORA_SEED ^ ((f as u64 + 1) << 56), i, j, 0));

    // Under water.
    if s < planet.radius - 0.5 {
        if col.elev > -30.0 && col.temp > 0.12 {
            if u < 0.05 {
                return vec![(k_above, Block::Seagrass)];
            }
            if u < 0.075 && col.elev < -6.0 && col.biome == Biome::Ocean {
                return vec![(k_above, Block::Kelp)];
            }
        }
        return vec![];
    }
    let b = col.biome;
    if matches!(b, Biome::Desert | Biome::Badlands) {
        let chance = if b == Biome::Desert { 0.012 } else { 0.003 };
        let h = splitmix(hash3(planet.seed ^ FLORA_SEED ^ 0xCAC, i, j, f as i32));
        if to01(h) < chance {
            let height = 1 + (splitmix(h) % 3) as i32;
            return (0..height).map(|d| (k_above + d, Block::Cactus)).collect();
        }
        return if u < 0.01 { vec![(k_above, Block::DeadBush)] } else { vec![] };
    }
    if !is_grassy(b) {
        return vec![];
    }
    // Sugar cane on low shores (1-3 high).
    if col.elev < 2.5 && !matches!(b, Biome::Tundra) {
        let h = splitmix(hash3(planet.seed ^ FLORA_SEED ^ 0x5C4, i, j, f as i32));
        if to01(h) < 0.05 {
            let height = 1 + (splitmix(h) % 3) as i32;
            return (0..height).map(|d| (k_above + d, Block::SugarCane)).collect();
        }
    }
    let pick = match b {
        Biome::Plains => table(u, &[(0.22, Block::TallGrass), (0.235, Block::FlowerRed), (0.25, Block::FlowerYellow), (0.256, Block::Shrub), (0.262, Block::FlowerBlue), (0.2635, Block::Pumpkin)]),
        Biome::Forest | Biome::BirchForest => table(u, &[(0.14, Block::TallGrass), (0.150, Block::FlowerRed), (0.158, Block::FlowerYellow), (0.166, Block::MushroomBrown), (0.172, Block::MushroomRed), (0.19, Block::Fern), (0.196, Block::FlowerBlue)]),
        Biome::Jungle => table(u, &[(0.30, Block::TallGrass), (0.37, Block::Shrub), (0.375, Block::FlowerRed), (0.42, Block::Fern), (0.424, Block::Melon)]),
        Biome::Taiga => table(u, &[(0.08, Block::TallGrass), (0.088, Block::MushroomBrown), (0.092, Block::MushroomRed), (0.16, Block::Fern)]),
        Biome::Savanna => table(u, &[(0.12, Block::TallGrass), (0.13, Block::Shrub)]),
        Biome::Swamp => table(u, &[(0.15, Block::TallGrass), (0.165, Block::MushroomBrown), (0.18, Block::MushroomRed), (0.19, Block::FlowerYellow)]),
        Biome::Tundra => table(u, &[(0.04, Block::TallGrass)]),
        _ => None,
    };
    pick.map(|bl| vec![(k_above, bl)]).unwrap_or_default()
}

// ------------------------------------------------------------------------------------------
// Trees

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreeKind {
    Oak,
    Birch,
    Spruce,
    Jungle,
    Acacia,
}

#[derive(Clone, Copy, Debug)]
pub struct Tree {
    /// The ground block the tree stands on.
    pub base: Cell,
    pub kind: TreeKind,
    pub height: i32,
    pub seed: u64,
}

fn tree_chance(b: Biome) -> (f64, &'static [(TreeKind, u32)]) {
    match b {
        Biome::Forest => (0.50, &[(TreeKind::Oak, 85), (TreeKind::Birch, 15)]),
        Biome::BirchForest => (0.45, &[(TreeKind::Birch, 90), (TreeKind::Oak, 10)]),
        Biome::Plains => (0.025, &[(TreeKind::Oak, 100)]),
        Biome::Jungle => (0.65, &[(TreeKind::Jungle, 80), (TreeKind::Oak, 20)]),
        Biome::Taiga => (0.45, &[(TreeKind::Spruce, 100)]),
        Biome::Tundra => (0.04, &[(TreeKind::Spruce, 100)]),
        Biome::SnowyPlains => (0.01, &[(TreeKind::Spruce, 100)]),
        Biome::Savanna => (0.07, &[(TreeKind::Acacia, 100)]),
        Biome::Swamp => (0.22, &[(TreeKind::Oak, 100)]),
        Biome::Mountains => (0.06, &[(TreeKind::Spruce, 100)]),
        _ => (0.0, &[]),
    }
}

fn column_hash(planet: &Planet, f: u8, i: i32, j: i32) -> u64 {
    hash3(planet.seed ^ TREE_SEED ^ ((f as u64 + 1) << 56), i, j, 0)
}

/// Is column `(i, j)` the smallest hash within two columns (spacing of at least three cells)?
fn is_spacing_minimum(planet: &Planet, f: u8, i: i32, j: i32, h: u64) -> bool {
    let n = planet.grid.n;
    for dj in -2..=2 {
        for di in -2..=2 {
            if (di, dj) == (0, 0) {
                continue;
            }
            let (ni, nj) = (i + di, j + dj);
            if ni < 0 || nj < 0 || ni >= n || nj >= n {
                continue;
            }
            let h2 = column_hash(planet, f, ni, nj);
            if h2 < h || (h2 == h && (di, dj) < (0, 0)) {
                return false;
            }
        }
    }
    true
}

/// Trees that may reach into `key`. Deterministic and independent of the chunk grid.
pub fn trees_near(planet: &Planet, key: ChunkKey) -> Vec<Tree> {
    let g = &planet.grid;
    let o = key.origin();
    let mut out = Vec::new();
    let m = TREE_REACH;
    for jj in (o.y - m)..(o.y + CHUNK_SIZE + m) {
        for ii in (o.x - m)..(o.x + CHUNK_SIZE + m) {
            let Some(a) = g.map_cell(key.f, ii, jj, 0) else { continue };
            let h = column_hash(planet, a.f, a.i, a.j);
            // Local minima of a uniform hash are small: cheap early-out.
            if to01(h) > 0.2 || !is_spacing_minimum(planet, a.f, a.i, a.j, h) {
                continue;
            }
            let col = planet.column(g.cell_dir(a.f, a.i, a.j));
            let (chance, kinds) = tree_chance(col.biome);
            if chance <= 0.0 || col.elev < 1.5 || col.river {
                continue;
            }
            if to01(splitmix(h ^ 9)) > chance / 0.66 {
                continue;
            }
            let k_top = planet.top_k(&col);
            let ground = planet.block_at(Cell::new(a.f, a.i, a.j, k_top));
            if !matches!(ground, Block::Grass | Block::DryGrass | Block::SwampGrass | Block::Snow | Block::Dirt | Block::Mud) {
                continue;
            }
            let mut pick = (splitmix(h ^ 5) % 100) as u32;
            let mut kind = kinds[0].0;
            for (k, w) in kinds {
                if pick < *w {
                    kind = *k;
                    break;
                }
                pick -= *w;
            }
            let r = splitmix(h ^ 7);
            let height = match kind {
                TreeKind::Oak => 4 + (r % 3) as i32,
                TreeKind::Birch => 5 + (r % 3) as i32,
                TreeKind::Spruce => 7 + (r % 5) as i32,
                TreeKind::Jungle => 10 + (r % 6) as i32,
                TreeKind::Acacia => 4 + (r % 3) as i32,
            };
            // Vertical overlap with the chunk?
            if k_top + height + 5 < o.z || k_top - 1 >= o.z + CHUNK_SIZE {
                continue;
            }
            out.push(Tree { base: Cell::new(a.f, a.i, a.j, k_top), kind, height, seed: h });
        }
    }
    out
}

fn leaves_of(kind: TreeKind) -> Block {
    match kind {
        TreeKind::Oak | TreeKind::Jungle => Block::OakLeaves,
        TreeKind::Birch => Block::BirchLeaves,
        TreeKind::Spruce => Block::SpruceLeaves,
        TreeKind::Acacia => Block::AcaciaLeaves,
    }
}

fn log_of(kind: TreeKind) -> Block {
    match kind {
        TreeKind::Oak | TreeKind::Jungle => Block::OakLog,
        TreeKind::Birch => Block::BirchLog,
        TreeKind::Spruce => Block::SpruceLog,
        TreeKind::Acacia => Block::AcaciaLog,
    }
}

/// Blocks of one tree as offsets `(di, dj, dk)` from the ground block. Leaves first, logs last
/// (logs win on overlap). The trunk is a straight column along `+k`.
pub fn tree_blocks(t: &Tree) -> Vec<((i32, i32, i32), Block)> {
    let h = t.height;
    let leaf = leaves_of(t.kind);
    let mut out = Vec::new();

    let ellipsoid = |out: &mut Vec<((i32, i32, i32), Block)>, centre_k: f64, rad: f64, ry: f64| {
        let ext = rad.max(ry).ceil() as i32 + 1;
        let ck = centre_k.round() as i32;
        for dk in (ck - ext)..=(ck + ext) {
            for dj in -ext..=ext {
                for di in -ext..=ext {
                    let (x, y, z) = (di as f64, dj as f64, dk as f64 - centre_k);
                    let f = (x * x + y * y) / (rad * rad) + z * z / (ry * ry);
                    if f <= 1.0 {
                        if f > 0.72 && to01(hash3(t.seed, di, dj, dk)) < 0.28 {
                            continue;
                        }
                        out.push(((di, dj, dk), leaf));
                    }
                }
            }
        }
    };

    match t.kind {
        TreeKind::Oak => ellipsoid(&mut out, h as f64 + 0.2, 2.7, 2.4),
        TreeKind::Birch => ellipsoid(&mut out, h as f64 + 0.2, 2.3, 2.4),
        TreeKind::Jungle => {
            ellipsoid(&mut out, h as f64 + 0.5, 4.0, 3.0);
            ellipsoid(&mut out, h as f64 * 0.6, 2.4, 2.0);
        }
        TreeKind::Acacia => ellipsoid(&mut out, h as f64 + 0.8, 3.6, 1.3),
        TreeKind::Spruce => {
            for dk in 2..=(h + 1) {
                let mut radius = ((h + 2) as f64 - dk as f64) * 0.38;
                if dk % 2 != 0 {
                    radius *= 0.62;
                }
                let ext = radius.ceil() as i32;
                for dj in -ext..=ext {
                    for di in -ext..=ext {
                        if ((di * di + dj * dj) as f64).sqrt() <= radius {
                            out.push(((di, dj, dk), leaf));
                        }
                    }
                }
            }
        }
    }
    let log = log_of(t.kind);
    for dk in 1..=h {
        out.push(((0, 0, dk), log));
    }
    if t.kind == TreeKind::Jungle {
        for dk in 1..=(h / 2) {
            for (di, dj) in [(1, 0), (0, 1), (1, 1)] {
                out.push(((di, dj, dk), log));
            }
        }
    }
    out
}

fn apply_trees(planet: &Planet, key: ChunkKey, chunk: &mut Chunk) {
    let g = &planet.grid;
    for t in trees_near(planet, key) {
        for ((di, dj, dk), b) in tree_blocks(&t) {
            let Some(c) = g.map_cell(t.base.f, t.base.i + di, t.base.j + dj, t.base.k + dk) else { continue };
            if ChunkKey::of(c) != key {
                continue;
            }
            let (li, lj, lk) = (c.i & 15, c.j & 15, c.k & 15);
            let cur = chunk.get(li, lj, lk);
            let is_log = matches!(b, Block::OakLog | Block::BirchLog | Block::SpruceLog | Block::AcaciaLog);
            let can = if is_log { cur.is_replaceable() || cur.info().render == RenderKind::Cutout } else { cur == Block::Air || cur.info().render == RenderKind::Cross };
            if can {
                chunk.set(li, lj, lk, b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::planet::Rng;

    fn planet(seed: u64) -> Planet {
        let mut c = WorldConfig::new("t", seed);
        c.size = PlanetSize::Small;
        c.structures = false;
        Planet::new(&c)
    }

    #[test]
    fn chunk_generation_is_deterministic() {
        let p = planet(11);
        let spawn = p.find_spawn();
        let key = ChunkKey::of(p.grid.world_to_cell(spawn));
        let a = generate_chunk(&p, key);
        let b = generate_chunk(&p, key);
        assert_eq!(a.raw().map(|r| r.to_vec()), b.raw().map(|r| r.to_vec()));
        assert!(!a.is_empty());
    }

    #[test]
    fn chunks_far_above_are_empty_and_deep_are_solid() {
        let p = planet(12);
        assert!(generate_chunk(&p, ChunkKey::new(4, 10, 10, 200)).is_empty());
        let core = generate_chunk(&p, ChunkKey::new(2, 5, 5, 0));
        assert!(core.get(0, 0, 0) != Block::Air);
    }

    #[test]
    fn surface_chunk_has_flat_aligned_ground() {
        // On a flat world the ground of a chunk is a perfectly flat layer in (i, j, k): the top
        // block of every column is at the same k, whatever the face.
        let mut cfg = WorldConfig::new("flat", 4);
        cfg.size = PlanetSize::Small;
        cfg.world_type = WorldType::Flat;
        cfg.structures = false;
        let p = Planet::new(&cfg);
        for f in 0..6u8 {
            let key = ChunkKey::new(f, 20, 20, (p.radius as i32 + 4) >> 4);
            let c = generate_chunk(&p, key);
            let mut tops = std::collections::HashSet::new();
            for lj in 0..16 {
                for li in 0..16 {
                    let t = (0..16).rev().find(|lk| c.get(li, lj, *lk).is_solid() && !matches!(c.get(li, lj, *lk), Block::OakLeaves | Block::OakLog | Block::BirchLeaves | Block::BirchLog | Block::SpruceLeaves | Block::SpruceLog | Block::AcaciaLeaves | Block::AcaciaLog | Block::Cactus));
                    if let Some(t) = t {
                        tops.insert(t);
                    }
                }
            }
            assert!(tops.len() <= 3, "face {f}: ground not flat in the local grid: {tops:?}");
        }
    }

    #[test]
    fn generation_matches_cell_queries_on_every_face() {
        // Terrain-only blocks must equal Planet::block_at (flora/trees excepted).
        let p = planet(13);
        for f in 0..6u8 {
            let k = ((p.radius as i32) - 6) >> 4;
            let key = ChunkKey::new(f, 25, 30, k);
            let ch = generate_chunk(&p, key);
            let o = key.origin();
            let mut compared = 0;
            for lk in (0..16).step_by(3) {
                for lj in (0..16).step_by(3) {
                    for li in (0..16).step_by(3) {
                        let b = p.block_at(Cell::new(f, o.x + li, o.y + lj, o.z + lk));
                        let got = ch.get(li, lj, lk);
                        if matches!(b, Block::Stone | Block::Water | Block::Dirt | Block::Sand | Block::Grass) && matches!(got, Block::Stone | Block::Water | Block::Dirt | Block::Sand | Block::Grass) {
                            assert_eq!(b, got);
                            compared += 1;
                        }
                    }
                }
            }
            assert!(compared > 10, "face {f}: nothing compared");
        }
    }

    #[test]
    fn trees_are_vertical_columns_of_logs_on_all_faces() {
        let mut found_faces = std::collections::HashSet::new();
        for seed in 1..8u64 {
            let p = planet(seed);
            let n = p.grid.n;
            for f in 0..6u8 {
                for s in 0..30 {
                    let mut rng = Rng(seed * 100 + s + f as u64 * 977);
                    let (i, j) = (20 + (rng.next_u64() % (n as u64 - 40)) as i32, 20 + (rng.next_u64() % (n as u64 - 40)) as i32);
                    let col = p.column(p.grid.cell_dir(f, i, j));
                    if !matches!(col.biome, Biome::Forest | Biome::Jungle | Biome::Taiga | Biome::BirchForest) || col.elev < 3.0 {
                        continue;
                    }
                    let k = p.top_k(&col);
                    let key = ChunkKey::of(Cell::new(f, i, j, k));
                    for t in trees_near(&p, key).iter().take(4) {
                        let blocks = tree_blocks(t);
                        let logs: Vec<_> = blocks.iter().filter(|(_, b)| matches!(b, Block::OakLog | Block::BirchLog | Block::SpruceLog | Block::AcaciaLog)).collect();
                        assert!(logs.iter().any(|((di, dj, dk), _)| (*di, *dj) == (0, 0) && *dk == t.height), "trunk must reach its full height straight up");
                        for dk in 1..=t.height {
                            assert!(logs.iter().any(|((di, dj, d), _)| (*di, *dj, *d) == (0, 0, dk)), "gap in trunk at {dk}");
                        }
                        found_faces.insert(f);
                    }
                }
            }
        }
        assert!(found_faces.len() >= 3, "trees found on only {} faces", found_faces.len());
    }

    #[test]
    fn trees_across_a_face_edge_are_completed_on_the_other_face() {
        // A tree near the seam must put its leaves into chunks of the neighbouring face too.
        let p = planet(21);
        let g = p.grid;
        let mut crossed = 0;
        'seeds: for seed in 1..40u64 {
            let p = planet(seed);
            for f in 0..6u8 {
                for jj in (0..g.n).step_by(2) {
                    let col = p.column(g.cell_dir(f, 1, jj));
                    if !matches!(col.biome, Biome::Forest | Biome::Jungle | Biome::BirchForest) {
                        continue;
                    }
                    let key = ChunkKey::of(Cell::new(f, 1, jj, p.top_k(&col)));
                    for t in trees_near(&p, key) {
                        if t.base.f == f && t.base.i <= 3 {
                            let mut faces = std::collections::HashSet::new();
                            for ((di, dj, dk), _) in tree_blocks(&t) {
                                if let Some(c) = g.map_cell(t.base.f, t.base.i + di, t.base.j + dj, t.base.k + dk) {
                                    faces.insert(c.f);
                                }
                            }
                            if faces.len() > 1 {
                                crossed += 1;
                                if crossed >= 1 {
                                    break 'seeds;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(crossed >= 1, "no tree straddling a seam found to test");
    }
}

