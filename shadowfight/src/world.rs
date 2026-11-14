//! Builds the 3D scene: the arena of the chapter, the shadow fighter, the player's arms.

use crate::fight::{Duel, Fighter, Move, State, spec};
use crate::gfx::{Scene, Shape};
use crate::story::Theme;
use glam::{Mat4, Quat, Vec2, Vec3};

pub struct Spark {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub color: Vec3,
}

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

pub fn apply_theme(s: &mut Scene, th: &Theme) {
    s.sky_top = v3(th.sky_top);
    s.sky_horizon = v3(th.sky_horizon);
    s.sun_dir = v3(th.sun_dir);
    s.sun = th.sun;
    s.sun_color = v3(th.sun_color);
    s.fog = th.fog;
    s.eclipse = th.eclipse;
}

pub fn arena(s: &mut Scene, th: &Theme, t: f32) {
    let stone = v3(th.stone);
    let torch = v3(th.torch);
    // Floor and the step around it.
    s.add_floor(Mat4::from_scale_rotation_translation(Vec3::new(9.4, 0.3, 9.4), Quat::IDENTITY, Vec3::new(0.0, -0.3, 0.0)), v3(th.floor));
    s.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(10.6, 0.45, 10.6), Quat::IDENTITY, Vec3::new(0.0, -0.75, 0.0)), stone * 0.7, Vec3::ZERO, 0.0, 0.0);
    // Ground far away.
    s.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(120.0, 0.1, 120.0), Quat::IDENTITY, Vec3::new(0.0, -1.4, 0.0)), v3(th.floor) * 0.45, Vec3::ZERO, 0.0, 0.0);
    // Pillars with torches.
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU + 0.2;
        let p = Vec3::new(a.cos() * 11.5, -0.5, a.sin() * 11.5);
        s.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(0.55, 4.6, 0.55), Quat::IDENTITY, p), stone, Vec3::ZERO, 0.0, 0.0);
        s.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(0.75, 0.3, 0.75), Quat::IDENTITY, p + Vec3::Y * 4.1), stone * 0.85, Vec3::ZERO, 0.0, 0.0);
        let flick = 1.0 + 0.15 * (t * 11.0 + k as f32 * 1.7).sin() + 0.1 * (t * 23.0 + k as f32).sin();
        let fire = p + Vec3::Y * 4.65;
        s.sphere(fire, Vec3::new(0.22, 0.32 * flick, 0.22), torch, torch, 0.0, 2.5 * flick);
        s.sphere(fire + Vec3::Y * 0.22, Vec3::new(0.11, 0.2 * flick, 0.11), Vec3::new(1.0, 0.95, 0.8), Vec3::ZERO, 0.0, 3.0);
    }
    // A gate arch behind the shadow's start.
    for x in [-2.2f32, 2.2] {
        s.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(0.5, 6.0, 0.5), Quat::IDENTITY, Vec3::new(x, -0.5, 14.5)), stone * 0.8, Vec3::ZERO, 0.0, 0.0);
    }
    s.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(0.45, 5.6, 0.45), Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::new(2.8, 5.6, 14.5)), stone * 0.8, Vec3::ZERO, 0.0, 0.0);
    s.sphere(Vec3::new(0.0, 2.6, 14.6), Vec3::new(1.7, 2.6, 0.15), Vec3::ZERO, v3(th.aura), 1.0, 0.0);
    // Mountains on the horizon.
    for k in 0..14 {
        let a = k as f32 / 14.0 * std::f32::consts::TAU;
        let h = 18.0 + ((k * 7919) % 13) as f32 * 2.5;
        let r = 70.0 + ((k * 31) % 5) as f32 * 6.0;
        s.add(Shape::Cone, Mat4::from_scale_rotation_translation(Vec3::new(22.0, h, 22.0), Quat::IDENTITY, Vec3::new(a.cos() * r, -1.5, a.sin() * r)), stone * 0.55, Vec3::ZERO, 0.0, 0.0);
    }
    // Ash and embers floating in the air.
    for k in 0..40u32 {
        let h = k.wrapping_mul(2654435761);
        let x = (h % 1000) as f32 / 1000.0 * 18.0 - 9.0;
        let z = ((h >> 10) % 1000) as f32 / 1000.0 * 18.0 - 9.0;
        let sp = 0.3 + ((h >> 20) % 100) as f32 / 200.0;
        let y = (t * sp + (h % 97) as f32 / 9.7).rem_euclid(6.0);
        let p = Vec3::new(x + (t * 0.7 + k as f32).sin() * 0.4, y, z);
        s.sphere(p, Vec3::splat(0.025), torch, Vec3::ZERO, 0.0, 2.0);
    }
}

