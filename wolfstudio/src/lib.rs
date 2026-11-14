//! WolfCraft for Android, the all-in-one app: the launcher (Kotlin) starts this library for
//! the game itself («Главная» → Играть) or for the editor («Редактор»). A file
//! `open_game.txt` in the app's folder asks for the game; otherwise it is the studio.

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    let flag = app.internal_data_path().map(|d| d.join("open_game.txt")).filter(|f| f.exists());
    if let Some(f) = &flag {
        let _ = std::fs::remove_file(f);
    }
    wolfcraft_game::android_entry(app, flag.is_none());
    // Its own process: end it, so the next start builds a fresh event loop.
    std::process::exit(0);
}
