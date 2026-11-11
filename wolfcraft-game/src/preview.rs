//! Planet globe preview (create-world screen and world list). Rendered from the real generator:
//! every pixel samples `Planet::column`, so the preview shows exactly the continents,
//! oceans, climate zones and mountains the world will have.

use egui::{Color32, ColorImage};
use glam::DVec3;
use rayon::prelude::*;
use wolfcraft_engine::biome::Biome;
use wolfcraft_engine::config::WorldConfig;
use wolfcraft_engine::planet::{Column, Planet};

pub fn biome_color(c: &Column) -> [f32; 3] {
    let e = c.elev;
    let base: [f32; 3] = match c.biome {
        Biome::Ocean => [0.10, 0.30, 0.65],
        Biome::DeepOcean => [0.05, 0.16, 0.45],
        Biome::FrozenOcean => [0.62, 0.78, 0.92],
        Biome::Beach => [0.86, 0.80, 0.58],
        Biome::Plains => [0.45, 0.70, 0.30],
        Biome::Forest => [0.20, 0.50, 0.20],
        Biome::BirchForest => [0.35, 0.60, 0.28],
        Biome::Jungle => [0.10, 0.45, 0.15],
        Biome::Taiga => [0.18, 0.40, 0.30],
        Biome::Tundra => [0.62, 0.66, 0.55],
        Biome::SnowyPlains => [0.92, 0.95, 0.98],
        Biome::Desert => [0.88, 0.78, 0.50],
        Biome::Badlands => [0.76, 0.42, 0.22],
        Biome::Savanna => [0.68, 0.64, 0.30],
        Biome::Swamp => [0.28, 0.40, 0.25],
        Biome::Mountains => [0.50, 0.50, 0.52],
        Biome::SnowyPeaks => [0.96, 0.97, 1.0],
        Biome::Volcano => [0.28, 0.20, 0.20],
    };
    // Subtle height shading on land, depth shading on water.
    let k = if e >= 0.0 { 1.0 + (e / 220.0).min(0.35) } else { 1.0 + (e / 120.0).max(-0.5) };
    if c.river && e < 0.0 {
        return [0.2, 0.45, 0.8];
    }
    [base[0] * k, base[1] * k, base[2] * k]
}

/// Renders a lit globe of `size`×`size` pixels. The view is centred on the first continent.
pub fn render_globe(cfg: &WorldConfig, size: usize) -> ColorImage {
    if cfg.shape == wolfcraft_engine::config::PlanetShape::Cube {
        return render_cube(cfg, size);
    }
    let planet = Planet::new(cfg);
    let z_axis = planet.continents.first().map(|(c, _)| *c).unwrap_or(DVec3::new(0.3, 0.2, 0.93).normalize());
    let up_hint = if z_axis.y.abs() > 0.95 { DVec3::X } else { DVec3::Y };
    let x_axis = up_hint.cross(z_axis).normalize();
    let y_axis = z_axis.cross(x_axis);
    let light = DVec3::new(-0.45, 0.55, 0.70).normalize();
    let n = size as f64;

    let pixels: Vec<Color32> = (0..size * size)
        .into_par_iter()
        .map(|i| {
            let (px, py) = ((i % size) as f64, (i / size) as f64);
            let x = (px + 0.5) / n * 2.0 - 1.0;
            let y = 1.0 - (py + 0.5) / n * 2.0;
            let r2 = x * x + y * y;
            let rim = 0.92;
            let rr = r2.sqrt() / rim;
            if rr > 1.0 {
                // Background: dark space with a soft atmosphere halo and a few stars.
                let halo = (-(rr - 1.0) * 14.0).exp() * 0.5;
                let star = {
                    let h = (px as u64).wrapping_mul(73856093) ^ (py as u64).wrapping_mul(19349663) ^ cfg.seed;
                    (h % 997 == 0) as u8 as f32 * 0.8
                };
                let v = (0.02 + halo as f32 * 0.35 + star).clamp(0.0, 1.0);
                return Color32::from_rgb((v * 120.0) as u8, (v * 190.0 + 4.0) as u8, (v * 255.0 + 8.0).min(255.0) as u8);
            }
            let (sx, sy) = (x / rim, y / rim);
            let sz = (1.0 - sx * sx - sy * sy).max(0.0).sqrt();
            let dir = (x_axis * sx + y_axis * sy + z_axis * sz).normalize();
            let col = planet.column(dir);
            let c = biome_color(&col);
            let view_n = DVec3::new(sx, sy, sz);
            let l = view_n.dot(light).max(0.0) as f32;
            let shade = 0.30 + 0.78 * l;
            let limb = 1.0 - (1.0 - sz as f32).powi(3) * 0.35;
            let atmos = (1.0 - sz as f32).powi(4) * 0.5;
            let f = |v: f32, a: f32| ((v * shade * limb + atmos * a).clamp(0.0, 1.0) * 255.0) as u8;
            Color32::from_rgb(f(c[0], 0.35), f(c[1], 0.6), f(c[2], 1.0))
        })
        .collect();
    ColorImage { size: [size, size], pixels }
}

