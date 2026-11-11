//! Cube-sphere voxel grid.
//!
//! The planet is six square faces of a cube projected onto a sphere (equal-angle mapping).
//! Every face has its own local grid `(i, j, k)`: `i`/`j` run along the face and `k` is the
//! radial layer (distance from the planet centre in blocks). A block is therefore always
//! aligned with the local "up", so terrain, trees and buildings stand upright everywhere, and
//! walking over a face edge needs no special handling: neighbouring faces line up cell by cell.

use glam::{DVec3, IVec3};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

/// Face normals, in the order +X, -X, +Y, -Y, +Z, -Z.
pub const NORMALS: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
/// Tangent axes per face (`a` increases with `i`, `b` with `j`); `a × b = normal`.
pub const TAN_A: [IVec3; 6] = [IVec3::Y, IVec3::Z, IVec3::Z, IVec3::X, IVec3::X, IVec3::Y];
pub const TAN_B: [IVec3; 6] = [IVec3::Z, IVec3::Y, IVec3::X, IVec3::Z, IVec3::Y, IVec3::X];

pub const CHUNK: i32 = 16;

/// One block position: face, tangent indices and radial layer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Cell {
    pub f: u8,
    pub i: i32,
    pub j: i32,
    pub k: i32,
}

impl Cell {
    pub fn new(f: u8, i: i32, j: i32, k: i32) -> Self {
        Self { f, i, j, k }
    }
}

/// A 16×16×16 chunk of a face.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkKey {
    pub f: u8,
    pub ci: i32,
    pub cj: i32,
    pub ck: i32,
}

impl ChunkKey {
    pub fn new(f: u8, ci: i32, cj: i32, ck: i32) -> Self {
        Self { f, ci, cj, ck }
    }
    pub fn of(c: Cell) -> Self {
        Self { f: c.f, ci: c.i >> 4, cj: c.j >> 4, ck: c.k >> 4 }
    }
    /// Index-space origin (smallest i, j, k) of the chunk.
    pub fn origin(&self) -> IVec3 {
        IVec3::new(self.ci * CHUNK, self.cj * CHUNK, self.ck * CHUNK)
    }
}

/// Position of a block inside its chunk → index into the chunk array.
#[inline]
pub fn local_idx(li: i32, lj: i32, lk: i32) -> usize {
    ((lk << 8) | (lj << 4) | li) as usize
}

#[inline]
pub fn idx_to_local(idx: usize) -> (i32, i32, i32) {
    ((idx & 15) as i32, ((idx >> 4) & 15) as i32, (idx >> 8) as i32)
}

#[derive(Clone, Copy, Debug)]
pub struct Obb {
    pub center: DVec3,
    /// Orthonormal axes (tangent i, tangent j, radial up).
    pub axes: [DVec3; 3],
    pub half: [f64; 3],
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Grid {
    /// Cells along one face edge (multiple of 16).
    pub n: i32,
    /// Cube planet: flat faces with rounded edges (a superellipsoid) instead of a sphere.
    pub cube: bool,
}

/// Exponent of the rounded cube |x|^P + |y|^P + |z|^P = r^P: high = sharper edges.
const CUBE_P: f64 = 8.0;

static CUBE_WORLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Shape of the world being played (set when a world loads). Used by [`radial`] and [`up`],
/// which are called all over the game without a grid at hand.
pub fn set_cube_world(cube: bool) {
    CUBE_WORLD.store(cube, std::sync::atomic::Ordering::Relaxed);
}

pub fn cube_world() -> bool {
    CUBE_WORLD.load(std::sync::atomic::Ordering::Relaxed)
}

/// Rounded-cube "radius" of `p` (the P-norm).
pub fn cube_radial(p: DVec3) -> f64 {
    let a = p.abs();
    let m = a.max_element();
    if m <= 0.0 {
        return 0.0;
    }
    let q = a / m;
    m * (q.x.powf(CUBE_P) + q.y.powf(CUBE_P) + q.z.powf(CUBE_P)).powf(1.0 / CUBE_P)
}

/// Surface normal of the rounded cube through `p`.
pub fn cube_up(p: DVec3) -> DVec3 {
    let m = p.abs().max_element();
    if m <= 0.0 {
        return DVec3::Y;
    }
    let q = p / m;
    let g = DVec3::new(q.x.signum() * q.x.abs().powf(CUBE_P - 1.0), q.y.signum() * q.y.abs().powf(CUBE_P - 1.0), q.z.signum() * q.z.abs().powf(CUBE_P - 1.0));
    g.normalize()
}

/// Height measure of `p` in the current world: distance from the centre on a sphere,
/// the rounded-cube radius on a cube planet. Terrain layers (`k`) use the same measure.
pub fn radial(p: DVec3) -> f64 {
    if cube_world() { cube_radial(p) } else { p.length() }
}

/// Local "up" at `p` in the current world.
pub fn up(p: DVec3) -> DVec3 {
    if cube_world() {
        cube_up(p)
    } else {
        let l = p.length();
        if l < 1e-9 { DVec3::Y } else { p / l }
    }
}

/// Point at height `r` in direction `dir` (unit) in the current world.
pub fn along(dir: DVec3, r: f64) -> DVec3 {
    if cube_world() { dir * (r / cube_radial(dir)) } else { dir * r }
}

impl Grid {
    /// Grid sized so that one cell is about one block wide at sea level `radius`.
    pub fn new(radius: f64) -> Self {
        let n = ((radius * FRAC_PI_2 / CHUNK as f64).ceil() as i32).max(1) * CHUNK;
        Self { n, cube: false }
    }