/// A point in a fighter's own frame (x right, y up, z towards the foe) in world space.
struct Frame {
    o: Vec3,
    r: Vec3,
    f: Vec3,
    /// Falling back (KO), radians.
    fall: f32,
    lean: f32,
}

impl Frame {
    fn p(&self, x: f32, y: f32, z: f32) -> Vec3 {
        let (s, c) = (self.fall + self.lean).sin_cos();
        let (y2, z2) = (y * c + z * s, z * c - y * s);
        self.o + self.r * x + Vec3::Y * y2 + self.f * z2
    }
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Strike progress: 0 in guard → 1 fully out, then back.
fn strike(m: Move, t: f32) -> f32 {
    let s = spec(m);
    if t < s.windup {
        -0.25 * ease(t / s.windup)
    } else if t < s.windup + s.active {
        ease((t - s.windup) / s.active)
    } else {
        1.0 - ease((t - s.windup - s.active) / s.recover)
    }
}

pub fn shadow(s: &mut Scene, f: &Fighter, foe: Vec2, aura: Vec3, t: f32) {
    let fwd2 = (foe - f.pos).normalize_or_zero();
    let fwd = Vec3::new(fwd2.x, 0.0, fwd2.y);
    let right = Vec3::new(fwd2.y, 0.0, -fwd2.x);
    let (fall, lean) = match f.state {
        State::Ko { t } => (-(t * 2.5).min(1.0) * 1.45, 0.0),
        State::Stagger { t } => (0.0, -t * 0.6),
        State::Dodge { .. } => (0.0, -0.15),
        _ => (0.0, 0.04),
    };
    let ko_drop = if let State::Ko { t } = f.state { -(t * 2.5).min(1.0) * 0.1 } else { 0.0 };
    let fr = Frame { o: Vec3::new(f.pos.x, ko_drop, f.pos.y), r: right, f: fwd, fall, lean };
    let body = Vec3::new(0.025, 0.022, 0.035);
    let flash = f.flash;
    let col = body + Vec3::new(0.6, 0.6, 0.6) * flash;
    let rim_k = 1.6 + flash * 2.0;
    let breathe = (t * 3.2).sin() * 0.015;
    let bob = (f.walk * 2.0).sin().abs() * 0.04 * f.moving;
    let hip_y = 0.95 + breathe - bob;
    // Arms: hand targets per state.
    let (mut lh, mut rh) = (Vec3::new(-0.16, 1.48, 0.32), Vec3::new(0.14, 1.42, 0.38));
    let mut glow = 0.0f32;
    let mut leg_kick = 0.0f32;
    match f.state {
        State::Attack { m, t: at, .. } => {
            let k = strike(m, at);
            match m {
                Move::Jab => rh = rh.lerp(Vec3::new(0.05, 1.45, 0.85), k.max(0.0)) + Vec3::new(0.0, 0.0, k.min(0.0)),
                Move::Hook => {
                    let a = k.max(-0.25) * 2.2;
                    rh = Vec3::new(0.45 * a.cos().max(-0.2) + 0.05, 1.45, 0.3 + 0.45 * a.sin().max(0.0)) + Vec3::new(0.0, 0.0, k.min(0.0) * 0.4);
                }
                Move::Kick => leg_kick = k,
                Move::Shadow => {
                    lh = lh.lerp(Vec3::new(-0.1, 1.4, 0.9), k.max(0.0));
                    rh = rh.lerp(Vec3::new(0.1, 1.4, 0.9), k.max(0.0));
                    glow = 1.0;
                }
            }
            if let Some((_, w)) = f.windup_of() {
                glow = glow.max(w);
            }
        }
        State::Block => {
            lh = Vec3::new(-0.06, 1.6, 0.28);
            rh = Vec3::new(0.06, 1.52, 0.3);
        }
        _ => {}
    }
    let ch = |v: Vec3| v + Vec3::new(0.0, breathe - bob, 0.0);
    lh = ch(lh);
    rh = ch(rh);
    let neck = fr.p(0.0, 1.5 + breathe - bob, 0.02);
    let pelvis = fr.p(0.0, hip_y, 0.0);
    // Torso: a tapered stack of spheres.
    s.limb(pelvis, neck, 0.16, col, aura, rim_k);
    s.add(Shape::Sphere, Mat4::from_scale_rotation_translation(Vec3::new(0.25, 0.2, 0.15), Quat::from_rotation_arc(Vec3::Z, fwd), fr.p(0.0, 1.33 + breathe - bob, 0.0)), col, aura, rim_k, 0.0);
    let head = fr.p(0.0, 1.66 + breathe - bob, 0.03);
    s.sphere(head, Vec3::new(0.12, 0.14, 0.13), col, aura, rim_k, 0.0);
    // Eyes burn brighter on the wind-up: that is the tell.
    let eye = aura * (1.2 + glow * 3.0);
    for x in [-0.045f32, 0.045] {
        s.sphere(fr.p(x, 1.68 + breathe - bob, 0.14), Vec3::new(0.022, 0.012, 0.01), eye, Vec3::ZERO, 0.0, 2.5 + glow * 4.0);
    }
    for (side, hand) in [(-1.0f32, lh), (1.0, rh)] {
        let sh = fr.p(0.22 * side, 1.44 + breathe - bob, 0.0);
        let hw = fr.p(hand.x, hand.y, hand.z);
        let mid = (sh + hw) * 0.5;
        let reach = (hw - sh).length();
        let bend = (0.56 - reach).max(0.0) * 0.9;
        let elbow = mid + right * (0.1 * side) - Vec3::Y * (0.08 + bend);
        s.limb(sh, elbow, 0.055, col, aura, rim_k);
        s.limb(elbow, hw, 0.05, col, aura, rim_k);
        s.sphere(hw, Vec3::splat(0.07), col + aura * glow * 0.6, aura, rim_k, glow * 1.5);
    }
    // Legs: a fighting stance, walking swing, the right leg kicks.
    for side in [-1.0f32, 1.0] {
        let hip = fr.p(0.12 * side, hip_y, 0.0);
        let phase = f.walk + if side > 0.0 { std::f32::consts::PI } else { 0.0 };
        let swing = phase.sin() * 0.22 * f.moving;
        let lift = phase.cos().max(0.0) * 0.12 * f.moving;
        let stance = if side > 0.0 { -0.12 } else { 0.14 };
        let (foot, knee) = if side > 0.0 && leg_kick != 0.0 {
            let k = leg_kick.max(0.0);
            let foot = fr.p(0.1, 0.05 + k * 1.05, stance + k * 0.95 - leg_kick.min(0.0) * 0.4);
            let knee = fr.p(0.12, 0.55 + k * 0.5, 0.2 + k * 0.35);
            (foot, knee)
        } else {
            let foot = fr.p(0.16 * side, 0.06 + lift, stance + swing);
            let knee = fr.p(0.14 * side, 0.5 + lift * 0.5, stance * 0.5 + swing * 0.5 + 0.1);
            (foot, knee)
        };
        s.limb(hip, knee, 0.075, col, aura, rim_k);
        s.limb(knee, foot, 0.065, col, aura, rim_k);
        s.sphere(foot + fwd * 0.06, Vec3::new(0.07, 0.05, 0.12), col, aura, rim_k, 0.0);
    }
    // A dark puddle of shadow under the feet.
    let base = Vec3::new(f.pos.x, 0.005, f.pos.y);
    s.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(0.6, 0.01, 0.6), Quat::IDENTITY, base), Vec3::splat(0.0), aura, 0.3, 0.0);
}

