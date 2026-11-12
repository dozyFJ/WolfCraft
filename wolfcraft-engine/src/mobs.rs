//! Creatures: hostile (ghoul, crawler) and peaceful (boar, sheep). Simple AI on the sphere:
//! hostile mobs spawn at night / in the dark and chase the player, peaceful ones wander by day.

use crate::physics::{Body, MoveInput, Voxels, step, up_at};
use crate::planet::splitmix;
use glam::DVec3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Ghoul,
    Crawler,
    Boar,
    Sheep,
}

impl MobKind {
    pub fn hostile(self) -> bool {
        matches!(self, MobKind::Ghoul | MobKind::Crawler)
    }
    pub fn name(self) -> &'static str {
        match self {
            MobKind::Ghoul => "Упырь",
            MobKind::Crawler => "Ползун",
            MobKind::Boar => "Кабан",
            MobKind::Sheep => "Овца",
        }
    }
    pub fn max_hp(self) -> f32 {
        match self {
            MobKind::Ghoul => 20.0,
            MobKind::Crawler => 12.0,
            MobKind::Boar => 10.0,
            MobKind::Sheep => 8.0,
        }
    }
    /// Walking speed, blocks per second.
    pub fn speed(self) -> f64 {
        match self {
            MobKind::Ghoul => 2.6,
            MobKind::Crawler => 4.4,
            MobKind::Boar => 2.2,
            MobKind::Sheep => 1.8,
        }
    }
    pub fn damage(self) -> f32 {
        match self {
            MobKind::Ghoul => 3.0,
            MobKind::Crawler => 2.0,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Mob {
    pub kind: MobKind,
    pub body: Body,
    pub hp: f32,
    /// Unit vector tangent to the sphere.
    pub heading: DVec3,
    pub anim: f32,
    pub amount: f32,
    /// Seconds of red flash after being hit.
    pub hurt: f32,
    attack_cd: f32,
    /// Seconds the mob still remembers where the player is after seeing them.
    alert: f32,
    wander: f32,
    wander_dir: DVec3,
}

impl Mob {
    pub fn new(kind: MobKind, pos: DVec3, heading: DVec3) -> Self {
        Self { kind, body: Body::new(pos), hp: kind.max_hp(), heading, anim: 0.0, amount: 0.0, hurt: 0.0, attack_cd: 0.0, alert: 0.0, wander: 0.0, wander_dir: DVec3::ZERO }
    }
    /// Centre of the body, for aiming.
    pub fn centre(&self) -> DVec3 {
        self.body.pos + up_at(self.body.pos) * 0.9
    }
}

pub struct MobManager {
    pub mobs: Vec<Mob>,
    /// New creatures appear (off in Studio baseplate worlds).
    pub spawning: bool,
    spawn_timer: f32,
    /// Cubes flying off dead creatures.
    pub debris: Vec<Debris>,
    rng: u64,
}

const HOSTILE_CAP: usize = 8;
/// Upper bound of live particles (death cubes + block chips).
pub const MAX_DEBRIS: usize = 400;
const PASSIVE_CAP: usize = 6;
const DESPAWN_DIST: f64 = 90.0;
/// Farther than this from the player a calm mob sleeps: no AI, no physics, no animation.
pub const SLEEP_DIST: f64 = 40.0;

fn tangent(v: DVec3, up: DVec3) -> DVec3 {
    v - up * v.dot(up)
}

impl MobManager {
    pub fn new(seed: u64) -> Self {
        Self { mobs: Vec::new(), spawning: true, debris: Vec::new(), spawn_timer: 3.0, rng: splitmix(seed ^ 0x40B5) }
    }

    pub fn rand(&mut self) -> f64 {
        self.rng = splitmix(self.rng);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Finds the ground along the radial line through `dir` (unit), searching down from above `r`.
    fn ground(world: &impl Voxels, dir: DVec3, r: f64) -> Option<DVec3> {
        let mut rad = r + 14.0;
        while rad > r - 24.0 {
            let g = world.grid();
            let p = g.along(dir, rad);
            let b = world.block_at_world(p);
            if b.is_solid() {
                let above = world.block_at_world(g.along(dir, rad + 1.0));
                let above2 = world.block_at_world(g.along(dir, rad + 2.0));
                if !above.is_solid() && !above.is_fluid() && !above2.is_solid() && !above2.is_fluid() {
                    return Some(g.along(dir, rad + 0.55));
                }
                return None;
            }
            rad -= 0.5;
        }
        None
    }

    /// Throws ~16 cubes in the given colours out of a body at pos (feet), size = body height.
    pub fn burst(&mut self, pos: DVec3, palette: &[[f32; 3]], size: f64) {
        let up = up_at(pos);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let t1 = tangent(helper, up).normalize();
        let t2 = up.cross(t1);
        for i in 0..16 {
            let a = self.rand() * std::f64::consts::TAU;
            let s = 2.0 + self.rand() * 4.0;
            let h = 0.15 + self.rand() * size;
            let vel = (t1 * a.cos() + t2 * a.sin()) * s + up * (3.0 + self.rand() * 5.0);
            let size = (0.10 + self.rand() * 0.14) as f32;
            let spin = (self.rand() * 6.0) as f32;
            self.debris.push(Debris { pos: pos + up * h, vel, color: palette[i % palette.len()], size, age: 0.0, spin, kind: FxKind::Fall, glow: false });
        }
    }

    /// Small block chips flying from `pos` (hits and breaking). The pool is capped: the oldest
    /// particles are reused, and all of them share one vertex buffer when drawn.
    pub fn chips(&mut self, pos: DVec3, palette: &[[f32; 3]], count: usize, speed: f64) {
        if palette.is_empty() {
            return;
        }
        let up = up_at(pos);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let t1 = tangent(helper, up).normalize();
        let t2 = up.cross(t1);
        for i in 0..count {
            let a = self.rand() * std::f64::consts::TAU;
            let s = speed * (0.3 + self.rand() * 0.7);
            let vel = (t1 * a.cos() + t2 * a.sin()) * s + up * (1.0 + self.rand() * 2.5) * (speed / 2.5);
            let off = (t1 * (self.rand() - 0.5) + t2 * (self.rand() - 0.5) + up * (self.rand() - 0.5)) * 0.5;
            let size = (0.045 + self.rand() * 0.055) as f32;
            let spin = (self.rand() * 6.0) as f32;
            // Start aged so chips live about a second instead of three.
            let d = Debris { pos: pos + off, vel, color: palette[i % palette.len()], size, age: 2.0 + (self.rand() * 0.4) as f32, spin, kind: FxKind::Fall, glow: false };
            if self.debris.len() >= MAX_DEBRIS {
                let oldest = self.debris.iter().enumerate().max_by(|a, b| a.1.age.total_cmp(&b.1.age)).map(|(i, _)| i).unwrap_or(0);
                self.debris[oldest] = d;
            } else {
                self.debris.push(d);
            }
        }
    }

    /// One effect particle (embers, smoke, leaves, splashes, dust) living `life` seconds.
    #[allow(clippy::too_many_arguments)]
    pub fn fx(&mut self, pos: DVec3, vel: DVec3, color: [f32; 3], size: f32, life: f32, kind: FxKind) {
        let spin = (self.rand() * 6.0) as f32;
        let d = Debris { pos, vel, color, size, age: (3.0 - life).clamp(0.0, 2.9), spin, kind, glow: false };
        if self.debris.len() >= MAX_DEBRIS {
            // Full: replace the oldest particle.
            if let Some((i, _)) = self.debris.iter().enumerate().max_by(|a, b| a.1.age.total_cmp(&b.1.age)) {
                self.debris[i] = d;
            }
        } else {
            self.debris.push(d);
        }
    }

    /// A glowing effect particle (embers, sparks): bright even at night.
    pub fn fx_glow(&mut self, pos: DVec3, vel: DVec3, color: [f32; 3], size: f32, life: f32, kind: FxKind) {
        self.fx(pos, vel, color, size, life, kind);
        if let Some(d) = self.debris.iter_mut().find(|d| d.pos == pos && !d.glow) {
            d.glow = true;
        }
    }

    fn step_debris(&mut self, world: &impl Voxels, dt: f64) {
        for d in self.debris.iter_mut() {
            d.age += dt as f32;
            let up = up_at(d.pos);
            match d.kind {
                FxKind::Fall => {}
                FxKind::Rise => {
                    // Embers and smoke: float up and slow down, no collisions.
                    d.vel = d.vel * (-1.8 * dt).exp() + up * 0.9 * dt;
                    d.pos += d.vel * dt;
                    continue;
                }
                FxKind::Leaf => {
                    // Leaves flutter down slowly, swaying from side to side.
                    let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                    let side = tangent(helper, up).normalize();
                    let target = -up * 0.9 + side * ((d.age * 2.6 + d.spin).sin() as f64 * 0.8);
                    d.vel += (target - d.vel) * (1.0 - (-3.0 * dt).exp());
                    let next = d.pos + d.vel * dt;
                    if !world.block_at_world(next).is_solid() {
                        d.pos = next;
                    } else {
                        d.vel = DVec3::ZERO;
                    }
                    continue;
                }
            }
            d.vel -= up * 20.0 * dt;
            let next = d.pos + d.vel * dt;
            if world.block_at_world(next).is_solid() {
                // Bounce a little and lose most speed.
                let vn = d.vel.dot(up);
                d.vel = (d.vel - up * vn) * 0.5 - up * vn * 0.25;
                if d.vel.dot(up).abs() < 1.0 {
                    d.vel *= 0.3;
                }
            } else {
                d.pos = next;
            }
        }
        self.debris.retain(|d| d.age < 3.0);
    }

    fn line_clear(world: &impl Voxels, a: DVec3, b: DVec3) -> bool {
        let d = b - a;
        let n = (d.length() / 0.5).ceil().max(1.0) as i32;
        (1..n).all(|i| !world.block_at_world(a + d * (i as f64 / n as f64)).is_solid())
    }

    fn try_spawn(&mut self, world: &impl Voxels, player: DVec3, day: bool) {
        let hostile = self.mobs.iter().filter(|m| m.kind.hostile()).count();
        let passive = self.mobs.len() - hostile;
        let up = up_at(player);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let t1 = tangent(helper, up).normalize();
        let t2 = up.cross(t1);
        let ang = self.rand() * std::f64::consts::TAU;
        let dist = 22.0 + self.rand() * 22.0;
        let dir = world.grid().col_dir(player + (t1 * ang.cos() + t2 * ang.sin()) * dist);
        let want_hostile = !day;
        if want_hostile && hostile >= HOSTILE_CAP || !want_hostile && passive >= PASSIVE_CAP {
            return;
        }
        let Some(pos) = Self::ground(world, dir, world.grid().radial(player)) else { return };
        let pick = self.rand();
        let kind = if want_hostile {
            if pick < 0.6 { MobKind::Ghoul } else { MobKind::Crawler }
        } else if pick < 0.5 {
            MobKind::Boar
        } else {
            MobKind::Sheep
        };
        let h = tangent(t1, up_at(pos));
        self.mobs.push(Mob::new(kind, pos, h.normalize_or_zero()));
    }

    /// Advances all mobs. Returns the damage dealt to the player this frame.
    pub fn update(&mut self, world: &impl Voxels, player: DVec3, day: bool, peaceful: bool, targetable: bool, dt: f64) -> f32 {
        if peaceful {
            self.mobs.retain(|m| !m.kind.hostile());
        }
        self.spawn_timer -= dt as f32;
        if self.spawn_timer <= 0.0 {
            self.spawn_timer = 2.0;
            if self.spawning {
                self.try_spawn(world, player, day);
            }
        }
        let mut dealt = 0.0;
        let mut rolls: Vec<f64> = (0..self.mobs.len() * 2).map(|_| self.rand()).collect();
        for (i, m) in self.mobs.iter_mut().enumerate() {
            let up = up_at(m.body.pos);
            let to_player = player - m.body.pos;
            let flat = tangent(to_player, up);
            let dist = to_player.length();
            m.hurt = (m.hurt - dt as f32).max(0.0);
            m.attack_cd = (m.attack_cd - dt as f32).max(0.0);
            let mut wish = DVec3::ZERO;
            m.alert = (m.alert - dt as f32).max(0.0);
            if dist > SLEEP_DIST && m.alert <= 0.0 && m.body.on_ground {
                // Asleep: stands still until the player comes closer.
                m.amount = 0.0;
                continue;
            }
            if m.kind.hostile() && targetable && dist < 16.0 {
                // Sees the player: close enough, in front of the mob and nothing solid between.
                let facing = flat.normalize_or_zero().dot(m.heading) > -0.2 || dist < 4.0;
                if facing && Self::line_clear(world, m.body.pos + up, player + up) {
                    m.alert = 4.0;
                }
            }
            if m.kind.hostile() && targetable && m.alert > 0.0 {
                if flat.length() > 0.4 {
                    wish = flat.normalize();
                    m.heading = wish;
                }
                if dist < 1.5 && m.attack_cd <= 0.0 {
                    dealt += m.kind.damage();
                    m.attack_cd = 1.1;
                }
            } else {
                m.wander -= dt as f32;
                if m.wander <= 0.0 {
                    m.wander = 2.0 + rolls[i * 2] as f32 * 4.0;
                    let a = rolls[i * 2 + 1] * std::f64::consts::TAU;
                    let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                    let t1 = tangent(helper, up).normalize();
                    let t2 = up.cross(t1);
                    // About a third of the time the animal just stands.
                    m.wander_dir = if rolls[i * 2] < 0.33 { DVec3::ZERO } else { t1 * a.cos() + t2 * a.sin() };
                }
                wish = tangent(m.wander_dir, up).normalize_or_zero();
                if wish.length_squared() > 0.0 {
                    m.heading = wish;
                }
            }
            let speed = m.kind.speed() * if m.kind.hostile() && day { 0.8 } else { 1.0 };
            let before = m.body.pos;
            step(world, &mut m.body, &MoveInput { wish, speed, jump: false, up: false, down: false }, dt);
            // The body is a thin column, but heads and the ghoul's outstretched arms reach
            // further forward: step back from a wall or tree trunk so they do not sink into it.
            // Only above step height, so one-block ledges are still climbed.
            // (Short animals have their heads below step height, so this is for tall mobs.)
            let reach = m.kind.reach();
            for _ in 0..if m.kind.size() > 1.4 { 4 } else { 0 } {
                let front = |h: f64| world.block_at_world(m.body.pos + m.heading * reach + up * h).is_solid();
                if !(front(1.25) || front(m.kind.size() * 0.85)) {
                    break;
                }
                m.body.pos -= m.heading * 0.06;
            }
            // Stuck against a wall: hop.
            let moved = (m.body.pos - before).length() / dt.max(1e-6);
            if wish.length_squared() > 0.0 && m.body.on_ground && moved < speed * 0.25 {
                m.body.vel += up * 7.5;
                m.body.on_ground = false;
            }
            let moving = wish.length_squared() > 0.0;
            m.amount += ((moving as i32 as f32) - m.amount) * (1.0 - (-8.0 * dt as f32).exp());
            m.anim += (m.kind.speed() as f32 * 3.2) * dt as f32 * m.amount;
            m.heading = tangent(m.heading, up).normalize_or_zero();
            if m.heading.length_squared() < 0.5 {
                m.heading = tangent(DVec3::Y, up).normalize_or_zero();
            }
        }
        rolls.clear();
        // Dead creatures fall apart into coloured cubes.
        let dead: Vec<(DVec3, MobKind)> = self.mobs.iter().filter(|m| m.hp <= 0.0).map(|m| (m.body.pos, m.kind)).collect();
        for (pos, kind) in dead {
            self.burst(pos, &kind.palette(), kind.size());
        }
        self.step_debris(world, dt);
        self.mobs.retain(|m| (m.body.pos - player).length() < DESPAWN_DIST && m.hp > 0.0);
        dealt
    }

    /// Nearest mob hit by a ray (origin, unit dir) within `reach`: (index, distance along the ray).
    pub fn raycast(&self, origin: DVec3, dir: DVec3, reach: f64) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (i, m) in self.mobs.iter().enumerate() {
            let to = m.centre() - origin;
            let t = to.dot(dir);
            if t < 0.0 || t > reach {
                continue;
            }
            if (to - dir * t).length() < 0.75 && best.is_none_or(|(_, bt)| t < bt) {
                best = Some((i, t));
            }
        }
        best
    }

    /// Damages a mob with knockback away from `from`. Returns the mob if it died.
    pub fn hurt(&mut self, i: usize, amount: f32, from: DVec3) -> Option<MobKind> {
        let m = self.mobs.get_mut(i)?;
        m.hp -= amount;
        m.hurt = 0.35;
        let up = up_at(m.body.pos);
        let away = tangent(m.body.pos - from, up).normalize_or_zero();
        m.body.vel += away * 7.0 + up * 4.0;
        m.body.on_ground = false;
        (m.hp <= 0.0).then_some(m.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_have_sane_stats() {
        for k in [MobKind::Ghoul, MobKind::Crawler, MobKind::Boar, MobKind::Sheep] {
            assert!(k.max_hp() > 0.0 && k.speed() > 0.0);
            assert_eq!(k.hostile(), k.damage() > 0.0);
        }
    }

    #[test]
    fn hurting_kills_and_knocks_back() {
        let mut m = MobManager::new(1);
        let pos = DVec3::new(0.0, 1000.0, 0.0);
        m.mobs.push(Mob::new(MobKind::Sheep, pos, DVec3::X));
        assert_eq!(m.hurt(0, 3.0, pos - DVec3::X), None);
        assert!(m.mobs[0].body.vel.length() > 1.0);
        assert_eq!(m.hurt(0, 100.0, pos - DVec3::X), Some(MobKind::Sheep));
    }

    #[test]
    fn raycast_finds_mob_in_front() {
        let mut m = MobManager::new(1);
        let pos = DVec3::new(0.0, 1000.0, 0.0);
        m.mobs.push(Mob::new(MobKind::Boar, pos + DVec3::new(2.0, 0.0, 0.0), DVec3::X));
        assert!(m.raycast(pos + DVec3::Y * 0.9, DVec3::X, 4.0).is_some());
        assert!(m.raycast(pos + DVec3::Y * 0.9, DVec3::Z, 4.0).is_none());
    }
}

/// One cube of a creature that fell apart.
#[derive(Clone, Copy, Debug)]
pub struct Debris {
    pub pos: DVec3,
    pub vel: DVec3,
    pub color: [f32; 3],
    pub size: f32,
    pub age: f32,
    pub spin: f32,
    pub kind: FxKind,
    /// Glows in the dark (embers, sparks): drawn unlit.
    pub glow: bool,
}

/// How a particle moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxKind {
    /// Falls and bounces (chips, cubes of dead creatures, splashes, dust).
    Fall,
    /// Floats up and fades (embers, smoke, puffs).
    Rise,
    /// Flutters down slowly (falling leaves).
    Leaf,
}

impl MobKind {
    /// Main body colours, used for the cubes when the creature dies.
    pub fn palette(self) -> Vec<[f32; 3]> {
        match self {
            MobKind::Ghoul => vec![[0.42, 0.52, 0.38], [0.30, 0.27, 0.34], [0.42, 0.52, 0.38]],
            MobKind::Crawler => vec![[0.22, 0.14, 0.30], [0.15, 0.10, 0.20], [0.95, 0.20, 0.15]],
            MobKind::Boar => vec![[0.62, 0.42, 0.36], [0.70, 0.50, 0.44], [0.45, 0.30, 0.26]],
            MobKind::Sheep => vec![[0.92, 0.91, 0.86], [0.92, 0.91, 0.86], [0.25, 0.22, 0.22]],
        }
    }

    /// Approximate body height in blocks.
    /// How far the model reaches forward from the body centre (head, arms).
    pub fn reach(self) -> f64 {
        match self {
            MobKind::Ghoul => 0.62,
            MobKind::Crawler => 0.55,
            MobKind::Boar | MobKind::Sheep => 0.62,
        }
    }

    pub fn size(self) -> f64 {
        match self {
            MobKind::Ghoul => 1.8,
            MobKind::Crawler => 0.7,
            MobKind::Boar | MobKind::Sheep => 0.9,
        }
    }
}