/// Rounded cube (|x|⁸+|y|⁸+|z|⁸ = 1) seen from a corner, so three faces show.
/// Orthographic rays are marched through the bounding box and refined by bisection.
pub fn render_cube(cfg: &WorldConfig, size: usize) -> ColorImage {
    let planet = Planet::new(cfg);
    let c = planet.continents.first().map(|(c, _)| *c).unwrap_or(DVec3::new(0.3, 0.2, 0.93).normalize());
    let corner = DVec3::new(c.x.signum(), c.y.signum(), c.z.signum()).normalize();
    let z_axis = (c + corner * 1.4).normalize();
    let up_hint = if z_axis.y.abs() > 0.95 { DVec3::X } else { DVec3::Y };
    let x_axis = up_hint.cross(z_axis).normalize();
    let y_axis = z_axis.cross(x_axis);
    let light = (z_axis * 0.7 + y_axis * 0.55 - x_axis * 0.45).normalize();
    let inside = |p: DVec3| {
        let (x, y, z) = (p.x * p.x, p.y * p.y, p.z * p.z);
        let (x, y, z) = (x * x, y * y, z * z);
        x * x + y * y + z * z < 1.0
    };
    let n = size as f64;
    let scale = 1.62;
    let pixels: Vec<Color32> = (0..size * size)
        .into_par_iter()
        .map(|i| {
            let (px, py) = ((i % size) as f64, (i / size) as f64);
            let sx = ((px + 0.5) / n * 2.0 - 1.0) * scale;
            let sy = (1.0 - (py + 0.5) / n * 2.0) * scale;
            let origin = x_axis * sx + y_axis * sy + z_axis * 3.0;
            let dir = -z_axis;
            // March from the front to the back of the box around the cube.
            let mut t = 1.0;
            let mut hit = None;
            let step = 0.012;
            while t < 5.0 {
                if inside(origin + dir * t) {
                    let (mut a, mut b) = (t - step, t);
                    for _ in 0..12 {
                        let m = (a + b) * 0.5;
                        if inside(origin + dir * m) { b = m } else { a = m }
                    }
                    hit = Some(origin + dir * b);
                    break;
                }
                t += step;
            }
            let Some(p) = hit else {
                let star = {
                    let h = (px as u64).wrapping_mul(73856093) ^ (py as u64).wrapping_mul(19349663) ^ cfg.seed;
                    (h % 997 == 0) as u8 as f32 * 0.8
                };
                let v = (0.02 + star).clamp(0.0, 1.0);
                return Color32::from_rgb((v * 120.0) as u8, (v * 190.0 + 4.0) as u8, (v * 255.0 + 8.0).min(255.0) as u8);
            };
            let col = planet.column(p.normalize());
            let c = biome_color(&col);
            let normal = wolfcraft_engine::cubesphere::cube_up(p);
            let l = normal.dot(light).max(0.0) as f32;
            let shade = 0.32 + 0.75 * l;
            let rim = (1.0 - normal.dot(z_axis).max(0.0) as f32).powi(3) * 0.35;
            let f = |v: f32, a: f32| ((v * shade + rim * a).clamp(0.0, 1.0) * 255.0) as u8;
            Color32::from_rgb(f(c[0], 0.35), f(c[1], 0.6), f(c[2], 1.0))
        })
        .collect();
    ColorImage { size: [size, size], pixels }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wolfcraft_engine::config::{PlanetSize, WorldType};

    fn cfg(seed: u64, wt: WorldType) -> WorldConfig {
        let mut c = WorldConfig::new("t", seed);
        c.size = PlanetSize::Small;
        c.world_type = wt;
        c
    }

    #[test]
    fn globe_shows_land_and_ocean_and_differs_by_seed() {
        let a = render_globe(&cfg(1, WorldType::Standard), 96);
        let b = render_globe(&cfg(2, WorldType::Standard), 96);
        assert_eq!(a.pixels.len(), 96 * 96);
        assert_ne!(a.pixels, b.pixels);
        let blueish = a.pixels.iter().filter(|c| c.b() as i32 > c.r() as i32 + 40 && c.b() > c.g()).count();
        let greenish = a.pixels.iter().filter(|c| c.g() > c.b()).count();
        assert!(blueish > 300, "no ocean visible");
        assert!(greenish > 300, "no land visible");
        // Corners are space, centre is planet.
        assert!(a.pixels[0].r() < 40 && a.pixels[0].g() < 70);
    }

    #[test]
    fn same_seed_same_picture() {
        let a = render_globe(&cfg(7, WorldType::Islands), 64);
        let b = render_globe(&cfg(7, WorldType::Islands), 64);
        assert_eq!(a.pixels, b.pixels);
    }

    #[test]
    fn cube_preview_has_planet_and_space() {
        let mut c = cfg(3, WorldType::Standard);
        c.shape = wolfcraft_engine::config::PlanetShape::Cube;
        let t = std::time::Instant::now();
        let img = render_globe(&c, 256);
        eprintln!("cube preview 256: {:?}", t.elapsed());
        if let Ok(path) = std::env::var("WC_SAVE_PREVIEW") {
            let raw: Vec<u8> = img.pixels.iter().flat_map(|p| [p.r(), p.g(), p.b(), 255]).collect();
            image::save_buffer(path, &raw, 256, 256, image::ExtendedColorType::Rgba8).unwrap();
        }
        let planet = img.pixels.iter().filter(|p| p.b() > 60 || p.g() > 60).count();
        assert!(planet > 256 * 256 * 3 / 10, "planet covers the middle: {planet}");
        assert!(img.pixels[0].r() < 40, "corner is space");
    }
}