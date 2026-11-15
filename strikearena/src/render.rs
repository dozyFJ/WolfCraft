//! Builds the picture: the warehouse, soldiers, the gun in hand, tracers and sparks.

use crate::gfx::{Scene, Shape, box_model};
use crate::map::{HALF, Map};
use crate::sim::{Gun, GUNS, Match, Soldier};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};

pub struct Tracer {
    pub a: Vec3,
    pub b: Vec3,
    pub life: f32,
}

pub struct Spark {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub color: Vec3,
}

pub fn team_color(team: u8) -> Vec3 {
    if team == 0 { Vec3::new(0.18, 0.36, 0.85) } else { Vec3::new(0.85, 0.2, 0.16) }
}

pub fn sky(s: &mut Scene) {
    s.sky_top = Vec3::new(0.22, 0.42, 0.78);
    s.sky_horizon = Vec3::new(0.78, 0.84, 0.9);
    s.sun_dir = Vec3::new(0.45, 0.75, 0.35);
    s.sun = 1.15;
    s.sun_color = Vec3::new(1.0, 0.95, 0.85);
    s.fog = 0.006;
    s.eclipse = false;
}

pub fn map(s: &mut Scene, m: &Map, t: f32) {
    s.add_floor(box_model(Vec3::new(-HALF, -0.3, -HALF), Vec3::new(HALF, 0.0, HALF)), Vec3::new(0.55, 0.55, 0.53));
    // Ground outside.
    s.add(Shape::Cube, box_model(Vec3::new(-200.0, -0.6, -200.0), Vec3::new(200.0, -0.3, 200.0)), Vec3::new(0.35, 0.42, 0.28), Vec3::ZERO, 0.0, 0.0);
    for b in &m.boxes {
        s.add(Shape::Cube, box_model(b.min, b.max), b.color, Vec3::ONE, 0.15, 0.0);
        // A darker band at the foot, a light cap on top: walls read better.
        let foot = Vec3::new(b.max.x, b.min.y + 0.12, b.max.z);
        s.add(Shape::Cube, box_model(b.min - Vec3::new(0.01, 0.0, 0.01), foot + Vec3::new(0.01, 0.0, 0.01)), b.color * 0.6, Vec3::ZERO, 0.0, 0.0);
    }
    for p in &m.pillars {
        s.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(p.r, p.h, p.r), Quat::IDENTITY, Vec3::new(p.at.x, 0.0, p.at.y)), Vec3::new(0.7, 0.68, 0.64), Vec3::ONE, 0.15, 0.0);
    }
    // Lamps on the outer walls.
    for k in 0..12 {
        let side = k % 4;
        let u = (k / 4) as f32 * 16.0 - 16.0;
        let at = match side {
            0 => Vec3::new(u, 4.2, -HALF + 0.2),
            1 => Vec3::new(u, 4.2, HALF - 0.2),
            2 => Vec3::new(-HALF + 0.2, 4.2, u),
            _ => Vec3::new(HALF - 0.2, 4.2, u),
        };
        let flick = 1.0 + 0.04 * (t * 9.0 + k as f32).sin();
        s.sphere(at, Vec3::new(0.25, 0.12, 0.25), Vec3::new(1.0, 0.92, 0.7), Vec3::ZERO, 0.0, 2.0 * flick);
    }
    // City blocks far behind the walls.
    for k in 0..24 {
        let a = k as f32 / 24.0 * std::f32::consts::TAU;
        let r = 70.0 + ((k * 37) % 7) as f32 * 6.0;
        let h = 12.0 + ((k * 53) % 9) as f32 * 4.0;
        let c = Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        let w = 8.0 + ((k * 11) % 5) as f32 * 2.0;
        s.add(Shape::Cube, box_model(c - Vec3::new(w, 0.5, w), c + Vec3::new(w, h, w)), Vec3::new(0.5, 0.55, 0.62), Vec3::ZERO, 0.0, 0.0);
    }
}

struct Frame {
    o: Vec3,
    r: Vec3,
    f: Vec3,
    fall: f32,
}

