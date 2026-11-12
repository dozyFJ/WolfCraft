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
pub const BUILTIN: [(&str, &str, u32); 5] = [
    (TOON_PACK, "Мультяшный 64×", 4),
    ("builtin:16", "Классика 16×", 1),
    ("builtin:32", "Гладкий HD 32×", 2),
    ("builtin:64", "Гладкий Full HD 64×", 4),
    ("builtin:128", "Гладкий 4K 128×", 8),
];

pub const DEFAULT_PACK: &str = TOON_PACK;

/// The cartoon pack: smooth juicy colours, soft blobs instead of noise, bevelled block edges.
pub const TOON_PACK: &str = "builtin:toon";

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
        .map(|(id, name, f)| PackInfo { id: id.to_string(), name: name.to_string(), description: if *id == TOON_PACK { "Яркий мультяшный стиль, как в топовых симуляторах: сочные гладкие цвета и скруглённые края блоков".into() } else if *f == 1 { "Стандартные текстуры игры".into() } else { format!("Встроенный, {}×{} пикселей на блок", 16 * f, 16 * f) }, path: None })
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
pub fn apply_user_pack(pack: &Path, kind: AtlasKind, sprites: &mut [(String, Image)]) {
    for (name, img) in sprites.iter_mut() {
        if let Some(bytes) = read_file(pack, &format!("{}/{name}.png", kind.folder())) {
            match Image::load_png_bytes(&bytes) {
                Ok(o) => *img = o,
                Err(e) => log::warn!("pack {}: {name}: {e}", pack.display()),
            }
        }
    }
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

/// Cartoon version of a 16× block sprite at `size` pixels: the noise is calmed into soft
/// blobs with clean flat colour steps (posterized), colours become juicy and light, and solid
/// blocks get a bevel (light top-left rim, darker bottom-right) with a thin soft outline, so
/// each block reads as a chunky toy cube. Sprites with transparent pixels keep their shape
/// (no bevel) but get the same colours. Seamless where the source is.
pub fn toon(src: &Image, size: u32, bevel: f32) -> Image {
    let (w, h) = (src.width as i32, src.height as i32);
    let opaque = !src.pixels.chunks_exact(4).any(|p| p[3] < 255);
    // Natural ground is calmed the most (smooth, like a sim's terrain); built blocks keep more detail.
    let detail = if bevel >= 1.0 { 0.35 } else { 0.12 };
    let get = |x: i32, y: i32| -> [f32; 4] {
        let p = src.get(x.rem_euclid(w) as u32, y.rem_euclid(h) as u32);
        [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0]
    };
    // Soft version of the source: a 3×3 blur (wrapping) mixed with a calmed copy of the detail.
    let mut soft = vec![[0.0f32; 4]; (w * h) as usize];
    let mut mean = [0.0f32; 3];
    for y in 0..h {
        for x in 0..w {
            let o = get(x, y);
            let mut s = [0.0f32; 3];
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let q = get(x + dx, y + dy);
                    let k = if dx == 0 && dy == 0 { 4.0 } else if dx == 0 || dy == 0 { 2.0 } else { 1.0 };
                    for c in 0..3 {
                        s[c] += q[c] * k;
                    }
                }
            }
            let mut v = [0.0f32; 4];
            for c in 0..3 {
                let b = s[c] / 16.0;
                v[c] = b + (o[c] - b) * detail;
                mean[c] += v[c];
            }
            v[3] = o[3];
            soft[(y * w + x) as usize] = v;
        }
    }
    for c in &mut mean {
        *c /= (w * h) as f32;
    }
    let at = |x: i32, y: i32| soft[(y.rem_euclid(h) * w + x.rem_euclid(w)) as usize];
    let n = size as i32;
    let mut out = Image::new(size, size);
    let f = w as f32 / n as f32;
    let band = (n as f32 * 0.10).max(2.0);
    let line = (n as f32 / 48.0).max(1.0);
    for y in 0..n {
        for x in 0..n {
            // Sample position in source pixels (centres), bilinear with wrap for opaque tiles,
            // nearest for cut-out sprites so their shape stays crisp.
            let sx = (x as f32 + 0.5) * f - 0.5;
            let sy = (y as f32 + 0.5) * f - 0.5;
            let mut c = if opaque {
                let (x0, y0) = (sx.floor() as i32, sy.floor() as i32);
                let (tx, ty) = (sx - x0 as f32, sy - y0 as f32);
                // Smoothstep weights make round blobs instead of diamond-shaped bilinear ones.
                let (tx, ty) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
                let (a, b, cc, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
                let mut v = [0.0; 4];
                for i in 0..4 {
                    v[i] = (a[i] * (1.0 - tx) + b[i] * tx) * (1.0 - ty) + (cc[i] * (1.0 - tx) + d[i] * tx) * ty;
                }
                v
            } else {
                let p = src.get(((x as f32 + 0.5) * f) as u32 % src.width, ((y as f32 + 0.5) * f) as u32 % src.height);
                let o = [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0];
                let b = at(((x as f32 + 0.5) * f) as i32, ((y as f32 + 0.5) * f) as i32);
                [b[0] * 0.7 + o[0] * 0.3, b[1] * 0.7 + o[1] * 0.3, b[2] * 0.7 + o[2] * 0.3, o[3]]
            };
            if c[3] < 0.02 {
                out.set(x as u32, y as u32, [0, 0, 0, 0]);
                continue;
            }
            // Flat colour steps around the tile's mean: clean cartoon patches.
            if opaque {
                let q = 0.075;
                for i in 0..3 {
                    c[i] = mean[i] + ((c[i] - mean[i]) / q).round() * q;
                }
            }
            // Juicy and light: strong saturation, shadows lifted.
            let l = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
            for i in 0..3 {
                c[i] = (l + (c[i] - l) * 1.55).max(0.0);
                c[i] = c[i] * 0.86 + 0.12;
            }
            if bevel > 0.0 && opaque {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let (l_, t_, r_, b_) = (fx, fy, n as f32 - fx, n as f32 - fy);
                let edge = l_.min(t_).min(r_).min(b_);
                if edge < band {
                    let k = 1.0 - edge / band;
                    let k = k * k;
                    // Light from the top-left: those rims shine, the others fall into shade.
                    let lit = if l_.min(t_) <= r_.min(b_) { 0.20 * bevel } else { -0.20 * bevel };
                    for i in 0..3 {
                        c[i] = if lit > 0.0 { c[i] + (1.0 - c[i]) * lit * k } else { c[i] * (1.0 + lit * k) };
                    }
                }
                if edge < line && bevel >= 1.0 {
                    for i in 0..3 {
                        c[i] *= 0.72;
                    }
                }
            }
            let px = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            out.set(x as u32, y as u32, [px(c[0]), px(c[1]), px(c[2]), px(c[3])]);
        }
    }
    out
}

