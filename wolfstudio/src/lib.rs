//! WolfStudio for Android: the WolfCraft game library started in studio mode (projects and
//! settings in its launcher; the game APK has no studio tab).

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    wolfcraft_game::android_entry(app, true);
}
