//! Explosives: lit dynamite, grenades and rockets in flight, and the blasts they make.
//!
//! Dynamite is a block; a lighter (or another blast) turns it into a "lit" charge that counts
//! down and explodes. The grenade launcher and the bazooka fire projectiles that explode on
//! impact. A blast removes the blocks in a sphere with a ragged edge, lights dynamite inside it
//! with a short fuse (chain reactions) and reports itself so the game can hurt creatures.

use crate::block::Block;
use crate::cubesphere::Cell;
use crate::interaction::WorldEdit;
use crate::items::{ItemId, ids};
use crate::mobs::Debris;
use crate::physics::{Voxels, up_at};
use crate::planet::splitmix;
use glam::DVec3;
use std::collections::HashSet;

/// Seconds between lighting dynamite and the blast.
pub const DYNAMITE_FUSE: f32 = 4.0;
pub const DYNAMITE_POWER: f32 = 4.0;
pub const GRENADE_POWER: f32 = 2.6;
pub const ROCKET_POWER: f32 = 4.5;
/// Blocks at least this hard survive any blast (obsidian, metal blocks are below it).
const BLAST_PROOF_HARDNESS: f32 = 30.0;
/// Grenades fall like stones; rockets fly almost straight.
const GRENADE_GRAVITY: f64 = 20.0;
const ROCKET_GRAVITY: f64 = 1.5;
/// Projectiles explode by themselves after this many seconds.
const GRENADE_LIFE: f32 = 4.0;
const ROCKET_LIFE: f32 = 6.0;
/// Hard cap of live projectiles and lit charges (protects against huge chain reactions).
const MAX_LIVE: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShotKind {
    Grenade,
    Rocket,
}

impl ShotKind {
    pub fn power(self) -> f32 {
        match self {
            ShotKind::Grenade => GRENADE_POWER,
            ShotKind::Rocket => ROCKET_POWER,
        }
    }
}

/// A projectile in flight.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub kind: ShotKind,
    pub pos: DVec3,
    pub vel: DVec3,
    pub age: f32,
}

/// A lit dynamite charge (the block is already gone from the world).
#[derive(Clone, Copy, Debug)]
pub struct Lit {
    pub pos: DVec3,
    pub fuse: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blast {
    pub pos: DVec3,
    pub power: f32,
}

/// What a held item fires: (ammo item, projectile, muzzle speed in blocks/s, cooldown in s).
/// A grenade in the hand is thrown (slower, same ammo = itself).
pub fn weapon(id: ItemId) -> Option<(ItemId, ShotKind, f64, f32)> {
    match id {
        ids::GRENADE_LAUNCHER => Some((ids::GRENADE, ShotKind::Grenade, 28.0, 0.8)),
        ids::BAZOOKA => Some((ids::ROCKET, ShotKind::Rocket, 45.0, 1.5)),
        ids::GRENADE => Some((ids::GRENADE, ShotKind::Grenade, 15.0, 0.6)),
        _ => None,
    }
}

pub struct Explosives {
    pub lit: Vec<Lit>,
    pub shots: Vec<Shot>,
    cooldown: f32,
    rng: u64,
}

impl Explosives {
    pub fn new(seed: u64) -> Self {
        Self { lit: Vec::new(), shots: Vec::new(), cooldown: 0.0, rng: splitmix(seed ^ 0xB00F) }
    }

