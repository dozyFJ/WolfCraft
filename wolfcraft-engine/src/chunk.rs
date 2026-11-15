//! 16x16x16 chunk storage. Chunks are addressed by `cubesphere::ChunkKey`; inside a chunk the
//! block order is `(lk << 8) | (lj << 4) | li`.

use crate::block::Block;
use crate::cubesphere::local_idx;

pub const CHUNK_SIZE: i32 = 16;
pub const CHUNK_VOL: usize = 4096;

/// How the blocks of a chunk are kept (like the FerriteCore / vanilla palette): all air, one
/// block everywhere (deep rock), or a palette of the kinds used plus 1–16 bits per block.
/// A typical mixed chunk takes 1–2 KB instead of 8 KB.
#[derive(Clone)]
enum Blocks {
    Air,
    Uniform(u16),
    Packed { palette: Vec<u16>, bits: u32, words: Box<[u64]> },
}

fn words_for(bits: u32) -> Box<[u64]> {
    vec![0u64; CHUNK_VOL / (64 / bits as usize)].into_boxed_slice()
}

impl Blocks {
    #[inline]
    fn get(&self, i: usize) -> u16 {
        match self {
            Blocks::Air => 0,
            Blocks::Uniform(b) => *b,
            Blocks::Packed { palette, bits, words } => {
                let per = 64 / *bits as usize;
                let w = words[i / per];
                let k = (w >> ((i % per) as u32 * bits)) & ((1u64 << bits) - 1);
                palette[k as usize]
            }
        }
    }

    fn set(&mut self, i: usize, id: u16) {
        match self {
            Blocks::Air if id == 0 => {}
            Blocks::Uniform(b) if *b == id => {}
            Blocks::Air | Blocks::Uniform(_) => {
                let base = self.get(0);
                let mut words = words_for(1);
                let per = 64;
                words[i / per] |= 1u64 << (i % per);
                *self = Blocks::Packed { palette: vec![base, id], bits: 1, words };
            }
            Blocks::Packed { palette, bits, words } => {
                let k = match palette.iter().position(|&p| p == id) {
                    Some(k) => k,
                    None => {
                        if palette.len() == 1usize << *bits {
                            // Full: twice the bits per block, copy everything over.
                            let nb = (*bits * 2).min(16);
                            let mut nw = words_for(nb);
                            let (po, pn) = (64 / *bits as usize, 64 / nb as usize);
                            let mask = (1u64 << *bits) - 1;
                            for j in 0..CHUNK_VOL {
                                let v = (words[j / po] >> ((j % po) as u32 * *bits)) & mask;
                                nw[j / pn] |= v << ((j % pn) as u32 * nb);
                            }
                            *bits = nb;
                            *words = nw;
                        }
                        palette.push(id);
                        palette.len() - 1
                    }
                };
                let per = 64 / *bits as usize;
                let sh = (i % per) as u32 * *bits;
                let w = &mut words[i / per];
                *w = (*w & !(((1u64 << *bits) - 1) << sh)) | ((k as u64) << sh);
            }
        }
    }

    fn from_dense(d: &[u16; CHUNK_VOL]) -> Self {
        let first = d[0];
        if d.iter().all(|&b| b == first) {
            return if first == 0 { Blocks::Air } else { Blocks::Uniform(first) };
        }
        let mut palette: Vec<u16> = Vec::new();
        for &b in d.iter() {
            if !palette.contains(&b) {
                palette.push(b);
                if palette.len() > 256 {
                    break;
                }
            }
        }
        let n = palette.len();
        let bits = [1u32, 2, 4, 8, 16].into_iter().find(|b| (1usize << b) >= n || *b == 16).unwrap_or(16);
        let mut words = words_for(bits);
        let per = 64 / bits as usize;
        let mut pal: Vec<u16> = Vec::with_capacity(n);
        for (i, &b) in d.iter().enumerate() {
            let k = match pal.iter().position(|&p| p == b) {
                Some(k) => k,
                None => {
                    pal.push(b);
                    pal.len() - 1
                }
            };
            words[i / per] |= (k as u64) << ((i % per) as u32 * bits);
        }
        Blocks::Packed { palette: pal, bits, words }
    }

    fn to_dense(&self) -> Option<Box<[u16; CHUNK_VOL]>> {
        match self {
            Blocks::Air => None,
            Blocks::Uniform(b) => Some(Box::new([*b; CHUNK_VOL])),
            Blocks::Packed { .. } => {
                let mut d = Box::new([0u16; CHUNK_VOL]);
                for (i, v) in d.iter_mut().enumerate() {
                    *v = self.get(i);
                }
                Some(d)
            }
        }
    }

    /// Bytes this storage takes (for the memory test and the debug screen).
    fn bytes(&self) -> usize {
        match self {
            Blocks::Air | Blocks::Uniform(_) => 0,
            Blocks::Packed { palette, words, .. } => palette.len() * 2 + words.len() * 8,
        }
    }
}

