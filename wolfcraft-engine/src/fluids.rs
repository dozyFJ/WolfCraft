//! Flowing water and lava: a source block spreads sideways (water 7 blocks,
//! lava 3), liquid falls straight down, flows thin out step by step and dry up when their
//! source is gone. Water meeting lava turns it into obsidian (source) or cobblestone (flow).
//!
//! Only cells near edits are simulated (event-driven), so the untouched oceans of the world
//! cost nothing. Water steps 4 times a second, lava every 1.5 s.

use crate::block::{Block, Fluid};
use crate::cubesphere::Cell;
use crate::interaction::WorldEdit;
use glam::DVec3;
use std::collections::HashSet;

/// Cells updated per step at most (keeps a burst of flooding from stalling a frame).
const MAX_PER_STEP: usize = 400;

#[derive(Default)]
pub struct Fluids {
    water: HashSet<Cell>,
    lava: HashSet<Cell>,
    water_t: f32,
    lava_t: f32,
}

/// Up (radial) is axis 2 of the cube-sphere grid; 0 and 1 are the sideways axes.
fn below(world: &impl WorldEdit, c: Cell) -> Option<Cell> {
    world.grid().neighbor(c, 2, -1)
}
fn above(world: &impl WorldEdit, c: Cell) -> Option<Cell> {
    world.grid().neighbor(c, 2, 1)
}
fn sides(world: &impl WorldEdit, c: Cell) -> Vec<Cell> {
    let g = world.grid();
    [(0, -1), (0, 1), (1, -1), (1, 1)].iter().filter_map(|&(a, s)| g.neighbor(c, a, s)).collect()
}
fn all_neighbors(world: &impl WorldEdit, c: Cell) -> Vec<Cell> {
    let g = world.grid();
    (0..3).flat_map(|a| [-1, 1].into_iter().filter_map(move |s| g.neighbor(c, a, s))).collect()
}

impl Fluids {
    pub fn new() -> Self {
        Self::default()
    }

    /// True while some liquid is still on the move.
    pub fn busy(&self) -> bool {
        !self.water.is_empty() || !self.lava.is_empty()
    }

    /// Wakes the liquid around a changed cell (a block placed or broken).
    pub fn changed(&mut self, world: &impl WorldEdit, c: Cell) {
        let mut cells = all_neighbors(world, c);
        cells.push(c);
        for n in cells {
            let b = world.cell_block(n);
            if b.is_water() || b.is_replaceable() {
                self.water.insert(n);
            }
            if b.is_lava() || b.is_replaceable() {
                self.lava.insert(n);
            }
        }
    }

    /// Advances the simulation. Returns the cells that changed (for the network and lighting).
    pub fn update(&mut self, world: &mut impl WorldEdit, dt: f32) -> Vec<Cell> {
        let mut changed = Vec::new();
        self.water_t += dt;
        if self.water_t >= 0.25 {
            self.water_t = 0.0;
            let cells = std::mem::take(&mut self.water);
            self.step(world, cells, false, &mut changed);
        }
        self.lava_t += dt;
        if self.lava_t >= 1.5 {
            self.lava_t = 0.0;
            let cells = std::mem::take(&mut self.lava);
            self.step(world, cells, true, &mut changed);
        }
        changed
    }

    fn step(&mut self, world: &mut impl WorldEdit, cells: HashSet<Cell>, lava: bool, changed: &mut Vec<Cell>) {
        let mut sets: Vec<(Cell, Block)> = Vec::new();
        let mut solid: Vec<(Cell, Block)> = Vec::new();
        for (n, c) in cells.into_iter().enumerate() {
            if n >= MAX_PER_STEP {
                // Too much at once: carry the rest over to the next step.
                if lava { self.lava.insert(c) } else { self.water.insert(c) };
                continue;
            }
            let cur = world.cell_block(c);
            // Water that is already in place still cools any lava it touches.
            if !lava && cur.is_water() {
                for n in all_neighbors(world, c) {
                    if let Some(f) = world.cell_block(n).fluid().filter(|f| f.lava) {
                        solid.push((n, if f.source { Block::Obsidian } else { Block::Cobblestone }));
                    }
                }
            }
            let mine = cur.fluid().filter(|f| f.lava == lava);
            if mine.is_some_and(|f| f.source) {
                // Sources never change, but they feed their neighbours.
                for n in sides(world, c).into_iter().chain(below(world, c)) {
                    if world.cell_block(n).is_replaceable() && !world.cell_block(n).fluid().is_some_and(|f| f.source) {
                        if lava { self.lava.insert(n) } else { self.water.insert(n) };
                    }
                }
                continue;
            }
            // Only flows of this liquid and empty (replaceable, not the other liquid) cells.
            if mine.is_none() && !(cur.is_replaceable() && !cur.is_fluid()) {
                continue;
            }
            let want = Self::wanted(world, c, lava);
            let new = want.map(|f| f.block()).unwrap_or(Block::Air);
            if new != cur && !(mine.is_none() && want.is_none()) {
                sets.push((c, new));
            }
        }
        for (c, b) in solid {
            if world.cell_block(c).is_lava() {
                world.set_block(c, b);
                changed.push(c);
            }
        }
        for (c, b) in sets {
            if world.cell_block(c) == b {
                continue;
            }
            let mut b = b;
            // Lava flowing into water becomes cobblestone.
            if lava && b != Block::Air && all_neighbors(world, c).iter().any(|n| world.cell_block(*n).is_water()) {
                b = Block::Cobblestone;
            }
            world.set_block(c, b);
            changed.push(c);
            // Water reaching lava: a lava source turns to obsidian, flowing lava to cobblestone.
            if !lava && b.is_water() {
                for n in all_neighbors(world, c) {
                    if let Some(f) = world.cell_block(n).fluid().filter(|f| f.lava) {
                        world.set_block(n, if f.source { Block::Obsidian } else { Block::Cobblestone });
                        changed.push(n);
                    }
                }
            }
            for n in all_neighbors(world, c).into_iter().chain([c]) {
                if lava { self.lava.insert(n) } else { self.water.insert(n) };
            }
        }
    }