    /// Grid of a cube planet whose faces sit `radius` blocks from the centre (edge = 2·radius).
    pub fn new_cube(radius: f64) -> Self {
        let n = ((radius * 2.0 / CHUNK as f64).ceil() as i32).max(1) * CHUNK;
        Self { n, cube: true }
    }

    pub fn with_cells(n: i32) -> Self {
        assert!(n % CHUNK == 0 && n > 0);
        Self { n, cube: false }
    }

    /// Height measure of `p` for this grid's shape.
    pub fn radial(&self, p: DVec3) -> f64 {
        if self.cube { cube_radial(p) } else { p.length() }
    }

    /// Local up at `p` for this grid's shape.
    pub fn up(&self, p: DVec3) -> DVec3 {
        if self.cube { cube_up(p) } else { p.normalize_or(DVec3::Y) }
    }

    /// Point at height `r` along the unit direction `dir`.
    pub fn along(&self, dir: DVec3, r: f64) -> DVec3 {
        if self.cube { dir * (r / cube_radial(dir)) } else { dir * r }
    }

    pub fn chunks_per_edge(&self) -> i32 {
        self.n / CHUNK
    }

    #[inline]
    fn tan_at(&self, c: f64) -> f64 {
        if self.cube {
            // Flat faces: evenly spaced cells.
            c / self.n as f64 * 2.0 - 1.0
        } else {
            (c / self.n as f64 * FRAC_PI_2 - FRAC_PI_4).tan()
        }
    }

    /// Unit direction at continuous tangent coordinates (`ci`, `cj` in cell units).
    pub fn dir_at(&self, f: u8, ci: f64, cj: f64) -> DVec3 {
        let f = f as usize;
        (NORMALS[f].as_dvec3() + TAN_A[f].as_dvec3() * self.tan_at(ci) + TAN_B[f].as_dvec3() * self.tan_at(cj)).normalize()
    }

    pub fn cell_dir(&self, f: u8, i: i32, j: i32) -> DVec3 {
        self.dir_at(f, i as f64 + 0.5, j as f64 + 0.5)
    }

    /// World position of a point given in continuous grid coordinates.
    pub fn point(&self, f: u8, ci: f64, cj: f64, ck: f64) -> DVec3 {
        self.along(self.dir_at(f, ci, cj), ck)
    }

    pub fn cell_center(&self, c: Cell) -> DVec3 {
        self.along(self.cell_dir(c.f, c.i, c.j), c.k as f64 + 0.5)
    }

