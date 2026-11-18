#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    wolfcut::run_desktop()
}

#[cfg(target_os = "android")]
fn main() {}
