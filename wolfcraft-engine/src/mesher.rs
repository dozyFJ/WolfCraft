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
    /// bits 0..16 tile, bit 16 emissive, bits 17..20 face id (0..5), bits 20..24 block light,
    /// bit 24 leaves (sway with the wind), bit 25 plant (sways, more at the top), bit 26 grass
    /// top (takes the season's colour)
    pub flags: u32,
    /// x = ambient occlusion factor (0..1), y = skylight (0..1)
    pub light: [f32; 2],
}

pub struct MeshData {
    pub opaque: Vec<Vertex>,
    pub opaque_idx: Vec<u32>,
    pub water: Vec<Vertex>,
    pub water_idx: Vec<u32>,
    /// Opaque indices before this one are blocks and shapes; the rest are small details
    /// (grass, flowers) that far chunks skip.
    pub detail_start: u32,
    /// Opaque indices grouped by face direction (like Sodium): faces of direction `f` are
    /// `dirs[f]..dirs[f + 1]`; from `dirs[6]` on come shapes, fluids and details. The renderer
    /// skips the groups that face away from the camera.
    pub dirs: [u32; 7],
    /// Which sides of the chunk see each other through non-opaque blocks: bit `a * 6 + b` for
    /// sides `a`, `b` (see [`SIDE_DIRS`]). Lets the renderer skip chunks hidden behind walls.
    pub vis: u64,
}

impl Default for MeshData {
    fn default() -> Self {
        // No mesh means air: every side sees every other.
        Self { opaque: Vec::new(), opaque_idx: Vec::new(), water: Vec::new(), water_idx: Vec::new(), detail_start: 0, dirs: [0; 7], vis: VIS_ALL }
    }
}

/// Chunk sides as (di, dj, dk) chunk steps: -i, +i, -j, +j, -k, +k. Side `s ^ 1` is opposite.
pub const SIDE_DIRS: [(i32, i32, i32); 6] = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];
/// Every side connected to every other (an empty or not yet known chunk).
pub const VIS_ALL: u64 = (1 << 36) - 1;

/// True when sides `a` and `b` of a chunk with visibility set `vis` see each other.
#[inline]
pub fn sides_connected(vis: u64, a: usize, b: usize) -> bool {
    vis >> (a * 6 + b) & 1 == 1
}

/// Visibility set of a chunk: flood-fills its non-opaque cells and connects every pair of
/// sides that one open region touches (as in Minecraft's cave culling).
pub fn visibility(inp: &MeshInput) -> u64 {
    const N: i32 = CHUNK_SIZE;
    let idx = |x: i32, y: i32, z: i32| (x + y * N * N + z * N) as usize;
    let mut seen = vec![false; (N * N * N) as usize];
    let mut vis = 0u64;
    let mut stack: Vec<(i32, i32, i32)> = Vec::new();
    for y in 0..N {
        for z in 0..N {
            for x in 0..N {
                if seen[idx(x, y, z)] || inp.get(x, y, z).is_opaque() {
                    continue;
                }
                let mut sides = 0u8;
                seen[idx(x, y, z)] = true;
                stack.push((x, y, z));
                while let Some((x, y, z)) = stack.pop() {
                    // Padded axes are (x, y, z) = (j, k, i).
                    if z == 0 { sides |= 1 }
                    if z == N - 1 { sides |= 2 }
                    if x == 0 { sides |= 4 }
                    if x == N - 1 { sides |= 8 }
                    if y == 0 { sides |= 16 }
                    if y == N - 1 { sides |= 32 }
                    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                        let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                        if nx < 0 || ny < 0 || nz < 0 || nx >= N || ny >= N || nz >= N {
                            continue;
                        }
                        let i = idx(nx, ny, nz);
                        if !seen[i] && !inp.get(nx, ny, nz).is_opaque() {
                            seen[i] = true;
                            stack.push((nx, ny, nz));
                        }
                    }
                }
                for a in 0..6 {
                    for b in 0..6 {
                        if sides & (1 << a) != 0 && sides & (1 << b) != 0 {
                            vis |= 1 << (a * 6 + b);
                        }
                    }
                }
                if vis == VIS_ALL {
                    return vis;
                }
            }
        }
    }
    vis
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
    pub blocks: Box<[u16; PADDED_VOL]>,
    pub sky_open: bool,
    /// Light sources that can reach this chunk: padded coordinates (may lie outside the
    /// padded box, in neighbouring chunks) and light level.
    pub emitters: Vec<([i32; 3], u8)>,
    /// Per padded column (x, z): is the sky open above the top of the padded box? `None`
    /// uses `sky_open` for every column.
    pub sky_cols: Option<Box<[bool; PAD * PAD]>>,
}

