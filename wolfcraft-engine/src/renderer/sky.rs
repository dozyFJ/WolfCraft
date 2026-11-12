//! Sky and space: stars, sun, moon, neighbouring planets, a comet and (from high up) our own
//! planet with an atmosphere halo. Drawn in its own pass before the world with unlit,
//! flat-coloured triangles (`tex = 3` in the model shader), so nothing needs a texture.

use super::model::ModelVertex;
use glam::{DVec3, Vec3};

pub struct SkyParams {
    /// Camera position in planet space (planet centre = origin).
    pub eye: DVec3,
    pub sun_dir: Vec3,
    /// 0 night … 1 day.
    pub day: f32,
    pub planet_radius: f64,
    /// Sky colour (display space) used to fade stars in and out.
    pub sky: [f32; 3],
    pub time: f32,
    pub underwater: bool,
    /// Our planet is a (rounded) cube: draw it that way from space.
    pub cube: bool,
}

const SKY_TEX: f32 = 3.0;

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^= x >> 16;
    x
}

fn rnd(i: u32, salt: u32) -> f32 {
    hash(i.wrapping_mul(0x9E3779B9) ^ salt) as f32 / u32::MAX as f32
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

struct Out {
    v: Vec<ModelVertex>,
}

impl Out {
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, col: [f32; 3]) {
        // Both windings: the sky is seen from inside and from outside alike.
        for p in [a, b, c, a, c, b] {
            self.v.push(ModelVertex { pos: p.to_array(), normal: [0.0, 1.0, 0.0], color: col, uv: [0.0; 2], tex: SKY_TEX });
        }
    }

    /// Flat disc facing the camera at direction `dir`, distance `dist`, angular radius `ang` (rad).
    fn disc(&mut self, dir: Vec3, dist: f32, ang: f32, col: [f32; 3], seg: u32) {
        let (t1, t2) = basis(dir);
        let c = dir * dist;
        let r = dist * ang.tan();
        for i in 0..seg {
            let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
            let p0 = c + (t1 * a0.cos() + t2 * a0.sin()) * r;
            let p1 = c + (t1 * a1.cos() + t2 * a1.sin()) * r;
            self.tri(c, p0, p1, col);
        }
    }

    /// Ellipse ring around a disc (planet rings), tilted by squashing the second axis.
    /// A small lit cube in the sky (the neighbouring worlds are cubes too).
    fn cube(&mut self, dir: Vec3, dist: f32, ang: f32, col: [f32; 3], sky: [f32; 3], vis: f32, sun: Vec3, spin: f32) {
        let (t1, t2) = basis(dir);
        // Tilt the cube so three faces show.
        let (s, c) = spin.sin_cos();
        let a = (t1 * c + t2 * s + dir * 0.55).normalize();
        let b = (dir.cross(a) + t2 * 0.3).normalize();
        let b = (b - a * b.dot(a)).normalize();
        let n = a.cross(b);
        let h = dist * ang;
        let centre = dir * dist;
        let axes = [a, b, n];
        for (i, ax) in axes.iter().enumerate() {
            for sign in [-1.0f32, 1.0] {
                let fnorm = *ax * sign;
                // Only faces turned toward the camera (which sits at the origin).
                if fnorm.dot(-centre) <= 0.0 {
                    continue;
                }
                let (u, v) = (axes[(i + 1) % 3] * h, axes[(i + 2) % 3] * h);
                let fc = centre + fnorm * h;
                let lit = 0.35 + 0.65 * fnorm.dot(sun).max(0.0);
                let shade = mix(sky, [col[0] * lit, col[1] * lit, col[2] * lit], vis);
                self.tri(fc - u - v, fc + u - v, fc + u + v, shade);
                self.tri(fc - u - v, fc + u + v, fc - u + v, shade);
            }
        }
    }

    fn ring(&mut self, dir: Vec3, dist: f32, inner: f32, outer: f32, squash: f32, tilt: f32, col: [f32; 3]) {
        let (t1, t2) = basis(dir);
        let (s, c) = tilt.sin_cos();
        let u = t1 * c + t2 * s;
        let w = (-t1 * s + t2 * c) * squash;
        let centre = dir * dist;
        let seg = 28u32;
        let pt = |a: f32, r: f32| centre + (u * a.cos() + w * a.sin()) * dist * r.tan();
        for i in 0..seg {
            let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
            let (i0, i1, o0, o1) = (pt(a0, inner), pt(a1, inner), pt(a0, outer), pt(a1, outer));
            self.tri(i0, o0, o1, col);
            self.tri(i0, o1, i1, col);
        }
    }

    fn quad_star(&mut self, dir: Vec3, dist: f32, ang: f32, col: [f32; 3]) {
        let (t1, t2) = basis(dir);
        let c = dir * dist;
        let r = dist * ang;
        let p = [c - t1 * r - t2 * r, c + t1 * r - t2 * r, c + t1 * r + t2 * r, c - t1 * r + t2 * r];
        self.tri(p[0], p[1], p[2], col);
        self.tri(p[0], p[2], p[3], col);
    }
}

