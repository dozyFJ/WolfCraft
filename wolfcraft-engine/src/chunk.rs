//! 16x16x16 chunk storage. Chunks are addressed by `cubesphere::ChunkKey`; inside a chunk the
//! block order is `(lk << 8) | (lj << 4) | li`.

use crate::block::Block;
use crate::cubesphere::local_idx;

pub const CHUNK_SIZE: i32 = 16;
pub const CHUNK_VOL: usize = 4096;

/// Dense block storage; `None` means all air (saves memory for sky chunks).
#[derive(Clone)]
pub struct Chunk {
    blocks: Option<Box<[u8; CHUNK_VOL]>>,
    /// Changed by the player since generation, so it must be saved.
    pub modified: bool,
    /// The chunk centre lies above the terrain: a cheap sky-exposure hint for the mesher.
    pub sky_open: bool,
}

impl Chunk {
    pub fn empty() -> Self {
        Self { blocks: None, modified: false, sky_open: true }
    }

    pub fn filled(b: Block) -> Self {
        if b == Block::Air {
            return Self::empty();
        }
        Self { blocks: Some(Box::new([b as u8; CHUNK_VOL])), modified: false, sky_open: false }
    }

    pub fn from_raw(data: Box<[u8; CHUNK_VOL]>) -> Self {
        let mut c = Self { blocks: Some(data), modified: false, sky_open: false };
        c.shrink_if_empty();
        c
    }

    #[inline]
    pub fn get_index(&self, i: usize) -> Block {
        match &self.blocks {
            Some(b) => Block::from_u8(b[i]),
            None => Block::Air,
        }
    }

    #[inline]
    pub fn get(&self, li: i32, lj: i32, lk: i32) -> Block {
        self.get_index(local_idx(li, lj, lk))
    }

    #[inline]
    pub fn set_index(&mut self, i: usize, b: Block) {
        match &mut self.blocks {
            Some(d) => d[i] = b as u8,
            None => {
                if b != Block::Air {
                    let mut d = Box::new([0u8; CHUNK_VOL]);
                    d[i] = b as u8;
                    self.blocks = Some(d);
                }
            }
        }
    }

    #[inline]
    pub fn set(&mut self, li: i32, lj: i32, lk: i32, b: Block) {
        self.set_index(local_idx(li, lj, lk), b);
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_none()
    }

    pub fn raw(&self) -> Option<&[u8; CHUNK_VOL]> {
        self.blocks.as_deref()
    }

    /// Drops the dense array if everything is air.
    pub fn shrink_if_empty(&mut self) {
        if let Some(d) = &self.blocks {
            if d.iter().all(|&b| b == 0) {
                self.blocks = None;
            }
        }
    }

    /// True when every block is opaque (nothing inside can be seen).
    pub fn is_full_opaque(&self) -> bool {
        match &self.blocks {
            Some(d) => d.iter().all(|&b| Block::from_u8(b).is_opaque()),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_and_shrink() {
        let mut c = Chunk::empty();
        assert!(c.is_empty());
        c.set(1, 2, 3, Block::Stone);
        assert_eq!(c.get(1, 2, 3), Block::Stone);
        assert_eq!(c.get(2, 1, 3), Block::Air);
        assert!(!c.is_empty());
        c.set(1, 2, 3, Block::Air);
        c.shrink_if_empty();
        assert!(c.is_empty());
    }

    #[test]
    fn filled_and_opaque() {
        assert!(Chunk::filled(Block::Stone).is_full_opaque());
        assert!(!Chunk::filled(Block::Water).is_full_opaque());
        assert!(Chunk::filled(Block::Air).is_empty());
    }
}
