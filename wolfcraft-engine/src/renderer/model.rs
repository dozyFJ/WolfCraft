//! Blocky player model (head, torso, arms, legs) with walk animation.
//! Built on the CPU each frame relative to the camera (floating origin) and drawn with a tiny
//! coloured-vertex pipeline, so it needs no extra texture.

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ModelVertex {
    /// Position relative to the camera.
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    /// Atlas uv (used when `tex` > 0).
    pub uv: [f32; 2],
    /// 0 = flat `color`, 1 = entities atlas (skins), 2 = items atlas (cut-out icons), 3 = flat
    /// and unlit (glowing particles).
    pub tex: f32,
}

/// Atlas rectangles the models sample: uv `[u0, v0, u1, v1]`.
#[derive(Clone, Debug, Default)]
pub struct ModelTextures {
    /// Player skin in the entities atlas.
    pub player: Option<[f32; 4]>,
    /// Every sprite of the items atlas, by sprite id.
    pub items: Vec<[f32; 4]>,
    /// 16x16 RGBA pixels of every item sprite (row-major), for 3D held items.
    pub item_pixels: Vec<Vec<[u8; 4]>>,
}

static TEXTURES: std::sync::RwLock<Option<ModelTextures>> = std::sync::RwLock::new(None);

/// Tells the model builders where the skins and item icons are (call once the atlases exist).
/// Without it everything is drawn in flat colours.
pub fn set_textures(t: ModelTextures) {
    if let Ok(mut g) = TEXTURES.write() {
        *g = Some(t);
    }
}

fn textures() -> Option<ModelTextures> {
    TEXTURES.read().ok().and_then(|g| g.clone())
}

