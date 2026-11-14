//! Destruction physics: structures that lose their support fall down, and blasts throw
//! blocks into the air.
//!
//! After blocks disappear (a blast, a pickaxe, a script), the solid blocks around the holes
//! are flood-filled. A connected group that reaches untouched terrain (or an unloaded chunk,
//! or bedrock) stands. A group that does not — the top of a tower whose base was blown up, a
//! bridge cut in the middle, a tree with its trunk cut — comes loose: its blocks turn into
//! falling blocks that drop under the planet's gravity and settle again where they land.
//! Fragile blocks (glass, leaves) and blocks that fall fast shatter instead of settling.

use crate::block::{Block, RenderKind};
use crate::cubesphere::{Cell, Grid};
use crate::physics::up_at;
use crate::planet::splitmix;
use glam::DVec3;
use std::collections::{HashSet, VecDeque};

/// A group bigger than this is treated as supported (huge structures, terrain).
pub const MAX_GROUP: usize = 700;
/// Live falling blocks at most (the rest of a collapse is removed with dust).
pub const MAX_FALLING: usize = 900;
const GRAVITY: f64 = 26.0;
/// Landing faster than this (blocks/s) shatters the block.
const SHATTER_SPEED: f64 = 24.0;
const MAX_AGE: f32 = 12.0;

/// A block in free fall (or thrown by a blast).
#[derive(Clone, Copy, Debug)]
pub struct FallingBlock {
    pub block: Block,
    pub pos: DVec3,
    pub vel: DVec3,
    pub spin: f32,
    pub age: f32,
    /// Thrown by a blast: always shatters when it lands (a crater does not rebuild itself).
    pub debris: bool,
}

/// What happened to a falling block this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Landing {
    /// Settled into this (empty) cell.
    Placed { cell: Cell, block: Block },
    /// Broke apart here (chips, a sound); `speed` is the impact speed.
    Shattered { pos: DVec3, block: Block, speed: f64 },
}

#[derive(Default)]
pub struct Collapse {
    pub falling: Vec<FallingBlock>,
    rng: u64,
}

/// Blocks that hold a structure together (and can fall).
pub fn connects(b: Block) -> bool {
    b != Block::Air && !b.is_fluid() && b.is_breakable() && (b.is_solid() || matches!(b.info().render, RenderKind::Cutout))
}

fn fragile(b: Block) -> bool {
    matches!(b.info().render, RenderKind::Cutout | RenderKind::Translucent) || b.info().hardness < 0.4
}

impl Collapse {
    pub fn new(seed: u64) -> Self {
        Self { falling: Vec::new(), rng: splitmix(seed ^ 0xC011A95E) }
    }

