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
    /// Resting: 0 no, 1 lying on the back (asleep in a bed; `feet` is the middle of the bed
    /// top, `heading` points to the pillow), 2 sitting (`feet` is the seat top).
    pub rest: u8,
    /// Skin preset (see `SKIN_NAMES`).
    pub skin: u8,
    /// Swing of the right arm 0..1 (0 = none); the motion depends on what is held.
    pub attack: f32,
    /// What the right hand holds (drawn in it, and it picks the swing).
    pub held: Held,
    /// Worn armour per slot (helmet, chestplate, leggings, boots): material 0 leather,
    /// 1 bronze, 2 iron, 3 diamond.
    pub armor: [Option<u8>; 4],
    /// Shield in the second hand (drawn on the left forearm).
    pub shield: bool,
}

/// Leg angle while sitting (forward from the hips).
const SIT_LEG: f32 = -1.45;

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
    /// A weapon drawn as a box-built 3D model (same `kind` as `GunPose`): grenade launcher 0,
    /// bazooka 1, pistol 2, rifle 3, shotgun 4, sniper 5, star blaster 6, star remote 7,
    /// star hammer 8.
    Gun(u8),
}

/// Aiming pose of the arms with a weapon: (right arm, left arm or `None` if it hangs free).
fn gun_arms(kind: u8, pitch: f32) -> (f32, Option<f32>) {
    let aim = -1.5 - pitch.clamp(-1.0, 1.0) * 0.8;
    match kind {
        // The remote is held out in one hand, the hammer rests on the shoulder.
        7 => (-1.1, None),
        8 => (-0.5, None),
        13 => (-0.9, None),
        _ => (aim, Some(aim + 0.25)),
    }
}

/// A weapon in the hand, built from boxes (third person). In weapon space x is right, y the
/// top of the weapon and z its barrel; with the arm raised forward the barrel points ahead.
fn add_gun_3p(v: &mut Vec<ModelVertex>, frame: &Frame, hand: Vec3, pivot: Vec3, angle: f32, kind: u8) {
    const DARK: [f32; 3] = [0.13, 0.14, 0.15];
    const GUN: [f32; 3] = [0.24, 0.25, 0.27];
    const STEEL: [f32; 3] = [0.45, 0.47, 0.50];
    const WOOD: [f32; 3] = [0.50, 0.33, 0.18];
    const OLIVE: [f32; 3] = [0.34, 0.40, 0.22];
    const WHITE: [f32; 3] = [0.82, 0.85, 0.92];
    const CYAN: [f32; 3] = [0.40, 0.95, 1.0];
    // Weapon space → arm space: barrel (z) along the arm, away from the shoulder (−y);
    // the weapon's top (y) to the arm's front (+z).
    let mut b = |a: [f32; 3], c: [f32; 3], col: [f32; 3]| {
        let lo = hand + Vec3::new(a[0], -c[2], a[1]);
        let hi = hand + Vec3::new(c[0], -a[2], c[1]);
        add_box(v, frame, lo, hi, pivot, angle, col);
    };
    match kind {
        0 => {
            b([-0.06, -0.05, -0.15], [0.06, 0.07, 0.35], OLIVE);
            b([-0.07, -0.06, 0.05], [0.07, 0.08, 0.20], DARK);
            b([-0.03, -0.20, -0.05], [0.03, -0.05, 0.03], DARK);
            b([-0.03, -0.04, -0.40], [0.03, 0.04, -0.15], WOOD);
        }
        1 => {
            b([-0.09, -0.09, -0.45], [0.09, 0.09, 0.55], OLIVE);
            b([-0.10, -0.10, 0.50], [0.10, 0.10, 0.60], DARK);
            b([-0.10, -0.10, -0.50], [0.10, 0.10, -0.42], DARK);
            b([-0.03, 0.09, 0.00], [0.06, 0.17, 0.12], DARK);
            b([-0.03, -0.22, 0.02], [0.03, -0.09, 0.08], DARK);
            b([-0.05, -0.05, 0.60], [0.05, 0.05, 0.66], [0.80, 0.18, 0.14]);
        }
        2 => {
            b([-0.035, 0.00, -0.02], [0.035, 0.07, 0.24], GUN);
            b([-0.03, -0.16, -0.02], [0.03, 0.00, 0.06], DARK);
            b([-0.012, 0.00, 0.24], [0.012, 0.03, 0.27], DARK);
        }
        3 => {
            b([-0.045, -0.05, -0.08], [0.045, 0.06, 0.32], DARK);
            b([-0.04, -0.04, 0.32], [0.04, 0.04, 0.58], OLIVE);
            b([-0.015, -0.015, 0.58], [0.015, 0.015, 0.70], DARK);
            b([-0.03, -0.20, 0.10], [0.03, -0.05, 0.18], GUN);
            b([-0.035, -0.14, -0.04], [0.035, -0.05, 0.03], DARK);
            b([-0.04, -0.07, -0.40], [0.04, 0.04, -0.08], GUN);
            b([-0.012, 0.06, 0.05], [0.012, 0.10, 0.25], DARK);
        }
        4 => {
            b([-0.045, -0.05, -0.05], [0.045, 0.06, 0.25], GUN);
            b([-0.025, 0.00, 0.25], [0.025, 0.05, 0.70], STEEL);
            b([-0.03, -0.06, 0.30], [0.03, 0.00, 0.55], WOOD);
            b([-0.045, -0.09, -0.42], [0.045, 0.04, -0.05], WOOD);
            b([-0.03, -0.15, -0.02], [0.03, -0.05, 0.05], DARK);
        }
        5 => {
            b([-0.045, -0.05, -0.08], [0.045, 0.06, 0.30], OLIVE);
            b([-0.015, -0.015, 0.30], [0.015, 0.015, 0.85], DARK);
            b([-0.03, 0.06, 0.00], [0.03, 0.12, 0.26], DARK);
            b([-0.025, 0.065, 0.26], [0.025, 0.115, 0.27], [0.45, 0.80, 1.0]);
            b([-0.045, -0.10, -0.50], [0.045, 0.04, -0.08], OLIVE);
            b([-0.03, -0.16, 0.00], [0.03, -0.05, 0.07], DARK);
        }
        6 => {
            b([-0.05, -0.02, -0.04], [0.05, 0.08, 0.32], WHITE);
            b([-0.05, 0.08, 0.00], [0.05, 0.10, 0.26], GUN);
            b([0.05, 0.005, 0.06], [0.066, 0.06, 0.22], CYAN);
            b([-0.035, -0.005, 0.32], [0.035, 0.065, 0.42], GUN);
            b([-0.012, 0.018, 0.42], [0.012, 0.042, 0.43], CYAN);
            b([-0.03, -0.17, -0.02], [0.03, -0.02, 0.07], GUN);
        }
        9 => {
            b([-0.045, -0.05, -0.06], [0.045, 0.06, 0.36], [0.20, 0.23, 0.30]);
            b([-0.035, -0.035, 0.36], [0.035, 0.045, 0.70], GUN);
            for k in 0..4 {
                let z = 0.40 + k as f32 * 0.07;
                b([-0.045, -0.02, z], [0.045, 0.03, z + 0.025], CYAN);
            }
            b([-0.03, -0.17, -0.02], [0.03, -0.05, 0.05], DARK);
            b([-0.04, -0.10, -0.38], [0.04, 0.04, -0.06], [0.20, 0.23, 0.30]);
        }
        11 => {
            b([-0.045, -0.04, -0.06], [0.045, 0.06, 0.24], [0.20, 0.23, 0.30]);
            b([-0.03, -0.17, -0.02], [0.03, -0.04, 0.06], DARK);
            b([-0.02, -0.01, 0.24], [0.02, 0.03, 0.50], STEEL);
        }
        12 => {
            b([-0.055, -0.06, -0.10], [0.055, 0.07, 0.20], [0.95, 0.55, 0.15]);
            b([-0.012, -0.02, 0.20], [0.012, 0.05, 0.60], STEEL);
            b([-0.03, -0.17, -0.08], [0.03, -0.06, 0.0], DARK);
        }
        14 => {
            b([-0.05, -0.05, -0.08], [0.05, 0.07, 0.20], [0.20, 0.23, 0.30]);
            b([-0.03, -0.17, -0.02], [0.03, -0.04, 0.06], DARK);
            b([-0.07, 0.00, 0.20], [0.07, 0.04, 0.30], STEEL);
            b([-0.015, -0.01, 0.30], [0.015, 0.03, 0.46], [1.0, 0.8, 0.25]);
        }
        15 => {
            b([-0.045, -0.06, -0.10], [0.045, 0.06, 0.30], [0.20, 0.23, 0.30]);
            b([-0.03, -0.03, 0.30], [0.03, 0.03, 0.50], [0.95, 0.25, 0.10]);
            b([-0.03, -0.17, -0.02], [0.03, -0.05, 0.05], DARK);
        }
        13 => {
            b([-0.02, -0.02, -0.12], [0.02, 0.02, 0.02], DARK);
            b([-0.06, -0.025, 0.02], [0.06, 0.025, 0.05], [1.0, 0.8, 0.25]);
            b([-0.016, -0.008, 0.05], [0.016, 0.008, 0.62], CYAN);
        }
        10 => {
            b([-0.05, -0.07, -0.08], [0.05, 0.07, 0.34], [0.20, 0.23, 0.30]);
            for sx in [-1.0f32, 1.0] {
                b([sx * 0.034 - 0.012, -0.04, 0.34], [sx * 0.034 + 0.012, 0.05, 0.95], STEEL);
            }
            b([-0.02, -0.015, 0.36], [0.02, 0.025, 0.92], [0.30, 0.50, 1.0]);
            b([-0.03, -0.18, -0.02], [0.03, -0.07, 0.05], DARK);
            b([-0.04, -0.13, -0.46], [0.04, 0.045, -0.08], [0.20, 0.23, 0.30]);
        }
        7 => {
            b([-0.055, -0.03, -0.02], [0.055, 0.02, 0.18], [0.12, 0.13, 0.18]);
            b([-0.022, 0.02, 0.04], [0.022, 0.034, 0.08], [0.95, 0.12, 0.10]);
            b([-0.03, 0.02, 0.025], [0.03, 0.085, 0.032], [0.55, 0.60, 0.70]);
            b([-0.045, 0.02, 0.11], [0.045, 0.025, 0.158], [0.45, 0.70, 1.0]);
            b([-0.003, 0.025, 0.15], [0.003, 0.11, 0.156], [0.32, 0.34, 0.40]);
            b([-0.014, 0.11, 0.14], [0.014, 0.13, 0.167], [1.0, 0.92, 0.45]);
        }
        _ => {
            // Hammer: handle along the arm, the head at its end.
            b([-0.02, -0.02, -0.10], [0.02, 0.02, 0.55], [0.42, 0.26, 0.14]);
            b([-0.13, -0.08, 0.55], [0.13, 0.08, 0.72], [0.70, 0.73, 0.80]);
            b([-0.15, -0.04, 0.60], [0.15, 0.04, 0.67], [0.55, 0.85, 1.0]);
        }
    }
}

/// Colour of armour of material `m` (leather, bronze, iron, diamond).
pub fn armor_color(m: u8) -> [f32; 3] {
    match m {
        0 => [0.55, 0.36, 0.20],
        1 => [0.80, 0.56, 0.26],
        2 => [0.80, 0.81, 0.84],
        // WolfTech: nano plating (graphite with a cyan sheen) and the jetpack's steel.
        4 => [0.22, 0.26, 0.34],
        5 => [0.45, 0.47, 0.52],
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
pub const SKIN_NAMES: [&str; 6] = ["Искатель", "Огонёк", "Лесник", "Мечта", "Солнце", "Тень"];

/// (skin tone, hair, jacket, sleeve, trousers) of skin preset `i`.
pub fn skin_colors(i: u8) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
    match i {
        1 => ([0.95, 0.78, 0.64], [0.85, 0.28, 0.12], [0.92, 0.26, 0.24], [0.80, 0.20, 0.20], [0.24, 0.26, 0.36]),
        2 => ([0.82, 0.62, 0.46], [0.42, 0.26, 0.12], [0.30, 0.62, 0.30], [0.24, 0.52, 0.24], [0.40, 0.30, 0.20]),
        3 => ([0.98, 0.84, 0.74], [0.95, 0.55, 0.78], [0.62, 0.42, 0.90], [0.52, 0.34, 0.80], [0.30, 0.30, 0.52]),
        4 => ([0.74, 0.52, 0.38], [0.98, 0.80, 0.30], [1.0, 0.70, 0.18], [0.92, 0.58, 0.12], [0.22, 0.42, 0.70]),
        5 => ([0.52, 0.36, 0.27], [0.10, 0.10, 0.14], [0.22, 0.22, 0.30], [0.16, 0.16, 0.24], [0.12, 0.12, 0.16]),
        _ => (SKIN, HAIR, JACKET, SLEEVE, TROUSERS),
    }
}

const SKIN: [f32; 3] = [0.94, 0.76, 0.62];
const HAIR: [f32; 3] = [0.36, 0.22, 0.12];
const JACKET: [f32; 3] = [0.12, 0.62, 0.68];
const SLEEVE: [f32; 3] = [0.10, 0.54, 0.60];
const TROUSERS: [f32; 3] = [0.20, 0.24, 0.40];
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
    add_box_opts(out, f, min, max, pivot, angle, color, if rounded { 0.26 } else { 0.0 }, true);
}

/// Character part: flat colour (no skin texture), a soft chamfer in the cartoon style.
#[allow(clippy::too_many_arguments)]
fn add_char_box(out: &mut Vec<ModelVertex>, f: &Frame, min: Vec3, max: Vec3, pivot: Vec3, angle: f32, color: [f32; 3]) {
    let r = if ROUNDED.load(std::sync::atomic::Ordering::Relaxed) { 0.16 } else { 0.0 };
    add_box_opts(out, f, min, max, pivot, angle, color, r, false);
}

/// First-person arm part: flat colour (no skin patches) and only a small chamfer.
#[allow(clippy::too_many_arguments)]
fn add_hand_box(out: &mut Vec<ModelVertex>, f: &Frame, min: Vec3, max: Vec3, pivot: Vec3, angle: f32, color: [f32; 3]) {
    let r = if ROUNDED.load(std::sync::atomic::Ordering::Relaxed) { 0.10 } else { 0.0 };
    add_box_opts(out, f, min, max, pivot, angle, color, r, false);
}