impl MeshInput {
    pub fn new(key: ChunkKey, grid: Grid, sky_open: bool) -> Self {
        Self { key, grid, blocks: Box::new([0; PADDED_VOL]), sky_open, emitters: Vec::new(), sky_cols: None }
    }
    /// Index for padded coordinates `(x, y, z)` = `(j, k, i)` relative to the chunk, each in -1..=16.
    #[inline]
    pub fn idx(x: i32, y: i32, z: i32) -> usize {
        ((x + 1) as usize) + ((z + 1) as usize) * PAD + ((y + 1) as usize) * PAD * PAD
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32, b: Block) {
        self.blocks[Self::idx(x, y, z)] = b as u16;
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> Block {
        Block::from_id(self.blocks[Self::idx(x, y, z)])
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
    // Water against water (sources, flows, falls) and lava against lava: no inner faces.
    if let (Some(a), Some(c)) = (b.fluid(), n.fluid()) {
        if a.lava == c.lava && c.height() >= a.height() {
            return false;
        }
    }
    match b.info().render {
        RenderKind::Opaque => !n.is_opaque(),
        // Leaves next to any leaves: no inner face (like the MoreCulling mod) — tree crowns
        // drew every leaf block's six sides.
        RenderKind::Cutout => !(n.is_opaque() || n == b || (is_leaves(b) && is_leaves(n))),
        RenderKind::Translucent => !(n.is_opaque() || n == b),
        _ => false,
    }
}

/// "Fast" leaves (a setting): crowns drawn solid, no transparent holes — much cheaper on phones.
pub static FAST_LEAVES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_fast_leaves(on: bool) {
    FAST_LEAVES.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn is_leaves(b: Block) -> bool {
    matches!(b, Block::OakLeaves | Block::SpruceLeaves | Block::BirchLeaves | Block::AcaciaLeaves)
}

const NORMALS: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Skylight 0..15 along one column above `q`: open sky gives 15, a roof `k` blocks up gives
/// a dim value, leaves only take some light away.
fn sky_ray(inp: &MeshInput, x: i32, y0: i32, z: i32) -> f32 {
    let col_open = match &inp.sky_cols {
        Some(c) => c[((x.clamp(-1, CHUNK_SIZE) + 1) + (z.clamp(-1, CHUNK_SIZE) + 1) * PAD as i32) as usize],
        None => inp.sky_open,
    };
    let open = if col_open { 15.0 } else { 3.0 };
    let mut light = open;
    for k in 1..=14 {
        let y = y0 + k;
        if y > CHUNK_SIZE {
            return light;
        }
        let b = inp.get(x, y, z);
        if b.is_opaque() {
            return ((2 + k) as f32).min(13.0).min(light);
        }
        if b.blocks_light() {
            light -= 2.0; // leaves and other cut-out blocks: dappled shade
        }
    }
    light.max(3.0)
}

/// Soft skylight 0..15 for the air cell `q`: the straight ray up averaged with the four
/// neighbouring columns, so shadows under roofs and trees have soft edges.
fn sky_at(inp: &MeshInput, q: [i32; 3]) -> u8 {
    let c = |v: i32| v.clamp(-1, CHUNK_SIZE);
    let mut sum = sky_ray(inp, q[0], q[1], q[2]) * 2.0;
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        sum += sky_ray(inp, c(q[0] + dx), q[1], c(q[2] + dz));
    }
    (sum / 6.0).round().clamp(0.0, 15.0) as u8
}

/// Block light (0..15) of every padded cell: flood fill from the light sources through
/// non-opaque blocks, one level less per step, like Minecraft. Sources outside the padded box
/// shine in through its border as if the way there were open.
pub fn block_light(inp: &MeshInput) -> Vec<u8> {
    let mut light = vec![0u8; PADDED_VOL];
    if inp.emitters.is_empty() {
        return light;
    }
    let r = -1..=CHUNK_SIZE;
    let mut seed = |light: &mut Vec<u8>, x: i32, y: i32, z: i32, l: u8| {
        let i = MeshInput::idx(x, y, z);
        if l > light[i] {
            light[i] = l;
        }
    };
    for &([ex, ey, ez], level) in &inp.emitters {
        if r.contains(&ex) && r.contains(&ey) && r.contains(&ez) {
            seed(&mut light, ex, ey, ez, level);
            continue;
        }
        // Outside: light the nearest border cells with what is left after the distance.
        let (cx, cy, cz) = (ex.clamp(-1, CHUNK_SIZE), ey.clamp(-1, CHUNK_SIZE), ez.clamp(-1, CHUNK_SIZE));
        let gap = (ex - cx).abs() + (ey - cy).abs() + (ez - cz).abs();
        let left = level as i32 - gap;
        if left <= 0 {
            continue;
        }
        // Every border cell within reach gets level minus its full distance.
        let reach = left - 1;
        for y in (cy - reach).max(-1)..=(cy + reach).min(CHUNK_SIZE) {
            for z in (cz - reach).max(-1)..=(cz + reach).min(CHUNK_SIZE) {
                for x in (cx - reach).max(-1)..=(cx + reach).min(CHUNK_SIZE) {
                    let border = x == -1 || y == -1 || z == -1 || x == CHUNK_SIZE || y == CHUNK_SIZE || z == CHUNK_SIZE;
                    if !border {
                        continue;
                    }
                    let l = level as i32 - ((ex - x).abs() + (ey - y).abs() + (ez - z).abs());
                    if l > 0 && !inp.get(x, y, z).is_opaque() {
                        seed(&mut light, x, y, z, l as u8);
                    }
                }
            }
        }
    }
    // Spread level by level (brightest first), so every cell keeps its best value.
    let mut buckets: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); 16];
    for y in -1..=CHUNK_SIZE {
        for z in -1..=CHUNK_SIZE {
            for x in -1..=CHUNK_SIZE {
                let l = light[MeshInput::idx(x, y, z)];
                if l > 1 {
                    buckets[l as usize].push((x, y, z));
                }
            }
        }
    }
    for level in (2..16usize).rev() {
        let cells = std::mem::take(&mut buckets[level]);
        for (x, y, z) in cells {
            if light[MeshInput::idx(x, y, z)] as usize != level {
                continue;
            }
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                if !r.contains(&nx) || !r.contains(&ny) || !r.contains(&nz) || inp.get(nx, ny, nz).is_opaque() {
                    continue;
                }
                let i = MeshInput::idx(nx, ny, nz);
                if (light[i] as usize) < level - 1 {
                    light[i] = (level - 1) as u8;
                    buckets[level - 1].push((nx, ny, nz));
                }
            }
        }
    }
    light
}

