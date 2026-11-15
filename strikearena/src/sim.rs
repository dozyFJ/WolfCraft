//! The match: soldiers, weapons, shooting, bots, score. No drawing here.

use crate::map::Map;
use glam::{Vec2, Vec3};

pub const EYE: f32 = 1.6;
pub const RADIUS: f32 = 0.4;
pub const KILLS_TO_WIN: u32 = 30;
pub const MATCH_TIME: f32 = 240.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gun {
    Rifle,
    Shotgun,
    Sniper,
    Pistol,
}

pub const GUNS: [Gun; 4] = [Gun::Rifle, Gun::Shotgun, Gun::Sniper, Gun::Pistol];

pub struct GunSpec {
    pub name: &'static str,
    pub damage: f32,
    pub interval: f32,
    pub mag: u32,
    pub reserve: u32,
    pub reload: f32,
    pub spread: f32,
    pub aim_spread: f32,
    pub pellets: u32,
    pub range: f32,
    pub auto: bool,
    pub kick: f32,
    pub zoom: f32,
    pub speed: f32,
}

pub fn gun(g: Gun) -> GunSpec {
    match g {
        Gun::Rifle => GunSpec { name: "Автомат", damage: 24.0, interval: 0.1, mag: 30, reserve: 120, reload: 2.0, spread: 0.035, aim_spread: 0.008, pellets: 1, range: 70.0, auto: true, kick: 0.012, zoom: 1.4, speed: 1.0 },
        Gun::Shotgun => GunSpec { name: "Дробовик", damage: 14.0, interval: 0.8, mag: 6, reserve: 30, reload: 2.6, spread: 0.09, aim_spread: 0.06, pellets: 8, range: 22.0, auto: false, kick: 0.05, zoom: 1.2, speed: 0.95 },
        Gun::Sniper => GunSpec { name: "Снайперка", damage: 95.0, interval: 1.2, mag: 5, reserve: 20, reload: 3.0, spread: 0.08, aim_spread: 0.0, pellets: 1, range: 120.0, auto: false, kick: 0.07, zoom: 4.0, speed: 0.85 },
        Gun::Pistol => GunSpec { name: "Пистолет", damage: 30.0, interval: 0.22, mag: 12, reserve: 48, reload: 1.4, spread: 0.025, aim_spread: 0.01, pellets: 1, range: 45.0, auto: false, kick: 0.025, zoom: 1.25, speed: 1.1 },
    }
}

#[derive(Clone, Debug)]
pub struct Bot {
    path: Vec<Vec2>,
    repath: f32,
    target: Option<usize>,
    seen: f32,
    strafe: f32,
    skill: f32,
}

#[derive(Clone, Debug)]
pub struct Soldier {
    pub name: String,
    pub team: u8,
    pub pos: Vec2,
    pub yaw: f32,
    pub pitch: f32,
    pub hp: f32,
    pub respawn: f32,
    pub protect: f32,
    pub gun: usize,
    pub mag: [u32; 4],
    pub reserve: [u32; 4],
    pub reload: f32,
    pub cooldown: f32,
    pub recoil: f32,
    pub kills: u32,
    pub deaths: u32,
    pub walk: f32,
    pub moving: f32,
    pub aiming: bool,
    pub fired: f32,
    pub hurt: f32,
    pub hurt_from: Vec2,
    /// Who hit last (the killer, once dead).
    pub last_by: usize,
    pub dead_t: f32,
    pub bot: Option<Bot>,
    trigger_was: bool,
}

impl Soldier {
    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }
    pub fn eye(&self) -> Vec3 {
        Vec3::new(self.pos.x, EYE, self.pos.y)
    }
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos())
    }
    pub fn spec(&self) -> GunSpec {
        gun(GUNS[self.gun])
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// x: strafe right, y: forward.
    pub mv: Vec2,
    /// Turn: x right, y up (radians).
    pub look: Vec2,
    pub fire: bool,
    pub aim: bool,
    pub reload: bool,
    pub switch: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Shot { shooter: usize, from: Vec3, to: Vec3 },
    Impact { at: Vec3 },
    Hit { victim: usize, by: usize, head: bool, at: Vec3 },
    Kill { killer: usize, victim: usize, gun: Gun, head: bool },
}

pub struct Match {
    pub map: Map,
    pub men: Vec<Soldier>,
    pub score: [u32; 2],
    pub time: f32,
    pub over: Option<u8>,
    pub events: Vec<Event>,
    rng: u32,
    pub skill: f32,
    /// Countdown before the match starts: everyone stands at the spawn and can only look.
    pub warmup: f32,
}

