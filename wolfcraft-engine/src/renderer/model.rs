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
    /// 0 = flat `color`, 1 = entities atlas (skins), 2 = items atlas (cut-out icons).
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

/// Appends an axis-aligned (in the character's local frame) box, optionally rotated about the
/// local X axis (the "right" axis) around `pivot` by `angle` radians.
fn add_box(out: &mut Vec<ModelVertex>, f: &Frame, min: Vec3, max: Vec3, pivot: Vec3, angle: f32, color: [f32; 3]) {
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
                // Flat colours get box-local pixel coordinates: the shader paints a 1/16-block pattern.
                let seed = fi as f32 * 3.7 + min.x * 5.1 + min.y * 7.3 + min.z * 2.9;
                let uv_of = |(x, y, z): (bool, bool, bool)| -> [f32; 2] {
                    let p = corners(x, y, z);
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
        // (right, up, forward) is a left-handed frame, so the winding is mirrored.
        for idx in [0usize, 2, 1, 0, 3, 2] {
            out.push(ModelVertex { pos: v[idx], normal: nw.to_array(), color, uv: uvs[idx], tex });
        }
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
        let x0 = if side < 0.0 { -0.5 } else { 0.25 };
        add_box(&mut v, &frame, Vec3::new(x0, ty0, -0.125), Vec3::new(x0 + 0.25, shoulder, 0.125), Vec3::new(x0 + 0.125, shoulder, 0.0), ang, sleeve_c);
        add_box(&mut v, &frame, Vec3::new(x0, ty0 - 0.02, -0.12), Vec3::new(x0 + 0.25, ty0 + 0.16, 0.12), Vec3::new(x0 + 0.125, shoulder, 0.0), ang, skin_c);
    }
    // Head follows the look pitch, pivoting at the neck.
    let neck = ty0 + 0.65;
    let head_ang = if swim { -1.3 - pose.pitch * 0.8 } else { -pose.pitch * 0.9 };
    add_box(&mut v, &frame, Vec3::new(-0.25, neck, -0.25), Vec3::new(0.25, neck + 0.5, 0.25), Vec3::new(0.0, neck, 0.0), head_ang, skin_c);
    add_box(&mut v, &frame, Vec3::new(-0.26, neck + 0.38, -0.26), Vec3::new(0.26, neck + 0.52, 0.26), Vec3::new(0.0, neck, 0.0), head_ang, hair_c);
    for ex in [-0.14f32, 0.06] {
        add_box(&mut v, &frame, Vec3::new(ex, neck + 0.22, 0.25), Vec3::new(ex + 0.08, neck + 0.30, 0.262), Vec3::new(0.0, neck, 0.0), head_ang, EYE);
    }
    v
}

