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
pub const BUILTIN: [(&str, &str, u32); 4] = [
    ("builtin:16", "Классика 16×", 1),
    ("builtin:32", "Гладкий HD 32×", 2),
    ("builtin:64", "Гладкий Full HD 64×", 4),
    ("builtin:128", "Гладкий 4K 128×", 8),
];

pub const DEFAULT_PACK: &str = "builtin:16";

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
        .map(|(id, name, f)| PackInfo { id: id.to_string(), name: name.to_string(), description: if *f == 1 { "Стандартные текстуры игры".into() } else { format!("Встроенный, {}×{} пикселей на блок", 16 * f, 16 * f) }, path: None })
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

/// The built-in look: richer, brighter colours (saturation +8 % and a little more light) over the procedural art.
pub fn vibrant(img: &mut Image) {
    for px in img.pixels.chunks_exact_mut(4) {
        if px[3] == 0 {
            continue;
        }
        let (r, g, b) = (px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0);
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        let sat = |c: f32| l + (c - l) * 1.08;
        let tone = |c: f32| (((sat(c) - 0.5) * 1.0 + 0.5) * 1.04).clamp(0.0, 1.0);
        px[0] = (tone(r) * 255.0) as u8;
        px[1] = (tone(g) * 255.0) as u8;
        px[2] = (tone(b) * 255.0) as u8;
    }
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
