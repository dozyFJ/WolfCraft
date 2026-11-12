//! The planet: a sphere of cubic blocks centred at the origin.
//!
//! Every terrain feature is a function of the *direction* from the planet centre, so there are
//! no seams anywhere and walking in one direction returns to the start. Voxels live in a plain
//! Cartesian grid, so chunks, meshing and storage need no special cases.

use crate::biome::Biome;
use crate::block::Block;
use crate::config::{ContinentCount, PlanetShape, WorldConfig, WorldType};
use crate::cubesphere::{Cell, Grid};
use glam::DVec3;
use noise::{Fbm, MultiFractal, NoiseFn, OpenSimplex, RidgedMulti};

pub fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

pub fn hash3(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let a = splitmix(seed ^ ((x as u32 as u64) | ((y as u32 as u64) << 32)));
    splitmix(a ^ (z as u32 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93))
}

pub fn to01(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

pub struct Rng(pub u64);
impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = splitmix(self.0);
        self.0
    }
    pub fn f64(&mut self) -> f64 {
        to01(self.next_u64())
    }
    pub fn range(&mut self, lo: u32, hi_inclusive: u32) -> u32 {
        lo + (self.next_u64() % (hi_inclusive - lo + 1) as u64) as u32
    }
    pub fn unit_vec(&mut self) -> DVec3 {
        let z = self.f64() * 2.0 - 1.0;
        let phi = self.f64() * std::f64::consts::TAU;
        let s = (1.0 - z * z).sqrt();
        DVec3::new(s * phi.cos(), z, s * phi.sin())
    }
}

#[inline]
pub fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Top radii of the interior layers.
#[derive(Clone, Copy, Debug)]
pub struct Layers {
    pub inner_core_top: f64,
    pub outer_core_top: f64,
    pub mantle_top: f64,
    pub deep_crust_top: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Space,
    Surface,
    Crust,
    DeepCrust,
    Mantle,
    OuterCore,
    InnerCore,
}