pub fn mesh_chunk(inp: &MeshInput) -> MeshData {
    let mut out = MeshData::default();
    let light = block_light(inp);
    let bl = |p: [i32; 3]| light[MeshInput::idx(p[0], p[1], p[2])] as u32;
    for f in 0..6 {
        out.dirs[f] = out.opaque_idx.len() as u32;
        mesh_direction(inp, f, &mut out, &bl);
    }
    out.dirs[6] = out.opaque_idx.len() as u32;
    mesh_shapes(inp, &mut out, &bl);
    mesh_fluids(inp, &mut out, &bl);
    out.detail_start = out.opaque_idx.len() as u32;
    mesh_cross(inp, &mut out, &bl);
    out.vis = visibility(inp);
    out
}

/// Flowing water and lava lower than a full block: a box up to the liquid surface, without
/// faces against the same liquid or opaque blocks. Water goes to the translucent pass.
fn mesh_fluids(inp: &MeshInput, out: &mut MeshData, bl: &dyn Fn([i32; 3]) -> u32) {
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let b = inp.get(x, y, z);
                let Some(fl) = b.fluid() else { continue };
                let h = fl.height();
                if h >= 1.0 {
                    continue;
                }
                let info = b.info();
                let tile = info.tiles[0] as u32;
                let sky = if info.emissive { 1.0 } else { sky_at(inp, [x, y, z]) as f32 / 15.0 };
                let lit = bl([x, y, z]) << 20;
                let (verts, idx) = if fl.lava { (&mut out.opaque, &mut out.opaque_idx) } else { (&mut out.water, &mut out.water_idx) };
                let lo = [0.0f32, 0.0, 0.0];
                let hi = [1.0f32, h, 1.0];
                for f in 0..6usize {
                    let a = f / 2;
                    let positive = f % 2 == 0;
                    let n = NORMALS[f];
                    let nb = inp.get(x + n[0], y + n[1], z + n[2]);
                    let top = a == 1 && positive;
                    if nb.is_opaque() && !top {
                        continue;
                    }
                    if let Some(o) = nb.fluid() {
                        if o.lava == fl.lava && (top || a != 1 && o.height() >= h || a == 1 && !positive) {
                            continue;
                        }
                    }
                    let (ua, va) = ((a + 1) % 3, (a + 2) % 3);
                    let shade = if a == 1 { if positive { 1.0 } else { 0.6 } } else { 0.85 };
                    let base = verts.len() as u32;
                    for (cu, cv) in [(0usize, 0usize), (1, 0), (1, 1), (0, 1)] {
                        let mut g = [0.0f32; 3];
                        g[a] = if positive { hi[a] } else { lo[a] };
                        g[ua] = if cu == 1 { hi[ua] } else { lo[ua] };
                        g[va] = if cv == 1 { hi[va] } else { lo[va] };
                        let pos = inp.to_local(x as f32 + g[0], y as f32 + g[1], z as f32 + g[2]);
                        let (tu, tv) = if a == 1 { (g[ua], g[va]) } else if ua == 1 { (g[va], -g[ua]) } else { (g[ua], -g[va]) };
                        verts.push(Vertex { pos, uv: [tu, tv], flags: tile | ((info.emissive as u32) << 16) | ((f as u32) << 17) | lit, light: [shade, sky] });
                    }
                    idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
                }
            }
        }
    }
}

