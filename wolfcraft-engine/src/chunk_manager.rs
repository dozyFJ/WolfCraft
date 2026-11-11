//! Chunk streaming: loads/generates chunks around the player on worker threads, meshes them
//! asynchronously, unloads far chunks and persists edited ones. Chunks are addressed by face
//! and index (`ChunkKey`); neighbours across cube edges are found through the cube-sphere grid.

use crate::block::Block;
use crate::chunk::{CHUNK_SIZE, Chunk};
use crate::cubesphere::{Cell, ChunkKey, Grid};
use crate::mesher::{MeshData, MeshInput, mesh_chunk};
use crate::physics::Voxels;
use crate::planet::Planet;
use crate::storage::Storage;
use crate::worldgen::generate_chunk;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;

#[derive(Default, Clone, Copy, Debug)]
pub struct ChunkStats {
    pub loaded: usize,
    pub meshed: usize,
    pub gen_inflight: usize,
    pub mesh_inflight: usize,
    pub generated_total: u64,
    pub meshed_total: u64,
}

#[derive(Default)]
struct MeshState {
    /// Latest version requested.
    requested: u32,
    /// Version of the mesh accepted last (0 = none).
    built: u32,
    pending: bool,
}

pub struct ChunkManager {
    pub planet: Arc<Planet>,
    pub storage: Arc<Storage>,
    chunks: HashMap<ChunkKey, Arc<Chunk>>,
    gen_pending: HashSet<ChunkKey>,
    mesh_state: HashMap<ChunkKey, MeshState>,
    gen_tx: Sender<(ChunkKey, Chunk)>,
    gen_rx: Receiver<(ChunkKey, Chunk)>,
    mesh_tx: Sender<(ChunkKey, u32, MeshData)>,
    mesh_rx: Receiver<(ChunkKey, u32, MeshData)>,
    gen_queue: VecDeque<ChunkKey>,
    mesh_ready: VecDeque<ChunkKey>,
    urgent: Vec<ChunkKey>,
    last_center: Option<ChunkKey>,
    last_view: i32,
    out_meshes: Vec<(ChunkKey, MeshData)>,
    out_removed: Vec<ChunkKey>,
    max_inflight: usize,
    pub stats: ChunkStats,
}

impl ChunkManager {
    pub fn new(planet: Arc<Planet>, storage: Arc<Storage>) -> Self {
        let (gen_tx, gen_rx) = channel();
        let (mesh_tx, mesh_rx) = channel();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        Self {
            planet,
            storage,
            chunks: HashMap::new(),
            gen_pending: HashSet::new(),
            mesh_state: HashMap::new(),
            gen_tx,
            gen_rx,
            mesh_tx,
            mesh_rx,
            gen_queue: VecDeque::new(),
            mesh_ready: VecDeque::new(),
            urgent: Vec::new(),
            last_center: None,
            last_view: -1,
            out_meshes: Vec::new(),
            out_removed: Vec::new(),
            max_inflight: (threads * 2).max(4),
            stats: ChunkStats::default(),
        }
    }

    pub fn grid(&self) -> Grid {
        self.planet.grid
    }

    pub fn is_loaded(&self, key: ChunkKey) -> bool {
        self.chunks.contains_key(&key)
    }

    pub fn get_block(&self, c: Cell) -> Option<Block> {
        self.chunks.get(&ChunkKey::of(c)).map(|ch| ch.get(c.i & 15, c.j & 15, c.k & 15))
    }

    pub fn chunk(&self, key: ChunkKey) -> Option<&Arc<Chunk>> {
        self.chunks.get(&key)
    }

    /// Distance between two chunk centres, in chunks.
    fn chunk_dist(&self, a: ChunkKey, b: ChunkKey) -> f64 {
        (self.planet.grid.chunk_center(a) - self.planet.grid.chunk_center(b)).length() / CHUNK_SIZE as f64
    }

    /// Changes a block. Returns false when the chunk is not loaded. The affected chunk meshes
    /// are rebuilt immediately (next `update`) so edits show up without delay.
    pub fn set_block(&mut self, c: Cell, b: Block) -> bool {
        let key = ChunkKey::of(c);
        let Some(arc) = self.chunks.get_mut(&key) else { return false };
        let chunk = Arc::make_mut(arc);
        let (li, lj, lk) = (c.i & 15, c.j & 15, c.k & 15);
        if chunk.get(li, lj, lk) == b {
            return true;
        }
        chunk.set(li, lj, lk, b);
        chunk.modified = true;
        // Every chunk that has this cell in its padding needs a new mesh.
        let g = self.planet.grid;
        for dk in -1..=1 {
            for dj in -1..=1 {
                for di in -1..=1 {
                    if let Some(n) = g.offset(c, di, dj, dk) {
                        let nk = ChunkKey::of(n);
                        if self.chunks.contains_key(&nk) && !self.urgent.contains(&nk) {
                            self.urgent.push(nk);
                        }
                    }
                }
            }
        }
        true
    }