/// Builds all creatures relative to `camera` (original blocky designs, flat colours).
pub fn build_mob_models(mobs: &[crate::mobs::Mob], camera: DVec3) -> Vec<ModelVertex> {
    use crate::mobs::MobKind;
    let mut v = Vec::with_capacity(mobs.len() * 10 * 36);
    for m in mobs {
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
                let skin = tint([0.42, 0.52, 0.38]);
                let rag = tint([0.30, 0.27, 0.34]);
                let eye = [0.95, 0.15, 0.10];
                for (side, a) in [(-1.0f32, swing), (1.0, -swing)] {
                    let x0 = if side < 0.0 { -0.25 } else { 0.0 };
                    add_box(&mut v, &f, Vec3::new(x0, 0.0, -0.125), Vec3::new(x0 + 0.25, 0.7, 0.125), Vec3::new(x0 + 0.125, 0.7, 0.0), a, rag);
                }
                add_box(&mut v, &f, Vec3::new(-0.25, 0.7, -0.15), Vec3::new(0.25, 1.35, 0.15), z, 0.0, rag);
                // Arms reach forward.
                for x0 in [-0.5f32, 0.25] {
                    add_box(&mut v, &f, Vec3::new(x0, 0.7, -0.125), Vec3::new(x0 + 0.25, 1.3, 0.125), Vec3::new(x0 + 0.125, 1.3, 0.0), 1.35 + swing * 0.2, skin);
                }
                add_box(&mut v, &f, Vec3::new(-0.25, 1.35, -0.25), Vec3::new(0.25, 1.85, 0.25), z, 0.0, skin);
                for ex in [-0.15f32, 0.07] {
                    add_box(&mut v, &f, Vec3::new(ex, 1.55, 0.25), Vec3::new(ex + 0.08, 1.63, 0.262), z, 0.0, eye);
                }
            }
            MobKind::Crawler => {
                let body = tint([0.22, 0.14, 0.30]);
                let leg = tint([0.15, 0.10, 0.20]);
                let eye = [0.95, 0.20, 0.15];
                add_box(&mut v, &f, Vec3::new(-0.3, 0.3, -0.5), Vec3::new(0.3, 0.7, 0.4), z, 0.0, body);
                add_box(&mut v, &f, Vec3::new(-0.22, 0.32, 0.4), Vec3::new(0.22, 0.68, 0.75), z, 0.0, body);
                for (i, zz) in [-0.35f32, -0.05, 0.25].iter().enumerate() {
                    let a = if i % 2 == 0 { swing } else { -swing };
                    add_box(&mut v, &f, Vec3::new(-0.5, 0.0, *zz), Vec3::new(-0.3, 0.5, *zz + 0.1), Vec3::new(-0.4, 0.5, *zz), a, leg);
                    add_box(&mut v, &f, Vec3::new(0.3, 0.0, *zz), Vec3::new(0.5, 0.5, *zz + 0.1), Vec3::new(0.4, 0.5, *zz), -a, leg);
                }
                for ex in [-0.14f32, 0.06] {
                    add_box(&mut v, &f, Vec3::new(ex, 0.5, 0.75), Vec3::new(ex + 0.08, 0.58, 0.762), z, 0.0, eye);
                }
            }
            MobKind::Boar | MobKind::Sheep => {
                let sheep = m.kind == MobKind::Sheep;
                let body = tint(if sheep { [0.92, 0.91, 0.86] } else { [0.62, 0.42, 0.36] });
                let head = tint(if sheep { [0.25, 0.22, 0.22] } else { [0.70, 0.50, 0.44] });
                let leg = tint(if sheep { [0.30, 0.27, 0.26] } else { [0.45, 0.30, 0.26] });
                let h = if sheep { 0.35 } else { 0.30 };
                add_box(&mut v, &f, Vec3::new(-0.3, h, -0.5), Vec3::new(0.3, h + 0.6, 0.45), z, 0.0, body);
                for (x0, z0, a) in [(-0.28f32, -0.45f32, swing), (0.08, -0.45, -swing), (-0.28, 0.25, -swing), (0.08, 0.25, swing)] {
                    add_box(&mut v, &f, Vec3::new(x0, 0.0, z0), Vec3::new(x0 + 0.2, h + 0.05, z0 + 0.2), Vec3::new(x0 + 0.1, h, z0 + 0.1), a, leg);
                }
                add_box(&mut v, &f, Vec3::new(-0.2, h + 0.3, 0.45), Vec3::new(0.2, h + 0.7, 0.8), z, 0.0, head);
                if !sheep {
                    add_box(&mut v, &f, Vec3::new(-0.1, h + 0.32, 0.8), Vec3::new(0.1, h + 0.48, 0.9), z, 0.0, tint([0.85, 0.62, 0.56]));
                }
                for ex in [-0.14f32, 0.06] {
                    add_box(&mut v, &f, Vec3::new(ex, h + 0.5, 0.8), Vec3::new(ex + 0.08, h + 0.58, 0.812), z, 0.0, [0.05, 0.05, 0.08]);
                }
            }
        }
    }
    v
}

/// First-person arms: camera basis and animation state.
#[derive(Clone, Copy, Debug)]
pub struct HandsPose {
    pub forward: Vec3,
    pub up: Vec3,
    /// Punch cycle; one full cycle = one right jab + one left jab.
    pub punch_cycle: f32,
    /// 0 = arms lowered at rest, 1 = boxing guard (punching while LMB is held).
    pub guard: f32,
    /// Walk bob phase (radians) and amount (0..1).
    pub bob_phase: f32,
    pub bob_amount: f32,
    /// Something is selected in the hotbar: the right hand stays in view holding it.
    pub holding: bool,
    /// Tool in the main hand: (kind 0 pickaxe, 1 axe, 2 shovel, 3 sword; material 0..3).
    /// LMB then swings it instead of boxing.
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
}

/// Swing of a held tool/weapon, 0 (raised) → 1 (struck) → 0, while LMB is held.
/// One strike over `t` 0..1: a short wind-up backwards (to about -0.18), a fast eased hit that
/// reaches 1 at `t = 0.5`, then a soft recovery back to 0. Continuous at both ends, so strikes
/// repeat without a jump while the button is held.
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