    fn rand(&mut self) -> f64 {
        self.rng = splitmix(self.rng);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Groups of blocks next to `holes` that no longer stand on anything.
    ///
    /// `get` reads a block (`None` = chunk not loaded: counts as support); `anchored` says
    /// whether a block is untouched terrain that holds everything on it.
    pub fn loose_groups(grid: &Grid, holes: &[Cell], get: impl Fn(Cell) -> Option<Block>, anchored: impl Fn(Cell, Block) -> bool) -> Vec<Vec<Cell>> {
        let mut seen: HashSet<Cell> = HashSet::new();
        let mut groups = Vec::new();
        let dirs = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
        for h in holes {
            for (di, dj, dk) in dirs {
                let Some(start) = grid.offset(*h, di, dj, dk) else { continue };
                if seen.contains(&start) {
                    continue;
                }
                match get(start) {
                    Some(b) if connects(b) => {}
                    _ => continue,
                }
                // Flood fill this group; stop as soon as it touches support.
                let mut group = vec![start];
                let mut queue = VecDeque::from([start]);
                let mut local: HashSet<Cell> = HashSet::from([start]);
                let mut supported = false;
                while let Some(c) = queue.pop_front() {
                    let b = match get(c) {
                        Some(b) => b,
                        None => {
                            supported = true;
                            break;
                        }
                    };
                    if c.k <= 1 || matches!(b, Block::Bedrock | Block::Baseplate) || anchored(c, b) {
                        supported = true;
                        break;
                    }
                    for (di, dj, dk) in dirs {
                        let Some(n) = grid.offset(c, di, dj, dk) else {
                            supported = true;
                            break;
                        };
                        if local.contains(&n) {
                            continue;
                        }
                        match get(n) {
                            None => {
                                supported = true;
                                break;
                            }
                            Some(nb) if connects(nb) => {
                                local.insert(n);
                                group.push(n);
                                queue.push_back(n);
                            }
                            _ => {}
                        }
                    }
                    if supported || group.len() > MAX_GROUP {
                        supported = true;
                        break;
                    }
                }
                seen.extend(local.iter().copied());
                if !supported {
                    groups.push(group);
                }
            }
        }
        groups
    }

    /// Turns a loose group into falling blocks. `set_air` removes each block from the world
    /// (and returns false if it could not). A small push away from `from` (a blast) and a
    /// little random spin make the collapse look alive.
    pub fn drop_group(&mut self, grid: &Grid, group: &[(Cell, Block)], mut set_air: impl FnMut(Cell) -> bool, from: Option<DVec3>) -> usize {
        let mut n = 0;
        for &(c, b) in group {
            if !set_air(c) {
                continue;
            }
            n += 1;
            if self.falling.len() >= MAX_FALLING {
                continue;
            }
            let pos = grid.cell_center(c);
            let push = from.map(|f| (pos - f).normalize_or_zero() * 1.5).unwrap_or(DVec3::ZERO);
            let jitter = DVec3::new(self.rand() - 0.5, self.rand() - 0.5, self.rand() - 0.5) * 0.6;
            let spin = (self.rand() as f32 - 0.5) * 2.0;
            self.falling.push(FallingBlock { block: b, pos, vel: push + jitter, spin, age: 0.0, debris: false });
        }
        n
    }

    /// Throws a block out of a blast (it shatters when it comes down).
    pub fn throw(&mut self, block: Block, pos: DVec3, centre: DVec3, power: f32) {
        if self.falling.len() >= MAX_FALLING || !connects(block) {
            return;
        }
        let up = up_at(pos);
        let out = (pos - centre).normalize_or_zero();
        let speed = 5.0 + power as f64 * 2.5 * (0.5 + self.rand());
        let jitter = DVec3::new(self.rand() - 0.5, self.rand() - 0.5, self.rand() - 0.5) * 3.0;
        let vel = out * speed + up * (4.0 + 6.0 * self.rand()) + jitter;
        let spin = (self.rand() as f32 - 0.5) * 12.0;
        self.falling.push(FallingBlock { block, pos, vel, spin, age: 0.0, debris: true });
    }

    /// Moves the falling blocks. `solid` says whether a point is inside a solid block;
    /// `free` whether a cell can take a landing block. Returns what landed this frame.
    pub fn update(&mut self, grid: &Grid, dt: f32, solid: impl Fn(DVec3) -> bool, free: impl Fn(Cell) -> bool) -> Vec<Landing> {
        let dtd = dt as f64;
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.falling.len() {
            let mut f = self.falling[i];
            f.age += dt;
            let up = up_at(f.pos);
            f.vel -= up * GRAVITY * dtd;
            // A little air drag sideways so thrown blocks do not fly forever.
            let side = f.vel - up * f.vel.dot(up);
            f.vel -= side * (0.6 * dtd).min(1.0);
            let d = f.vel * dtd;
            let steps = (d.length() / 0.3).ceil().max(1.0) as i32;
            let mut landed = None;
            for s in 1..=steps {
                let p = f.pos + d * (s as f64 / steps as f64);
                // The bottom of the block touches something solid.
                let bottom = p - up * 0.45;
                if solid(bottom) {
                    landed = Some(f.pos + d * ((s - 1) as f64 / steps as f64));
                    break;
                }
                if solid(p + (d / d.length().max(1e-9)) * 0.45) {
                    // Hit a wall sideways: lose the sideways speed, keep falling.
                    f.vel = up * f.vel.dot(up).min(0.0);
                    landed = None;
                    break;
                }
            }
            if let Some(p) = landed {
                let speed = f.vel.length();
                let cell = grid.world_to_cell(p);
                if f.debris || fragile(f.block) || speed > SHATTER_SPEED || !free(cell) {
                    out.push(Landing::Shattered { pos: p, block: f.block, speed });
                } else {
                    out.push(Landing::Placed { cell, block: f.block });
                }
                self.falling.swap_remove(i);
                continue;
            }
            f.pos += f.vel * dtd;
            if f.age > MAX_AGE {
                out.push(Landing::Shattered { pos: f.pos, block: f.block, speed: 0.0 });
                self.falling.swap_remove(i);
                continue;
            }
            self.falling[i] = f;
            i += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A stone floor (anchored) with a pillar of planks; the pillar's base gets removed.
    fn scene() -> (Grid, HashMap<Cell, Block>) {
        let grid = Grid::with_cells(64);
        let mut w = HashMap::new();
        for i in 10..20 {
            for j in 10..20 {
                w.insert(Cell::new(4, i, j, 40), Block::Stone);
            }
        }
        for k in 41..46 {
            w.insert(Cell::new(4, 15, 15, k), Block::OakPlanks);
        }
        // A beam on top of the pillar.
        for i in 16..19 {
            w.insert(Cell::new(4, i, 15, 45), Block::OakPlanks);
        }
        (grid, w)
    }

    #[test]
    fn cut_pillar_falls_and_settles() {
        let (grid, mut w) = scene();
        let anchored = |_: Cell, b: Block| b == Block::Stone;
        // Still standing: nothing loose.
        let holes = [Cell::new(4, 14, 15, 41)];
        assert!(Collapse::loose_groups(&grid, &holes, |c| Some(*w.get(&c).unwrap_or(&Block::Air)), anchored).is_empty());
        // Remove the base: the rest of the pillar and the beam come loose.
        let base = Cell::new(4, 15, 15, 41);
        w.remove(&base);
        let groups = Collapse::loose_groups(&grid, &[base], |c| Some(*w.get(&c).unwrap_or(&Block::Air)), anchored);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 4 + 3);
        let mut col = Collapse::new(1);
        let group: Vec<(Cell, Block)> = groups[0].iter().map(|c| (*c, w[c])).collect();
        let n = col.drop_group(&grid, &group, |c| w.remove(&c).is_some(), None);
        assert_eq!(n, 7);
        assert_eq!(col.falling.len(), 7);
        // Let them fall onto the floor.
        let mut placed = 0;
        for _ in 0..240 {
            let land = col.update(&grid, 1.0 / 60.0, |p| w.get(&grid.world_to_cell(p)).is_some_and(|b| b.is_solid()), |c| !w.contains_key(&c));
            for l in land {
                if let Landing::Placed { cell, block } = l {
                    w.insert(cell, block);
                    placed += 1;
                }
            }
        }
        assert!(col.falling.is_empty());
        assert!(placed >= 5, "most planks settle on the floor ({placed})");
    }

    #[test]
    fn unloaded_chunks_count_as_support() {
        let (grid, mut w) = scene();
        let base = Cell::new(4, 15, 15, 41);
        w.remove(&base);
        // The beam's far end runs into an unloaded chunk.
        let groups = Collapse::loose_groups(&grid, &[base], |c| if c.i >= 18 { None } else { Some(*w.get(&c).unwrap_or(&Block::Air)) }, |_, b| b == Block::Stone);
        assert!(groups.is_empty());
    }
}
