//! Resource packs, like in the classic block game.
//!
//! * Built-in packs: the original procedural art at 16× and smooth HD versions of it
//!   (32× "HD", 64× "Full HD", 128× "4K") made with Scale2x upscaling plus fine grain.
//! * User packs: a folder or a `.zip` in a `resourcepacks` directory with sprites in
//!   `blocks/`, `items/`, `entities/`, `gui/`, `particles/` (`<sprite>.png`, any size) and an
//!   optional `pack.json` `{"name": "...", "description": "..."}`. A user pack is drawn on top
//!   of the chosen built-in base, so it only has to contain the textures it changes.

use crate::atlas_manager::AtlasKind;
use crate::image::Image;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Built-in packs: (id, name, upscale factor).
pub const BUILTIN: [(&str, &str, u32); 8] = [
    ("builtin:toon16", "Мультяшный Лайт 16× (основной)", 1),
    ("builtin:toon32", "Мультяшный 32×", 2),
    (TOON_PACK, "Мультяшный HD 64×", 4),
    ("builtin:toon128", "Мультяшный 4K 128×", 8),
    ("builtin:16", "Старый пак 16×", 1),
    ("builtin:32", "Старый пак HD 32×", 2),
    ("builtin:64", "Старый пак Full HD 64×", 4),
    ("builtin:128", "Старый пак 4K 128×", 8),
];

/// The game's main look: the light 16× cartoon pack (fast on every phone).
pub const DEFAULT_PACK: &str = "builtin:toon16";

/// The cartoon pack: smooth juicy colours, soft blobs instead of noise, bevelled block edges.
pub const TOON_PACK: &str = "builtin:toon";

/// Any of the cartoon packs (the main one and its other resolutions).
pub fn is_toon(id: &str) -> bool {
    id.starts_with(TOON_PACK)
}

#[derive(Clone, Debug, PartialEq)]
pub struct PackInfo {
    /// `builtin:<size>` or `user:<file name>`.
    pub id: String,
    pub name: String,
    pub description: String,
    /// Folder or zip of a user pack.
    pub path: Option<PathBuf>,
}

/// All packs: built-ins first, then user packs found in `dirs` (sorted by name).
pub fn list(dirs: &[PathBuf]) -> Vec<PackInfo> {
    let mut v: Vec<PackInfo> = BUILTIN
        .iter()
        .map(|(id, name, f)| PackInfo { id: id.to_string(), name: name.to_string(), description: if *id == DEFAULT_PACK { "Основной стиль игры: мультяшные блоки 16×16, лёгкие шейдеры, быстро на любом телефоне".into() } else if *id == TOON_PACK { "Мультяшный в HD: мягче края и детали".into() } else if *id == "builtin:toon32" { "Мультяшный чуть чётче основного".into() } else if *id == "builtin:toon128" { "Мультяшный в высоком разрешении (128×128 на блок): чёткие края, для сильных устройств".into() } else if *f == 1 { "Дополнение: старые пиксельные текстуры".into() } else { format!("Дополнение: старые текстуры, {}×{} пикселей на блок", 16 * f, 16 * f) }, path: None })
        .collect();
    let mut users = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            let file = e.file_name().to_string_lossy().to_string();
            let is_zip = path.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip"));
            // Legacy loose overrides (`resourcepacks/blocks/...`) are not a pack.
            if !(path.is_dir() || is_zip) || AtlasKind::ALL.iter().any(|k| k.folder() == file) || file.starts_with('.') {
                continue;
            }
            let (name, description) = read_meta(&path).unwrap_or_else(|| (file.trim_end_matches(".zip").to_string(), String::new()));
            users.push(PackInfo { id: format!("user:{file}"), name, description, path: Some(path) });
        }
    }
    users.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    v.extend(users);
    v
}

/// Upscale factor of the built-in base for `id` (user packs sit on the 16× base).
pub fn base_scale(id: &str) -> u32 {
    BUILTIN.iter().find(|(i, ..)| *i == id).map(|b| b.2).unwrap_or(1)
}

fn read_meta(path: &Path) -> Option<(String, String)> {
    let text = read_file(path, "pack.json")?;
    let v: serde_json::Value = serde_json::from_slice(&text).ok()?;
    let name = v.get("name")?.as_str()?.to_string();
    let desc = v.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
    Some((name, desc))
}

