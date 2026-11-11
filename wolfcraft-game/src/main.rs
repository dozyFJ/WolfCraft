//! WolfCraft game executable (`WolfCraft.exe`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    wolfcraft_game::run_desktop();
}