    /// Cell containing the world point `p`.
    pub fn world_to_cell(&self, p: DVec3) -> Cell {
        let a = p.abs();
        let f: usize = if a.x >= a.y && a.x >= a.z {
            if p.x >= 0.0 { 0 } else { 1 }
        } else if a.y >= a.z {
            if p.y >= 0.0 { 2 } else { 3 }
        } else if p.z >= 0.0 {
            4
        } else {
            5
        };
        let d = p.dot(NORMALS[f].as_dvec3()).max(1e-12);
        let x = p.dot(TAN_A[f].as_dvec3()) / d;
        let y = p.dot(TAN_B[f].as_dvec3()) / d;
        let cube = self.cube;
        let to_idx = |t: f64| {
            let u = if cube { (t + 1.0) / 2.0 } else { (t.atan() + FRAC_PI_4) / FRAC_PI_2 };
            ((u * self.n as f64).floor() as i32).clamp(0, self.n - 1)
        };
        Cell { f: f as u8, i: to_idx(x), j: to_idx(y), k: self.radial(p).floor() as i32 }
    }

    /// Maps a cell whose `i` or `j` may lie outside `0..n` by crossing one cube edge.
    /// Returns `None` past a cube corner (both indices out of range) or below the centre.
    pub fn map_cell(&self, f: u8, i: i32, j: i32, k: i32) -> Option<Cell> {
        if k < 0 {
            return None;
        }
        let n = self.n;
        let (i_out, j_out) = (i < 0 || i >= n, j < 0 || j >= n);
        if !i_out && !j_out {
            return Some(Cell { f, i, j, k });
        }
        if i_out && j_out {
            return None;
        }
        let fu = f as usize;
        let (cross_axis, along_axis, along_idx, beyond, over) = if i_out {
            (TAN_A[fu], TAN_B[fu], j, if i < 0 { -1 } else { 1 }, if i < 0 { -i - 1 } else { i - n })
        } else {
            (TAN_B[fu], TAN_A[fu], i, if j < 0 { -1 } else { 1 }, if j < 0 { -j - 1 } else { j - n })
        };
        if over >= n {
            return None;
        }
        let nb_vec = cross_axis * beyond;
        let b = NORMALS.iter().position(|v| *v == nb_vec)?;
        // In face `b` the shared edge runs along `along_axis`; the third axis is our normal.
        let toward_a = NORMALS[fu];
        let across = if toward_a == TAN_A[b] || toward_a == TAN_B[b] { n - 1 - over } else { over };
        let (ni, nj) = if along_axis == TAN_A[b] { (along_idx, across) } else { (across, along_idx) };
        Some(Cell { f: b as u8, i: ni, j: nj, k })
    }

    /// Neighbour of `c` along `axis` (0 = i, 1 = j, 2 = k) in direction `sign` (±1).
    pub fn neighbor(&self, c: Cell, axis: usize, sign: i32) -> Option<Cell> {
        match axis {
            0 => self.map_cell(c.f, c.i + sign, c.j, c.k),
            1 => self.map_cell(c.f, c.i, c.j + sign, c.k),
            _ => self.map_cell(c.f, c.i, c.j, c.k + sign),
        }
    }

    /// Cell at offset (di, dj, dk) from `c`, following face edges (at most one edge crossing).
    pub fn offset(&self, c: Cell, di: i32, dj: i32, dk: i32) -> Option<Cell> {
        self.map_cell(c.f, c.i + di, c.j + dj, c.k + dk)
    }

    /// Chunk next to `key` at (di, dj, dk) ∈ {-1,0,1}³, across face edges when needed.
    pub fn chunk_neighbor(&self, key: ChunkKey, di: i32, dj: i32, dk: i32) -> Option<ChunkKey> {
        if (di, dj, dk) == (0, 0, 0) {
            return Some(key);
        }
        let o = key.origin();
        let rep = |d: i32, o: i32| match d {
            -1 => o - 1,
            1 => o + CHUNK,
            _ => o + CHUNK / 2,
        };
        let c = self.map_cell(key.f, rep(di, o.x), rep(dj, o.y), rep(dk, o.z))?;
        Some(ChunkKey::of(c))
    }

