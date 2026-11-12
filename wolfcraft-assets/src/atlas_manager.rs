//! Atlas Manager: one packed texture per category with UV lookup, PNG overrides/export and
//! in-place dynamic sprite updates (used for animated water).
//!
//! Categories are kept in separate textures on purpose: the world pass binds only the block
//! atlas, the GUI only the GUI/items atlases, so no single giant texture has to be resident
//! and sampled by every pass.

use crate::image::Image;
use crate::procedural;
use crate::texture_packer::{self, PackedRect};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AtlasKind {
    Blocks,
    Items,
    Entities,
    Gui,
    Particles,
}

impl AtlasKind {
    pub const ALL: [AtlasKind; 5] = [AtlasKind::Blocks, AtlasKind::Items, AtlasKind::Entities, AtlasKind::Gui, AtlasKind::Particles];

    pub fn file_name(self) -> &'static str {
        match self {
            AtlasKind::Blocks => "blocks_atlas.png",
            AtlasKind::Items => "items_atlas.png",
            AtlasKind::Entities => "entities_atlas.png",
            AtlasKind::Gui => "gui_atlas.png",
            AtlasKind::Particles => "particles_atlas.png",
        }
    }
    pub fn folder(self) -> &'static str {
        match self {
            AtlasKind::Blocks => "blocks",
            AtlasKind::Items => "items",
            AtlasKind::Entities => "entities",
            AtlasKind::Gui => "gui",
            AtlasKind::Particles => "particles",
        }
    }
    /// Pixels of extruded padding around each sprite.
    fn padding(self) -> u32 {
        match self {
            AtlasKind::Blocks => 2,
            _ => 1,
        }
    }
}

/// Normalized UV rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvRect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

/// Region of the atlas that changed and must be re-uploaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirtyRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub struct TextureAtlas {
    kind: AtlasKind,
    sprites: Vec<(String, Image)>,
    names: HashMap<String, usize>,
    image: Image,
    rects: Vec<PackedRect>,
    uvs: Vec<UvRect>,
    /// Bumped whenever the atlas is repacked (size/layout may change → recreate GPU texture).
    generation: u64,
    dirty: Vec<DirtyRect>,
}

impl TextureAtlas {
    pub fn build(kind: AtlasKind, sprites: Vec<(String, Image)>) -> Result<Self, String> {
        let names = sprites.iter().enumerate().map(|(i, (n, _))| (n.clone(), i)).collect();
        let mut atlas = Self {
            kind,
            sprites,
            names,
            image: Image::new(1, 1),
            rects: vec![],
            uvs: vec![],
            generation: 0,
            dirty: vec![],
        };
        atlas.repack()?;
        Ok(atlas)
    }

    fn repack(&mut self) -> Result<(), String> {
        let pad = self.kind.padding();
        let sizes: Vec<(u32, u32)> = self.sprites.iter().map(|(_, i)| (i.width, i.height)).collect();
        let packed = texture_packer::pack(&sizes, pad, 4096).ok_or_else(|| format!("{:?} atlas does not fit 4096x4096", self.kind))?;
        let mut image = Image::new(packed.width, packed.height);
        for ((_, sprite), r) in self.sprites.iter().zip(&packed.rects) {
            image.blit_extruded(sprite, r.x, r.y, pad);
        }
        let (aw, ah) = (packed.width as f32, packed.height as f32);
        self.uvs = packed
            .rects
            .iter()
            .map(|r| UvRect { min: [r.x as f32 / aw, r.y as f32 / ah], max: [(r.x + r.w) as f32 / aw, (r.y + r.h) as f32 / ah] })
            .collect();
        self.rects = packed.rects;
        self.image = image;
        self.generation += 1;
        self.dirty.clear();
        Ok(())
    }

    pub fn kind(&self) -> AtlasKind {
        self.kind
    }
    pub fn image(&self) -> &Image {
        &self.image
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn len(&self) -> usize {
        self.sprites.len()
    }
    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }
    pub fn uv(&self, id: usize) -> UvRect {
        self.uvs.get(id).copied().unwrap_or(UvRect { min: [0.0; 2], max: [0.0; 2] })
    }
    pub fn uvs(&self) -> &[UvRect] {
        &self.uvs
    }
    pub fn rect(&self, id: usize) -> PackedRect {
        self.rects[id]
    }
    pub fn id(&self, name: &str) -> Option<usize> {
        self.names.get(name).copied()
    }
    pub fn name(&self, id: usize) -> &str {
        &self.sprites[id].0
    }

    /// Replaces a sprite. Same size → in-place write + dirty rect (cheap partial upload);
    /// different size → full repack (generation changes).
    pub fn update_sprite(&mut self, id: usize, img: Image) -> Result<(), String> {
        let Some(slot) = self.sprites.get_mut(id) else { return Err(format!("sprite {id} out of range")) };
        if slot.1.width == img.width && slot.1.height == img.height {
            let r = self.rects[id];
            let pad = self.kind.padding();
            self.image.blit_extruded(&img, r.x, r.y, pad);
            slot.1 = img;
            self.dirty.push(DirtyRect { x: r.x - pad, y: r.y - pad, w: r.w + 2 * pad, h: r.h + 2 * pad });
            Ok(())
        } else {
            slot.1 = img;
            self.repack()
        }
    }

