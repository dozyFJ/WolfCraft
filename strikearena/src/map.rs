//! The arena «Склад»: walls, crates, pillars; movement collision, ray casts and a walk grid.

use glam::{Vec2, Vec3};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const HALF: f32 = 24.0;

#[derive(Clone, Copy, Debug)]
pub struct Block3 {
    pub min: Vec3,
    pub max: Vec3,
    pub color: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct Pillar {
    pub at: Vec2,
    pub r: f32,
    pub h: f32,
}

pub struct Map {
    pub boxes: Vec<Block3>,
    pub pillars: Vec<Pillar>,
    pub spawns: [Vec<Vec2>; 2],
    nav: Vec<bool>,
    n: usize,
}

const WALL: Vec3 = Vec3::new(0.62, 0.6, 0.56);
const CRATE: Vec3 = Vec3::new(0.62, 0.44, 0.24);
const METAL: Vec3 = Vec3::new(0.32, 0.42, 0.5);

fn bx(x0: f32, z0: f32, x1: f32, z1: f32, h: f32, color: Vec3) -> Block3 {
    Block3 { min: Vec3::new(x0.min(x1), 0.0, z0.min(z1)), max: Vec3::new(x0.max(x1), h, z0.max(z1)), color }
}

fn crate_at(x: f32, z: f32, s: f32, stack: bool, out: &mut Vec<Block3>) {
    out.push(bx(x - s / 2.0, z - s / 2.0, x + s / 2.0, z + s / 2.0, s, CRATE));
    if stack {
        let mut top = bx(x - s * 0.4, z - s * 0.4, x + s * 0.4, z + s * 0.4, s * 0.8, CRATE * 0.9);
        top.min.y = s;
        top.max.y = s + s * 0.8;
        out.push(top);
    }
}

impl Map {
    pub fn warehouse() -> Self {
        let mut b = Vec::new();
        // Outer walls.
        let h = HALF;
        b.push(bx(-h - 1.0, -h - 1.0, h + 1.0, -h, 5.0, WALL * 0.8));
        b.push(bx(-h - 1.0, h, h + 1.0, h + 1.0, 5.0, WALL * 0.8));
        b.push(bx(-h - 1.0, -h, -h, h, 5.0, WALL * 0.8));
        b.push(bx(h, -h, h + 1.0, h, 5.0, WALL * 0.8));
        // The house in the middle: four walls with a doorway in each.
        for s in [-1.0f32, 1.0] {
            b.push(bx(-5.0, 5.0 * s - 0.25, -1.2, 5.0 * s + 0.25, 3.2, WALL));
            b.push(bx(1.2, 5.0 * s - 0.25, 5.0, 5.0 * s + 0.25, 3.2, WALL));
            b.push(bx(5.0 * s - 0.25, -5.0, 5.0 * s + 0.25, -1.2, 3.2, WALL));
            b.push(bx(5.0 * s - 0.25, 1.2, 5.0 * s + 0.25, 5.0, 3.2, WALL));
        }
        // Long side walls and low cover, mirrored for the two teams.
        for s in [-1.0f32, 1.0] {
            b.push(bx(12.0 * s - 0.3, -4.0, 12.0 * s + 0.3, 4.0, 2.6, WALL));
            b.push(bx(-3.0, 13.0 * s - 0.3, 3.0, 13.0 * s + 0.3, 1.1, METAL));
            b.push(bx(16.0 * s - 3.0, 9.0 * s - 0.3, 16.0 * s + 3.0, 9.0 * s + 0.3, 1.1, METAL));
            b.push(bx(-16.0 * s - 3.0, 9.0 * s - 0.3, -16.0 * s + 3.0, 9.0 * s + 0.3, 1.1, METAL));
            crate_at(9.0 * s, 9.0 * s, 1.4, true, &mut b);
            crate_at(-9.0 * s, 9.0 * s, 1.4, false, &mut b);
            crate_at(18.0 * s, 0.0, 1.6, true, &mut b);
            crate_at(6.0 * s, 17.0 * s, 1.4, false, &mut b);
            crate_at(-6.0 * s, 17.0 * s, 1.4, true, &mut b);
            crate_at(19.0 * s, 17.0 * s, 1.6, true, &mut b);
            crate_at(-19.0 * s, 15.0 * s, 1.4, false, &mut b);
            crate_at(2.5 * s, 2.5 * s, 1.0, false, &mut b);
        }
        let pillars = [(-8.0, -4.0), (8.0, 4.0), (-8.0, 4.0), (8.0, -4.0)].into_iter().map(|(x, z)| Pillar { at: Vec2::new(x, z), r: 0.55, h: 5.0 }).collect();
        let spawns = [(0..5).map(|k| Vec2::new(-8.0 + k as f32 * 4.0, -21.5)).collect(), (0..5).map(|k| Vec2::new(-8.0 + k as f32 * 4.0, 21.5)).collect()];
        let n = (HALF * 2.0) as usize;
        let mut m = Map { boxes: b, pillars, spawns, nav: vec![true; n * n], n };
        for j in 0..n {
            for i in 0..n {
                let c = m.cell_center(i, j);
                m.nav[j * n + i] = !m.blocked(c, 0.5);
            }
        }
        m
    }

