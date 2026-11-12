//! Arrows: fly under the planet's gravity, hit creatures, stick into blocks and can be
//! picked up again.

use crate::mobs::{MobKind, MobManager};
use crate::physics::{Voxels, up_at};
use glam::DVec3;

pub const ARROW_GRAVITY: f64 = 20.0;
/// Seconds a stuck arrow stays in the world.
const STUCK_LIFE: f32 = 60.0;

#[derive(Clone, Debug)]
pub struct Arrow {
    pub pos: DVec3,
    pub vel: DVec3,
    /// Direction it points (kept when it sticks into a block).
    pub dir: DVec3,
    pub stuck: bool,
    pub age: f32,
    pub damage: f32,
    /// Can be picked up (survival shots; creative arrows just vanish).
    pub pickup: bool,
    /// Tipped arrow: the effect it puts on what it hits.
    pub effect: Option<crate::magic::Effect>,
}

/// What happened during a step.
#[derive(Default, Debug)]
pub struct ArrowEvents {
    /// Creatures killed by arrows (for loot and xp).
    /// Creatures killed by arrows (for loot, bones and xp), with where they fell.
    pub killed: Vec<(MobKind, DVec3)>,
    /// Tipped arrows that hit: where and which effect (for the splash of colour).
    pub effect_hits: Vec<(DVec3, crate::magic::Effect)>,
    /// Arrows picked up by the player.
    pub picked: u32,
    /// Something was hit (for a sound or a hit marker).
    pub hits: u32,
}

#[derive(Default)]
pub struct Arrows {
    pub list: Vec<Arrow>,
}

impl Arrows {
    /// Shoots an arrow from `from` along `dir` with power 0..1.
    pub fn shoot(&mut self, from: DVec3, dir: DVec3, power: f32, pickup: bool) {
        self.shoot_with(from, dir, power, pickup, 0, None);
    }

    /// `shoot` from a bow with «Меткость» `power_ench` (+25 % damage per level), with a
    /// tipped arrow's effect.
    pub fn shoot_with(&mut self, from: DVec3, dir: DVec3, power: f32, pickup: bool, power_ench: u8, effect: Option<crate::magic::Effect>) {
        let speed = 12.0 + 48.0 * power as f64;
        let dir = dir.normalize_or_zero();
        let damage = (2.0 + 8.0 * power * power) * (1.0 + 0.25 * power_ench as f32);
        self.list.push(Arrow { pos: from + dir * 0.4, vel: dir * speed, dir, stuck: false, age: 0.0, damage, pickup, effect });
        if self.list.len() > 256 {
            self.list.remove(0);
        }
    }

    pub fn step(&mut self, world: &impl Voxels, mobs: &mut MobManager, player_feet: DVec3, can_pick: bool, dt: f64) -> ArrowEvents {
        let mut ev = ArrowEvents::default();
        let mut keep = Vec::with_capacity(self.list.len());
        for mut a in self.list.drain(..) {
            a.age += dt as f32;
            if a.stuck {
                // Walk over a stuck arrow to take it back.
                if a.pickup && can_pick && a.age > 0.5 && (a.pos - (player_feet + up_at(player_feet) * 0.8)).length() < 1.6 {
                    ev.picked += 1;
                    continue;
                }
                if a.age < STUCK_LIFE {
                    keep.push(a);
                }
                continue;
            }
            if a.age > 12.0 {
                continue;
            }
            a.vel -= up_at(a.pos) * ARROW_GRAVITY * dt;
            let travel = a.vel * dt;
            let len = travel.length();
            let dir = travel / len.max(1e-9);
            // Creatures in the way.
            if let Some((i, _)) = mobs.raycast(a.pos, dir, len + 0.3) {
                let from = a.pos - dir * 2.0;
                if let Some(m) = mobs.mobs.get_mut(i) {
                    match a.effect {
                        Some(crate::magic::Effect::Poison) => m.poison = m.poison.max(12.0),
                        Some(crate::magic::Effect::Slowness) => m.slow = m.slow.max(15.0),
                        _ => {}
                    }
                }
                let extra = if a.effect == Some(crate::magic::Effect::Harm) { 6.0 } else { 0.0 };
                if let Some(e) = a.effect {
                    ev.effect_hits.push((a.pos, e));
                }
                if let Some(kind) = mobs.hurt(i, a.damage + extra, from) {
                    ev.killed.push((kind, a.pos));
                }
                ev.hits += 1;
                continue;
            }
            // Blocks: march in small steps and stop just before the first solid one.
            let steps = (len / 0.2).ceil().max(1.0) as i32;
            let mut stuck = false;
            for s in 1..=steps {
                let p = a.pos + dir * (len * s as f64 / steps as f64);
                if world.block_at_world(p).is_solid() {
                    a.pos += dir * (len * (s as f64 - 0.6) / steps as f64);
                    stuck = true;
                    break;
                }
            }
            if stuck {
                a.stuck = true;
                a.age = 0.0;
                a.dir = dir;
                a.vel = DVec3::ZERO;
                ev.hits += 1;
            } else {
                a.pos += travel;
                a.dir = dir;
            }
            keep.push(a);
        }
        self.list = keep;
        ev
    }
}