impl Frame {
    fn p(&self, x: f32, y: f32, z: f32) -> Vec3 {
        let (s, c) = self.fall.sin_cos();
        let (y2, z2) = (y * c + z * s, z * c - y * s);
        self.o + self.r * x + Vec3::Y * y2 + self.f * z2
    }
}

fn limb(s: &mut Scene, a: Vec3, b: Vec3, r: f32, color: Vec3) {
    s.limb(a, b, r, color, Vec3::ZERO, 0.0);
}

pub fn soldier(s: &mut Scene, m: &Soldier, t: f32) {
    let f = Vec3::new(m.yaw.sin(), 0.0, m.yaw.cos());
    let r = f.cross(Vec3::Y);
    let fall = if m.alive() { 0.0 } else { -(m.dead_t * 3.0).min(1.0) * 1.5 };
    let fr = Frame { o: Vec3::new(m.pos.x, 0.0, m.pos.y), r, f, fall };
    let team = team_color(m.team);
    let flash = if m.hurt > 0.7 { 0.6 } else { 0.0 };
    let uni = team * 0.75 + Vec3::splat(flash);
    let dark = Vec3::new(0.16, 0.17, 0.15);
    let skin = Vec3::new(0.78, 0.58, 0.45);
    let bob = (m.walk * 2.0).sin().abs() * 0.04 * m.moving;
    let hip = 0.95 - bob;
    let rot = Quat::from_rotation_y(m.yaw) * Quat::from_rotation_x(fall);
    // Torso and vest.
    limb(s, fr.p(0.0, hip, 0.0), fr.p(0.0, 1.44 - bob, 0.0), 0.16, uni);
    s.add(Shape::Cube, Mat4::from_scale_rotation_translation(Vec3::new(0.42, 0.42, 0.28), rot, fr.p(0.0, 1.22 - bob, 0.0)), team * 0.45, team, 0.25, 0.0);
    // Head and helmet.
    s.sphere(fr.p(0.0, 1.64 - bob, 0.02), Vec3::splat(0.13), skin, Vec3::ZERO, 0.0, 0.0);
    s.sphere(fr.p(0.0, 1.71 - bob, 0.0), Vec3::new(0.155, 0.1, 0.16), team * 0.55, Vec3::ZERO, 0.0, 0.0);
    s.add(Shape::Cube, Mat4::from_scale_rotation_translation(Vec3::new(0.2, 0.05, 0.02), rot, fr.p(0.0, 1.66 - bob, 0.13)), Vec3::splat(0.05), Vec3::ZERO, 0.0, 0.0);
    // The gun, pointing where they look.
    let p = if m.alive() { m.pitch } else { 0.0 };
    let gdir = (f * p.cos() + Vec3::Y * p.sin()).normalize();
    let grip = fr.p(0.12, 1.3 - bob, 0.28);
    let gun_rot = Quat::from_rotation_y(m.yaw) * Quat::from_rotation_x(-p + fall);
    s.add(Shape::Cube, Mat4::from_scale_rotation_translation(Vec3::new(0.07, 0.12, 0.6), gun_rot, grip + gdir * 0.15), dark, Vec3::ZERO, 0.0, 0.0);
    let muzzle = grip + gdir * 0.62;
    s.rod(grip + gdir * 0.4, muzzle, 0.025, dark * 0.7, 0.0);
    if m.fired > 0.0 && m.alive() {
        s.sphere(muzzle, Vec3::splat(0.09), Vec3::new(1.0, 0.8, 0.35), Vec3::ZERO, 0.0, 4.0);
    }
    // Arms to the gun.
    for (side, hand) in [(1.0f32, grip), (-1.0, grip + gdir * 0.32)] {
        let sh = fr.p(0.22 * side, 1.42 - bob, 0.0);
        let elbow = (sh + hand) * 0.5 - Vec3::Y * 0.1 + r * (0.08 * side);
        limb(s, sh, elbow, 0.055, uni);
        limb(s, elbow, hand, 0.05, uni);
        s.sphere(hand, Vec3::splat(0.055), Vec3::new(0.12, 0.12, 0.1), Vec3::ZERO, 0.0, 0.0);
    }
    // Legs.
    for side in [-1.0f32, 1.0] {
        let ph = m.walk + if side > 0.0 { std::f32::consts::PI } else { 0.0 };
        let swing = ph.sin() * 0.25 * m.moving;
        let lift = ph.cos().max(0.0) * 0.12 * m.moving;
        let hp = fr.p(0.11 * side, hip, 0.0);
        let knee = fr.p(0.12 * side, 0.5 + lift * 0.5, swing * 0.5 + 0.06);
        let foot = fr.p(0.13 * side, 0.08 + lift, swing);
        limb(s, hp, knee, 0.08, dark * 1.4);
        limb(s, knee, foot, 0.07, dark * 1.4);
        s.sphere(foot + f * 0.06, Vec3::new(0.08, 0.06, 0.13), dark, Vec3::ZERO, 0.0, 0.0);
    }
    let _ = t;
}

