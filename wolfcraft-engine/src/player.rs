//! The player: orientation on a sphere, movement, survival stats, persistence (`player.dat`).

use crate::config::{Difficulty, GameMode};
use crate::inventory::Inventory;
use crate::physics::{self, Body, EYE_HEIGHT, MoveInput, Voxels, up_at};
use glam::DVec3;

pub const MAX_HEALTH: f32 = 20.0;
pub const MAX_HUNGER: f32 = 20.0;
const MAX_AIR: f32 = 15.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// +1 forward, -1 back.
    pub forward: f32,
    /// +1 right, -1 left.
    pub strafe: f32,
    pub jump: bool,
    pub sprint: bool,
    pub sneak: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Survival {
    pub health: f32,
    pub hunger: f32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub air: f32,
    pub xp: u32,
    pub dead: bool,
    /// Seconds of red flash left after taking damage (UI feedback).
    pub hurt_flash: f32,
    regen_timer: f32,
    starve_timer: f32,
    drown_timer: f32,
    lava_timer: f32,
}

impl Default for Survival {
    fn default() -> Self {
        Self {
            health: MAX_HEALTH,
            hunger: MAX_HUNGER,
            saturation: 5.0,
            exhaustion: 0.0,
            air: MAX_AIR,
            xp: 0,
            dead: false,
            hurt_flash: 0.0,
            regen_timer: 0.0,
            starve_timer: 0.0,
            drown_timer: 0.0,
            lava_timer: 0.0,
        }
    }
}

impl Survival {
    pub fn level(&self) -> u32 {
        (self.xp as f32).sqrt() as u32
    }
    /// Progress to the next level in 0..1.
    pub fn level_progress(&self) -> f32 {
        let l = self.level();
        (self.xp - l * l) as f32 / (2 * l + 1) as f32
    }
    pub fn add_xp(&mut self, n: u32) {
        self.xp += n;
    }
    pub fn exhaust(&mut self, amount: f32) {
        self.exhaustion += amount;
    }
    pub fn eat(&mut self, hunger: f32, sat: f32) {
        self.hunger = (self.hunger + hunger).min(MAX_HUNGER);
        self.saturation = (self.saturation + sat).min(self.hunger);
    }
    pub fn damage(&mut self, amount: f32, difficulty: Difficulty) {
        let a = amount * difficulty.damage_scale();
        if a <= 0.0 || self.dead {
            return;
        }
        self.health -= a;
        self.hurt_flash = 0.35;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.dead = true;
        }
    }
}

pub struct Player {
    pub body: Body,
    /// Unit vector tangent to the sphere: where "forward" points on the ground.
    pub heading: DVec3,
    /// Look pitch in radians (+ = up).
    pub pitch: f64,
    pub mode: GameMode,
    pub inventory: Inventory,
    /// Holding the shield up (RMB): walks slowly.
    pub blocking: bool,
    pub survival: Survival,
    pub spawn: DVec3,
    pub sneaking: bool,
    pub sprinting: bool,
    pub world_time: f64,
    /// Walk-cycle phase (radians) and swing amount 0..1 for the character model.
    pub anim_phase: f32,
    pub anim_amount: f32,
    anim_prev: DVec3,
    air_time: f32,
    fall_peak: f64,
}

pub fn tangent_fallback(up: DVec3) -> DVec3 {
    let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    (helper - up * helper.dot(up)).normalize()
}

impl Player {
    pub fn new(spawn: DVec3, mode: GameMode) -> Self {
        let up = up_at(spawn);
        Self {
            body: Body::new(spawn),
            heading: tangent_fallback(up),
            pitch: 0.0,
            mode,
            inventory: Inventory::default(),
            blocking: false,
            survival: Survival::default(),
            spawn,
            sneaking: false,
            sprinting: false,
            world_time: 0.25 * DAY_LENGTH,
            anim_phase: 0.0,
            anim_amount: 0.0,
            anim_prev: spawn,
            air_time: 0.0,
            fall_peak: 0.0,
        }
    }

    pub fn up(&self) -> DVec3 {
        up_at(self.body.pos)
    }

