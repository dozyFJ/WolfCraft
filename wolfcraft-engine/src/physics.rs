//! Player physics on the cube-sphere: radial gravity, sphere-vs-cell-box collision, ray casting.
//!
//! The body is a vertical stack of three spheres aligned with the local "up" (the radial
//! direction). Every block is a (slightly tapered) box aligned with the same up, approximated
//! by an oriented box, so walking, stepping and jumping behave identically on every face and
//! across the cube edges.

use crate::block::{Block, RenderKind};
use crate::cubesphere::{Cell, Grid, Obb};
use glam::DVec3;

pub trait Voxels {
    fn grid(&self) -> &Grid;
    fn cell_block(&self, c: Cell) -> Block;

    fn block_at_world(&self, p: DVec3) -> Block {
        self.cell_block(self.grid().world_to_cell(p))
    }
}

pub const BODY_RADIUS: f64 = 0.3;
pub const BODY_HEIGHT: f64 = 1.8;
pub const EYE_HEIGHT: f64 = 1.62;
pub const GRAVITY: f64 = 30.0;
pub const TERMINAL_VELOCITY: f64 = 60.0;
pub const JUMP_SPEED: f64 = 8.7;
const SPHERE_HEIGHTS: [f64; 3] = [0.3, 0.9, 1.5];
/// Ledges up to this height are climbed automatically while walking.
pub const STEP_HEIGHT: f64 = 1.1;

#[derive(Clone, Debug)]
pub struct Body {
    /// Position of the feet.
    pub pos: DVec3,
    pub vel: DVec3,
    pub on_ground: bool,
    pub in_water: bool,
    pub head_in_water: bool,
    pub in_lava: bool,
    pub flying: bool,
}

impl Body {
    pub fn new(pos: DVec3) -> Self {
        Self { pos, vel: DVec3::ZERO, on_ground: false, in_water: false, head_in_water: false, in_lava: false, flying: false }
    }
}

#[inline]
pub fn up_at(p: DVec3) -> DVec3 {
    crate::cubesphere::up(p)
}

/// Per-frame movement intent in the local tangent frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct MoveInput {
    /// Desired horizontal velocity direction (unit or zero), world space, tangent to the sphere.
    pub wish: DVec3,
    pub speed: f64,
    pub jump: bool,
    /// Swim up / fly up.
    pub up: bool,
    /// Swim down / fly down.
    pub down: bool,
}

