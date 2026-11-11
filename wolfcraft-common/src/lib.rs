//! Shared code for the WolfCraft launcher and game:
//! timing/game loop helpers (`time`), Windows integration (`platform`),
//! GPU/window bootstrap (`gpu`), launch protocol (`launch`) and logging (`logger`).

pub mod gpu;
pub mod launch;
pub mod logger;
pub mod platform;
pub mod time;

/// Version of this build (shared by launcher and game).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub mod version;
