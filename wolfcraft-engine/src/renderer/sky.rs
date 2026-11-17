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

/// The real planet seen from space: colours of its surface, `n × n` samples per cube face
/// (face-major, then j, then i). Set by the game when a world (or dimension) is loaded.
static GLOBE: std::sync::Mutex<Option<(u32, Vec<[f32; 3]>, Vec<f32>)>> = std::sync::Mutex::new(None);

/// Gives the sky the planet's surface colours for the view from orbit (`n × n` per face).
pub fn set_globe(n: u32, colors: Vec<[f32; 3]>) {
    let count = colors.len();
    set_globe_terrain(n, colors, vec![1.0; count]);
}

/// Coarse solid terrain columns for orbit; nearby chunks remain the actual editable voxels.
pub fn set_globe_terrain(n: u32, colors: Vec<[f32; 3]>, heights: Vec<f32>) {
    if let Ok(mut g) = GLOBE.lock() {
        *g = (n > 0 && colors.len() == (6 * n as usize * n as usize)
            && heights.len() == colors.len() && heights.iter().all(|h| h.is_finite() && *h >= 1.0 && *h <= 2.0))
            .then_some((n, colors, heights));
    }
}

// The sky pass deliberately does not write world depth. Sort only the globe, leaving
// stars and atmosphere behind it, so hidden terrain cannot paint over its near side.
fn sort_globe_surface(vertices: &mut [ModelVertex]) {
    let distance = |t: &[ModelVertex;3]| {
        let c = (Vec3::from(t[0].pos)+Vec3::from(t[1].pos)+Vec3::from(t[2].pos))/3.0;
        c.length_squared()
    };
    let mut triangles: Vec<[ModelVertex;3]> = vertices.chunks_exact(3).map(|t|[t[0],t[1],t[2]]).collect();
    triangles.sort_by(|a,b|distance(b).total_cmp(&distance(a)));
    for (dst,tri) in vertices.chunks_exact_mut(3).zip(triangles) { dst.copy_from_slice(&tri); }
}

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
                let u = axes[(i + 1) % 3];
                let v = axes[(i + 2) % 3];
                // Small solid columns, rather than one painted quad per face.
                const N: u32 = 6;
                let step = 2.0 * h / N as f32;
                for y in 0..N {
                    for x in 0..N {
                        let noise = rnd(x + y * N + i as u32 * N * N, 17);
                        let relief = (noise * 3.0).floor() * step * 0.22;
                        let base = centre + fnorm * h + u * (-h + x as f32 * step) + v * (-h + y as f32 * step);
                        let bottom = [base, base + u * step, base + (u + v) * step, base + v * step];
                        let top = bottom.map(|p| p + fnorm * relief);
                        let lit = (0.35 + 0.65 * fnorm.dot(sun).max(0.0)) * (0.88 + noise * 0.20);
                        let shade = mix(sky, col.map(|c| c * lit), vis);
                        self.tri(top[0], top[1], top[2], shade);
                        self.tri(top[0], top[2], top[3], shade);
                        if relief > 0.0 {
                            for edge in 0..4 {
                                let next = (edge + 1) % 4;
                                let side = mix(sky, col.map(|c| c * lit * 0.68), vis);
                                self.tri(bottom[edge], bottom[next], top[next], side);
                                self.tri(bottom[edge], top[next], top[edge], side);
                            }
                        }
                    }
                }
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