    /// The (up to 26) chunks that surround `key` and are needed to mesh it.
    fn neighbor_keys(&self, key: ChunkKey) -> Vec<ChunkKey> {
        let g = &self.planet.grid;
        let mut out = Vec::with_capacity(26);
        for dk in -1..=1 {
            for dj in -1..=1 {
                for di in -1..=1 {
                    if (di, dj, dk) == (0, 0, 0) {
                        continue;
                    }
                    if let Some(n) = g.chunk_neighbor(key, di, dj, dk) {
                        if !out.contains(&n) {
                            out.push(n);
                        }
                    }
                }
            }
        }
        out
    }

    fn build_input(&self, key: ChunkKey) -> Option<MeshInput> {
        let centre = self.chunks.get(&key)?;
        let g = self.planet.grid;
        let mut inp = MeshInput::new(key, g, centre.sky_open);
        let o = key.origin();
        // Inside the chunk: copy directly. Padded coordinates are (x, y, z) = (j, k, i).
        for lk in 0..CHUNK_SIZE {
            for lj in 0..CHUNK_SIZE {
                for li in 0..CHUNK_SIZE {
                    let b = centre.get(li, lj, lk);
                    if b != Block::Air {
                        inp.set(lj, lk, li, b);
                    }
                }
            }
        }
        // The one-cell shell around it comes from the neighbours (possibly over a cube edge).
        for y in -1..=CHUNK_SIZE {
            for z in -1..=CHUNK_SIZE {
                for x in -1..=CHUNK_SIZE {
                    let inside = (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&y) && (0..CHUNK_SIZE).contains(&z);
                    if inside {
                        continue;
                    }
                    let Some(c) = g.map_cell(key.f, o.x + z, o.y + x, o.z + y) else { continue };
                    let nk = ChunkKey::of(c);
                    let Some(ch) = self.chunks.get(&nk) else {
                        if self.neighbor_keys(key).contains(&nk) {
                            return None; // a needed neighbour is not loaded yet
                        }
                        continue;
                    };
                    let b = ch.get(c.i & 15, c.j & 15, c.k & 15);
                    if b != Block::Air {
                        inp.set(x, y, z, b);
                    }
                }
            }
        }
        Some(inp)
    }

    fn mesh_ready_check(&mut self, key: ChunkKey, center: ChunkKey, view: i32) {
        if self.chunk_dist(key, center) > view as f64 {
            return;
        }
        if !self.chunks.contains_key(&key) {
            return;
        }
        let st = self.mesh_state.entry(key).or_default();
        if st.pending || st.built > 0 && st.built == st.requested {
            return;
        }
        if self.neighbor_keys(key).iter().all(|n| self.chunks.contains_key(n)) && !self.mesh_ready.contains(&key) {
            self.mesh_ready.push_back(key);
        }
    }

    fn spawn_mesh(&mut self, key: ChunkKey) {
        let Some(chunk) = self.chunks.get(&key) else { return };
        if chunk.is_empty() {
            // Nothing to draw; clear any old mesh.
            let st = self.mesh_state.entry(key).or_default();
            st.requested += 1;
            st.built = st.requested;
            st.pending = false;
            self.out_meshes.push((key, MeshData::default()));
            return;
        }
        let Some(input) = self.build_input(key) else { return };
        let st = self.mesh_state.entry(key).or_default();
        st.requested += 1;
        st.pending = true;
        let version = st.requested;
        let tx = self.mesh_tx.clone();
        rayon::spawn(move || {
            let data = mesh_chunk(&input);
            let _ = tx.send((key, version, data));
        });
    }

