//! World storage: `worlds/<id>/{world.toml, player.dat, regions/, structures/}`.
//!
//! Only chunks changed by the player are stored. They are grouped into regions of 8x8x8
//! chunks of one cube face (one file per region, RLE-compressed). Everything else is
//! regenerated from the seed.

use crate::chunk::{CHUNK_VOL, Chunk};
use crate::cubesphere::ChunkKey;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAGIC: &[u8; 4] = b"WCR1";
/// (face, region i, region j, region k)
type RegionKey = (u8, i32, i32, i32);

#[derive(Default)]
struct Region {
    chunks: HashMap<u16, Vec<u8>>,
    dirty: bool,
}

pub struct Storage {
    dir: PathBuf,
    regions: Mutex<HashMap<RegionKey, Region>>,
}

fn region_of(key: ChunkKey) -> (RegionKey, u16) {
    let r = (key.f, key.ci >> 3, key.cj >> 3, key.ck >> 3);
    let idx = (key.ci & 7) + (key.cj & 7) * 8 + (key.ck & 7) * 64;
    (r, idx as u16)
}

pub fn rle_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let v = data[i];
        let mut n = 1usize;
        while i + n < data.len() && data[i + n] == v && n < 65535 {
            n += 1;
        }
        out.extend_from_slice(&(n as u16).to_le_bytes());
        out.push(v);
        i += n;
    }
    out
}

pub fn rle_decode(data: &[u8], expected: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
        out.extend(std::iter::repeat(data[i + 2]).take(n));
        i += 3;
        if out.len() > expected {
            return None;
        }
    }
    (out.len() == expected).then_some(out)
}