fn basis(dir: Vec3) -> (Vec3, Vec3) {
    let helper = if dir.y.abs() < 0.95 { Vec3::Y } else { Vec3::X };
    let t1 = dir.cross(helper).normalize_or_zero();
    let t2 = dir.cross(t1).normalize_or_zero();
    (t1, t2)
}

/// Builds the sky triangles relative to the camera.
pub fn build_sky(p: &SkyParams) -> Vec<ModelVertex> {
    let mut o = Out { v: Vec::with_capacity(12_000) };
    if p.underwater {
        return o.v;
    }
    let dist_c = p.eye.length();
    let up = (p.eye / dist_c.max(1e-6)).as_vec3();
    let r = p.planet_radius;
    let ratio = (r / dist_c).clamp(0.0, 0.9999);
    // Directions inside this cone are hidden by the planet itself.
    let cos_hidden = (1.0 - ratio * ratio).sqrt() as f32;
    let hidden = |d: Vec3| d.dot(-up) > cos_hidden;
    let altitude = (dist_c - r) as f32;
    // Stars and planets appear at night and, fading in, when leaving the atmosphere.
    let space = ((altitude - 200.0) / 900.0).clamp(0.0, 1.0);
    let night = (1.0 - p.day * 1.25).clamp(0.0, 1.0);
    let vis = night.max(space);

    // ---- stars ----
    if vis > 0.03 {
        for i in 0..320u32 {
            let z = rnd(i, 1) * 2.0 - 1.0;
            let a = rnd(i, 2) * std::f32::consts::TAU;
            let rr = (1.0 - z * z).sqrt();
            let d = Vec3::new(rr * a.cos(), z, rr * a.sin());
            if hidden(d) {
                continue;
            }
            let tw = 0.8 + 0.2 * (p.time * (1.5 + rnd(i, 3) * 2.0) + i as f32).sin();
            let b = (0.35 + rnd(i, 4) * 0.65) * tw * vis;
            let tint = match i % 5 {
                0 => [0.80, 0.88, 1.0],
                1 => [1.0, 0.92, 0.80],
                _ => [1.0, 1.0, 1.0],
            };
            let col = mix(p.sky, tint, b);
            let size = 0.0022 + rnd(i, 5) * 0.0035;
            o.quad_star(d, 36.0, size, col);
        }
    }

    // ---- neighbouring planets (fixed in the sky, like distant worlds) ----
    if vis > 0.08 {
        let planets: [(Vec3, f32, [f32; 3], bool); 4] = [
            (Vec3::new(0.55, 0.45, 0.70), 0.040, [0.86, 0.44, 0.30], false),
            (Vec3::new(-0.62, 0.52, -0.58), 0.065, [0.88, 0.76, 0.56], true),
            (Vec3::new(0.15, 0.78, -0.62), 0.028, [0.62, 0.82, 0.92], false),
            (Vec3::new(-0.45, 0.30, 0.84), 0.034, [0.55, 0.40, 0.70], false),
        ];
        for (n, (d, ang, col, ring)) in planets.iter().enumerate() {
            let d = d.normalize();
            if hidden(d) {
                continue;
            }
            if *ring {
                o.ring(d, 32.5, ang * 1.35, ang * 2.1, 0.28, 0.45, mix(p.sky, [0.80, 0.70, 0.55], vis));
            }
            // Neighbouring worlds are slowly turning cubes.
            o.cube(d, 32.0 - n as f32 * 0.01, ang * 0.8, *col, p.sky, vis, p.sun_dir.normalize_or_zero(), p.time * 0.02 + n as f32 * 1.7);
        }
        // Comet: a bright head with a fading tail drifting across the sky.
        let phi = p.time * 0.012;
        let (ca, cb) = (Vec3::new(0.8, 0.3, 0.5).normalize(), Vec3::new(-0.3, 0.9, 0.1).normalize());
        let head = (ca * phi.cos() + cb * phi.sin()).normalize();
        let vel = (-ca * phi.sin() + cb * phi.cos()).normalize();
        if !hidden(head) {
            for k in 0..9 {
                let f = k as f32 / 9.0;
                let d = (head - vel * f * 0.16).normalize();
                let w = 0.012 * (1.0 - f * 0.8);
                o.disc(d, 33.0 + k as f32 * 0.01, w, mix(p.sky, [0.75, 0.95, 1.0], vis * (1.0 - f * 0.85)), 8);
            }
            o.disc(head, 32.9, 0.016, mix(p.sky, [1.0, 1.0, 1.0], vis), 10);
        }
    }

    // ---- sun and moon ----
    let sun = p.sun_dir.normalize_or_zero();
    if sun != Vec3::ZERO {
        if !hidden(sun) {
            let warm = mix([1.0, 0.95, 0.75], [1.0, 0.6, 0.3], (1.0 - p.day).clamp(0.0, 0.8));
            o.disc(sun, 31.0, 0.11, mix(p.sky, warm, 0.22), 24);
            o.disc(sun, 30.5, 0.065, mix(p.sky, warm, 0.6), 24);
            o.disc(sun, 30.0, 0.042, [1.0, 0.98, 0.88], 20);
        }
        let moon = -sun;
        if !hidden(moon) {
            o.disc(moon, 30.0, 0.036, [0.86, 0.88, 0.92], 20);
            let (t1, t2) = basis(moon);
            for (i, (a, b, s)) in [(0.3f32, 0.2f32, 0.008f32), (-0.4, 0.1, 0.006), (0.1, -0.35, 0.007)].iter().enumerate() {
                let d = (moon + (t1 * *a + t2 * *b) * 0.036).normalize();
                o.disc(d, 29.9 - i as f32 * 0.01, *s, [0.68, 0.70, 0.76], 8);
            }
        }
    }

    // ---- our planet seen from above, with an atmosphere halo ----
    if altitude > 300.0 {
        let down = -up;
        let d_minus_r = (dist_c - r).max(1.0);
        let k = (15.0 / d_minus_r).min(80.0 / dist_c);
        let centre = down * (dist_c * k) as f32;
        let rad = (r * k) as f32;
        // Atmosphere: soft blue rings behind the limb.
        let (t1, t2) = basis(down);
        let behind = down * (dist_c * k) as f32 * 1.0 + down * rad * 0.25;
        let halo = |scale: f32, col: [f32; 3], o: &mut Out| {
            let rr = rad * scale;
            let seg = 40u32;
            for i in 0..seg {
                let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
                let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
                let p0 = behind + (t1 * a0.cos() + t2 * a0.sin()) * rr;
                let p1 = behind + (t1 * a1.cos() + t2 * a1.sin()) * rr;
                o.tri(behind, p0, p1, col);
            }
        };
        halo(1.07, mix(p.sky, [0.10, 0.20, 0.45], space.max(0.6)), &mut o);
        halo(1.035, mix(p.sky, [0.30, 0.55, 0.95], space.max(0.6)), &mut o);
        // Globe: latitude/longitude patches coloured by a cheap continent noise.
        let (lat_n, lon_n) = if p.cube { (28u32, 56u32) } else { (14u32, 28u32) };
        let sun_n = p.sun_dir.normalize_or_zero();
        let cube = p.cube;
        let vert = |la: u32, lo: u32| -> Vec3 {
            let th = std::f32::consts::PI * la as f32 / lat_n as f32;
            let ph = std::f32::consts::TAU * lo as f32 / lon_n as f32;
            let v = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
            if cube { crate::cubesphere::Grid { n: 2, cube: true }.along(v.as_dvec3(), 1.0).as_vec3() } else { v }
        };
        let land = |n: Vec3| -> [f32; 3] {
            let f = (n.x * 3.1 + 1.3).sin() * (n.y * 2.7).cos() + (n.z * 4.3 - n.x * 1.7).sin() * 0.6 + (n.y * 7.0 + n.z * 5.0).sin() * 0.25;
            if n.y.abs() > 0.86 {
                [0.92, 0.95, 0.98]
            } else if f > 0.35 {
                if f > 0.9 { [0.55, 0.50, 0.32] } else { [0.20, 0.52, 0.22] }
            } else {
                [0.09, 0.25, 0.62]
            }
        };
        for la in 0..lat_n {
            for lo in 0..lon_n {
                let (a, b, c, d) = (vert(la, lo), vert(la + 1, lo), vert(la + 1, lo + 1), vert(la, lo + 1));
                let n = (a + b + c + d).normalize();
                let lit = n.dot(sun_n).clamp(-0.2, 1.0) * 0.8 + 0.2;
                let base = land(n);
                let col = [(base[0] * lit.max(0.08)).min(1.0), (base[1] * lit.max(0.08)).min(1.0), (base[2] * lit.max(0.08)).min(1.0)];
                // Atmospheric rim light toward the limb.
                let rim = (1.0 - (n.dot(-down)).abs()).powi(3) * 0.35 * lit.max(0.1);
                let col = [col[0] + rim * 0.3, col[1] + rim * 0.5, col[2] + rim];
                let col = [col[0].min(1.0), col[1].min(1.0), col[2].min(1.0)];
                o.tri(centre + a * rad, centre + b * rad, centre + c * rad, col);
                o.tri(centre + a * rad, centre + c * rad, centre + d * rad, col);
            }
        }
    }
    o.v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(eye: DVec3, day: f32) -> SkyParams {
        SkyParams { eye, sun_dir: Vec3::new(0.0, 1.0, 0.0), day, planet_radius: 1000.0, sky: [0.1, 0.2, 0.4], time: 1.0, underwater: false, cube: false }
    }

    #[test]
    fn night_has_stars_and_day_ground_has_none() {
        let night = build_sky(&params(DVec3::new(0.0, 1002.0, 0.0), 0.0));
        let day = build_sky(&params(DVec3::new(0.0, 1002.0, 0.0), 1.0));
        assert!(night.len() > day.len());
        assert!(night.iter().all(|v| v.tex == SKY_TEX));
    }

    #[test]
    fn our_planet_appears_only_from_high_up() {
        let ground = build_sky(&params(DVec3::new(0.0, 1002.0, 0.0), 1.0));
        let orbit = build_sky(&params(DVec3::new(0.0, 3500.0, 0.0), 1.0));
        assert!(orbit.len() > ground.len() + 1000);
    }

    #[test]
    fn nothing_drawn_underwater() {
        let mut p = params(DVec3::new(0.0, 1002.0, 0.0), 0.0);
        p.underwater = true;
        assert!(build_sky(&p).is_empty());
    }

    #[test]
    fn all_vertices_stay_inside_the_far_plane() {
        for eye in [DVec3::new(0.0, 1002.0, 0.0), DVec3::new(0.0, 1400.0, 0.0), DVec3::new(0.0, 6000.0, 0.0)] {
            for v in build_sky(&params(eye, 0.0)) {
                assert!(Vec3::from(v.pos).length() < 99.0, "{:?}", v.pos);
            }
        }
    }
}