/// Advances the body by `dt`. Returns the landing speed (blocks/s along gravity) if the body
/// touched ground this step, else 0: the survival layer turns that into fall damage.
pub fn step(world: &impl Voxels, body: &mut Body, input: &MoveInput, dt: f64) -> f64 {
    let up = up_at(body.pos);
    // Fluid state.
    let feet = world.block_at_world(body.pos + up * 0.2);
    let mid = world.block_at_world(body.pos + up * 0.9);
    let head = world.block_at_world(body.pos + up * EYE_HEIGHT);
    body.in_water = feet.is_water() || mid.is_water();
    body.head_in_water = head.is_water();
    body.in_lava = feet.is_lava() || mid.is_lava();

    // Split velocity into tangential and radial parts.
    let mut v_up = body.vel.dot(up);
    let mut v_h = body.vel - up * v_up;

    let on_ground_before = body.on_ground;
    if body.flying {
        let target = input.wish * input.speed;
        v_h = v_h.lerp(target, (1.0 - (-12.0 * dt).exp()).clamp(0.0, 1.0));
        let vert = (input.up as i32 - input.down as i32) as f64 * input.speed.max(8.0) * 0.8;
        v_up += (vert - v_up) * (1.0 - (-12.0 * dt).exp());
    } else if body.in_water || body.in_lava {
        let k = if body.in_lava { 0.35 } else { 0.55 };
        let target = input.wish * input.speed * k;
        v_h = v_h.lerp(target, (1.0 - (-6.0 * dt).exp()).clamp(0.0, 1.0));
        // Buoyancy: slow sink, swim up with the jump key.
        let swim = if input.up || input.jump { 4.2 } else if input.down { -4.0 } else { -2.0 };
        v_up += (swim - v_up) * (1.0 - (-5.0 * dt).exp());
    } else {
        let accel = if body.on_ground { 70.0 } else { 14.0 };
        let target = input.wish * input.speed;
        let diff = target - v_h;
        let max_change = accel * dt;
        v_h += if diff.length() > max_change { diff.normalize() * max_change } else { diff };
        v_up = (v_up - GRAVITY * dt).max(-TERMINAL_VELOCITY);
        if input.jump && body.on_ground {
            v_up = JUMP_SPEED;
            body.on_ground = false;
        }
    }
    body.vel = v_h + up * v_up;

    let start_pos = body.pos;
    // Sub-stepped integration with penetration resolution.
    let dist = body.vel.length() * dt;
    let n = ((dist / 0.2).ceil() as usize).clamp(1, 24);
    let h = dt / n as f64;
    let mut grounded = false;
    let mut impact = 0.0;
    let can_step = input.wish.length_squared() > 0.01;
    for _ in 0..n {
        let before = body.pos;
        let before_vel = body.vel;
        body.pos += body.vel * h;
        let up0 = up_at(before);
        let hv = before_vel - up0 * before_vel.dot(up0);
        let hdir = if hv.length_squared() > 1e-6 { hv.normalize() } else { DVec3::ZERO };
        let (g, imp, wall) = resolve(world, body, hdir);
        grounded |= g;
        if imp > impact {
            impact = imp;
        }
        if wall && can_step && (on_ground_before || grounded) && !body.flying && !body.in_water && !body.in_lava {
            let lifted = before + up0 * STEP_HEIGHT;
            let target = lifted + hv * h;
            if !overlaps(world, lifted) && !overlaps(world, target) {
                body.pos = target;
                body.vel = before_vel - up0 * before_vel.dot(up0).max(0.0);
                grounded = true;
            }
        }
    }
    body.on_ground = grounded && !body.flying;
    // Static friction: a standing player must not creep on the (tapered) cell boxes.
    let idle = input.wish.length_squared() < 1e-6 && !input.jump && !input.up;
    if idle && body.on_ground && on_ground_before && !body.in_water && !body.in_lava {
        body.pos = start_pos;
        body.vel = DVec3::ZERO;
    }
    if body.on_ground && !on_ground_before {
        impact
    } else {
        0.0
    }
}

/// Sphere against an oriented box. Returns the outward push normal, the push distance and the
/// local axis (index, sign) when the contact is a clean face contact.
fn sphere_obb(c: DVec3, r: f64, o: &Obb) -> Option<(DVec3, f64, Option<(usize, i32)>)> {
    let d = c - o.center;
    let local = [d.dot(o.axes[0]), d.dot(o.axes[1]), d.dot(o.axes[2])];
    let mut delta = [0.0; 3];
    for a in 0..3 {
        let cl = local[a].clamp(-o.half[a], o.half[a]);
        delta[a] = local[a] - cl;
    }
    let d2 = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
    if d2 >= r * r {
        return None;
    }
    let (n_local, push) = if d2 > 1e-12 {
        let dist = d2.sqrt();
        ([delta[0] / dist, delta[1] / dist, delta[2] / dist], r - dist)
    } else {
        // Centre inside the box: leave through the nearest face.
        let mut best = (f64::MAX, 0usize, 1.0f64);
        for a in 0..3 {
            let pen = o.half[a] - local[a].abs();
            if pen < best.0 {
                best = (pen, a, if local[a] >= 0.0 { 1.0 } else { -1.0 });
            }
        }
        let mut n = [0.0; 3];
        n[best.1] = best.2;
        (n, best.0 + r)
    };
    let normal = o.axes[0] * n_local[0] + o.axes[1] * n_local[1] + o.axes[2] * n_local[2];
    let face = (0..3).find(|a| n_local[*a].abs() > 0.99).map(|a| (a, if n_local[a] > 0.0 { 1 } else { -1 }));
    Some((normal, push, face))
}

