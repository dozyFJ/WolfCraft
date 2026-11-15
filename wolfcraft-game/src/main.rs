//! WolfCraft for PC (Windows .exe, Linux): the same game as on the phones — one code base, so
//! everything of the mobile version is here too; touch controls give way to mouse and keyboard.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    wolfcraft_game::run_desktop();
}