#[derive(Clone)]
pub struct Chunk {
    blocks: Blocks,
    /// Changed by the player since generation, so it must be saved.
    pub modified: bool,
    /// The chunk centre lies above the terrain: a cheap sky-exposure hint for the mesher.
    pub sky_open: bool,
    /// Light-emitting blocks (index, light level), kept up to date by `set`, so lighting
    /// neighbouring chunks does not need to scan this one.
    emitters: Vec<(u16, u8)>,
}

impl Chunk {
    pub fn empty() -> Self {
        Self { blocks: Blocks::Air, modified: false, sky_open: true, emitters: Vec::new() }
    }

    pub fn filled(b: Block) -> Self {
        if b == Block::Air {
            return Self::empty();
        }
        let emitters = if b.light_emission() > 0 { (0..CHUNK_VOL as u16).map(|i| (i, b.light_emission())).collect() } else { Vec::new() };
        Self { blocks: Blocks::Uniform(b as u16), modified: false, sky_open: false, emitters }
    }

    pub fn from_raw(data: Box<[u16; CHUNK_VOL]>) -> Self {
        let emitters = data.iter().enumerate().filter_map(|(i, &b)| Some((i as u16, Block::from_id(b).light_emission())).filter(|e| e.1 > 0)).collect();
        Self { blocks: Blocks::from_dense(&data), modified: false, sky_open: false, emitters }
    }

    #[inline]
    pub fn get_index(&self, i: usize) -> Block {
        Block::from_id(self.blocks.get(i))
    }

    #[inline]
    pub fn get(&self, li: i32, lj: i32, lk: i32) -> Block {
        self.get_index(local_idx(li, lj, lk))
    }

    #[inline]
    pub fn set_index(&mut self, i: usize, b: Block) {
        let level = b.light_emission();
        if level > 0 || !self.emitters.is_empty() {
            self.emitters.retain(|e| e.0 as usize != i);
            if level > 0 {
                self.emitters.push((i as u16, level));
            }
        }
        self.blocks.set(i, b as u16);
    }

    #[inline]
    pub fn set(&mut self, li: i32, lj: i32, lk: i32, b: Block) {
        self.set_index(local_idx(li, lj, lk), b);
    }

    /// Light sources in this chunk: (index, level).
    pub fn emitters(&self) -> &[(u16, u8)] {
        &self.emitters
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.blocks, Blocks::Air)
    }

    /// Every block as a plain array (`None` = all air), e.g. for saving.
    pub fn raw(&self) -> Option<Box<[u16; CHUNK_VOL]>> {
        self.blocks.to_dense()
    }

    /// Memory the blocks take, in bytes.
    pub fn block_bytes(&self) -> usize {
        self.blocks.bytes()
    }

    /// Drops the storage if everything is air, and packs it tighter when it can.
    pub fn shrink_if_empty(&mut self) {
        if let Blocks::Packed { .. } = &self.blocks {
            if let Some(d) = self.blocks.to_dense() {
                self.blocks = Blocks::from_dense(&d);
            }
        }
    }

    /// True when every block is opaque (nothing inside can be seen).
    pub fn is_full_opaque(&self) -> bool {
        match &self.blocks {
            Blocks::Air => false,
            Blocks::Uniform(b) => Block::from_id(*b).is_opaque(),
            Blocks::Packed { .. } => (0..CHUNK_VOL).all(|i| Block::from_id(self.blocks.get(i)).is_opaque()),
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
    fn palette_storage_matches_a_plain_array() {
        let mut c = Chunk::empty();
        let mut plain = [0u16; CHUNK_VOL];
        let mut x = 12345u32;
        // Many kinds of blocks, written and overwritten in random places.
        for n in 0..20_000 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = (x as usize) % CHUNK_VOL;
            let kinds = if n < 10_000 { 6 } else { 300 };
            let id = Block::ALL[(x >> 12) as usize % kinds.min(Block::ALL.len())];
            c.set_index(i, id);
            plain[i] = id as u16;
        }
        for i in 0..CHUNK_VOL {
            assert_eq!(c.get_index(i) as u16, plain[i], "block {i}");
        }
        assert_eq!(&*c.raw().unwrap(), &plain);
    }

    #[test]
    fn rock_and_mixed_chunks_take_little_memory() {
        let rock = Chunk::from_raw(Box::new([Block::Stone as u16; CHUNK_VOL]));
        assert_eq!(rock.block_bytes(), 0);
        assert!(rock.is_full_opaque());
        let mut d = Box::new([Block::Stone as u16; CHUNK_VOL]);
        for (i, v) in d.iter_mut().enumerate() {
            if i % 7 == 0 {
                *v = Block::Dirt as u16;
            }
            if i > 3000 {
                *v = 0;
            }
        }
        let mixed = Chunk::from_raw(d);
        assert!(mixed.block_bytes() <= 1100, "{} bytes", mixed.block_bytes());
        assert_eq!(mixed.get_index(7), Block::Dirt);
        assert_eq!(mixed.get_index(4000), Block::Air);
    }

    #[test]
    fn filled_and_opaque() {
        assert!(Chunk::filled(Block::Stone).is_full_opaque());
        assert!(!Chunk::filled(Block::Water).is_full_opaque());
        assert!(Chunk::filled(Block::Air).is_empty());
    }
}