/// Reads `rel` from a pack folder or zip (also accepts a single top-level folder inside the zip).
fn read_file(pack: &Path, rel: &str) -> Option<Vec<u8>> {
    if pack.is_dir() {
        return std::fs::read(pack.join(rel)).ok();
    }
    let f = std::fs::File::open(pack).ok()?;
    let mut z = zip::ZipArchive::new(f).ok()?;
    let name = (0..z.len()).filter_map(|i| z.by_index(i).ok().map(|e| e.name().to_string())).find(|n| n == rel || n.ends_with(&format!("/{rel}")))?;
    let mut e = z.by_name(&name).ok()?;
    let mut buf = Vec::new();
    e.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Replaces sprites with the pack's PNGs where present.
pub fn apply_user_pack(pack: &Path, kind: AtlasKind, sprites: &mut [(String, Image)]) -> Vec<String> {
    let mut applied = Vec::new();
    for (name, img) in sprites.iter_mut() {
        if let Some(bytes) = read_file(pack, &format!("{}/{name}.png", kind.folder())) {
            match Image::load_png_bytes(&bytes) {
                Ok(o) => { *img = o; applied.push(name.clone()); },
                Err(e) => log::warn!("pack {}: {name}: {e}", pack.display()),
            }
        }
    }
    applied
}

/// The built-in look: rich, bright colours (saturation +25 % and more light) over the procedural art.
pub fn vibrant(img: &mut Image) {
    for px in img.pixels.chunks_exact_mut(4) {
        if px[3] == 0 {
            continue;
        }
        let (r, g, b) = (px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0);
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        let sat = |c: f32| l + (c - l) * 1.18;
        let tone = |c: f32| (((sat(c) - 0.5) * 1.04 + 0.5) * 1.03).clamp(0.0, 1.0);
        px[0] = (tone(r) * 255.0) as u8;
        px[1] = (tone(g) * 255.0) as u8;
        px[2] = (tone(b) * 255.0) as u8;
    }
}

/// Low-poly cartoon version of a 16× block sprite at `size` pixels.
///
/// * Colours: juicy and light, the fine noise flattened into a few clean patches.
/// * Shapes that matter (mortar, planks seams, ore gems, door windows) stay crisp: pixels that
///   differ clearly from their surroundings keep their own (posterized) colour, sharp-edged.
/// * No low-poly facets: a cartoon cushion instead (ink outline, light rim top-left, shade
///   bottom-right, one texel wide), so blocks read like crisp cartoon pixel art.
/// * A soft bevel on the rim (`bevel` 0 = none, for liquids; natural ground gets little).
///
/// Sprites with transparent pixels (plants, torches, glass) keep their exact shape, upscaled
/// crisply with the same colours. `seed` varies the facets per sprite. Seamless where the
/// source is (facets and bevel follow the tile edges).
pub fn toon(src: &Image, size: u32, bevel: f32) -> Image {
    toon_seeded(src, size, bevel, 0)
}

/// `toon` with a facet seed (per sprite, e.g. a hash of its name).
pub fn toon_seeded(src: &Image, size: u32, bevel: f32, seed: u32) -> Image {
    let (w, h) = (src.width as i32, src.height as i32);
    let opaque = !src.pixels.chunks_exact(4).any(|p| p[3] < 255);
    let get = |x: i32, y: i32| -> [f32; 4] {
        let p = src.get(x.rem_euclid(w) as u32, y.rem_euclid(h) as u32);
        [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0]
    };
    let lum = |c: &[f32; 4]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    let juicy = |c: [f32; 4]| -> [f32; 4] {
        let l = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
        let f = |v: f32| ((l + (v - l) * 1.25).max(0.0) * 0.92 + 0.07).min(1.0);
        [f(c[0]), f(c[1]), f(c[2]), c[3]]
    };
    let n = size as i32;
    let mut out = Image::new(size, size);
    if !opaque {
        // Cut-out sprites: exact shape, crisp pixels, juicy colours. Natural ones (leaves,
        // grass) get an almost solid colour, like low-poly foliage; flowers keep their colours.
        let mut m = [0.0f32; 3];
        let mut k = 0.0;
        for y in 0..h {
            for x in 0..w {
                let o = get(x, y);
                if o[3] >= 0.5 {
                    for c in 0..3 {
                        m[c] += o[c];
                    }
                    k += 1.0;
                }
            }
        }
        for c in &mut m {
            *c /= f32::max(k, 1.0);
        }
        let keep = if bevel < 0.5 { 0.3 } else { 1.0 };
        for y in 0..n {
            for x in 0..n {
                let o = get(x * w / n, y * h / n);
                if o[3] == 0.0 {
                    out.set(x as u32, y as u32, [0, 0, 0, 0]);
                    continue;
                }
                let o = [m[0] + (o[0] - m[0]) * keep, m[1] + (o[1] - m[1]) * keep, m[2] + (o[2] - m[2]) * keep, o[3]];
                let c = juicy(o);
                let px = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                out.set(x as u32, y as u32, [px(c[0]), px(c[1]), px(c[2]), px(o[3])]);
            }
        }
        return out;
    }
    // Per source pixel: flat (calmed) colour, or its own colour where it is a real feature.
    let mut mean = [0.0f32; 4];
    for y in 0..h {
        for x in 0..w {
            let o = get(x, y);
            for c in 0..3 {
                mean[c] += o[c] / (w * h) as f32;
            }
        }
    }
    mean[3] = 1.0;
    let mean_l = lum(&mean);
    let mut flat = vec![[0.0f32; 4]; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let o = get(x, y);
            let mut b = [0.0f32; 4];
            let mut wsum = 0.0;
            for dy in -2..=2i32 {
                for dx in -2..=2i32 {
                    let k = 1.0 / (1.0 + (dx * dx + dy * dy) as f32);
                    let q = get(x + dx, y + dy);
                    for c in 0..3 {
                        b[c] += q[c] * k;
                    }
                    wsum += k;
                }
            }
            for c in b.iter_mut().take(3) {
                *c /= wsum;
            }
            // Roblox low-poly look: nearly solid colour; only strong shapes (ore gems, windows,
            // mortar, seams) survive, and those a little softened toward the block's colour.
            // Natural ground and foliage (faint bevel) is calmed much harder: big soft patches
            // of colour, no pixel noise; built blocks keep their mortar and seams.
            let natural = bevel < 0.3;
            // Rock and logs (bevel between): speckles go, ore gems and bark lines stay.
            let rock = !natural && bevel < 0.95;
            let (thr, dev, calm) = if natural { (0.12, 0.30, 0.75) } else if rock { (0.10, 0.24, 0.75) } else { (0.08, 0.24, 0.85) };
            let chroma = (0..3).map(|i| (o[i] - b[i]).abs()).fold(0.0f32, f32::max);
            let feature = chroma > 0.16 || (lum(&o) - lum(&b)).abs() > thr || (lum(&o) - mean_l).abs() > dev;
            let base = if feature {
                [mean[0] + (o[0] - mean[0]) * 0.85, mean[1] + (o[1] - mean[1]) * 0.85, mean[2] + (o[2] - mean[2]) * 0.85, 1.0]
            } else {
                // Mostly the texel's own colour (pixel grain stays), a little of the blur.
                let q = [o[0] * 0.65 + b[0] * 0.35, o[1] * 0.65 + b[1] * 0.35, o[2] * 0.65 + b[2] * 0.35];
                [mean[0] + (q[0] - mean[0]) * calm, mean[1] + (q[1] - mean[1]) * calm, mean[2] + (q[2] - mean[2]) * calm, 1.0]
            };
            // Posterize around the mean: a few clean colour steps.
            let step = if feature { 0.10 } else { 0.07 };
            let mut c = [0.0f32; 4];
            for i in 0..3 {
                c[i] = mean[i] + ((base[i] - mean[i]) / step).round() * step;
            }
            c[3] = 1.0;
            flat[(y * w + x) as usize] = juicy(c);
        }
    }
    let _ = seed;
    for y in 0..n {
        for x in 0..n {
            let mut c = flat[((y * h / n) * w + x * w / n) as usize];
            // Natural ground and foliage stay seamless: facets or a bevel repeat on every
            // block and make a visible grid over a meadow.
            if bevel >= 0.3 {
                // Pixel-art block edge (crisp, one texel wide, no smooth plastic shading):
                // a dark ink line on built blocks, then a light texel row on the top and left
                // and a dark one on the bottom and right.
                let texel = n as f32 / w as f32;
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let (l_, t_, r_, b_) = (fx, fy, n as f32 - fx, n as f32 - fy);
                let edge = l_.min(t_).min(r_).min(b_);
                let ink = if bevel >= 1.0 { texel } else { 0.0 };
                if edge < ink {
                    for v in c.iter_mut().take(3) {
                        *v *= 0.58;
                    }
                } else if edge < ink + texel {
                    let lit = if l_.min(t_) <= r_.min(b_) { 0.16 * bevel } else { -0.16 * bevel };
                    for i in 0..3 {
                        c[i] = if lit > 0.0 { c[i] + (1.0 - c[i]) * lit } else { c[i] * (1.0 + lit) };
                    }
                }
            }
            let px = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            out.set(x as u32, y as u32, [px(c[0]), px(c[1]), px(c[2]), 255]);
        }
    }
    out
}