/// The camera: the player's eyes, always facing the foe.
pub struct Cam {
    pub eye: Vec3,
    pub fwd: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

pub fn camera(d: &Duel, shake: f32, t: f32) -> Cam {
    let p = &d.player;
    let foe = d.enemy.pos;
    let fwd2 = (foe - p.pos).normalize_or_zero();
    let bob = (p.walk * 2.0).sin() * 0.03 * p.moving;
    let sway = (p.walk).sin() * 0.02 * p.moving;
    let mut eye = Vec3::new(p.pos.x, 1.62 + bob, p.pos.y);
    let mut look = Vec3::new(foe.x, 1.45, foe.y);
    match p.state {
        State::Dodge { t: dt, .. } => eye.y -= 0.18 * (1.0 - dt / 0.38),
        State::Block => eye.y -= 0.05,
        State::Ko { t: kt } => {
            eye.y = (1.62 - kt * 2.0).max(0.25);
            look.y = 3.0 + kt;
        }
        State::Stagger { t: st } => look.y += st * 0.6,
        _ => {}
    }
    let right3 = Vec3::new(fwd2.y, 0.0, -fwd2.x);
    eye += right3 * sway;
    if shake > 0.0 {
        let k = shake * shake * 0.12;
        eye += Vec3::new((t * 61.0).sin(), (t * 47.0).cos(), (t * 53.0).sin()) * k;
    }
    let fwd = (look - eye).normalize();
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd);
    Cam { eye, fwd, right, up }
}