    fn rand(&mut self) -> f64 {
        self.rng = splitmix(self.rng);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    /// The weapon cooldown has passed.
    pub fn ready(&self) -> bool {
        self.cooldown <= 0.0
    }

    /// Launches a projectile from `from` along the unit vector `dir`.
    pub fn fire(&mut self, kind: ShotKind, from: DVec3, dir: DVec3, speed: f64, cooldown: f32) {
        self.cooldown = cooldown;
        if self.shots.len() >= MAX_LIVE {
            return;
        }
        self.shots.push(Shot { kind, pos: from + dir * 0.6, vel: dir * speed, age: 0.0 });
    }

    /// Lights the dynamite block at `cell`: the block disappears and a charge starts counting.
    pub fn light(&mut self, world: &mut impl WorldEdit, cell: Cell, fuse: f32) -> bool {
        if world.cell_block(cell) != Block::Dynamite {
            return false;
        }
        let pos = world.grid().cell_center(cell);
        if !world.set_block(cell, Block::Air) {
            return false;
        }
        if self.lit.len() < MAX_LIVE {
            self.lit.push(Lit { pos, fuse });
        }
        true
    }

    /// Advances fuses and projectiles. `targets` are creature centres a projectile explodes on.
    /// Returns the blasts of this frame and every block they removed.
    pub fn update(&mut self, world: &mut impl WorldEdit, targets: &[DVec3], dt: f32) -> (Vec<Blast>, Vec<(Cell, Block)>) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        let mut blasts = Vec::new();

        for l in &mut self.lit {
            l.fuse -= dt;
        }
        for l in self.lit.iter().filter(|l| l.fuse <= 0.0) {
            blasts.push(Blast { pos: l.pos, power: DYNAMITE_POWER });
        }
        self.lit.retain(|l| l.fuse > 0.0);

        let dtd = dt as f64;
        let mut i = 0;
        while i < self.shots.len() {
            let mut s = self.shots[i];
            s.age += dt;
            let up = up_at(s.pos);
            let g = if s.kind == ShotKind::Grenade { GRENADE_GRAVITY } else { ROCKET_GRAVITY };
            s.vel -= up * g * dtd;
            let d = s.vel * dtd;
            // Sub-steps so fast rockets do not pass through thin walls.
            let n = (d.length() / 0.4).ceil().max(1.0) as i32;
            let mut hit = None;
            for k in 1..=n {
                let p = s.pos + d * (k as f64 / n as f64);
                let b = world.block_at_world(p);
                if b.is_solid() || b == Block::Lava || targets.iter().any(|t| (*t - p).length_squared() < 0.8 * 0.8) {
                    // Explode just in front of the surface, so the blast is not buried in it.
                    hit = Some(s.pos + d * ((k - 1) as f64 / n as f64));
                    break;
                }
            }
            let life = if s.kind == ShotKind::Grenade { GRENADE_LIFE } else { ROCKET_LIFE };
            if let Some(p) = hit {
                blasts.push(Blast { pos: p, power: s.kind.power() });
                self.shots.swap_remove(i);
                continue;
            }
            s.pos += d;
            if s.age > life {
                blasts.push(Blast { pos: s.pos, power: s.kind.power() });
                self.shots.swap_remove(i);
                continue;
            }
            self.shots[i] = s;
            i += 1;
        }

        let mut broken = Vec::new();
        for b in &blasts {
            self.explode(world, *b, &mut broken);
        }
        (blasts, broken)
    }

    /// Removes the blocks around a blast. Dynamite caught in it is lit with a short fuse.
    fn explode(&mut self, world: &mut impl WorldEdit, blast: Blast, broken: &mut Vec<(Cell, Block)>) {
        let r = blast.power as f64;
        let steps = (r / 0.5).ceil() as i32;
        let mut seen: HashSet<Cell> = HashSet::new();
        for x in -steps..=steps {
            for y in -steps..=steps {
                for z in -steps..=steps {
                    let off = DVec3::new(x as f64, y as f64, z as f64) * 0.5;
                    let dist = off.length();
                    if dist > r {
                        continue;
                    }
                    let c = world.grid().world_to_cell(blast.pos + off);
                    if c.k < 1 || !seen.insert(c) {
                        continue;
                    }
                    let b = world.cell_block(c);
                    if b == Block::Air || b.is_fluid() || !b.is_breakable() || b.info().hardness >= BLAST_PROOF_HARDNESS {
                        continue;
                    }
                    // Ragged edge: the outer shell survives at random.
                    let roll = self.rand();
                    if dist > r * (0.6 + 0.4 * roll) {
                        continue;
                    }
                    if b == Block::Dynamite {
                        let fuse = 0.3 + roll as f32 * 0.7;
                        if self.light(world, c, fuse) {
                            broken.push((c, b));
                        }
                        continue;
                    }
                    if world.set_block(c, Block::Air) {
                        broken.push((c, b));
                    }
                }
            }
        }
    }

    /// Cubes to draw for lit charges (blinking) and projectiles. `time` drives the blinking.
    pub fn debris(&self, time: f32) -> Vec<Debris> {
        let mut v = Vec::with_capacity(self.lit.len() + self.shots.len());
        for l in &self.lit {
            // Blinks faster as the fuse runs out.
            let rate = if l.fuse < 1.0 { 10.0 } else { 4.0 };
            let white = (time * rate).fract() < 0.5;
            let color = if white { [1.0, 0.95, 0.90] } else { [0.86, 0.20, 0.16] };
            let size = 0.98 + if white { 0.06 } else { 0.0 };
            v.push(Debris { pos: l.pos, vel: DVec3::ZERO, color, size, age: 0.0, spin: 0.0 });
        }
        for s in &self.shots {
            let (color, size) = match s.kind {
                ShotKind::Grenade => ([0.25, 0.38, 0.18], 0.26),
                ShotKind::Rocket => ([0.60, 0.62, 0.66], 0.32),
            };
            v.push(Debris { pos: s.pos, vel: s.vel, color, size, age: 0.0, spin: s.age * 4.0 });
        }
        v
    }