    pub fn eye_pos(&self) -> DVec3 {
        let h = if self.sneaking { EYE_HEIGHT - 0.3 } else { EYE_HEIGHT };
        self.body.pos + self.up() * h
    }

    pub fn right(&self) -> DVec3 {
        self.heading.cross(self.up()).normalize_or_zero()
    }

    pub fn look_dir(&self) -> DVec3 {
        (self.heading * self.pitch.cos() + self.up() * self.pitch.sin()).normalize()
    }

    /// Keeps `heading` tangent to the sphere at the current position (parallel transport).
    pub fn align_heading(&mut self) {
        let up = self.up();
        let h = self.heading - up * self.heading.dot(up);
        self.heading = if h.length_squared() < 1e-9 { tangent_fallback(up) } else { h.normalize() };
    }

    /// Mouse look. `dyaw` positive turns right.
    pub fn look(&mut self, dyaw: f64, dpitch: f64) {
        self.align_heading();
        let up = self.up();
        let (s, c) = (-dyaw).sin_cos();
        // Rodrigues rotation of heading around up.
        self.heading = (self.heading * c + up.cross(self.heading) * s).normalize();
        self.pitch = (self.pitch + dpitch).clamp(-1.553, 1.553);
    }

    pub fn is_creative(&self) -> bool {
        self.mode == GameMode::Creative
    }

    pub fn toggle_flight(&mut self) {
        if self.is_creative() {
            self.body.flying = !self.body.flying;
        }
    }

    /// Total defence from the worn armor.
    pub fn armor_points(&self) -> u32 {
        self.inventory.armor.iter().flatten().map(|s| crate::items::armor_points(s.id) as u32).sum()
    }

    pub fn damage(&mut self, amount: f32, difficulty: Difficulty) {
        if self.is_creative() {
            return;
        }
        self.survival.damage(amount, difficulty);
    }

    pub fn respawn(&mut self) {
        self.body = Body::new(self.spawn);
        self.survival = Survival::default();
        self.pitch = 0.0;
        self.fall_peak = 0.0;
        self.align_heading();
    }

    /// One frame of movement and survival simulation.
    pub fn update(&mut self, world: &impl Voxels, c: &Controls, difficulty: Difficulty, dt: f64) {
        if self.survival.dead {
            return;
        }
        self.world_time += dt;
        self.align_heading();
        let right = self.right();
        self.sneaking = c.sneak && !self.body.flying;
        self.sprinting = c.sprint && c.forward > 0.0 && !self.sneaking && (self.survival.hunger > 6.0 || self.is_creative());

        let mut wish = self.heading * c.forward as f64 + right * c.strafe as f64;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        let speed = if self.body.flying {
            if c.sprint { 21.0 } else { 10.9 }
        } else if self.sneaking {
            1.7
        } else if self.sprinting {
            5.9
        } else {
            4.3
        };
        let speed: f64 = if self.blocking && !self.body.flying { f64::min(speed, 1.9) } else { speed };
        // Creative flight speeds up with height: in space you cover distances quickly.
        let speed = if self.body.flying { speed * space_boost(world.grid(), world.grid().radial(self.body.pos)) } else { speed };
        let input = MoveInput { wish, speed, jump: c.jump, up: c.jump, down: c.sneak };
        let altitude_before = crate::cubesphere::radial(self.body.pos);
        let was_grounded = self.body.on_ground;
        let impact = physics::step(world, &mut self.body, &input, dt);
        let _ = altitude_before;

        if self.body.flying {
            self.body.on_ground = false;
        }
        self.align_heading();
        self.animate(dt as f32);

        // Fall tracking.
        let alt = crate::cubesphere::radial(self.body.pos);
        if self.body.on_ground || self.body.in_water || self.body.flying {
            self.fall_peak = alt;
        } else {
            self.fall_peak = self.fall_peak.max(alt);
        }
        if impact > 0.0 && !self.body.in_water && !self.is_creative() {
            let height = impact * impact / (2.0 * physics::GRAVITY);
            if height > 3.5 {
                self.damage((height - 3.0) as f32, difficulty);
            }
        }
        let _ = was_grounded;

        if self.is_creative() {
            self.survival.hunger = MAX_HUNGER;
            self.survival.health = MAX_HEALTH;
            self.survival.air = MAX_AIR;
            return;
        }
        self.tick_survival(c, difficulty, dt as f32);
    }