    fn cell_center(&self, i: usize, j: usize) -> Vec2 {
        Vec2::new(i as f32 + 0.5 - HALF, j as f32 + 0.5 - HALF)
    }

    fn cell_of(&self, p: Vec2) -> (usize, usize) {
        let f = |v: f32| ((v + HALF).floor().max(0.0) as usize).min(self.n - 1);
        (f(p.x), f(p.y))
    }

    /// A body of radius `r` at `p` overlaps something solid.
    pub fn blocked(&self, p: Vec2, r: f32) -> bool {
        if p.x.abs() > HALF - r || p.y.abs() > HALF - r {
            return true;
        }
        self.boxes.iter().any(|b| {
            let cx = p.x.clamp(b.min.x, b.max.x);
            let cz = p.y.clamp(b.min.z, b.max.z);
            b.min.y < 1.0 && Vec2::new(p.x - cx, p.y - cz).length_squared() < r * r
        }) || self.pillars.iter().any(|c| (p - c.at).length() < c.r + r)
    }

    /// Moves from `from` towards `to`, sliding along walls.
    pub fn slide(&self, from: Vec2, to: Vec2, r: f32) -> Vec2 {
        if !self.blocked(to, r) {
            return to;
        }
        let x = Vec2::new(to.x, from.y);
        if !self.blocked(x, r) {
            return x;
        }
        let z = Vec2::new(from.x, to.y);
        if !self.blocked(z, r) {
            return z;
        }
        from
    }

    /// Distance along the ray to the first wall, crate, pillar or the floor.
    pub fn ray(&self, o: Vec3, d: Vec3, max: f32) -> Option<f32> {
        let mut best = max;
        let mut hit = false;
        if d.y < -1e-5 {
            let t = -o.y / d.y;
            if t > 0.0 && t < best {
                best = t;
                hit = true;
            }
        }
        for b in &self.boxes {
            if let Some(t) = ray_box(o, d, b.min, b.max) {
                if t < best {
                    best = t;
                    hit = true;
                }
            }
        }
        for c in &self.pillars {
            let oc = Vec2::new(o.x - c.at.x, o.z - c.at.y);
            let dd = Vec2::new(d.x, d.z);
            let a = dd.length_squared();
            if a < 1e-8 {
                continue;
            }
            let bq = oc.dot(dd);
            let cq = oc.length_squared() - c.r * c.r;
            let disc = bq * bq - a * cq;
            if disc < 0.0 {
                continue;
            }
            let t = (-bq - disc.sqrt()) / a;
            if t > 0.0 && t < best && (o.y + d.y * t) <= c.h {
                best = t;
                hit = true;
            }
        }
        hit.then_some(best)
    }

    pub fn visible(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let len = d.length();
        len < 1e-3 || self.ray(a, d / len, len - 0.05).is_none()
    }