/// Skin band (pixel rows of the 64x64 player skin) for a body-part colour.
fn skin_rows(color: [f32; 3]) -> Option<(f32, f32)> {
    Some(match color {
        c if c == HAIR => (0.0, 4.0),
        c if c == SKIN => (4.0, 16.0),
        c if c == JACKET || c == SLEEVE => (16.0, 30.0),
        c if c == BELT => (30.0, 32.0),
        c if c == TROUSERS => (32.0, 48.0),
        c if c == BOOTS => (48.0, 64.0),
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerPose {
    pub feet: DVec3,
    pub heading: DVec3,
    pub up: DVec3,
    /// Look pitch, radians (+ up).
    pub pitch: f32,
    /// Walk cycle phase, radians.
    pub phase: f32,
    /// 0 = standing, 1 = full walking swing.
    pub amount: f32,
    pub sneaking: bool,
    /// Swimming (in water, off the ground): the body lies flat and crawls.
    pub swimming: bool,
    /// Skin preset (see `SKIN_NAMES`).
    pub skin: u8,
    /// Swing of the right arm 0..1 (0 = none); the motion depends on what is held.
    pub attack: f32,
    /// What the right hand holds (drawn in it, and it picks the swing).
    pub held: Held,
    /// Worn armour per slot (helmet, chestplate, leggings, boots): material 0 leather,
    /// 1 bronze, 2 iron, 3 diamond.
    pub armor: [Option<u8>; 4],
}

/// Item in the character's right hand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Held {
    #[default]
    Nothing,
    /// Tool: (kind 0 pickaxe, 1 axe, 2 shovel, 3 sword; material 0..3).
    Tool(u8, u8),
    /// A block, drawn as a small cube of this colour.
    Block([f32; 3]),
    /// Another item: its items-atlas sprite.
    Item(u16),
}

/// Colour of armour of material `m` (leather, bronze, iron, diamond).
pub fn armor_color(m: u8) -> [f32; 3] {
    match m {
        0 => [0.55, 0.36, 0.20],
        1 => [0.80, 0.56, 0.26],
        2 => [0.80, 0.81, 0.84],
        _ => [0.36, 0.90, 0.86],
    }
}

/// Right-arm angle (negative = raised forward) during a swing `a` (0..1) with `held`.
/// Pickaxe and axe are lifted high and brought down hard, the shovel is thrust low, the sword
/// sweeps wide, a fist punches forward.
fn swing_angle(a: f32, held: Held) -> f32 {
    let a = a.clamp(0.0, 1.0);
    if a <= 0.0 {
        return 0.0;
    }
    let ss = |x: f32| x * x * (3.0 - 2.0 * x);
    let pi = std::f32::consts::PI;
    match held {
        Held::Tool(0 | 1, _) => {
            // Wind up over the head, chop down, recover.
            if a < 0.4 { -2.7 * ss(a / 0.4) } else if a < 0.55 { -2.7 + 2.1 * ss((a - 0.4) / 0.15) } else { -0.6 * (1.0 - ss((a - 0.55) / 0.45)) }
        }
        Held::Tool(2, _) => -1.1 * (a * pi).sin() - 0.3 * (a * 2.0 * pi).sin().max(0.0),
        Held::Tool(_, _) => -2.0 * (a * pi).sin(),
        _ => -1.5 * (a * pi).sin(),
    }
}

/// Names of the built-in skins, in the order of `skin_colors`.
pub const SKIN_NAMES: [&str; 6] = ["Бирюзовый", "Красный", "Зелёный", "Фиолетовый", "Оранжевый", "Тёмный"];

/// (skin tone, hair, jacket, sleeve, trousers) of skin preset `i`.
pub fn skin_colors(i: u8) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
    match i {
        1 => ([0.86, 0.68, 0.55], [0.12, 0.08, 0.06], [0.75, 0.15, 0.15], [0.62, 0.12, 0.12], [0.20, 0.20, 0.24]),
        2 => ([0.78, 0.60, 0.46], [0.35, 0.22, 0.10], [0.20, 0.60, 0.25], [0.15, 0.50, 0.20], [0.30, 0.24, 0.16]),
        3 => ([0.92, 0.76, 0.64], [0.55, 0.25, 0.70], [0.50, 0.25, 0.75], [0.42, 0.20, 0.64], [0.14, 0.14, 0.26]),
        4 => ([0.70, 0.50, 0.38], [0.85, 0.35, 0.08], [0.95, 0.55, 0.12], [0.82, 0.45, 0.10], [0.22, 0.22, 0.28]),
        5 => ([0.55, 0.38, 0.28], [0.06, 0.06, 0.08], [0.16, 0.16, 0.20], [0.12, 0.12, 0.16], [0.08, 0.08, 0.10]),
        _ => (SKIN, HAIR, JACKET, SLEEVE, TROUSERS),
    }
}

const SKIN: [f32; 3] = [0.86, 0.68, 0.55];
const HAIR: [f32; 3] = [0.20, 0.13, 0.08];
const JACKET: [f32; 3] = [0.10, 0.50, 0.55];
const SLEEVE: [f32; 3] = [0.08, 0.42, 0.47];
const TROUSERS: [f32; 3] = [0.18, 0.20, 0.30];
const BOOTS: [f32; 3] = [0.26, 0.17, 0.11];
const EYE: [f32; 3] = [0.10, 0.12, 0.18];
const BELT: [f32; 3] = [0.95, 0.60, 0.15];

struct Frame {
    origin: DVec3, // feet relative to camera
    right: DVec3,
    up: DVec3,
    fwd: DVec3,
}

impl Frame {
    fn to_world(&self, local: Vec3) -> DVec3 {
        self.origin + self.right * local.x as f64 + self.up * local.y as f64 + self.fwd * local.z as f64
    }
    fn dir(&self, local: Vec3) -> Vec3 {
        (self.right * local.x as f64 + self.up * local.y as f64 + self.fwd * local.z as f64).as_vec3()
    }
}

/// Cartoon style: creature and hand boxes get rounded (chamfered) edges.
static ROUNDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Turns the rounded cartoon shapes of creatures, players and hands on or off.
pub fn set_rounded(on: bool) {
    ROUNDED.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Appends an axis-aligned (in the character's local frame) box, optionally rotated about the
/// local X axis (the "right" axis) around `pivot` by `angle` radians.
fn add_box(out: &mut Vec<ModelVertex>, f: &Frame, min: Vec3, max: Vec3, pivot: Vec3, angle: f32, color: [f32; 3]) {
    add_box_r(out, f, min, max, pivot, angle, color, ROUNDED.load(std::sync::atomic::Ordering::Relaxed));
}

#[allow(clippy::too_many_arguments)]
fn add_box_r(out: &mut Vec<ModelVertex>, f: &Frame, min: Vec3, max: Vec3, pivot: Vec3, angle: f32, color: [f32; 3], rounded: bool) {
    let (s, c) = angle.sin_cos();
    let rot = |p: Vec3| -> Vec3 {
        let d = p - pivot;
        // Rotation about X: y' = y*c - z*s ; z' = y*s + z*c  (positive angle swings forward).
        pivot + Vec3::new(d.x, d.y * c - d.z * s, d.y * s + d.z * c)
    };
    let rot_n = |n: Vec3| Vec3::new(n.x, n.y * c - n.z * s, n.y * s + n.z * c);
    let corners = |x: bool, y: bool, z: bool| Vec3::new(if x { max.x } else { min.x }, if y { max.y } else { min.y }, if z { max.z } else { min.z });
    // (normal, four corners CCW seen from outside)
    let faces: [(Vec3, [(bool, bool, bool); 4]); 6] = [
        (Vec3::X, [(true, false, false), (true, true, false), (true, true, true), (true, false, true)]),
        (Vec3::NEG_X, [(false, false, true), (false, true, true), (false, true, false), (false, false, false)]),
        (Vec3::Y, [(false, true, false), (false, true, true), (true, true, true), (true, true, false)]),
        (Vec3::NEG_Y, [(false, false, true), (false, false, false), (true, false, false), (true, false, true)]),
        (Vec3::Z, [(true, false, true), (true, true, true), (false, true, true), (false, false, true)]),
        (Vec3::NEG_Z, [(false, false, false), (false, true, false), (true, true, false), (true, false, false)]),
    ];
    let skin = textures().and_then(|t| t.player).zip(skin_rows(color));
    let size = max - min;
    // Cartoon style: chamfered edges, radius from the part's thinnest side (thin overlays stay sharp).
    let round = (rounded && size.min_element() >= 0.05).then(|| (size.min_element() * 0.26).min(0.08));
    let centre = (min + max) * 0.5;
    let half = size * 0.5;
    // Box corner as signs (-1/+1 per axis) and its point pulled in by `r` on every axis but `axis`.
    let corner_sign = |(x, y, z): (bool, bool, bool)| Vec3::new(if x { 1.0 } else { -1.0 }, if y { 1.0 } else { -1.0 }, if z { 1.0 } else { -1.0 });
    let inset_s = |s: Vec3, axis: usize, r: f32| {
        let mut p = centre + s * half;
        for k in 0..3 {
            if k != axis {
                p[k] -= s[k] * r;
            }
        }
        p
    };
    let inset = |q: (bool, bool, bool), axis: usize, r: f32| inset_s(corner_sign(q), axis, r);
    let sign_index = |s: Vec3| (s.x > 0.0) as usize | ((s.y > 0.0) as usize) << 1 | ((s.z > 0.0) as usize) << 2;
    let corner_index = |q: (bool, bool, bool)| sign_index(corner_sign(q));
    let mut face_uv = [[[0.0f32; 2]; 8]; 6];
    let mut face_tex = [0.0f32; 6];
    for (fi, (n, quad)) in faces.into_iter().enumerate() {
        let nw = f.dir(rot_n(n)).normalize_or_zero();
        let v: Vec<[f32; 3]> = quad.iter().map(|(x, y, z)| f.to_world(rot(corners(*x, *y, *z))).as_vec3().to_array()).collect();
        // Skin window: 32 px per block along the face, inside the part's band.
        let (tex, uvs) = match skin {
            Some((r, (y0, y1))) => {
                let (w, h) = match fi {
                    0 | 1 => (size.z, size.y),
                    2 | 3 => (size.x, size.z),
                    _ => (size.x, size.y),
                };
                let pw = (w * 32.0).clamp(1.0, 64.0);
                let x0 = ((fi as f32 * 11.0) % (64.0 - pw + 1.0)).max(0.0);
                let ph = (h * 32.0).clamp(1.0, y1 - y0);
                let to = |px: f32, py: f32| [r[0] + (r[2] - r[0]) * px / 64.0, r[1] + (r[3] - r[1]) * py / 64.0];
                (1.0, [to(x0, y0 + ph), to(x0, y0), to(x0 + pw, y0), to(x0 + pw, y0 + ph)])
            }
            None => {
                // Flat colours get box-local pixel coordinates: the shader paints a 1/16-block
                // pattern. Measured from the box's own corner and seeded by its size and colour
                // only, so the pattern sticks to the box while it moves (no shimmering).
                let seed = fi as f32 * 3.7 + size.x * 5.1 + size.y * 7.3 + size.z * 2.9 + color[0] * 11.0 + color[1] * 13.0;
                let uv_of = |(x, y, z): (bool, bool, bool)| -> [f32; 2] {
                    let p = corners(x, y, z) - min;
                    let (a, b) = match fi {
                        0 | 1 => (p.z, p.y),
                        2 | 3 => (p.x, p.z),
                        _ => (p.x, p.y),
                    };
                    [a * 16.0 + seed, b * 16.0 + seed * 1.7]
                };
                (0.0, [uv_of(quad[0]), uv_of(quad[1]), uv_of(quad[2]), uv_of(quad[3])])
            }
        };
        if let Some(r) = round {
            for (qi, q) in quad.iter().enumerate() {
                face_uv[fi][corner_index(*q)] = uvs[qi];
            }
            face_tex[fi] = tex;
            // Inset face: each corner pulled in by `r` along the face's own plane, with a normal
            // leaning a little towards the corner so the face shades like a soft cushion.
            let axis = fi / 2;
            let pts: Vec<Vec3> = quad.iter().map(|q| inset(*q, axis, r)).collect();
            for idx in [0usize, 2, 1, 0, 3, 2] {
                let s = corner_sign(quad[idx]);
                let ln = (n + s * 0.35).normalize();
                let p = f.to_world(rot(pts[idx])).as_vec3().to_array();
                out.push(ModelVertex { pos: p, normal: f.dir(rot_n(ln)).normalize_or_zero().to_array(), color, uv: uvs[idx], tex });
            }
            continue;
        }
        // (right, up, forward) is a left-handed frame, so the winding is mirrored.
        for idx in [0usize, 2, 1, 0, 3, 2] {
            out.push(ModelVertex { pos: v[idx], normal: nw.to_array(), color, uv: uvs[idx], tex });
        }
    }
    let Some(r) = round else { return };
    // Rounded edges: a bevel strip between every two neighbouring faces and a small triangle
    // at every corner, with normals that blend between the faces (smooth, toy-like shading).
    let mut tri = |pts: &[(Vec3, Vec3, [f32; 2])], tex: f32| {
        // Make the local winding counter-clockwise from outside, then mirror it like the faces.
        let (a, b, c) = (pts[0].0, pts[1].0, pts[2].0);
        let outward = pts.iter().map(|p| p.1).sum::<Vec3>();
        let ccw = (b - a).cross(c - a).dot(outward) >= 0.0;
        let order: [usize; 3] = if ccw { [0, 2, 1] } else { [0, 1, 2] };
        for i in order {
            let (p, nl, uv) = pts[i];
            out.push(ModelVertex { pos: f.to_world(rot(p)).as_vec3().to_array(), normal: f.dir(rot_n(nl)).normalize_or_zero().to_array(), color, uv, tex });
        }
    };
    let face_n = |axis: usize, s: f32| { let mut v = Vec3::ZERO; v[axis] = s; v };
    for a in 0..3 {
        for b in (a + 1)..3 {
            let c = 3 - a - b;
            for sa in [-1.0f32, 1.0] {
                for sb in [-1.0f32, 1.0] {
                    let fa = a * 2 + (sa < 0.0) as usize;
                    let fb = b * 2 + (sb < 0.0) as usize;
                    let corner = |sc: f32| { let mut s = Vec3::ZERO; s[a] = sa; s[b] = sb; s[c] = sc; s };
                    let (c0, c1) = (corner(-1.0), corner(1.0));
                    let (na, nb) = (face_n(a, sa), face_n(b, sb));
                    let mid = (na + nb).normalize();
                    let uv0 = face_uv[fa][sign_index(c0)];
                    let uv1 = face_uv[fa][sign_index(c1)];
                    let p = |s: Vec3, axis: usize| inset_s(s, axis, r);
                    let blend = |nf: Vec3, s: Vec3| (nf * 0.6 + mid * 0.6 + s * 0.2).normalize();
                    let q = [(p(c0, a), blend(na, c0), uv0), (p(c1, a), blend(na, c1), uv1), (p(c1, b), blend(nb, c1), uv1), (p(c0, b), blend(nb, c0), uv0)];
                    tri(&[q[0], q[1], q[2]], face_tex[fa]);
                    tri(&[q[0], q[2], q[3]], face_tex[fa]);
                }
            }
        }
    }
    for i in 0..8 {
        let s = Vec3::new(if i & 1 != 0 { 1.0 } else { -1.0 }, if i & 2 != 0 { 1.0 } else { -1.0 }, if i & 4 != 0 { 1.0 } else { -1.0 });
        let fx = (s.x < 0.0) as usize;
        let uv = face_uv[fx][sign_index(s)];
        let nc = s.normalize();
        let pts: Vec<(Vec3, Vec3, [f32; 2])> = (0..3).map(|axis| (inset_s(s, axis, r), (face_n(axis, s[axis]) * 0.5 + nc).normalize(), uv)).collect();
        tri(&pts, face_tex[fx]);
    }
}

/// Builds the model triangles relative to `camera`.
pub fn build_player_model(pose: &PlayerPose, camera: DVec3) -> Vec<ModelVertex> {
    let (skin_c, hair_c, jacket_c, sleeve_c, trousers_c) = skin_colors(pose.skin);
    let up = pose.up.normalize();
    let fwd = (pose.heading - up * pose.heading.dot(up)).normalize_or_zero();
    let right = fwd.cross(up).normalize_or_zero();
    let mut frame = Frame { origin: pose.feet - camera, right, up, fwd };
    let mut v = Vec::with_capacity(11 * 36);
    if pose.swimming {
        // Lie face-down along the heading, centred on the old body centre.
        frame = Frame { origin: pose.feet - camera + up * 0.9 - fwd * 0.9, right, up: fwd, fwd: -up };
    }
    let swim = pose.swimming;
    let swing = if swim { 0.5 * (pose.phase * 0.8).sin() } else { pose.phase.sin() * 0.85 * pose.amount };
    let crouch = if pose.sneaking { 0.12 } else { 0.0 };
    let lean = if pose.sneaking { 0.0 } else { 0.0 };
    let _ = lean;

    // Legs (pivot at the hip).
    let hip = 0.68;
    for (side, ang) in [(-1.0f32, swing), (1.0, -swing)] {
        let x0 = if side < 0.0 { -0.25 } else { 0.0 };
        add_box(&mut v, &frame, Vec3::new(x0, 0.0, -0.125), Vec3::new(x0 + 0.25, hip, 0.125), Vec3::new(x0 + 0.125, hip, 0.0), ang, trousers_c);
        add_box(&mut v, &frame, Vec3::new(x0 - 0.005, 0.0, -0.13), Vec3::new(x0 + 0.255, 0.18, 0.15), Vec3::new(x0 + 0.125, hip, 0.0), ang, BOOTS);
    }
    // Torso.
    let ty0 = hip - crouch * 0.4;
    add_box(&mut v, &frame, Vec3::new(-0.25, ty0, -0.125), Vec3::new(0.25, ty0 + 0.65, 0.125), Vec3::ZERO, 0.0, jacket_c);
    add_box(&mut v, &frame, Vec3::new(-0.255, ty0, -0.13), Vec3::new(0.255, ty0 + 0.08, 0.13), Vec3::ZERO, 0.0, BELT);
    // Arms (pivot at the shoulder), opposite phase to the legs.
    let shoulder = ty0 + 0.6;
    for (side, ang) in [(-1.0f32, -swing), (1.0, swing)] {
        // Crawl stroke: the arms circle all the way round, one half a turn behind the other.
        let ang = if swim { pose.phase * 0.8 + if side < 0.0 { 0.0 } else { std::f32::consts::PI } + 2.6 } else { ang };
        // Attack: the right arm's motion depends on the tool. Holding something keeps the arm a
        // little forward, as in Minecraft.
        let a = pose.attack.clamp(0.0, 1.0);
        let hold = if side > 0.0 && pose.held != Held::Nothing && !swim { -0.3 } else { 0.0 };
        let ang = if side > 0.0 && a > 0.0 && !swim {
            let k = (a * std::f32::consts::PI).sin();
            ang * (1.0 - k) + swing_angle(a, pose.held) + hold * (1.0 - k)
        } else {
            ang * if hold != 0.0 { 0.4 } else { 1.0 } + hold
        };
        let x0 = if side < 0.0 { -0.5 } else { 0.25 };
        add_box(&mut v, &frame, Vec3::new(x0, ty0, -0.125), Vec3::new(x0 + 0.25, shoulder, 0.125), Vec3::new(x0 + 0.125, shoulder, 0.0), ang, sleeve_c);
        add_box(&mut v, &frame, Vec3::new(x0, ty0 - 0.02, -0.12), Vec3::new(x0 + 0.25, ty0 + 0.16, 0.12), Vec3::new(x0 + 0.125, shoulder, 0.0), ang, skin_c);
        let pivot = Vec3::new(x0 + 0.125, shoulder, 0.0);
        if let Some(m) = pose.armor[1] {
            // Chestplate shoulder pads.
            add_box(&mut v, &frame, Vec3::new(x0 - 0.03, shoulder - 0.28, -0.155), Vec3::new(x0 + 0.28, shoulder + 0.03, 0.155), pivot, ang, armor_color(m));
        }
        if side > 0.0 && !swim {
            add_held(&mut v, &frame, Vec3::new(x0 + 0.125, ty0 + 0.02, 0.0), pivot, ang, pose.held);
        }
    }
    // Armour over the body.
    if let Some(m) = pose.armor[1] {
        add_box(&mut v, &frame, Vec3::new(-0.28, ty0 + 0.06, -0.155), Vec3::new(0.28, ty0 + 0.66, 0.155), Vec3::ZERO, 0.0, armor_color(m));
    }
    if pose.armor[2].is_some() || pose.armor[3].is_some() {
        for (side, lang) in [(-1.0f32, swing), (1.0, -swing)] {
            let x0 = if side < 0.0 { -0.25 } else { 0.0 };
            let lp = Vec3::new(x0 + 0.125, hip, 0.0);
            if let Some(m) = pose.armor[2] {
                add_box(&mut v, &frame, Vec3::new(x0 - 0.02, 0.22, -0.145), Vec3::new(x0 + 0.27, hip + 0.02, 0.145), lp, lang, armor_color(m));
            }
            if let Some(m) = pose.armor[3] {
                add_box(&mut v, &frame, Vec3::new(x0 - 0.03, -0.01, -0.15), Vec3::new(x0 + 0.28, 0.22, 0.175), lp, lang, armor_color(m));
            }
        }
    }
    // Head follows the look pitch, pivoting at the neck.
    let neck = ty0 + 0.65;
    let head_ang = if swim { -1.3 - pose.pitch * 0.8 } else { -pose.pitch * 0.9 };
    add_box(&mut v, &frame, Vec3::new(-0.25, neck, -0.25), Vec3::new(0.25, neck + 0.5, 0.25), Vec3::new(0.0, neck, 0.0), head_ang, skin_c);
    add_box(&mut v, &frame, Vec3::new(-0.26, neck + 0.38, -0.26), Vec3::new(0.26, neck + 0.52, 0.26), Vec3::new(0.0, neck, 0.0), head_ang, hair_c);
    for ex in [-0.14f32, 0.06] {
        add_box(&mut v, &frame, Vec3::new(ex, neck + 0.22, 0.25), Vec3::new(ex + 0.08, neck + 0.30, 0.262), Vec3::new(0.0, neck, 0.0), head_ang, EYE);
    }
    if let Some(m) = pose.armor[0] {
        // Helmet: cap over the top and the back of the head, the face stays free.
        let c = armor_color(m);
        let hp = Vec3::new(0.0, neck, 0.0);
        add_box(&mut v, &frame, Vec3::new(-0.285, neck + 0.36, -0.285), Vec3::new(0.285, neck + 0.56, 0.285), hp, head_ang, c);
        add_box(&mut v, &frame, Vec3::new(-0.285, neck + 0.08, -0.285), Vec3::new(0.285, neck + 0.36, -0.12), hp, head_ang, c);
        for sx in [-0.285f32, 0.215] {
            add_box(&mut v, &frame, Vec3::new(sx, neck + 0.12, -0.285), Vec3::new(sx + 0.07, neck + 0.36, 0.2), hp, head_ang, c);
        }
    }
    v
}

/// Draws the held item at the hand `hand` (before the arm rotation `angle` about `pivot`).
/// Tools and items are 3D pixel icons pointing forward and up, blocks a small cube.
fn add_held(v: &mut Vec<ModelVertex>, frame: &Frame, hand: Vec3, pivot: Vec3, angle: f32, held: Held) {
    let sprite = match held {
        Held::Nothing => return,
        Held::Block(c) => {
            let h = 0.13;
            let c0 = hand + Vec3::new(0.0, -0.05, 0.12);
            add_box(v, frame, c0 - Vec3::splat(h), c0 + Vec3::splat(h), pivot, angle, c);
            return;
        }
        Held::Tool(kind, mat) => wolfcraft_assets::items::TOOLS_BASE as usize + mat as usize * 4 + kind as usize,
        Held::Item(i) => i as usize,
    };
    let Some(px) = textures().and_then(|t| t.item_pixels.get(sprite).cloned()).filter(|p| p.len() == 256) else {
        if let Held::Tool(kind, mat) = held {
            // No icon pixels (tests): the simple box tool, pointing forward.
            add_tool(v, frame, hand + Vec3::new(0.0, 0.0, 0.05), pivot, angle + 1.4, kind, mat);
        }
        return;
    };
    // Icon plane: image right = backwards, image up = up. Our tool icons run from the handle
    // (bottom-right) to the head (top-left), so the head points forward and up from the fist.
    let (u, w) = (Vec3::NEG_Z, Vec3::Y);
    let n = u.cross(w);
    let s = if matches!(held, Held::Tool(..)) { 0.62 } else { 0.42 };
    let grip = if matches!(held, Held::Tool(..)) { (12.5, 12.5) } else { (8.0, 12.0) };
    let o = hand - u * (grip.0 / 16.0 * s) - w * ((16.0 - grip.1) / 16.0 * s) - n * 0.02;
    add_voxel_icon(v, frame, &px, o, u, w, n, s, pivot, angle);
}

/// Builds all creatures relative to `camera` (original blocky designs, flat colours).
/// `forward`: camera view direction; mobs behind the camera or beyond `max_dist` are skipped
/// (their triangles would be thrown away by the GPU anyway).
pub fn build_mob_models(mobs: &[crate::mobs::Mob], camera: DVec3, forward: DVec3, max_dist: f64) -> Vec<ModelVertex> {
    use crate::mobs::MobKind;
    let mut v = Vec::with_capacity(mobs.len() * 10 * 36);
    for m in mobs {
        let to = m.body.pos - camera;
        if to.length_squared() > max_dist * max_dist || (to.dot(forward) < -2.0 && to.length_squared() > 9.0) {
            continue;
        }
        let up = crate::physics::up_at(m.body.pos);
        let fwd = (m.heading - up * m.heading.dot(up)).normalize_or_zero();
        if fwd.length_squared() < 0.5 {
            continue;
        }
        let right = fwd.cross(up).normalize_or_zero();
        let f = Frame { origin: m.body.pos - camera, right, up, fwd };
        let swing = m.anim.sin() * 0.7 * m.amount;
        // Red flash when hurt.
        let tint = |c: [f32; 3]| -> [f32; 3] {
            let k = (m.hurt / 0.35).clamp(0.0, 1.0) * 0.6;
            [c[0] + (1.0 - c[0]) * k, c[1] * (1.0 - k), c[2] * (1.0 - k)]
        };
        let z = Vec3::ZERO;
        match m.kind {
            MobKind::Ghoul => {
                // Hunched, ragged walker with glowing eyes.
                let skin = tint([0.47, 0.60, 0.42]);
                let skin_d = tint([0.36, 0.47, 0.32]);
                let shirt = tint([0.32, 0.36, 0.52]);
                let rag = tint([0.24, 0.22, 0.30]);
                let glow = [1.0, 0.85, 0.25];
                for (side, a) in [(-1.0f32, swing), (1.0, -swing)] {
                    let x0 = if side < 0.0 { -0.24 } else { 0.01 };
                    add_box(&mut v, &f, Vec3::new(x0, 0.0, -0.12), Vec3::new(x0 + 0.23, 0.72, 0.12), Vec3::new(x0 + 0.115, 0.72, 0.0), a, rag);
                    add_box(&mut v, &f, Vec3::new(x0 - 0.01, 0.0, -0.13), Vec3::new(x0 + 0.24, 0.14, 0.16), Vec3::new(x0 + 0.115, 0.72, 0.0), a, skin_d);
                }
                // Torso leans forward a little.
                let lean = 0.18;
                add_box(&mut v, &f, Vec3::new(-0.26, 0.72, -0.14), Vec3::new(0.26, 1.36, 0.14), Vec3::new(0.0, 0.72, 0.0), lean, shirt);
                add_box(&mut v, &f, Vec3::new(-0.27, 0.72, -0.15), Vec3::new(0.27, 0.86, 0.15), Vec3::new(0.0, 0.72, 0.0), lean, rag);
                for x0 in [-0.5f32, 0.26] {
                    add_box(&mut v, &f, Vec3::new(x0, 0.72, -0.11), Vec3::new(x0 + 0.24, 1.32, 0.11), Vec3::new(x0 + 0.12, 1.3, 0.0), 1.3 + swing * 0.25, skin);
                    add_box(&mut v, &f, Vec3::new(x0 - 0.01, 1.1, -0.12), Vec3::new(x0 + 0.25, 1.33, 0.12), Vec3::new(x0 + 0.12, 1.3, 0.0), 1.3 + swing * 0.25, shirt);
                }
                let hp = Vec3::new(0.0, 1.36, 0.08);
                let ha = lean + 0.1 + (m.anim * 0.5).sin() * 0.06;
                add_box(&mut v, &f, Vec3::new(-0.24, 1.36, -0.16), Vec3::new(0.24, 1.84, 0.32), hp, ha, skin);
                add_box(&mut v, &f, Vec3::new(-0.25, 1.74, -0.17), Vec3::new(0.25, 1.86, 0.33), hp, ha, skin_d);
                for ex in [-0.16f32, 0.06] {
                    add_box(&mut v, &f, Vec3::new(ex, 1.58, 0.32), Vec3::new(ex + 0.1, 1.66, 0.335), hp, ha, glow);
                }
                add_box(&mut v, &f, Vec3::new(-0.1, 1.44, 0.32), Vec3::new(0.1, 1.49, 0.335), hp, ha, [0.15, 0.05, 0.05]);
            }
            MobKind::Crawler => {
                // Eight-legged crawler: round abdomen, head with a cluster of eyes.
                let body = tint([0.26, 0.17, 0.34]);
                let back = tint([0.38, 0.22, 0.46]);
                let leg = tint([0.17, 0.11, 0.22]);
                let eye = [1.0, 0.25, 0.20];
                add_box(&mut v, &f, Vec3::new(-0.32, 0.28, -0.62), Vec3::new(0.32, 0.78, 0.0), z, 0.0, body);
                add_box(&mut v, &f, Vec3::new(-0.2, 0.78, -0.5), Vec3::new(0.2, 0.82, -0.12), z, 0.0, back);
                add_box(&mut v, &f, Vec3::new(-0.2, 0.3, 0.0), Vec3::new(0.2, 0.6, 0.22), z, 0.0, body);
                add_box(&mut v, &f, Vec3::new(-0.22, 0.32, 0.22), Vec3::new(0.22, 0.66, 0.56), z, 0.0, body);
                for (i, zz) in [-0.1f32, 0.02, 0.14, 0.26].iter().enumerate() {
                    let a = if i % 2 == 0 { swing } else { -swing } * 0.8;
                    let spread = 0.18 + i as f32 * 0.06;
                    add_box(&mut v, &f, Vec3::new(-0.62, 0.0, *zz - spread * 0.3), Vec3::new(-0.2, 0.08, *zz - spread * 0.3 + 0.07), Vec3::new(-0.2, 0.45, *zz), a, leg);
                    add_box(&mut v, &f, Vec3::new(-0.6, 0.0, *zz - spread * 0.3), Vec3::new(-0.52, 0.45, *zz - spread * 0.3 + 0.07), Vec3::new(-0.2, 0.45, *zz), a, leg);
                    add_box(&mut v, &f, Vec3::new(0.2, 0.0, *zz - spread * 0.3), Vec3::new(0.62, 0.08, *zz - spread * 0.3 + 0.07), Vec3::new(0.2, 0.45, *zz), -a, leg);
                    add_box(&mut v, &f, Vec3::new(0.52, 0.0, *zz - spread * 0.3), Vec3::new(0.6, 0.45, *zz - spread * 0.3 + 0.07), Vec3::new(0.2, 0.45, *zz), -a, leg);
                }
                for (ex, ey, sz) in [(-0.15f32, 0.5f32, 0.08f32), (0.07, 0.5, 0.08), (-0.08, 0.58, 0.05), (0.03, 0.58, 0.05), (-0.17, 0.4, 0.04), (0.13, 0.4, 0.04)] {
                    add_box(&mut v, &f, Vec3::new(ex, ey, 0.56), Vec3::new(ex + sz, ey + sz, 0.572), z, 0.0, eye);
                }
            }
            MobKind::Boar | MobKind::Sheep => {
                let sheep = m.kind == MobKind::Sheep;
                let bob = (m.anim * 2.0).sin().abs() * 0.03 * m.amount;
                if sheep {
                    // Fluffy sheep: big wool body, wool cap, cream face.
                    let wool = tint([0.96, 0.95, 0.90]);
                    let wool_d = tint([0.86, 0.85, 0.80]);
                    let face = tint([0.93, 0.82, 0.72]);
                    let leg = tint([0.32, 0.28, 0.27]);
                    let h = 0.36;
                    for (x0, z0, a) in [(-0.27f32, -0.42f32, swing), (0.09, -0.42, -swing), (-0.27, 0.22, -swing), (0.09, 0.22, swing)] {
                        add_box(&mut v, &f, Vec3::new(x0, 0.0, z0), Vec3::new(x0 + 0.18, h + 0.06, z0 + 0.18), Vec3::new(x0 + 0.09, h, z0 + 0.09), a, leg);
                    }
                    add_box(&mut v, &f, Vec3::new(-0.36, h + bob, -0.55), Vec3::new(0.36, h + 0.62 + bob, 0.48), z, 0.0, wool);
                    add_box(&mut v, &f, Vec3::new(-0.3, h + 0.62 + bob, -0.45), Vec3::new(0.3, h + 0.7 + bob, 0.38), z, 0.0, wool_d);
                    add_box(&mut v, &f, Vec3::new(-0.08, h + 0.4 + bob, -0.62), Vec3::new(0.08, h + 0.56 + bob, -0.55), z, 0.0, wool_d);
                    let hz = 0.48;
                    let hy = h + 0.36 + bob;
                    add_box(&mut v, &f, Vec3::new(-0.18, hy, hz), Vec3::new(0.18, hy + 0.36, hz + 0.32), z, 0.0, face);
                    add_box(&mut v, &f, Vec3::new(-0.2, hy + 0.28, hz - 0.02), Vec3::new(0.2, hy + 0.42, hz + 0.24), z, 0.0, wool);
                    for ex in [-0.3f32, 0.18] {
                        add_box(&mut v, &f, Vec3::new(ex, hy + 0.2, hz + 0.08), Vec3::new(ex + 0.12, hy + 0.27, hz + 0.18), z, 0.0, face);
                    }
                    for ex in [-0.13f32, 0.05] {
                        add_box(&mut v, &f, Vec3::new(ex, hy + 0.17, hz + 0.32), Vec3::new(ex + 0.08, hy + 0.25, hz + 0.332), z, 0.0, [0.08, 0.07, 0.09]);
                        add_box(&mut v, &f, Vec3::new(ex + 0.05, hy + 0.22, hz + 0.333), Vec3::new(ex + 0.075, hy + 0.245, hz + 0.336), z, 0.0, [1.0, 1.0, 1.0]);
                    }
                    add_box(&mut v, &f, Vec3::new(-0.05, hy + 0.06, hz + 0.32), Vec3::new(0.05, hy + 0.11, hz + 0.333), z, 0.0, [0.92, 0.55, 0.60]);
                } else {
                    // Boar: dark bristly back, pink snout, tusks.
                    let hide = tint([0.55, 0.36, 0.26]);
                    let dark = tint([0.36, 0.23, 0.17]);
                    let snout = tint([0.88, 0.62, 0.56]);
                    let leg = tint([0.38, 0.25, 0.19]);
                    let h = 0.3;
                    for (x0, z0, a) in [(-0.28f32, -0.42f32, swing), (0.08, -0.42, -swing), (-0.28, 0.22, -swing), (0.08, 0.22, swing)] {
                        add_box(&mut v, &f, Vec3::new(x0, 0.0, z0), Vec3::new(x0 + 0.2, h + 0.05, z0 + 0.2), Vec3::new(x0 + 0.1, h, z0 + 0.1), a, leg);
                    }
                    add_box(&mut v, &f, Vec3::new(-0.32, h + bob, -0.52), Vec3::new(0.32, h + 0.56 + bob, 0.46), z, 0.0, hide);
                    add_box(&mut v, &f, Vec3::new(-0.1, h + 0.56 + bob, -0.48), Vec3::new(0.1, h + 0.64 + bob, 0.44), z, 0.0, dark);
                    add_box(&mut v, &f, Vec3::new(-0.03, h + 0.3 + bob, -0.6), Vec3::new(0.03, h + 0.46 + bob, -0.52), z, 0.0, dark);
                    let hz = 0.46;
                    let hy = h + 0.2 + bob;
                    add_box(&mut v, &f, Vec3::new(-0.22, hy, hz), Vec3::new(0.22, hy + 0.38, hz + 0.3), z, 0.0, hide);
                    add_box(&mut v, &f, Vec3::new(-0.12, hy + 0.04, hz + 0.3), Vec3::new(0.12, hy + 0.2, hz + 0.4), z, 0.0, snout);
                    for nx in [-0.07f32, 0.03] {
                        add_box(&mut v, &f, Vec3::new(nx, hy + 0.09, hz + 0.4), Vec3::new(nx + 0.04, hy + 0.14, hz + 0.405), z, 0.0, [0.35, 0.18, 0.16]);
                    }
                    for tx in [-0.17f32, 0.13] {
                        add_box(&mut v, &f, Vec3::new(tx, hy + 0.02, hz + 0.28), Vec3::new(tx + 0.04, hy + 0.14, hz + 0.33), z, 0.0, [0.96, 0.94, 0.86]);
                    }
                    for ex in [-0.24f32, 0.14] {
                        add_box(&mut v, &f, Vec3::new(ex, hy + 0.34, hz + 0.04), Vec3::new(ex + 0.1, hy + 0.46, hz + 0.12), z, 0.0, dark);
                    }
                    for ex in [-0.15f32, 0.07] {
                        add_box(&mut v, &f, Vec3::new(ex, hy + 0.24, hz + 0.3), Vec3::new(ex + 0.08, hy + 0.31, hz + 0.312), z, 0.0, [0.06, 0.05, 0.05]);
                        add_box(&mut v, &f, Vec3::new(ex + 0.05, hy + 0.28, hz + 0.313), Vec3::new(ex + 0.075, hy + 0.305, hz + 0.316), z, 0.0, [1.0, 1.0, 1.0]);
                    }
                }
            }
        }
    }
    v
}

/// Items lying in the world: blocks as small spinning textured cubes, other items as 3D
/// pixel icons; a stack shows a second copy behind the first. Bob up and down a little.
pub fn build_dropped_models(drops: &[crate::drops::Dropped], camera: DVec3, time: f32, max_dist: f64) -> Vec<ModelVertex> {
    let mut v = Vec::new();
    let tex = textures();
    for d in drops {
        if (d.pos - camera).length_squared() > max_dist * max_dist {
            continue;
        }
        let up = crate::physics::up_at(d.pos);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let r0 = up.cross(helper).normalize();
        let f0 = r0.cross(up);
        // In the air it spins and bobs; on the ground it lies still at its own angle.
        let a = if d.resting { (d.seed * 6.28) as f64 } else { (time * 1.6 + d.seed * 6.28) as f64 };
        let (s, c) = a.sin_cos();
        let right = r0 * c + f0 * s;
        let fwd = right.cross(up) * -1.0;
        let bob = if d.resting { -0.115 } else { 0.13 + ((time * 2.4 + d.seed * 9.0).sin() * 0.05) as f64 };
        let copies = if d.stack.count > 1 { 2 } else { 1 };
        let info = d.stack.info();
        let block = info.place.filter(|b| matches!(b.info().render, crate::block::RenderKind::Opaque | crate::block::RenderKind::Cutout));
        for k in 0..copies {
            // A stack shows a second copy: beside the first on the ground, behind it in the air.
            let off = if d.resting { right * (k as f64 * 0.11) + fwd * (k as f64 * 0.08) + up * (k as f64 * 0.012) } else { right * (k as f64 * 0.07) + fwd * (k as f64 * 0.06) + up * (k as f64 * 0.05) };
            let frame = Frame { origin: d.pos - camera + up * bob + off, right, up, fwd };
            if let Some(b) = block {
                add_tiled_cube(&mut v, &frame, 0.12, b.info().tiles);
            } else {
                let px = match info.icon {
                    crate::items::Icon::Item(i) => tex.as_ref().and_then(|t| t.item_pixels.get(i as usize).cloned()),
                    crate::items::Icon::Block(_) => None,
                };
                if let Some(px) = px.filter(|p| p.len() == 256) {
                    let s = 0.34f32;
                    if d.resting {
                        // Lying flat on the ground, one pixel thick.
                        let (u, w) = (Vec3::X, Vec3::NEG_Z);
                        add_voxel_icon(&mut v, &frame, &px, Vec3::new(-s * 0.5, s / 16.0 + 0.004, s * 0.5), u, w, u.cross(w), s, Vec3::ZERO, 0.0);
                    } else {
                        add_voxel_icon(&mut v, &frame, &px, Vec3::new(-s * 0.5, 0.0, 0.0), Vec3::X, Vec3::Y, Vec3::NEG_Z, s, Vec3::ZERO, 0.0);
                    }
                } else if let crate::items::Icon::Block(t) = info.icon {
                    // Plants and other non-cube blocks: a two-sided card with the block sprite.
                    add_tiled_card(&mut v, &frame, 0.3, t);
                }
            }
        }
    }
    v
}

/// Small cube textured with block sprites (`tiles` in +X, -X, +Y, -Y, +Z, -Z order), drawn by
/// the model shader from the blocks atlas (`tex` 4, sprite id in `color[0]`).
fn add_tiled_cube(out: &mut Vec<ModelVertex>, f: &Frame, h: f32, tiles: [u16; 6]) {
    let faces: [(usize, Vec3, Vec3, Vec3); 6] = [
        (0, Vec3::X, Vec3::Z, Vec3::Y),
        (1, Vec3::NEG_X, Vec3::NEG_Z, Vec3::Y),
        (2, Vec3::Y, Vec3::X, Vec3::NEG_Z),
        (3, Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (4, Vec3::Z, Vec3::NEG_X, Vec3::Y),
        (5, Vec3::NEG_Z, Vec3::X, Vec3::Y),
    ];
    let c = Vec3::new(0.0, h, 0.0);
    for (ti, n, ua, va) in faces {
        let corner = |u: f32, w: f32| c + (n + ua * (u * 2.0 - 1.0) + va * (w * 2.0 - 1.0)) * h;
        let p = [corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)];
        let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let nw = f.dir(n).normalize_or_zero().to_array();
        let color = [tiles[ti] as f32, 0.0, 0.0];
        // Both windings: tiny and always seen from outside, so culling stays simple.
        for i in [0usize, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] {
            out.push(ModelVertex { pos: f.to_world(p[i]).as_vec3().to_array(), normal: nw, color, uv: uv[i], tex: 4.0 });
        }
    }
}

/// Upright two-sided card with one block sprite (flowers, torches…).
fn add_tiled_card(out: &mut Vec<ModelVertex>, f: &Frame, s: f32, tile: u16) {
    let p = [Vec3::new(-s * 0.5, 0.0, 0.0), Vec3::new(s * 0.5, 0.0, 0.0), Vec3::new(s * 0.5, s, 0.0), Vec3::new(-s * 0.5, s, 0.0)];
    let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let nw = f.dir(Vec3::NEG_Z).normalize_or_zero().to_array();
    for i in [0usize, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] {
        out.push(ModelVertex { pos: f.to_world(p[i]).as_vec3().to_array(), normal: nw, color: [tile as f32, 0.0, 0.0], uv: uv[i], tex: 4.0 });
    }
}

/// Arrows in flight or stuck in blocks: `(tip position, direction)` relative to nothing
/// (world space); drawn relative to `camera`.
pub fn build_arrow_models(arrows: &[(DVec3, DVec3)], camera: DVec3) -> Vec<ModelVertex> {
    let mut v = Vec::with_capacity(arrows.len() * 3 * 36);
    for (pos, dir) in arrows {
        let fwd = dir.normalize_or_zero();
        if fwd.length_squared() < 0.5 {
            continue;
        }
        let helper = if fwd.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let right = fwd.cross(helper).normalize();
        let up = right.cross(fwd);
        let f = Frame { origin: *pos - camera, right, up, fwd };
        let z = Vec3::ZERO;
        add_box(&mut v, &f, Vec3::new(-0.025, -0.025, -0.62), Vec3::new(0.025, 0.025, -0.06), z, 0.0, [0.55, 0.40, 0.22]);
        add_box(&mut v, &f, Vec3::new(-0.045, -0.045, -0.08), Vec3::new(0.045, 0.045, 0.02), z, 0.0, [0.72, 0.74, 0.78]);
        add_box(&mut v, &f, Vec3::new(-0.08, -0.008, -0.62), Vec3::new(0.08, 0.008, -0.48), z, 0.0, [0.94, 0.94, 0.92]);
        add_box(&mut v, &f, Vec3::new(-0.008, -0.08, -0.62), Vec3::new(0.008, 0.08, -0.48), z, 0.0, [0.94, 0.94, 0.92]);
    }
    v
}

/// First-person arms: camera basis and animation state.
///
/// The animation set follows the Bedrock animation packs: the right arm is always in view, one
/// swing per click that always plays to the end, the hand lags behind camera turns, bobs in a
/// figure eight while walking, drops and tilts while sprinting, lifts on a jump and dips on
/// landing.
#[derive(Clone, Copy, Debug)]
pub struct HandsPose {
    pub forward: Vec3,
    pub up: Vec3,
    /// Swing progress 0..1 of the current strike (0 = resting, no swing).
    pub punch_cycle: f32,
    /// Size of the animations, 0..1 (setting "Размах анимации").
    pub guard: f32,
    /// Walk bob phase (radians, π per step) and amount (0..1).
    pub bob_phase: f32,
    pub bob_amount: f32,
    /// Something is selected in the hotbar: the right hand holds it instead of being empty.
    pub holding: bool,
    /// Tool in the main hand: (kind 0 pickaxe, 1 axe, 2 shovel, 3 sword; material 0..3).
    pub tool: Option<(u8, u8)>,
    /// Other (non-block, non-tool) item in the main hand: its items-atlas sprite.
    pub icon: Option<u16>,
    /// Shield in the second hand (left arm stays in view with it).
    pub shield: bool,
    /// Shield raise blend 0..1 (RMB held).
    pub blocking: f32,
    /// Block-placing motion: 1 right after placing, fading to 0.
    pub place: f32,
    /// Skin preset of the arm (see `SKIN_NAMES`).
    pub skin: u8,
    /// Sprint blend 0..1: arm lower and turned in.
    pub sprint: f32,
    /// Sneak blend 0..1: arm a little lower and closer.
    pub sneak: f32,
    /// Vertical lag of the hand (camera-local units, + up): trails jumps and dips on landing.
    pub lift: f32,
    /// Inertia: how far the hand trails behind a camera turn (x right, y up), roughly −1..1.
    pub sway: [f32; 2],
    /// Seconds since start, for idle breathing.
    pub time: f32,
    /// Colour of the worn chestplate: the sleeve takes it.
    pub sleeve: Option<[f32; 3]>,
    /// A bow in the main hand and how far it is drawn (0..1).
    pub bow: Option<f32>,
}

impl HandsPose {
    /// Resting pose looking along `forward` (handy for tests and previews).
    pub fn rest(forward: Vec3, up: Vec3) -> Self {
        HandsPose { forward, up, punch_cycle: 0.0, guard: 1.0, bob_phase: 0.0, bob_amount: 0.0, holding: false, tool: None, icon: None, shield: false, blocking: 0.0, place: 0.0, skin: 0, sprint: 0.0, sneak: 0.0, lift: 0.0, sway: [0.0; 2], time: 0.0, sleeve: None, bow: None }
    }
}

/// Swing of a held tool/weapon, 0 (rest) → 1 (struck) → 0 over `t` 0..1: a short wind-up
/// backwards (to about -0.18), a fast eased hit that reaches 1 at `t = 0.5`, then a soft
/// recovery back to 0. Continuous at both ends.
pub fn strike_curve(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let ss = |x: f32| x * x * (3.0 - 2.0 * x);
    if t < 0.30 {
        -0.18 * ss(t / 0.30)
    } else if t <= 0.50 {
        let x = ((t - 0.30) / 0.20).clamp(0.0, 1.0);
        let e = 1.0 - (1.0 - x).powi(3);
        -0.18 * (1.0 - e) + e
    } else {
        1.0 - ss((t - 0.50) / 0.50)
    }
}

/// Moment of a swing (0..1) when the blow lands: chips fly and the hit sound plays.
pub fn strike_moment(tool: bool) -> f32 {
    if tool { 0.5 } else { 0.35 }
}

/// Swings per second: a fist is quick, tools are heavier.
pub fn swing_speed(tool: Option<(u8, u8)>) -> f32 {
    match tool.map(|t| t.0) {
        None => 3.3,
        Some(3) => 2.8, // sword
        Some(2) => 2.4, // shovel
        Some(_) => 2.0, // pickaxe, axe
    }
}

const TOOL_MATERIALS: [[f32; 3]; 4] = [[0.62, 0.45, 0.25], [0.55, 0.55, 0.55], [0.85, 0.85, 0.88], [0.35, 0.90, 0.85]];
const HANDLE: [f32; 3] = [0.45, 0.32, 0.18];
const PLANK: [f32; 3] = [0.62, 0.45, 0.25];
const IRON: [f32; 3] = [0.62, 0.64, 0.68];

/// One keyframe of a hand animation: time in the swing (0..1), hand offset from the resting
/// pose (camera-local x right, y up, z forward), forearm tilt (+ tip down), yaw (+ towards the
/// view centre) and roll (+ top of the hand towards the centre).
#[derive(Clone, Copy, Debug)]
struct AnimKey {
    t: f32,
    x: f32,
    y: f32,
    z: f32,
    tilt: f32,
    yaw: f32,
    roll: f32,
}

#[allow(clippy::too_many_arguments)]
const fn key(t: f32, x: f32, y: f32, z: f32, tilt: f32, yaw: f32, roll: f32) -> AnimKey {
    AnimKey { t, x, y, z, tilt, yaw, roll }
}

const REST: f32 = 0.0;

/// Empty hand: the Minecraft punch, a quick arc in towards the crosshair and back.
const FIST_ANIM: &[AnimKey] = &[
    key(0.00, REST, REST, REST, REST, REST, REST),
    key(0.12, -0.04, 0.05, 0.06, -0.20, 0.25, 0.10),
    key(0.35, -0.17, 0.07, 0.18, -0.30, 0.55, 0.25),
    key(0.55, -0.12, 0.00, 0.10, -0.05, 0.40, 0.15),
    key(1.00, REST, REST, REST, REST, REST, REST),
];

/// Tool strikes, every one starts and ends at rest so a swing blends in from the idle pose and
/// repeats without a jump while mining. Order: pickaxe, axe, shovel, sword.
const TOOL_ANIMS: [&[AnimKey]; 4] = [
    // Pickaxe: raised high over the shoulder, a heavy blow down into the block, a jolt from
    // the impact, then back.
    &[
        key(0.00, REST, REST, REST, REST, REST, REST),
        key(0.28, 0.02, 0.12, -0.04, -1.05, 0.05, 0.00),
        key(0.48, -0.07, -0.05, 0.15, 0.62, 0.24, 0.10),
        key(0.56, -0.07, -0.03, 0.13, 0.50, 0.22, 0.08),
        key(0.66, -0.07, -0.04, 0.13, 0.56, 0.22, 0.08),
        key(1.00, REST, REST, REST, REST, REST, REST),
    ],
    // Axe: wide chop on a diagonal from the upper right to the lower left.
    &[
        key(0.00, REST, REST, REST, REST, REST, REST),
        key(0.28, 0.08, 0.12, -0.03, -1.00, -0.30, -0.35),
        key(0.48, -0.16, -0.05, 0.13, 0.60, 0.50, 0.45),
        key(0.62, -0.16, -0.05, 0.11, 0.52, 0.46, 0.40),
        key(1.00, REST, REST, REST, REST, REST, REST),
    ],
    // Shovel: lifted, driven down into the ground blade first, then a scoop up and back.
    &[
        key(0.00, REST, REST, REST, REST, REST, REST),
        key(0.26, 0.00, 0.08, -0.06, -0.55, 0.00, 0.00),
        key(0.48, -0.05, -0.06, 0.18, 0.72, 0.14, 0.05),
        key(0.62, -0.05, -0.05, 0.17, 0.66, 0.14, 0.05),
        key(0.80, -0.02, 0.02, 0.06, -0.35, 0.05, 0.00),
        key(1.00, REST, REST, REST, REST, REST, REST),
    ],
    // Sword: wide diagonal slash across the view.
    &[
        key(0.00, REST, REST, REST, REST, REST, REST),
        key(0.22, 0.08, 0.10, -0.02, -0.65, -0.35, -0.35),
        key(0.45, -0.24, -0.06, 0.12, 0.55, 0.85, 0.55),
        key(0.60, -0.24, -0.07, 0.10, 0.48, 0.80, 0.50),
        key(1.00, REST, REST, REST, REST, REST, REST),
    ],
];

/// Pose of an animation at `t` (0..1): eased blend between the two surrounding keys.
fn sample_anim(keys: &[AnimKey], t: f32) -> AnimKey {
    let t = t.clamp(0.0, 1.0);
    let i = keys.iter().rposition(|k| k.t <= t).unwrap_or(0).min(keys.len() - 2);
    let (a, b) = (keys[i], keys[i + 1]);
    let f = ((t - a.t) / (b.t - a.t).max(1e-4)).clamp(0.0, 1.0);
    let f = f * f * (3.0 - 2.0 * f);
    let l = |p: f32, q: f32| p + (q - p) * f;
    key(t, l(a.x, b.x), l(a.y, b.y), l(a.z, b.z), l(a.tilt, b.tilt), l(a.yaw, b.yaw), l(a.roll, b.roll))
}

/// Where one arm is this frame: shoulder-side start of the forearm (camera-local), its tilt
/// about the local X axis, and the yaw/roll of the whole hand around that point.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ArmPlace {
    pos: Vec3,
    tilt: f32,
    yaw: f32,
    roll: f32,
}

/// Motion shared by everything in view: walk bob, breathing, turn inertia, jump lag, sprint
/// and sneak. Returns (offset, tilt, yaw, roll).
fn body_motion(pose: &HandsPose) -> (Vec3, f32, f32, f32) {
    let k = pose.guard.clamp(0.0, 1.0);
    let b = pose.bob_amount.clamp(0.0, 1.0) * (1.0 + 0.6 * pose.sprint.clamp(0.0, 1.0));
    // Figure eight: one side-to-side sway per two steps, a dip on every step.
    let ph = pose.bob_phase;
    let bob = Vec3::new(ph.cos() * 0.016 * b, -ph.sin().abs() * 0.022 * b, 0.0);
    let bob_roll = ph.cos() * 0.05 * b;
    let breath = Vec3::new((pose.time * 1.3).cos() * 0.003, (pose.time * 1.9).sin() * 0.005, 0.0) * (1.0 - pose.bob_amount.clamp(0.0, 1.0));
    let sway = Vec3::new(pose.sway[0].clamp(-1.0, 1.0) * 0.05, pose.sway[1].clamp(-1.0, 1.0) * 0.04, 0.0);
    let lift = Vec3::new(0.0, pose.lift.clamp(-0.08, 0.08), 0.0);
    let s = pose.sprint.clamp(0.0, 1.0);
    let n = pose.sneak.clamp(0.0, 1.0);
    let sprint = Vec3::new(-0.02 * s, -0.035 * s, -0.02 * s);
    let sneak = Vec3::new(-0.01 * n, -0.03 * n, 0.0);
    let off = (bob + sway + lift) * k + breath + sprint + sneak;
    let tilt = 0.15 * s + pose.sway[1].clamp(-1.0, 1.0) * 0.10 * k;
    let yaw = 0.20 * s - pose.sway[0].clamp(-1.0, 1.0) * 0.12 * k;
    let roll = 0.30 * s + bob_roll * k;
    (off, tilt, yaw, roll)
}

fn arm_place(pose: &HandsPose, side: f32) -> ArmPlace {
    let k = pose.guard.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let (off, btilt, byaw, broll) = body_motion(pose);
    let swing = pose.punch_cycle;
    let swinging = swing > 0.0 && swing < 1.0;
    if side > 0.0 {
        // Resting place of the right hand: low on the right, like in Minecraft.
        let mut p = Vec3::new(0.30, -0.30, 0.42) + off;
        let mut a = ArmPlace { pos: p, tilt: 0.2 + btilt, yaw: byaw, roll: broll };
        if swinging {
            let keys = match pose.tool {
                // Hoes (kind 4) chop down like a pickaxe.
                Some((kind, _)) => TOOL_ANIMS[if kind == 4 { 0 } else { (kind as usize).min(TOOL_ANIMS.len() - 1) }],
                None => FIST_ANIM,
            };
            let s = sample_anim(keys, swing);
            p += Vec3::new(s.x, s.y, s.z) * k;
            a.tilt += s.tilt * k;
            a.yaw += s.yaw * k;
            a.roll += s.roll * k;
        }
        if let Some(c) = pose.bow {
            // Drawing a bow: the hand comes in front of the chest, the bow stands up and leans
            // a little, the forearm levels out.
            let c = c.clamp(0.0, 1.0);
            let e = c * c * (3.0 - 2.0 * c);
            p = p.lerp(Vec3::new(0.05, -0.17, 0.34) + off * 0.4, e);
            a.tilt = lerp(a.tilt, -0.05, e);
            a.yaw = lerp(a.yaw, 0.35, e);
            a.roll = lerp(a.roll, -0.25, e);
            // A small tremble when fully drawn.
            if c >= 1.0 {
                p.y += (pose.time * 31.0).sin() * 0.003;
            }
        }
        if pose.place > 0.0 {
            // The right hand pushes forward and down quickly, then eases back with a recoil.
            let t = 1.0 - pose.place.clamp(0.0, 1.0);
            let ss = |x: f32| x * x * (3.0 - 2.0 * x);
            let e = if t < 0.3 { 1.0 - (1.0 - t / 0.3).powi(3) } else { 1.0 - ss((t - 0.3) / 0.7) };
            let recoil = (t * std::f32::consts::PI).sin() * (1.0 - t) * 0.03;
            p += Vec3::new(-0.03 * e, -0.08 * e + recoil, 0.17 * e - recoil) * k.max(0.4);
            a.tilt -= 0.5 * e * k.max(0.4);
        }
        a.pos = p;
        return a;
    }
    // Left arm: below the screen unless it carries a shield.
    if pose.shield {
        let b = pose.blocking.clamp(0.0, 1.0);
        // Raised shield stays left of the crosshair so the target remains visible.
        let x = lerp(-0.34, -0.19, b);
        let y = lerp(-0.36, -0.25, b);
        return ArmPlace { pos: Vec3::new(x, y, 0.42) + off, tilt: lerp(0.25, -0.05, b) + btilt * (1.0 - b), yaw: -byaw * (1.0 - b), roll: -broll * (1.0 - b) };
    }
    ArmPlace { pos: Vec3::new(-0.34, -0.80, 0.40), tilt: 0.25, yaw: 0.0, roll: 0.0 }
}

/// Forearm tilt about the local X axis through `pivot` (+ tips the forearm down).
fn tilt_point(p: Vec3, pivot: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    let d = p - pivot;
    pivot + Vec3::new(d.x, d.y * c - d.z * s, d.y * s + d.z * c)
}

/// Yaw and roll of a whole hand around `pivot`, in camera-local coordinates. `side` mirrors
/// them for the left hand so "towards the centre" stays the same.
fn twist_dir(v: Vec3, yaw: f32, roll: f32, side: f32) -> Vec3 {
    let (ys, yc) = (yaw * side).sin_cos();
    // Yaw about up: + turns the forward axis to the left (towards the view centre for the right hand).
    let v = Vec3::new(v.x * yc - v.z * ys, v.y, v.x * ys + v.z * yc);
    let (rs, rc) = (roll * side).sin_cos();
    // Roll about forward: + leans the top to the left.
    Vec3::new(v.x * rc - v.y * rs, v.x * rs + v.y * rc, v.z)
}

/// Applies an arm's yaw/roll to vertices already in camera-relative world space.
fn twist_verts(verts: &mut [ModelVertex], frame: &Frame, a: &ArmPlace, side: f32) {
    if a.yaw.abs() < 1e-5 && a.roll.abs() < 1e-5 {
        return;
    }
    let (r, u, f) = (frame.right.as_vec3(), frame.up.as_vec3(), frame.fwd.as_vec3());
    let local = |w: Vec3| Vec3::new(w.dot(r), w.dot(u), w.dot(f));
    let world = |l: Vec3| r * l.x + u * l.y + f * l.z;
    for v in verts.iter_mut() {
        let p = local(Vec3::from(v.pos)) - a.pos;
        v.pos = world(a.pos + twist_dir(p, a.yaw, a.roll, side)).to_array();
        v.normal = world(twist_dir(local(Vec3::from(v.normal)), a.yaw, a.roll, side)).to_array();
    }
}

fn camera_frame(pose: &HandsPose) -> Frame {
    let fwd = pose.forward.normalize_or_zero().as_dvec3();
    let up0 = pose.up.normalize_or_zero().as_dvec3();
    let right = fwd.cross(up0).normalize_or_zero();
    let up = right.cross(fwd).normalize_or_zero();
    Frame { origin: DVec3::ZERO, right, up, fwd }
}

/// Textured mini block held in the right fist (camera-relative world-pipeline vertices).
/// `tiles`: atlas sprite per face (+X, -X, +Y, -Y, +Z, -Z), as in `BlockInfo::tiles`.
/// `cross`: a plant or torch: two crossed sprites standing in the fist instead of a cube.
pub fn build_held_block(pose: &HandsPose, tiles: [u16; 6], emissive: bool, cross: bool) -> Vec<crate::mesher::Vertex> {
    let frame = camera_frame(pose);
    let a = arm_place(pose, 1.0);
    // Fist centre after the forearm tilt, then a bit above and in front of it.
    let fist = tilt_point(a.pos + Vec3::new(0.0, 0.0, 0.29), a.pos, a.tilt);
    let centre = fist + Vec3::new(0.05, 0.06, 0.06);
    let h = 0.15f32;
    let (ys, yc) = 0.72f32.sin_cos(); // turned so two sides show, like a held block
    let (ps, pc) = (-0.32f32).sin_cos(); // top tipped towards the camera, as in Minecraft
    let rot = |v: Vec3| {
        let r = Vec3::new(v.x * yc + v.z * ys, v.y, -v.x * ys + v.z * yc);
        Vec3::new(r.x, r.y * pc - r.z * ps, r.y * ps + r.z * pc)
    };
    let twist = |l: Vec3| a.pos + twist_dir(l - a.pos, a.yaw, a.roll, 1.0);
    let mut out = Vec::with_capacity(72);
    if cross {
        // Two crossed sprites standing up from the fist (torch flame / flower on top).
        let foot = fist + Vec3::new(0.02, -0.04, 0.04);
        let (hw, ht) = (0.13f32, 0.30f32);
        for ang in [0.6f32, 0.6 + std::f32::consts::FRAC_PI_2] {
            let d = Vec3::new(ang.cos(), 0.0, ang.sin()) * hw;
            let corner = |u: f32, v: f32| -> crate::mesher::Vertex {
                let local = twist(foot + d * (u * 2.0 - 1.0) + Vec3::Y * (v * ht));
                crate::mesher::Vertex { pos: frame.to_world(local).as_vec3().to_array(), uv: [0.001 + u * 0.998, 0.999 - v * 0.998], flags: tiles[0] as u32 | ((emissive as u32) << 16) | (2 << 17), light: [1.0, 1.0] }
            };
            let q = [corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)];
            for i in [0usize, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] {
                out.push(q[i]);
            }
        }
        return out;
    }
    // (face index in tiles, normal, u axis, v axis)
    let faces: [(usize, Vec3, Vec3, Vec3); 6] = [
        (0, Vec3::X, Vec3::Z, Vec3::Y),
        (1, Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (2, Vec3::Y, Vec3::X, Vec3::Z),
        (3, Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (4, Vec3::Z, Vec3::X, Vec3::Y),
        (5, Vec3::NEG_Z, Vec3::X, Vec3::Y),
    ];
    for (fi, n, ua, va) in faces {
        let corner = |u: f32, v: f32| -> crate::mesher::Vertex {
            let local = twist(centre + rot((n + ua * (u * 2.0 - 1.0) + va * (v * 2.0 - 1.0)) * h));
            let w = frame.to_world(local).as_vec3();
            crate::mesher::Vertex {
                pos: w.to_array(),
                uv: [0.001 + u * 0.998, 0.999 - v * 0.998],
                flags: tiles[fi] as u32 | ((emissive as u32) << 16) | ((fi as u32) << 17),
                light: [1.0, 1.0],
            }
        };
        let q = [corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)];
        // Both windings: the cube is tiny and always seen from outside, culling stays simple.
        for i in [0usize, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] {
            out.push(q[i]);
        }
    }
    out
}

/// Builds both first-person arms relative to the camera (camera at the origin).
/// Vertices: right arm first (72 when empty-handed), then the left arm (72).
pub fn build_first_person_hands(pose: &HandsPose) -> Vec<ModelVertex> {
    let frame = camera_frame(pose);
    let (skin_c, _, _, sleeve_c, _) = skin_colors(pose.skin);
    let sleeve_c = pose.sleeve.unwrap_or(sleeve_c);

    let mut v = Vec::with_capacity(4 * 36);
    for side in [1.0f32, -1.0] {
        let a = arm_place(pose, side);
        let start = v.len();
        let (pivot, angle) = (a.pos, a.tilt);
        let (x, y, z) = (pivot.x, pivot.y, pivot.z);
        // Both arms are always built (the left one rests below the screen unless it carries a
        // shield); the right one grips the held item, like in Minecraft.
        {
            // Square arm like the third-person body: sleeve with a slightly wider hand.
            add_box(&mut v, &frame, Vec3::new(x - 0.07, y - 0.07, z), Vec3::new(x + 0.07, y + 0.07, z + 0.26), pivot, angle, sleeve_c);
            add_box(&mut v, &frame, Vec3::new(x - 0.075, y - 0.075, z + 0.25), Vec3::new(x + 0.075, y + 0.075, z + 0.36), pivot, angle, skin_c);
        }
        let f = Vec3::new(x, y, z + 0.29); // fist centre before the tilt
        if side > 0.0 {
            if let Some(c) = pose.bow {
                add_bow(&mut v, &frame, f, pivot, angle, c.clamp(0.0, 1.0));
            } else if let Some((kind, mat)) = pose.tool {
                add_tool(&mut v, &frame, f, pivot, angle, kind, mat);
            } else if let Some(px) = pose.icon.and_then(|i| textures().and_then(|t| t.item_pixels.get(i as usize).cloned())).filter(|p| p.len() == 256) {
                // Stick, food, ingots…: a small 3D icon in the fist, facing the camera a bit.
                let a = 0.55f32;
                let u = Vec3::new(-a.cos(), 0.0, a.sin());
                let n = u.cross(Vec3::Y).normalize();
                add_voxel_icon(&mut v, &frame, &px, f + Vec3::new(0.08, -0.06, 0.0), u, Vec3::Y, n, 0.24, pivot, angle);
            }
        } else if pose.shield {
            add_shield(&mut v, &frame, f, pivot, angle);
        }
        twist_verts(&mut v[start..], &frame, &a, side);
    }
    v
}

/// Tool held upright in the fist; boxes share the forearm tilt.
fn add_tool(v: &mut Vec<ModelVertex>, frame: &Frame, f: Vec3, pivot: Vec3, angle: f32, kind: u8, mat: u8) {
    let sprite = if kind == 4 { wolfcraft_assets::items::HOE_BASE as usize + mat as usize } else { wolfcraft_assets::items::TOOLS_BASE as usize + mat as usize * 4 + kind as usize };
    if let Some(px) = textures().and_then(|t| t.item_pixels.get(sprite).cloned()).filter(|p| p.len() == 256) {
        // 3D item: every opaque icon pixel is a little box. The icon's handle (bottom-left)
        // sits in the fist; its right edge points left and a bit away, so the tip leans
        // towards the crosshair.
        // Minecraft grip. Our tool icons run from the handle (bottom-right pixel) to the head
        // (top-left). `d` is that diagonal in the view: up, forward and a bit right; the flat
        // side looks back towards the camera at about 45°.
        let d = Vec3::new(-0.22, 0.92, 0.32).normalize();
        let to_cam = Vec3::new(-0.55, 0.0, -1.0).normalize();
        let to_cam = (to_cam - d * d.dot(to_cam)).normalize();
        let e = d.cross(to_cam);
        let u = (e - d).normalize();
        let w = (e + d).normalize();
        let n = u.cross(w).normalize();
        let s = 0.46;
        // The grip pixel (12.5, 12.5 from the top-left) sits in the fist.
        let o = f + Vec3::new(-0.01, 0.03, 0.02) - u * (s * 0.78) - w * (s * 0.22);
        add_voxel_icon(v, frame, &px, o, u, w, n, s, pivot, angle);
        return;
    }
    if let Some(r) = textures().and_then(|t| t.items.get(sprite).copied()) {
        // Icon handle is bottom-left, head top-right: card from the fist up and forward,
        // turned towards the view centre so it is seen at an angle.
        let side = Vec3::new(-0.55, 0.0, 0.83).normalize();
        let s = 0.34;
        let o = f + Vec3::new(0.05, -0.12, -0.05) - side * 0.02;
        let p = |u: f32, w: f32| o + side * (u * s) + Vec3::Y * (w * s);
        add_card(v, frame, [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)], [[r[0], r[3]], [r[2], r[3]], [r[2], r[1]], [r[0], r[1]]], pivot, angle);
        return;
    }
    let m = TOOL_MATERIALS[(mat as usize).min(3)];
    let b = |v: &mut Vec<ModelVertex>, lo: Vec3, hi: Vec3, c: [f32; 3]| add_box(v, frame, f + lo, f + hi, pivot, angle, c);
    if kind == 3 {
        // Sword: grip, cross guard, long blade.
        b(v, Vec3::new(-0.015, -0.03, -0.015), Vec3::new(0.015, 0.08, 0.015), HANDLE);
        b(v, Vec3::new(-0.07, 0.08, -0.02), Vec3::new(0.07, 0.11, 0.02), HANDLE);
        b(v, Vec3::new(-0.022, 0.11, -0.008), Vec3::new(0.022, 0.50, 0.008), m);
        return;
    }
    b(v, Vec3::new(-0.015, -0.05, -0.015), Vec3::new(0.015, 0.33, 0.015), HANDLE);
    match kind {
        0 => b(v, Vec3::new(-0.02, 0.28, -0.13), Vec3::new(0.02, 0.33, 0.13), m),  // pickaxe head
        1 => b(v, Vec3::new(-0.018, 0.22, 0.0), Vec3::new(0.018, 0.34, 0.10), m),  // axe blade
        _ => b(v, Vec3::new(-0.05, 0.31, -0.008), Vec3::new(0.05, 0.42, 0.008), m), // shovel plate
    }
}

/// Bow held upright in the fist, seen from the side: the wooden arc bulges away from the
/// camera, the string runs back towards it. `c` (0..1) is the draw: the string is pulled back,
/// the limbs bend more and an arrow lies on the string.
fn add_bow(v: &mut Vec<ModelVertex>, frame: &Frame, f: Vec3, pivot: Vec3, angle: f32, c: f32) {
    const WOOD: [f32; 3] = [0.55, 0.36, 0.18];
    const WOOD_DARK: [f32; 3] = [0.40, 0.25, 0.12];
    const STRING: [f32; 3] = [0.92, 0.92, 0.88];
    let mut cube = |p: Vec3, s: f32, col: [f32; 3]| add_box(v, frame, p - Vec3::splat(s * 0.5), p + Vec3::splat(s * 0.5), pivot, angle, col);
    let half = 0.30; // half the bow's height
    let bend = 0.10 + 0.05 * c;
    let grip = f + Vec3::new(0.0, 0.10, 0.05);
    // Limb point at t (-1 bottom .. 1 top): the tips curve back towards the string.
    let limb = |t: f32| grip + Vec3::new(0.0, t * half, -t * t * bend);
    let steps = 24;
    for i in 0..=steps {
        let t = i as f32 / steps as f32 * 2.0 - 1.0;
        let col = if t.abs() < 0.15 { WOOD_DARK } else { WOOD };
        cube(limb(t), if t.abs() < 0.15 { 0.045 } else { 0.032 }, col);
    }
    // String: tips to the nock, which the draw pulls back.
    let (top, bottom) = (limb(1.0), limb(-1.0));
    let nock = Vec3::new(grip.x, grip.y, top.z - 0.20 * c);
    for (a, b) in [(top, nock), (nock, bottom)] {
        let n = ((a - b).length() / 0.008).ceil() as usize;
        for i in 0..=n {
            cube(a.lerp(b, i as f32 / n as f32), 0.013, STRING);
        }
    }
    if c > 0.05 {
        // Arrow on the string, pointing away from the camera past the grip.
        let len = 0.62;
        let tail = nock;
        let head = tail + Vec3::new(0.0, 0.0, len);
        let n = 40;
        for i in 0..=n {
            cube(tail.lerp(head, i as f32 / n as f32), 0.018, [0.62, 0.45, 0.25]);
        }
        cube(head + Vec3::new(0.0, 0.0, 0.02), 0.034, [0.72, 0.74, 0.78]);
        for dy in [-0.02f32, 0.02] {
            cube(tail + Vec3::new(0.0, dy, 0.04), 0.022, [0.95, 0.95, 0.93]);
        }
    }
}

/// Wooden shield with an iron rim and boss, facing away from the camera.
fn add_shield(v: &mut Vec<ModelVertex>, frame: &Frame, f: Vec3, pivot: Vec3, angle: f32) {
    let b = |v: &mut Vec<ModelVertex>, lo: Vec3, hi: Vec3, c: [f32; 3]| add_box(v, frame, f + lo, f + hi, pivot, angle, c);
    b(v, Vec3::new(-0.11, -0.10, 0.045), Vec3::new(0.11, 0.19, 0.06), IRON);
    b(v, Vec3::new(-0.10, -0.09, 0.04), Vec3::new(0.10, 0.18, 0.065), PLANK);
    b(v, Vec3::new(-0.022, 0.02, 0.035), Vec3::new(0.022, 0.065, 0.04), IRON);
    if let Some(r) = textures().and_then(|t| t.items.get(wolfcraft_assets::items::SHIELD as usize).copied()) {
        // Painted front (towards the camera) from the shield icon.
        let z = 0.034;
        let p = |x: f32, y: f32| f + Vec3::new(x, y, z);
        add_card(v, frame, [p(-0.11, -0.10), p(0.11, -0.10), p(0.11, 0.19), p(-0.11, 0.19)], [[r[0], r[3]], [r[2], r[3]], [r[2], r[1]], [r[0], r[1]]], pivot, angle);
    }
}

/// Extrudes a 16x16 icon into pixel boxes in the plane (`u` right, `w` up), one pixel deep
/// along `n`; `o` is the icon's bottom-left corner. Inner side faces are skipped.
#[allow(clippy::too_many_arguments)]
fn add_voxel_icon(out: &mut Vec<ModelVertex>, frame: &Frame, px: &[[u8; 4]], o: Vec3, u: Vec3, w: Vec3, n: Vec3, s: f32, pivot: Vec3, angle: f32) {
    let (sn, cs) = angle.sin_cos();
    let rot = |p: Vec3| {
        let d = p - pivot;
        pivot + Vec3::new(d.x, d.y * cs - d.z * sn, d.y * sn + d.z * cs)
    };
    let rot_n = |v: Vec3| Vec3::new(v.x, v.y * cs - v.z * sn, v.y * sn + v.z * cs);
    let solid = |x: i32, y: i32| (0..16).contains(&x) && (0..16).contains(&y) && px[(y * 16 + x) as usize][3] > 127;
    let d = s / 16.0;
    for y in 0..16i32 {
        for x in 0..16i32 {
            if !solid(x, y) {
                continue;
            }
            let c = px[(y * 16 + x) as usize];
            let color = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0];
            // Pixel cell: image row 0 is the top.
            let base = o + u * (x as f32 * d) + w * ((15 - y) as f32 * d) - n * (d * 0.5);
            let corner = |a: f32, b: f32, k: f32| base + u * (a * d) + w * (b * d) + n * (k * d);
            // (normal, 4 corners in (u, w, n) unit coords, neighbour that hides the face)
            let faces: [(Vec3, [(f32, f32, f32); 4], Option<(i32, i32)>); 6] = [
                (n, [(0.0, 0.0, 1.0), (1.0, 0.0, 1.0), (1.0, 1.0, 1.0), (0.0, 1.0, 1.0)], None),
                (-n, [(0.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0), (1.0, 0.0, 0.0)], None),
                (u, [(1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (1.0, 1.0, 1.0), (1.0, 0.0, 1.0)], Some((x + 1, y))),
                (-u, [(0.0, 0.0, 0.0), (0.0, 0.0, 1.0), (0.0, 1.0, 1.0), (0.0, 1.0, 0.0)], Some((x - 1, y))),
                (w, [(0.0, 1.0, 0.0), (0.0, 1.0, 1.0), (1.0, 1.0, 1.0), (1.0, 1.0, 0.0)], Some((x, y - 1))),
                (-w, [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 0.0, 1.0), (0.0, 0.0, 1.0)], Some((x, y + 1))),
            ];
            for (nl, q, nb) in faces {
                if nb.is_some_and(|(a, b)| solid(a, b)) {
                    continue;
                }
                let pts: Vec<Vec3> = q.iter().map(|(a, b, k)| frame.to_world(rot(corner(*a, *b, *k))).as_vec3()).collect();
                let nw = frame.dir(rot_n(nl)).normalize_or_zero();
                // Both windings: orientation of (u, w, n) differs per item, culling stays simple.
                for i in [0usize, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] {
                    out.push(ModelVertex { pos: pts[i].to_array(), normal: nw.to_array(), color, uv: [0.0; 2], tex: 0.0 });
                }
            }
        }
    }
}

