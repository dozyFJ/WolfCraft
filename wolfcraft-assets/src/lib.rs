//! WolfCraft assets: original procedural textures, a rectangle texture packer and an
//! atlas manager that keeps one GPU texture per category (blocks, items, entities, GUI,
//! particles).
//!
//! Every texture is painted in code; nothing is copied from other games. PNG files dropped
//! into `<data>/resourcepacks/<category>/<sprite>.png` override the generated sprites (any
//! resolution), and the generated atlases are exported as `blocks_atlas.png`,
//! `items_atlas.png`, `entities_atlas.png`, `gui_atlas.png` and `particles_atlas.png`.

pub mod atlas_manager;
pub mod image;
pub mod procedural;
pub mod texture_packer;

pub use atlas_manager::{AtlasKind, AtlasManager, TextureAtlas, UvRect};
pub use image::Image;
pub use procedural::{entities, gui, items, particles, tiles};