/// How strongly the cartoon pack bevels a block sprite: built blocks (bricks, planks, stone…)
/// get the full chunky bevel with an outline, natural ground and plants only a faint one so
/// terrain flows smoothly instead of looking like floor tiles, liquids none.
pub fn toon_bevel(name: &str) -> f32 {
    const NATURAL: [&str; 16] = ["grass", "dirt", "sand", "snow", "leaves", "gravel", "clay", "mud", "moss", "podzol", "mycel", "ice", "farmland", "path", "netherrack", "peat"];
    if matches!(name, "water" | "lava") || name.contains("water") || name.contains("lava") {
        0.0
    } else if name == "baseplate" {
        // The studio's base plate: a light pixel rim, no ink grid over the whole floor.
        0.5
    } else if name.contains("bricks") || name.contains("sandstone") {
        1.0
    } else if NATURAL.iter().any(|n| name.contains(n)) {
        0.12
    } else if name.contains("log") || name.contains("cactus") || name.contains("pumpkin") || name.contains("melon") {
        0.5
    } else if (name.contains("stone") && !name.contains("smooth") && !name.contains("polished") && !name.contains("ore")) || name.contains("cobble") || ["granite", "marble", "slate", "limestone", "basalt", "andesite", "obsidian"].iter().any(|r| name.contains(r)) {
        // Rock: the full chunky bevel, a little softer, no ink outline.
        0.9
    } else {
        1.0
    }
}