    /// Returns and clears pending partial updates.
    pub fn take_dirty(&mut self) -> Vec<DirtyRect> {
        std::mem::take(&mut self.dirty)
    }

    pub fn export_png(&self, dir: &Path) -> Result<(), String> {
        self.image.save_png(&dir.join(self.kind.file_name()))
    }
}

pub struct AtlasManager {
    atlases: Vec<TextureAtlas>,
    water_frame: u32,
    animate_water: bool,
}

impl AtlasManager {
    /// Builds all atlases from procedural sprites, applying PNG overrides from
    /// `override_dir/<category>/<sprite>.png` when present.
    pub fn new(override_dir: Option<&Path>) -> Result<Self, String> {
        let mut atlases = Vec::new();
        let mut animate_water = true;
        for kind in AtlasKind::ALL {
            let mut sprites = procedural::sprites(kind);
            if let Some(dir) = override_dir {
                for (name, img) in sprites.iter_mut() {
                    let p = dir.join(kind.folder()).join(format!("{name}.png"));
                    if p.exists() {
                        match Image::load_png(&p) {
                            Ok(o) => {
                                log::info!("texture override: {}", p.display());
                                if kind == AtlasKind::Blocks && name == "water" {
                                    animate_water = false;
                                }
                                *img = o;
                            }
                            Err(e) => log::warn!("override failed: {e}"),
                        }
                    }
                }
            }
            atlases.push(TextureAtlas::build(kind, sprites)?);
        }
        Ok(Self { atlases, water_frame: 0, animate_water })
    }

    pub fn get(&self, kind: AtlasKind) -> &TextureAtlas {
        &self.atlases[kind as usize]
    }
    pub fn get_mut(&mut self, kind: AtlasKind) -> &mut TextureAtlas {
        &mut self.atlases[kind as usize]
    }

    pub fn export_all(&self, dir: &Path) -> Result<(), String> {
        for a in &self.atlases {
            a.export_png(dir)?;
        }
        Ok(())
    }

    /// Advances texture animations (water). Returns true if anything changed.
    pub fn animate(&mut self, time_s: f32) -> bool {
        if !self.animate_water {
            return false;
        }
        let frame = (time_s * 6.0) as u32 % 16;
        if frame == self.water_frame {
            return false;
        }
        self.water_frame = frame;
        let blocks = self.get_mut(AtlasKind::Blocks);
        let id = procedural::tiles::WATER as usize;
        let size = blocks.sprites[id].1.width;
        if size != 16 {
            return false; // overridden by a pack with another size: keep it static
        }
        blocks.update_sprite(id, procedural::water_frame(frame)).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_atlases_build_and_export() {
        let m = AtlasManager::new(None).unwrap();
        for kind in AtlasKind::ALL {
            let a = m.get(kind);
            assert!(!a.is_empty(), "{kind:?} empty");
            for i in 0..a.len() {
                let uv = a.uv(i);
                assert!(uv.min[0] < uv.max[0] && uv.max[0] <= 1.0 && uv.max[1] <= 1.0);
            }
        }
        let dir = std::env::temp_dir().join("wolfcraft_atlas_test");
        m.export_all(&dir).unwrap();
        assert!(dir.join("gui_atlas.png").exists());
        let back = Image::load_png(&dir.join("blocks_atlas.png")).unwrap();
        assert_eq!(&back, m.get(AtlasKind::Blocks).image());
    }

    #[test]
    fn dynamic_update_marks_dirty_and_repacks() {
        let mut m = AtlasManager::new(None).unwrap();
        let g0 = m.get(AtlasKind::Blocks).generation();
        assert!(m.animate(1.0));
        let blocks = m.get_mut(AtlasKind::Blocks);
        assert_eq!(blocks.generation(), g0, "same-size update must not repack");
        assert_eq!(blocks.take_dirty().len(), 1);
        blocks.update_sprite(0, Image::new(32, 32)).unwrap();
        assert_eq!(blocks.generation(), g0 + 1, "resized sprite must repack");
        assert_eq!(blocks.rect(0).w, 32);
    }

    #[test]
    fn water_override_survives_animation_at_native_and_pack_sizes() {
        for size in [16, 32] {
            let dir = std::env::temp_dir().join(format!("wolfcraft_water_override_{}_{size}", std::process::id()));
            let mut water = Image::new(size, size);
            for pixel in water.pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[32, 75, 90, 180]);
            }
            water.save_png(&dir.join("blocks/water.png")).unwrap();
            let mut m = AtlasManager::new(Some(&dir)).unwrap();
            assert!(!m.animate(1.0));
            let atlas = m.get(AtlasKind::Blocks);
            let r = atlas.rect(procedural::tiles::WATER as usize);
            assert_eq!(atlas.image().crop(r.x, r.y, r.w, r.h), water);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn names_resolve() {
        let m = AtlasManager::new(None).unwrap();
        let gui = m.get(AtlasKind::Gui);
        assert_eq!(gui.id("logo"), Some(procedural::gui::LOGO as usize));
    }
}

