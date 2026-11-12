//! Items lying in the world, as on Bedrock: broken blocks and slain creatures drop them,
//! the player can throw them out of the inventory. They fall towards the planet, bounce a
//! little, float up in water, burn in lava and merge into one stack when they touch.
//!
//! Own touches: a magnet pulls items to the player from a few blocks away, thrown items
//! cannot be picked up for a moment (so a throw does not land straight back in the bag),
//! and items left alone vanish after five minutes.

use crate::block::Block;
use crate::items::ItemStack;
use crate::physics::{Voxels, up_at};
use glam::DVec3;

/// Seconds a dropped item lives.
pub const LIFETIME: f32 = 300.0;
/// Items at most (the oldest go first).
const MAX_ITEMS: usize = 256;
/// Pull radius and pick-up radius around the player's body centre (blocks).
const MAGNET: f64 = 2.6;
const PICK: f64 = 1.05;

#[derive(Clone, Debug)]
pub struct Dropped {
    pub stack: ItemStack,
    pub pos: DVec3,
    pub vel: DVec3,
    pub age: f32,
    /// Seconds before the player can pick it up.
    pub delay: f32,
    /// Fixed per item: spin and bob phase.
    pub seed: f32,
    /// Lying still on the ground (drawn flat instead of spinning in the air).
    pub resting: bool,
}

#[derive(Default)]
pub struct Drops {
    pub list: Vec<Dropped>,
    next_seed: u32,
}

impl Drops {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an item at `pos` moving with `vel`; it can be picked up after `delay` seconds.
    pub fn spawn(&mut self, stack: ItemStack, pos: DVec3, vel: DVec3, delay: f32) {
        if stack.count == 0 {
            return;
        }
        self.next_seed = self.next_seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let seed = (self.next_seed >> 8) as f32 / (1u32 << 24) as f32;
        if self.list.len() >= MAX_ITEMS {
            self.list.remove(0);
        }
        self.list.push(Dropped { stack, pos, vel, age: 0.0, delay, seed, resting: false });
    }

    /// Pops an item out of a block: from the centre of the cell, a small hop in a random
    /// sideways direction.
    pub fn pop_from_block(&mut self, stack: ItemStack, centre: DVec3) {
        let up = up_at(centre);
        let s = self.list.len() as f64 * 2.399 + centre.x * 0.37 + centre.z * 0.61;
        let side = DVec3::new(s.cos(), s.sin(), (s * 1.7).cos());
        let side = (side - up * side.dot(up)).normalize_or_zero();
        self.spawn(stack, centre, up * 3.2 + side * 1.4, 0.4);
    }

    /// Moves every item, merges touching stacks and lets the player pick items up. `give`
    /// tries to put a stack into the inventory and returns what did not fit. Returns how
    /// many items were picked up.
    pub fn step(&mut self, world: &impl Voxels, player_centre: DVec3, can_pick: bool, dt: f64, give: impl FnMut(ItemStack) -> Option<ItemStack>) -> u32 {
        self.step_log(world, player_centre, can_pick, dt, give, |_, _| {})
    }

    /// `step`, also telling `picked` each stack (item, count) that went into the bag.
    pub fn step_log(&mut self, world: &impl Voxels, player_centre: DVec3, can_pick: bool, dt: f64, mut give: impl FnMut(ItemStack) -> Option<ItemStack>, mut log: impl FnMut(crate::items::ItemId, u32)) -> u32 {
        let dt = dt.min(0.1);
        let mut picked = 0u32;
        let g = world.grid();
        let solid = |p: DVec3| {
            let b = world.cell_block(g.world_to_cell(p));
            b.is_solid() && b.info().render != crate::block::RenderKind::Cross
        };
        self.list.retain_mut(|d| {
            d.age += dt as f32;
            d.delay -= dt as f32;
            if d.age > LIFETIME {
                return false;
            }
            let up = up_at(d.pos);
            let here = world.cell_block(g.world_to_cell(d.pos));
            if here.is_lava() || here == Block::Magma {
                return false; // burnt
            }
            if here.is_water() {
                // Floats up slowly, drifts.
                d.vel += up * (14.0 * dt);
                d.vel *= 1.0 - (3.0 * dt).min(0.9);
            } else {
                d.vel -= up * (22.0 * dt);
            }
            // Magnet towards the player.
            let to = player_centre - d.pos;
            let dist = to.length();
            if can_pick && d.delay <= 0.0 && dist < MAGNET {
                if dist < PICK {
                    let before = d.stack.count;
                    match give(d.stack) {
                        None => {
                            picked += before as u32;
                            log(d.stack.id, before as u32);
                            return false;
                        }
                        Some(rest) => {
                            picked += (before - rest.count) as u32;
                            if rest.count < before {
                                log(d.stack.id, (before - rest.count) as u32);
                            }
                            d.stack = rest;
                        }
                    }
                } else {
                    d.vel = d.vel * (1.0 - (6.0 * dt).min(0.9)) + to / dist * (40.0 * dt);
                }
            }
            if solid(d.pos) {
                // Buried (a block was placed on it): climb out.
                d.pos += up * (4.0 * dt);
                d.vel = DVec3::ZERO;
                return true;
            }
            // Vertical then horizontal move, each stopped by solid blocks.
            let v_up = d.vel.dot(up);
            let flat = d.vel - up * v_up;
            let foot = 0.12;
            let next_v = d.pos + up * (v_up * dt);
            if solid(next_v - up * foot) && v_up < 0.0 {
                // Landed: a small bounce, then rest on the ground.
                d.vel = flat * 0.6 + if v_up < -4.0 { up * (-v_up * 0.25) } else { DVec3::ZERO };
            } else if solid(next_v + up * 0.1) && v_up > 0.0 {
                d.vel = flat;
            } else {
                d.pos = next_v;
            }
            let next_h = d.pos + flat * dt;
            if solid(next_h) || solid(next_h - up * (foot * 0.5)) {
                d.vel -= flat;
            } else {
                d.pos = next_h;
            }
            d.resting = false;
            if solid(d.pos - up * (foot + 0.02)) {
                // Ground friction.
                let v_up = d.vel.dot(up);
                let flat = d.vel - up * v_up;
                d.vel = flat * (1.0 - (8.0 * dt).min(0.95)) + up * v_up;
                d.resting = d.vel.length_squared() < 0.25 && !here.is_water();
            }
            true
        });
        self.merge();
        picked
    }