    /// Walkable path between two points on the 1 m grid (A*), as cell centres.
    pub fn path(&self, from: Vec2, to: Vec2) -> Vec<Vec2> {
        let n = self.n;
        let (si, sj) = self.cell_of(from);
        let (gi, gj) = self.nearest_free(self.cell_of(to));
        let start = sj * n + si;
        let goal = gj * n + gi;
        let mut came = vec![usize::MAX; n * n];
        let mut cost = vec![u32::MAX; n * n];
        let mut open = BinaryHeap::new();
        cost[start] = 0;
        let hfn = |k: usize| {
            let (i, j) = (k % n, k / n);
            let (dx, dz) = ((i as i32 - gi as i32).unsigned_abs(), (j as i32 - gj as i32).unsigned_abs());
            10 * dx.max(dz) + 4 * dx.min(dz)
        };
        open.push(Reverse((hfn(start), start)));
        let mut found = false;
        while let Some(Reverse((_, k))) = open.pop() {
            if k == goal {
                found = true;
                break;
            }
            let (i, j) = ((k % n) as i32, (k / n) as i32);
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || nj < 0 || ni >= n as i32 || nj >= n as i32 {
                    continue;
                }
                let nk = nj as usize * n + ni as usize;
                if !self.nav[nk] {
                    continue;
                }
                // No cutting corners past walls.
                if di != 0 && dj != 0 && (!self.nav[j as usize * n + ni as usize] || !self.nav[nj as usize * n + i as usize]) {
                    continue;
                }
                let step = if di != 0 && dj != 0 { 14 } else { 10 };
                let c = cost[k].saturating_add(step);
                if c < cost[nk] {
                    cost[nk] = c;
                    came[nk] = k;
                    open.push(Reverse((c + hfn(nk), nk)));
                }
            }
        }
        if !found {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut k = goal;
        while k != start && k != usize::MAX {
            out.push(self.cell_center(k % n, k / n));
            k = came[k];
        }
        out.reverse();
        out
    }

    fn nearest_free(&self, (i, j): (usize, usize)) -> (usize, usize) {
        let n = self.n as i32;
        for r in 0..8i32 {
            for dj in -r..=r {
                for di in -r..=r {
                    let (a, b) = (i as i32 + di, j as i32 + dj);
                    if a >= 0 && b >= 0 && a < n && b < n && self.nav[(b * n + a) as usize] {
                        return (a as usize, b as usize);
                    }
                }
            }
        }
        (i, j)
    }
}

fn ray_box(o: Vec3, d: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv = Vec3::ONE / d;
    let t1 = (min - o) * inv;
    let t2 = (max - o) * inv;
    let tmin = t1.min(t2).max_element();
    let tmax = t1.max(t2).min_element();
    (tmax >= tmin.max(0.0) && tmin > 0.0).then_some(tmin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawns_are_free_and_connected() {
        let m = Map::warehouse();
        for s in m.spawns.iter().flatten() {
            assert!(!m.blocked(*s, 0.4), "spawn {s:?} is inside something");
        }
        let p = m.path(m.spawns[0][0], m.spawns[1][4]);
        assert!(p.len() > 30, "no way across the map: {}", p.len());
        // Every step of the path is walkable.
        assert!(p.iter().all(|c| !m.blocked(*c, 0.3)));
    }

    #[test]
    fn walls_stop_rays_and_open_floor_does_not() {
        let m = Map::warehouse();
        // Through the house wall.
        assert!(!m.visible(Vec3::new(-3.0, 1.6, -8.0), Vec3::new(-3.0, 1.6, 8.0)));
        // Along the open side.
        assert!(m.visible(Vec3::new(-21.0, 1.6, -20.0), Vec3::new(-21.0, 1.6, -11.0)));
        // Into the floor.
        let t = m.ray(Vec3::new(0.0, 1.6, -20.0), Vec3::new(0.0, -1.0, 0.0), 10.0).unwrap();
        assert!((t - 1.6).abs() < 1e-3);
    }

    #[test]
    fn sliding_along_a_wall() {
        let m = Map::warehouse();
        // Walk diagonally into the outer wall: keep moving along it.
        let p = m.slide(Vec2::new(0.0, -23.0), Vec2::new(0.5, -23.6), 0.4);
        assert!(p.x > 0.4 && p.y >= -23.6 + 0.0);
    }
}