fn swing_amount(pose: &HandsPose) -> f32 {
    pose.guard.clamp(0.0, 1.0) * strike_curve(pose.punch_cycle.rem_euclid(1.0))
}

const TOOL_MATERIALS: [[f32; 3]; 4] = [[0.62, 0.45, 0.25], [0.55, 0.55, 0.55], [0.85, 0.85, 0.88], [0.35, 0.90, 0.85]];
const HANDLE: [f32; 3] = [0.45, 0.32, 0.18];
const PLANK: [f32; 3] = [0.62, 0.45, 0.25];
const IRON: [f32; 3] = [0.62, 0.64, 0.68];

/// Where one arm is this frame: shoulder-side start (camera-local x right, y up, z forward)
/// and forearm tilt about the local X axis.
fn arm_place(pose: &HandsPose, side: f32, jab: f32) -> (Vec3, f32) {
    let (mut p, mut a) = arm_place_base(pose, side, jab);
    if side > 0.0 && pose.place > 0.0 {
        // The right hand pushes forward and down quickly, then eases back with a little recoil.
        let t = 1.0 - pose.place.clamp(0.0, 1.0);
        let ss = |x: f32| x * x * (3.0 - 2.0 * x);
        let k = if t < 0.3 { 1.0 - (1.0 - t / 0.3).powi(3) } else { 1.0 - ss((t - 0.3) / 0.7) };
        let recoil = (t * std::f32::consts::PI).sin() * (1.0 - t) * 0.03;
        p += Vec3::new(-0.03 * k, -0.08 * k + recoil, 0.17 * k - recoil);
        a -= 0.5 * k;
    }
    (p, a)
}

