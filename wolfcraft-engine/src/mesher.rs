//! Greedy mesher with per-face ambient occlusion and sky exposure.
//!
//! Works on an 18³ padded copy of the chunk (1 block of neighbours on every side), so it can
//! run on any thread without touching the world. The padded array is indexed `(x, y, z) =
//! (j, k, i)` of the face grid: `y` is the radial "up", so skylight is a straight ray along `+y`
//! and textures keep their orientation everywhere on the planet. Vertex positions are then
//! projected onto the sphere through the cube-sphere mapping and stored relative to the chunk's
//! reference corner (small `f32` values, ready for floating-origin rendering).

use crate::block::{Block, RenderKind};
use crate::chunk::CHUNK_SIZE;
use crate::cubesphere::{ChunkKey, Grid};
use bytemuck::{Pod, Zeroable};

pub const PAD: usize = 18;
pub const PADDED_VOL: usize = PAD * PAD * PAD;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct Vertex {
    /// Position relative to the chunk's reference corner.
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// bits 0..16 tile, bit 16 emissive, bits 17..20 face id (0..5)
    pub flags: u32,
    /// x = ambient occlusion factor (0..1), y = skylight (0..1)
    pub light: [f32; 2],
}

#[derive(Default)]
pub struct MeshData {
    pub opaque: Vec<Vertex>,
    pub opaque_idx: Vec<u32>,
    pub water: Vec<Vertex>,
    pub water_idx: Vec<u32>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.opaque_idx.is_empty() && self.water_idx.is_empty()
    }
    pub fn quad_count(&self) -> usize {
        (self.opaque_idx.len() + self.water_idx.len()) / 6
    }
}

/// Everything the mesher needs; cheap to move to a worker thread.
pub struct MeshInput {
    pub key: ChunkKey,
    pub grid: Grid,
    pub blocks: Box<[u8; PADDED_VOL]>,
    pub sky_open: bool,
}