    /// World position of the chunk's minimum corner (reference point for its mesh).
    pub fn chunk_ref(&self, key: ChunkKey) -> DVec3 {
        let o = key.origin();
        self.point(key.f, o.x as f64, o.y as f64, o.z as f64)
    }

    /// Bounding sphere (centre, radius) of a chunk.
    pub fn chunk_bounds(&self, key: ChunkKey) -> (DVec3, f64) {
        let o = key.origin();
        let mut pts = [DVec3::ZERO; 8];
        for (n, p) in pts.iter_mut().enumerate() {
            let (a, b, c) = ((n & 1) as f64, ((n >> 1) & 1) as f64, ((n >> 2) & 1) as f64);
            *p = self.point(key.f, o.x as f64 + a * 16.0, o.y as f64 + b * 16.0, o.z as f64 + c * 16.0);
        }
        let centre = pts.iter().fold(DVec3::ZERO, |s, p| s + *p) / 8.0;
        let r = pts.iter().map(|p| (*p - centre).length()).fold(0.0, f64::max);
        (centre, r + 0.5)
    }

    pub fn chunk_center(&self, key: ChunkKey) -> DVec3 {
        let o = key.origin();
        self.point(key.f, o.x as f64 + 8.0, o.y as f64 + 8.0, o.z as f64 + 8.0)
    }

    /// The eight corner points of a cell.
    pub fn cell_corners(&self, c: Cell) -> [DVec3; 8] {
        let mut out = [DVec3::ZERO; 8];
        for (n, p) in out.iter_mut().enumerate() {
            let (a, b, k) = ((n & 1) as f64, ((n >> 1) & 1) as f64, ((n >> 2) & 1) as f64);
            *p = self.point(c.f, c.i as f64 + a, c.j as f64 + b, c.k as f64 + k);
        }
        out
    }