/// Collision box of `block` in `cell`: the whole cell, or the smaller box of a shaped block.
pub fn block_obb(grid: &Grid, cell: Cell, block: Block) -> Obb {
    let o = grid.cell_obb(cell);
    match block.shape() {
        Some(s) => sub_obb(&o, &s),
        None => o,
    }
}

fn sub_obb(cell: &Obb, s: &[f32; 6]) -> Obb {
    let mut o = cell.clone();
    for a in 0..3 {
        let (lo, hi) = (s[a] as f64, s[a + 3] as f64);
        o.center += o.axes[a] * (((lo + hi) * 0.5 - 0.5) * 2.0 * o.half[a]);
        o.half[a] *= hi - lo;
    }
    o
}

/// All collision boxes of `block` in `cell` (stairs have two).
pub fn block_obbs(grid: &Grid, cell: Cell, block: Block) -> Vec<Obb> {
    let o = grid.cell_obb(cell);
    let boxes = block.boxes();
    if boxes.is_empty() { vec![o] } else { boxes.iter().map(|s| sub_obb(&o, s)).collect() }
}

/// True if a body with its feet at `feet` intersects any solid block.
fn overlaps(world: &impl Voxels, feet: DVec3) -> bool {
    let g = world.grid();
    let up = up_at(feet);
    SPHERE_HEIGHTS.iter().any(|h| {
        let c = feet + up * h;
        let base = g.world_to_cell(c);
        for dk in -1..=1 {
            for dj in -1..=1 {
                for di in -1..=1 {
                    let Some(cell) = g.offset(base, di, dj, dk) else { continue };
                    let b = world.cell_block(cell);
                    if b.is_solid() && block_obbs(g, cell, b).iter().any(|o| sphere_obb(c, BODY_RADIUS - 0.01, o).is_some()) {
                        return true;
                    }
                }
            }
        }
        false
    })
}

/// Pushes the body out of solid blocks.
/// Returns (touched ground, impact speed into ground, pushed sideways by a wall).
fn resolve(world: &impl Voxels, body: &mut Body, hdir: DVec3) -> (bool, f64, bool) {
    let g = world.grid();
    let mut grounded = false;
    let mut wall = false;
    let mut impact = 0.0f64;
    for _ in 0..4 {
        let up = up_at(body.pos);
        let mut moved = false;
        for h in SPHERE_HEIGHTS {
            let c0 = body.pos + up * h;
            let base = g.world_to_cell(c0);
            for dk in -1..=1 {
                for dj in -1..=1 {
                    for di in -1..=1 {
                        let Some(cell) = g.offset(base, di, dj, dk) else { continue };
                        let block = world.cell_block(cell);
                        if !block.is_solid() {
                            continue;
                        }
                        for obb in block_obbs(g, cell, block) {
                        let c = body.pos + up * h;
                        let Some((normal, push, face)) = sphere_obb(c, BODY_RADIUS, &obb) else { continue };
                        // A contact with a face that is covered by another solid block is internal
                        // (only between full cubes: a slab or a pane does not cover its neighbour).
                        if let Some((axis, sign)) = face.filter(|_| block.shape().is_none()) {
                            if g.neighbor(cell, axis, sign).map(|nb| world.cell_block(nb)).is_some_and(|nb| nb.is_solid() && nb.shape().is_none()) {
                                continue;
                            }
                        }
                        body.pos += normal * push;
                        let vn = body.vel.dot(normal);
                        if vn < 0.0 {
                            body.vel -= normal * vn;
                            if normal.dot(up) > 0.55 {
                                impact = impact.max(-vn);
                            }
                        }
                        if normal.dot(up) > 0.55 {
                            grounded = true;
                        } else if normal.dot(hdir) < -0.3 && push > 1e-4 {
                            wall = true;
                        }
                        moved = true;
                        }
                    }
                }
            }
        }
        if !moved {
            break;
        }
    }
    (grounded, impact, wall)
}