    /// Streams chunks around `center` within `view` chunks.
    pub fn update(&mut self, center: ChunkKey, view: i32, mesh_budget: usize) {
        // 1. Finished generation.
        while let Ok((key, chunk)) = self.gen_rx.try_recv() {
            self.gen_pending.remove(&key);
            self.stats.generated_total += 1;
            if let Some(c) = self.last_center {
                if self.chunk_dist(key, c) > (self.last_view + 3) as f64 {
                    continue; // the player moved away while it was generating
                }
            }
            self.chunks.insert(key, Arc::new(chunk));
            let (c, v) = (self.last_center.unwrap_or(center), self.last_view.max(view));
            let mut around = self.neighbor_keys(key);
            around.push(key);
            for n in around {
                self.mesh_ready_check(n, c, v);
            }
        }

        // 2. Re-plan when the player changed chunk or the view distance changed.
        if self.last_center != Some(center) || self.last_view != view {
            self.replan(center, view);
        }

        // 3. Start generation tasks, nearest first.
        while self.gen_pending.len() < self.max_inflight {
            let Some(key) = self.gen_queue.pop_front() else { break };
            if self.chunks.contains_key(&key) || self.gen_pending.contains(&key) {
                continue;
            }
            self.gen_pending.insert(key);
            let planet = self.planet.clone();
            let storage = self.storage.clone();
            let tx = self.gen_tx.clone();
            rayon::spawn(move || {
                let chunk = storage.load_chunk(key).unwrap_or_else(|| generate_chunk(&planet, key));
                let _ = tx.send((key, chunk));
            });
        }

        // 4. Urgent re-meshes after edits run synchronously.
        let urgent = std::mem::take(&mut self.urgent);
        for key in urgent {
            if let Some(input) = self.build_input(key) {
                let st = self.mesh_state.entry(key).or_default();
                st.requested += 1;
                st.built = st.requested;
                let data = mesh_chunk(&input);
                self.out_meshes.push((key, data));
            } else if self.chunks.contains_key(&key) {
                if let Some(st) = self.mesh_state.get_mut(&key) {
                    st.built = 0;
                }
                self.mesh_ready_check(key, center, view);
            }
        }

        // 5. Start a few meshing tasks.
        let mut started = 0;
        while started < mesh_budget {
            let Some(key) = self.mesh_ready.pop_front() else { break };
            if self.chunk_dist(key, center) > view as f64 {
                continue;
            }
            self.spawn_mesh(key);
            started += 1;
        }

        // 6. Collect finished meshes.
        while let Ok((key, version, data)) = self.mesh_rx.try_recv() {
            self.stats.meshed_total += 1;
            let Some(st) = self.mesh_state.get_mut(&key) else { continue };
            st.pending = false;
            if !self.chunks.contains_key(&key) {
                continue;
            }
            if version == st.requested {
                st.built = version;
                self.out_meshes.push((key, data));
            }
        }

        self.stats.loaded = self.chunks.len();
        self.stats.meshed = self.mesh_state.values().filter(|s| s.built > 0).count();
        self.stats.gen_inflight = self.gen_pending.len();
        self.stats.mesh_inflight = self.mesh_state.values().filter(|s| s.pending).count();
    }