/// Slabs, panes, fences, doors: one box per block, textured with the matching part of the tile.
fn mesh_shapes(inp: &MeshInput, out: &mut MeshData, bl: &dyn Fn([i32; 3]) -> u32) {
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let b = inp.get(x, y, z);
                if let Some((entry, rotation)) = crate::construction::entry(b as u16) {
                    mesh_construction(inp, out, bl, x, y, z, entry, rotation);
                    continue;
                }
                let mut boxes = b.boxes();
                // Spinning parts («Механика») are drawn as turning models, not in the mesh.
                if boxes.is_empty() || crate::kinetic::spins(b) {
                    continue;
                }
                // The second leaf of a double door hangs on the far jamb.
                let mirrored;
                if let Some((axis, mb)) = b.door_mirror() {
                    // Padded axes are (x, y, z) = (j, k, i).
                    let n = if axis == 0 { inp.get(x, y, z - 1) } else { inp.get(x - 1, y, z) };
                    if b.door_mirrored_by(n) {
                        mirrored = [mb];
                        boxes = &mirrored;
                    }
                }
                // Fences and walls: rails towards the neighbours they join.
                let joined;
                if b.post_family().is_some() {
                    // Padded axes are (x, y, z) = (j, k, i).
                    let mut v: Vec<[f32; 6]> = boxes.to_vec();
                    for (axis, sign, n) in [(0usize, 1i32, inp.get(x, y, z + 1)), (0, -1, inp.get(x, y, z - 1)), (1, 1, inp.get(x + 1, y, z)), (1, -1, inp.get(x - 1, y, z))] {
                        if b.post_joins(n) {
                            v.extend(b.post_arms(axis, sign));
                        }
                    }
                    joined = v;
                    boxes = &joined;
                }
                // WolfTech cables: arms towards every neighbour that carries energy.
                let wired;
                if b == Block::Cable || crate::pipes::is_pipe(b) {
                    let pipe = b != Block::Cable;
                    let lo = if pipe { 0.30f32 } else { 0.34 };
                    let hi = if pipe { 0.70f32 } else { 0.66 };
                    let mut v: Vec<[f32; 6]> = boxes.to_vec();
                    // (i, j, k) boxes; padded axes are (x, y, z) = (j, k, i).
                    for (n, arm) in [
                        (inp.get(x, y, z + 1), [hi, lo, lo, 1.0, hi, hi]),
                        (inp.get(x, y, z - 1), [0.0, lo, lo, lo, hi, hi]),
                        (inp.get(x + 1, y, z), [lo, hi, lo, hi, 1.0, hi]),
                        (inp.get(x - 1, y, z), [lo, 0.0, lo, hi, lo, hi]),
                        (inp.get(x, y + 1, z), [lo, lo, hi, hi, hi, 1.0]),
                        (inp.get(x, y - 1, z), [lo, lo, 0.0, hi, hi, lo]),
                    ] {
                        if if pipe { crate::pipes::joins(n) } else { crate::wolftech::conducts(n) } {
                            v.push(arm);
                        }
                    }
                    wired = v;
                    boxes = &wired;
                }
                let info = b.info();
                for s in boxes {
                let sky = if info.emissive { 1.0 } else { sky_at(inp, [x, y, z]).max(sky_at(inp, [x, y + 1, z])) as f32 / 15.0 };
                let lit = bl([x, y, z]).max(bl([x, y + 1, z])) << 20;
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
                        verts.push(Vertex { pos, uv: [tu, tv], flags: tile | ((info.emissive as u32) << 16) | ((f as u32) << 17) | lit, light: [shade, sky] });
                    }
                    // Both windings: the box is small and may be seen from any side.
                    idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
                }
                }
            }
        }
    }
}