pub const WARMUP: f32 = 5.0;

const NAMES: [&str; 10] = ["Ты", "Ястреб", "Гром", "Сокол", "Буря", "Кобра", "Шторм", "Призрак", "Клык", "Вихрь"];

impl Match {
    pub fn new(seed: u32, skill: f32) -> Self {
        let map = Map::warehouse();
        let mut men = Vec::new();
        for k in 0..10usize {
            let team = (k / 5) as u8;
            let gun_k = match k % 5 {
                3 => 1,
                4 => 2,
                _ => 0,
            };
            let mut s = Soldier {
                name: NAMES[k].to_string(),
                team,
                pos: map.spawns[team as usize][k % 5],
                yaw: if team == 0 { 0.0 } else { std::f32::consts::PI },
                pitch: 0.0,
                hp: 100.0,
                respawn: 0.0,
                protect: 1.5,
                gun: if k == 0 { 0 } else { gun_k },
                mag: [0; 4],
                reserve: [0; 4],
                reload: 0.0,
                cooldown: 0.0,
                recoil: 0.0,
                kills: 0,
                deaths: 0,
                walk: 0.0,
                moving: 0.0,
                aiming: false,
                fired: 0.0,
                hurt: 0.0,
                hurt_from: Vec2::ZERO,
                last_by: 0,
                dead_t: 0.0,
                bot: (k != 0).then(|| Bot { path: Vec::new(), repath: 0.0, target: None, seen: 0.0, strafe: 1.0, skill }),
                trigger_was: false,
            };
            refill(&mut s);
            men.push(s);
        }
        Self { map, men, score: [0, 0], time: MATCH_TIME, over: None, events: Vec::new(), rng: seed | 1, skill, warmup: WARMUP }
    }

    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x % 100_000) as f32 / 100_000.0
    }

    pub fn update(&mut self, dt: f32, player: Input) {
        self.events.clear();
        if self.over.is_some() {
            return;
        }
        if self.warmup > 0.0 {
            self.warmup -= dt;
            let me = &mut self.men[0];
            me.yaw -= player.look.x;
            me.pitch = (me.pitch + player.look.y).clamp(-1.3, 1.3);
            return;
        }
        self.time -= dt;
        for i in 0..self.men.len() {
            let input = if self.men[i].bot.is_some() { self.think(i, dt) } else { player };
            self.step(i, dt, input);
        }
        if self.score[0] >= KILLS_TO_WIN || self.score[1] >= KILLS_TO_WIN || self.time <= 0.0 {
            self.time = self.time.max(0.0);
            self.over = Some(if self.score[0] > self.score[1] {
                0
            } else if self.score[1] > self.score[0] {
                1
            } else {
                2
            });
        }
    }

    fn step(&mut self, i: usize, dt: f32, inp: Input) {
        let map = &self.map;
        let s = &mut self.men[i];
        s.fired = (s.fired - dt).max(0.0);
        s.hurt = (s.hurt - dt * 1.5).max(0.0);
        s.recoil = (s.recoil - dt * 3.0 * (s.recoil + 0.02)).max(0.0);
        if !s.alive() {
            s.dead_t += dt;
            s.respawn -= dt;
            if s.respawn <= 0.0 {
                let spot = (i + s.deaths as usize) % 5;
                s.pos = map.spawns[s.team as usize][spot];
                s.yaw = if s.team == 0 { 0.0 } else { std::f32::consts::PI };
                s.pitch = 0.0;
                s.hp = 100.0;
                s.protect = 1.5;
                s.dead_t = 0.0;
                refill(s);
                if let Some(b) = &mut s.bot {
                    b.path.clear();
                    b.target = None;
                }
            }
            return;
        }
        s.protect = (s.protect - dt).max(0.0);
        s.yaw -= inp.look.x;
        s.pitch = (s.pitch + inp.look.y).clamp(-1.3, 1.3);
        if let Some(k) = inp.switch {
            if k != s.gun && k < 4 {
                s.gun = k;
                s.reload = 0.0;
                s.cooldown = 0.35;
            }
        }
        let spec = s.spec();
        s.aiming = inp.aim && s.reload <= 0.0;
        // Moving.
        let f = Vec2::new(s.yaw.sin(), s.yaw.cos());
        let r = Vec2::new(-s.yaw.cos(), s.yaw.sin());
        let mv = inp.mv.clamp_length_max(1.0);
        let speed = 5.0 * spec.speed * if s.aiming { 0.55 } else { 1.0 };
        let want = s.pos + (f * mv.y + r * mv.x) * speed * dt;
        s.pos = map.slide(s.pos, want, RADIUS);
        let m = mv.length();
        s.walk += dt * 8.0 * m;
        s.moving = if m > 0.1 { (s.moving + dt * 6.0).min(1.0) } else { (s.moving - dt * 6.0).max(0.0) };
        // Reload.
        if s.reload > 0.0 {
            s.reload -= dt;
            if s.reload <= 0.0 {
                let g = s.gun;
                let need = spec.mag - s.mag[g];
                let take = need.min(s.reserve[g]);
                s.mag[g] += take;
                s.reserve[g] -= take;
            }
        } else if (inp.reload || s.mag[s.gun] == 0) && s.mag[s.gun] < spec.mag && s.reserve[s.gun] > 0 {
            s.reload = spec.reload;
        }
        s.cooldown -= dt;
        let pull = inp.fire && (spec.auto || !s.trigger_was);
        s.trigger_was = inp.fire;
        if pull && s.cooldown <= 0.0 && s.reload <= 0.0 && s.mag[s.gun] > 0 {
            s.cooldown = spec.interval;
            s.mag[s.gun] -= 1;
            s.fired = 0.07;
            s.protect = 0.0;
            let spread = (if s.aiming { spec.aim_spread } else { spec.spread }) * (1.0 + s.moving * 0.8) + s.recoil * 0.6;
            s.recoil = (s.recoil + spec.kick).min(0.15);
            s.pitch = (s.pitch + spec.kick * 0.35).min(1.3);
            self.fire(i, spread, spec);
        }
    }

    fn fire(&mut self, i: usize, spread: f32, spec: GunSpec) {
        let eye = self.men[i].eye();
        let fwd = self.men[i].forward();
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(fwd);
        let team = self.men[i].team;
        for _ in 0..spec.pellets {
            let (a, b) = (self.rand() * std::f32::consts::TAU, self.rand().sqrt() * spread);
            let d = (fwd + right * a.cos() * b + up * a.sin() * b).normalize();
            let wall = self.map.ray(eye, d, spec.range).unwrap_or(spec.range);
            let mut best: Option<(usize, f32, bool)> = None;
            for (k, o) in self.men.iter().enumerate() {
                if o.team == team || !o.alive() {
                    continue;
                }
                if let Some((t, head)) = ray_soldier(eye, d, o.pos) {
                    if t < wall && best.is_none_or(|b| t < b.1) {
                        best = Some((k, t, head));
                    }
                }
            }
            let end = eye + d * best.map(|b| b.1).unwrap_or(wall);
            self.events.push(Event::Shot { shooter: i, from: eye, to: end });
            match best {
                Some((k, t, head)) => {
                    let falloff = if t > spec.range * 0.6 { 0.7 } else { 1.0 };
                    let dmg = spec.damage * falloff * if head { 2.0 } else { 1.0 };
                    self.damage(k, i, dmg, head, end);
                }
                None if wall < spec.range => self.events.push(Event::Impact { at: end }),
                None => {}
            }
        }
    }

    fn damage(&mut self, victim: usize, by: usize, dmg: f32, head: bool, at: Vec3) {
        let from = self.men[by].pos;
        let g = GUNS[self.men[by].gun];
        let v = &mut self.men[victim];
        if !v.alive() || v.protect > 0.0 {
            return;
        }
        v.hp -= dmg;
        v.hurt = 1.0;
        v.hurt_from = from;
        v.last_by = by;
        // A bot that gets shot turns to the shooter.
        if let Some(b) = &mut v.bot {
            b.target = Some(by);
            b.seen = b.seen.max(0.2);
        }
        self.events.push(Event::Hit { victim, by, head, at });
        if v.hp <= 0.0 {
            v.hp = 0.0;
            v.deaths += 1;
            v.respawn = 3.0;
            v.dead_t = 0.0;
            v.reload = 0.0;
            let team = self.men[by].team as usize;
            self.men[by].kills += 1;
            self.score[team] += 1;
            self.events.push(Event::Kill { killer: by, victim, gun: g, head });
        }
    }

    fn think(&mut self, i: usize, dt: f32) -> Input {
        let mut out = Input::default();
        if !self.men[i].alive() {
            return out;
        }
        let me = self.men[i].clone();
        let Some(mut bot) = me.bot.clone() else { return out };
        let eye = me.eye();
        // Who can I see?
        let mut seen: Option<(usize, f32)> = None;
        for (k, o) in self.men.iter().enumerate() {
            if o.team == me.team || !o.alive() {
                continue;
            }
            let d = (o.pos - me.pos).length();
            if d > 45.0 {
                continue;
            }
            // Only in front (a wide view), unless it is the one who shot me.
            let to = (o.pos - me.pos).normalize_or_zero();
            let f = Vec2::new(me.yaw.sin(), me.yaw.cos());
            if to.dot(f) < -0.2 && bot.target != Some(k) {
                continue;
            }
            if self.map.visible(eye, Vec3::new(o.pos.x, 1.4, o.pos.y)) && seen.is_none_or(|s| d < s.1) {
                seen = Some((k, d));
            }
        }
        match seen {
            Some((k, _)) => {
                if bot.target != Some(k) {
                    bot.target = Some(k);
                    bot.seen = 0.0;
                }
                bot.seen += dt;
            }
            None => {
                bot.target = None;
                bot.seen = 0.0;
            }
        }
        let skill = bot.skill;
        if let (Some(k), true) = (bot.target, seen.is_some()) {
            let o = &self.men[k];
            let aim = Vec3::new(o.pos.x, 1.35 + if skill > 0.7 { 0.25 } else { 0.0 }, o.pos.y) - eye;
            let want_yaw = aim.x.atan2(aim.z);
            let want_pitch = (aim.y / aim.length().max(0.1)).asin();
            let dyaw = wrap(me.yaw - want_yaw);
            let turn = (3.0 + skill * 5.0) * dt;
            out.look.x = dyaw.clamp(-turn, turn);
            out.look.y = (want_pitch - me.pitch).clamp(-turn, turn);
            let react = 0.55 - skill * 0.35;
            let dist = aim.length();
            let good_range = dist < me.spec().range * 0.9;
            if bot.seen > react && dyaw.abs() < 0.12 && good_range {
                out.fire = true;
                out.aim = me.gun == 2 || (dist > 15.0 && self.rand() < 0.5);
            }
            if self.rand() < dt * 0.8 {
                bot.strafe = -bot.strafe;
            }
            out.mv = Vec2::new(bot.strafe * 0.8, if dist > 18.0 { 0.6 } else if dist < 5.0 { -0.4 } else { 0.0 });
            bot.path.clear();
        } else {
            // Walk the map: towards the enemy side, to a random spot.
            bot.repath -= dt;
            if bot.path.is_empty() || bot.repath <= 0.0 {
                bot.repath = 4.0 + self.rand() * 3.0;
                let side = if me.team == 0 { 1.0 } else { -1.0 };
                let goal = Vec2::new(self.rand() * 40.0 - 20.0, side * (self.rand() * 24.0 - 4.0));
                bot.path = self.map.path(me.pos, goal);
            }
            while let Some(p) = bot.path.first().copied() {
                if (p - me.pos).length() < 0.6 {
                    bot.path.remove(0);
                } else {
                    break;
                }
            }
            if let Some(p) = bot.path.first().copied() {
                let to = p - me.pos;
                let want_yaw = to.x.atan2(to.y);
                let dyaw = wrap(me.yaw - want_yaw);
                let turn = 6.0 * dt;
                out.look.x = dyaw.clamp(-turn, turn);
                out.look.y = (-me.pitch).clamp(-turn, turn);
                out.mv = Vec2::new(0.0, if dyaw.abs() < 0.8 { 1.0 } else { 0.3 });
            }
        }
        out.reload = me.mag[me.gun] == 0 || (bot.target.is_none() && me.mag[me.gun] < me.spec().mag / 3);
        self.men[i].bot = Some(bot);
        out
    }
}

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(t) - std::f32::consts::PI
}