    /// All chunks within `radius` chunks of `center`, nearest first (walks over cube edges).
    pub fn chunks_within(&self, center: ChunkKey, radius: f64) -> Vec<(f64, ChunkKey)> {
        let g = &self.planet.grid;
        let mut seen: HashSet<ChunkKey> = HashSet::new();
        let mut queue = VecDeque::new();
        let mut out = Vec::new();
        seen.insert(center);
        queue.push_back(center);
        while let Some(k) = queue.pop_front() {
            out.push((self.chunk_dist(k, center), k));
            for (axis, sign) in [(0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 1)] {
                let (di, dj, dk) = match axis {
                    0 => (sign, 0, 0),
                    1 => (0, sign, 0),
                    _ => (0, 0, sign),
                };
                if let Some(n) = g.chunk_neighbor(k, di, dj, dk) {
                    if !seen.contains(&n) && self.chunk_dist(n, center) <= radius {
                        seen.insert(n);
                        queue.push_back(n);
                    }
                }
            }
        }
        out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    fn replan(&mut self, center: ChunkKey, view: i32) {
        self.last_center = Some(center);
        self.last_view = view;
        let gen_r = (view + 1) as f64;

        // Unload far chunks (hysteresis of 2 chunks).
        let keep = (view + 3) as f64;
        let far: Vec<ChunkKey> = self.chunks.keys().copied().filter(|k| self.chunk_dist(*k, center) > keep).collect();
        for key in far {
            if let Some(arc) = self.chunks.remove(&key) {
                if arc.modified {
                    self.storage.store_chunk(key, &arc);
                }
            }
            self.mesh_state.remove(&key);
            self.out_removed.push(key);
        }
        self.mesh_ready.retain(|k| (self.planet.grid.chunk_center(*k) - self.planet.grid.chunk_center(center)).length() / 16.0 <= view as f64);

        // Everything inside the generation sphere that is missing, nearest first.
        let want = self.chunks_within(center, gen_r);
        self.gen_queue = want.iter().filter(|(_, k)| !self.chunks.contains_key(k)).map(|(_, k)| *k).collect();

        // Loaded chunks inside the mesh sphere that still need a mesh.
        let ready: Vec<ChunkKey> = want.iter().filter(|(d, k)| *d <= view as f64 && self.chunks.contains_key(k)).map(|(_, k)| *k).collect();
        for key in ready {
            self.mesh_ready_check(key, center, view);
        }
    }

    pub fn take_meshes(&mut self) -> Vec<(ChunkKey, MeshData)> {
        std::mem::take(&mut self.out_meshes)
    }

    pub fn take_removed(&mut self) -> Vec<ChunkKey> {
        std::mem::take(&mut self.out_removed)
    }

    /// (ready, total) among chunks inside `radius` of `center`: used for the loading screen.
    pub fn progress(&self, center: ChunkKey, radius: i32) -> (usize, usize) {
        let list = self.chunks_within(center, radius as f64);
        let done = list.iter().filter(|(_, k)| self.mesh_state.get(k).map(|s| s.built > 0 && !s.pending).unwrap_or(false)).count();
        if std::env::var("WC_DEBUG_PROGRESS").is_ok() {
            for (d, k) in &list {
                if !self.mesh_state.get(k).map(|s| s.built > 0 && !s.pending).unwrap_or(false) {
                    eprintln!("not ready {k:?} d={d:.2} loaded={} state={:?}", self.chunks.contains_key(k), self.mesh_state.get(k).map(|s| (s.built, s.pending)));
                }
            }
        }
        (done, list.len())
    }

    /// Map of the chunk layer around `center` for the loading screen: `(2 * radius + 1)^2` cells,
    /// row by row; 0 = not loaded, 1 = generated, 2 = meshed (ready to draw).
    pub fn progress_grid(&self, center: ChunkKey, radius: i32) -> Vec<u8> {
        let g = self.planet.grid;
        let mut out = Vec::with_capacity(((2 * radius + 1) * (2 * radius + 1)) as usize);
        for dj in -radius..=radius {
            for di in -radius..=radius {
                let state = match g.chunk_neighbor(center, di, dj, 0) {
                    Some(k) if self.mesh_state.get(&k).is_some_and(|s| s.built > 0 && !s.pending) => 2,
                    Some(k) if self.chunks.contains_key(&k) => 1,
                    _ => 0,
                };
                out.push(state);
            }
        }
        out
    }

    /// Writes every edited chunk to disk.
    pub fn save_all(&mut self) -> Result<usize, String> {
        let keys: Vec<ChunkKey> = self.chunks.iter().filter(|(_, c)| c.modified).map(|(k, _)| *k).collect();
        for key in keys {
            if let Some(arc) = self.chunks.get_mut(&key) {
                self.storage.store_chunk(key, arc);
                Arc::make_mut(arc).modified = false;
            }
        }
        self.storage.flush()
    }
}

impl Voxels for ChunkManager {
    fn grid(&self) -> &Grid {
        &self.planet.grid
    }

