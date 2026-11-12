//! Export the generated artwork as editable PNG overrides.
use std::path::PathBuf;
use wolfcraft_assets::{AtlasKind, procedural};
fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "texture-pack".into()));
    for kind in AtlasKind::ALL {
        for (name, image) in procedural::sprites(kind) {
            image.save_png(&dir.join(kind.folder()).join(format!("{name}.png"))).unwrap();
        }
    }
    println!("Exported textures to {}", dir.display());
}