impl Layer {
    pub fn label(self) -> &'static str {
        match self {
            Layer::Space => "Атмосфера",
            Layer::Surface => "Поверхность",
            Layer::Crust => "Земная кора",
            Layer::DeepCrust => "Глубокая кора",
            Layer::Mantle => "Мантия",
            Layer::OuterCore => "Внешнее ядро",
            Layer::InnerCore => "Внутреннее ядро",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Column {
    /// Surface height above sea level (blocks); negative = under water.
    pub elev: f32,
    pub biome: Biome,
    pub temp: f32,
    pub humid: f32,
    /// Continent mask (>0 land).
    pub land: f32,
    pub river: bool,
    pub volcanic: f32,
    pub lava_lake: bool,
}

pub struct Planet {
    pub seed: u64,
    pub radius: f64,
    pub world_type: WorldType,
    pub structures: bool,
    /// The Hell dimension: a hollow cavern shell with a lava sea instead of normal terrain.
    pub hell: bool,
    pub layers: Layers,
    pub grid: Grid,
    /// Height scale for mountains (bigger planets get slightly taller mountains).
    pub amp: f64,
    /// Highest possible terrain radius (without trees/structures).
    pub max_surface: f64,
    pub continents: Vec<(DVec3, f64)>,
    n_warp: OpenSimplex,
    n_edge: Fbm<OpenSimplex>,
    n_hill: Fbm<OpenSimplex>,
    n_ridge: RidgedMulti<OpenSimplex>,
    n_mmask: Fbm<OpenSimplex>,
    n_erosion: Fbm<OpenSimplex>,
    n_river: OpenSimplex,
    n_lake: Fbm<OpenSimplex>,
    n_temp: Fbm<OpenSimplex>,
    n_humid: Fbm<OpenSimplex>,
    n_volc: Fbm<OpenSimplex>,
    n_misc: Fbm<OpenSimplex>,
    n_cave_a: OpenSimplex,
    n_cave_b: OpenSimplex,
    n_cavern: Fbm<OpenSimplex>,
}

impl Planet {
    pub fn new(cfg: &WorldConfig) -> Self {
        let seed = cfg.seed;
        let radius = cfg.size.radius() as f64;
        let s = |k: u64| (splitmix(seed ^ k) & 0xFFFF_FFFF) as u32;
        let fbm = |k: u64, oct: usize| Fbm::<OpenSimplex>::new(s(k)).set_octaves(oct);

        let crust_bottom = radius - (radius * 0.08).max(110.0);
        let deep_bottom = crust_bottom - (radius * 0.10).max(140.0);
        let layers = Layers {
            inner_core_top: radius * 0.15,
            outer_core_top: radius * 0.30,
            mantle_top: deep_bottom,
            deep_crust_top: crust_bottom,
        };
        let amp = (radius / 1024.0).clamp(0.7, 1.5);

        let mut p = Self {
            seed,
            radius,
            world_type: cfg.world_type,
            structures: cfg.structures,
            hell: false,
            layers,
            grid: if cfg.shape == PlanetShape::Cube { Grid::new_cube(radius) } else { Grid::new(radius) },
            amp,
            max_surface: radius + 300.0 * amp + 40.0,
            continents: vec![],
            n_warp: OpenSimplex::new(s(1)),
            n_edge: fbm(2, 5),
            n_hill: fbm(3, 4),
            n_ridge: RidgedMulti::<OpenSimplex>::new(s(4)).set_octaves(5),
            n_mmask: fbm(5, 2),
            n_erosion: fbm(6, 3),
            n_river: OpenSimplex::new(s(7)),
            n_lake: fbm(8, 2),
            n_temp: fbm(9, 3),
            n_humid: fbm(10, 3),
            n_volc: fbm(11, 2),
            n_misc: fbm(12, 2),
            n_cave_a: OpenSimplex::new(s(13)),
            n_cave_b: OpenSimplex::new(s(14)),
            n_cavern: fbm(15, 2),
        };
        p.continents = p.make_continents(cfg.continents);
        p
    }

    /// The Hell dimension of a world: same size, its own seed, no structures.
    pub fn new_hell(cfg: &WorldConfig) -> Self {
        let mut c = cfg.clone();
        c.seed ^= 0x4E37_4845_4C4C;
        c.structures = false;
        let mut p = Self::new(&c);
        p.hell = true;
        p.max_surface = p.radius + 92.0;
        p
    }

    /// Hell terrain: floor height variation only (everything else is decided in `hell_block`).
    fn hell_column(&self, d: DVec3) -> Column {
        let e = Self::n3(&self.n_misc, d * 24.0) * 5.0 + Self::n3(&self.n_hill, d * 9.0) * 4.0;
        Column { elev: e as f32, biome: Biome::Volcano, temp: 1.0, humid: 0.0, land: 1.0, river: false, volcanic: 1.0, lava_lake: false }
    }

    fn hell_block(&self, c: Cell, dir: DVec3, r: f64) -> Block {
        let floor = self.radius + self.hell_column(dir).elev as f64;
        let sea = self.radius - 1.0;
        let ceil_bottom = self.radius + 50.0 + Self::n3(&self.n_cavern, dir * 14.0) * 7.0;
        let h = hash3(self.seed, c.i + c.f as i32 * 7919, c.j, c.k);
        let u = to01(h);
        if r < floor {
            // Netherrack with glowing veins and magma pockets.
            return if u > 0.992 { Block::Glowstone } else if u < 0.004 { Block::Magma } else { Block::Netherrack };
        }
        if r > self.radius + 82.0 {
            return Block::Air;
        }
        if r > self.radius + 76.0 {
            return Block::Bedrock;
        }
        if r >= ceil_bottom {
            return if u > 0.985 { Block::Glowstone } else { Block::Netherrack };
        }
        if r < sea {
            return Block::Lava;
        }
        // Stalactites hanging from the ceiling and glowstone clusters.
        let drop = (Self::n3(&self.n_warp, dir * 55.0) + 1.0) * 5.0;
        if r > ceil_bottom - drop && Self::n3(&self.n_warp, dir * 55.0) > 0.25 {
            return if u > 0.95 { Block::Glowstone } else { Block::Netherrack };
        }
        Block::Air
    }

    /// Chooses continent centres (well separated) and their angular radii.
    fn make_continents(&self, count: ContinentCount) -> Vec<(DVec3, f64)> {
        let mut rng = Rng(splitmix(self.seed ^ 0xC0FFEE));
        let (n, land_frac) = match self.world_type {
            WorldType::Flat | WorldType::Baseplate => return vec![],
            WorldType::Standard => {
                let (lo, hi) = count.range();
                (rng.range(lo, hi), 0.38)
            }
            WorldType::Supercontinent => (1, 0.52),
            WorldType::Islands => (rng.range(22, 36), 0.13),
        };
        let mut centers: Vec<DVec3> = Vec::new();
        for _ in 0..n {
            let mut best = rng.unit_vec();
            let mut best_d = -1.0;
            for _ in 0..40 {
                let c = rng.unit_vec();
                let d = centers.iter().map(|o| o.dot(c).clamp(-1.0, 1.0).acos()).fold(f64::MAX, f64::min);
                let d = if centers.is_empty() { 1.0 } else { d };
                if d > best_d {
                    best_d = d;
                    best = c;
                }
            }
            centers.push(best);
        }
        let base = (1.0 - 2.0 * land_frac / n as f64).clamp(-1.0, 1.0).acos();
        centers.into_iter().map(|c| (c, base * (0.8 + 0.4 * rng.f64()))).collect()
    }

    #[inline]
    fn n3(f: &impl NoiseFn<f64, 3>, v: DVec3) -> f64 {
        f.get([v.x, v.y, v.z])
    }

    /// Terrain description for the direction `d` (unit vector). Pure function of (seed, d).
    pub fn column(&self, d: DVec3) -> Column {
        if self.hell {
            return self.hell_column(d);
        }
        let flat = matches!(self.world_type, WorldType::Flat | WorldType::Baseplate);
        let wp = d * self.radius;
        let amp = self.amp;

        let w = DVec3::new(
            Self::n3(&self.n_warp, d * 2.0 + DVec3::new(11.3, 0.0, 0.0)),
            Self::n3(&self.n_warp, d * 2.0 + DVec3::new(0.0, 7.7, 0.0)),
            Self::n3(&self.n_warp, d * 2.0 + DVec3::new(0.0, 0.0, 3.1)),
        ) * 0.12;
        let dw = (d + w).normalize();

        // Continent mask: >0 land. Honest continent count: one falloff cone per centre.
        let m = if flat {
            1.0
        } else {
            let mut best = -9.0f64;
            for (c, r) in &self.continents {
                let ang = c.dot(d).clamp(-1.0, 1.0).acos();
                best = best.max(1.0 - ang / r);
            }
            best + 0.55 * Self::n3(&self.n_edge, dw * 3.0)
        };

        let erosion = (Self::n3(&self.n_erosion, dw * 4.0) * 0.5 + 0.5).clamp(0.0, 1.0);
        let mut e;
        if flat {
            e = 4.0 + Self::n3(&self.n_hill, wp * 0.02) * 0.6;
        } else if m <= 0.0 {
            let t = (-m / 0.45).min(1.0);
            e = -2.0 - 64.0 * t.powf(1.25) + Self::n3(&self.n_hill, wp * 0.02) * 2.0;
        } else {
            let coast = smoothstep(0.0, 0.10, m);
            let base = -2.0 + 6.0 * coast + (m - 0.10).max(0.0) * 16.0;
            let hills = Self::n3(&self.n_hill, wp * 0.012) * 9.0 * smoothstep(0.08, 0.3, m);
            let mmask = smoothstep(0.1, 0.55, Self::n3(&self.n_mmask, dw * 2.4)) * smoothstep(0.18, 0.5, m);
            let ridge = (Self::n3(&self.n_ridge, wp * 0.005) * 0.5 + 0.5).clamp(0.0, 1.0).powf(1.4);
            e = base + hills + mmask * ridge * 150.0 * amp * (1.0 - 0.45 * erosion);
            // Block-game relief: small bumps of 2-4 blocks every few dozen blocks, so even
            // plains are not flat, fading out on beaches.
            let land = smoothstep(0.04, 0.14, m);
            e += (Self::n3(&self.n_hill, wp * 0.045 + DVec3::splat(91.7)) * 3.2 + Self::n3(&self.n_misc, wp * 0.11) * 1.1) * land;
        }

        let mut river = false;
        let mut volcanic = 0.0;
        let mut lava_lake = false;
        if !flat && m > 0.04 {
            // Rivers: thin bands where a smooth noise field crosses zero.
            let rn = Self::n3(&self.n_river, wp * 0.0017).abs();
            let rf = 1.0 - smoothstep(0.010, 0.032, rn);
            let wgt = smoothstep(0.05, 0.18, m) * (1.0 - smoothstep(18.0, 45.0, e));
            if rf * wgt > 0.0 {
                e = lerp(e, -3.5, rf * wgt);
                river = rf * wgt > 0.5;
            }
            // Lakes: noise basins in the lowlands.
            let ln = Self::n3(&self.n_lake, wp * 0.007);
            if ln > 0.5 {
                let lw = smoothstep(0.5, 0.62, ln) * smoothstep(0.12, 0.3, m) * (1.0 - smoothstep(10.0, 30.0, e));
                e = lerp(e, -4.0, lw);
            }
            // Volcanoes: cone + crater (+ lava lake).
            if m > 0.2 {
                let f = Self::n3(&self.n_volc, dw * 3.3);
                volcanic = smoothstep(0.42, 0.60, f) * smoothstep(0.2, 0.3, m);
                let cone = smoothstep(0.36, 0.60, f) * smoothstep(0.2, 0.3, m);
                e += cone * 85.0 * amp;
                if f > 0.66 {
                    e -= (f - 0.66) * 350.0;
                }
                if f > 0.70 {
                    lava_lake = true;
                }
            }
        }

        let lat = d.y.abs();
        let temp = (1.0 - lat.powf(1.4) * 1.05 + 0.12 * Self::n3(&self.n_temp, dw * 2.0) - e.max(0.0) / 230.0).clamp(0.0, 1.0);
        let humid = (0.5 + 0.75 * Self::n3(&self.n_humid, dw * 2.3) + 0.15 * (1.0 - smoothstep(0.0, 0.5, m))).clamp(0.0, 1.0);
        let misc = Self::n3(&self.n_misc, dw * 6.0);

        let biome = if m <= 0.0 && !flat {
            if temp < 0.12 {
                Biome::FrozenOcean
            } else if e < -28.0 {
                Biome::DeepOcean
            } else {
                Biome::Ocean
            }
        } else if volcanic > 0.2 {
            Biome::Volcano
        } else if e > 95.0 && temp < 0.62 {
            Biome::SnowyPeaks
        } else if e > 60.0 {
            Biome::Mountains
        } else if e < 3.5 && !flat {
            Biome::Beach
        } else if temp < 0.10 {
            Biome::SnowyPlains
        } else if temp < 0.28 {
            if humid > 0.45 { Biome::Taiga } else { Biome::Tundra }
        } else if temp < 0.72 {
            if humid < 0.28 {
                Biome::Plains
            } else if humid < 0.5 {
                if misc > 0.3 { Biome::Plains } else { Biome::Forest }
            } else if humid > 0.78 && e < 10.0 {
                Biome::Swamp
            } else if misc > 0.0 {
                Biome::BirchForest
            } else {
                Biome::Forest
            }
        } else if humid < 0.28 {
            if erosion > 0.58 { Biome::Badlands } else { Biome::Desert }
        } else if humid < 0.55 {
            Biome::Savanna
        } else {
            Biome::Jungle
        };

        Column { elev: e as f32, biome, temp: temp as f32, humid: humid as f32, land: m as f32, river, volcanic: volcanic as f32, lava_lake }
    }

    #[inline]
    pub fn surface_radius(&self, d: DVec3) -> f64 {
        self.radius + self.column(d).elev as f64
    }

    pub fn layer_at(&self, r: f64, surface_r: f64) -> Layer {
        let l = &self.layers;
        if r > surface_r + 40.0 {
            Layer::Space
        } else if r >= surface_r - 20.0 {
            Layer::Surface
        } else if r >= l.deep_crust_top {
            Layer::Crust
        } else if r >= l.mantle_top {
            Layer::DeepCrust
        } else if r >= l.outer_core_top {
            Layer::Mantle
        } else if r >= l.inner_core_top {
            Layer::OuterCore
        } else {
            Layer::InnerCore
        }
    }

    /// Is the terrain column needed to decide the block at radius `r`?
    #[inline]
    pub fn needs_column(&self, r: f64) -> bool {
        !self.hell && r >= self.radius - 80.0 && r <= self.max_surface && r >= self.layers.deep_crust_top
    }

    /// Block of cell `c`. `dir` is the cell's unit direction; `col` the (optional) terrain column.
    pub fn cell_block(&self, c: Cell, dir: DVec3, col: Option<&Column>) -> Block {
        let r = c.k as f64 + 0.5;
        let l = &self.layers;
        if r < l.deep_crust_top {
            return if r < l.inner_core_top {
                Block::InnerCore
            } else if r < l.outer_core_top {
                Block::Lava
            } else if r < l.mantle_top {
                Block::Magma
            } else {
                self.deep_crust_block(c, r)
            };
        }
        if self.hell {
            return self.hell_block(c, dir, r);
        }
        if r > self.max_surface {
            return Block::Air;
        }
        if r < self.radius - 80.0 {
            return self.underground_block(c, dir * r, r, 12.0);
        }
        let owned;
        let col = match col {
            Some(c) => c,
            None => {
                owned = self.column(dir);
                &owned
            }
        };
        let s = self.radius + col.elev as f64;
        self.crust_block(c, dir * r, r, s, col)
    }

    /// Block and (when near the surface) the terrain column of a cell.
    pub fn voxel(&self, c: Cell) -> (Block, Option<Column>) {
        let dir = self.grid.cell_dir(c.f, c.i, c.j);
        let r = c.k as f64 + 0.5;
        if self.needs_column(r) {
            let col = self.column(dir);
            (self.cell_block(c, dir, Some(&col)), Some(col))
        } else {
            (self.cell_block(c, dir, None), None)
        }
    }

    pub fn block_at(&self, c: Cell) -> Block {
        if self.world_type == WorldType::Baseplate && !self.hell {
            let top = self.radius.round() as i32;
            return if c.k == top { Block::Baseplate } else if c.k == top - 1 { Block::Bedrock } else { Block::Air };
        }
        self.voxel(c).0
    }

    /// Generated block at a world position (ignores player edits).
    pub fn block_at_world(&self, p: DVec3) -> Block {
        self.block_at(self.grid.world_to_cell(p))
    }

    /// Index of the topmost solid layer of a column (from the terrain height).
    pub fn top_k(&self, col: &Column) -> i32 {
        (self.radius + col.elev as f64 - 0.5).floor() as i32
    }

    fn deep_crust_block(&self, c: Cell, r: f64) -> Block {
        // Rare precious veins in the deep crust.
        if let Some(b) = self.ore_at(c, self.radius - r) {
            if b == Block::GoldOre || b == Block::DiamondOre {
                return b;
            }
        }
        Block::Basalt
    }

    fn crust_block(&self, c: Cell, centre: DVec3, r: f64, s: f64, col: &Column) -> Block {
        // Cliffs and overhangs on high ground: 3D noise bends the surface in and out.
        let cliff = smoothstep(22.0, 70.0, col.elev as f64);
        let s = if cliff > 0.0 && (r - s).abs() < 12.0 {
            s + Self::n3(&self.n_cave_a, centre * 0.045 + DVec3::splat(33.3)) * 9.0 * cliff
        } else {
            s
        };
        if r > s {
            // Above ground: lava lake, sea, ice sheet or air.
            if col.lava_lake && r <= self.radius + 72.0 * self.amp {
                return Block::Lava;
            }
            if r <= self.radius {
                if r > self.radius - 1.0 && col.temp < 0.12 {
                    return Block::Ice;
                }
                return Block::Water;
            }
            return Block::Air;
        }
        let depth = s - r;
        let underwater = s < self.radius;
        let (top, filler, under) = if underwater {
            let h = hash3(self.seed ^ 0x5EA, c.i >> 2, c.j >> 2, (c.k >> 2) + c.f as i32 * 4096) % 100;
            let floor = if col.elev > -12.0 {
                Block::Sand
            } else if h < 18 {
                Block::Clay
            } else {
                Block::Gravel
            };
            (floor, if floor == Block::Sand { Block::Sand } else { Block::Dirt }, Block::Stone)
        } else {
            let (mut t, f, u) = col.biome.palette();
            if col.biome == Biome::Mountains {
                let h = hash3(self.seed ^ 0x6A7, c.i >> 1, c.j >> 1, c.f as i32 * 8192) % 100;
                if h < 12 {
                    t = Block::Gravel;
                } else if col.elev < 40.0 && col.humid > 0.3 && h < 70 {
                    t = Block::Grass;
                }
            }
            // Patches: podzol under the taiga, moss in jungles and swamps.
            let patch = hash3(self.seed ^ 0x9D2, c.i >> 2, c.j >> 2, c.f as i32 * 8192) % 100;
            if col.biome == Biome::Taiga && patch < 35 {
                t = Block::Podzol;
            } else if matches!(col.biome, Biome::Jungle | Biome::Swamp) && patch < 12 {
                t = Block::MossBlock;
            }
            (t, f, u)
        };
        if depth < 1.0 {
            return top;
        }
        if depth < 4.0 {
            return filler;
        }
        if depth < 7.0 && under != Block::Stone {
            return under;
        }
        self.underground_block(c, centre, r, depth)
    }

    /// Stone body: caves, ores. `depth` is the (lower bound of) depth below the surface.
    fn underground_block(&self, c: Cell, centre: DVec3, r: f64, depth: f64) -> Block {
        if let Some(fill) = self.cave(centre, r, depth) {
            return fill;
        }
        if let Some(ore) = self.ore_at(c, self.radius - r) {
            return ore;
        }
        Block::Stone
    }

    fn cave(&self, c: DVec3, r: f64, depth: f64) -> Option<Block> {
        if depth < 5.0 || r < self.layers.deep_crust_top {
            return None;
        }
        let k = 0.028;
        let tunnel = {
            let a = self.n_cave_a.get([c.x * k, c.y * k, c.z * k]);
            a.abs() < 0.07 && self.n_cave_b.get([c.x * k + 31.0, c.y * k - 17.0, c.z * k + 5.0]).abs() < 0.07
        };
        // Large caverns exist only in radial bands: several stacked cave levels.
        let band = (r * 0.045).sin();
        let cavern = depth > 18.0 && band > 0.3 && Self::n3(&self.n_cavern, c * 0.011) > 0.5 - (band - 0.3) * 0.25;
        if !tunnel && !cavern {
            return None;
        }
        let lava_level = self.layers.deep_crust_top + 35.0;
        Some(if r < lava_level {
            Block::Lava
        } else if cavern && r > self.radius - 120.0 && r < self.radius - 35.0 {
            Block::Water
        } else {
            Block::Air
        })
    }

    /// Ore veins: one candidate blob per 6x6x6 block of cells.
    pub fn ore_at(&self, c: Cell, depth_below_sea: f64) -> Option<Block> {
        let (ci, cj, ck) = (c.i.div_euclid(6), c.j.div_euclid(6), c.k.div_euclid(6));
        let h = hash3(self.seed ^ 0x0BE5 ^ ((c.f as u64 + 1) << 56), ci, cj, ck);
        let roll = (h % 1000) as u32;
        let kind = if roll < 48 && depth_below_sea > 2.0 {
            Block::CoalOre
        } else if roll < 76 && depth_below_sea > 10.0 {
            Block::IronOre
        } else if roll < 85 && depth_below_sea > 30.0 {
            Block::GoldOre
        } else if roll < 89 && depth_below_sea > 55.0 {
            Block::DiamondOre
        } else if roll < 99 && depth_below_sea > 16.0 {
            Block::IskritOre
        } else if roll < 102 && depth_below_sea > 45.0 {
            Block::UraniumOre
        } else if roll < 110 && depth_below_sea > 4.0 {
            Block::CopperOre
        } else if roll < 128 && depth_below_sea > 8.0 {
            Block::TinOre
        } else if roll < 138 && depth_below_sea > 35.0 {
            Block::SilverOre
        } else if roll < 144 && depth_below_sea > 60.0 {
            Block::CobaltOre
        } else if roll < 160 && depth_below_sea > 3.0 {
            Block::Granite
        } else if roll < 176 && depth_below_sea > 3.0 {
            Block::Limestone
        } else if roll < 188 && depth_below_sea > 10.0 {
            Block::Marble
        } else if roll < 198 && depth_below_sea > 20.0 {
            Block::Slate
        } else {
            return None;
        };
        let jx = to01(splitmix(h ^ 1)) * 6.0;
        let jy = to01(splitmix(h ^ 2)) * 6.0;
        let jz = to01(splitmix(h ^ 3)) * 6.0;
        let center = DVec3::new(ci as f64 * 6.0 + jx, cj as f64 * 6.0 + jy, ck as f64 * 6.0 + jz);
        let radius = 1.5 + ((h >> 20) & 3) as f64 * 0.45;
        let d = (DVec3::new(c.i as f64 + 0.5, c.j as f64 + 0.5, c.k as f64 + 0.5) - center).length();
        (d < radius).then_some(kind)
    }

    /// Finds a safe spawn: dry land, gentle terrain, standing on solid ground. Deterministic.
    pub fn find_spawn(&self) -> DVec3 {
        let mut rng = Rng(splitmix(self.seed ^ 0x5EED));
        let mut fallback = None;
        for i in 0..4000 {
            let d = rng.unit_vec();
            let col = self.column(d);
            let ok_biome = matches!(col.biome, Biome::Plains | Biome::Forest | Biome::BirchForest | Biome::Savanna | Biome::Taiga | Biome::Desert | Biome::Jungle | Biome::Tundra);
            if col.elev < 3.0 || col.elev > 40.0 || col.river {
                continue;
            }
            // Stay away from face seams so the start area is one simple grid.
            let cell = self.grid.world_to_cell(d * self.radius);
            let margin = self.grid.n / 4; // central half of a face: cells are almost square there
            if cell.i < margin || cell.j < margin || cell.i >= self.grid.n - margin || cell.j >= self.grid.n - margin {
                continue;
            }
            if let Some(pos) = self.stand_position(d) {
                if ok_biome {
                    return pos;
                }
                if fallback.is_none() && col.biome != Biome::Volcano && !col.biome.is_ocean() {
                    fallback = Some(pos);
                }
            }
            if i > 2000 && fallback.is_some() {
                break;
            }
        }
        fallback.unwrap_or_else(|| DVec3::new(0.0, self.radius + 80.0, 0.0))
    }

    /// Scans down the column below direction `d` for solid ground with two free blocks above.
    /// Returns the feet position just above the surface.
    pub fn stand_position(&self, d: DVec3) -> Option<DVec3> {
        let base = self.grid.world_to_cell(d * self.radius);
        let dir = self.grid.cell_dir(base.f, base.i, base.j);
        let col = self.column(dir);
        let top = self.top_k(&col);
        let is_leaf_or_log = |b: Block| {
            matches!(b, Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves | Block::AcaciaLeaves | Block::OakLog | Block::BirchLog | Block::SpruceLog | Block::AcaciaLog)
        };
        let free = |k: i32| {
            let b = self.block_at(Cell::new(base.f, base.i, base.j, k));
            !b.is_solid() && !b.is_fluid()
        };
        for k in ((top - 12)..=(top + 24)).rev() {
            let b = self.block_at(Cell::new(base.f, base.i, base.j, k));
            if b.is_solid() && !is_leaf_or_log(b) {
                return if free(k + 1) && free(k + 2) { Some(self.grid.along(dir, k as f64 + 1.3)) } else { None };
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    fn planet(size: PlanetSize, wt: WorldType, seed: u64) -> Planet {
        let mut c = WorldConfig::new("t", seed);
        c.size = size;
        c.world_type = wt;
        Planet::new(&c)
    }

    #[test]
    fn deterministic_by_seed() {
        let a = planet(PlanetSize::Small, WorldType::Standard, 1);
        let b = planet(PlanetSize::Small, WorldType::Standard, 1);
        let c = planet(PlanetSize::Small, WorldType::Standard, 2);
        let mut diff = 0;
        for i in 0..200 {
            let d = Rng(i).unit_vec();
            assert_eq!(a.column(d).elev, b.column(d).elev);
            if (a.column(d).elev - c.column(d).elev).abs() > 0.5 {
                diff += 1;
            }
        }
        assert!(diff > 50, "different seeds must give different planets");
    }

    #[test]
    fn continent_count_is_honoured() {
        let mut c = WorldConfig::new("t", 5);
        c.continents = ContinentCount::C8to12;
        let p = Planet::new(&c);
        assert!((8..=12).contains(&p.continents.len()), "{}", p.continents.len());
        c.continents = ContinentCount::C3to5;
        let p = Planet::new(&c);
        assert!((3..=5).contains(&p.continents.len()));
    }

    #[test]
    fn has_land_and_ocean() {
        let p = planet(PlanetSize::Small, WorldType::Standard, 9);
        let (mut land, mut sea) = (0, 0);
        for i in 0..2000 {
            if p.column(Rng(i + 77).unit_vec()).elev >= 0.0 {
                land += 1
            } else {
                sea += 1
            }
        }
        assert!(land > 200 && sea > 200, "land {land} sea {sea}");
    }

    #[test]
    fn terrain_is_continuous_around_the_whole_planet() {
        // Three great circles that cross every face edge: no seams in the height map.
        let p = planet(PlanetSize::Small, WorldType::Standard, 3);
        let steps = 4000;
        for ring in 0..3 {
            let mut prev: Option<f32> = None;
            for i in 0..=steps {
                let a = i as f64 / steps as f64 * std::f64::consts::TAU;
                let d = match ring {
                    0 => DVec3::new(a.cos(), 0.0, a.sin()),
                    1 => DVec3::new(a.cos(), a.sin(), 0.0),
                    _ => DVec3::new(0.0, a.cos(), a.sin()),
                };
                let e = p.column(d).elev;
                if let Some(pe) = prev {
                    assert!((e - pe).abs() < 14.0, "seam at ring {ring} step {i}: {pe} -> {e}");
                }
                prev = Some(e);
            }
        }
    }

    #[test]
    fn cell_terrain_matches_the_height_map_on_every_face() {
        // Columns on all six faces, including cells next to the seams: the top solid block is
        // exactly where the continuous terrain function says.
        let p = planet(PlanetSize::Small, WorldType::Standard, 11);
        let n = p.grid.n;
        for f in 0..6u8 {
            for (i, j) in [(0, 0), (1, n - 2), (n / 2, n / 2), (n - 1, 3), (5, n / 2), (n - 1, n - 1)] {
                let dir = p.grid.cell_dir(f, i, j);
                let col = p.column(dir);
                let top = p.top_k(&col);
                let solid_top = p.block_at(Cell::new(f, i, j, top));
                let above = p.block_at(Cell::new(f, i, j, top + 1));
                assert!(solid_top.is_solid(), "face {f} ({i},{j}): top block {solid_top:?} at k={top}");
                assert!(!above.is_solid(), "face {f}: solid block directly above the surface ({above:?})");
            }
        }
    }

    #[test]
    fn layers_ordered_and_blocks_match() {
        let p = planet(PlanetSize::Small, WorldType::Standard, 4);
        let at = |r: f64| p.block_at_world(DVec3::new(0.3, 1.0, 0.2).normalize() * r);
        assert_eq!(at(3.0), Block::InnerCore);
        assert_eq!(at(p.radius * 0.22), Block::Lava);
        assert_eq!(at(p.radius * 0.5), Block::Magma);
        assert_eq!(at(p.layers.mantle_top + 10.0), Block::Basalt);
        assert_eq!(at(p.max_surface + 5.0), Block::Air);
        assert!(p.layers.inner_core_top < p.layers.outer_core_top);
        assert!(p.layers.outer_core_top < p.layers.mantle_top);
        assert!(p.layers.mantle_top < p.layers.deep_crust_top);
        assert!(p.layers.deep_crust_top < p.radius);
    }

    #[test]
    fn spawn_is_on_dry_ground() {
        for seed in [1u64, 2, 3, 77] {
            let p = planet(PlanetSize::Small, WorldType::Standard, seed);
            let s = p.find_spawn();
            let cell = p.grid.world_to_cell(s);
            assert!(!p.block_at(cell).is_solid(), "spawn inside a block");
            assert!(!p.block_at(cell).is_fluid(), "spawn in fluid");
            let below = Cell::new(cell.f, cell.i, cell.j, cell.k - 1);
            assert!(p.block_at(below).is_solid(), "no ground below spawn (seed {seed})");
            assert!(s.length() > p.radius, "spawn must be above sea level");
        }
    }

    #[test]
    fn flat_world_has_no_ocean() {
        let p = planet(PlanetSize::Small, WorldType::Flat, 1);
        for i in 0..300 {
            assert!(p.column(Rng(i).unit_vec()).elev > 3.0);
        }
    }

    #[test]
    fn hell_has_a_standable_floor_under_a_cavern_roof() {
        let cfg = WorldConfig::new("h", 5);
        let p = Planet::new_hell(&cfg);
        let d = DVec3::new(0.3, 0.8, 0.5).normalize();
        let pos = p.stand_position(d).expect("a spot to stand in hell");
        assert!(!p.block_at_world(pos).is_solid());
        assert!(p.block_at_world(pos + pos.normalize() * 60.0).is_solid() || p.block_at_world(pos + pos.normalize() * 70.0).is_solid());
    }}
