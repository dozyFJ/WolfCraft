#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(not(target_os = "android"))]
fn main() {
    wolfcraft_game::run_studio_desktop();
}

// Android enters through NativeActivity in lib.rs.
#[cfg(target_os = "android")]
fn main() {}