fn arm_place_base(pose: &HandsPose, side: f32, jab: f32) -> (Vec3, f32) {
    let g = pose.guard.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let bob_y = pose.bob_phase.sin().abs() * 0.025 * pose.bob_amount;
    let bob_x = pose.bob_phase.cos() * 0.012 * pose.bob_amount;
    // At rest the arms are hidden below the screen; only the right one stays up while holding
    // an item. Holding LMB brings both up into the boxing guard.
    if side > 0.0 && pose.tool.is_some() {
        // Every tool swings its own way; `sw` runs wind-up (<0) -> hit (1) -> back (0).
        let sw = swing_amount(pose);
        let kind = pose.tool.map(|t| t.0).unwrap_or(0);
        let (dx, dy, dz, tilt) = match kind {
            // Pickaxe: raised high, then straight down.
            0 => (-0.05 * sw, 0.10 * g - 0.15 * sw, 0.08 * sw, g * (-0.85 + 1.65 * sw)),
            // Axe: diagonal chop from the upper right to the lower left.
            1 => (0.10 * g - 0.24 * sw, 0.08 * g - 0.12 * sw, 0.06 * sw, g * (-0.60 + 1.30 * sw)),
            // Shovel: a digging thrust forward and down.
            2 => (-0.03 * sw, -0.02 * g - 0.10 * sw, -0.05 * g + 0.22 * sw, g * (0.25 + 0.45 * sw)),
            // Sword: a quick sideways sweep.
            _ => (0.15 * g - 0.36 * sw, 0.02 * g - 0.03 * sw, 0.05 * sw, g * (-0.25 + 0.55 * sw)),
        };
        return (Vec3::new(0.30 + dx + bob_x, -0.30 + dy - bob_y, 0.42 + dz), 0.2 + tilt);
    }
    if side < 0.0 && (pose.shield || pose.tool.is_some()) {
        if pose.shield {
            // Shield at the side; RMB brings it to the middle of the view.
            let b = pose.blocking.clamp(0.0, 1.0);
            // Raised shield stays left of the crosshair so the target remains visible.
            let x = lerp(-0.34, -0.19, b) + bob_x;
            let y = lerp(-0.36, -0.25, b) - bob_y;
            return (Vec3::new(x, y, 0.42), lerp(0.25, -0.05, b));
        }
        return (Vec3::new(-0.34, -0.80, 0.40), 0.25);
    }
    let shown = side > 0.0 && pose.holding;
    let rest_y = if shown { -0.30 } else { -0.80 };
    let x = lerp(side * lerp(0.34, 0.17, g), side * 0.07, jab) + bob_x;
    let y = lerp(lerp(rest_y, -0.20, g), -0.11, jab) - bob_y;
    let z = 0.40 + 0.24 * jab;
    // Tilt: forearm points down at rest, slightly up in guard, straight on impact.
    let angle = lerp(0.25, -0.10, g) * (1.0 - jab);
    (Vec3::new(x, y, z), angle)
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
pub fn build_held_block(pose: &HandsPose, tiles: [u16; 6], emissive: bool) -> Vec<crate::mesher::Vertex> {
    let frame = camera_frame(pose);
    let (p, angle) = arm_place(pose, 1.0, punch_extension(pose)[0]);
    // Fist centre after the forearm tilt, then a bit above and in front of it.
    let (s, c) = angle.sin_cos();
    let fist = p + Vec3::new(0.0, -0.29 * s, 0.29 * c);
    let centre = fist + Vec3::new(0.05, 0.06, 0.06);
    let h = 0.15f32;
    let (ys, yc) = 0.72f32.sin_cos(); // turned so two sides show, like a held block
    let (ps, pc) = (-0.32f32).sin_cos(); // top tipped towards the camera, as in Minecraft
    let rot = |v: Vec3| {
        let r = Vec3::new(v.x * yc + v.z * ys, v.y, -v.x * ys + v.z * yc);
        Vec3::new(r.x, r.y * pc - r.z * ps, r.y * ps + r.z * pc)
    };
    let mut out = Vec::with_capacity(72);
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
            let local = centre + rot((n + ua * (u * 2.0 - 1.0) + va * (v * 2.0 - 1.0)) * h);
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

/// How far each arm is extended (0..1): `[right, left]`. The arms take turns.
pub fn punch_extension(pose: &HandsPose) -> [f32; 2] {
    let g = pose.guard.clamp(0.0, 1.0);
    // The right fist strikes in the first half of the cycle, the left one in the second.
    let c = pose.punch_cycle.rem_euclid(1.0);
    let right = if c < 0.5 { strike_curve(c * 2.0) } else { 0.0 };
    let left = if c >= 0.5 { strike_curve((c - 0.5) * 2.0) } else { 0.0 };
    [right * g, left * g]
}

/// Builds both first-person arms relative to the camera (camera at the origin).
/// Vertices: right arm first (72), then left arm (72).
pub fn build_first_person_hands(pose: &HandsPose) -> Vec<ModelVertex> {
    let frame = camera_frame(pose);
    let ext = punch_extension(pose);
    let (skin_c, _, _, sleeve_c, _) = skin_colors(pose.skin);

    let mut v = Vec::with_capacity(4 * 36);
    for (side, jab) in [(1.0f32, ext[0]), (-1.0, ext[1])] {
        let (pivot, angle) = arm_place(pose, side, jab);
        let (x, y, z) = (pivot.x, pivot.y, pivot.z);
        // A held item replaces the hand: only empty hands are drawn as arms.
        let item_in_hand = if side > 0.0 { pose.holding } else { pose.shield };
        if !item_in_hand {
            // Square arm like the third-person body: sleeve with a slightly wider hand.
            add_box(&mut v, &frame, Vec3::new(x - 0.07, y - 0.07, z), Vec3::new(x + 0.07, y + 0.07, z + 0.26), pivot, angle, sleeve_c);
            add_box(&mut v, &frame, Vec3::new(x - 0.075, y - 0.075, z + 0.25), Vec3::new(x + 0.075, y + 0.075, z + 0.36), pivot, angle, skin_c);
        }
        let f = Vec3::new(x, y, z + 0.29); // fist centre before the tilt
        if side > 0.0 {
            if let Some((kind, mat)) = pose.tool {
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
    }
    v
}

/// Tool held upright in the fist; boxes share the forearm tilt.
fn add_tool(v: &mut Vec<ModelVertex>, frame: &Frame, f: Vec3, pivot: Vec3, angle: f32, kind: u8, mat: u8) {
    let sprite = wolfcraft_assets::items::TOOLS_BASE as usize + mat as usize * 4 + kind as usize;
    if let Some(px) = textures().and_then(|t| t.item_pixels.get(sprite).cloned()).filter(|p| p.len() == 256) {
        // 3D item: every opaque icon pixel is a little box. The icon's handle (bottom-left)
        // sits in the fist; its right edge points left and a bit away, so the tip leans
        // towards the crosshair.
        let a = 0.42f32;
        let u = Vec3::new(-a.cos(), 0.0, a.sin());
        let w = Vec3::Y;
        let n = u.cross(w).normalize();
        let s = 0.36;
        let o = f + Vec3::new(0.03, -0.10, 0.0);
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
        HandsPose { forward: Vec3::new(0.3, -0.2, -1.0).normalize(), up: Vec3::Y, punch_cycle: cycle, guard, bob_phase: 0.0, bob_amount: 0.0, holding: false, tool: None, icon: None, shield: false, blocking: 0.0, place: 0.0, skin: 0 }
    }

    /// Furthest reach of an arm along the view direction.
    fn reach(verts: &[ModelVertex], fwd: Vec3) -> f32 {
        verts.iter().map(|v| Vec3::from(v.pos).dot(fwd)).fold(f32::MIN, f32::max)
    }

    #[test]
    fn hands_face_outwards_and_stay_in_front_of_the_camera() {
        for (c, g) in [(0.0, 0.0), (0.25, 1.0), (0.75, 1.0), (0.4, 0.5)] {
            let p = hands(c, g);
            let verts = build_first_person_hands(&p);
            assert_eq!(verts.len(), 4 * 36);
            for t in verts.chunks(3) {
                let (a, b, cc) = (Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos));
                let n = (b - a).cross(cc - a).normalize_or_zero();
                assert!(n.dot(Vec3::from(t[0].normal)) > 0.99, "inside-out triangle");
            }
            let fwd = p.forward.normalize();
            assert!(verts.iter().all(|v| Vec3::from(v.pos).dot(fwd) > 0.08), "arm behind the near plane");
        }
    }

    #[test]
    fn sword_swings_and_shield_comes_to_the_middle_when_blocking() {
        let mut p = hands(0.0, 1.0);
        p.holding = true;
        p.tool = Some((3, 2));
        p.shield = true;
        let fwd = p.forward.normalize();
        let raised = build_first_person_hands(&p);
        p.punch_cycle = 0.5;
        let struck = build_first_person_hands(&p);
        assert!(raised.len() > 4 * 36, "sword and shield add boxes");
        assert!(raised.iter().zip(&struck).take(72).any(|(a, b)| a.pos != b.pos), "the sword arm swings");
        // Shield boxes are the last 3 boxes: their centre moves towards the view axis when blocking.
        let centre = |v: &[ModelVertex]| {
            let s = &v[v.len() - 3 * 36..];
            let c = s.iter().fold(Vec3::ZERO, |a, v| a + Vec3::from(v.pos)) / s.len() as f32;
            (c - fwd * c.dot(fwd)).length()
        };
        let down = centre(&build_first_person_hands(&p));
        p.blocking = 1.0;
        let up = centre(&build_first_person_hands(&p));
        assert!(up < down - 0.1, "shield must move to the middle ({down} -> {up})");
    }

    #[test]
    fn arms_take_turns_punching() {
        let fwd = hands(0.0, 1.0).forward.normalize();
        let r = build_first_person_hands(&hands(0.25, 1.0));
        let l = build_first_person_hands(&hands(0.75, 1.0));
        let (r_right, r_left) = (reach(&r[..72], fwd), reach(&r[72..], fwd));
        let (l_right, l_left) = (reach(&l[..72], fwd), reach(&l[72..], fwd));
        assert!(r_right > r_left + 0.2, "right jab at a quarter cycle ({r_right} vs {r_left})");
        assert!(l_left > l_right + 0.2, "left jab at three quarters ({l_left} vs {l_right})");
        assert_eq!(punch_extension(&hands(0.25, 1.0)), [1.0, 0.0]);
    }

    #[test]
    fn no_punching_at_rest_and_guard_raises_arms() {
        let rest_a = build_first_person_hands(&hands(0.0, 0.0));
        let rest_b = build_first_person_hands(&hands(0.3, 0.0));
        assert!(rest_a.iter().zip(&rest_b).all(|(a, b)| a.pos == b.pos), "cycle must not matter at rest");
        let guard = build_first_person_hands(&hands(0.0, 1.0));
        let top = |v: &[ModelVertex]| v.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
        assert!(top(&guard) > top(&rest_a) + 0.1, "guard lifts the fists");
    }

    fn pose(up: DVec3) -> PlayerPose {
        let heading = if up.y.abs() > 0.9 { DVec3::X } else { DVec3::Y.cross(up).normalize() };
        PlayerPose { feet: up * 1000.0, heading, up, pitch: 0.0, phase: 0.0, amount: 0.0, sneaking: false, swimming: false, skin: 0 }
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
        add_box(&mut v, &f, Vec3::new(-h, -h, -h), Vec3::new(h, h, h), Vec3::ZERO, d.spin + d.age * 6.0, d.color);
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