/// Height map of a block texture for the relief shaders (stored in the red channel, 0 = deep,
/// 255 = high): the brightness, softened a little and stretched to the full range per tile,
/// so mortar, cracks and gaps sink in and stones, bricks and planks stand out. Seamless.
/// Sprites with transparent pixels (glass, plants) stay flat (all high).
pub fn height_map(img: &Image) -> Image {
    let (w, h) = (img.width as i32, img.height as i32);
    let mut out = Image::new(img.width, img.height);
    if w < 2 || img.pixels.chunks_exact(4).any(|p| p[3] < 255) {
        for y in 0..img.height {
            for x in 0..img.width {
                out.set(x, y, [255, 255, 255, 255]);
            }
        }
        return out;
    }
    let lum = |x: i32, y: i32| {
        let p = img.get(x.rem_euclid(w) as u32, y.rem_euclid(h) as u32);
        (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0
    };
    let r = (w / 48).max(1);
    let mut hm = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0;
            let mut n = 0.0;
            for dy in -r..=r {
                for dx in -r..=r {
                    sum += lum(x + dx, y + dy);
                    n += 1.0;
                }
            }
            hm[(y * w + x) as usize] = sum / n;
        }
    }
    let (lo, hi) = hm.iter().fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    let span = (hi - lo).max(0.05);
    for y in 0..h {
        for x in 0..w {
            let v = ((hm[(y * w + x) as usize] - lo) / span).clamp(0.0, 1.0);
            // A gentle curve: most of the surface stays up, gaps fall off faster.
            let v = v.powf(0.7);
            let b = (v * 255.0) as u8;
            out.set(x as u32, y as u32, [b, b, b, 255]);
        }
    }
    out
}

/// Gives a flat block texture some depth: the brightness is read as a height map (softened
/// a little), and each pixel is lit as if the light came from the top-left, so stones bulge
/// and mortar, cracks and grain sink in. Wraps around the edges, so tiling stays seamless.
/// Sprites with transparent pixels (glass, plants, torches) are left alone.
pub fn emboss(img: &mut Image, strength: f32) {
    let (w, h) = (img.width as i32, img.height as i32);
    if w < 2 || img.pixels.chunks_exact(4).any(|p| p[3] < 255) {
        return;
    }
    let lum = |x: i32, y: i32| {
        let p = img.get(x.rem_euclid(w) as u32, y.rem_euclid(h) as u32);
        (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0
    };
    // Height: a small blur of the brightness (radius grows with the resolution).
    let r = (w / 64).max(1);
    let mut height = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0;
            let mut n = 0.0;
            for dy in -r..=r {
                for dx in -r..=r {
                    sum += lum(x + dx, y + dy);
                    n += 1.0;
                }
            }
            height[(y * w + x) as usize] = sum / n;
        }
    }
    let hgt = |x: i32, y: i32| height[(y.rem_euclid(h) * w + x.rem_euclid(w)) as usize];
    let step = r;
    let mut out = img.pixels.clone();
    for y in 0..h {
        for x in 0..w {
            // Slope towards the light (top-left) brightens, away from it darkens.
            let slope = (hgt(x - step, y - step) - hgt(x + step, y + step)) * 4.0;
            let k = (1.0 - slope * strength).clamp(0.6, 1.4);
            let i = ((y * w + x) * 4) as usize;
            for c in 0..3 {
                out[i + c] = (img.pixels[i + c] as f32 * k).clamp(0.0, 255.0) as u8;
            }
        }
    }
    img.pixels = out;
}