/// A layer of blocky clouds 110 blocks above the sea, drawn on the sky dome's scale (so the
/// world always covers them). They drift slowly; the top is white, the sides and the belly
/// shaded, warm at sunset, dim at night; far ones fade into the sky.
fn clouds(o: &mut Out, p: &SkyParams, up: Vec3, altitude: f64) {
    const LAYER: f64 = 110.0;
    const CELL: f32 = 48.0;
    const VIEW: f32 = 420.0;
    if p.underwater || altitude > LAYER - 6.0 {
        return;
    }
    let k = 30.0 / VIEW; // blocks → dome units
    let h = (LAYER - altitude) as f32;
    // A frame fixed to the world (two axes across the face of the cube we are on), so the
    // clouds stay put while we walk under them.
    let ax = if up.x.abs() >= up.y.abs() && up.x.abs() >= up.z.abs() { 0 } else if up.y.abs() >= up.z.abs() { 1 } else { 2 };
    let axis = |i: usize| [Vec3::X, Vec3::Y, Vec3::Z][i];
    let (a_ax, b_ax) = (axis((ax + 1) % 3), axis((ax + 2) % 3));
    let t1 = (a_ax - up * a_ax.dot(up)).normalize_or_zero();
    let t2 = up.cross(t1).normalize_or_zero();
    let t2 = if t2.dot(b_ax) < 0.0 { -t2 } else { t2 };
    let drift = p.time * 1.6;
    let ou = p.eye.dot(a_ax.as_dvec3()) as f32 + drift;
    let ov = p.eye.dot(b_ax.as_dvec3()) as f32;
    let (ci, cj) = ((ou / CELL).floor() as i32, (ov / CELL).floor() as i32);
    let n = (VIEW / CELL) as i32;
    let sun = p.sun_dir.normalize_or_zero();
    let twilight = 1.0 - (p.day * 2.0 - 1.0).abs();
    let lightc = mix(mix([0.32, 0.36, 0.48], [1.0, 1.0, 1.0], p.day), [1.0, 0.80, 0.66], twilight * 0.6);
    for i in ci - n..=ci + n {
        for j in cj - n..=cj + n {
            let id = (i as u32).wrapping_mul(73_856_093) ^ (j as u32).wrapping_mul(19_349_663);
            if rnd(id, 11) > 0.42 {
                continue;
            }
            let cx = i as f32 * CELL + rnd(id, 12) * 20.0 - ou;
            let cz = j as f32 * CELL + rnd(id, 13) * 20.0 - ov;
            let dist = (cx * cx + cz * cz).sqrt();
            if dist > VIEW {
                continue;
            }
            let fade = (1.0 - dist / VIEW).clamp(0.0, 1.0).powf(0.7);
            let parts = 2 + (rnd(id, 14) * 2.0) as u32;
            for q in 0..parts {
                let w = 14.0 + rnd(id, 20 + q) * 22.0;
                let d = 10.0 + rnd(id, 30 + q) * 18.0;
                let ox = (rnd(id, 40 + q) - 0.5) * 24.0;
                let oz = (rnd(id, 50 + q) - 0.5) * 18.0;
                let th = 4.0 + rnd(id, 60 + q) * 3.0;
                let c = (t1 * (cx + ox) + t2 * (cz + oz) + up * (h + th * 0.5)) * k;
                let half = [t1 * (w * 0.5 * k), up * (th * 0.5 * k), t2 * (d * 0.5 * k)];
                for (axis, base) in [(1usize, 1.0f32), (0, 0.86), (2, 0.80)] {
                    for sign in [-1.0f32, 1.0] {
                        let nrm = half[axis].normalize_or_zero() * sign;
                        let fc = c + half[axis] * sign;
                        // Only the faces turned to the camera (it sits at the origin).
                        if nrm.dot(-fc) <= 0.0 {
                            continue;
                        }
                        let shade = if axis == 1 { if sign > 0.0 { 1.0 } else { 0.72 } } else { base } * (0.85 + 0.15 * nrm.dot(sun).max(0.0));
                        let col = mix(p.sky, [lightc[0] * shade, lightc[1] * shade, lightc[2] * shade], fade * 0.92);
                        let (a, b) = (half[(axis + 1) % 3], half[(axis + 2) % 3]);
                        o.tri(fc - a - b, fc + a - b, fc + a + b, col);
                        o.tri(fc - a - b, fc + a + b, fc - a + b, col);
                    }
                }
            }
        }
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
            (Vec3::new(0.55, 0.45, 0.70), 0.040, [0.64, 0.65, 0.69], false),
            (Vec3::new(-0.62, 0.52, -0.58), 0.065, [0.86, 0.30, 0.13], false),
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

    // ---- clouds: puffy cartoon blocks drifting over the land (like Hytale / Minecraft) ----
    clouds(&mut o, p, up, altitude as f64);

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
        let globe_start = o.v.len();
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
        // The real surface, face by face (continents, deserts, ice where they really are).
        let globe = GLOBE.lock().ok().and_then(|g| g.clone());
        let fake_rows = if globe.is_some() { 0 } else { lat_n };
        if let Some((gn, cols, heights)) = globe {
            let shape = crate::cubesphere::Grid { n: 2, cube };
            let sampler = crate::cubesphere::Grid { n: gn as i32, cube };
            let pt = |f: u8, a: u32, b: u32| shape.along(sampler.dir_at(f, a as f64, b as f64), 1.0).as_vec3();
            // Average the incident cells at each vertex, including across cube-face seams.
            // Every patch uses the same edge; no walls extending down to sea level.
            let key = |p: Vec3| ((p.x * 100000.0).round() as i32, (p.y * 100000.0).round() as i32, (p.z * 100000.0).round() as i32);
            let mut corners = std::collections::HashMap::<_, (f32, u32)>::new();
            for f in 0..6u8 {
                for b in 0..gn {
                    for a in 0..gn {
                        let h = heights[(f as u32 * gn * gn + b * gn + a) as usize];
                        for (i, j) in [(a,b), (a+1,b), (a+1,b+1), (a,b+1)] {
                            let e = corners.entry(key(pt(f,i,j))).or_default();
                            e.0 += h; e.1 += 1;
                        }
                    }
                }
            }
            let vertex = |f, a, b| {
                let p = pt(f,a,b);
                let (sum, count) = corners[&key(p)];
                p * (sum / count as f32)
            };
            for f in 0..6u8 {
                for b in 0..gn {
                    for a in 0..gn {
                        let [p0, p1, p2, p3] = [vertex(f,a,b), vertex(f,a+1,b), vertex(f,a+1,b+1), vertex(f,a,b+1)];
                        let n = (p1 - p0).cross(p3 - p0).normalize_or_zero();
                        let n = if n.dot(p0) < 0.0 { -n } else { n };
                        // Back side of the planet: not seen.
                        if n.dot(up) < -0.35 {
                            continue;
                        }
                        let lit = n.dot(sun_n).clamp(-0.2, 1.0) * 0.85 + 0.18;
                        let base = cols[(f as u32 * gn * gn + b * gn + a) as usize];
                        let rim = (1.0 - n.dot(up).abs()).powi(3) * 0.35 * lit.max(0.1);
                        let c = [(base[0] * lit.max(0.07) + rim * 0.3).min(1.0), (base[1] * lit.max(0.07) + rim * 0.5).min(1.0), (base[2] * lit.max(0.07) + rim).min(1.0)];
                        o.tri(centre + p0 * rad, centre + p1 * rad, centre + p2 * rad, c);
                        o.tri(centre + p0 * rad, centre + p2 * rad, centre + p3 * rad, c);
                        o.tri(centre + p0 * rad, centre + p2 * rad, centre + p1 * rad, c);
                        o.tri(centre + p0 * rad, centre + p3 * rad, centre + p2 * rad, c);
                    }
                }
            }
        }
        for la in 0..fake_rows {
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
        sort_globe_surface(&mut o.v[globe_start..]);
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
    fn orbit_terrain_is_a_connected_shell_without_sea_level_walls() {
        let colors = vec![[0.2, 0.5, 0.3]; 24];
        let p = params(DVec3::new(0.0, 1400.0, 0.0), 1.0);
        set_globe(2, colors.clone());
        let flat = build_sky(&p).len();
        set_globe_terrain(2, colors, vec![1.1; 24]);
        let raised = build_sky(&p);
        assert_eq!(raised.len(), flat, "relief must not add sea-level curtain walls");
        assert!(raised.iter().all(|v| v.pos.iter().all(|p| p.is_finite())));
        if let Ok(mut g) = GLOBE.lock() { *g = None; }
    }

    #[test]
    fn far_globe_faces_are_painted_before_near_faces() {
        let mut o=Out{v:vec![]};
        for z in [2.0,10.0,4.0] {
            o.tri(Vec3::new(0.0,0.0,z),Vec3::new(1.0,0.0,z),Vec3::new(0.0,1.0,z),[1.0,1.0,1.0]);
        }
        sort_globe_surface(&mut o.v);
        let depths:Vec<_>=o.v.chunks_exact(3).map(|t|t[0].pos[2]).collect();
        assert_eq!(depths,vec![10.0,10.0,4.0,4.0,2.0,2.0]);
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