    fn animate(&mut self, dt: f32) {
        let up = self.up();
        // Measured from the real displacement, so stepping up stairs still counts as walking.
        let d = self.body.pos - self.anim_prev;
        self.anim_prev = self.body.pos;
        let speed = ((d - up * d.dot(up)).length() / dt.max(1e-4) as f64) as f32;
        // Brief hops over steps still count as walking.
        self.air_time = if self.body.on_ground || self.body.in_water { 0.0 } else { self.air_time + dt };
        let moving = self.air_time < 0.3 && speed > 0.2 && speed < 40.0;
        let target = if moving { (speed / 4.3).clamp(0.0, 1.0) } else { 0.0 };
        self.anim_amount += (target - self.anim_amount) * (1.0 - (-12.0 * dt).exp());
        if moving {
            self.anim_phase = (self.anim_phase + speed * dt * 4.6).rem_euclid(std::f32::consts::TAU);
        }
    }

    fn tick_survival(&mut self, c: &Controls, difficulty: Difficulty, dt: f32) {
        let moving = c.forward != 0.0 || c.strafe != 0.0;
        let scale = difficulty.hunger_scale();
        let up_vel = self.body.vel.dot(self.up());
        let s = &mut self.survival;
        s.hurt_flash = (s.hurt_flash - dt).max(0.0);
        if moving && self.body.on_ground {
            s.exhaustion += (if self.sprinting { 0.10 } else { 0.012 }) * dt * 10.0 * scale;
        }
        if c.jump && !self.body.on_ground && up_vel > 5.0 {
            s.exhaustion += 0.05 * scale;
        }
        while s.exhaustion >= 4.0 {
            s.exhaustion -= 4.0;
            if s.saturation > 0.0 {
                s.saturation = (s.saturation - 1.0).max(0.0);
            } else if s.hunger > 0.0 {
                s.hunger -= 1.0;
            }
        }
        // Regeneration and starvation.
        if s.hunger >= 18.0 && s.health < MAX_HEALTH {
            s.regen_timer += dt;
            if s.regen_timer >= 3.0 {
                s.regen_timer = 0.0;
                s.health = (s.health + 1.0).min(MAX_HEALTH);
                s.exhaustion += 1.5 * scale;
            }
        } else {
            s.regen_timer = 0.0;
        }
        if s.hunger <= 0.0 && scale > 0.0 {
            s.starve_timer += dt;
            if s.starve_timer >= 4.0 {
                s.starve_timer = 0.0;
                let floor = match difficulty {
                    Difficulty::Hard => 0.0,
                    Difficulty::Normal => 1.0,
                    _ => 5.0,
                };
                if s.health > floor {
                    s.damage(1.0 / difficulty.damage_scale().max(0.01), difficulty);
                }
            }
        } else {
            s.starve_timer = 0.0;
        }
        // Drowning.
        if self.body.head_in_water {
            s.air = (s.air - dt).max(0.0);
            if s.air <= 0.0 {
                s.drown_timer += dt;
                if s.drown_timer >= 1.0 {
                    s.drown_timer = 0.0;
                    s.damage(2.0, difficulty);
                }
            }
        } else {
            s.air = (s.air + dt * 6.0).min(MAX_AIR);
            s.drown_timer = 0.0;
        }
        // Lava.
        if self.body.in_lava {
            s.lava_timer += dt;
            if s.lava_timer >= 0.5 {
                s.lava_timer = 0.0;
                s.damage(2.0, difficulty);
            }
        } else {
            s.lava_timer = 0.0;
        }
    }