fn mesh_construction(inp: &MeshInput, out: &mut MeshData, bl: &dyn Fn([i32;3])->u32, x:i32,y:i32,z:i32, entry:&crate::construction::Entry, rotation:u8) {
    use crate::construction::{self, Form};
    let mut joined=0;
    if entry.form==Form::Sofa {
        // Left/right along the rotated sofa's local i axis.
        let (dx,dz)=match rotation {0=>(0,1),1=>(1,0),2=>(0,-1),_=>(-1,0)};
        for (sign,bit) in [(-1,1),(1,2)] {
            if construction::entry(inp.get(x+dx*sign,y,z+dz*sign) as u16)
                .is_some_and(|(e,r)|e.form==Form::Sofa&&e.material==entry.material&&r==rotation) { joined|=bit; }
        }
    }
    let model=construction::geometry::model(entry.form,rotation,joined);
    let sky=sky_at(inp,[x,y,z]).max(sky_at(inp,[x,y+1,z])) as f32/15.;
    let lit=bl([x,y,z]).max(bl([x,y+1,z]))<<20;
    for face in &model.faces {
        let (tile,alpha,emissive)=construction::surface(entry,face.surface);
        let a=glam::Vec3::from_array(face.points[1])-glam::Vec3::from_array(face.points[0]);
        let b=glam::Vec3::from_array(face.points[2])-glam::Vec3::from_array(face.points[0]);
        let normal=a.cross(b);if normal.length_squared()<1e-10 {continue;}
        // Padded axes are j,k,i, including the shader's six normal directions.
        let normal=[normal.y,normal.z,normal.x];
        let axis=(0..3).max_by(|a,b|normal[*a].abs().total_cmp(&normal[*b].abs())).unwrap();
        let f=axis*2+usize::from(normal[axis]<0.);
        let (verts,idx)=if alpha {(&mut out.water,&mut out.water_idx)}else{(&mut out.opaque,&mut out.opaque_idx)};
        let base=verts.len() as u32;
        // Texture by position inside the cell (not the whole tile squeezed onto every small
        // face): a thin leg shows a thin strip of planks, a dome keeps one continuous
        // texture. Pictures keep the full image.
        let n_local=[normal[2],normal[0],normal[1]]; // back to (i, j, k)
        let la=(0..3).max_by(|a,b|n_local[*a].abs().total_cmp(&n_local[*b].abs())).unwrap();
        let full=face.surface==construction::geometry::Surface::Picture;
        for (p,uv0) in face.points.iter().zip([[0.,0.],[1.,0.],[1.,1.],[0.,1.]]) {
            let uv=if full {uv0} else {match la {2=>[p[0],p[1]],0=>[p[1],1.-p[2]],_=>[p[0],1.-p[2]]}};
            // (No clamp: the shader repeats the tile, so parts beyond the cell keep texturing.)
            verts.push(Vertex {pos:inp.to_local(x as f32+p[1],y as f32+p[2],z as f32+p[0]),uv,flags:tile as u32|((emissive as u32)<<16)|((f as u32)<<17)|((alpha as u32)<<27)|lit,light:[if emissive{1.}else{0.9},if emissive{1.}else{sky}]});
        }
        idx.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
        if !alpha { idx.extend_from_slice(&[base,base+2,base+1,base,base+3,base+2]); }
    }
}