    /// Unloaded space counts as solid so nothing can fall through the world while it streams in.
    fn cell_block(&self, c: Cell) -> Block {
        self.get_block(c).unwrap_or(Block::Stone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use std::time::{Duration, Instant};

    fn manager(dir: &str, seed: u64) -> (ChunkManager, Arc<Planet>) {
        let mut cfg = WorldConfig::new("t", seed);
        cfg.size = PlanetSize::Small;
        cfg.structures = false;
        let path = std::env::temp_dir().join(dir);
        let _ = std::fs::remove_dir_all(&path);
        let storage = Arc::new(Storage::open(&path).unwrap());
        let planet = Arc::new(Planet::new(&cfg));
        (ChunkManager::new(planet.clone(), storage), planet)
    }

    fn pump(m: &mut ChunkManager, center: ChunkKey, view: i32) {
        let t = Instant::now();
        loop {
            m.update(center, view, 32);
            let (done, total) = m.progress(center, view - 1);
            if done == total && m.stats.gen_inflight == 0 && m.stats.mesh_inflight == 0 {
                break;
            }
            assert!(t.elapsed() < Duration::from_secs(60), "streaming timed out: {done}/{total} {:?}", m.stats);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn streams_meshes_and_unloads() {
        let (mut m, planet) = manager("wc_cm_test1", 5);
        let spawn = planet.find_spawn();
        let center = ChunkKey::of(planet.grid.world_to_cell(spawn));
        pump(&mut m, center, 4);
        assert!(m.stats.loaded > 100);
        let meshes = m.take_meshes();
        assert!(meshes.iter().any(|(_, d)| !d.is_empty()), "surface chunks must produce geometry");
        // The block under the spawn is solid and the air at the feet is not.
        let cell = planet.grid.world_to_cell(spawn);
        assert!(m.cell_block(Cell::new(cell.f, cell.i, cell.j, cell.k - 1)).is_solid());
        assert!(!m.cell_block(cell).is_solid());
        // Teleport far away (other face): old chunks unload and are reported.
        let far = ChunkKey::new((center.f + 3) % 6, 20, 20, center.ck);
        m.update(far, 4, 8);
        assert!(!m.take_removed().is_empty());
        assert!(!m.is_loaded(center));
    }

    #[test]
    fn streaming_walks_across_a_cube_edge() {
        // Start next to a face edge: chunks of the neighbouring face must load and mesh too.
        let (mut m, planet) = manager("wc_cm_edge", 8);
        let g = planet.grid;
        let k = (planet.radius as i32) >> 4;
        let center = ChunkKey::new(2, g.chunks_per_edge() - 1, g.chunks_per_edge() / 2, k);
        pump(&mut m, center, 4);
        let faces: HashSet<u8> = m.chunks.keys().map(|k| k.f).collect();
        assert!(faces.len() >= 2, "no chunks from the neighbouring face were loaded: {faces:?}");
        let mesh_faces: HashSet<u8> = m.take_meshes().into_iter().filter(|(_, d)| !d.is_empty()).map(|(k, _)| k.f).collect();
        assert!(mesh_faces.len() >= 2, "no meshes from the neighbouring face: {mesh_faces:?}");
    }

    #[test]
    fn edits_remesh_and_persist() {
        let (mut m, planet) = manager("wc_cm_test2", 6);
        let spawn = planet.find_spawn();
        let cell = planet.grid.world_to_cell(spawn);
        let center = ChunkKey::of(cell);
        pump(&mut m, center, 3);
        m.take_meshes();
        assert!(m.set_block(cell, Block::Lamp));
        m.update(center, 3, 8);
        let meshes = m.take_meshes();
        assert!(!meshes.is_empty(), "edit must trigger remesh");
        assert_eq!(m.get_block(cell), Some(Block::Lamp));
        m.save_all().unwrap();
        // New manager on the same folder sees the edit.
        let path = m.storage.dir().to_path_buf();
        drop(m);
        let storage = Arc::new(Storage::open(&path).unwrap());
        let mut m2 = ChunkManager::new(planet.clone(), storage);
        pump(&mut m2, center, 3);
        assert_eq!(m2.get_block(cell), Some(Block::Lamp), "edit must survive a reload");
    }

    #[test]
    fn editing_next_to_a_seam_updates_the_chunk_on_the_other_face() {
        let (mut m, planet) = manager("wc_cm_seam", 9);
        let g = planet.grid;
        let k = (planet.radius as i32) >> 4;
        let center = ChunkKey::new(2, g.chunks_per_edge() - 1, 10, k);
        pump(&mut m, center, 3);
        m.take_meshes();
        let edge_cell = Cell::new(2, g.n - 1, 170, planet.radius as i32 - 3);
        assert!(m.set_block(edge_cell, Block::Air));
        m.update(center, 3, 8);
        let faces: HashSet<u8> = m.take_meshes().into_iter().map(|(k, _)| k.f).collect();
        assert!(faces.contains(&2), "own chunk remeshed");
        assert!(faces.len() >= 2, "the neighbouring face's chunk must be remeshed too: {faces:?}");
    }

    #[test]
    fn chunks_within_covers_a_ball_over_edges() {
        let (m, planet) = manager("wc_cm_ball", 3);
        let g = planet.grid;
        let c = ChunkKey::new(0, g.chunks_per_edge() - 1, g.chunks_per_edge() - 1, 60);
        let list = m.chunks_within(c, 5.0);
        let faces: HashSet<u8> = list.iter().map(|(_, k)| k.f).collect();
        assert!(faces.len() >= 3, "a corner region touches three faces, got {faces:?}");
        assert!(list.iter().all(|(d, _)| *d <= 5.0));
        assert!(list.windows(2).all(|w| w[0].0 <= w[1].0), "sorted by distance");
    }
}