impl MeshInput {
    pub fn new(key: ChunkKey, grid: Grid, sky_open: bool) -> Self {
        Self { key, grid, blocks: Box::new([0; PADDED_VOL]), sky_open }
    }
    /// Index for padded coordinates `(x, y, z)` = `(j, k, i)` relative to the chunk, each in -1..=16.
    #[inline]
    pub fn idx(x: i32, y: i32, z: i32) -> usize {
        ((x + 1) as usize) + ((z + 1) as usize) * PAD + ((y + 1) as usize) * PAD * PAD
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32, b: Block) {
        self.blocks[Self::idx(x, y, z)] = b as u8;
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> Block {
        Block::from_u8(self.blocks[Self::idx(x, y, z)])
    }
    #[inline]
    fn getv(&self, p: [i32; 3]) -> Block {
        self.get(p[0], p[1], p[2])
    }

    /// Projects padded-array coordinates (possibly fractional) to a position relative to the
    /// chunk's reference corner.
    fn to_local(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let o = self.key.origin();
        let p = self.grid.point(self.key.f, o.x as f64 + z as f64, o.y as f64 + x as f64, o.z as f64 + y as f64);
        let r = self.grid.chunk_ref(self.key);
        let d = p - r;
        [d.x as f32, d.y as f32, d.z as f32]
    }
}

fn face_visible(b: Block, n: Block) -> bool {
    match b.info().render {
        RenderKind::Opaque => !n.is_opaque(),
        RenderKind::Cutout => !(n.is_opaque() || n == b),
        RenderKind::Translucent => !(n.is_opaque() || n == b),
        _ => false,
    }
}

const NORMALS: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Skylight 0..15 for the air cell `q` (padded coordinates): a straight ray up the column.
fn sky_at(inp: &MeshInput, q: [i32; 3]) -> u8 {
    for k in 1..=14 {
        let y = q[1] + k;
        if y > CHUNK_SIZE {
            return if inp.sky_open { 15 } else { 3 };
        }
        if inp.get(q[0], y, q[2]).blocks_light() {
            return (2 + k).min(13) as u8;
        }
    }
    if inp.sky_open { 15 } else { 3 }
}

pub fn mesh_chunk(inp: &MeshInput) -> MeshData {
    let mut out = MeshData::default();
    for f in 0..6 {
        mesh_direction(inp, f, &mut out);
    }
    mesh_cross(inp, &mut out);
    mesh_shapes(inp, &mut out);
    out
}

/// Slabs, panes, fences, doors: one box per block, textured with the matching part of the tile.
fn mesh_shapes(inp: &MeshInput, out: &mut MeshData) {
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let b = inp.get(x, y, z);
                let Some(s) = b.shape() else { continue };
                let info = b.info();
                let sky = if info.emissive { 1.0 } else { sky_at(inp, [x, y, z]).max(sky_at(inp, [x, y + 1, z])) as f32 / 15.0 };
                // Padded axes are (x, y, z) = (j, k, i); the shape is given as (i, j, k).
                let lo = [s[1], s[2], s[0]];
                let hi = [s[4], s[5], s[3]];
                let verts = &mut out.opaque;
                let idx = &mut out.opaque_idx;
                for f in 0..6usize {
                    let a = f / 2;
                    let positive = f % 2 == 0;
                    let (ua, va) = ((a + 1) % 3, (a + 2) % 3);
                    let tile = if a == 1 { if positive { info.tiles[2] } else { info.tiles[3] } } else { info.tiles[f] } as u32;
                    let shade = if a == 1 { if positive { 1.0 } else { 0.6 } } else { 0.85 };
                    let base = verts.len() as u32;
                    for (cu, cv) in [(0usize, 0usize), (1, 0), (1, 1), (0, 1)] {
                        let mut g = [0.0f32; 3];
                        g[a] = if positive { hi[a] } else { lo[a] };
                        g[ua] = if cu == 1 { hi[ua] } else { lo[ua] };
                        g[va] = if cv == 1 { hi[va] } else { lo[va] };
                        let pos = inp.to_local(x as f32 + g[0], y as f32 + g[1], z as f32 + g[2]);
                        // Same texture orientation as full cubes: image-up points away from the planet.
                        let (tu, tv) = if a == 1 {
                            (g[ua], g[va])
                        } else if ua == 1 {
                            (g[va], -g[ua])
                        } else {
                            (g[ua], -g[va])
                        };
                        verts.push(Vertex { pos, uv: [tu, tv], flags: tile | ((info.emissive as u32) << 16) | ((f as u32) << 17), light: [shade, sky] });
                    }
                    // Both windings: the box is small and may be seen from any side.
                    idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
                }
            }
        }
    }
}