/// True if a body standing at `feet` would overlap `cell`.
pub fn body_overlaps_cell(grid: &Grid, feet: DVec3, cell: Cell) -> bool {
    let up = up_at(feet);
    let obb = grid.cell_obb(cell);
    SPHERE_HEIGHTS.iter().any(|h| sphere_obb(feet + up * h, BODY_RADIUS, &obb).is_some())
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub cell: Cell,
    /// Face normal pointing out of the hit block towards the ray origin: (axis 0=i 1=j 2=k, sign).
    pub normal: (usize, i32),
    pub dist: f64,
    pub block: Block,
}

impl Hit {
    /// The cell in front of the hit face (where a block would be placed).
    pub fn front_cell(&self, grid: &Grid) -> Option<Cell> {
        grid.neighbor(self.cell, self.normal.0, self.normal.1)
    }
}

/// Which face of `hit` was entered when coming from `prev`.
fn entry_face(grid: &Grid, prev: Cell, hit: Cell) -> (usize, i32) {
    for axis in [2usize, 0, 1] {
        for sign in [-1, 1] {
            if grid.neighbor(hit, axis, sign) == Some(prev) {
                return (axis, sign);
            }
        }
    }
    (2, 1)
}

/// Marches along the ray in small steps and returns the first non-air, non-fluid block.
pub fn raycast(world: &impl Voxels, origin: DVec3, dir: DVec3, max_dist: f64) -> Option<Hit> {
    let dir = dir.normalize_or_zero();
    if dir == DVec3::ZERO {
        return None;
    }
    let g = world.grid();
    let mut prev = g.world_to_cell(origin);
    let mut t = 0.0;
    const STEP: f64 = 0.04;
    while t < max_dist {
        t += STEP;
        let c = g.world_to_cell(origin + dir * t);
        if c == prev {
            continue;
        }
        let b = world.cell_block(c);
        if b.info().render != RenderKind::Invisible && !b.is_fluid() {
            return Some(Hit { cell: c, normal: entry_face(g, prev, c), dist: t, block: b });
        }
        prev = c;
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A solid ball of stone with optional edits.
    pub(crate) struct Ball {
        pub grid: Grid,
        pub r: f64,
        pub edits: HashMap<Cell, Block>,
    }
    impl Ball {
        pub fn new(r: f64) -> Self {
            Self { grid: Grid::new(r), r, edits: HashMap::new() }
        }
    }
    impl Voxels for Ball {
        fn grid(&self) -> &Grid {
            &self.grid
        }
        fn cell_block(&self, c: Cell) -> Block {
            if let Some(b) = self.edits.get(&c) {
                return *b;
            }
            if c.k as f64 + 0.5 <= self.r { Block::Stone } else { Block::Air }
        }
    }

    fn settle(world: &Ball, start: DVec3) -> Body {
        let mut b = Body::new(start);
        for _ in 0..600 {
            step(world, &mut b, &MoveInput::default(), 1.0 / 60.0);
        }
        b
    }

    #[test]
    fn gravity_points_to_the_centre_from_every_side() {
        let w = Ball::new(60.0);
        let dirs = [DVec3::X, -DVec3::X, DVec3::Y, -DVec3::Y, DVec3::Z, -DVec3::Z, DVec3::new(1.0, 1.0, 1.0).normalize(), DVec3::new(-1.0, 0.5, -0.3).normalize(), DVec3::new(1.0, 1.0, 0.0).normalize()];
        for d in dirs {
            let b = settle(&w, d * 66.0);
            assert!(b.on_ground, "not grounded from {d}");
            let r = b.pos.length();
            assert!((r - 60.0).abs() < 0.3, "rest radius {r} for direction {d}");
        }
    }

    #[test]
    fn standing_still_never_creeps_anywhere_on_the_planet() {
        let w = Ball::new(40.0);
        let dirs = [DVec3::new(1.0, 1.0, 0.0), DVec3::new(1.0, 1.0, 1.0), DVec3::new(0.6, 1.0, 0.3), DVec3::new(0.3, 0.2, 1.0), DVec3::new(1.0, 0.45, 0.45), DVec3::new(1.0, 0.999, 0.01)];
        for d in dirs {
            let d = d.normalize();
            let mut b = settle(&w, d * 47.0);
            let rest = b.pos;
            for _ in 0..1200 {
                step(&w, &mut b, &MoveInput::default(), 1.0 / 60.0);
            }
            assert!((b.pos - rest).length() < 0.02, "crept {} blocks standing at {d}", (b.pos - rest).length());
            assert!(b.on_ground);
        }
    }

    #[test]
    fn walking_over_cube_edges_and_around_the_planet_returns_to_start() {
        // Walk along a great circle that crosses four cube edges and comes back.
        let w = Ball::new(40.0);
        let mut b = settle(&w, DVec3::new(0.0, 0.0, 47.0));
        let start = b.pos;
        let mut heading = DVec3::X;
        let mut angle = 0.0;
        let (mut min_r, mut max_r) = (f64::MAX, 0.0f64);
        let dt = 1.0 / 60.0;
        let mut frames = 0;
        let mut faces = std::collections::HashSet::new();
        while angle < std::f64::consts::TAU - 0.05 && frames < 30000 {
            let up = up_at(b.pos);
            heading = (heading - up * heading.dot(up)).normalize();
            let before = up;
            step(&w, &mut b, &MoveInput { wish: heading, speed: 4.0, ..Default::default() }, dt);
            angle += before.dot(up_at(b.pos)).clamp(-1.0, 1.0).acos();
            min_r = min_r.min(b.pos.length());
            max_r = max_r.max(b.pos.length());
            faces.insert(w.grid.world_to_cell(b.pos).f);
            frames += 1;
        }
        assert!(frames < 30000, "walking got stuck at angle {angle}");
        assert!(max_r - min_r < 2.5, "left the surface: {min_r}..{max_r}");
        assert!((b.pos - start).length() < 12.0, "did not come back: {}", (b.pos - start).length());
        assert!(faces.len() >= 4, "the lap must cross several cube faces, got {}", faces.len());
    }

    #[test]
    fn jump_leaves_ground_and_lands() {
        let w = Ball::new(40.0);
        let mut b = settle(&w, DVec3::new(0.0, 46.0, 0.0));
        let r0 = b.pos.length();
        step(&w, &mut b, &MoveInput { jump: true, ..Default::default() }, 1.0 / 60.0);
        let mut peak: f64 = 0.0;
        for _ in 0..60 {
            step(&w, &mut b, &MoveInput::default(), 1.0 / 60.0);
            peak = peak.max(b.pos.length() - r0);
        }
        assert!(peak > 1.0 && peak < 1.6, "jump height {peak}");
        for _ in 0..60 {
            step(&w, &mut b, &MoveInput::default(), 1.0 / 60.0);
        }
        assert!(b.on_ground);
    }

    #[test]
    fn solid_walls_block_horizontal_motion_and_one_block_steps_are_climbed() {
        let mut w = Ball::new(60.0);
        let b0 = settle(&w, DVec3::new(0.0, 66.0, 0.0));
        let base = w.grid.world_to_cell(b0.pos);
        let forward = w.grid.cell_obb(base).axes[0];
        // A 3-high, wide wall four cells ahead along +i.
        for dj in -4..=4 {
            for dk in 0..3 {
                w.edits.insert(Cell::new(base.f, base.i + 4, base.j + dj, base.k + dk), Block::Stone);
            }
        }
        let mut b = Body::new(b0.pos);
        b.on_ground = true;
        for _ in 0..240 {
            step(&w, &mut b, &MoveInput { wish: forward, speed: 5.0, ..Default::default() }, 1.0 / 60.0);
        }
        let reached = (b.pos - b0.pos).dot(forward);
        assert!(reached < 4.0 - BODY_RADIUS + 0.2, "walked through the wall ({reached})");
        assert!(reached > 1.5, "did not approach the wall ({reached})");

        // A one-block ledge is climbed automatically.
        let mut w2 = Ball::new(60.0);
        let b1 = settle(&w2, DVec3::new(0.0, 66.0, 0.0));
        let base = w2.grid.world_to_cell(b1.pos);
        for di in 3..40 {
            for dj in -6..=6 {
                w2.edits.insert(Cell::new(base.f, base.i + di, base.j + dj, base.k), Block::Stone);
            }
        }
        let mut b = Body::new(b1.pos);
        b.on_ground = true;
        let k_before = w2.grid.world_to_cell(b.pos).k;
        for _ in 0..240 {
            step(&w2, &mut b, &MoveInput { wish: forward, speed: 4.3, ..Default::default() }, 1.0 / 60.0);
        }
        let cell_now = w2.grid.world_to_cell(b.pos);
        assert!(cell_now.i >= base.i + 6, "stuck at the ledge (i = {})", cell_now.i);
        assert!(cell_now.k > k_before, "should stand on top of the ledge");
    }

    #[test]
    fn landing_reports_impact() {
        let w = Ball::new(40.0);
        let mut b = Body::new(DVec3::new(0.0, 60.0, 0.0));
        let mut impact = 0.0;
        for _ in 0..300 {
            let i = step(&w, &mut b, &MoveInput::default(), 1.0 / 60.0);
            if i > 0.0 {
                impact = i;
                break;
            }
        }
        assert!(impact > 20.0, "fall from 19 blocks should be fast: {impact}");
    }

    #[test]
    fn raycast_hits_first_block_with_normal() {
        let w = Ball::new(20.0);
        let origin = DVec3::new(0.3, 40.0, 0.4);
        let hit = raycast(&w, origin, DVec3::NEG_Y, 30.0).unwrap();
        assert_eq!(hit.cell.k, 19);
        assert_eq!(hit.normal, (2, 1), "entered through the top face");
        assert_eq!(hit.front_cell(&w.grid).unwrap().k, 20);
        assert!(raycast(&w, origin, DVec3::Y, 30.0).is_none());
        assert!(raycast(&w, origin, DVec3::NEG_Y, 5.0).is_none());
        // A ray along the surface hits the side of a pillar and reports a horizontal normal.
        let mut w = Ball::new(20.0);
        let c = w.grid.world_to_cell(DVec3::new(0.0, 20.0, 0.0));
        w.edits.insert(Cell::new(c.f, c.i + 3, c.j, c.k + 1), Block::Stone);
        let o = w.grid.cell_obb(Cell::new(c.f, c.i, c.j, c.k + 1));
        let hit = raycast(&w, o.center, o.axes[0], 8.0).unwrap();
        assert_eq!((hit.cell.i, hit.normal), (c.i + 3, (0, -1)));
    }

    #[test]
    fn raycast_crosses_cube_edges() {
        let w = Ball::new(30.0);
        let start = DVec3::new(0.0, 31.5, 0.0);
        let dir = DVec3::new(1.0, -0.6, 0.0).normalize();
        let hit = raycast(&w, start, dir, 40.0).expect("must hit the planet");
        assert!(hit.block == Block::Stone);
        assert!(hit.dist > 1.0 && hit.dist < 40.0);
    }

    #[test]
    fn shaped_blocks_collide_with_their_own_box() {
        let g = Grid::new(1024.0);
        let cell = g.world_to_cell(DVec3::new(0.0, 1030.5, 0.0));
        let full = g.cell_obb(cell);
        let slab = block_obb(&g, cell, Block::StoneSlab);
        assert!((slab.half[2] - 0.25).abs() < 1e-9, "half as tall");
        assert!(((full.center - slab.center).dot(full.axes[2]) - 0.25).abs() < 1e-9, "sits on the bottom of the cell");
        assert!((slab.half[0] - full.half[0]).abs() < 1e-9, "full width");
        let pane = block_obb(&g, cell, Block::GlassPaneA);
        assert!(pane.half[0] < full.half[0] * 0.2, "thin along i");
        assert!((pane.center - full.center).length() < 1e-9, "centred");
        assert_eq!(block_obb(&g, cell, Block::Stone).half, full.half);
    }
}