    /// Oriented box approximating the cell, used for collisions.
    pub fn cell_obb(&self, c: Cell) -> Obb {
        let r = c.k as f64 + 0.5;
        let p = |a: f64, b: f64| self.point(c.f, c.i as f64 + a, c.j as f64 + b, r);
        let centre = p(0.5, 0.5);
        let n = self.up(centre);
        let ei = (p(1.0, 0.0) - p(0.0, 0.0) + p(1.0, 1.0) - p(0.0, 1.0)) * 0.5;
        let ej = (p(0.0, 1.0) - p(0.0, 0.0) + p(1.0, 1.0) - p(1.0, 0.0)) * 0.5;
        let ui = (ei - n * ei.dot(n)).normalize();
        let uj = n.cross(ui);
        Obb { center: centre, axes: [ui, uj, n], half: [ei.dot(ui).abs() * 0.5, ej.dot(uj).abs() * 0.5, 0.5] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 64 cells per edge ↔ sea-level radius ≈ 41 blocks (cells ≈ 1 block wide at k = 41).
    fn grid() -> Grid {
        Grid::with_cells(64)
    }

    #[test]
    fn faces_are_right_handed() {
        for f in 0..6 {
            assert_eq!(TAN_A[f].cross(TAN_B[f]), NORMALS[f], "face {f}");
        }
    }

    #[test]
    fn grid_size_matches_radius() {
        let g = Grid::new(1024.0);
        assert_eq!(g.n % 16, 0);
        let width = 1024.0 * FRAC_PI_2 / g.n as f64;
        assert!((0.9..=1.0).contains(&width), "cell width at sea level {width}");
    }

    #[test]
    fn world_to_cell_roundtrips_for_every_cell_sample() {
        let g = grid();
        for f in 0..6u8 {
            for i in (0..g.n).step_by(7) {
                for j in (0..g.n).step_by(5) {
                    let c = Cell::new(f, i, j, 100);
                    let back = g.world_to_cell(g.cell_center(c));
                    assert_eq!(back, c, "roundtrip failed for {c:?}");
                }
            }
        }
    }

    #[test]
    fn up_is_radial_and_blocks_are_aligned_with_it() {
        let g = grid();
        for f in 0..6u8 {
            for (i, j) in [(0, 0), (3, 60), (32, 32), (63, 63), (10, 1)] {
                let c = Cell::new(f, i, j, 41);
                let o = g.cell_obb(c);
                assert!((o.axes[2] - g.cell_dir(f, i, j)).length() < 1e-9, "radial axis");
                for a in 0..3 {
                    for b in 0..3 {
                        let d = o.axes[a].dot(o.axes[b]);
                        assert!((d - if a == b { 1.0 } else { 0.0 }).abs() < 1e-9, "orthonormal {a} {b}");
                    }
                }
                // Right-handed (i, j, up).
                assert!(o.axes[0].cross(o.axes[1]).dot(o.axes[2]) > 0.999);
                assert!(o.half[0] > 0.1 && o.half[0] < 1.0, "cell width {}", o.half[0] * 2.0);
            }
        }
    }

    #[test]
    fn cell_sizes_stay_within_a_modest_range() {
        // Equal-angle projection: corners are ~1.3× smaller than face centres at the same radius.
        let g = Grid::new(1024.0);
        let centre = g.cell_obb(Cell::new(4, g.n / 2, g.n / 2, 1024)).half[0] * 2.0;
        let corner = g.cell_obb(Cell::new(4, 0, 0, 1024)).half[0] * 2.0;
        assert!((0.9..=1.01).contains(&centre), "{centre}");
        assert!(corner > 0.55 && corner < centre, "{corner}");
    }

    #[test]
    fn edge_neighbours_are_adjacent_in_space_and_invertible() {
        let g = grid();
        let mut checked = 0;
        for f in 0..6u8 {
            for t in (0..g.n).step_by(3) {
                for (i, j, axis, sign) in [(0, t, 0, -1), (g.n - 1, t, 0, 1), (t, 0, 1, -1), (t, g.n - 1, 1, 1)] {
                    let c = Cell::new(f, i, j, 41);
                    let nb = g.neighbor(c, axis, sign).expect("edge crossing");
                    assert_ne!(nb.f, f);
                    let gap = (g.cell_center(c) - g.cell_center(nb)).length();
                    assert!((0.7..1.45).contains(&gap), "face {f} cell {c:?}: neighbour {nb:?} is {gap} away");
                    // Walking back returns to the start.
                    let back = (0..2).filter_map(|ax| (0..2).find_map(|s| g.neighbor(nb, ax, if s == 0 { -1 } else { 1 }).filter(|b| *b == c))).next();
                    assert_eq!(back, Some(c), "no way back from {nb:?} to {c:?}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 500);
    }

    #[test]
    fn crossing_several_cells_over_the_edge_stays_continuous() {
        let g = grid();
        // Walking along a line of cells straight across the edge never jumps in space.
        for f in 0..6u8 {
            let mut prev: Option<DVec3> = None;
            for i in -6..6 {
                let c = g.map_cell(f, i, 20, 41).expect("cross");
                let p = g.cell_center(c);
                if let Some(q) = prev {
                    let step = (p - q).length();
                    assert!((0.6..1.5).contains(&step), "face {f} step {i}: {step}");
                }
                prev = Some(p);
            }
        }
    }

    #[test]
    fn corners_are_unmapped_and_negative_k_is_rejected() {
        let g = grid();
        assert!(g.map_cell(0, -1, -1, 5).is_none());
        assert!(g.map_cell(2, g.n, g.n, 5).is_none());
        assert!(g.map_cell(0, 3, 3, -1).is_none());
        assert_eq!(g.map_cell(3, 5, 6, 7), Some(Cell::new(3, 5, 6, 7)));
    }

    #[test]
    fn world_covers_the_sphere_without_gaps_at_seams() {
        // Points just either side of each seam map to adjacent cells (indices differ by ≤ 1 across).
        let g = grid();
        for step in 0..200 {
            let t = step as f64 / 200.0 * FRAC_PI_2 - FRAC_PI_4;
            for (a, b) in [(DVec3::new(1.0, 1.0 - 1e-6, t.tan()), DVec3::new(1.0 - 1e-6, 1.0, t.tan()))] {
                let (ca, cb) = (g.world_to_cell(a * 41.0), g.world_to_cell(b * 41.0));
                assert_ne!(ca.f, cb.f);
                let gap = (g.cell_center(ca) - g.cell_center(cb)).length();
                assert!(gap < 1.6, "seam gap {gap} at t={t}");
            }
        }
    }

    #[test]
    fn chunk_keys_and_neighbours() {
        let g = grid();
        let k = ChunkKey::of(Cell::new(2, 35, 17, 100));
        assert_eq!(k, ChunkKey::new(2, 2, 1, 6));
        assert_eq!(k.origin(), IVec3::new(32, 16, 96));
        assert_eq!(g.chunk_neighbor(k, 1, 0, 0), Some(ChunkKey::new(2, 3, 1, 6)));
        assert_eq!(g.chunk_neighbor(k, 0, -1, 1), Some(ChunkKey::new(2, 2, 0, 7)));
        // Across an edge.
        let edge = ChunkKey::new(0, 0, 2, 5);
        let nb = g.chunk_neighbor(edge, -1, 0, 0).unwrap();
        assert_ne!(nb.f, 0);
        assert_eq!(g.chunk_neighbor(ChunkKey::new(0, 0, 0, 5), -1, -1, 0), None, "cube corner");
        assert_eq!(g.chunk_neighbor(ChunkKey::new(0, 1, 1, 0), 0, 0, -1), None, "below the centre");
        assert_eq!(local_idx(15, 15, 15), 4095);
        assert_eq!(idx_to_local(local_idx(3, 9, 12)), (3, 9, 12));
    }

    #[test]
    fn chunk_bounds_contain_their_cells() {
        let g = Grid::new(512.0);
        let key = ChunkKey::new(1, 7, 20, 31);
        let (c, r) = g.chunk_bounds(key);
        let o = key.origin();
        for (di, dj, dk) in [(0, 0, 0), (15, 15, 15), (0, 15, 0), (15, 0, 15)] {
            let cell = Cell::new(1, o.x + di, o.y + dj, o.z + dk);
            assert!((g.cell_center(cell) - c).length() <= r, "cell outside bounds");
        }
        assert!(r < 40.0, "bounding radius {r}");
    }

    fn cube() -> Grid {
        Grid { n: 64, cube: true }
    }

    #[test]
    fn cube_world_to_cell_roundtrips() {
        let g = cube();
        for f in 0..6u8 {
            for i in (0..g.n).step_by(3) {
                for j in (0..g.n).step_by(5) {
                    for k in [20, 32, 60] {
                        let c = Cell::new(f, i, j, k);
                        assert_eq!(g.world_to_cell(g.cell_center(c)), c);
                    }
                }
            }
        }
    }

    #[test]
    fn cube_faces_are_flat_and_edges_connect() {
        let g = cube();
        // Middle of a face: up is the face normal and layers are flat planes.
        let mid = g.cell_center(Cell::new(0, 32, 32, 32));
        assert!((g.up(mid) - DVec3::X).length() < 1e-3);
        let off = g.cell_center(Cell::new(0, 20, 40, 32));
        assert!((mid.x - off.x).abs() < 0.05, "face is flat: {} vs {}", mid.x, off.x);
        // Neighbours across every edge are about one block away and lead back.
        let mut checked = 0;
        for f in 0..6u8 {
            for t in (0..g.n).step_by(3) {
                for (i, j, axis, sign) in [(0, t, 0, -1), (g.n - 1, t, 0, 1), (t, 0, 1, -1), (t, g.n - 1, 1, 1)] {
                    let c = Cell::new(f, i, j, 32);
                    let nb = g.neighbor(c, axis, sign).expect("edge crossing");
                    assert_ne!(nb.f, f);
                    let gap = (g.cell_center(c) - g.cell_center(nb)).length();
                    assert!((0.4..1.6).contains(&gap), "face {f} cell {c:?}: neighbour {nb:?} is {gap} away");
                    let back = (0..2).filter_map(|ax| (0..2).find_map(|s| g.neighbor(nb, ax, if s == 0 { -1 } else { 1 }).filter(|b| *b == c))).next();
                    assert_eq!(back, Some(c));
                    checked += 1;
                }
            }
        }
        assert!(checked > 300);
    }
}