    /// Touching items of the same kind become one stack (after a short settle).
    fn merge(&mut self) {
        let n = self.list.len();
        if n < 2 {
            return;
        }
        let mut gone = vec![false; n];
        for i in 0..n {
            if gone[i] || self.list[i].age < 0.5 {
                continue;
            }
            for j in (i + 1)..n {
                if gone[j] || self.list[j].age < 0.5 {
                    continue;
                }
                let (a, b) = (&self.list[i], &self.list[j]);
                if (a.pos - b.pos).length_squared() > 0.7 * 0.7 || !a.stack.can_stack_with(&b.stack) {
                    continue;
                }
                let max = a.stack.info().max_stack;
                if a.stack.count as u32 + b.stack.count as u32 > max as u32 {
                    continue;
                }
                let add = b.stack.count;
                let age = a.age.min(b.age);
                let a = &mut self.list[i];
                a.stack.count += add;
                a.age = age;
                gone[j] = true;
            }
        }
        let mut k = 0;
        self.list.retain(|_| {
            k += 1;
            !gone[k - 1]
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cubesphere::{Cell, Grid};
    use std::collections::HashMap;

    struct W {
        g: Grid,
        m: HashMap<Cell, Block>,
    }
    impl Voxels for W {
        fn grid(&self) -> &Grid {
            &self.g
        }
        fn cell_block(&self, c: Cell) -> Block {
            self.m.get(&c).copied().unwrap_or(Block::Air)
        }
    }

    fn world() -> (W, Cell) {
        let mut w = W { g: Grid::new(1024.0), m: HashMap::new() };
        for di in -6..=6 {
            for dj in -6..=6 {
                w.m.insert(Cell::new(4, 600 + di, 600 + dj, 999), Block::Stone);
            }
        }
        (w, Cell::new(4, 600, 600, 1000))
    }

    #[test]
    fn items_fall_land_merge_and_get_picked_up() {
        let (w, c) = world();
        let mut d = Drops::new();
        let top = w.g.cell_center(c) + up_at(w.g.cell_center(c)) * 1.5;
        d.spawn(ItemStack::of_block(Block::Dirt, 3), top, DVec3::ZERO, 0.0);
        d.spawn(ItemStack::of_block(Block::Dirt, 2), top, DVec3::ZERO, 0.0);
        let far = top + up_at(top) * 50.0;
        for _ in 0..180 {
            d.step(&w, far, true, 1.0 / 60.0, |s| Some(s));
        }
        assert_eq!(d.list.len(), 1, "the two stacks merged");
        assert_eq!(d.list[0].stack.count, 5);
        // Resting on the floor: inside the air cell above the stone, not in it.
        let lying = d.list[0].pos;
        assert_eq!(w.g.world_to_cell(lying), c, "lies on the floor");
        // The player walks up: the magnet pulls it in and it lands in the bag.
        let mut bag = 0;
        let player = lying + up_at(lying) * 0.9 + (w.g.cell_center(Cell::new(4, 601, 600, 1000)) - w.g.cell_center(c)) * 1.5;
        for _ in 0..120 {
            d.step(&w, player, true, 1.0 / 60.0, |s| {
                bag += s.count;
                None
            });
        }
        assert_eq!(bag, 5);
        assert!(d.list.is_empty());
    }

    #[test]
    fn a_thrown_item_waits_before_it_can_be_picked_up_and_lava_burns_it() {
        let (mut w, c) = world();
        let mut d = Drops::new();
        let p = w.g.cell_center(c);
        d.spawn(ItemStack::of_block(Block::Sand, 1), p, DVec3::ZERO, 1.0);
        let n = d.step(&w, p, true, 0.05, |_| None);
        assert_eq!(n, 0, "still in the throw delay");
        w.m.insert(c, Block::Lava);
        d.step(&w, p + up_at(p) * 40.0, true, 0.05, |s| Some(s));
        assert!(d.list.is_empty(), "burnt in lava");
    }
}