    /// The liquid state `c` should have, from its neighbours.
    fn wanted(world: &impl WorldEdit, c: Cell, lava: bool) -> Option<Fluid> {
        let same = |b: Block| b.fluid().filter(|f| f.lava == lava);
        // Liquid above: a falling column.
        if above(world, c).and_then(|a| same(world.cell_block(a))).is_some() {
            return Some(Fluid { lava, level: Fluid::max_level(lava), source: false, falling: true });
        }
        let max = Fluid::max_level(lava);
        let mut best = 0u8;
        for n in sides(world, c) {
            let Some(f) = same(world.cell_block(n)) else { continue };
            // A flow that can still fall does not spread sideways.
            let can_fall = below(world, n).is_some_and(|b| {
                let bb = world.cell_block(b);
                bb.is_replaceable() && same(bb).is_none()
            });
            let level = if f.source {
                max
            } else if can_fall {
                0
            } else if f.falling {
                max
            } else {
                f.level.saturating_sub(1)
            };
            best = best.max(level);
        }
        (best > 0).then_some(Fluid { lava, level: best, source: false, falling: false })
    }

    /// Push of the current on a body standing in flowing water at `pos`: towards lower water
    /// levels (blocks per second²).
    pub fn current_at(world: &impl WorldEdit, pos: DVec3) -> DVec3 {
        let g = *world.grid();
        let c = g.world_to_cell(pos);
        let Some(f) = world.cell_block(c).fluid().filter(|f| !f.lava && !f.source) else { return DVec3::ZERO };
        let here = if f.falling { 8.0 } else { f.level as f64 };
        let centre = g.cell_center(c);
        let mut push = DVec3::ZERO;
        for n in sides(world, c) {
            let b = world.cell_block(n);
            let there = match b.fluid() {
                Some(o) if !o.lava => if o.source { 8.0 } else { o.level as f64 },
                _ if b.is_replaceable() => 0.0,
                _ => continue,
            };
            push += (g.cell_center(n) - centre).normalize_or_zero() * (here - there);
        }
        let up = crate::physics::up_at(pos);
        let flat = push - up * push.dot(up);
        flat.normalize_or_zero() * 6.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::Voxels;
    use crate::cubesphere::Grid;
    use std::collections::HashMap;

    struct W {
        g: Grid,
        m: HashMap<Cell, Block>,
    }
    impl crate::physics::Voxels for W {
        fn grid(&self) -> &Grid {
            &self.g
        }
        fn cell_block(&self, c: Cell) -> Block {
            self.m.get(&c).copied().unwrap_or(Block::Air)
        }
    }
    impl WorldEdit for W {
        fn set_block(&mut self, c: Cell, b: Block) -> bool {
            self.m.insert(c, b);
            true
        }
    }

    fn world() -> (W, Cell) {
        let mut w = W { g: Grid::new(1024.0), m: HashMap::new() };
        let o = Cell::new(4, 600, 600, 1000);
        // A flat stone floor 21x21 under k = 1000.
        for di in -10..=10 {
            for dj in -10..=10 {
                w.m.insert(Cell::new(4, 600 + di, 600 + dj, 999), Block::Stone);
            }
        }
        (w, o)
    }

    fn run(w: &mut W, f: &mut Fluids, secs: f32) {
        for _ in 0..(secs / 0.05) as usize {
            f.update(w, 0.05);
        }
    }

    #[test]
    fn water_spreads_seven_blocks_and_dries_up_without_its_source() {
        let (mut w, o) = world();
        let mut f = Fluids::new();
        w.set_block(o, Block::Water);
        f.changed(&w, o);
        run(&mut w, &mut f, 4.0);
        let at = |w: &W, d: i32| w.cell_block(Cell::new(4, 600 + d, 600, 1000));
        assert_eq!(at(&w, 1), Block::WaterFlow7);
        assert_eq!(at(&w, 7), Block::WaterFlow1, "seven blocks out");
        assert_eq!(at(&w, 8), Block::Air, "no further");
        // Remove the source: everything drains.
        w.set_block(o, Block::Air);
        f.changed(&w, o);
        run(&mut w, &mut f, 6.0);
        assert!((1..=8).all(|d| at(&w, d) == Block::Air), "dried up");
    }

    #[test]
    fn water_falls_down_and_meets_lava() {
        let (mut w, o) = world();
        let mut f = Fluids::new();
        let high = Cell::new(4, 600, 600, 1004);
        w.set_block(high, Block::Water);
        f.changed(&w, high);
        run(&mut w, &mut f, 3.0);
        assert_eq!(w.cell_block(Cell::new(4, 600, 600, 1002)), Block::WaterFall, "a falling column");
        assert!(w.cell_block(Cell::new(4, 602, 600, 1000)).is_water(), "spreads on the floor");
        // Lava source next to the spreading water turns to obsidian.
        let lava = Cell::new(4, 609, 600, 1000);
        w.set_block(lava, Block::Lava);
        let edge = Cell::new(4, 608, 600, 1000);
        w.set_block(edge, Block::WaterFlow1);
        f.changed(&w, edge);
        run(&mut w, &mut f, 1.0);
        assert_eq!(w.cell_block(lava), Block::Obsidian, "water touching a lava source turns it to obsidian");
    }
}