fn refill(s: &mut Soldier) {
    for (k, g) in GUNS.iter().enumerate() {
        let sp = gun(*g);
        s.mag[k] = sp.mag;
        s.reserve[k] = sp.reserve;
    }
    s.reload = 0.0;
}

/// A soldier as a vertical cylinder body and a sphere head: distance along the ray and
/// whether it is the head.
pub fn ray_soldier(o: Vec3, d: Vec3, at: Vec2) -> Option<(f32, bool)> {
    let mut best: Option<(f32, bool)> = None;
    let head = Vec3::new(at.x, 1.68, at.y);
    let oc = o - head;
    let b = oc.dot(d);
    let c = oc.length_squared() - 0.2 * 0.2;
    let disc = b * b - c;
    if disc >= 0.0 {
        let t = -b - disc.sqrt();
        if t > 0.0 {
            best = Some((t, true));
        }
    }
    let oc2 = Vec2::new(o.x - at.x, o.z - at.y);
    let dd = Vec2::new(d.x, d.z);
    let a = dd.length_squared();
    if a > 1e-8 {
        let bq = oc2.dot(dd);
        let cq = oc2.length_squared() - 0.34 * 0.34;
        let disc = bq * bq - a * cq;
        if disc >= 0.0 {
            let t = (-bq - disc.sqrt()) / a;
            let y = o.y + d.y * t;
            if t > 0.0 && (0.0..=1.5).contains(&y) && best.is_none_or(|h| t < h.0) {
                best = Some((t, false));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nobody_moves_or_shoots_before_the_start() {
        let mut m = Match::new(4, 0.9);
        let start: Vec<Vec2> = m.men.iter().map(|s| s.pos).collect();
        let mut t = 0.0;
        while t < WARMUP - 0.1 {
            m.update(1.0 / 60.0, Input { mv: Vec2::new(0.0, 1.0), fire: true, ..Default::default() });
            t += 1.0 / 60.0;
        }
        assert!(m.men.iter().zip(&start).all(|(s, p)| s.pos == *p));
        assert_eq!(m.men[0].mag[0], 30);
        assert!(m.warmup > 0.0);
    }

    #[test]
    fn a_shot_hits_a_soldier_in_the_open_and_not_through_a_wall() {
        let d = Vec3::new(0.0, 0.0, 1.0);
        let hit = ray_soldier(Vec3::new(0.0, 1.2, 0.0), d, Vec2::new(0.0, 5.0)).unwrap();
        assert!((hit.0 - 4.66).abs() < 0.01 && !hit.1);
        let head = ray_soldier(Vec3::new(0.0, 1.68, 0.0), d, Vec2::new(0.0, 5.0)).unwrap();
        assert!(head.1);
        assert!(ray_soldier(Vec3::new(1.0, 1.2, 0.0), d, Vec2::new(0.0, 5.0)).is_none());
    }

    #[test]
    fn the_player_kills_a_bot_standing_in_front() {
        let mut m = Match::new(5, 0.5);
        m.warmup = 0.0;
        // Put an enemy 6 m ahead of the player in the open, frozen.
        m.men[5].pos = m.men[0].pos + Vec2::new(0.0, 6.0);
        m.men[5].protect = 0.0;
        m.men[0].protect = 0.0;
        // Everyone else waits dead; the target cannot shoot back.
        for k in (1..5).chain(6..10) {
            m.men[k].hp = 0.0;
            m.men[k].respawn = 99.0;
        }
        m.men[5].bot = None;
        m.men[5].mag = [0; 4];
        m.men[5].reserve = [0; 4];
        let mut t = 0.0;
        while m.men[5].alive() && t < 5.0 {
            m.update(1.0 / 60.0, Input { fire: true, aim: true, ..Default::default() });
            m.men[0].pitch = -0.05;
            t += 1.0 / 60.0;
        }
        assert!(!m.men[5].alive(), "hp left {}", m.men[5].hp);
        assert_eq!(m.men[0].kills, 1);
    }

    #[test]
    fn a_bot_match_ends_with_kills() {
        for skill in [0.3f32, 0.8] {
            let mut m = Match::new(9, skill);
            let mut t = 0.0;
            while m.over.is_none() && t < MATCH_TIME + WARMUP + 1.0 {
                m.update(1.0 / 30.0, Input::default());
                t += 1.0 / 30.0;
            }
            assert!(m.over.is_some());
            let total = m.score[0] + m.score[1];
            assert!(total >= 8, "skill {skill}: bots barely fight: {:?}", m.score);
        }
    }

    #[test]
    fn reloading_refills_the_magazine() {
        let mut m = Match::new(3, 0.5);
        m.warmup = 0.0;
        m.men[0].mag[0] = 0;
        let mut t = 0.0;
        while t < 2.5 {
            m.update(1.0 / 60.0, Input::default());
            t += 1.0 / 60.0;
        }
        assert_eq!(m.men[0].mag[0], 30);
        assert_eq!(m.men[0].reserve[0], 90);
    }
}
