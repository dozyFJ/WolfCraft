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

const MAGIC: &[u8; 4] = b"WCR3";
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

pub fn rle_encode(data: &[u16]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let v = data[i];
        let mut n = 1usize;
        while i + n < data.len() && data[i + n] == v && n < 65535 {
            n += 1;
        }
        out.extend_from_slice(&(n as u16).to_le_bytes());
        out.extend_from_slice(&v.to_le_bytes());
        i += n;
    }
    out
}

pub fn rle_decode(data: &[u8], expected: usize) -> Option<Vec<u16>> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i + 4 <= data.len() {
        let n = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
        if n == 0 || n > expected.saturating_sub(out.len()) { return None; }
        out.extend(std::iter::repeat_n(u16::from_le_bytes([data[i + 2], data[i + 3]]), n));
        i += 4;
        if out.len() > expected {
            return None;
        }
    }
    (out.len() == expected && i == data.len()).then_some(out)
}

/// CRC-32 (IEEE), to notice files damaged on the disk or cut by a power loss.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

fn parse_region(bytes: &[u8]) -> Result<Region, String> {
    let mut region = Region::default();
    if bytes.len() < 8 {
        return Err("too short".into());
    }
    let magic = &bytes[0..4];
    let body = if magic == MAGIC {
        // WCR3: a checksum over everything before it.
        if bytes.len() < 12 {
            return Err("too short".into());
        }
        let (body, tail) = bytes.split_at(bytes.len() - 4);
        if crc32(body) != u32::from_le_bytes(tail.try_into().unwrap()) {
            return Err("checksum mismatch".into());
        }
        body
    } else if magic == b"WCR2" || magic == b"WCR1" {
        bytes
    } else {
        return Err("bad header".into());
    };
    let legacy = magic == b"WCR1";
    let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
    let mut off = 8;
    for _ in 0..count {
        if off + 6 > body.len() {
            return Err("truncated".into());
        }
        let idx = u16::from_le_bytes([body[off], body[off + 1]]);
        let len = u32::from_le_bytes(body[off + 2..off + 6].try_into().unwrap()) as usize;
        off += 6;
        if off + len > body.len() {
            return Err("truncated chunk".into());
        }
        let comp = &body[off..off + len];
        let data = if legacy && !comp.is_empty() {
            let mut raw = Vec::with_capacity(CHUNK_VOL);
            let mut i = 0;
            while i + 3 <= comp.len() {
                let n = u16::from_le_bytes([comp[i], comp[i + 1]]) as usize;
                if n == 0 || n > CHUNK_VOL.saturating_sub(raw.len()) {
                    break;
                }
                raw.extend(std::iter::repeat_n(comp[i + 2] as u16, n));
                i += 3;
            }
            if raw.len() != CHUNK_VOL || i != comp.len() {
                log::warn!("invalid legacy chunk {idx}");
                off += len;
                continue;
            }
            rle_encode(&raw)
        } else {
            comp.to_vec()
        };
        region.chunks.insert(idx, data);
        off += len;
    }
    Ok(region)
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

    /// Reads a region file. A damaged file (bad header, cut short, wrong checksum) is not
    /// trusted: it is copied aside to `regions/corrupt/` and the previous good copy (`.bak`,
    /// kept by every save) is used instead — like Java's handling of broken chunk files.
    fn read_region(&self, r: RegionKey) -> Region {
        let path = self.region_path(r);
        let Ok(bytes) = std::fs::read(&path) else { return Region::default() };
        match parse_region(&bytes) {
            Ok(region) => region,
            Err(why) => {
                log::warn!("region {r:?}: {why}; trying the backup");
                let bad = self.dir.join("regions").join("corrupt");
                let _ = std::fs::create_dir_all(&bad);
                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                if let Some(name) = path.file_name() {
                    let _ = std::fs::copy(&path, bad.join(format!("{}.{stamp}", name.to_string_lossy())));
                }
                match std::fs::read(path.with_extension("bak")).map_err(|e| e.to_string()).and_then(|b| parse_region(&b)) {
                    Ok(mut region) => {
                        log::warn!("region {r:?}: restored from the backup");
                        // Write the good copy back at the next save.
                        region.dirty = true;
                        region
                    }
                    Err(e) => {
                        log::warn!("region {r:?}: no usable backup ({e}); the area regenerates");
                        Region::default()
                    }
                }
            }
        }
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
        let arr: Box<[u16; CHUNK_VOL]> = raw.into_boxed_slice().try_into().ok()?;
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
            let crc = crc32(&buf);
            buf.extend_from_slice(&crc.to_le_bytes());
            let path = self.region_path(*r);
            let tmp = path.with_extension("tmp");
            {
                use std::io::Write;
                let mut f = std::fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
                f.write_all(&buf).map_err(|e| format!("{}: {e}", tmp.display()))?;
                // On the disk for real before the swap: a power cut cannot leave half a file.
                let _ = f.sync_all();
            }
            // The previous good version stays as .bak for recovery.
            if path.exists() {
                let _ = std::fs::rename(&path, path.with_extension("bak"));
            }
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
    fn a_damaged_region_comes_back_from_the_backup() {
        let dir = std::env::temp_dir().join(format!("wc_backup_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let key = ChunkKey::new(0, 0, 0, 0);
        let storage = Storage::open(&dir).unwrap();
        let mut c = Chunk::empty();
        c.set(1, 2, 3, Block::Stone);
        storage.store_chunk(key, &c);
        storage.flush().unwrap();
        // Second save keeps the first as .bak.
        c.set(4, 4, 4, Block::Dirt);
        storage.store_chunk(key, &c);
        storage.flush().unwrap();
        let path = storage.region_path((0, 0, 0, 0));
        assert!(path.with_extension("bak").exists());
        // A bit flips on the disk.
        let mut bytes = std::fs::read(&path).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x40;
        std::fs::write(&path, bytes).unwrap();
        let fresh = Storage::open(&dir).unwrap();
        let got = fresh.load_chunk(key).expect("restored from the backup");
        assert_eq!(got.get(1, 2, 3), Block::Stone);
        assert!(std::fs::read_dir(dir.join("regions").join("corrupt")).unwrap().count() >= 1, "the broken file is kept aside");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crc32_matches_the_standard() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn old_region_migrates_and_keeps_wide_furniture_ids() {
        use crate::construction::{self, Form};
        let dir=std::env::temp_dir().join(format!("wc_wide_migration_{}",std::process::id()));
        let _=std::fs::remove_dir_all(&dir);
        let key=ChunkKey::new(0,0,0,0);
        let storage=Storage::open(&dir).unwrap();
        let mut old=b"WCR1".to_vec();old.extend(1u32.to_le_bytes());old.extend(0u16.to_le_bytes());old.extend(3u32.to_le_bytes());old.extend(4096u16.to_le_bytes());old.push(Block::Stone as u8);
        std::fs::write(storage.region_path((0,0,0,0)),old).unwrap();
        let mut chunk=storage.load_chunk(key).unwrap();
        assert_eq!(chunk.get(0,0,0),Block::Stone);
        let chair=construction::block(Form::Chair,0,3).unwrap();
        chunk.set(1,2,3,chair);storage.store_chunk(key,&chunk);storage.flush().unwrap();drop(storage);
        let storage=Storage::open(&dir).unwrap();
        let chunk=storage.load_chunk(key).unwrap();
        assert_eq!(chunk.get(1,2,3),chair);
        assert_eq!(chunk.get(0,0,0),Block::Stone);
        assert_eq!(&std::fs::read(storage.region_path((0,0,0,0))).unwrap()[..4],b"WCR3");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn wide_rle_rejects_trailing_bytes_and_zero_runs() {
        let mut enc=rle_encode(&[2319,2319,1,0]);assert_eq!(rle_decode(&enc,4),Some(vec![2319,2319,1,0]));
        enc.push(0);assert!(rle_decode(&enc,4).is_none());
        assert!(rle_decode(&[0,0,1,0],1).is_none());
    }

    #[test]
    fn rle_roundtrip() {
        let mut data = vec![0u16; 4096];
        for (i, d) in data.iter_mut().enumerate() {
            *d = ((i / 37) % 5) as u16;
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