    // ---- persistence ----
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(512);
        o.extend_from_slice(b"WCP1");
        let f64s = |o: &mut Vec<u8>, v: &[f64]| v.iter().for_each(|x| o.extend_from_slice(&x.to_le_bytes()));
        f64s(&mut o, &[self.body.pos.x, self.body.pos.y, self.body.pos.z]);
        f64s(&mut o, &[self.heading.x, self.heading.y, self.heading.z]);
        f64s(&mut o, &[self.spawn.x, self.spawn.y, self.spawn.z]);
        f64s(&mut o, &[self.pitch, self.world_time]);
        o.push(matches!(self.mode, GameMode::Creative) as u8);
        o.push(self.body.flying as u8);
        for v in [self.survival.health, self.survival.hunger, self.survival.saturation, self.survival.air] {
            o.extend_from_slice(&v.to_le_bytes());
        }
        o.extend_from_slice(&self.survival.xp.to_le_bytes());
        self.inventory.to_bytes(&mut o);
        o
    }

    pub fn from_bytes(data: &[u8]) -> Option<Player> {
        if data.len() < 4 || &data[0..4] != b"WCP1" {
            return None;
        }
        let mut off = 4;
        let f64_ = |off: &mut usize| -> Option<f64> {
            let b = data.get(*off..*off + 8)?;
            *off += 8;
            Some(f64::from_le_bytes(b.try_into().ok()?))
        };
        let pos = DVec3::new(f64_(&mut off)?, f64_(&mut off)?, f64_(&mut off)?);
        let heading = DVec3::new(f64_(&mut off)?, f64_(&mut off)?, f64_(&mut off)?);
        let spawn = DVec3::new(f64_(&mut off)?, f64_(&mut off)?, f64_(&mut off)?);
        let pitch = f64_(&mut off)?;
        let world_time = f64_(&mut off)?;
        let mode = if *data.get(off)? == 1 { GameMode::Creative } else { GameMode::Survival };
        let flying = *data.get(off + 1)? == 1;
        off += 2;
        let f32_ = |off: &mut usize| -> Option<f32> {
            let b = data.get(*off..*off + 4)?;
            *off += 4;
            Some(f32::from_le_bytes(b.try_into().ok()?))
        };
        let (health, hunger, saturation, air) = (f32_(&mut off)?, f32_(&mut off)?, f32_(&mut off)?, f32_(&mut off)?);
        let xp = u32::from_le_bytes(data.get(off..off + 4)?.try_into().ok()?);
        off += 4;
        let (inventory, _) = Inventory::from_bytes(data.get(off..)?)?;
        if !pos.is_finite() || !heading.is_finite() || pos.length() < 1.0 {
            return None;
        }
        let mut p = Player::new(spawn, mode);
        p.body = Body::new(pos);
        p.body.flying = flying && mode == GameMode::Creative;
        p.heading = heading;
        p.pitch = pitch.clamp(-1.55, 1.55);
        p.world_time = world_time;
        p.survival.health = health.clamp(1.0, MAX_HEALTH);
        p.survival.hunger = hunger.clamp(0.0, MAX_HUNGER);
        p.survival.saturation = saturation.clamp(0.0, MAX_HUNGER);
        p.survival.air = air.clamp(0.0, MAX_AIR);
        p.survival.xp = xp;
        p.inventory = inventory;
        p.align_heading();
        p.fall_peak = crate::cubesphere::radial(pos);
        Some(p)
    }
}

/// Real seconds per full in-game day.
pub const DAY_LENGTH: f64 = 1200.0;

/// 0..1 → fraction of the day (0 = midnight? no: 0.25 = sunrise, 0.5 = noon, 0.75 = sunset).
pub fn day_fraction(world_time: f64) -> f64 {
    (world_time / DAY_LENGTH).rem_euclid(1.0)
}