/// Two-sided textured quad (items atlas, transparent pixels cut out), tilted with the forearm.
fn add_card(out: &mut Vec<ModelVertex>, frame: &Frame, pts: [Vec3; 4], uv: [[f32; 2]; 4], pivot: Vec3, angle: f32) {
    let (s, c) = angle.sin_cos();
    let rot = |p: Vec3| {
        let d = p - pivot;
        pivot + Vec3::new(d.x, d.y * c - d.z * s, d.y * s + d.z * c)
    };
    let w: Vec<Vec3> = pts.iter().map(|p| frame.to_world(rot(*p)).as_vec3()).collect();
    let n = (w[1] - w[0]).cross(w[3] - w[0]).normalize_or_zero();
    for (order, nn) in [([0usize, 1, 2, 0, 2, 3], n), ([0usize, 2, 1, 0, 3, 2], -n)] {
        for i in order {
            out.push(ModelVertex { pos: w[i].to_array(), normal: nn.to_array(), color: [1.0; 3], uv: uv[i], tex: 2.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hands(cycle: f32, guard: f32) -> HandsPose {
        HandsPose { punch_cycle: cycle, guard, ..HandsPose::rest(Vec3::new(0.3, -0.2, -1.0).normalize(), Vec3::Y) }
    }

    /// Furthest reach of an arm along the view direction.
    fn reach(verts: &[ModelVertex], fwd: Vec3) -> f32 {
        verts.iter().map(|v| Vec3::from(v.pos).dot(fwd)).fold(f32::MIN, f32::max)
    }

    fn centre(verts: &[ModelVertex]) -> Vec3 {
        verts.iter().fold(Vec3::ZERO, |a, v| a + Vec3::from(v.pos)) / verts.len().max(1) as f32
    }

    fn assert_outward(verts: &[ModelVertex]) {
        for t in verts.chunks(3) {
            let (a, b, cc) = (Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos));
            let n = (b - a).cross(cc - a).normalize_or_zero();
            if n == Vec3::ZERO {
                continue;
            }
            assert!(n.dot(Vec3::from(t[0].normal)) > 0.99, "inside-out triangle");
        }
    }

    #[test]
    fn rounded_boxes_are_closed_and_face_outwards() {
        let frame = Frame { origin: DVec3::new(1.0, 2.0, 3.0), right: DVec3::X, up: DVec3::Y, fwd: DVec3::Z };
        let (min, max) = (Vec3::new(-0.3, 0.0, -0.2), Vec3::new(0.3, 0.8, 0.25));
        let mut v = Vec::new();
        add_box_r(&mut v, &frame, min, max, Vec3::ZERO, 0.3, [0.5; 3], true);
        assert_eq!(v.len(), (6 * 2 + 12 * 2 + 8) * 3, "faces, edge strips and corners");
        let c = v.iter().fold(Vec3::ZERO, |a, x| a + Vec3::from(x.pos)) / v.len() as f32;
        for t in v.chunks(3) {
            let (a, b, cc) = (Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos));
            // Left-handed frame: the mirrored winding gives the outward normal as (c-a)x(b-a).
            let n = (cc - a).cross(b - a).normalize_or_zero();
            let mid = (a + b + cc) / 3.0;
            assert!(n.dot(mid - c) > 0.0, "every triangle faces away from the centre");
            for x in t {
                assert!(Vec3::from(x.normal).dot(n) > 0.3, "smooth normals lean the same way");
            }
        }
        // The silhouette keeps the box's size.
        let ext = v.iter().fold(f32::MIN, |m, x| m.max(Vec3::from(x.pos).x - 1.0));
        assert!((ext - 0.3).abs() < 1e-4);
    }

    #[test]
    fn hands_face_outwards_and_stay_in_front_of_the_camera() {
        for (c, g) in [(0.0, 0.0), (0.12, 1.0), (0.35, 1.0), (0.7, 1.0), (0.4, 0.5)] {
            let mut p = hands(c, g);
            p.sway = [0.8, -0.6];
            p.sprint = 1.0;
            p.bob_amount = 1.0;
            p.bob_phase = 1.0;
            let verts = build_first_person_hands(&p);
            assert_eq!(verts.len(), 4 * 36);
            assert_outward(&verts);
            let fwd = p.forward.normalize();
            assert!(verts.iter().all(|v| Vec3::from(v.pos).dot(fwd) > 0.08), "arm behind the near plane");
        }
    }

    #[test]
    fn right_arm_is_in_view_at_rest_and_the_left_one_is_not() {
        let p = hands(0.0, 1.0);
        let v = build_first_person_hands(&p);
        let (r, l) = (centre(&v[..72]), centre(&v[72..]));
        let fwd = p.forward.normalize();
        // Angle below the view axis: the right hand is on screen, the left one far below it.
        let down = |c: Vec3| (c - fwd * c.dot(fwd)).length() / c.dot(fwd);
        assert!(down(r) < 0.9, "right arm visible ({})", down(r));
        assert!(down(l) > 1.2, "left arm hidden ({})", down(l));
    }

    #[test]
    fn a_punch_swings_the_right_arm_in_towards_the_crosshair_and_back() {
        let p0 = hands(0.0, 1.0);
        let fwd = p0.forward.normalize();
        let off_axis = |v: &[ModelVertex]| {
            let c = centre(v);
            (c - fwd * c.dot(fwd)).length()
        };
        let rest = build_first_person_hands(&p0);
        let hit = build_first_person_hands(&hands(strike_moment(false), 1.0));
        let done = build_first_person_hands(&hands(0.999, 1.0));
        assert!(reach(&hit[..72], fwd) > reach(&rest[..72], fwd) + 0.08, "the fist goes forward");
        assert!(off_axis(&hit[..72]) < off_axis(&rest[..72]) - 0.05, "and in towards the crosshair");
        assert!(rest.iter().zip(&done).all(|(a, b)| (Vec3::from(a.pos) - Vec3::from(b.pos)).length() < 0.01), "ends where it started");
        assert!(rest[72..].iter().zip(&hit[72..]).all(|(a, b)| a.pos == b.pos), "the left arm stays down");
    }

    #[test]
    fn every_swing_starts_and_ends_at_rest_without_jumps() {
        for tool in [None, Some((0u8, 1u8)), Some((1, 1)), Some((2, 1)), Some((3, 1))] {
            let mk = |c: f32| {
                let mut p = hands(c, 1.0);
                p.holding = tool.is_some();
                p.tool = tool;
                build_first_person_hands(&p)
            };
            let rest = mk(0.0);
            let mut prev = rest.clone();
            for i in 1..=200 {
                let cur = mk(i as f32 / 200.0);
                let jump = prev.iter().zip(&cur).map(|(a, b)| (Vec3::from(a.pos) - Vec3::from(b.pos)).length()).fold(0.0, f32::max);
                assert!(jump < 0.06, "{tool:?}: jump {jump} at {i}");
                prev = cur;
            }
            let end = prev.iter().zip(&rest).map(|(a, b)| (Vec3::from(a.pos) - Vec3::from(b.pos)).length()).fold(0.0, f32::max);
            assert!(end < 1e-4, "{tool:?}: swing must end at rest ({end})");
        }
    }

    #[test]
    fn sword_swings_and_shield_comes_to_the_middle_when_blocking() {
        let mut p = hands(0.0, 1.0);
        p.holding = true;
        p.tool = Some((3, 2));
        p.shield = true;
        let fwd = p.forward.normalize();
        let rest = build_first_person_hands(&p);
        p.punch_cycle = strike_moment(true);
        let struck = build_first_person_hands(&p);
        assert!(rest.len() > 4 * 36, "sword and shield add boxes");
        assert_eq!(rest.len(), struck.len());
        assert!(rest.iter().zip(&struck).any(|(a, b)| a.pos != b.pos), "the sword arm swings");
        // Shield boxes are the last 3 boxes: their centre moves towards the view axis when blocking.
        let centre = |v: &[ModelVertex]| {
            let c = centre(&v[v.len() - 3 * 36..]);
            (c - fwd * c.dot(fwd)).length()
        };
        let down = centre(&build_first_person_hands(&p));
        p.blocking = 1.0;
        let up = centre(&build_first_person_hands(&p));
        assert!(up < down - 0.1, "shield must move to the middle ({down} -> {up})");
    }

    #[test]
    fn hand_trails_camera_turns_and_drops_when_sprinting() {
        let base = hands(0.0, 1.0);
        let (fwd, up) = (base.forward.normalize(), base.up);
        let right = fwd.cross(up).normalize();
        let c0 = centre(&build_first_person_hands(&base)[..72]);
        // Turning right: the hand is left behind, to the left.
        let mut turn = base;
        turn.sway = [-1.0, 0.0];
        let c1 = centre(&build_first_person_hands(&turn)[..72]);
        assert!((c1 - c0).dot(right) < -0.03, "sway to the left ({})", (c1 - c0).dot(right));
        let mut run = base;
        run.sprint = 1.0;
        let c2 = centre(&build_first_person_hands(&run)[..72]);
        let upv = right.cross(fwd);
        assert!((c2 - c0).dot(upv) < -0.03, "sprinting lowers the hand");
    }

    #[test]
    fn held_block_follows_the_swing() {
        let mut p = hands(0.0, 1.0);
        p.holding = true;
        let tiles = [1u16; 6];
        let rest = build_held_block(&p, tiles, false, false);
        p.punch_cycle = strike_moment(false);
        let hit = build_held_block(&p, tiles, false, false);
        assert_eq!(build_held_block(&p, tiles, false, true).len(), 24, "a torch is two crossed sprites");
        assert_eq!(rest.len(), 72);
        let c = |v: &[crate::mesher::Vertex]| v.iter().fold(Vec3::ZERO, |a, v| a + Vec3::from(v.pos)) / v.len() as f32;
        assert!((c(&hit) - c(&rest)).length() > 0.08, "the block moves with the punch");
    }

    fn pose(up: DVec3) -> PlayerPose {
        let heading = if up.y.abs() > 0.9 { DVec3::X } else { DVec3::Y.cross(up).normalize() };
        PlayerPose { feet: up * 1000.0, heading, up, pitch: 0.0, phase: 0.0, amount: 0.0, sneaking: false, swimming: false, skin: 0, attack: 0.0, held: Held::Nothing, armor: [None; 4] }
    }

    #[test]
    fn model_has_expected_size_on_every_side_of_the_planet() {
        for up in [DVec3::Y, DVec3::NEG_Y, DVec3::X, DVec3::new(1.0, 1.0, 1.0).normalize(), DVec3::new(-0.3, 0.2, -1.0).normalize()] {
            let p = pose(up);
            let verts = build_player_model(&p, p.feet - up * 5.0);
            assert_eq!(verts.len() % 36, 0);
            assert!(verts.len() >= 10 * 36);
            let heights: Vec<f64> = verts.iter().map(|v| (DVec3::new(v.pos[0] as f64, v.pos[1] as f64, v.pos[2] as f64) - (p.feet - (p.feet - up * 5.0))).dot(up)).collect();
            let (lo, hi) = heights.iter().fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
            assert!((hi - lo - 1.8).abs() < 0.15, "model height {} for up {up}", hi - lo);
            assert!(verts.iter().all(|v| v.pos.iter().all(|c| c.is_finite()) && (Vec3::from(v.normal).length() - 1.0).abs() < 1e-3));
        }
    }

    #[test]
    fn walking_swings_limbs_and_standing_is_symmetric() {
        let mut p = pose(DVec3::Y);
        let cam = p.feet - DVec3::Y * 3.0;
        let still = build_player_model(&p, cam);
        p.phase = std::f32::consts::FRAC_PI_2;
        p.amount = 1.0;
        let walking = build_player_model(&p, cam);
        assert_eq!(still.len(), walking.len());
        let moved = still.iter().zip(&walking).filter(|(a, b)| a.pos != b.pos).count();
        assert!(moved > 100, "limbs must move while walking ({moved})");
        // Mirror symmetry when standing: bounding box centred on the body axis.
        let xs: Vec<f32> = still.iter().map(|v| v.pos[0]).collect();
        let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
        assert!(((lo + hi) / 2.0 - (p.feet.x - cam.x) as f32).abs() < 0.05);
    }

    #[test]
    fn armour_and_tools_show_on_the_character() {
        let mut p = pose(DVec3::Y);
        let cam = p.feet - DVec3::Y * 3.0;
        let bare = build_player_model(&p, cam).len();
        p.armor = [Some(2), Some(2), Some(2), Some(2)];
        let dressed = build_player_model(&p, cam);
        assert!(dressed.len() > bare + 8 * 36, "armour adds boxes");
        let iron = armor_color(2);
        assert!(dressed.iter().any(|v| v.color == iron));
        p.held = Held::Tool(0, 2);
        let rest = build_player_model(&p, cam);
        assert!(rest.len() > dressed.len(), "the pickaxe is drawn");
        p.armor = [None; 4];
        let rest = build_player_model(&p, cam);
        p.attack = 0.4;
        let raised = build_player_model(&p, cam);
        // The arm (sleeve) goes up over the head for the wind-up.
        let top = |v: &[ModelVertex]| v.iter().filter(|x| x.color == SLEEVE).map(|x| x.pos[1]).fold(f32::MIN, f32::max);
        assert!(top(&raised) > top(&rest) + 0.3, "the arm is lifted over the head ({} vs {})", top(&raised), top(&rest));
        assert!(swing_angle(0.5, Held::Tool(3, 0)) < -1.5 && swing_angle(0.5, Held::Nothing) < -1.0);
    }

    #[test]
    fn faces_point_outwards() {
        let p = pose(DVec3::Y);
        let cam = p.feet - DVec3::Y * 3.0;
        let verts = build_player_model(&p, cam);
        // Every triangle's geometric normal must agree with its stored normal.
        for t in verts.chunks(3) {
            let (a, b, c) = (Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos));
            let n = (b - a).cross(c - a).normalize_or_zero();
            assert!(n.dot(Vec3::from(t[0].normal)) > 0.99, "inside-out triangle");
        }
    }
}


/// Colours of the player's body parts, for the cubes of a death.
pub fn player_palette() -> [[f32; 3]; 5] {
    [SKIN, JACKET, TROUSERS, BOOTS, HAIR]
}

/// Cubes of dead creatures, relative to camera.
pub fn build_debris(debris: &[crate::mobs::Debris], camera: DVec3) -> Vec<ModelVertex> {
    let mut v = Vec::with_capacity(debris.len() * 36);
    for d in debris {
        let up = crate::physics::up_at(d.pos);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let fwd = (helper - up * helper.dot(up)).normalize_or_zero();
        let right = fwd.cross(up).normalize_or_zero();
        let f = Frame { origin: d.pos - camera, right, up, fwd };
        let h = d.size * 0.5;
        // Shrinks away in the last half second.
        let k = ((3.0 - d.age) / 0.5).clamp(0.0, 1.0);
        let h = h * k.max(0.05);
        let start = v.len();
        add_box(&mut v, &f, Vec3::new(-h, -h, -h), Vec3::new(h, h, h), Vec3::ZERO, d.spin + d.age * 6.0, d.color);
        if d.glow {
            for vx in &mut v[start..] {
                vx.tex = 3.0; // unlit
            }
        }
    }
    v
}

#[cfg(test)]
mod strike_tests {
    use super::*;

    #[test]
    fn strike_curve_is_continuous_and_peaks_once() {
        assert!(strike_curve(0.0).abs() < 1e-6 && strike_curve(1.0).abs() < 1e-6, "starts and ends at rest");
        assert!((strike_curve(0.5) - 1.0).abs() < 1e-6, "full hit in the middle");
        assert!(strike_curve(0.2) < 0.0, "wind-up goes backwards first");
        // No jumps anywhere: neighbouring samples stay close.
        let mut prev = strike_curve(0.0);
        for i in 1..=1000 {
            let v = strike_curve(i as f32 / 1000.0);
            assert!((v - prev).abs() < 0.02, "jump at {i}: {prev} -> {v}");
            prev = v;
        }
    }
}