fn mesh_direction(inp: &MeshInput, f: usize, out: &mut MeshData) {
    let a = f / 2;
    let sign: i32 = if f % 2 == 0 { 1 } else { -1 };
    let ua = (a + 1) % 3;
    let va = (a + 2) % 3;
    let n = NORMALS[f];
    let mut mask = [0u64; 256];

    for s in 0..CHUNK_SIZE {
        let mut any = false;
        for j in 0..CHUNK_SIZE {
            for i in 0..CHUNK_SIZE {
                let mut p = [0i32; 3];
                p[a] = s;
                p[ua] = i;
                p[va] = j;
                let b = inp.getv(p);
                mask[(i + j * 16) as usize] = 0;
                let info = b.info();
                if !matches!(info.render, RenderKind::Opaque | RenderKind::Cutout | RenderKind::Translucent) {
                    continue;
                }
                let q = [p[0] + n[0], p[1] + n[1], p[2] + n[2]];
                let nb = inp.getv(q);
                if !face_visible(b, nb) {
                    continue;
                }
                // Texture selection: the radial axis (y) gives top/bottom; the rest are sides.
                let tiles = &info.tiles;
                let tile = if a == 1 {
                    if sign > 0 { tiles[2] } else { tiles[3] }
                } else {
                    tiles[f]
                };
                // Ambient occlusion per corner (c0=(0,0) c1=(1,0) c2=(1,1) c3=(0,1) in (u,v)).
                let mut ao = [3u64; 4];
                for (ci, (cu, cv)) in [(-1, -1), (1, -1), (1, 1), (-1, 1)].iter().enumerate() {
                    let mut s1 = q;
                    s1[ua] += cu;
                    let mut s2 = q;
                    s2[va] += cv;
                    let mut c = q;
                    c[ua] += cu;
                    c[va] += cv;
                    let e1 = inp.getv(s1).occludes_ao() as u64;
                    let e2 = inp.getv(s2).occludes_ao() as u64;
                    let ec = inp.getv(c).occludes_ao() as u64;
                    ao[ci] = if e1 == 1 && e2 == 1 { 0 } else { 3 - (e1 + e2 + ec) };
                }
                let emit = info.emissive as u64;
                let sky = if info.emissive { 15 } else { sky_at(inp, q) as u64 };
                let trans = (info.render == RenderKind::Translucent) as u64;
                let key = (1u64 << 63) | (tile as u64) | (ao[0] << 16) | (ao[1] << 18) | (ao[2] << 20) | (ao[3] << 22) | (sky << 24) | (emit << 28) | (trans << 29);
                mask[(i + j * 16) as usize] = key;
                any = true;
            }
        }
        if !any {
            continue;
        }
        // One quad per cell face. Merging faces into big rectangles (greedy meshing) leaves
        // T-junction gaps on the curved cube-sphere: a long straight edge does not pass through
        // the neighbours' vertices, and the sky shows through as thin lines.
        for j in 0..16usize {
            let mut i = 0usize;
            while i < 16 {
                let key = mask[i + j * 16];
                if key == 0 {
                    i += 1;
                    continue;
                }
                let (w, h) = (1usize, 1usize);
                for jj in 0..h {
                    for ii in 0..w {
                        mask[i + ii + (j + jj) * 16] = 0;
                    }
                }
                emit_quad(inp, out, key, f, a, ua, va, sign, s, i as i32, j as i32, w as i32, h as i32);
                i += w;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_quad(inp: &MeshInput, out: &mut MeshData, key: u64, f: usize, a: usize, ua: usize, va: usize, sign: i32, s: i32, i0: i32, j0: i32, w: i32, h: i32) {
    let tile = (key & 0xFFFF) as u32;
    let ao = [((key >> 16) & 3) as u32, ((key >> 18) & 3) as u32, ((key >> 20) & 3) as u32, ((key >> 22) & 3) as u32];
    let sky = ((key >> 24) & 15) as f32 / 15.0;
    let emit = ((key >> 28) & 1) as u32;
    let trans = (key >> 29) & 1 == 1;

    let plane = (s + if sign > 0 { 1 } else { 0 }) as f32;
    let corners = [(i0, j0), (i0 + w, j0), (i0 + w, j0 + h), (i0, j0 + h)];
    let (verts, idx) = if trans { (&mut out.water, &mut out.water_idx) } else { (&mut out.opaque, &mut out.opaque_idx) };
    let base = verts.len() as u32;
    for (ci, (cu, cv)) in corners.iter().enumerate() {
        let mut g = [0.0f32; 3];
        g[a] = plane;
        g[ua] = *cu as f32;
        g[va] = *cv as f32;
        let pos = inp.to_local(g[0], g[1], g[2]);
        // Texture coordinates: image-up points away from the planet (+y).
        let (tu, tv) = if a == 1 {
            (*cu as f32, *cv as f32)
        } else if ua == 1 {
            (*cv as f32, -(*cu as f32))
        } else {
            (*cu as f32, -(*cv as f32))
        };
        let aof = [0.45, 0.65, 0.82, 1.0][ao[ci] as usize];
        verts.push(Vertex { pos, uv: [tu, tv], flags: tile | (emit << 16) | ((f as u32) << 17), light: [aof, sky] });
    }
    let flip = ao[0] + ao[2] < ao[1] + ao[3];
    let tri: [u32; 6] = match (sign > 0, flip) {
        (true, false) => [0, 1, 2, 0, 2, 3],
        (true, true) => [1, 2, 3, 1, 3, 0],
        (false, false) => [0, 2, 1, 0, 3, 2],
        (false, true) => [1, 3, 2, 1, 0, 3],
    };
    idx.extend(tri.iter().map(|t| base + t));
}

/// Crossed-quad plants (grass, flowers, ...), standing along the radial axis.
fn mesh_cross(inp: &MeshInput, out: &mut MeshData) {
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let b = inp.get(x, y, z);
                if b.info().render != RenderKind::Cross {
                    continue;
                }
                let sky = sky_at(inp, [x, y, z]) as f32 / 15.0;
                let tile = b.info().tiles[0] as u32;
                let verts = &mut out.opaque;
                let idx = &mut out.opaque_idx;
                // Two diagonal planes through the block centre; each emitted double-sided.
                for (c0, c1) in [((0.0f32, 0.0f32), (1.0f32, 1.0f32)), ((1.0, 0.0), (0.0, 1.0))] {
                    let base = verts.len() as u32;
                    for (t, hgt) in [(0.0f32, 0.0f32), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                        let gx = x as f32 + c0.0 + (c1.0 - c0.0) * t;
                        let gz = z as f32 + c0.1 + (c1.1 - c0.1) * t;
                        let gy = y as f32 + hgt;
                        let pos = inp.to_local(gx, gy, gz);
                        verts.push(Vertex { pos, uv: [t, 1.0 - hgt], flags: tile | (2 << 17), light: [1.0, sky] });
                    }
                    idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_with(blocks: &[(i32, i32, i32, Block)]) -> MeshInput {
        let mut m = MeshInput::new(ChunkKey::new(4, 40, 40, 64), Grid::new(1024.0), true);
        for (x, y, z, b) in blocks {
            m.set(*x, *y, *z, *b);
        }
        m
    }

    #[test]
    fn single_block_has_six_faces() {
        let m = mesh_chunk(&input_with(&[(5, 5, 5, Block::Stone)]));
        assert_eq!(m.opaque.len(), 24);
        assert_eq!(m.opaque_idx.len(), 36);
        assert!(m.water.is_empty());
    }

    #[test]
    fn adjacent_blocks_hide_shared_faces() {
        let m = mesh_chunk(&input_with(&[(5, 5, 5, Block::Stone), (6, 5, 5, Block::Stone)]));
        assert_eq!(m.quad_count(), 10, "two blocks: 12 faces minus the 2 touching ones");
    }

    #[test]
    fn big_slab_has_one_quad_per_cell_face_so_curved_edges_stay_watertight() {
        let mut blocks = Vec::new();
        for x in 0..16 {
            for z in 0..16 {
                blocks.push((x, 3, z, Block::Stone));
            }
        }
        let m = mesh_chunk(&input_with(&blocks));
        // Top + bottom: 256 each; four sides: 16 each.
        assert_eq!(m.quad_count(), 2 * 256 + 4 * 16);
    }

    #[test]
    fn water_goes_to_its_own_pass_and_plants_to_opaque() {
        let m = mesh_chunk(&input_with(&[(2, 2, 2, Block::Water), (8, 2, 8, Block::TallGrass)]));
        assert_eq!(m.water.len(), 24);
        assert_eq!(m.opaque.len(), 8);
        assert_eq!(m.opaque_idx.len(), 24);
    }

    #[test]
    fn transparent_leaves_do_not_hide_neighbours() {
        let m = mesh_chunk(&input_with(&[(4, 4, 4, Block::Stone), (5, 4, 4, Block::OakLeaves)]));
        assert_eq!(m.quad_count(), 11);
    }

    #[test]
    fn ambient_occlusion_darkens_inner_corner() {
        let m = mesh_chunk(&input_with(&[(5, 5, 5, Block::Stone), (6, 6, 5, Block::Stone)]));
        let min_ao = m.opaque.iter().map(|v| v.light[0]).fold(1.0, f32::min);
        assert!(min_ao < 1.0, "expected some occlusion");
    }

    #[test]
    fn covered_cells_are_darker_than_open_ones() {
        let top_sky = |m: &MeshData| m.opaque.iter().filter(|v| (v.flags >> 17) & 7 == 2).map(|v| v.light[1]).fold(1.0, f32::min);
        let open = mesh_chunk(&input_with(&[(5, 5, 5, Block::Stone)]));
        let covered = mesh_chunk(&input_with(&[(5, 5, 5, Block::Stone), (5, 8, 5, Block::Stone)]));
        // The covered stone's top face is shaded; the roof block itself is lit.
        assert!(top_sky(&covered) < top_sky(&open), "{} vs {}", top_sky(&covered), top_sky(&open));
    }

    #[test]
    fn vertices_are_small_chunk_relative_and_follow_the_sphere() {
        let m = mesh_chunk(&input_with(&[(0, 0, 0, Block::Stone), (15, 15, 15, Block::Stone)]));
        for v in &m.opaque {
            assert!(v.pos.iter().all(|c| c.is_finite() && c.abs() < 40.0), "{:?}", v.pos);
        }
        // Block faces are about one block wide: the bottom-left block spans ~1 unit.
        let xs: Vec<f32> = m.opaque.iter().take(24).map(|v| v.pos[0]).collect();
        let span = xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
        assert!((0.5..1.5).contains(&span), "block span {span}");
    }

    #[test]
    fn top_faces_point_away_from_the_planet_on_every_face() {
        // On each cube face, a block's top quad must be a "radial" patch: its centre is farther
        // from the origin than its bottom quad's centre.
        for f in 0..6u8 {
            let mut m = MeshInput::new(ChunkKey::new(f, 30, 12, 62), Grid::new(1024.0), true);
            m.set(7, 7, 7, Block::Stone);
            let data = mesh_chunk(&m);
            let r = m.grid.chunk_ref(m.key);
            let centre_of = |face: u32| {
                let vs: Vec<_> = data.opaque.iter().filter(|v| (v.flags >> 17) & 7 == face).collect();
                assert_eq!(vs.len(), 4);
                let c = vs.iter().fold(glam::DVec3::ZERO, |s, v| s + glam::DVec3::new(v.pos[0] as f64, v.pos[1] as f64, v.pos[2] as f64)) / 4.0;
                (c + r).length()
            };
            assert!(centre_of(2) > centre_of(3) + 0.9, "face {f}: top not above bottom");
        }
    }

    #[test]
    fn slab_is_a_half_height_box_and_does_not_hide_its_neighbours() {
        let m = mesh_chunk(&input_with(&[(5, 5, 5, Block::StoneSlab)]));
        assert_eq!(m.opaque.len(), 24, "six faces");
        // Compare with a full cube at the same place: the slab is about half as tall.
        let full = mesh_chunk(&input_with(&[(5, 5, 5, Block::Stone)]));
        let span = |v: &[Vertex]| {
            let up = glam::Vec3::from(v[0].pos).normalize();
            let h: Vec<f32> = v.iter().map(|p| glam::Vec3::from(p.pos).dot(up)).collect();
            h.iter().cloned().fold(f32::MIN, f32::max) - h.iter().cloned().fold(f32::MAX, f32::min)
        };
        let _ = span;
        let extent = |v: &[Vertex]| {
            let c = v.iter().fold(glam::Vec3::ZERO, |a, p| a + glam::Vec3::from(p.pos)) / v.len() as f32;
            v.iter().map(|p| (glam::Vec3::from(p.pos) - c).length()).fold(0.0f32, f32::max)
        };
        assert!(extent(&m.opaque) < extent(&full.opaque) - 0.05, "slab is smaller than a cube");
        // A stone block under the slab keeps its top face (the slab does not cover it).
        let both = mesh_chunk(&input_with(&[(5, 4, 5, Block::Stone), (5, 5, 5, Block::StoneSlab)]));
        assert_eq!(both.quad_count(), 6 + 12, "cube keeps 6 faces, slab adds 6 double-sided");
    }
}