impl Storage {
    /// Opens (and creates) the world folder structure.
    pub fn open(world_dir: &Path) -> Result<Self, String> {
        for sub in ["regions", "structures"] {
            std::fs::create_dir_all(world_dir.join(sub)).map_err(|e| format!("{}: {e}", world_dir.display()))?;
        }
        Ok(Self { dir: world_dir.to_path_buf(), regions: Mutex::new(HashMap::new()) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn region_path(&self, r: RegionKey) -> PathBuf {
        self.dir.join("regions").join(format!("r.{}.{}.{}.{}.wcr", r.0, r.1, r.2, r.3))
    }

    fn read_region(&self, r: RegionKey) -> Region {
        let Ok(bytes) = std::fs::read(self.region_path(r)) else { return Region::default() };
        let mut region = Region::default();
        if bytes.len() < 8 || &bytes[0..4] != MAGIC {
            log::warn!("region {r:?}: bad header, ignoring file");
            return region;
        }
        let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let mut off = 8;
        for _ in 0..count {
            if off + 6 > bytes.len() {
                log::warn!("region {r:?}: truncated");
                break;
            }
            let idx = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
            let len = u32::from_le_bytes(bytes[off + 2..off + 6].try_into().unwrap()) as usize;
            off += 6;
            if off + len > bytes.len() {
                log::warn!("region {r:?}: truncated chunk");
                break;
            }
            region.chunks.insert(idx, bytes[off..off + len].to_vec());
            off += len;
        }
        region
    }

    /// Returns the saved version of a chunk, if the player changed it earlier.
    pub fn load_chunk(&self, key: ChunkKey) -> Option<Chunk> {
        let (r, idx) = region_of(key);
        let mut regions = self.regions.lock().ok()?;
        let region = regions.entry(r).or_insert_with(|| self.read_region(r));
        let comp = region.chunks.get(&idx)?;
        if comp.is_empty() {
            return Some(Chunk::empty());
        }
        let raw = rle_decode(comp, CHUNK_VOL)?;
        let arr: Box<[u8; CHUNK_VOL]> = raw.into_boxed_slice().try_into().ok()?;
        let mut c = Chunk::from_raw(arr);
        c.modified = false;
        c.sky_open = false;
        Some(c)
    }

    /// Stores a modified chunk in memory; call `flush` to write files.
    pub fn store_chunk(&self, key: ChunkKey, chunk: &Chunk) {
        let (r, idx) = region_of(key);
        let Ok(mut regions) = self.regions.lock() else { return };
        let region = regions.entry(r).or_insert_with(|| self.read_region(r));
        let data = match chunk.raw() {
            Some(raw) => rle_encode(raw),
            None => Vec::new(),
        };
        region.chunks.insert(idx, data);
        region.dirty = true;
    }

    /// Writes all changed regions to disk (atomically via temp file + rename).
    pub fn flush(&self) -> Result<usize, String> {
        let Ok(mut regions) = self.regions.lock() else { return Err("storage lock poisoned".into()) };
        let mut written = 0;
        for (r, region) in regions.iter_mut() {
            if !region.dirty {
                continue;
            }
            let mut buf = Vec::new();
            buf.extend_from_slice(MAGIC);
            buf.extend_from_slice(&(region.chunks.len() as u32).to_le_bytes());
            let mut keys: Vec<_> = region.chunks.keys().copied().collect();
            keys.sort_unstable();
            for k in keys {
                let d = &region.chunks[&k];
                buf.extend_from_slice(&k.to_le_bytes());
                buf.extend_from_slice(&(d.len() as u32).to_le_bytes());
                buf.extend_from_slice(d);
            }
            let path = self.region_path(*r);
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, &buf).map_err(|e| format!("{}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
            region.dirty = false;
            written += 1;
        }
        Ok(written)
    }

    pub fn player_path(&self) -> PathBuf {
        self.dir.join("player.dat")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;

    #[test]
    fn rle_roundtrip() {
        let mut data = vec![0u8; 4096];
        for (i, d) in data.iter_mut().enumerate() {
            *d = ((i / 37) % 5) as u8;
        }
        let enc = rle_encode(&data);
        assert!(enc.len() < data.len());
        assert_eq!(rle_decode(&enc, 4096).unwrap(), data);
        assert!(rle_decode(&enc, 4095).is_none());
    }

    #[test]
    fn save_and_reload_modified_chunks() {
        let dir = std::env::temp_dir().join("wc_storage_test");
        let _ = std::fs::remove_dir_all(&dir);
        let key = ChunkKey::new(3, -3, 70, 12);
        let other_face = ChunkKey::new(4, -3, 70, 12);
        {
            let s = Storage::open(&dir).unwrap();
            let mut c = Chunk::filled(Block::Stone);
            c.set(1, 2, 3, Block::Lamp);
            c.set(15, 15, 15, Block::Air);
            s.store_chunk(key, &c);
            s.store_chunk(ChunkKey::new(0, 0, 0, 0), &Chunk::empty());
            assert_eq!(s.flush().unwrap(), 2);
        }
        let s = Storage::open(&dir).unwrap();
        let c = s.load_chunk(key).expect("chunk persisted");
        assert_eq!(c.get(1, 2, 3), Block::Lamp);
        assert_eq!(c.get(15, 15, 15), Block::Air);
        assert_eq!(c.get(0, 0, 0), Block::Stone);
        assert!(s.load_chunk(ChunkKey::new(0, 0, 0, 0)).unwrap().is_empty());
        assert!(s.load_chunk(ChunkKey::new(5, 5, 5, 5)).is_none());
        assert!(s.load_chunk(other_face).is_none(), "same indices on another face are a different chunk");
    }

    #[test]
    fn region_math_handles_negatives_and_faces() {
        assert_eq!(region_of(ChunkKey::new(2, -1, 0, 8)), ((2, -1, 0, 1), 7));
        assert_eq!(region_of(ChunkKey::new(5, -8, -9, 7)).0, (5, -1, -2, 0));
        assert_ne!(region_of(ChunkKey::new(1, 0, 0, 0)).0, region_of(ChunkKey::new(2, 0, 0, 0)).0);
    }
}