/// Cartoon items: a dark outline around the icon's shape (like a sticker), drawn on the
/// transparent pixels touching it, in a deep shade of the neighbouring colour.
pub fn sticker_outline(img: &mut Image) {
    let (w, h) = (img.width as i32, img.height as i32);
    let t = (w / 32).max(1);
    let src = img.clone();
    for y in 0..h {
        for x in 0..w {
            if src.get(x as u32, y as u32)[3] > 40 {
                continue;
            }
            let mut best: Option<[u8; 4]> = None;
            'n: for dy in -t..=t {
                for dx in -t..=t {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let p = src.get(nx as u32, ny as u32);
                    if p[3] > 200 {
                        best = Some(p);
                        break 'n;
                    }
                }
            }
            if let Some(p) = best {
                img.set(x as u32, y as u32, [(p[0] as f32 * 0.28) as u8, (p[1] as f32 * 0.28) as u8, (p[2] as f32 * 0.32) as u8, 255]);
            }
        }
    }
}

/// How strongly the cartoon pack bevels a block sprite: built blocks (bricks, planks, stone…)
/// get the full chunky bevel with an outline, natural ground and plants only a faint one so
/// terrain flows smoothly instead of looking like floor tiles, liquids none.
pub fn toon_bevel(name: &str) -> f32 {
    const NATURAL: [&str; 16] = ["grass", "dirt", "sand", "snow", "leaves", "gravel", "clay", "mud", "moss", "podzol", "mycel", "ice", "farmland", "path", "netherrack", "peat"];
    if matches!(name, "water" | "lava") || name.contains("water") || name.contains("lava") {
        0.0
    } else if name.contains("bricks") || name.contains("sandstone") {
        1.0
    } else if NATURAL.iter().any(|n| name.contains(n)) {
        0.18
    } else if name.contains("log") || name.contains("cactus") || name.contains("pumpkin") || name.contains("melon") {
        0.5
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
