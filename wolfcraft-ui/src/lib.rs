//! WolfCraft UI: Modern Solid + Material + Industrial interface built on egui + wgpu.

pub mod chat_ui;
pub mod container_ui;
pub mod gui_renderer;
pub mod hud;
pub mod inventory_ui;
pub mod launcher;
pub mod studio_ui;
pub mod stellar_ui;
pub mod texture_editor;
pub mod main_menu;
pub mod osk;
pub mod theme_manager;
pub mod ui_components;

pub use gui_renderer::{GuiIcons, GuiRenderer};