/// Smooth HD version of a pixel-art sprite: Scale2x applied `factor` times over, then a soft
/// deterministic grain so large surfaces do not look flat.
pub fn upscale(img: &Image, factor: u32) -> Image {
    let mut out = img.clone();
    let mut f = factor;
    while f > 1 {
        out = scale2x(&out);
        f /= 2;
    }
    if factor > 1 {
        grain(&mut out, factor);
    }
    out
}

fn scale2x(src: &Image) -> Image {
    let (w, h) = (src.width, src.height);
    let mut dst = Image::new(w * 2, h * 2);
    let px = |x: i64, y: i64| src.get(x.clamp(0, w as i64 - 1) as u32, y.clamp(0, h as i64 - 1) as u32);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let (a, b, c, d, e) = (px(x, y - 1), px(x - 1, y), px(x, y), px(x + 1, y), px(x, y + 1));
            let (mut e0, mut e1, mut e2, mut e3) = (c, c, c, c);
            if a != e && b != d {
                if b == a {
                    e0 = b;
                }
                if a == d {
                    e1 = d;
                }
                if b == e {
                    e2 = b;
                }
                if e == d {
                    e3 = d;
                }
            }
            let (dx, dy) = (x as u32 * 2, y as u32 * 2);
            dst.set(dx, dy, e0);
            dst.set(dx + 1, dy, e1);
            dst.set(dx, dy + 1, e2);
            dst.set(dx + 1, dy + 1, e3);
        }
    }
    dst
}

fn grain(img: &mut Image, factor: u32) {
    let amp = 6.0 + factor as f32;
    for y in 0..img.height {
        for x in 0..img.width {
            let mut p = img.get(x, y);
            if p[3] == 0 {
                continue;
            }
            let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
            h ^= h >> 15;
            h = h.wrapping_mul(0x2C1B_3C6D);
            h ^= h >> 13;
            let n = ((h & 0xff) as f32 / 255.0 - 0.5) * amp;
            for c in p.iter_mut().take(3) {
                *c = (*c as f32 + n).clamp(0.0, 255.0) as u8;
            }
            img.set(x, y, p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cartoon_keeps_translucent_alpha_at_every_resolution() {
        let mut src = Image::new(16, 16);
        for y in 0..16 { for x in 0..16 {
            src.set(x, y, [40, 110, 190, if x == 0 { 0 } else if x == 1 { 64 } else { 184 }]);
        }}
        for size in [16, 32, 64, 128] {
            let out = toon(&src, size, 0.0);
            for y in 0..size { for x in 0..size {
                assert_eq!(out.get(x, y)[3], src.get(x * 16 / size, y * 16 / size)[3]);
            }}
        }
    }

    #[test]
    fn upscale_sizes_and_builtins_listed() {
        let mut img = Image::new(16, 16);
        img.set(3, 3, [255, 0, 0, 255]);
        for (_, _, f) in BUILTIN {
            let o = upscale(&img, f);
            assert_eq!((o.width, o.height), (16 * f, 16 * f));
        }
        let l = list(&[]);
        assert_eq!(l.len(), BUILTIN.len());
        assert_eq!(base_scale("builtin:64"), 4);
        assert_eq!(base_scale("user:x"), 1);
    }

    #[test]
    fn folder_pack_overrides_sprites() {
        let dir = std::env::temp_dir().join("wolfcraft_pack_test");
        let pack = dir.join("MyPack");
        let mut red = Image::new(32, 32);
        red.set(0, 0, [255, 0, 0, 255]);
        red.save_png(&pack.join("blocks").join("stone.png")).unwrap();
        std::fs::write(pack.join("pack.json"), r#"{"name":"Мой пак","description":"тест"}"#).unwrap();
        let l = list(&[dir.clone()]);
        let p = l.iter().find(|p| p.id == "user:MyPack").expect("pack listed");
        assert_eq!(p.name, "Мой пак");
        let mut sprites = vec![("stone".to_string(), Image::new(16, 16))];
        apply_user_pack(p.path.as_ref().unwrap(), AtlasKind::Blocks, &mut sprites);
        assert_eq!(sprites[0].1.width, 32);
    }
}