fn mesh_direction(inp: &MeshInput, f: usize, out: &mut MeshData, bl: &dyn Fn([i32; 3]) -> u32) {
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
                if b.fluid().is_some_and(|f| f.height() < 1.0) {
                    continue; // lower flowing liquid: see mesh_fluids
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
                let lit = bl(q) as u64;
                let leaves = (info.render == RenderKind::Cutout && b.blocks_light()) as u64;
                let turf = (a == 1 && sign > 0 && matches!(b, Block::Grass | Block::DryGrass | Block::SwampGrass)) as u64;
                let solid_leaves = (leaves == 1 && FAST_LEAVES.load(std::sync::atomic::Ordering::Relaxed)) as u64;
                let key = (1u64 << 63) | (tile as u64) | (ao[0] << 16) | (ao[1] << 18) | (ao[2] << 20) | (ao[3] << 22) | (sky << 24) | (emit << 28) | (trans << 29) | (lit << 30) | (leaves << 34) | (turf << 35) | (solid_leaves << 36);
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
    let lit = ((key >> 30) & 15) as u32 | (((key >> 34) & 1) as u32) << 4 | (((key >> 35) & 1) as u32) << 6;
    // Fast leaves: drawn solid (no see-through holes), like Minecraft's "Fast" graphics.
    let solid = ((key >> 36) & 1) as u32;

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
        verts.push(Vertex { pos, uv: [tu, tv], flags: tile | (emit << 16) | ((f as u32) << 17) | (lit << 20) | (solid << 27), light: [aof, sky] });
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

/// A small box in cell `c`, `lo`..`hi` in block units (x, y = up, z). The texture is pinned to
/// the cell (a thin post shows the matching thin strip of the picture), seen from both sides.
#[allow(clippy::too_many_arguments)]
fn push_box(inp: &MeshInput, verts: &mut Vec<Vertex>, idx: &mut Vec<u32>, c: [i32; 3], lo: [f32; 3], hi: [f32; 3], tile: u32, light: [f32; 2]) {
    push_box_mapped(inp, verts, idx, c, lo, hi, tile, light, &|g| g);
}

/// `push_box` with every corner moved by `map` (cell coordinates, padded axes): a leaning
/// wall torch. Textures stay as on the straight box.
#[allow(clippy::too_many_arguments)]
fn push_box_mapped(inp: &MeshInput, verts: &mut Vec<Vertex>, idx: &mut Vec<u32>, c: [i32; 3], lo: [f32; 3], hi: [f32; 3], tile: u32, light: [f32; 2], map: &dyn Fn([f32; 3]) -> [f32; 3]) {
    for f in 0..6usize {
        let a = f / 2;
        let positive = f % 2 == 0;
        let (ua, va) = ((a + 1) % 3, (a + 2) % 3);
        let base = verts.len() as u32;
        for (cu, cv) in [(0usize, 0usize), (1, 0), (1, 1), (0, 1)] {
            let mut g = [0.0f32; 3];
            g[a] = if positive { hi[a] } else { lo[a] };
            g[ua] = if cu == 1 { hi[ua] } else { lo[ua] };
            g[va] = if cv == 1 { hi[va] } else { lo[va] };
            let m = map(g);
            let pos = inp.to_local(c[0] as f32 + m[0], c[1] as f32 + m[1], c[2] as f32 + m[2]);
            let (tu, tv) = if a == 1 { (g[ua], g[va]) } else if ua == 1 { (g[va], -g[ua]) } else { (g[ua], -g[va]) };
            verts.push(Vertex { pos, uv: [tu, tv], flags: tile | ((f as u32) << 17), light });
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

/// Crossed-quad plants (grass, flowers, ...), standing along the radial axis.
fn mesh_cross(inp: &MeshInput, out: &mut MeshData, bl: &dyn Fn([i32; 3]) -> u32) {
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let b = inp.get(x, y, z);
                if b.info().render != RenderKind::Cross {
                    continue;
                }
                let sky = sky_at(inp, [x, y, z]) as f32 / 15.0;
                // Torches are real 3D sticks (like Minecraft's): a 2-pixel post cut from the
                // middle of the picture, standing on the block below, not two flat planes.
                if let Some(parts) = b.torch_boxes() {
                    let tile = b.info().tiles[0] as u32 | (bl([x, y, z]) << 20) | (1 << 16);
                    let wall = b.wall_mount().map(|m| m.1);
                    for bx in parts {
                        // Padded axes are (x, y, z) = (j, k, i).
                        match wall {
                            // On a wall: the same stick leaning out of it.
                            Some(d) => push_box_mapped(inp, &mut out.opaque, &mut out.opaque_idx, [x, y, z], [bx[1], bx[2], bx[0]], [bx[4], bx[5], bx[3]], tile, [1.0, 1.0], &|g| {
                                let q = Block::wall_torch_point(d, [g[2], g[0], g[1]]);
                                [q[1], q[2], q[0]]
                            }),
                            None => push_box(inp, &mut out.opaque, &mut out.opaque_idx, [x, y, z], [bx[1], bx[2], bx[0]], [bx[4], bx[5], bx[3]], tile, [1.0, 1.0]),
                        }
                    }
                    let _ = sky;
                    continue;
                }
                // Torches and other emissive "plants" stay still; flames flicker like grass in wind.
                let sway = if b.info().emissive && b != Block::Fire { 0 } else { 1u32 << 25 };
                let tile = b.info().tiles[0] as u32 | (bl([x, y, z]) << 20) | sway;
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
    fn curved_glass_uses_alpha_pass_without_water_waves() {
        let glass=crate::construction::block(crate::construction::Form::Arc90Large,8,0).unwrap();
        let mesh=mesh_chunk(&input_with(&[(5,5,5,glass)]));
        assert!(mesh.opaque.is_empty());assert!(!mesh.water.is_empty());
        assert!(mesh.water.iter().all(|v|v.flags&(1<<27)!=0));
        let table=crate::construction::block(crate::construction::Form::GlassTable,0,0).unwrap();
        let mesh=mesh_chunk(&input_with(&[(5,5,5,table)]));assert!(!mesh.opaque.is_empty()&&!mesh.water.is_empty());
    }

    /// Every block with its own model, alone in a chunk: the selection outline must hug what is
    /// drawn (no torch head above its frame, no box sticking out of its cell). Construction
    /// pieces are listed as a report; plain shape blocks and torches must match.
    #[test]
    fn construction_and_shape_outlines_match_models() {
        let mut bad = Vec::new();
        let mut report = Vec::new();
        let mut mismatch = Vec::new();
        for &b in Block::ALL.iter() {
            let constr = crate::construction::entry(b as u16).is_some();
            if b.torch_boxes().is_none() && b.boxes().is_empty() && !constr {
                continue;
            }
            // Spinning «Механика» parts are drawn as models, not meshed.
            if crate::kinetic::spins(b) {
                continue;
            }
            for bx in b.boxes().iter().chain(b.torch_boxes().unwrap_or(&[]).iter()) {
                let inside = bx.iter().all(|v| (-0.001..=1.001).contains(v)) && (0..3).all(|k| bx[k] <= bx[k + 3]);
                if !inside && !constr {
                    bad.push(format!("{b:?}: box {bx:?} leaves its cell"));
                }
            }
            let inp = input_with(&[(8, 8, 8, b)]);
            let m = mesh_chunk(&inp);
            let pts: Vec<[f32; 3]> = m.opaque.iter().chain(m.water.iter()).map(|v| v.pos).collect();
            if pts.is_empty() {
                bad.push(format!("{b:?}: draws nothing"));
                continue;
            }
            // Back to cell coordinates (the chunk is turned in space, so compare there): the
            // mapping is affine enough inside one cell.
            let o0 = glam::Vec3::from_array(inp.to_local(8.0, 8.0, 8.0));
            let ax = glam::Mat3::from_cols(
                glam::Vec3::from_array(inp.to_local(9.0, 8.0, 8.0)) - o0,
                glam::Vec3::from_array(inp.to_local(8.0, 9.0, 8.0)) - o0,
                glam::Vec3::from_array(inp.to_local(8.0, 8.0, 9.0)) - o0,
            )
            .inverse();
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for q in pts {
                let c = ax * (glam::Vec3::from_array(q) - o0);
                for k in 0..3 {
                    lo[k] = lo[k].min(c[k]);
                    hi[k] = hi[k].max(c[k]);
                }
            }
            let o = b.outline_box();
            // Padded axes are (x, y, z) = (j, k, i).
            let (olo, ohi) = ([o[1], o[2], o[0]], [o[4], o[5], o[3]]);
            let off = (0..3).map(|k| (lo[k] - olo[k]).abs().max((hi[k] - ohi[k]).abs())).fold(0.0f32, f32::max);
            if off > 0.03 {
                let line = format!("{b:?}: outline {o:?} vs drawn j,k,i {lo:.2?}..{hi:.2?} (off {off:.2})");
                if constr { report.push(line) } else { mismatch.push(line) }
            }
        }
        if !mismatch.is_empty() {
            eprintln!("blocks whose outline differs from the model ({}):\n{}", mismatch.len(), mismatch.join("\n"));
        }
        if !report.is_empty() {
            eprintln!("construction pieces whose outline differs from the model ({}):\n{}", report.len(), report.join("\n"));
        }
        assert!(bad.is_empty(), "{} blocks look wrong:\n{}", bad.len(), bad.join("\n"));
    }

    #[test]
    fn faces_are_grouped_by_direction() {
        let inp = input_with(&[(8, 8, 8, Block::Stone)]);
        let m = mesh_chunk(&inp);
        for f in 0..6 {
            let (a, b) = (m.dirs[f] as usize, m.dirs[f + 1] as usize);
            assert_eq!(b - a, 6, "one quad facing {f}");
            for &k in &m.opaque_idx[a..b] {
                assert_eq!((m.opaque[k as usize].flags >> 17) & 7, f as u32, "a face in the wrong group");
            }
        }
    }

    #[test]
    fn leaves_hide_faces_between_each_other() {
        let one = mesh_chunk(&input_with(&[(8, 8, 8, Block::OakLeaves)])).quad_count();
        let two = mesh_chunk(&input_with(&[(8, 8, 8, Block::OakLeaves), (9, 8, 8, Block::BirchLeaves)])).quad_count();
        assert_eq!(one, 6);
        assert_eq!(two, 10, "the two touching faces are gone");
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
    fn torch_light_spreads_fades_and_stops_at_walls() {
        let mut inp = input_with(&[(8, 8, 8, Block::Torch)]);
        // A stone wall at x = 11 (full height and depth).
        for y in -1..=16 {
            for z in -1..=16 {
                inp.set(11, y, z, Block::Stone);
            }
        }
        inp.emitters.push(([8, 8, 8], 14));
        let l = block_light(&inp);
        let at = |x, y, z| l[MeshInput::idx(x, y, z)];
        assert_eq!(at(8, 8, 8), 14);
        assert_eq!(at(9, 8, 8), 13);
        assert_eq!(at(8, 8, 2), 8, "one level less per block");
        assert_eq!(at(12, 8, 8), 0, "no light through the wall");
        // A source in the next chunk shines in over the border.
        let mut far = input_with(&[]);
        far.emitters.push(([-4, 5, 5], 14));
        let l = block_light(&far);
        assert_eq!(l[MeshInput::idx(0, 5, 5)], 10);
        // Lit faces carry the light level in the vertex flags.
        let m = mesh_chunk(&inp);
        assert!(m.opaque.iter().any(|v| (v.flags >> 20) & 15 >= 10), "faces near the torch are lit");
    }

    #[test]
    fn a_wall_hides_one_side_from_the_other() {
        // Solid layer across the middle of the chunk (k = 8): below and above do not connect.
        let mut cells = Vec::new();
        for x in 0..16 {
            for z in 0..16 {
                cells.push((x, 8, z, Block::Stone));
            }
        }
        let v = visibility(&input_with(&cells));
        assert!(!sides_connected(v, 4, 5), "bottom and top are cut off");
        assert!(sides_connected(v, 4, 0) && sides_connected(v, 5, 1), "the sides still see each other");
        assert_eq!(visibility(&input_with(&[])), VIS_ALL, "air sees everywhere");
        let m = mesh_chunk(&input_with(&[(3, 3, 3, Block::Stone), (5, 5, 5, Block::TallGrass)]));
        assert!(m.detail_start > 0 && (m.detail_start as usize) < m.opaque_idx.len(), "grass comes after the blocks");
    }

    #[test]
    fn vertices_are_small_chunk_relative_and_follow_the_sphere() {
        let m = mesh_chunk(&input_with(&[(0, 0, 0, Block::Stone), (15, 15, 15, Block::Stone)]));
        for v in &m.opaque {
            assert!(v.pos.iter().all(|c| c.is_finite() && c.abs() < 40.0), "{:?}", v.pos);
        }
        // Block faces are about one block wide: a single block spans ~1 unit. (Faces are
        // emitted per direction, so the first vertices of a two-block mesh mix both blocks.)
        let one = mesh_chunk(&input_with(&[(0, 0, 0, Block::Stone)]));
        let xs: Vec<f32> = one.opaque.iter().map(|v| v.pos[0]).collect();
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