/// First-person arms (and the kicking leg) in front of the camera.
pub fn arms(s: &mut Scene, d: &Duel, cam: &Cam, t: f32) {
    let p = &d.player;
    let at = |x: f32, y: f32, z: f32| cam.eye + cam.right * x + cam.up * y + cam.fwd * z;
    let skin = Vec3::new(0.55, 0.38, 0.28);
    let wrap = Vec3::new(0.85, 0.82, 0.75);
    let sleeve = Vec3::new(0.12, 0.1, 0.14);
    let breathe = (t * 2.6).sin() * 0.008;
    let bob = (p.walk * 2.0).sin() * 0.012 * p.moving;
    let mut lh = Vec3::new(-0.2, -0.17 + breathe + bob, 0.42);
    let mut rh = Vec3::new(0.21, -0.2 + breathe - bob, 0.38);
    let mut glow = 0.0;
    let mut kick = 0.0f32;
    match p.state {
        State::Attack { m, t: st, .. } => {
            let k = strike(m, st);
            match m {
                Move::Jab => rh = rh.lerp(Vec3::new(0.04, -0.06, 0.82), k.max(0.0)) + Vec3::new(0.02, 0.0, k.min(0.0) * 0.3),
                Move::Hook => {
                    let a = k.max(-0.3);
                    let ang = a * 2.3;
                    rh = Vec3::new(0.48 * ang.cos() - 0.1 * k.max(0.0), -0.08, 0.32 + 0.38 * ang.sin().max(0.0)) + Vec3::new(0.15 * k.min(0.0), 0.0, 0.0);
                    lh.y -= 0.03 * k.max(0.0);
                }
                Move::Kick => kick = k,
                Move::Shadow => {
                    lh = lh.lerp(Vec3::new(-0.08, -0.05, 0.85), k.max(0.0));
                    rh = rh.lerp(Vec3::new(0.08, -0.05, 0.85), k.max(0.0));
                    glow = 1.0;
                }
            }
        }
        State::Block => {
            lh = Vec3::new(-0.07, -0.02, 0.3);
            rh = Vec3::new(0.07, -0.07, 0.32);
        }
        State::Stagger { t: st } => {
            lh.y -= st * 0.3;
            rh.y -= st * 0.3;
        }
        State::Ko { .. } => return,
        _ => {}
    }
    let power = Vec3::new(0.55, 0.35, 1.0);
    for (side, h) in [(-1.0f32, lh), (1.0, rh)] {
        let fist = at(h.x, h.y, h.z);
        let elbow = at(h.x * 1.2 + 0.1 * side, h.y - 0.3, (h.z - 0.32).max(0.02));
        let shoulder = at(0.32 * side, -0.55, -0.1);
        s.limb(shoulder, elbow, 0.07, sleeve, Vec3::ZERO, 0.0);
        s.limb(elbow, fist - cam.fwd * 0.05, 0.05, skin, Vec3::ZERO, 0.0);
        s.limb(fist - cam.fwd * 0.08, fist - cam.fwd * 0.02, 0.054, wrap, Vec3::ZERO, 0.0);
        let fist_col = skin + power * glow * 0.6;
        s.add(Shape::Sphere, Mat4::from_scale_rotation_translation(Vec3::new(0.06, 0.055, 0.07), Quat::from_mat3(&glam::Mat3::from_cols(cam.right, cam.up, -cam.fwd)), fist), fist_col, power, glow * 1.2 + p.power * 0.6, glow * 1.5);
    }
    if kick != 0.0 {
        let k = kick.max(0.0);
        let foot = at(0.12, -0.9 + k * 0.75, 0.35 + k * 0.45);
        let knee = at(0.15, -0.75 + k * 0.4, 0.15 + k * 0.2);
        s.limb(knee, foot, 0.075, sleeve, Vec3::ZERO, 0.0);
        s.sphere(foot + cam.fwd * 0.05, Vec3::new(0.07, 0.06, 0.12), skin, Vec3::ZERO, 0.0, 0.0);
    }
}

pub fn sparks(s: &mut Scene, list: &mut Vec<Spark>, dt: f32) {
    for p in list.iter_mut() {
        p.life -= dt;
        p.vel.y -= 6.0 * dt;
        p.pos += p.vel * dt;
    }
    list.retain(|p| p.life > 0.0);
    for p in list.iter() {
        s.sphere(p.pos, Vec3::splat(0.025 + p.life * 0.04), p.color, Vec3::ZERO, 0.0, 3.0);
    }
}