/// `round`: chamfer radius as a share of the box's thinnest side (0 = sharp box);
/// `textured`: skin colours are painted from the player skin.
#[allow(clippy::too_many_arguments)]
fn add_box_opts(out: &mut Vec<ModelVertex>, f: &Frame, min: Vec3, max: Vec3, pivot: Vec3, angle: f32, color: [f32; 3], round_k: f32, textured: bool) {
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
    let skin = if textured { textures().and_then(|t| t.player).zip(skin_rows(color)) } else { None };
    let size = max - min;
    // Cartoon style: chamfered edges, radius from the part's thinnest side (thin overlays stay sharp).
    let round = (round_k > 0.0 && size.min_element() >= 0.05).then(|| (size.min_element() * round_k).min(0.08));
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
                // Low poly: every facet is flat.
                let _ = s;
                let ln = n;
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
                    let _fb = b * 2 + (sb < 0.0) as usize;
                    let corner = |sc: f32| { let mut s = Vec3::ZERO; s[a] = sa; s[b] = sb; s[c] = sc; s };
                    let (c0, c1) = (corner(-1.0), corner(1.0));
                    let (na, nb) = (face_n(a, sa), face_n(b, sb));
                    let mid = (na + nb).normalize();
                    let uv0 = face_uv[fa][sign_index(c0)];
                    let uv1 = face_uv[fa][sign_index(c1)];
                    let p = |s: Vec3, axis: usize| inset_s(s, axis, r);
                    let blend = |_nf: Vec3, _s: Vec3| mid;
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
        let pts: Vec<(Vec3, Vec3, [f32; 2])> = (0..3).map(|axis| (inset_s(s, axis, r), nc, uv)).collect();
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
    let asleep = pose.rest == 1;
    let sitting = pose.rest == 2;
    if asleep {
        // On the back: the head towards the pillow, the face to the sky, breathing slowly.
        let breath = (pose.phase * 0.5).sin() as f64 * 0.012;
        frame = Frame { origin: pose.feet - camera - fwd * 0.9 + up * (0.24 + breath), right: -right, up: fwd, fwd: up };
    } else if sitting {
        // The hips rest on the seat.
        frame.origin = pose.feet - camera - up * 0.62;
    }
    let swim = pose.swimming;
    let swing = if asleep || sitting { 0.0 } else if swim { 0.5 * (pose.phase * 0.8).sin() } else { pose.phase.sin() * 0.85 * pose.amount };
    let crouch = if pose.sneaking { 0.12 } else { 0.0 };
    let lean = if pose.sneaking { 0.0 } else { 0.0 };
    let _ = lean;

    // Cartoon look: flat colours (no skin texture), soft chamfers.
    let shirt = [(jacket_c[0] * 0.35 + 0.65).min(1.0), (jacket_c[1] * 0.35 + 0.65).min(1.0), (jacket_c[2] * 0.35 + 0.65).min(1.0)];
    let dark = |c: [f32; 3], k: f32| [c[0] * k, c[1] * k, c[2] * k];
    let style = pose.skin % 3;
    // Legs (pivot at the hip): trousers and sneakers with a white sole.
    let hip = 0.68;
    for (side, ang) in [(-1.0f32, swing), (1.0, -swing)] {
        // Sitting: the legs point forward from the hips.
        let ang = if sitting { SIT_LEG } else { ang };
        let x0 = if side < 0.0 { -0.25 } else { 0.0 };
        let lp = Vec3::new(x0 + 0.125, hip, 0.0);
        add_char_box(&mut v, &frame, Vec3::new(x0 + 0.01, 0.12, -0.12), Vec3::new(x0 + 0.24, hip, 0.12), lp, ang, trousers_c);
        add_char_box(&mut v, &frame, Vec3::new(x0, 0.04, -0.13), Vec3::new(x0 + 0.25, 0.17, 0.17), lp, ang, dark(BOOTS, 1.25));
        add_char_box(&mut v, &frame, Vec3::new(x0 - 0.005, 0.0, -0.135), Vec3::new(x0 + 0.255, 0.05, 0.18), lp, ang, [0.95, 0.95, 0.93]);
    }
    // Torso: an open jacket over a light shirt, a zip line and a hood behind the neck.
    let ty0 = hip - crouch * 0.4;
    add_char_box(&mut v, &frame, Vec3::new(-0.25, ty0, -0.125), Vec3::new(0.25, ty0 + 0.62, 0.125), Vec3::ZERO, 0.0, jacket_c);
    add_char_box(&mut v, &frame, Vec3::new(-0.09, ty0 + 0.08, 0.12), Vec3::new(0.09, ty0 + 0.6, 0.135), Vec3::ZERO, 0.0, shirt);
    add_char_box(&mut v, &frame, Vec3::new(-0.11, ty0 + 0.08, 0.125), Vec3::new(-0.09, ty0 + 0.6, 0.138), Vec3::ZERO, 0.0, dark(jacket_c, 0.7));
    add_char_box(&mut v, &frame, Vec3::new(0.09, ty0 + 0.08, 0.125), Vec3::new(0.11, ty0 + 0.6, 0.138), Vec3::ZERO, 0.0, dark(jacket_c, 0.7));
    add_char_box(&mut v, &frame, Vec3::new(-0.2, ty0 + 0.48, -0.17), Vec3::new(0.2, ty0 + 0.64, -0.1), Vec3::ZERO, 0.0, dark(jacket_c, 0.85));
    add_char_box(&mut v, &frame, Vec3::new(-0.255, ty0, -0.13), Vec3::new(0.255, ty0 + 0.07, 0.13), Vec3::ZERO, 0.0, dark(trousers_c, 0.7));
    // Arms (pivot at the shoulder), opposite phase to the legs.
    let shoulder = ty0 + 0.6;
    for (side, ang) in [(-1.0f32, -swing), (1.0, swing)] {
        // Crawl stroke: the arms circle all the way round, one half a turn behind the other.
        let ang = if swim { pose.phase * 0.8 + if side < 0.0 { 0.0 } else { std::f32::consts::PI } + 2.6 } else { ang };
        // Attack: the right arm's motion depends on the tool. Holding something keeps the arm a
        // little forward, as in Minecraft.
        let a = pose.attack.clamp(0.0, 1.0);
        // A weapon: both arms come up to aim it (one-handed ones only the right).
        if let (Held::Gun(k), false) = (pose.held, swim) {
            let (r, l) = gun_arms(k, pose.pitch);
            let kick = -0.25 * (a * std::f32::consts::PI).sin();
            let ang = if side > 0.0 { r + kick } else { l.map(|l| l + kick).unwrap_or(ang) };
            let x0 = if side < 0.0 { -0.48 } else { 0.25 };
            let pivot = Vec3::new(x0 + 0.115, shoulder, 0.0);
            // Two-handed: the left arm reaches in towards the weapon.
            let x_in = if side < 0.0 && l.is_some() { 0.12 } else { 0.0 };
            add_char_box(&mut v, &frame, Vec3::new(x0 + x_in, ty0 + 0.18, -0.115), Vec3::new(x0 + x_in + 0.23, shoulder + 0.02, 0.115), pivot, ang, sleeve_c);
            add_char_box(&mut v, &frame, Vec3::new(x0 + x_in - 0.005, ty0 + 0.13, -0.12), Vec3::new(x0 + x_in + 0.235, ty0 + 0.2, 0.12), pivot, ang, dark(sleeve_c, 0.72));
            add_char_box(&mut v, &frame, Vec3::new(x0 + x_in + 0.015, ty0 - 0.02, -0.1), Vec3::new(x0 + x_in + 0.215, ty0 + 0.14, 0.1), pivot, ang, skin_c);
            if side > 0.0 {
                add_held(&mut v, &frame, Vec3::new(x0 + 0.115, ty0 + 0.02, 0.0), pivot, ang, pose.held);
            }
            continue;
        }
        let hold = if side > 0.0 && pose.held != Held::Nothing && !swim { -0.3 } else { 0.0 };
        let ang = if side > 0.0 && a > 0.0 && !swim {
            let k = (a * std::f32::consts::PI).sin();
            ang * (1.0 - k) + swing_angle(a, pose.held) + hold * (1.0 - k)
        } else {
            ang * if hold != 0.0 { 0.4 } else { 1.0 } + hold
        };
        let x0 = if side < 0.0 { -0.48 } else { 0.25 };
        let pivot = Vec3::new(x0 + 0.115, shoulder, 0.0);
        add_char_box(&mut v, &frame, Vec3::new(x0, ty0 + 0.18, -0.115), Vec3::new(x0 + 0.23, shoulder + 0.02, 0.115), pivot, ang, sleeve_c);
        add_char_box(&mut v, &frame, Vec3::new(x0 - 0.005, ty0 + 0.13, -0.12), Vec3::new(x0 + 0.235, ty0 + 0.2, 0.12), pivot, ang, dark(sleeve_c, 0.72));
        add_char_box(&mut v, &frame, Vec3::new(x0 + 0.015, ty0 - 0.02, -0.1), Vec3::new(x0 + 0.215, ty0 + 0.14, 0.1), pivot, ang, skin_c);
        // Five fingers: four in a row under the palm (front to back, the middle one longest),
        // the thumb on the front, a little apart. The palm faces the body.
        let (fx0, fx1) = if side < 0.0 { (x0 + 0.11, x0 + 0.21) } else { (x0 + 0.02, x0 + 0.12) };
        for &(zc, len) in &[(-0.07f32, 0.055f32), (-0.024, 0.075), (0.022, 0.085), (0.066, 0.07)] {
            add_char_box(&mut v, &frame, Vec3::new(fx0, ty0 - 0.02 - len, zc - 0.019), Vec3::new(fx1, ty0, zc + 0.019), pivot, ang, skin_c);
        }
        add_char_box(&mut v, &frame, Vec3::new(fx0, ty0 + 0.0, 0.09), Vec3::new(fx1, ty0 + 0.09, 0.135), pivot, ang, skin_c);
        if let Some(m) = pose.armor[1] {
            // Chestplate shoulder pads.
            add_box(&mut v, &frame, Vec3::new(x0 - 0.03, shoulder - 0.28, -0.155), Vec3::new(x0 + 0.26, shoulder + 0.03, 0.155), pivot, ang, armor_color(m));
        }
        if side > 0.0 && !swim {
            add_held(&mut v, &frame, Vec3::new(x0 + 0.115, ty0 + 0.02, 0.0), pivot, ang, pose.held);
        }
        if side < 0.0 && pose.shield && !swim {
            // Shield strapped to the left forearm, its face to the front: planks, iron rim
            // and boss.
            let cx = x0 + 0.115;
            add_box(&mut v, &frame, Vec3::new(cx - 0.21, ty0 - 0.20, 0.13), Vec3::new(cx + 0.21, ty0 + 0.44, 0.18), pivot, ang, IRON);
            add_box(&mut v, &frame, Vec3::new(cx - 0.19, ty0 - 0.18, 0.125), Vec3::new(cx + 0.19, ty0 + 0.42, 0.185), pivot, ang, PLANK);
            add_box(&mut v, &frame, Vec3::new(cx - 0.05, ty0 + 0.07, 0.18), Vec3::new(cx + 0.05, ty0 + 0.17, 0.21), pivot, ang, IRON);
        }
    }
    // Armour over the body.
    if let Some(m) = pose.armor[1] {
        add_box(&mut v, &frame, Vec3::new(-0.28, ty0 + 0.06, -0.155), Vec3::new(0.28, ty0 + 0.66, 0.155), Vec3::ZERO, 0.0, armor_color(m));
    }
    if pose.armor[2].is_some() || pose.armor[3].is_some() {
        for (side, lang) in [(-1.0f32, swing), (1.0, -swing)] {
            let lang = if sitting { SIT_LEG } else { lang };
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
    // Head follows the look pitch, pivoting at the neck: a big cartoon head with large eyes.
    let neck = ty0 + 0.62;
    let head_ang = if swim { -1.3 - pose.pitch * 0.8 } else { -pose.pitch * 0.9 };
    let hp = Vec3::new(0.0, neck, 0.0);
    let (hw, hh) = (0.28f32, 0.54f32);
    let fz = hw; // the face plane
    add_char_box(&mut v, &frame, Vec3::new(-hw, neck, -hw), Vec3::new(hw, neck + hh, hw), hp, head_ang, skin_c);
    // Eyes: white, a coloured pupil looking a little inward, a light glint.
    for ex in [-0.17f32, 0.05] {
        let ey = neck + 0.2;
        add_char_box(&mut v, &frame, Vec3::new(ex, ey, fz), Vec3::new(ex + 0.12, ey + 0.13, fz + 0.012), hp, head_ang, [0.98, 0.98, 0.98]);
        let px = if ex < 0.0 { ex + 0.05 } else { ex + 0.02 };
        add_char_box(&mut v, &frame, Vec3::new(px, ey + 0.01, fz + 0.01), Vec3::new(px + 0.06, ey + 0.1, fz + 0.02), hp, head_ang, [0.18, 0.24, 0.42]);
        add_char_box(&mut v, &frame, Vec3::new(px + 0.03, ey + 0.065, fz + 0.019), Vec3::new(px + 0.05, ey + 0.088, fz + 0.024), hp, head_ang, [1.0, 1.0, 1.0]);
        // Eyebrow.
        add_char_box(&mut v, &frame, Vec3::new(ex, ey + 0.16, fz), Vec3::new(ex + 0.12, ey + 0.19, fz + 0.012), hp, head_ang, dark(hair_c, 0.8));
        // Blush.
        add_char_box(&mut v, &frame, Vec3::new(ex + if ex < 0.0 { -0.04 } else { 0.08 }, neck + 0.12, fz), Vec3::new(ex + if ex < 0.0 { 0.04 } else { 0.16 }, neck + 0.165, fz + 0.008), hp, head_ang, [0.98, 0.62, 0.60]);
    }
    // Smile.
    add_char_box(&mut v, &frame, Vec3::new(-0.06, neck + 0.1, fz), Vec3::new(0.06, neck + 0.125, fz + 0.01), hp, head_ang, [0.55, 0.22, 0.20]);
    for sx in [-0.08f32, 0.06] {
        add_char_box(&mut v, &frame, Vec3::new(sx, neck + 0.12, fz), Vec3::new(sx + 0.02, neck + 0.145, fz + 0.01), hp, head_ang, [0.55, 0.22, 0.20]);
    }
    // Hair: a cap over the top and back, a fringe, and the style of the preset.
    add_char_box(&mut v, &frame, Vec3::new(-hw - 0.015, neck + hh - 0.12, -hw - 0.015), Vec3::new(hw + 0.015, neck + hh + 0.06, hw + 0.015), hp, head_ang, hair_c);
    add_char_box(&mut v, &frame, Vec3::new(-hw - 0.015, neck + 0.18, -hw - 0.015), Vec3::new(hw + 0.015, neck + hh, -hw + 0.08), hp, head_ang, hair_c);
    for (i, fx) in [-0.26f32, -0.12, 0.02, 0.14].iter().enumerate() {
        let drop = [0.1f32, 0.06, 0.09, 0.05][i];
        add_char_box(&mut v, &frame, Vec3::new(*fx, neck + hh - 0.12 - drop, fz - 0.02), Vec3::new(fx + 0.13, neck + hh - 0.1, fz + 0.016), hp, head_ang, hair_c);
    }
    match style {
        // Long hair down the back and over the sides.
        1 => {
            add_char_box(&mut v, &frame, Vec3::new(-hw - 0.02, neck - 0.18, -hw - 0.02), Vec3::new(hw + 0.02, neck + 0.2, -hw + 0.06), hp, head_ang, hair_c);
            for sx in [-hw - 0.02, hw - 0.03] {
                add_char_box(&mut v, &frame, Vec3::new(sx, neck + 0.05, -hw), Vec3::new(sx + 0.05, neck + hh - 0.08, hw - 0.06), hp, head_ang, hair_c);
            }
        }
        // Spiky top.
        2 => {
            for (sx, sz) in [(-0.18f32, -0.1f32), (0.02, -0.14), (-0.06, 0.06), (0.12, 0.02)] {
                add_char_box(&mut v, &frame, Vec3::new(sx, neck + hh + 0.05, sz), Vec3::new(sx + 0.12, neck + hh + 0.16, sz + 0.12), hp, head_ang, hair_c);
            }
        }
        // Short: side tufts over the ears.
        _ => {
            for sx in [-hw - 0.02, hw - 0.03] {
                add_char_box(&mut v, &frame, Vec3::new(sx, neck + 0.28, -hw + 0.04), Vec3::new(sx + 0.05, neck + hh - 0.08, hw - 0.1), hp, head_ang, hair_c);
            }
        }
    }
    if let Some(m) = pose.armor[0] {
        // Helmet: cap over the top and the back of the head, the face stays free.
        let c = armor_color(m);
        add_box(&mut v, &frame, Vec3::new(-0.315, neck + 0.4, -0.315), Vec3::new(0.315, neck + 0.62, 0.315), hp, head_ang, c);
        add_box(&mut v, &frame, Vec3::new(-0.315, neck + 0.08, -0.315), Vec3::new(0.315, neck + 0.4, -0.14), hp, head_ang, c);
        for sx in [-0.315f32, 0.245] {
            add_box(&mut v, &frame, Vec3::new(sx, neck + 0.12, -0.315), Vec3::new(sx + 0.07, neck + 0.4, 0.22), hp, head_ang, c);
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
        Held::Gun(kind) => {
            add_gun_3p(v, frame, hand + Vec3::new(0.0, -0.04, 0.0), pivot, angle, kind);
            return;
        }
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
    add_voxel_icon(v, frame, &px, o, u, w, n, s, icon_depth(sprite), pivot, angle);
}

/// Builds all creatures relative to `camera` (original blocky designs, flat colours).
/// `forward`: camera view direction; mobs behind the camera or beyond `max_dist` are skipped
/// (their triangles would be thrown away by the GPU anyway).
pub fn build_mob_models(mobs: &[crate::mobs::Mob], camera: DVec3, forward: DVec3, max_dist: f64) -> Vec<ModelVertex> {
    use crate::mobs::MobKind;
    let mut v = Vec::with_capacity(mobs.len() * 10 * 36);
    for m in mobs {
        if m.hidden {
            continue;
        }
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
        if let Some(role)=&m.boss {
            add_guardian(&mut v,&f,role,m.anim,m.amount,m.hp,&tint);
            continue;
        }
        match m.kind {
            MobKind::Ghoul => {
                // Cartoon ghoul: a shambling figure with the player's build, green skin, torn
                // clothes, arms reaching forward and big glowing eyes.
                let skin = tint([0.52, 0.70, 0.45]);
                let skin_d = tint([0.40, 0.56, 0.35]);
                let shirt = tint([0.36, 0.42, 0.62]);
                let pants = tint([0.30, 0.26, 0.36]);
                let glow = [1.0, 0.88, 0.30];
                for (side, a) in [(-1.0f32, swing), (1.0, -swing)] {
                    let x0 = if side < 0.0 { -0.25 } else { 0.0 };
                    let lp = Vec3::new(x0 + 0.125, 0.68, 0.0);
                    add_char_box(&mut v, &f, Vec3::new(x0 + 0.01, 0.12, -0.12), Vec3::new(x0 + 0.24, 0.68, 0.12), lp, a, pants);
                    add_char_box(&mut v, &f, Vec3::new(x0, 0.0, -0.13), Vec3::new(x0 + 0.25, 0.14, 0.16), lp, a, skin_d);
                }
                let lean = 0.12;
                let tp = Vec3::new(0.0, 0.68, 0.0);
                add_char_box(&mut v, &f, Vec3::new(-0.25, 0.68, -0.13), Vec3::new(0.25, 1.3, 0.13), tp, lean, shirt);
                add_char_box(&mut v, &f, Vec3::new(-0.26, 0.68, -0.14), Vec3::new(0.26, 0.8, 0.14), tp, lean, pants);
                // Torn patches.
                add_char_box(&mut v, &f, Vec3::new(-0.12, 0.9, 0.13), Vec3::new(0.04, 1.02, 0.145), tp, lean, skin);
                for x0 in [-0.48f32, 0.25] {
                    let sp = Vec3::new(x0 + 0.115, 1.26, 0.0);
                    let reach = 1.35 + swing * 0.2;
                    add_char_box(&mut v, &f, Vec3::new(x0, 0.74, -0.11), Vec3::new(x0 + 0.23, 1.3, 0.11), sp, reach, skin);
                    add_char_box(&mut v, &f, Vec3::new(x0 - 0.005, 1.06, -0.12), Vec3::new(x0 + 0.235, 1.31, 0.12), sp, reach, shirt);
                }
                let hp = Vec3::new(0.0, 1.3, 0.04);
                let ha = lean + (m.anim * 0.5).sin() * 0.08;
                add_char_box(&mut v, &f, Vec3::new(-0.27, 1.3, -0.24), Vec3::new(0.27, 1.82, 0.3), hp, ha, skin);
                add_char_box(&mut v, &f, Vec3::new(-0.28, 1.7, -0.25), Vec3::new(0.28, 1.86, 0.31), hp, ha, skin_d);
                for ex in [-0.19f32, 0.05] {
                    add_char_box(&mut v, &f, Vec3::new(ex, 1.52, 0.3), Vec3::new(ex + 0.14, 1.64, 0.312), hp, ha, [0.15, 0.12, 0.10]);
                    add_char_box(&mut v, &f, Vec3::new(ex + 0.03, 1.55, 0.31), Vec3::new(ex + 0.11, 1.62, 0.32), hp, ha, glow);
                }
                add_char_box(&mut v, &f, Vec3::new(-0.12, 1.38, 0.3), Vec3::new(0.12, 1.44, 0.312), hp, ha, [0.20, 0.08, 0.08]);
                add_char_box(&mut v, &f, Vec3::new(-0.06, 1.42, 0.31), Vec3::new(-0.02, 1.45, 0.316), hp, ha, [0.95, 0.95, 0.85]);
            }
            MobKind::Crawler => {
                // Spider: a big round abdomen behind a smaller head, eight jointed legs that go
                // up to the knee and down to the ground on both sides, eyes and fangs in front.
                let body = tint([0.30, 0.20, 0.38]);
                let back = tint([0.44, 0.28, 0.54]);
                let leg = tint([0.20, 0.13, 0.25]);
                let red = [1.0, 0.25, 0.22];
                let bob = (m.anim * 2.0).sin().abs() * 0.03 * m.amount;
                add_char_box(&mut v, &f, Vec3::new(-0.34, 0.3 + bob, -0.72), Vec3::new(0.34, 0.82 + bob, -0.06), z, 0.0, body);
                add_char_box(&mut v, &f, Vec3::new(-0.2, 0.78 + bob, -0.6), Vec3::new(0.2, 0.86 + bob, -0.2), z, 0.0, back);
                add_char_box(&mut v, &f, Vec3::new(-0.24, 0.3 + bob, -0.1), Vec3::new(0.24, 0.66 + bob, 0.36), z, 0.0, body);
                // Legs: hip on the body's side, knee up and out, foot on the ground.
                for (i, zz) in [-0.42f32, -0.24, -0.04, 0.16].iter().enumerate() {
                    let phase = if i % 2 == 0 { swing } else { -swing } * 0.18;
                    for side in [-1.0f32, 1.0] {
                        let step = phase * side;
                        let hip = Vec3::new(side * 0.22, 0.52 + bob, *zz);
                        let knee = Vec3::new(side * 0.62, 0.8 + bob, zz + step * 0.5);
                        let foot = Vec3::new(side * 0.88, 0.0, zz * 1.3 + step);
                        add_rod(&mut v, &f, hip, knee, 0.075, z, 0.0, leg);
                        add_rod(&mut v, &f, knee, foot, 0.06, z, 0.0, leg);
                    }
                }
                // Eyes: two big ones with a glint, four small ones above; fangs below.
                for ex in [-0.15f32, 0.05] {
                    add_char_box(&mut v, &f, Vec3::new(ex, 0.46 + bob, 0.36), Vec3::new(ex + 0.1, 0.56 + bob, 0.372), z, 0.0, red);
                    add_char_box(&mut v, &f, Vec3::new(ex + 0.06, 0.52 + bob, 0.371), Vec3::new(ex + 0.085, 0.545 + bob, 0.376), z, 0.0, [1.0, 1.0, 1.0]);
                }
                for (ex, ey) in [(-0.17f32, 0.59f32), (-0.07, 0.61), (0.03, 0.61), (0.13, 0.59)] {
                    add_char_box(&mut v, &f, Vec3::new(ex, ey + bob, 0.36), Vec3::new(ex + 0.05, ey + 0.04 + bob, 0.37), z, 0.0, red);
                }
                for fx in [-0.08f32, 0.04] {
                    add_char_box(&mut v, &f, Vec3::new(fx, 0.3 + bob, 0.3), Vec3::new(fx + 0.04, 0.4 + bob, 0.38), z, 0.0, [0.92, 0.90, 0.82]);
                }
            }
            MobKind::Cow => {
                // Black-and-white cow: big body with patches, horns, pink muzzle and udder.
                let white = tint([0.96, 0.95, 0.91]);
                let black = tint([0.17, 0.15, 0.15]);
                let pink = tint([0.95, 0.70, 0.72]);
                let horn = tint([0.92, 0.88, 0.76]);
                let bob = (m.anim * 2.0).sin().abs() * 0.03 * m.amount;
                let h = 0.52;
                for (x0, z0, a) in [(-0.34f32, -0.5f32, swing), (0.12, -0.5, -swing), (-0.34, 0.3, -swing), (0.12, 0.3, swing)] {
                    add_box(&mut v, &f, Vec3::new(x0, 0.0, z0), Vec3::new(x0 + 0.22, h + 0.05, z0 + 0.22), Vec3::new(x0 + 0.11, h, z0 + 0.11), a, white);
                    add_box(&mut v, &f, Vec3::new(x0 - 0.005, 0.0, z0 - 0.005), Vec3::new(x0 + 0.225, 0.1, z0 + 0.225), Vec3::new(x0 + 0.11, h, z0 + 0.11), a, black);
                }
                add_box(&mut v, &f, Vec3::new(-0.4, h + bob, -0.62), Vec3::new(0.4, h + 0.62 + bob, 0.56), z, 0.0, white);
                // Patches on the sides and back.
                for (x0, y0, z0, x1, y1, z1) in [(-0.41f32, 0.2f32, -0.3f32, -0.39f32, 0.5f32, 0.1f32), (0.39, 0.1, 0.05, 0.41, 0.45, 0.4), (-0.2, 0.61, -0.4, 0.15, 0.63, -0.1), (-0.41, 0.05, 0.25, -0.39, 0.3, 0.5)] {
                    add_box(&mut v, &f, Vec3::new(x0, h + y0 + bob, z0), Vec3::new(x1, h + y1 + bob, z1), z, 0.0, black);
                }
                add_box(&mut v, &f, Vec3::new(-0.12, h - 0.06 + bob, -0.3), Vec3::new(0.12, h + bob, -0.05), z, 0.0, pink);
                let hz = 0.56;
                let hy = h + 0.3 + bob;
                add_box(&mut v, &f, Vec3::new(-0.24, hy, hz), Vec3::new(0.24, hy + 0.42, hz + 0.3), z, 0.0, white);
                add_box(&mut v, &f, Vec3::new(-0.2, hy - 0.02, hz + 0.28), Vec3::new(0.2, hy + 0.16, hz + 0.38), z, 0.0, pink);
                for nx in [-0.12f32, 0.06] {
                    add_box(&mut v, &f, Vec3::new(nx, hy + 0.06, hz + 0.38), Vec3::new(nx + 0.06, hy + 0.11, hz + 0.385), z, 0.0, [0.45, 0.25, 0.26]);
                }
                for ex in [-0.17f32, 0.07] {
                    add_box(&mut v, &f, Vec3::new(ex, hy + 0.24, hz + 0.3), Vec3::new(ex + 0.1, hy + 0.33, hz + 0.312), z, 0.0, [0.08, 0.07, 0.09]);
                    add_box(&mut v, &f, Vec3::new(ex + 0.06, hy + 0.29, hz + 0.313), Vec3::new(ex + 0.09, hy + 0.32, hz + 0.316), z, 0.0, [1.0, 1.0, 1.0]);
                }
                for sx in [-0.34f32, 0.24] {
                    add_box(&mut v, &f, Vec3::new(sx, hy + 0.36, hz + 0.06), Vec3::new(sx + 0.1, hy + 0.44, hz + 0.14), z, 0.0, horn);
                }
                add_box(&mut v, &f, Vec3::new(-0.26, hy + 0.3, hz + 0.02), Vec3::new(-0.2, hy + 0.36, hz + 0.1), z, 0.0, black);
            }
            MobKind::Chicken => {
                // Little white chicken: round body, red comb and wattle, yellow beak, orange legs.
                let white = tint([0.98, 0.97, 0.94]);
                let red = tint([0.95, 0.20, 0.18]);
                let beak = tint([1.0, 0.75, 0.20]);
                let leg = tint([0.98, 0.58, 0.18]);
                let bob = (m.anim * 2.5).sin().abs() * 0.04 * m.amount;
                for (x0, a) in [(-0.1f32, swing), (0.04, -swing)] {
                    add_box(&mut v, &f, Vec3::new(x0, 0.0, -0.03), Vec3::new(x0 + 0.06, 0.2, 0.03), Vec3::new(x0 + 0.03, 0.2, 0.0), a, leg);
                    add_box(&mut v, &f, Vec3::new(x0 - 0.02, 0.0, -0.02), Vec3::new(x0 + 0.08, 0.02, 0.08), Vec3::new(x0 + 0.03, 0.2, 0.0), a, leg);
                }
                add_box(&mut v, &f, Vec3::new(-0.16, 0.2 + bob, -0.2), Vec3::new(0.16, 0.48 + bob, 0.16), z, 0.0, white);
                // Wings flap while walking.
                let flap = (m.anim * 3.0).sin() * 0.3 * m.amount;
                for sx in [-0.2f32, 0.16] {
                    add_box(&mut v, &f, Vec3::new(sx, 0.26 + bob, -0.14), Vec3::new(sx + 0.04, 0.44 + bob, 0.1), Vec3::new(sx + 0.02, 0.44 + bob, 0.0), flap, white);
                }
                add_box(&mut v, &f, Vec3::new(-0.1, 0.4 + bob, -0.28), Vec3::new(0.1, 0.52 + bob, -0.2), z, 0.0, white);
                let hy = 0.44 + bob;
                add_box(&mut v, &f, Vec3::new(-0.11, hy, 0.06), Vec3::new(0.11, hy + 0.26, 0.24), z, 0.0, white);
                add_box(&mut v, &f, Vec3::new(-0.05, hy + 0.08, 0.24), Vec3::new(0.05, hy + 0.14, 0.33), z, 0.0, beak);
                add_box(&mut v, &f, Vec3::new(-0.03, hy + 0.0, 0.23), Vec3::new(0.03, hy + 0.08, 0.27), z, 0.0, red);
                add_box(&mut v, &f, Vec3::new(-0.03, hy + 0.26, 0.1), Vec3::new(0.03, hy + 0.34, 0.22), z, 0.0, red);
                for ex in [-0.1f32, 0.07] {
                    add_box(&mut v, &f, Vec3::new(ex, hy + 0.15, 0.2), Vec3::new(ex + 0.03, hy + 0.19, 0.245), z, 0.0, [0.08, 0.07, 0.09]);
                }
            }
            MobKind::Wolf => {
                // Grey wolf: lean body, pointy ears, long snout, bushy tail, fierce yellow eyes.
                let fur = tint([0.55, 0.56, 0.60]);
                let dark = tint([0.38, 0.39, 0.43]);
                let light = tint([0.85, 0.85, 0.86]);
                let eye = [1.0, 0.82, 0.25];
                let h = 0.36;
                for (x0, z0, a) in [(-0.21f32, -0.36f32, swing), (0.04, -0.36, -swing), (-0.21, 0.2, -swing), (0.04, 0.2, swing)] {
                    add_box(&mut v, &f, Vec3::new(x0, 0.0, z0), Vec3::new(x0 + 0.17, h + 0.04, z0 + 0.17), Vec3::new(x0 + 0.085, h, z0 + 0.085), a, fur);
                    add_box(&mut v, &f, Vec3::new(x0 - 0.005, 0.0, z0 - 0.005), Vec3::new(x0 + 0.175, 0.07, z0 + 0.19), Vec3::new(x0 + 0.085, h, z0 + 0.085), a, dark);
                }
                add_box(&mut v, &f, Vec3::new(-0.22, h, -0.42), Vec3::new(0.22, h + 0.36, 0.36), z, 0.0, fur);
                add_box(&mut v, &f, Vec3::new(-0.25, h + 0.08, 0.12), Vec3::new(0.25, h + 0.44, 0.4), z, 0.0, dark);
                add_box(&mut v, &f, Vec3::new(-0.15, h - 0.01, -0.3), Vec3::new(0.15, h + 0.05, 0.3), z, 0.0, light);
                // Tail up and swishing.
                let wag = (m.anim * 1.5).sin() * 0.2;
                add_box(&mut v, &f, Vec3::new(-0.07, h + 0.15, -0.72), Vec3::new(0.07, h + 0.29, -0.42), Vec3::new(0.0, h + 0.22, -0.42), -0.6 + wag, dark);
                let hz = 0.38;
                let hy = h + 0.16;
                add_box(&mut v, &f, Vec3::new(-0.17, hy, hz), Vec3::new(0.17, hy + 0.3, hz + 0.24), z, 0.0, fur);
                add_box(&mut v, &f, Vec3::new(-0.09, hy, hz + 0.24), Vec3::new(0.09, hy + 0.14, hz + 0.42), z, 0.0, light);
                add_box(&mut v, &f, Vec3::new(-0.04, hy + 0.1, hz + 0.42), Vec3::new(0.04, hy + 0.15, hz + 0.44), z, 0.0, [0.1, 0.09, 0.1]);
                for sx in [-0.16f32, 0.07] {
                    add_box(&mut v, &f, Vec3::new(sx, hy + 0.3, hz + 0.06), Vec3::new(sx + 0.09, hy + 0.44, hz + 0.12), z, 0.0, dark);
                }
                for ex in [-0.13f32, 0.06] {
                    add_box(&mut v, &f, Vec3::new(ex, hy + 0.18, hz + 0.24), Vec3::new(ex + 0.07, hy + 0.23, hz + 0.252), z, 0.0, eye);
                }
            }
            MobKind::Fish => {
                // A little fish: body, tail fin that wags, a back fin, eyes.
                let body = tint([0.95, 0.55, 0.20]);
                let belly = tint([0.98, 0.92, 0.85]);
                let fin = tint([0.30, 0.55, 0.85]);
                let wag = (m.anim * 6.0).sin() * 0.5;
                let y = 0.15;
                add_box(&mut v, &f, Vec3::new(-0.08, y, -0.18), Vec3::new(0.08, y + 0.2, 0.2), z, 0.0, body);
                add_box(&mut v, &f, Vec3::new(-0.07, y - 0.01, -0.12), Vec3::new(0.07, y + 0.05, 0.16), z, 0.0, belly);
                add_box(&mut v, &f, Vec3::new(-0.01, y + 0.2, -0.08), Vec3::new(0.01, y + 0.28, 0.08), z, 0.0, fin);
                // Tail: a thin plate, swung side to side by moving along x.
                let tx = wag * 0.08;
                add_box(&mut v, &f, Vec3::new(-0.015 + tx, y - 0.02, -0.34), Vec3::new(0.015 + tx, y + 0.22, -0.18), z, 0.0, fin);
                for ex in [-0.085f32, 0.075] {
                    add_box(&mut v, &f, Vec3::new(ex, y + 0.11, 0.12), Vec3::new(ex + 0.01, y + 0.15, 0.16), z, 0.0, [0.05, 0.05, 0.08]);
                }
            }
            MobKind::Squid => {
                // Squid: a tall mantle and eight tentacles that open and close as it swims.
                let skin = tint([0.28, 0.36, 0.55]);
                let dark = tint([0.20, 0.25, 0.42]);
                let pulse = (m.anim * 2.0).sin() * 0.5 + 0.5;
                add_box(&mut v, &f, Vec3::new(-0.3, 0.45, -0.3), Vec3::new(0.3, 1.15, 0.3), z, 0.0, skin);
                for ex in [-0.2f32, 0.12] {
                    add_box(&mut v, &f, Vec3::new(ex, 0.62, 0.3), Vec3::new(ex + 0.08, 0.72, 0.31), z, 0.0, [0.95, 0.95, 0.9]);
                    add_box(&mut v, &f, Vec3::new(ex + 0.02, 0.63, 0.305), Vec3::new(ex + 0.06, 0.69, 0.315), z, 0.0, [0.05, 0.05, 0.08]);
                }
                for k in 0..8 {
                    let a = k as f32 * std::f32::consts::TAU / 8.0;
                    let (s, c) = a.sin_cos();
                    let spread = 0.18 + pulse * 0.22;
                    let top = Vec3::new(c * 0.2, 0.45, s * 0.2);
                    let tip = Vec3::new(c * spread, 0.0, s * spread);
                    add_rod(&mut v, &f, top, tip, 0.045, z, 0.0, dark);
                }
            }
            MobKind::Dolphin | MobKind::Shark => {
                // Long streamlined body, a fin on the back, a tail fin that beats up and down.
                let shark = m.kind == MobKind::Shark;
                let back = tint(if shark { [0.45, 0.50, 0.56] } else { [0.55, 0.65, 0.75] });
                let belly = tint(if shark { [0.88, 0.88, 0.90] } else { [0.85, 0.88, 0.92] });
                let len = if shark { 1.2 } else { 0.95 };
                let w = if shark { 0.28 } else { 0.22 };
                let y = 0.2;
                let beat = (m.anim * 2.5).sin() * 0.35;
                add_box(&mut v, &f, Vec3::new(-w, y, -len * 0.55), Vec3::new(w, y + w * 2.0, len * 0.5), z, 0.0, back);
                add_box(&mut v, &f, Vec3::new(-w + 0.02, y - 0.02, -len * 0.45), Vec3::new(w - 0.02, y + w * 0.6, len * 0.45), z, 0.0, belly);
                // Head / snout.
                if shark {
                    add_box(&mut v, &f, Vec3::new(-w * 0.8, y + 0.04, len * 0.5), Vec3::new(w * 0.8, y + w * 1.6, len * 0.8), z, 0.0, back);
                    // Teeth.
                    for tx in [-0.12f32, -0.04, 0.04, 0.12] {
                        add_box(&mut v, &f, Vec3::new(tx - 0.02, y + 0.02, len * 0.74), Vec3::new(tx + 0.02, y + 0.08, len * 0.76), z, 0.0, [0.98, 0.98, 0.95]);
                    }
                } else {
                    add_box(&mut v, &f, Vec3::new(-w * 0.8, y + 0.06, len * 0.5), Vec3::new(w * 0.8, y + w * 1.7, len * 0.62), z, 0.0, back);
                    add_box(&mut v, &f, Vec3::new(-0.06, y + 0.08, len * 0.62), Vec3::new(0.06, y + 0.16, len * 0.8), z, 0.0, belly);
                }
                for ex in [-w * 0.8 - 0.005, w * 0.8 - 0.035] {
                    add_box(&mut v, &f, Vec3::new(ex, y + w * 1.1, len * 0.55), Vec3::new(ex + 0.04, y + w * 1.35, len * 0.6), z, 0.0, [0.05, 0.05, 0.08]);
                }
                // Back fin (tall for the shark).
                let fh = if shark { 0.5 } else { 0.25 };
                add_box(&mut v, &f, Vec3::new(-0.03, y + w * 2.0, -0.1), Vec3::new(0.03, y + w * 2.0 + fh, 0.15), Vec3::new(0.0, y + w * 2.0, 0.15), -0.3, back);
                // Side fins.
                for sx in [-1.0f32, 1.0] {
                    let (x0, x1) = if sx < 0.0 { (-w - 0.24, -w) } else { (w, w + 0.24) };
                    add_box(&mut v, &f, Vec3::new(x0, y + 0.05, 0.05), Vec3::new(x1, y + 0.09, 0.3), z, 0.0, back);
                }
                // Tail: beats about the body's side axis.
                let tp = Vec3::new(0.0, y + w, -len * 0.55);
                add_box(&mut v, &f, Vec3::new(-w * 0.5, y + w * 0.6, -len * 0.95), Vec3::new(w * 0.5, y + w * 1.4, -len * 0.55), tp, beat, back);
                add_box(&mut v, &f, Vec3::new(-w * 1.4, y + w * 0.85, -len * 1.1), Vec3::new(w * 1.4, y + w * 1.15, -len * 0.9), tp, beat * 1.4, back);
            }
            MobKind::Dragon => {
                // Green dragon: long body, neck and horned head in front, a tapering tail
                // behind, four short legs tucked up in flight and two big wings that flap.
                let scale = tint([0.16, 0.55, 0.38]);
                let belly = tint([0.85, 0.75, 0.40]);
                let dark = tint([0.10, 0.30, 0.22]);
                let membrane = tint([0.30, 0.68, 0.48]);
                let horn = [0.92, 0.88, 0.76];
                let bob = (m.anim * 0.5).sin() * 0.08;
                let h = 1.0 + bob;
                add_box(&mut v, &f, Vec3::new(-0.5, h, -0.9), Vec3::new(0.5, h + 0.8, 0.9), z, 0.0, scale);
                add_box(&mut v, &f, Vec3::new(-0.4, h - 0.04, -0.7), Vec3::new(0.4, h + 0.1, 0.8), z, 0.0, belly);
                // Spikes along the back.
                for i in 0..6 {
                    let zz = -0.8 + i as f32 * 0.32;
                    add_box(&mut v, &f, Vec3::new(-0.06, h + 0.8, zz), Vec3::new(0.06, h + 1.0, zz + 0.14), z, 0.0, dark);
                }
                // Saddle when worn.
                if m.saddled {
                    add_box(&mut v, &f, Vec3::new(-0.52, h + 0.78, -0.2), Vec3::new(0.52, h + 0.9, 0.4), z, 0.0, [0.55, 0.32, 0.16]);
                    add_box(&mut v, &f, Vec3::new(-0.2, h + 0.9, 0.3), Vec3::new(0.2, h + 1.05, 0.42), z, 0.0, [0.36, 0.20, 0.10]);
                }
                // Neck and head.
                add_box(&mut v, &f, Vec3::new(-0.24, h + 0.4, 0.8), Vec3::new(0.24, h + 1.2, 1.3), Vec3::new(0.0, h + 0.5, 0.9), -0.5, scale);
                let hy = h + 1.1;
                let hz = 1.45;
                add_box(&mut v, &f, Vec3::new(-0.3, hy, hz - 0.2), Vec3::new(0.3, hy + 0.45, hz + 0.3), z, 0.0, scale);
                add_box(&mut v, &f, Vec3::new(-0.2, hy, hz + 0.3), Vec3::new(0.2, hy + 0.25, hz + 0.75), z, 0.0, scale);
                add_box(&mut v, &f, Vec3::new(-0.18, hy - 0.08, hz + 0.3), Vec3::new(0.18, hy + 0.02, hz + 0.7), z, 0.0, belly);
                for nx in [-0.14f32, 0.08] {
                    add_box(&mut v, &f, Vec3::new(nx, hy + 0.18, hz + 0.75), Vec3::new(nx + 0.06, hy + 0.23, hz + 0.76), z, 0.0, [0.05, 0.05, 0.05]);
                }
                for ex in [-0.31f32, 0.25] {
                    add_box(&mut v, &f, Vec3::new(ex, hy + 0.24, hz + 0.05), Vec3::new(ex + 0.06, hy + 0.34, hz + 0.18), z, 0.0, [1.0, 0.80, 0.15]);
                }
                for sx in [-0.24f32, 0.14] {
                    add_box(&mut v, &f, Vec3::new(sx, hy + 0.4, hz - 0.3), Vec3::new(sx + 0.1, hy + 0.5, hz + 0.0), Vec3::new(sx, hy + 0.45, hz), 0.5, horn);
                }
                // Tail: segments that get thinner and sway.
                let sway = (m.anim * 0.7).sin() * 0.15;
                for i in 0..5 {
                    let k = i as f32;
                    let w = 0.3 - k * 0.05;
                    let zz = -0.9 - k * 0.45;
                    let dx = sway * k * k * 0.15;
                    add_box(&mut v, &f, Vec3::new(-w + dx, h + 0.3 - k * 0.05, zz - 0.45), Vec3::new(w + dx, h + 0.3 + w * 1.4 - k * 0.05, zz), z, 0.0, if i % 2 == 0 { scale } else { dark });
                }
                // Legs, tucked under in flight.
                let tuck = if m.body.flying { 0.9 } else { swing };
                for (x0, z0) in [(-0.5f32, -0.6f32), (0.32, -0.6), (-0.5, 0.5), (0.32, 0.5)] {
                    add_box(&mut v, &f, Vec3::new(x0, h - 0.7, z0), Vec3::new(x0 + 0.18, h + 0.1, z0 + 0.22), Vec3::new(x0 + 0.09, h, z0 + 0.11), tuck, scale);
                    add_box(&mut v, &f, Vec3::new(x0 - 0.02, h - 0.76, z0 - 0.02), Vec3::new(x0 + 0.2, h - 0.66, z0 + 0.3), Vec3::new(x0 + 0.09, h, z0 + 0.11), tuck, dark);
                }
                // Wings: each in its own frame rolled about the body's length, so they flap.
                let flap = (m.anim * 1.2).sin() * 0.7;
                for side in [-1.0f64, 1.0] {
                    let a = flap as f64;
                    let (s, c) = a.sin_cos();
                    let r = f.right * c + f.up * s * side;
                    let u = f.up * c - f.right * s * side;
                    let wf = Frame { origin: f.to_world(Vec3::new(side as f32 * 0.45, h + 0.7, 0.2)), right: r, up: u, fwd: f.fwd };
                    let sx = side as f32;
                    let (x0, x1) = if side < 0.0 { (-2.6, 0.0) } else { (0.0, 2.6) };
                    add_box(&mut v, &wf, Vec3::new(x0, -0.03, -0.9), Vec3::new(x1, 0.03, 0.5), z, 0.0, membrane);
                    add_box(&mut v, &wf, Vec3::new(if side < 0.0 { -2.7 } else { 0.0 }, -0.06, 0.4), Vec3::new(if side < 0.0 { 0.0 } else { 2.7 }, 0.08, 0.58), z, 0.0, dark);
                    for k in 1..4 {
                        let xx = sx * k as f32 * 0.75;
                        add_box(&mut v, &wf, Vec3::new(xx - 0.04, -0.05, -0.9 + k as f32 * 0.15), Vec3::new(xx + 0.04, 0.05, 0.5), z, 0.0, dark);
                    }
                }
            }
            MobKind::Slime => {
                // Jelly cube: a see-through-looking green shell with a darker core and big
                // cartoon eyes; squashes while it is on the ground, stretches in the air.
                let shell = tint([0.52, 0.80, 0.44]);
                let core = tint([0.34, 0.62, 0.30]);
                let squash = if m.body.on_ground { 0.85 } else { 1.12 };
                let s = 0.42f32;
                let hgt = 2.0 * s * squash;
                let wide = s / squash.sqrt();
                add_box(&mut v, &f, Vec3::new(-wide, 0.0, -wide), Vec3::new(wide, hgt, wide), z, 0.0, shell);
                add_box(&mut v, &f, Vec3::new(-wide * 0.5, hgt * 0.2, -wide * 0.5), Vec3::new(wide * 0.5, hgt * 0.62, wide * 0.5 + 0.01), z, 0.0, core);
                let fz = wide + 0.005;
                for ex in [-0.22f32, 0.07] {
                    add_box(&mut v, &f, Vec3::new(ex, hgt * 0.55, fz), Vec3::new(ex + 0.15, hgt * 0.55 + 0.16, fz + 0.01), z, 0.0, [0.98, 0.98, 0.98]);
                    add_box(&mut v, &f, Vec3::new(ex + 0.05, hgt * 0.55 + 0.02, fz + 0.008), Vec3::new(ex + 0.12, hgt * 0.55 + 0.11, fz + 0.016), z, 0.0, [0.08, 0.10, 0.08]);
                }
                add_box(&mut v, &f, Vec3::new(-0.1, hgt * 0.38, fz), Vec3::new(0.1, hgt * 0.38 + 0.04, fz + 0.01), z, 0.0, [0.15, 0.40, 0.15]);
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
                let depth = match info.icon {
                    crate::items::Icon::Item(i) => icon_depth(i as usize),
                    crate::items::Icon::Block(_) => 1.0,
                };
                if let Some(px) = px.filter(|p| p.len() == 256) {
                    let s = 0.34f32;
                    if d.resting {
                        // Lying flat on the ground, one pixel thick.
                        let (u, w) = (Vec3::X, Vec3::NEG_Z);
                        add_voxel_icon(&mut v, &frame, &px, Vec3::new(-s * 0.5, s / 16.0 * depth + 0.004, s * 0.5), u, w, u.cross(w), s, depth, Vec3::ZERO, 0.0);
                    } else {
                        add_voxel_icon(&mut v, &frame, &px, Vec3::new(-s * 0.5, 0.0, 0.0), Vec3::X, Vec3::Y, Vec3::NEG_Z, s, depth, Vec3::ZERO, 0.0);
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
    /// The held item is enchanted: it shimmers red.
    pub glint: bool,
    /// Colour of the potion on the tip of the arrow on the bow string (tipped arrows).
    pub arrow_tip: Option<[f32; 3]>,
    /// Grenade launcher or bazooka in the hands: a full 3D weapon held with both hands.
    pub gun: Option<GunPose>,
    /// Eating (1 just started → 0 done): the food goes up to the mouth and bobs with bites.
    pub eat: f32,
    /// Putting armour on (1 → 0): the hand sweeps down to the chest and back.
    pub equip: f32,
}

/// State of a held grenade launcher (`kind` 0) or bazooka (`kind` 1) for the view model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunPose {
    pub kind: u8,
    /// 1 right after a shot, fading to 0: kick back and up, muzzle flash.
    pub recoil: f32,
    /// Reload progress 0..1 (0 = ready): the weapon dips and comes back.
    pub reload: f32,
    /// A round sits in the muzzle (rocket tip / grenade visible). Electric tools: charged.
    pub loaded: bool,
    /// Electric tools: how hard the motor runs, 0 idle → 1 full (spins up while digging,
    /// winds down after).
    pub work: f32,
}

impl HandsPose {
    /// Resting pose looking along `forward` (handy for tests and previews).
    pub fn rest(forward: Vec3, up: Vec3) -> Self {
        HandsPose { forward, up, punch_cycle: 0.0, guard: 1.0, bob_phase: 0.0, bob_amount: 0.0, holding: false, tool: None, icon: None, shield: false, blocking: 0.0, place: 0.0, skin: 0, sprint: 0.0, sneak: 0.0, lift: 0.0, sway: [0.0; 2], time: 0.0, sleeve: None, bow: None, glint: false, arrow_tip: None, gun: None, eat: 0.0, equip: 0.0 }
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
            p = p.lerp(Vec3::new(0.14, -0.22, 0.44) + off * 0.4, e);
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
        // Eating, like Minecraft: the food comes in front of the mouth and bobs with every
        // bite, then the hand goes back.
        if pose.eat > 0.0 {
            let e = pose.eat.clamp(0.0, 1.0);
            let w = (e * std::f32::consts::PI).sin().powf(0.4);
            let bite = (e * 30.0).sin() * 0.025 * w;
            p = p.lerp(Vec3::new(0.06, -0.20 + bite, 0.30), w);
            a.tilt -= 0.55 * w;
            a.yaw += 0.35 * w;
        }
        // Putting armour on: a quick sweep down to the chest and back.
        if pose.equip > 0.0 {
            let e = pose.equip.clamp(0.0, 1.0);
            let w = (e * std::f32::consts::PI).sin();
            p += Vec3::new(-0.12 * w, -0.22 * w, -0.06 * w);
            a.tilt += 0.6 * w;
            a.roll -= 0.4 * w;
        }
        a.pos = p;
        return a;
    }
    // Left arm: below the screen unless it carries a shield.
    if pose.shield {
        let b = pose.blocking.clamp(0.0, 1.0);
        // Raised shield stays left of the crosshair so the target remains visible.
        let x = lerp(-0.38, -0.22, b);
        let y = lerp(-0.42, -0.24, b);
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
pub fn build_held_construction(pose: &HandsPose, block: crate::block::Block) -> Vec<crate::mesher::Vertex> {
    let Some((entry, rotation)) = crate::construction::entry(block as u16) else { return Vec::new() };
    let model = crate::construction::geometry::model(entry.form, rotation, 0);
    let frame=camera_frame(pose);let arm=arm_place(pose,1.0);
    let fist=tilt_point(arm.pos+Vec3::new(0.,0.,0.29),arm.pos,arm.tilt);
    let center=fist+Vec3::new(0.05,0.06,0.06);
    let (ys,yc)=0.72f32.sin_cos();let (ps,pc)=(-0.32f32).sin_cos();
    let point=|p:[f32;3]| {
        let v=Vec3::new(p[0]-0.5,p[2]-0.5,p[1]-0.5)*0.30;
        let v=Vec3::new(v.x*yc+v.z*ys,v.y,-v.x*ys+v.z*yc);
        let v=center+Vec3::new(v.x,v.y*pc-v.z*ps,v.y*ps+v.z*pc);
        frame.to_world(arm.pos+twist_dir(v-arm.pos,arm.yaw,arm.roll,1.)).as_vec3().to_array()
    };
    let mut out=Vec::with_capacity(model.faces.len()*12);
    for face in &model.faces {
        let (tile,alpha,emissive)=crate::construction::surface(entry,face.surface);
        let verts:Vec<_>=face.points.iter().zip([[0.,0.],[1.,0.],[1.,1.],[0.,1.]]).map(|(p,uv)|crate::mesher::Vertex {
            pos:point(*p),uv,flags:tile as u32|((emissive as u32)<<16)|(2<<17)|((alpha as u32)<<27),light:[1.,1.]
        }).collect();
        for i in [0,1,2,0,2,3,0,2,1,0,3,2] {out.push(verts[i]);}
    }
    out
}

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
/// Vertices: right arm first (`ARM` when empty-handed), then the left arm (`ARM`).
/// Vertices of one bare arm: sleeve, cuff and hand, 36 each (sharp boxes).
/// Sleeve, cuff, hand, four two-joint fingers and a two-joint thumb: 13 boxes.
pub const ARM: usize = 13 * 36;

pub fn build_first_person_hands(pose: &HandsPose) -> Vec<ModelVertex> {
    if let Some(g) = pose.gun {
        return build_gun_view(pose, g);
    }
    let frame = camera_frame(pose);
    let (skin_c, _, _, sleeve_c, _) = skin_colors(pose.skin);
    let sleeve_c = pose.sleeve.unwrap_or(sleeve_c);

    let mut v = Vec::with_capacity(2 * ARM);
    for side in [1.0f32, -1.0] {
        let a = arm_place(pose, side);
        let start = v.len();
        let (pivot, angle) = (a.pos, a.tilt);
        let (x, y, z) = (pivot.x, pivot.y, pivot.z);
        // Both arms are always built (the left one rests below the screen unless it carries a
        // shield); the right one grips the held item, like in Minecraft.
        {
            // Square arm like the third-person body: sleeve with a slightly wider hand.
            // Sleeve, a darker cuff and the hand: clean flat colours (no skin patches).
            let cuff = [sleeve_c[0] * 0.72, sleeve_c[1] * 0.72, sleeve_c[2] * 0.72];
            // Like Minecraft: an item in the right hand is shown on its own, without the arm
            // (bows keep the arm: it draws the string).
            let hide_arm = (side > 0.0 && pose.holding && pose.bow.is_none()) || (side < 0.0 && pose.shield);
            if !hide_arm {
                add_hand_box(&mut v, &frame, Vec3::new(x - 0.07, y - 0.07, z), Vec3::new(x + 0.07, y + 0.07, z + 0.15), pivot, angle, sleeve_c);
                add_hand_box(&mut v, &frame, Vec3::new(x - 0.074, y - 0.074, z + 0.14), Vec3::new(x + 0.074, y + 0.074, z + 0.18), pivot, angle, cuff);
                add_hand_box(&mut v, &frame, Vec3::new(x - 0.066, y - 0.066, z + 0.175), Vec3::new(x + 0.066, y + 0.066, z + 0.36), pivot, angle, skin_c);
                // Five fingers: four from the end of the palm, slightly curled (a second,
                // lower joint), the middle one longest; the thumb from the inner side.
                let shade = [skin_c[0] * 0.93, skin_c[1] * 0.93, skin_c[2] * 0.93];
                for (fxo, len) in [(-0.046f32, 0.055f32), (-0.0155, 0.07), (0.0155, 0.066), (0.046, 0.05)] {
                    let fxx = x + fxo * side;
                    add_hand_box(&mut v, &frame, Vec3::new(fxx - 0.0135, y - 0.02, z + 0.355), Vec3::new(fxx + 0.0135, y + 0.045, z + 0.355 + len), pivot, angle, skin_c);
                    add_hand_box(&mut v, &frame, Vec3::new(fxx - 0.0125, y - 0.045, z + 0.345 + len), Vec3::new(fxx + 0.0125, y + 0.01, z + 0.37 + len), pivot, angle, shade);
                }
                let tx = x - 0.078 * side;
                add_hand_box(&mut v, &frame, Vec3::new(tx - 0.016, y - 0.03, z + 0.24), Vec3::new(tx + 0.016, y + 0.03, z + 0.33), pivot, angle, skin_c);
                add_hand_box(&mut v, &frame, Vec3::new(tx - 0.015 - 0.012 * side, y - 0.028, z + 0.32), Vec3::new(tx + 0.015 - 0.012 * side, y + 0.028, z + 0.37), pivot, angle, shade);
            }
        }
        let f = Vec3::new(x, y, z + 0.29); // fist centre before the tilt
        let held_start = v.len();
        if side > 0.0 {
            if let Some(c) = pose.bow {
                add_bow(&mut v, &frame, f, pivot, angle, c.clamp(0.0, 1.0), pose.arrow_tip);
            } else if let Some((kind, mat)) = pose.tool {
                add_tool(&mut v, &frame, f, pivot, angle, kind, mat);
            } else if let Some(px) = pose.icon.and_then(|i| textures().and_then(|t| t.item_pixels.get(i as usize).cloned())).filter(|p| p.len() == 256) {
                // Stick, food, ingots…: a small 3D icon in the fist, facing the camera a bit.
                let a = 0.55f32;
                let u = Vec3::new(-a.cos(), 0.0, a.sin());
                let n = u.cross(Vec3::Y).normalize();
                add_voxel_icon(&mut v, &frame, &px, f + Vec3::new(0.04, -0.10, 0.0), u, Vec3::Y, n, 0.32, icon_depth(pose.icon.unwrap_or(0) as usize), pivot, angle);
            }
        } else if pose.shield {
            add_shield(&mut v, &frame, f, pivot, angle);
        }
        if side > 0.0 && pose.glint {
            // Enchanted: like the icon in the hotbar — the item keeps its own colours with a
            // faint red tint, and a narrow bright band sweeps along it now and then.
            for q in v[held_start..].iter_mut() {
                let p = Vec3::from(q.pos);
                let wave = 0.5 + 0.5 * (pose.time * 3.0 - (p.x + p.y + p.z) * 9.0).sin();
                let k = 0.06 + 0.30 * wave.powi(8);
                q.color = [q.color[0] + (1.0 - q.color[0]) * k, q.color[1] * (1.0 - k * 0.75), q.color[2] * (1.0 - k * 0.75)];
            }
        }
        twist_verts(&mut v[start..], &frame, &a, side);
    }
    v
}

/// Grenade launcher / bazooka in first person: a box-built 3D weapon held with both hands
/// (camera-local: x right, y up, z forward). The bazooka lies on the right shoulder, the
/// launcher is held at the chest. Recoil kicks it back and up with a muzzle flash; while
/// reloading it dips and tips down, then comes back with a fresh round in the muzzle.
fn build_gun_view(pose: &HandsPose, g: GunPose) -> Vec<ModelVertex> {
    if g.kind >= 2 {
        return build_firearm_view(pose, g);
    }
    const OLIVE: [f32; 3] = [0.34, 0.40, 0.22];
    const OLIVE_D: [f32; 3] = [0.26, 0.31, 0.17];
    const DARK: [f32; 3] = [0.15, 0.16, 0.15];
    const STEEL: [f32; 3] = [0.40, 0.42, 0.45];
    const WOOD: [f32; 3] = [0.52, 0.36, 0.20];
    const ROCKET: [f32; 3] = [0.80, 0.18, 0.14];
    const GRENADE: [f32; 3] = [0.32, 0.48, 0.24];
    const LENS: [f32; 3] = [0.45, 0.80, 1.0];
    let frame = camera_frame(pose);
    let (skin_c, _, _, sleeve_c, _) = skin_colors(pose.skin);
    let sleeve_c = pose.sleeve.unwrap_or(sleeve_c);
    let (off, btilt, _, _) = body_motion(pose);
    let r = g.recoil.clamp(0.0, 1.0);
    let kick = r * r;
    let rl = g.reload.clamp(0.0, 1.0);
    let dip = (rl * std::f32::consts::PI).sin();
    let bazooka = g.kind == 1;
    let anchor = if bazooka { Vec3::new(0.21, -0.17, 0.12) } else { Vec3::new(0.19, -0.21, 0.20) };
    let base = anchor + off + Vec3::new(0.0, -0.12 * dip + 0.015 * kick, -0.13 * kick);
    // + tips the muzzle down: reload dips it, recoil throws it up.
    let angle = 0.50 * dip - 0.30 * kick + btilt * 0.5;
    let pivot = base;
    let mut v: Vec<ModelVertex> = Vec::with_capacity(1200);
    let bx = |v: &mut Vec<ModelVertex>, a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
        add_hand_box(v, &frame, base + Vec3::from(a), base + Vec3::from(b), pivot, angle, c);
    };
    let muzzle_z;
    if bazooka {
        // Tube with two bands, a steel muzzle ring and a flared exhaust behind the shoulder.
        bx(&mut v, [-0.075, -0.075, -0.25], [0.075, 0.075, 1.02], OLIVE);
        bx(&mut v, [-0.081, -0.081, 0.24], [0.081, 0.081, 0.29], OLIVE_D);
        bx(&mut v, [-0.081, -0.081, 0.70], [0.081, 0.081, 0.75], OLIVE_D);
        bx(&mut v, [-0.092, -0.092, 0.98], [0.092, 0.092, 1.08], STEEL);
        bx(&mut v, [-0.10, -0.10, -0.34], [0.10, 0.10, -0.24], DARK);
        // Optical sight on the left side, its lens towards the target.
        bx(&mut v, [-0.17, -0.005, 0.30], [-0.075, 0.065, 0.46], DARK);
        bx(&mut v, [-0.155, 0.008, 0.46], [-0.09, 0.052, 0.47], LENS);
        bx(&mut v, [-0.10, -0.03, 0.34], [-0.075, 0.03, 0.42], DARK);
        // Rear pistol grip with trigger guard, front grip, shoulder rest.
        bx(&mut v, [-0.025, -0.23, 0.13], [0.025, -0.075, 0.20], DARK);
        bx(&mut v, [-0.012, -0.13, 0.20], [0.012, -0.075, 0.27], DARK);
        bx(&mut v, [-0.025, -0.20, 0.46], [0.025, -0.075, 0.52], DARK);
        bx(&mut v, [-0.06, -0.12, -0.10], [0.06, -0.075, 0.06], DARK);
        if g.loaded {
            // Rocket warhead sticking out of the muzzle.
            bx(&mut v, [-0.058, -0.058, 1.08], [0.058, 0.058, 1.15], ROCKET);
            bx(&mut v, [-0.036, -0.036, 1.15], [0.036, 0.036, 1.21], ROCKET);
            bx(&mut v, [-0.014, -0.014, 1.21], [0.014, 0.014, 1.24], [0.55, 0.12, 0.10]);
        }
        muzzle_z = 1.08;
        // Hands: right on the rear grip, left under the front grip.
        bx(&mut v, [-0.066, -0.26, 0.09], [0.066, -0.13, 0.24], skin_c);
        bx(&mut v, [-0.07, -0.55, -0.02], [0.07, -0.25, 0.16], sleeve_c);
        bx(&mut v, [-0.066, -0.24, 0.42], [0.066, -0.12, 0.56], skin_c);
        bx(&mut v, [-0.28, -0.55, 0.34], [-0.04, -0.22, 0.52], sleeve_c);
    } else {
        // Short heavy barrel, revolving drum, wooden stock, sights.
        bx(&mut v, [-0.058, -0.058, 0.08], [0.058, 0.058, 0.76], STEEL);
        bx(&mut v, [-0.074, -0.074, 0.70], [0.074, 0.074, 0.80], DARK);
        bx(&mut v, [-0.13, -0.14, 0.12], [0.13, 0.11, 0.37], OLIVE);
        bx(&mut v, [-0.136, -0.146, 0.16], [0.136, 0.116, 0.18], OLIVE_D);
        bx(&mut v, [-0.136, -0.146, 0.31], [0.136, 0.116, 0.33], OLIVE_D);
        bx(&mut v, [-0.045, -0.17, -0.30], [0.045, -0.02, 0.12], WOOD);
        bx(&mut v, [-0.05, -0.20, -0.33], [0.05, 0.0, -0.27], DARK);
        bx(&mut v, [-0.03, -0.28, 0.04], [0.03, -0.12, 0.11], DARK);
        bx(&mut v, [-0.03, -0.22, 0.48], [0.03, -0.058, 0.55], DARK);
        bx(&mut v, [-0.01, 0.058, 0.66], [0.01, 0.11, 0.70], DARK);
        bx(&mut v, [-0.035, 0.11, 0.06], [-0.015, 0.16, 0.09], DARK);
        bx(&mut v, [0.015, 0.11, 0.06], [0.035, 0.16, 0.09], DARK);
        if g.loaded {
            bx(&mut v, [-0.046, -0.046, 0.76], [0.046, 0.046, 0.81], GRENADE);
        }
        muzzle_z = 0.80;
        bx(&mut v, [-0.066, -0.31, 0.01], [0.066, -0.17, 0.15], skin_c);
        bx(&mut v, [-0.07, -0.60, -0.12], [0.07, -0.29, 0.08], sleeve_c);
        bx(&mut v, [-0.066, -0.25, 0.44], [0.066, -0.12, 0.58], skin_c);
        bx(&mut v, [-0.30, -0.58, 0.36], [-0.04, -0.23, 0.54], sleeve_c);
    }
    if kick > 0.55 {
        // Muzzle flash for the first moments after the shot: bright, unlit.
        let start = v.len();
        let k = (kick - 0.55) / 0.45;
        let z = muzzle_z;
        let f = 0.10 + 0.10 * k;
        bx(&mut v, [-f, -f, z], [f, f, z + 0.10 + 0.20 * k], [1.0, 0.62, 0.18]);
        bx(&mut v, [-f * 0.55, -f * 0.55, z], [f * 0.55, f * 0.55, z + 0.18 + 0.30 * k], [1.0, 0.95, 0.60]);
        for q in v[start..].iter_mut() {
            q.tex = 3.0;
        }
    }
    v
}

/// Firearms in first person, box-built like the bazooka: pistol (`kind` 2), assault rifle
/// (3), pump shotgun (4), sniper rifle with a scope (5). Recoil kicks the gun back and up
/// (the pistol's slide snaps back); after a shot the shotgun racks its pump and the sniper
/// rifle works its bolt.
fn build_firearm_view(pose: &HandsPose, g: GunPose) -> Vec<ModelVertex> {
    const DARK: [f32; 3] = [0.13, 0.14, 0.15];
    const GUN: [f32; 3] = [0.24, 0.25, 0.27];
    const STEEL: [f32; 3] = [0.42, 0.44, 0.47];
    const WOOD: [f32; 3] = [0.50, 0.33, 0.18];
    const WOOD_D: [f32; 3] = [0.38, 0.24, 0.13];
    const OLIVE: [f32; 3] = [0.34, 0.37, 0.25];
    const LENS: [f32; 3] = [0.45, 0.80, 1.0];
    const BRASS: [f32; 3] = [0.85, 0.66, 0.28];
    let frame = camera_frame(pose);
    let (skin_c, _, _, sleeve_c, _) = skin_colors(pose.skin);
    let sleeve_c = pose.sleeve.unwrap_or(sleeve_c);
    let (off, btilt, _, _) = body_motion(pose);
    let shot = g.recoil.clamp(0.0, 1.0);
    let kick = shot * shot;
    // Pump / bolt cycle after the shot: 0 → 1 → 0.
    let cycle = (g.reload.clamp(0.0, 1.0) * std::f32::consts::PI).sin();
    let anchor = match g.kind {
        2 | 6 => Vec3::new(0.15, -0.16, 0.30),
        9 | 10 => Vec3::new(0.18, -0.20, 0.16),
        7 => Vec3::new(0.17, -0.15, 0.32),
        8 => Vec3::new(0.24, -0.08, 0.36),
        5 => Vec3::new(0.17, -0.19, 0.12),
        _ => Vec3::new(0.18, -0.20, 0.16),
    };
    let heavy = matches!(g.kind, 4 | 5);
    let base = anchor + off + Vec3::new(0.0, 0.02 * kick, -(if heavy { 0.12 } else { 0.07 }) * kick);
    // Electric tools buzz while they work.
    let work = if g.loaded { g.work.clamp(0.0, 1.0) } else { 0.0 };
    let electric = matches!(g.kind, 11 | 12 | 14);
    // Idle hum when charged, a hard buzz while it works.
    let buzz = if electric && g.loaded { Vec3::new((pose.time * 97.0).sin(), (pose.time * 113.0).cos(), 0.0) * (0.0008 + 0.0045 * work) } else { Vec3::ZERO };
    // Drill: pushed into the block and pecking; chainsaw: sawing back and forth.
    let push = match g.kind {
        11 | 14 => Vec3::new(0.0, 0.0, work * (0.05 + 0.015 * (pose.time * 14.0).sin())),
        12 => Vec3::new(0.0, -0.02 * work, work * (0.03 + 0.03 * (pose.time * 9.0).sin())),
        _ => Vec3::ZERO,
    };
    let base = base + buzz + push;
    let angle = if g.kind == 13 {
        // The energy sword swings down and across like any blade.
        1.3 * (pose.punch_cycle.clamp(0.0, 1.0) * std::f32::consts::PI).sin() + btilt * 0.5
    } else if g.kind == 8 {
        // The hammer comes down: the head swings forward over the grip.
        1.4 * (shot * std::f32::consts::PI).sin() + btilt * 0.5
    } else if g.kind == 12 {
        // The chainsaw bar dips into the cut.
        0.28 * work + btilt * 0.5
    } else {
        -(if heavy { 0.28 } else { 0.18 }) * kick + btilt * 0.5
    };
    let pivot = base;
    let mut v: Vec<ModelVertex> = Vec::with_capacity(1500);
    let bx = |v: &mut Vec<ModelVertex>, a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
        add_hand_box(v, &frame, base + Vec3::from(a), base + Vec3::from(b), pivot, angle, c);
    };
    let muzzle_z;
    match g.kind {
        2 => {
            // Pistol: the slide runs back on recoil.
            let s = -0.06 * kick;
            bx(&mut v, [-0.030, 0.000, 0.00 + s], [0.030, 0.062, 0.30 + s], GUN);
            bx(&mut v, [-0.031, 0.030, 0.03 + s], [0.031, 0.050, 0.10 + s], DARK);
            bx(&mut v, [-0.006, 0.062, 0.27 + s], [0.006, 0.078, 0.29 + s], DARK);
            bx(&mut v, [-0.024, 0.062, 0.01 + s], [-0.010, 0.076, 0.03 + s], DARK);
            bx(&mut v, [0.010, 0.062, 0.01 + s], [0.024, 0.076, 0.03 + s], DARK);
            bx(&mut v, [-0.028, -0.030, 0.02], [0.028, 0.002, 0.27], DARK);
            bx(&mut v, [-0.012, -0.012, 0.29], [0.012, 0.012, 0.305], DARK);
            bx(&mut v, [-0.027, -0.17, -0.01], [0.027, -0.028, 0.08], DARK);
            bx(&mut v, [-0.029, -0.15, 0.0], [0.029, -0.05, 0.07], GUN);
            bx(&mut v, [-0.007, -0.075, 0.08], [0.007, -0.028, 0.15], DARK);
            muzzle_z = 0.31;
            // Two-handed grip.
            bx(&mut v, [-0.050, -0.15, -0.03], [0.050, -0.04, 0.09], skin_c);
            bx(&mut v, [-0.060, -0.46, -0.22], [0.060, -0.14, 0.01], sleeve_c);
            bx(&mut v, [-0.110, -0.15, -0.01], [-0.030, -0.05, 0.10], skin_c);
            bx(&mut v, [-0.32, -0.46, -0.10], [-0.08, -0.13, 0.06], sleeve_c);
        }
        3 => {
            // Assault rifle: receiver, handguard, barrel with a muzzle brake, curved magazine,
            // stock, rail with sights.
            bx(&mut v, [-0.040, -0.050, -0.06], [0.040, 0.050, 0.40], DARK);
            bx(&mut v, [-0.041, -0.010, 0.10], [0.041, 0.030, 0.22], GUN);
            bx(&mut v, [-0.036, -0.042, 0.40], [0.036, 0.042, 0.72], OLIVE);
            for z in [0.46f32, 0.54, 0.62] {
                bx(&mut v, [-0.038, -0.010, z], [0.038, 0.010, z + 0.04], DARK);
            }
            bx(&mut v, [-0.013, -0.013, 0.72], [0.013, 0.013, 0.90], STEEL);
            bx(&mut v, [-0.021, -0.021, 0.88], [0.021, 0.021, 0.95], DARK);
            bx(&mut v, [-0.030, -0.20, 0.17], [0.030, -0.05, 0.27], GUN);
            bx(&mut v, [-0.030, -0.29, 0.21], [0.030, -0.19, 0.30], GUN);
            bx(&mut v, [-0.025, -0.17, 0.01], [0.025, -0.05, 0.08], DARK);
            bx(&mut v, [-0.008, -0.085, 0.08], [0.008, -0.05, 0.14], DARK);
            bx(&mut v, [-0.034, -0.11, -0.42], [0.034, 0.030, -0.06], DARK);
            bx(&mut v, [-0.038, -0.13, -0.46], [0.038, 0.040, -0.40], GUN);
            bx(&mut v, [-0.018, 0.050, -0.02], [0.018, 0.068, 0.38], GUN);
            bx(&mut v, [-0.020, 0.068, 0.02], [0.020, 0.105, 0.06], DARK);
            bx(&mut v, [-0.006, 0.042, 0.66], [0.006, 0.105, 0.68], DARK);
            muzzle_z = 0.95;
            bx(&mut v, [-0.060, -0.17, -0.02], [0.060, -0.05, 0.10], skin_c);
            bx(&mut v, [-0.070, -0.52, -0.16], [0.070, -0.16, 0.04], sleeve_c);
            bx(&mut v, [-0.065, -0.10, 0.48], [0.065, 0.01, 0.62], skin_c);
            bx(&mut v, [-0.30, -0.55, 0.38], [-0.04, -0.08, 0.56], sleeve_c);
        }
        4 => {
            // Pump shotgun: the forend slides back and forth after the shot.
            let pump = -0.13 * cycle;
            bx(&mut v, [-0.028, 0.000, 0.06], [0.028, 0.054, 0.96], STEEL);
            bx(&mut v, [-0.024, -0.050, 0.06], [0.024, 0.000, 0.86], DARK);
            bx(&mut v, [-0.046, -0.072, 0.44 + pump], [0.046, 0.012, 0.66 + pump], WOOD);
            for z in [0.48f32, 0.53, 0.58] {
                bx(&mut v, [-0.048, -0.060, z + pump], [0.048, -0.040, z + 0.02 + pump], WOOD_D);
            }
            bx(&mut v, [-0.042, -0.065, -0.10], [0.042, 0.062, 0.10], DARK);
            bx(&mut v, [-0.008, -0.11, -0.02], [0.008, -0.065, 0.05], DARK);
            bx(&mut v, [-0.036, -0.15, -0.48], [0.036, 0.025, -0.10], WOOD);
            bx(&mut v, [-0.040, -0.17, -0.52], [0.040, 0.030, -0.47], WOOD_D);
            bx(&mut v, [-0.006, 0.054, 0.92], [0.006, 0.068, 0.94], BRASS);
            if cycle > 0.4 {
                // A spent red shell flies out of the port.
                let t = (g.reload - 0.5).max(0.0) * 2.0;
                let p = [0.06 + 0.25 * t, 0.07 + 0.12 * t, 0.02];
                bx(&mut v, p, [p[0] + 0.03, p[1] + 0.03, p[2] + 0.07], [0.80, 0.16, 0.14]);
            }
            muzzle_z = 0.96;
            bx(&mut v, [-0.060, -0.17, -0.10], [0.060, -0.06, 0.02], skin_c);
            bx(&mut v, [-0.070, -0.52, -0.24], [0.070, -0.16, -0.04], sleeve_c);
            bx(&mut v, [-0.065, -0.11, 0.48 + pump], [0.065, 0.0, 0.62 + pump], skin_c);
            bx(&mut v, [-0.30, -0.55, 0.38 + pump], [-0.04, -0.09, 0.56 + pump], sleeve_c);
        }
        6 => {
            // Star blaster: a chunky sci-fi pistol — white-steel body, a glowing cyan energy
            // cell on the side, a ribbed emitter at the front, a gold trigger guard.
            const WHITE: [f32; 3] = [0.82, 0.85, 0.92];
            const CYAN: [f32; 3] = [0.40, 0.95, 1.0];
            const GOLD: [f32; 3] = [1.0, 0.82, 0.30];
            bx(&mut v, [-0.045, -0.020, -0.04], [0.045, 0.075, 0.34], WHITE);
            bx(&mut v, [-0.046, 0.075, 0.00], [0.046, 0.095, 0.28], GUN);
            for (k, z) in [0.34f32, 0.38, 0.42].iter().enumerate() {
                let r = 0.040 - k as f32 * 0.006;
                bx(&mut v, [-r, 0.028 - r, *z], [r, 0.028 + r, z + 0.03], if k % 2 == 0 { GUN } else { WHITE });
            }
            let glow_from = v.len();
            bx(&mut v, [0.046, 0.005, 0.06], [0.062, 0.060, 0.24], CYAN);
            bx(&mut v, [-0.012, 0.016, 0.45], [0.012, 0.040, 0.46], CYAN);
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            bx(&mut v, [-0.030, -0.17, -0.02], [0.030, -0.020, 0.08], GUN);
            bx(&mut v, [-0.008, -0.075, 0.08], [0.008, -0.020, 0.15], GOLD);
            bx(&mut v, [-0.006, -0.08, 0.08], [0.006, -0.07, 0.16], GOLD);
            muzzle_z = 0.47;
            bx(&mut v, [-0.052, -0.15, -0.03], [0.052, -0.04, 0.09], skin_c);
            bx(&mut v, [-0.062, -0.46, -0.22], [0.062, -0.14, 0.01], sleeve_c);
            bx(&mut v, [-0.115, -0.15, -0.01], [-0.032, -0.05, 0.10], skin_c);
            bx(&mut v, [-0.32, -0.46, -0.10], [-0.08, -0.13, 0.06], sleeve_c);
        }
        7 => {
            // Star remote: a dark handheld box with a screen, a flip cover lifted over the red
            // button, a little antenna with a glowing star on top. Held in one hand.
            const BODY: [f32; 3] = [0.12, 0.13, 0.18];
            const GREY: [f32; 3] = [0.32, 0.34, 0.40];
            bx(&mut v, [-0.055, -0.030, -0.02], [0.055, 0.020, 0.18], BODY);
            bx(&mut v, [-0.050, 0.020, 0.10], [0.050, 0.024, 0.165], GREY);
            // Flip cover, open: stands up at the back of the button.
            bx(&mut v, [-0.030, 0.020, 0.025], [0.030, 0.085, 0.032], [0.55, 0.60, 0.70]);
            bx(&mut v, [-0.003, 0.090, 0.150], [0.003, 0.17, 0.156], GREY);
            let glow_from = v.len();
            bx(&mut v, [-0.022, 0.020, 0.040], [0.022, 0.034, 0.080], [0.95, 0.12, 0.10]);
            bx(&mut v, [-0.045, 0.025, 0.110], [0.045, 0.027, 0.158], [0.45, 0.70, 1.0]);
            bx(&mut v, [-0.014, 0.165, 0.139], [0.014, 0.19, 0.167], [1.0, 0.92, 0.45]);
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            muzzle_z = 0.2;
            bx(&mut v, [-0.065, -0.07, -0.04], [0.065, -0.005, 0.10], skin_c);
            bx(&mut v, [-0.065, -0.46, -0.24], [0.065, -0.06, 0.0], sleeve_c);
        }
        8 => {
            // Star hammer: a long handle wrapped in leather, a massive steel head with a glowing
            // blue star core on both faces; swung down hard (the recoil lifts it high first).
            const STEELW: [f32; 3] = [0.70, 0.73, 0.80];
            const LEATHER: [f32; 3] = [0.42, 0.26, 0.14];
            bx(&mut v, [-0.020, -0.30, -0.02], [0.020, 0.30, 0.02], LEATHER);
            for y in [-0.24f32, -0.12, 0.0, 0.12] {
                bx(&mut v, [-0.023, y, -0.023], [0.023, y + 0.03, 0.023], [0.30, 0.18, 0.10]);
            }
            bx(&mut v, [-0.13, 0.30, -0.075], [0.13, 0.46, 0.075], STEELW);
            bx(&mut v, [-0.15, 0.32, -0.085], [-0.13, 0.44, 0.085], STEEL);
            bx(&mut v, [0.13, 0.32, -0.085], [0.15, 0.44, 0.085], STEEL);
            let glow_from = v.len();
            bx(&mut v, [-0.155, 0.35, -0.04], [-0.15, 0.41, 0.04], [0.55, 0.85, 1.0]);
            bx(&mut v, [0.15, 0.35, -0.04], [0.155, 0.41, 0.04], [0.55, 0.85, 1.0]);
            bx(&mut v, [-0.04, 0.35, 0.075], [0.04, 0.41, 0.08], [0.55, 0.85, 1.0]);
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            muzzle_z = 0.0;
            bx(&mut v, [-0.045, -0.20, -0.045], [0.045, -0.08, 0.045], skin_c);
            bx(&mut v, [-0.06, -0.55, -0.20], [0.06, -0.18, 0.02], sleeve_c);
        }
        9 => {
            // WolfTech plasma rifle: graphite body, a glowing cyan coil wound round the barrel,
            // a battery pack under it, a short emitter with fins.
            const GRAPH: [f32; 3] = [0.20, 0.23, 0.30];
            const CYAN: [f32; 3] = [0.35, 0.92, 1.0];
            bx(&mut v, [-0.042, -0.050, -0.06], [0.042, 0.060, 0.40], GRAPH);
            bx(&mut v, [-0.030, -0.030, 0.40], [0.030, 0.040, 0.78], GUN);
            for k in 0..5 {
                let z = 0.44 + k as f32 * 0.065;
                bx(&mut v, [-0.040, -0.040, z], [0.040, 0.050, z + 0.025], DARK);
            }
            bx(&mut v, [-0.050, -0.055, 0.78], [0.050, 0.065, 0.84], DARK);
            bx(&mut v, [-0.034, -0.11, 0.08], [0.034, -0.05, 0.30], DARK);
            bx(&mut v, [-0.026, 0.060, 0.04], [0.026, 0.11, 0.28], DARK);
            bx(&mut v, [-0.040, -0.12, -0.42], [0.040, 0.040, -0.06], GRAPH);
            bx(&mut v, [-0.028, -0.17, -0.02], [0.028, -0.05, 0.05], DARK);
            let glow_from = v.len();
            for k in 0..5 {
                let z = 0.465 + k as f32 * 0.065;
                bx(&mut v, [-0.044, -0.020, z], [0.044, 0.030, z + 0.02], CYAN);
            }
            bx(&mut v, [0.042, -0.02, 0.06], [0.050, 0.035, 0.34], CYAN);
            bx(&mut v, [-0.020, -0.010, 0.841], [0.020, 0.030, 0.845], CYAN);
            bx(&mut v, [-0.018, 0.066, 0.25], [0.018, 0.104, 0.282], CYAN);
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            muzzle_z = 0.85;
            bx(&mut v, [-0.060, -0.19, -0.05], [0.060, -0.07, 0.07], skin_c);
            bx(&mut v, [-0.070, -0.55, -0.20], [0.070, -0.18, 0.02], sleeve_c);
            bx(&mut v, [-0.065, -0.15, 0.46], [0.065, -0.04, 0.60], skin_c);
            bx(&mut v, [-0.30, -0.56, 0.36], [-0.04, -0.12, 0.54], sleeve_c);
        }
        11 => {
            // WolfTech drill: a graphite body with a pistol grip, a battery pack, a steel
            // chuck and a long spiral bit that spins while it digs.
            const GRAPH: [f32; 3] = [0.20, 0.23, 0.30];
            const CYAN: [f32; 3] = [0.35, 0.92, 1.0];
            bx(&mut v, [-0.045, -0.040, -0.06], [0.045, 0.060, 0.24], GRAPH);
            bx(&mut v, [-0.035, -0.17, -0.02], [0.035, -0.04, 0.06], DARK);
            bx(&mut v, [-0.042, -0.21, -0.06], [0.042, -0.16, 0.10], GRAPH);
            bx(&mut v, [-0.030, -0.025, 0.24], [0.030, 0.045, 0.31], STEEL);
            let spin = pose.time * 45.0 * work;
            for k in 0..6 {
                let z = 0.31 + k as f32 * 0.045;
                let a = spin + k as f32 * 1.1;
                let r = 0.022 - k as f32 * 0.002;
                let (sx, sy) = (a.cos() * 0.008, a.sin() * 0.008);
                bx(&mut v, [-r + sx, -r + 0.01 + sy, z], [r + sx, r + 0.01 + sy, z + 0.04], if k % 2 == 0 { STEEL } else { [0.62, 0.64, 0.68] });
            }
            let glow_from = v.len();
            // Charge strip: dim when empty, pulsing while it works.
            let pulse = if g.loaded { 0.55 + 0.45 * work * (0.5 + 0.5 * (pose.time * 12.0).sin()) } else { 0.15 };
            bx(&mut v, [0.045, 0.0, 0.04], [0.050, 0.04, 0.18], [CYAN[0] * pulse, CYAN[1] * pulse, CYAN[2] * pulse]);
            // Sparks flying off the bit tip.
            if work > 0.4 {
                for k in 0..5u32 {
                    let t = (pose.time * 7.0 + k as f32 * 0.37).fract();
                    let a = k as f32 * 2.4 + (pose.time * 3.0).floor() * 1.7;
                    let (px, py, pz) = (a.cos() * 0.10 * t, a.sin() * 0.08 * t + 0.03 * t, 0.58 - 0.12 * t);
                    let f = 0.006 * (1.0 - t);
                    bx(&mut v, [px - f, py - f, pz - f], [px + f, py + f, pz + f], [1.0, 0.85 - 0.4 * t, 0.35]);
                }
            }
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            muzzle_z = 0.6;
            bx(&mut v, [-0.055, -0.17, -0.05], [0.055, -0.06, 0.07], skin_c);
            bx(&mut v, [-0.065, -0.52, -0.22], [0.065, -0.16, 0.02], sleeve_c);
        }
        12 => {
            // WolfTech chainsaw: a motor housing with a top handle, a long bar and a chain
            // whose teeth run while it cuts.
            const GRAPH: [f32; 3] = [0.20, 0.23, 0.30];
            const ORANGE: [f32; 3] = [0.95, 0.55, 0.15];
            bx(&mut v, [-0.055, -0.060, -0.10], [0.055, 0.070, 0.20], ORANGE);
            bx(&mut v, [-0.020, 0.070, -0.06], [0.020, 0.12, 0.14], DARK);
            bx(&mut v, [-0.012, -0.020, 0.20], [0.012, 0.050, 0.62], STEEL);
            let run = (pose.time * (2.0 + 30.0 * work)).fract() * 0.04 * (work > 0.05) as u8 as f32;
            for k in 0..10 {
                let z = 0.21 + ((k as f32 * 0.04 + run) % 0.40);
                bx(&mut v, [-0.014, 0.050, z], [0.014, 0.062, z + 0.02], DARK);
                bx(&mut v, [-0.014, -0.032, z], [0.014, -0.020, z + 0.02], DARK);
            }
            bx(&mut v, [-0.040, -0.17, -0.10], [0.040, -0.06, 0.0], GRAPH);
            if work > 0.3 {
                // Sawdust thrown back from the tip of the bar.
                for k in 0..6u32 {
                    let t = (pose.time * 5.0 + k as f32 * 0.29).fract();
                    let sx = ((k as f32 * 1.9).sin()) * 0.07 * t;
                    let (py, pz) = (-0.03 - 0.10 * t + 0.06 * t * t, 0.60 - 0.22 * t);
                    let f = 0.008 * (1.0 - 0.5 * t);
                    bx(&mut v, [sx - f, py - f, pz - f], [sx + f, py + f, pz + f], [0.80, 0.62, 0.38]);
                }
            }
            if g.loaded {
                // Motor exhaust: grey puffs drifting up from the back, thicker under load.
                for k in 0..(1 + (work * 3.0) as u32) {
                    let t = (pose.time * 1.6 + k as f32 * 0.33).fract();
                    let f = 0.012 + 0.025 * t;
                    let (px, py, pz) = (0.06 + 0.02 * t, 0.04 + 0.16 * t, -0.10 - 0.05 * t);
                    let c = 0.55 + 0.25 * t;
                    bx(&mut v, [px - f, py - f, pz - f], [px + f, py + f, pz + f], [c, c, c]);
                }
            }
            muzzle_z = 0.6;
            bx(&mut v, [-0.055, -0.17, -0.10], [0.055, -0.06, 0.02], skin_c);
            bx(&mut v, [-0.065, -0.52, -0.28], [0.065, -0.16, -0.04], sleeve_c);
            bx(&mut v, [-0.05, 0.10, -0.04], [0.05, 0.16, 0.08], skin_c);
        }
        14 => {
            // WolfTech omni-tool: a dark body, a steel head with pick, axe and shovel edges, a
            // gold bit that spins while it digs, a pulsing charge light.
            const GRAPH: [f32; 3] = [0.20, 0.23, 0.30];
            const GOLD: [f32; 3] = [1.0, 0.80, 0.25];
            bx(&mut v, [-0.050, -0.050, -0.08], [0.050, 0.070, 0.20], GRAPH);
            bx(&mut v, [-0.075, 0.000, 0.20], [0.075, 0.045, 0.28], STEEL);
            bx(&mut v, [-0.095, -0.020, 0.22], [-0.065, 0.060, 0.30], STEEL);
            bx(&mut v, [0.065, -0.020, 0.22], [0.095, 0.060, 0.30], STEEL);
            let spin = pose.time * 40.0 * work;
            for k in 0..4 {
                let a = spin + k as f32 * std::f32::consts::FRAC_PI_2;
                let (sx, sy) = (a.cos() * 0.022, a.sin() * 0.022 + 0.02);
                bx(&mut v, [sx - 0.010, sy - 0.010, 0.28], [sx + 0.010, sy + 0.010, 0.46], GOLD);
            }
            let glow_from = v.len();
            let pulse = if g.loaded { 0.55 + 0.45 * work * (0.5 + 0.5 * (pose.time * 12.0).sin()) } else { 0.15 };
            bx(&mut v, [0.050, 0.010, 0.00], [0.054, 0.050, 0.16], [0.35 * pulse, 0.92 * pulse, 1.0 * pulse]);
            if work > 0.4 {
                for k in 0..4u32 {
                    let t = (pose.time * 6.0 + k as f32 * 0.41).fract();
                    let a = k as f32 * 2.1 + (pose.time * 2.0).floor();
                    let (px, py, pz) = (a.cos() * 0.09 * t, a.sin() * 0.07 * t + 0.03, 0.47 - 0.10 * t);
                    let f = 0.006 * (1.0 - t);
                    bx(&mut v, [px - f, py - f, pz - f], [px + f, py + f, pz + f], [1.0, 0.85 - 0.4 * t, 0.35]);
                }
            }
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            muzzle_z = 0.48;
            bx(&mut v, [-0.03, -0.17, -0.02], [0.03, -0.04, 0.06], DARK);
            bx(&mut v, [-0.055, -0.17, -0.06], [0.055, -0.06, 0.06], skin_c);
            bx(&mut v, [-0.065, -0.52, -0.28], [0.065, -0.16, -0.04], sleeve_c);
        }
        15 => {
            // WolfTech mining laser: a long dark body, a red emitter with glowing rings, a
            // charge strip; the emitter flares right after a shot (recoil).
            const GRAPH: [f32; 3] = [0.20, 0.23, 0.30];
            const RED: [f32; 3] = [0.95, 0.25, 0.10];
            bx(&mut v, [-0.045, -0.060, -0.12], [0.045, 0.060, 0.32], GRAPH);
            bx(&mut v, [-0.030, -0.030, 0.32], [0.030, 0.030, 0.52], RED);
            let glow_from = v.len();
            let flare = 0.5 + 1.5 * kick;
            for k in 0..3 {
                let z = 0.36 + k as f32 * 0.05;
                bx(&mut v, [-0.034, -0.034, z], [0.034, 0.034, z + 0.012], [RED[0] * flare, (RED[1] + 0.3) * flare, RED[2] * flare]);
            }
            let charge = if g.loaded { 1.0 } else { 0.2 };
            bx(&mut v, [0.045, 0.00, 0.00], [0.049, 0.040, 0.24], [0.35 * charge, 0.92 * charge, 1.0 * charge]);
            if kick > 0.2 {
                bx(&mut v, [-0.012, -0.012, 0.52], [0.012, 0.012, 0.52 + 1.6 * kick], [1.0, 0.45, 0.30]);
            }
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            muzzle_z = 0.55;
            bx(&mut v, [-0.03, -0.17, -0.02], [0.03, -0.05, 0.05], DARK);
            bx(&mut v, [-0.055, -0.17, -0.06], [0.055, -0.06, 0.04], skin_c);
            bx(&mut v, [-0.065, -0.52, -0.28], [0.065, -0.16, -0.04], sleeve_c);
            bx(&mut v, [-0.05, -0.04, 0.12], [0.05, 0.02, 0.22], skin_c);
        }
        13 => {
            // WolfTech energy sword: a dark hilt with a gold guard and a long glowing blade.
            const GOLD: [f32; 3] = [1.0, 0.80, 0.25];
            bx(&mut v, [-0.020, -0.12, -0.02], [0.020, 0.02, 0.02], DARK);
            bx(&mut v, [-0.065, 0.02, -0.025], [0.065, 0.05, 0.025], GOLD);
            let glow_from = v.len();
            bx(&mut v, [-0.018, 0.05, -0.008], [0.018, 0.62, 0.008], [0.35, 0.95, 1.0]);
            bx(&mut v, [-0.008, 0.05, -0.010], [0.008, 0.64, 0.010], [0.90, 1.0, 1.0]);
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            muzzle_z = 0.0;
            bx(&mut v, [-0.045, -0.10, -0.045], [0.045, 0.0, 0.045], skin_c);
            bx(&mut v, [-0.06, -0.48, -0.18], [0.06, -0.10, 0.02], sleeve_c);
        }
        10 => {
            // WolfTech railgun: two long blue rails with a charge glowing between them, a
            // heavy breech with capacitors, a stock.
            const GRAPH: [f32; 3] = [0.20, 0.23, 0.30];
            const BLUE: [f32; 3] = [0.30, 0.50, 1.0];
            let charge = if g.loaded { 1.0 } else { 0.3 };
            bx(&mut v, [-0.050, -0.070, -0.08], [0.050, 0.070, 0.36], GRAPH);
            for sx in [-1.0f32, 1.0] {
                bx(&mut v, [sx * 0.034 - 0.012, -0.040, 0.36], [sx * 0.034 + 0.012, 0.050, 1.10], STEEL);
                bx(&mut v, [sx * 0.034 - 0.014, -0.044, 0.40], [sx * 0.034 + 0.014, -0.030, 1.06], DARK);
            }
            for z in [0.50f32, 0.75, 1.00] {
                bx(&mut v, [-0.052, 0.050, z], [0.052, 0.066, z + 0.04], DARK);
            }
            for k in 0..3 {
                let z = 0.0 + k as f32 * 0.11;
                bx(&mut v, [0.050, -0.040, z], [0.080, 0.040, z + 0.08], DARK);
            }
            bx(&mut v, [-0.036, 0.070, 0.0], [0.036, 0.13, 0.30], DARK);
            bx(&mut v, [-0.040, -0.14, -0.50], [0.040, 0.045, -0.08], GRAPH);
            bx(&mut v, [-0.028, -0.18, -0.02], [0.028, -0.07, 0.05], DARK);
            let glow_from = v.len();
            bx(&mut v, [-0.020, -0.015, 0.38], [0.020, 0.025, 0.38 + 0.70 * charge], BLUE);
            for k in 0..3 {
                let z = 0.02 + k as f32 * 0.11;
                bx(&mut v, [0.080, -0.020, z], [0.084, 0.020, z + 0.04], BLUE);
            }
            bx(&mut v, [-0.026, 0.131, 0.27], [0.026, 0.135, 0.30], [0.45, 0.80, 1.0]);
            for q in v[glow_from..].iter_mut() {
                q.tex = 3.0;
            }
            muzzle_z = 1.12;
            bx(&mut v, [-0.060, -0.20, -0.05], [0.060, -0.08, 0.07], skin_c);
            bx(&mut v, [-0.070, -0.56, -0.20], [0.070, -0.19, 0.02], sleeve_c);
            bx(&mut v, [-0.065, -0.16, 0.50], [0.065, -0.05, 0.64], skin_c);
            bx(&mut v, [-0.30, -0.57, 0.40], [-0.04, -0.13, 0.58], sleeve_c);
        }
        _ => {
            // Sniper rifle: long barrel, big scope, bolt that is worked after every shot.
            let bolt = -0.08 * cycle;
            bx(&mut v, [-0.038, -0.060, -0.04], [0.038, 0.045, 0.34], DARK);
            bx(&mut v, [-0.036, -0.075, 0.34], [0.036, 0.030, 0.74], OLIVE);
            bx(&mut v, [-0.015, -0.015, 0.74], [0.015, 0.015, 1.12], STEEL);
            bx(&mut v, [-0.026, -0.026, 1.08], [0.026, 0.026, 1.18], DARK);
            // Scope with lenses, mounts and turrets.
            bx(&mut v, [-0.036, 0.075, -0.02], [0.036, 0.145, 0.42], DARK);
            bx(&mut v, [-0.046, 0.066, 0.36], [0.046, 0.155, 0.46], DARK);
            bx(&mut v, [-0.030, 0.080, 0.461], [0.030, 0.140, 0.465], LENS);
            bx(&mut v, [-0.026, 0.085, -0.025], [0.026, 0.135, -0.02], LENS);
            bx(&mut v, [-0.016, 0.145, 0.16], [0.016, 0.175, 0.22], GUN);
            bx(&mut v, [0.036, 0.095, 0.16], [0.062, 0.125, 0.22], GUN);
            bx(&mut v, [-0.020, 0.045, 0.02], [0.020, 0.075, 0.06], GUN);
            bx(&mut v, [-0.020, 0.045, 0.30], [0.020, 0.075, 0.34], GUN);
            // Bolt handle on the right.
            bx(&mut v, [0.038, 0.010, 0.04 + bolt], [0.085, 0.030, 0.07 + bolt], STEEL);
            bx(&mut v, [0.080, -0.005, 0.035 + bolt], [0.105, 0.040, 0.075 + bolt], DARK);
            bx(&mut v, [-0.028, -0.13, 0.12], [0.028, -0.06, 0.20], GUN);
            bx(&mut v, [-0.026, -0.18, -0.02], [0.026, -0.06, 0.05], OLIVE);
            bx(&mut v, [-0.008, -0.10, 0.05], [0.008, -0.06, 0.11], DARK);
            bx(&mut v, [-0.036, -0.13, -0.50], [0.036, 0.040, -0.04], OLIVE);
            bx(&mut v, [-0.040, -0.15, -0.54], [0.040, 0.045, -0.49], DARK);
            bx(&mut v, [-0.030, 0.040, -0.36], [0.030, 0.070, -0.12], OLIVE);
            muzzle_z = 1.18;
            bx(&mut v, [-0.060, -0.19, -0.05], [0.060, -0.07, 0.07], skin_c);
            bx(&mut v, [-0.070, -0.55, -0.20], [0.070, -0.18, 0.02], sleeve_c);
            bx(&mut v, [-0.065, -0.13, 0.50], [0.065, -0.02, 0.64], skin_c);
            bx(&mut v, [-0.30, -0.56, 0.40], [-0.04, -0.11, 0.58], sleeve_c);
        }
    }
    if shot > 0.45 && !matches!(g.kind, 7 | 8 | 11 | 12 | 13) {
        // Muzzle flash: a bright star for the first moments, unlit.
        let start = v.len();
        let k = (shot - 0.45) / 0.55;
        let z = muzzle_z;
        let f = if g.kind == 4 { 0.08 } else { 0.05 } * (0.6 + 0.6 * k);
        bx(&mut v, [-f, -f, z], [f, f, z + 0.06 + 0.16 * k], [1.0, 0.70, 0.25]);
        bx(&mut v, [-f * 1.8, -f * 0.3, z + 0.02], [f * 1.8, f * 0.3, z + 0.06], [1.0, 0.85, 0.45]);
        bx(&mut v, [-f * 0.3, -f * 1.8, z + 0.02], [f * 0.3, f * 1.8, z + 0.06], [1.0, 0.85, 0.45]);
        bx(&mut v, [-f * 0.5, -f * 0.5, z], [f * 0.5, f * 0.5, z + 0.10 + 0.22 * k], [1.0, 0.97, 0.75]);
        for q in v[start..].iter_mut() {
            q.tex = 3.0;
        }
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
        let d = Vec3::new(0.10, 1.0, -0.02).normalize();
        let to_cam = Vec3::new(-0.55, 0.0, -1.0).normalize();
        let to_cam = (to_cam - d * d.dot(to_cam)).normalize();
        let e = d.cross(to_cam);
        // The shaft on the icon (from the grip up to the head) in icon axes (x right, y up):
        // the pixel diagonal for most tools; the pickaxe handle is steeper. It is turned to
        // run along `d`, so every tool stands straight up in the fist.
        let (sx, sy) = if kind == 0 { (-0.47f32, 0.88f32) } else { (-std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2) };
        let l = (sx * sx + sy * sy).sqrt();
        let (sx, sy) = (sx / l, sy / l);
        let u = (d * sx + e * sy).normalize();
        let w = (d * sy - e * sx).normalize();
        let n = u.cross(w).normalize();
        // Same proportions as the hotbar icon, not a giant: the tool fills about a quarter of
        // the view's height, like in Minecraft.
        let s = 0.46;
        // The grip pixel (12.5, 12.5 from the top-left) sits in the fist.
        let o = f + Vec3::new(-0.01, 0.03, 0.02) - u * (s * 0.78) - w * (s * 0.22);
        add_voxel_icon(v, frame, &px, o, u, w, n, s, 1.0, pivot, angle);
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

/// Thin rod between `a` and `b` (a square prism `t` thick), tilted with the forearm like
/// `add_box`: bow limbs, the string and the arrow follow their real slope instead of a
/// staircase of cubes.
#[allow(clippy::too_many_arguments)]
fn add_rod(out: &mut Vec<ModelVertex>, f: &Frame, a: Vec3, b: Vec3, t: f32, pivot: Vec3, angle: f32, color: [f32; 3]) {
    let d = b - a;
    let len = d.length();
    if len < 1e-5 {
        return;
    }
    let dir = d / len;
    let helper = if dir.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let p1 = dir.cross(helper).normalize() * (t * 0.5);
    let p2 = dir.cross(p1).normalize() * (t * 0.5);
    let (s, c) = angle.sin_cos();
    let rot = |p: Vec3| {
        let q = p - pivot;
        pivot + Vec3::new(q.x, q.y * c - q.z * s, q.y * s + q.z * c)
    };
    let rot_n = |n: Vec3| Vec3::new(n.x, n.y * c - n.z * s, n.y * s + n.z * c);
    // Four long sides and two end caps.
    let corner = |end: Vec3, i: usize| end + match i { 0 => p1 + p2, 1 => -p1 + p2, 2 => -p1 - p2, _ => p1 - p2 };
    let mut quad = |q: [Vec3; 4], n: Vec3| {
        // Make the quad counter-clockwise seen from outside, then mirror it like `add_box`.
        let q = if (q[1] - q[0]).cross(q[2] - q[0]).dot(n) >= 0.0 { q } else { [q[0], q[3], q[2], q[1]] };
        let nw = f.dir(rot_n(n)).normalize_or_zero().to_array();
        let w: Vec<[f32; 3]> = q.iter().map(|p| f.to_world(rot(*p)).as_vec3().to_array()).collect();
        for idx in [0usize, 2, 1, 0, 3, 2] {
            out.push(ModelVertex { pos: w[idx], normal: nw, color, uv: [0.0; 2], tex: 0.0 });
        }
    };
    for i in 0..4 {
        let j = (i + 1) % 4;
        let n = ((corner(a, i) + corner(a, j)) * 0.5 - a).normalize();
        quad([corner(a, i), corner(a, j), corner(b, j), corner(b, i)], n);
    }
    quad([corner(a, 0), corner(a, 1), corner(a, 2), corner(a, 3)], -dir);
    quad([corner(b, 0), corner(b, 1), corner(b, 2), corner(b, 3)], dir);
}

/// Bow held upright in the fist, seen from the side: the wooden arc bulges away from the
/// camera, the string runs back towards it. `c` (0..1) is the draw: the string is pulled back,
/// the limbs bend more and an arrow lies on the string.
fn add_bow(v: &mut Vec<ModelVertex>, frame: &Frame, f: Vec3, pivot: Vec3, angle: f32, c: f32, tip: Option<[f32; 3]>) {
    const WOOD: [f32; 3] = [0.55, 0.36, 0.18];
    const GRIP: [f32; 3] = [0.36, 0.22, 0.11];
    const STRING: [f32; 3] = [0.92, 0.92, 0.88];
    let half = 0.22; // half the bow's height
    let bend = 0.07 + 0.04 * c;
    let grip = f + Vec3::new(0.0, 0.08, 0.04);
    // Limb point at t (-1 bottom .. 1 top): the tips curve back towards the string.
    let limb = |t: f32| grip + Vec3::new(0.0, t * half, -t * t * bend);
    let steps = 8;
    for i in 0..steps {
        let (t0, t1) = (i as f32 / steps as f32 * 2.0 - 1.0, (i + 1) as f32 / steps as f32 * 2.0 - 1.0);
        let mid = (t0 + t1) * 0.5;
        let thick = if mid.abs() < 0.3 { 0.036 } else { 0.026 - mid.abs() * 0.006 };
        add_rod(v, frame, limb(t0), limb(t1), thick, pivot, angle, if mid.abs() < 0.3 { GRIP } else { WOOD });
    }
    // String: tips to the nock, which the draw pulls back.
    let (top, bottom) = (limb(1.0), limb(-1.0));
    let nock = Vec3::new(grip.x, grip.y, top.z - 0.15 * c);
    add_rod(v, frame, top, nock, 0.007, pivot, angle, STRING);
    add_rod(v, frame, nock, bottom, 0.007, pivot, angle, STRING);
    if c > 0.05 {
        // Arrow on the string, pointing away from the camera past the grip.
        let head = nock + Vec3::new(0.0, 0.0, 0.46);
        add_rod(v, frame, nock, head, 0.012, pivot, angle, [0.62, 0.45, 0.25]);
        // The head; a tipped arrow's head is dipped in its potion and grows brighter as the
        // string is pulled.
        let head_c = match tip {
            Some(t) => [t[0] * (0.7 + 0.3 * c), t[1] * (0.7 + 0.3 * c), t[2] * (0.7 + 0.3 * c)],
            None => [0.72, 0.74, 0.78],
        };
        add_rod(v, frame, head, head + Vec3::new(0.0, 0.0, 0.04), 0.026, pivot, angle, head_c);
        if tip.is_some() {
            // Drops of the potion along the shaft.
            add_rod(v, frame, head - Vec3::new(0.0, 0.0, 0.08), head, 0.016, pivot, angle, head_c);
        }
        for dy in [-0.016f32, 0.016] {
            add_rod(v, frame, nock + Vec3::new(0.0, dy, 0.01), nock + Vec3::new(0.0, dy, 0.07), 0.01, pivot, angle, [0.95, 0.95, 0.93]);
        }
    }
}

/// Wooden shield with an iron rim and boss, facing away from the camera. Minecraft size: a
/// tall board that covers a good part of the left of the screen when raised.
fn add_shield(v: &mut Vec<ModelVertex>, frame: &Frame, f: Vec3, pivot: Vec3, angle: f32) {
    let b = |v: &mut Vec<ModelVertex>, lo: Vec3, hi: Vec3, c: [f32; 3]| add_box(v, frame, f + lo, f + hi, pivot, angle, c);
    let plank_d = [PLANK[0] * 0.82, PLANK[1] * 0.82, PLANK[2] * 0.82];
    // Iron rim, then the boards (alternating shades), the boss and a band across.
    b(v, Vec3::new(-0.17, -0.20, 0.04), Vec3::new(0.17, 0.32, 0.075), IRON);
    for i in 0..4 {
        let x = -0.155 + i as f32 * 0.0775;
        b(v, Vec3::new(x, -0.185, 0.035), Vec3::new(x + 0.0775, 0.305, 0.08), if i % 2 == 0 { PLANK } else { plank_d });
    }
    b(v, Vec3::new(-0.16, 0.04, 0.03), Vec3::new(0.16, 0.07, 0.085), IRON);
    b(v, Vec3::new(-0.04, 0.02, 0.025), Vec3::new(0.04, 0.10, 0.09), IRON);
    // Grip behind the board, in the fist.
    b(v, Vec3::new(-0.02, -0.04, -0.01), Vec3::new(0.02, 0.12, 0.035), [0.30, 0.22, 0.14]);
}

/// Extrudes a 16x16 icon into pixel boxes in the plane (`u` right, `w` up), one pixel deep
/// along `n`; `o` is the icon's bottom-left corner. Inner side faces are skipped.
#[allow(clippy::too_many_arguments)]
/// Thickness in pixels of a held or dropped icon: WolfTech items are chunky (3 px), the rest
/// one pixel like in Minecraft.
pub fn icon_depth(sprite: usize) -> f32 {
    if (wolfcraft_assets::items::IRON_DUST as usize..wolfcraft_assets::items::COUNT as usize).contains(&sprite) { 2.0 } else { 1.0 }
}

#[allow(clippy::too_many_arguments)]
fn add_voxel_icon(out: &mut Vec<ModelVertex>, frame: &Frame, px: &[[u8; 4]], o: Vec3, u: Vec3, w: Vec3, n: Vec3, s: f32, depth: f32, pivot: Vec3, angle: f32) {
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
            let base = o + u * (x as f32 * d) + w * ((15 - y) as f32 * d) - n * (d * depth * 0.5);
            let corner = |a: f32, b: f32, k: f32| base + u * (a * d) + w * (b * d) + n * (k * d * depth);
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
            assert_eq!(verts.len(), 2 * ARM);
            assert_outward(&verts);
            let fwd = p.forward.normalize();
            assert!(verts.iter().all(|v| Vec3::from(v.pos).dot(fwd) > 0.08), "arm behind the near plane");
        }
    }

    #[test]
    fn right_arm_is_in_view_at_rest_and_the_left_one_is_not() {
        let p = hands(0.0, 1.0);
        let v = build_first_person_hands(&p);
        let (r, l) = (centre(&v[..ARM]), centre(&v[ARM..]));
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
        assert!(reach(&hit[..ARM], fwd) > reach(&rest[..ARM], fwd) + 0.08, "the fist goes forward");
        assert!(off_axis(&hit[..ARM]) < off_axis(&rest[..ARM]) - 0.05, "and in towards the crosshair");
        assert!(rest.iter().zip(&done).all(|(a, b)| (Vec3::from(a.pos) - Vec3::from(b.pos)).length() < 0.01), "ends where it started");
        assert!(rest[ARM..].iter().zip(&hit[ARM..]).all(|(a, b)| a.pos == b.pos), "the left arm stays down");
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
        // In Minecraft style the arm holding an item is hidden: only the sword and the shield.
        assert!(rest.len() > 2 * 3 * 36, "sword and shield add boxes");
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
        let c0 = centre(&build_first_person_hands(&base)[..ARM]);
        // Turning right: the hand is left behind, to the left.
        let mut turn = base;
        turn.sway = [-1.0, 0.0];
        let c1 = centre(&build_first_person_hands(&turn)[..ARM]);
        assert!((c1 - c0).dot(right) < -0.03, "sway to the left ({})", (c1 - c0).dot(right));
        let mut run = base;
        run.sprint = 1.0;
        let c2 = centre(&build_first_person_hands(&run)[..ARM]);
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
        PlayerPose { feet: up * 1000.0, heading, up, pitch: 0.0, phase: 0.0, amount: 0.0, sneaking: false, swimming: false, rest: 0, skin: 0, attack: 0.0, held: Held::Nothing, armor: [None; 4], shield: false }
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
        match d.kind {
            // A thin streak along "up" for rain, a flat flake for snow.
            crate::mobs::FxKind::Rain => add_box(&mut v, &f, Vec3::new(-0.012, -0.24, -0.012), Vec3::new(0.012, 0.24, 0.012), Vec3::ZERO, 0.0, d.color),
            crate::mobs::FxKind::Snow => add_box(&mut v, &f, Vec3::new(-0.035, -0.01, -0.035), Vec3::new(0.035, 0.01, 0.035), Vec3::ZERO, d.spin + d.age, d.color),
            _ => add_box(&mut v, &f, Vec3::new(-h, -h, -h), Vec3::new(h, h, h), Vec3::ZERO, d.spin + d.age * 6.0, d.color),
        }
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



/// Two feathered wings attached to the back; folding uses the same equipment state as gliding.
pub fn build_wings(pose:&PlayerPose,camera:DVec3,spread:bool)->Vec<ModelVertex>{
    let mut out=Vec::new();let f=Frame{origin:pose.feet-camera,right:pose.heading.cross(pose.up).normalize_or_zero(),up:pose.up,fwd:pose.heading};
    let reach=if spread{1.45}else{0.45};
    for side in [-1.0f32,1.0] {for feather in 0..6 {
        let t=feather as f32/6.;let x=side*(0.2+reach*t);
        add_box(&mut out,&f,Vec3::new(x-0.13,0.55+t*0.5,-0.24-t*0.1),Vec3::new(x+0.13,1.45-t*0.15,-0.17-t*0.1),Vec3::ZERO,0.,[0.75+t*0.15,0.86+t*0.1,0.94]);
    }}out
}

/// Item-frame contents remain flat against the board and share its four rotations.
pub fn build_frame_models(grid:&crate::cubesphere::Grid,frames:&[(crate::cubesphere::Cell,crate::block::Block,crate::items::ItemStack)],camera:DVec3)->Vec<ModelVertex>{
    let mut out=Vec::new();
    for &(cell,block,stack) in frames {
        let Some((entry,rotation))=crate::construction::entry(block as u16)else{continue};
        if entry.form!=crate::construction::Form::ItemFrame{continue;}
        let (tile,tex)=match crate::items::info(stack.id).icon {crate::items::Icon::Item(tile)=>(tile,2.0),crate::items::Icon::Block(tile)=>(tile,4.0)};
        let mut points=[[0.18,0.025,0.18],[0.82,0.025,0.18],[0.82,0.025,0.82],[0.18,0.025,0.82]];
        for p in &mut points {for _ in 0..rotation {*p=[1.0-p[1],p[0],p[2]];}}
        let points=points.map(|p|grid.point(cell.f,cell.i as f64+p[0],cell.j as f64+p[1],cell.k as f64+p[2])-camera);
        let normal=(points[1]-points[0]).cross(points[2]-points[0]).normalize_or_zero().as_vec3().to_array();
        let uv=[[0.,1.],[1.,1.],[1.,0.],[0.,0.]];
        for i in [0usize,1,2,0,2,3,0,2,1,0,3,2] {out.push(ModelVertex{pos:points[i].as_vec3().to_array(),normal,color:[tile as f32,0.,0.],uv:uv[i],tex});}
    }out
}

/// The three guardians, each its own figure (built in the mob's frame: x right, y up, z
/// forward). Arms rise while an attack is cast; cracks glow brighter in later phases.
fn add_guardian(v:&mut Vec<ModelVertex>,f:&Frame,role:&crate::bosses::BossRole,anim:f32,amount:f32,hp:f32,tint:&dyn Fn([f32;3])->[f32;3]) {
    use crate::bosses::{Attack,BossKind};
    let z=Vec3::ZERO;
    let swing=anim.sin()*0.5*amount;
    let phase=role.kind.phase(hp);
    let casting=role.casting>0.0;
    let raise=if casting {match role.attack {Attack::Slam|Attack::Boulder=>-2.6,Attack::Heal|Attack::Summon=>-1.9,_=>-1.3}} else if role.charge>0.0 {-1.4} else {0.0};
    let glow_k=0.75+0.25*phase as f32;
    let mut b=|lo:Vec3,hi:Vec3,pivot:Vec3,angle:f32,c:[f32;3]|add_box(v,f,lo,hi,pivot,angle,tint(c));
    match role.kind {
        BossKind::Stone => {
            // A hulking stone golem: pillar legs, a broad cracked chest with glowing runes,
            // moss on the shoulders, a small sunken head and great boulder fists.
            let (st,st_d,st_l)=([0.50,0.54,0.58],[0.36,0.39,0.43],[0.62,0.66,0.70]);
            let moss=[0.30,0.55,0.25];
            let rune=[0.35*glow_k,0.95*glow_k,1.0];
            for side in [-1.0f32,1.0] {
                let hip=Vec3::new(side*0.42,1.0,0.0);
                b(Vec3::new(side*0.42-0.30,0.0,-0.32),Vec3::new(side*0.42+0.30,1.05,0.32),hip,swing*side,st_d);
                b(Vec3::new(side*0.42-0.34,0.0,-0.36),Vec3::new(side*0.42+0.34,0.22,0.40),hip,swing*side,st);
            }
            b(Vec3::new(-0.80,1.0,-0.50),Vec3::new(0.80,2.55,0.50),z,0.0,st);
            b(Vec3::new(-0.62,1.15,0.48),Vec3::new(0.62,2.35,0.56),z,0.0,st_l);
            // Runes and cracks on the chest.
            b(Vec3::new(-0.05,1.35,0.55),Vec3::new(0.05,2.20,0.60),z,0.0,rune);
            b(Vec3::new(-0.40,1.95,0.55),Vec3::new(0.40,2.03,0.60),z,0.0,rune);
            b(Vec3::new(-0.30,1.50,0.55),Vec3::new(-0.22,1.85,0.60),z,0.0,rune);
            b(Vec3::new(0.22,1.50,0.55),Vec3::new(0.30,1.85,0.60),z,0.0,rune);
            // Head, sunk between the shoulders, with a brow and two glowing eyes.
            b(Vec3::new(-0.34,2.45,-0.20),Vec3::new(0.34,3.05,0.40),z,0.0,st_l);
            b(Vec3::new(-0.38,2.85,0.30),Vec3::new(0.38,2.97,0.46),z,0.0,st_d);
            for side in [-1.0f32,1.0] {
                b(Vec3::new(side*0.17-0.08,2.68,0.40),Vec3::new(side*0.17+0.08,2.80,0.44),z,0.0,rune);
                // Shoulder boulders with moss, then the arm and a huge fist.
                let sh=Vec3::new(side*1.05,2.35,0.0);
                b(Vec3::new(side*1.05-0.38,2.15,-0.38),Vec3::new(side*1.05+0.38,2.75,0.38),z,0.0,st_d);
                b(Vec3::new(side*1.05-0.30,2.75,-0.25),Vec3::new(side*1.05+0.30,2.85,0.25),z,0.0,moss);
                let arm=raise-swing*side*0.6;
                b(Vec3::new(side*1.05-0.26,1.05,-0.26),Vec3::new(side*1.05+0.26,2.25,0.26),sh,arm,st);
                b(Vec3::new(side*1.05-0.40,0.55,-0.40),Vec3::new(side*1.05+0.40,1.15,0.40),sh,arm,st_d);
                b(Vec3::new(side*1.05-0.12,0.80,0.40),Vec3::new(side*1.05+0.12,0.92,0.44),sh,arm,rune);
            }
        }
        BossKind::Root => {
            // A walking tree: root legs, a bark trunk with a hollow glowing face, long branch
            // arms with leaf tufts and a leafy crown with flowers.
            let (bark,bark_d)=([0.42,0.28,0.17],[0.30,0.20,0.12]);
            let (leaf,leaf_l)=([0.22,0.55,0.22],[0.36,0.70,0.30]);
            let eye=[0.75*glow_k,1.0,0.35];
            for side in [-1.0f32,1.0] {
                let hip=Vec3::new(side*0.30,1.0,0.0);
                b(Vec3::new(side*0.30-0.20,0.15,-0.20),Vec3::new(side*0.30+0.20,1.05,0.20),hip,swing*side,bark);
                // Roots splaying out at the foot.
                for (dx,dz) in [(0.30,0.10),(-0.05,0.35),(0.05,-0.35)] {
                    b(Vec3::new(side*0.30+side*dx-0.08,0.0,dz-0.08),Vec3::new(side*0.30+side*dx+0.08,0.20,dz+0.08),hip,swing*side,bark_d);
                }
            }
            b(Vec3::new(-0.50,1.0,-0.42),Vec3::new(0.50,2.75,0.42),z,0.0,bark);
            for x in [-0.32f32,0.0,0.30] {b(Vec3::new(x-0.04,1.05,0.42),Vec3::new(x+0.04,2.70,0.46),z,0.0,bark_d);}
            // Hollow face: dark sockets with glowing eyes and a crooked mouth.
            for side in [-1.0f32,1.0] {
                b(Vec3::new(side*0.20-0.12,2.25,0.40),Vec3::new(side*0.20+0.12,2.45,0.47),z,0.0,[0.08,0.06,0.04]);
                b(Vec3::new(side*0.20-0.06,2.30,0.46),Vec3::new(side*0.20+0.06,2.40,0.49),z,0.0,eye);
            }
            b(Vec3::new(-0.18,1.95,0.42),Vec3::new(0.18,2.03,0.47),z,0.0,[0.08,0.06,0.04]);
            // Crown.
            for (x,y,zz,s) in [(0.0f32,3.05f32,0.0f32,0.65f32),(-0.55,2.85,0.10,0.45),(0.55,2.90,-0.10,0.45),(0.10,3.45,-0.15,0.42),(-0.25,3.30,0.30,0.35)] {
                b(Vec3::new(x-s,y-s*0.6,zz-s),Vec3::new(x+s,y+s*0.6,zz+s),z,0.0,if (x+zz)>0.0 {leaf_l} else {leaf});
            }
            for (x,zz) in [(-0.40f32,0.45f32),(0.35,0.40),(0.0,0.60)] {
                b(Vec3::new(x-0.07,3.0,zz),Vec3::new(x+0.07,3.14,zz+0.10),z,0.0,[0.95,0.45,0.70]);
            }
            for side in [-1.0f32,1.0] {
                // Branch arm with twigs and a leaf tuft at the hand.
                let sh=Vec3::new(side*0.62,2.50,0.0);
                let arm=raise-swing*side*0.7;
                b(Vec3::new(side*0.62-0.13,1.05,-0.13),Vec3::new(side*0.62+0.13,2.55,0.13),sh,arm,bark);
                b(Vec3::new(side*0.62+side*0.10,1.55,-0.06),Vec3::new(side*0.62+side*0.40,1.65,0.06),sh,arm,bark_d);
                b(Vec3::new(side*0.62-0.28,0.75,-0.28),Vec3::new(side*0.62+0.28,1.15,0.28),sh,arm,leaf);
            }
        }
        BossKind::Ember => {
            // A floating molten heart: a glowing core inside cracked basalt plates, rock
            // shards circling it, flames on top. No legs: it hovers and bobs.
            let bob=(anim*0.8).sin()*0.12;
            let core=[1.0,0.48+0.12*glow_k,0.10];
            let hot=[1.0,0.85,0.30];
            let (rock,rock_d)=([0.22,0.18,0.17],[0.13,0.11,0.11]);
            let y0=1.0+bob;
            b(Vec3::new(-0.58,y0+0.30,-0.58),Vec3::new(0.58,y0+1.50,0.58),z,0.0,core);
            b(Vec3::new(-0.30,y0+0.60,-0.30),Vec3::new(0.30,y0+1.20,0.30),z,0.0,hot);
            // Basalt shell broken into corner chunks: wide glowing cracks between them show the
            // core on every side.
            for sx in [-1.0f32,1.0] {
                for sz in [-1.0f32,1.0] {
                    for (y_lo,y_hi,c) in [(0.20f32,0.75f32,rock),(1.05,1.60,rock_d)] {
                        b(Vec3::new(sx*0.18,y0+y_lo,sz*0.18).min(Vec3::new(sx*0.66,y0+y_hi,sz*0.66)),Vec3::new(sx*0.18,y0+y_lo,sz*0.18).max(Vec3::new(sx*0.66,y0+y_hi,sz*0.66)),z,0.0,c);
                    }
                }
            }
            // Eyes in the front gap, a slit mouth of fire.
            for side in [-1.0f32,1.0] {b(Vec3::new(side*0.09-0.06,y0+0.82,0.66),Vec3::new(side*0.09+0.06,y0+0.98,0.70),z,0.0,[0.15,0.05,0.02]);}
            // Flames on top.
            for (x,zz,h) in [(0.0f32,0.0f32,0.55f32),(-0.28,0.15,0.35),(0.30,-0.12,0.40),(0.10,0.30,0.30)] {
                let fl=h*(0.85+0.15*(anim*3.0+x*7.0).sin());
                b(Vec3::new(x-0.10,y0+1.55,zz-0.10),Vec3::new(x+0.10,y0+1.55+fl,zz+0.10),z,0.0,if h>0.4 {core} else {hot});
            }
            // Rock shards circling (faster while casting).
            let spin=anim*if casting {1.6} else {0.6};
            for k in 0..5 {
                let a=spin+k as f32*std::f32::consts::TAU/5.0;
                let (sx,sz)=(a.cos()*1.25,a.sin()*1.25);
                let y=y0+0.6+(a*2.0).sin()*0.25;
                b(Vec3::new(sx-0.15,y,sz-0.15),Vec3::new(sx+0.15,y+0.30,sz+0.15),z,0.0,if k%2==0 {rock} else {rock_d});
            }
            // Short stubby arms that lift when it casts.
            for side in [-1.0f32,1.0] {
                let sh=Vec3::new(side*0.80,y0+1.25,0.0);
                b(Vec3::new(side*0.80-0.15,y0+0.55,-0.15),Vec3::new(side*0.80+0.15,y0+1.30,0.15),sh,raise*0.7,rock);
                b(Vec3::new(side*0.80-0.12,y0+0.45,-0.12),Vec3::new(side*0.80+0.12,y0+0.60,0.12),sh,raise*0.7,core);
            }
        }
    }
}


/// «Механика»: the spinning shafts, cogwheels, water wheels and cranks, as boxes turned about
/// their axis. `parts`: centre, axis, two directions across it, kind (0 shaft, 1 cog, 2 wheel,
/// 3 crank) and the angle.
pub fn build_kinetic(parts: &[(DVec3, DVec3, DVec3, DVec3, u8, f32)], eye: DVec3) -> Vec<ModelVertex> {
    const STEEL: [f32; 3] = [0.62, 0.64, 0.68];
    const STEEL_D: [f32; 3] = [0.45, 0.47, 0.52];
    const WOOD: [f32; 3] = [0.60, 0.44, 0.26];
    const WOOD_D: [f32; 3] = [0.42, 0.30, 0.17];
    let mut v = Vec::new();
    for &(c, ea, eu, ev, kind, angle) in parts {
        let (sn, cs) = angle.sin_cos();
        // Local (x along the axis, y and z across it, turned by the angle) → world.
        let to_world = |x: f32, y: f32, z: f32| {
            let (y2, z2) = (y * cs - z * sn, y * sn + z * cs);
            c + ea * x as f64 + eu * y2 as f64 + ev * z2 as f64
        };
        let mut cuboid = |lo: [f32; 3], hi: [f32; 3], col: [f32; 3]| {
            let p = |i: usize| {
                let x = if i & 1 == 0 { lo[0] } else { hi[0] };
                let y = if i & 2 == 0 { lo[1] } else { hi[1] };
                let z = if i & 4 == 0 { lo[2] } else { hi[2] };
                to_world(x, y, z)
            };
            let mid = to_world((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5);
            for quad in [[0usize, 2, 6, 4], [1, 5, 7, 3], [0, 4, 5, 1], [2, 3, 7, 6], [0, 1, 3, 2], [4, 6, 7, 5]] {
                let q = quad.map(p);
                let fc = (q[0] + q[1] + q[2] + q[3]) / 4.0;
                let n = (fc - mid).normalize_or_zero();
                for tri in [[0usize, 1, 2], [0, 2, 3]] {
                    let (a, b, cc) = (q[tri[0]], q[tri[1]], q[tri[2]]);
                    // Outward winding for this renderer: (c - a) × (b - a) points out.
                    let (b, cc) = if (cc - a).cross(b - a).dot(n) < 0.0 { (cc, b) } else { (b, cc) };
                    for pt in [a, b, cc] {
                        v.push(ModelVertex { pos: (pt - eye).as_vec3().to_array(), normal: n.as_vec3().to_array(), color: col, uv: [0.0; 2], tex: 0.0 });
                    }
                }
            }
        };
        match kind {
            0 => {
                cuboid([-0.5, -0.11, -0.11], [0.5, 0.11, 0.11], STEEL);
                for x in [-0.3f32, 0.1] {
                    cuboid([x, -0.125, -0.125], [x + 0.05, 0.125, 0.125], STEEL_D);
                }
            }
            1 => {
                cuboid([-0.5, -0.1, -0.1], [0.5, 0.1, 0.1], STEEL);
                cuboid([-0.11, -0.34, -0.34], [0.11, 0.34, 0.34], WOOD);
                for t in 0..8 {
                    let a = t as f32 * std::f32::consts::FRAC_PI_4;
                    let (s, k) = a.sin_cos();
                    let (y, z) = (k * 0.40, s * 0.40);
                    let w = 0.06;
                    // A tooth: a small box around (y, z), not turned (good enough at this size).
                    cuboid([-0.10, y - w, z - w], [0.10, y + w, z + w], WOOD_D);
                }
            }
            2 => {
                cuboid([-0.5, -0.1, -0.1], [0.5, 0.1, 0.1], STEEL);
                cuboid([-0.18, -0.16, -0.16], [0.18, 0.16, 0.16], WOOD_D);
                for t in 0..8 {
                    let a = t as f32 * std::f32::consts::FRAC_PI_4;
                    let (s, k) = a.sin_cos();
                    // Paddles: planks running out from the hub.
                    for r in [0.24f32, 0.36, 0.46] {
                        let (y, z) = (k * r, s * r);
                        cuboid([-0.22, y - 0.05, z - 0.05], [0.22, y + 0.05, z + 0.05], if r > 0.4 { WOOD } else { WOOD_D });
                    }
                }
            }
            _ => {
                // Crank: a stub under it, an arm out to the side and a handle up.
                cuboid([-0.5, -0.08, -0.08], [0.0, 0.08, 0.08], STEEL);
                cuboid([-0.05, -0.06, -0.06], [0.05, 0.36, 0.06], WOOD);
                cuboid([-0.05, 0.30, -0.06], [0.35, 0.40, 0.06], WOOD_D);
            }
        }
    }
    v
}