    /// Positions of rockets in flight (for the smoke trail).
    pub fn rockets(&self) -> impl Iterator<Item = DVec3> + '_ {
        self.shots.iter().filter(|s| s.kind == ShotKind::Rocket).map(|s| s.pos)
    }
}

/// Damage a blast deals at distance `dist` (zero beyond twice its power).
pub fn blast_damage(power: f32, dist: f64) -> f32 {
    let reach = power as f64 * 2.0;
    if dist >= reach {
        return 0.0;
    }
    let k = (1.0 - dist / reach) as f32;
    k * k * power * 6.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cubesphere::Grid;
    use std::collections::HashMap;

    struct Mock {
        grid: Grid,
        blocks: HashMap<Cell, Block>,
    }
    impl Voxels for Mock {
        fn grid(&self) -> &Grid {
            &self.grid
        }
        fn cell_block(&self, c: Cell) -> Block {
            *self.blocks.get(&c).unwrap_or(&Block::Air)
        }
    }
    impl WorldEdit for Mock {
        fn set_block(&mut self, c: Cell, b: Block) -> bool {
            self.blocks.insert(c, b);
            true
        }
    }

    /// A flat-ish slab of stone on face +Z of a small grid, with dynamite on top.
    fn world() -> (Mock, Cell) {
        let mut w = Mock { grid: Grid::with_cells(64), blocks: HashMap::new() };
        for i in 10..30 {
            for j in 10..30 {
                for k in 40..50 {
                    w.blocks.insert(Cell::new(4, i, j, k), Block::Stone);
                }
            }
        }
        let tnt = Cell::new(4, 20, 20, 50);
        w.blocks.insert(tnt, Block::Dynamite);
        w.blocks.insert(Cell::new(4, 20, 20, 49), Block::Obsidian);
        (w, tnt)
    }

    #[test]
    fn weapons_have_ammo() {
        assert_eq!(weapon(ids::BAZOOKA).map(|w| w.0), Some(ids::ROCKET));
        assert_eq!(weapon(ids::GRENADE_LAUNCHER).map(|w| w.0), Some(ids::GRENADE));
        assert!(weapon(ids::STICK).is_none());
    }

    #[test]
    fn lit_dynamite_explodes_after_its_fuse_and_spares_obsidian() {
        let (mut w, tnt) = world();
        let mut e = Explosives::new(1);
        assert!(e.light(&mut w, tnt, DYNAMITE_FUSE));
        assert_eq!(w.cell_block(tnt), Block::Air);
        let (b, _) = e.update(&mut w, &[], 1.0);
        assert!(b.is_empty(), "still burning");
        let mut broken = Vec::new();
        for _ in 0..10 {
            let (b, br) = e.update(&mut w, &[], 0.5);
            broken.extend(br);
            if !b.is_empty() {
                break;
            }
        }
        assert!(broken.len() > 10, "a crater appears: {}", broken.len());
        assert_eq!(w.cell_block(Cell::new(4, 20, 20, 49)), Block::Obsidian);
    }

    #[test]
    fn blast_lights_nearby_dynamite_and_rockets_hit_walls() {
        let (mut w, tnt) = world();
        let other = Cell::new(4, 22, 20, 50);
        w.blocks.insert(other, Block::Dynamite);
        let mut e = Explosives::new(2);
        e.light(&mut w, tnt, 0.01);
        e.update(&mut w, &[], 0.05);
        assert_eq!(w.cell_block(other), Block::Air);
        assert_eq!(e.lit.len(), 1, "the second stick is now burning");

        // A rocket fired down at the slab explodes on it.
        let (mut w, _) = world();
        let start = w.grid.cell_center(Cell::new(4, 15, 15, 60));
        let down = -up_at(start);
        let mut e = Explosives::new(3);
        e.fire(ShotKind::Rocket, start, down, 45.0, 1.5);
        let mut blasts = Vec::new();
        for _ in 0..60 {
            blasts.extend(e.update(&mut w, &[], 1.0 / 60.0).0);
        }
        assert_eq!(blasts.len(), 1);
        assert!(e.shots.is_empty());
    }

    #[test]
    fn damage_falls_off_with_distance() {
        assert!(blast_damage(4.0, 0.0) > blast_damage(4.0, 3.0));
        assert_eq!(blast_damage(4.0, 9.0), 0.0);
    }
}