pub struct Cam {
    pub eye: Vec3,
    pub fwd: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

pub fn camera(m: &Soldier, shake: f32, t: f32) -> Cam {
    let mut eye = m.eye();
    let mut fwd = m.forward();
    if m.alive() {
        eye.y += (m.walk * 2.0).sin().abs() * -0.035 * m.moving;
        eye += fwd.cross(Vec3::Y).normalize_or_zero() * (m.walk.sin() * 0.02 * m.moving);
    } else {
        // Dead: the view sinks to the floor and tips over.
        let k = (m.dead_t * 2.0).min(1.0);
        eye.y = 1.6 - 1.2 * k;
        fwd = (fwd + Vec3::Y * k * 0.8).normalize();
    }
    if shake > 0.0 {
        eye += Vec3::new((t * 57.0).sin(), (t * 43.0).cos(), (t * 61.0).sin()) * shake * shake * 0.05;
    }
    let right = fwd.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(fwd);
    Cam { eye, fwd, right, up }
}

/// The gun in the player's hands.
pub fn viewmodel(s: &mut Scene, m: &Soldier, c: &Cam, t: f32) {
    if !m.alive() {
        return;
    }
    let g = GUNS[m.gun];
    let aim = m.aiming;
    if aim && g == Gun::Sniper {
        return; // the scope covers the screen
    }
    let rot = Quat::from_mat3(&Mat3::from_cols(c.up.cross(c.fwd), c.up, c.fwd));
    // Reload, in stages: tilt the gun, the old magazine drops, the left hand brings a new one
    // from below, pushes it in, and the gun is racked back into place.
    let rk = if m.reload > 0.0 { (1.0 - m.reload / m.spec().reload).clamp(0.0, 1.0) } else { 1.0 };
    let reloading = m.reload > 0.0;
    let tilt_k = if reloading { (rk * std::f32::consts::PI).sin().powf(0.6) } else { 0.0 };
    let mag_drop = if !reloading {
        0.0
    } else if rk < 0.3 {
        (rk / 0.3).powi(2) * 0.45
    } else if rk < 0.55 {
        (1.0 - (rk - 0.3) / 0.25) * 0.45
    } else {
        0.0
    };
    let rack = if reloading && rk > 0.78 { ((rk - 0.78) / 0.22 * std::f32::consts::PI).sin() * 0.045 } else { 0.0 };
    let reload = tilt_k * 0.35;
    let kick = m.fired * 1.6 + m.recoil * 0.6;
    let sway = Vec2::new((m.walk).sin() * 0.012 * m.moving, (m.walk * 2.0).sin().abs() * 0.01 * m.moving + (t * 1.8).sin() * 0.004);
    let base = if aim { Vec3::new(0.0, -0.105, 0.32) } else { Vec3::new(0.17, -0.19, 0.38) };
    let o = base + Vec3::new(sway.x - tilt_k * 0.04, -sway.y - reload * 0.18, -kick * 0.25 - rack);
    let at = |x: f32, y: f32, z: f32| c.eye + c.right * x + c.up * y + c.fwd * z;
    let body = Vec3::new(0.13, 0.13, 0.12);
    let metal = Vec3::new(0.2, 0.21, 0.22);
    let wood = Vec3::new(0.45, 0.28, 0.14);
    let (len, col, barrel) = match g {
        Gun::Rifle => (0.42, body, 0.22),
        Gun::Shotgun => (0.46, wood, 0.26),
        Gun::Sniper => (0.55, Vec3::new(0.22, 0.26, 0.18), 0.32),
        Gun::Pistol => (0.16, metal, 0.06),
    };
    let tilt = Quat::from_axis_angle(c.right, -kick * 0.6 - reload * 0.5) * Quat::from_axis_angle(c.fwd, tilt_k * 0.55);
    let r2 = tilt * rot;
    let center = at(o.x, o.y, o.z + len * 0.3);
    s.add(Shape::Cube, Mat4::from_scale_rotation_translation(Vec3::new(0.06, 0.09, len), r2, center), col, Vec3::ZERO, 0.0, 0.0);
    let fwd2 = tilt * c.fwd;
    let front = center + fwd2 * (len * 0.5);
    s.rod(front, front + fwd2 * barrel, 0.018, metal, 0.0);
    // Magazine and grip.
    let mag_home = center - tilt * c.up * 0.09 + fwd2 * 0.05;
    let mag_at = mag_home - c.up * mag_drop;
    s.add(Shape::Cube, Mat4::from_scale_rotation_translation(Vec3::new(0.045, 0.12, 0.06), r2, mag_at), metal, Vec3::ZERO, 0.0, 0.0);
    if g == Gun::Sniper {
        let top = center + tilt * c.up * 0.075;
        s.rod(top - fwd2 * 0.12, top + fwd2 * 0.16, 0.028, Vec3::splat(0.08), 0.0);
    }
    if m.fired > 0.0 {
        let muzzle = front + fwd2 * (barrel + 0.04);
        s.sphere(muzzle, Vec3::new(0.06, 0.06, 0.09), Vec3::new(1.0, 0.82, 0.4), Vec3::ZERO, 0.0, 5.0);
    }
    // Hands and sleeves.
    let sleeve = crate::render::team_color(m.team) * 0.7;
    let skin = Vec3::new(0.78, 0.58, 0.45);
    let grip = center - tilt * c.up * 0.06 - fwd2 * (len * 0.25);
    // While reloading the left hand holds the magazine.
    let fore = if reloading && rk < 0.75 { mag_at - c.up * 0.06 } else { center + fwd2 * (len * 0.3) - tilt * c.up * 0.05 };
    for (hand, sh) in [(grip, at(0.3, -0.5, 0.0)), (fore, at(-0.25, -0.5, 0.1))] {
        s.limb(sh, hand, 0.045, sleeve, Vec3::ZERO, 0.0);
        s.sphere(hand, Vec3::splat(0.04), skin, Vec3::ZERO, 0.0, 0.0);
    }
}

pub fn effects(s: &mut Scene, tracers: &mut Vec<Tracer>, sparks: &mut Vec<Spark>, dt: f32) {
    for tr in tracers.iter_mut() {
        tr.life -= dt;
    }
    tracers.retain(|t| t.life > 0.0);
    for tr in tracers.iter() {
        s.rod(tr.a, tr.b, 0.012, Vec3::new(1.0, 0.85, 0.45), 4.0);
    }
    for p in sparks.iter_mut() {
        p.life -= dt;
        p.vel.y -= 9.0 * dt;
        p.pos += p.vel * dt;
    }
    sparks.retain(|p| p.life > 0.0);
    for p in sparks.iter() {
        s.sphere(p.pos, Vec3::splat(0.02 + p.life * 0.05), p.color, Vec3::ZERO, 0.0, 2.5);
    }
}

pub fn everyone(s: &mut Scene, m: &Match, me: usize, t: f32) {
    for (k, o) in m.men.iter().enumerate() {
        if k != me && (o.alive() || o.dead_t < 2.8) {
            soldier(s, o, t);
        }
    }
}