/// "HH:MM" in-game clock (06:00 at sunrise).
pub fn clock_string(world_time: f64) -> String {
    let minutes = (day_fraction(world_time) * 24.0 * 60.0) as u32;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Sun direction as seen by someone standing where the local "up" is `up`: the clock is the
/// same everywhere, so 06:00 is sunrise in the east, 12:00 the sun is overhead, 18:00 sunset
/// and 00:00 night, wherever the player is on the planet.
pub fn local_sun(world_time: f64, up: DVec3) -> DVec3 {
    let up = up.normalize_or_zero();
    let axis = if up.y.abs() > 0.98 { DVec3::X } else { DVec3::Y };
    let east = axis.cross(up).normalize();
    let north = up.cross(east);
    let th = day_fraction(world_time) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
    (east * th.cos() + up * th.sin() + north * 0.25).normalize()
}

/// Direction from the planet centre towards the sun.
pub fn sun_direction(world_time: f64) -> DVec3 {
    let a = day_fraction(world_time) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2 * 2.0;
    // Orbit in the XY plane tilted slightly, so the poles see long days/nights.
    let tilt: f64 = 0.41;
    let (s, c) = a.sin_cos();
    DVec3::new(c, s * tilt.cos(), s * tilt.sin()).normalize()
}

/// Flight speed multiplier at distance `r` from the planet centre: 1 near the ground,
/// growing with altitude up to 60× far out in space.
pub fn space_boost(grid: &crate::cubesphere::Grid, r: f64) -> f64 {
    let surface = if grid.cube { grid.n as f64 / 2.0 } else { grid.n as f64 * std::f64::consts::FRAC_2_PI };
    let h = r - surface - 64.0;
    if h <= 0.0 { 1.0 } else { (1.0 + h / 25.0).min(60.0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use crate::physics::tests::Ball as BallWorld;

    #[allow(non_snake_case)]
    fn Ball(r: f64) -> BallWorld {
        BallWorld::new(r)
    }

    #[test]
    fn heading_stays_tangent_while_circling_the_planet() {
        let w = Ball(40.0);
        let mut p = Player::new(DVec3::new(0.0, 46.0, 0.0), GameMode::Survival);
        let c = Controls { forward: 1.0, ..Default::default() };
        let start = p.body.pos;
        let mut angle = 0.0;
        let mut frames = 0;
        while angle < std::f64::consts::TAU - 0.05 && frames < 20000 {
            let before = p.up();
            p.update(&w, &c, Difficulty::Normal, 1.0 / 60.0);
            angle += before.dot(p.up()).clamp(-1.0, 1.0).acos();
            assert!(p.heading.dot(p.up()).abs() < 1e-6, "heading left the tangent plane");
            assert!((p.heading.length() - 1.0).abs() < 1e-9);
            frames += 1;
        }
        assert!(frames < 20000, "never completed a lap (angle {angle})");
        assert!((p.body.pos - start).length() < 14.0, "lap did not return to the start: {}", (p.body.pos - start).length());
        assert!(!p.survival.dead);
    }

    #[test]
    fn look_rotates_heading_and_clamps_pitch() {
        let mut p = Player::new(DVec3::new(0.0, 46.0, 0.0), GameMode::Survival);
        let h0 = p.heading;
        p.look(std::f64::consts::FRAC_PI_2, 5.0);
        assert!(p.heading.dot(h0).abs() < 1e-6, "90° yaw must be perpendicular");
        assert!(p.pitch <= 1.56);
        let right_before = p.right();
        assert!(right_before.dot(p.up()).abs() < 1e-9);
        // Turning right by 90° from heading h makes the new heading equal the old right vector.
        let mut q = Player::new(DVec3::new(0.0, 46.0, 0.0), GameMode::Survival);
        let r = q.right();
        q.look(std::f64::consts::FRAC_PI_2, 0.0);
        assert!((q.heading - r).length() < 1e-6, "mouse right must turn right");
    }

    #[test]
    fn falling_hurts_but_creative_is_immune() {
        let w = Ball(40.0);
        let mut p = Player::new(DVec3::new(0.0, 80.0, 0.0), GameMode::Survival);
        for _ in 0..400 {
            p.update(&w, &Controls::default(), Difficulty::Normal, 1.0 / 60.0);
        }
        assert!(p.survival.health < MAX_HEALTH, "40 block fall should hurt");
        let mut c = Player::new(DVec3::new(0.0, 80.0, 0.0), GameMode::Creative);
        for _ in 0..400 {
            c.update(&w, &Controls::default(), Difficulty::Normal, 1.0 / 60.0);
        }
        assert_eq!(c.survival.health, MAX_HEALTH);
    }

    #[test]
    fn hunger_drains_with_sprinting_and_starvation_hurts() {
        let w = Ball(200.0);
        let mut p = Player::new(DVec3::new(0.0, 206.0, 0.0), GameMode::Survival);
        let c = Controls { forward: 1.0, sprint: true, ..Default::default() };
        let h0 = p.survival.hunger;
        for _ in 0..(60 * 120) {
            p.update(&w, &c, Difficulty::Normal, 1.0 / 60.0);
            if p.survival.hunger <= 6.0 {
                break; // sprinting disabled below 6
            }
        }
        assert!(p.survival.hunger < h0, "hunger must drain");
        p.survival.hunger = 0.0;
        p.survival.saturation = 0.0;
        let hp = p.survival.health;
        for _ in 0..(60 * 9) {
            p.update(&w, &Controls::default(), Difficulty::Normal, 1.0 / 60.0);
        }
        assert!(p.survival.health < hp, "starving must hurt");
        // Peaceful never drains.
        let mut q = Player::new(DVec3::new(0.0, 206.0, 0.0), GameMode::Survival);
        q.survival.hunger = 0.0;
        for _ in 0..(60 * 20) {
            q.update(&w, &Controls::default(), Difficulty::Peaceful, 1.0 / 60.0);
        }
        assert_eq!(q.survival.health, MAX_HEALTH);
    }

    #[test]
    fn death_and_respawn() {
        let mut p = Player::new(DVec3::new(0.0, 46.0, 0.0), GameMode::Survival);
        p.survival.damage(100.0, Difficulty::Normal);
        assert!(p.survival.dead);
        p.respawn();
        assert!(!p.survival.dead && p.survival.health == MAX_HEALTH);
    }

    #[test]
    fn save_roundtrip_keeps_everything() {
        let mut p = Player::new(DVec3::new(10.5, 46.25, -3.0), GameMode::Creative);
        p.body.flying = true;
        p.survival.xp = 123;
        p.survival.hunger = 7.0;
        p.world_time = 777.5;
        p.inventory.add(crate::items::ItemStack::of_block(Block::Lamp, 9));
        p.look(0.7, 0.2);
        let bytes = p.to_bytes();
        let q = Player::from_bytes(&bytes).unwrap();
        assert_eq!(q.body.pos, p.body.pos);
        assert_eq!(q.mode, GameMode::Creative);
        assert!(q.body.flying);
        assert_eq!(q.survival.xp, 123);
        assert_eq!(q.survival.hunger, 7.0);
        assert_eq!(q.world_time, 777.5);
        assert_eq!(q.inventory, p.inventory);
        assert!((q.heading - p.heading).length() < 1e-9);
        assert!(Player::from_bytes(&bytes[..30]).is_none());
        assert!(Player::from_bytes(b"junk").is_none());
    }

    #[test]
    fn walk_animation_runs_only_while_moving() {
        let w = Ball(40.0);
        let mut p = Player::new(DVec3::new(0.0, 46.0, 0.0), GameMode::Survival);
        for _ in 0..120 {
            p.update(&w, &Controls::default(), Difficulty::Normal, 1.0 / 60.0);
        }
        assert!(p.anim_amount < 0.05, "standing still must not swing limbs");
        let c = Controls { forward: 1.0, ..Default::default() };
        let phase0 = p.anim_phase;
        for _ in 0..90 {
            p.update(&w, &c, Difficulty::Normal, 1.0 / 60.0);
        }
        assert!(p.anim_amount > 0.6, "walking swings the limbs ({})", p.anim_amount);
        assert!((p.anim_phase - phase0).abs() > 0.5);
    }

    #[test]
    fn sun_and_clock() {
        assert_eq!(clock_string(0.0), "00:00");
        assert_eq!(clock_string(DAY_LENGTH * 0.25), "06:00");
        let noon = sun_direction(DAY_LENGTH * 0.5);
        let midnight = sun_direction(0.0);
        assert!(noon.dot(midnight) < -0.99, "sun opposite at midnight");
        assert!((noon.length() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn xp_levels() {
        let mut s = Survival::default();
        s.add_xp(16);
        assert_eq!(s.level(), 4);
        assert_eq!(s.level_progress(), 0.0);
        s.add_xp(4);
        assert!(s.level_progress() > 0.4);
    }
}
