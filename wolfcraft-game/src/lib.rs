//! WolfCraft game library: desktop `main` and the Android entry point share this code.

mod app;
mod audio;
mod commands;
mod loader;
mod preview;
mod session;
mod signs;
mod touch;
mod worldedit;

use winit::event_loop::EventLoop;
use wolfcraft_common::launch::LaunchArgs;
use wolfcraft_common::{logger, platform};

/// Runs the game with an already-built event loop.
pub fn run(args: LaunchArgs, event_loop: EventLoop<()>) {
    let data_dir = args.resolved_data_dir();
    logger::init(&data_dir.join("logs").join("latest.log"), log::LevelFilter::Info);
    log::info!("WolfCraft {} starting (data dir {}, launcher: {}, version arg: {:?})", wolfcraft_common::VERSION, data_dir.display(), args.from_launcher, args.game_version);

    // Leave one core for the render/main thread; the rest generate and mesh chunks.
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let _ = rayon::ThreadPoolBuilder::new().num_threads((cores.saturating_sub(1)).max(2)).thread_name(|i| format!("worker-{i}")).build_global();

    let mut app = app::App::new(args);
    if let Err(e) = event_loop.run_app(&mut app) {
        log::error!("event loop error: {e}");
        platform::message_box("WolfCraft - error", &format!("{e}"));
    }
    log::info!("WolfCraft exited normally");
}

/// Desktop entry point.
pub fn run_desktop() {
    let args = LaunchArgs::parse(std::env::args().skip(1));
    match EventLoop::new() {
        Ok(e) => run(args, e),
        Err(e) => {
            log::error!("event loop: {e}");
            platform::message_box("WolfCraft - startup error", &format!("Could not create the event loop: {e}"));
        }
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::event_loop::EventLoopBuilder;
    use winit::platform::android::EventLoopBuilderExtAndroid;
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("WolfCraft"));
    orientation::set_app(app.clone());
    let data_dir = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp/WolfCraft"));
    let args = LaunchArgs { data_dir: Some(data_dir), game_version: None, from_launcher: false };
    match EventLoopBuilder::default().with_android_app(app).build() {
        Ok(e) => run(args, e),
        Err(e) => log::error!("event loop: {e}"),
    }
}
/// Screen orientation on phones: the launcher stands upright, the game lies in landscape.
pub mod orientation {
    #[cfg(target_os = "android")]
    static APP: std::sync::OnceLock<winit::platform::android::activity::AndroidApp> = std::sync::OnceLock::new();

    #[cfg(target_os = "android")]
    pub fn set_app(app: winit::platform::android::activity::AndroidApp) {
        let _ = APP.set(app);
    }

    /// Lets the Wi-Fi chip pass broadcast packets to the app (many phones drop them to save
    /// power), so servers on the local network show up in "Рядом". Done once.
    pub fn acquire_multicast_lock() {
        #[cfg(target_os = "android")]
        {
            static DONE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            if DONE.set(()).is_err() {
                return;
            }
            let Some(app) = APP.get() else { return };
            let Ok(vm) = (unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }) else { return };
            let Ok(mut env) = vm.attach_current_thread() else { return };
            let activity = unsafe { jni::objects::JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
            let r = (|| -> jni::errors::Result<()> {
                let svc = env.new_string("wifi")?;
                let wm = env.call_method(&activity, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[jni::objects::JValue::Object(&svc)])?.l()?;
                if wm.is_null() {
                    return Ok(());
                }
                let tag = env.new_string("wolfcraft")?;
                let lock = env.call_method(&wm, "createMulticastLock", "(Ljava/lang/String;)Landroid/net/wifi/WifiManager$MulticastLock;", &[jni::objects::JValue::Object(&tag)])?.l()?;
                env.call_method(&lock, "acquire", "()V", &[])?;
                // Keep the lock alive for the whole run.
                std::mem::forget(env.new_global_ref(lock)?);
                Ok(())
            })();
            if let Err(e) = r {
                let _ = env.exception_clear();
                log::warn!("multicast lock: {e}");
            } else {
                log::info!("multicast lock acquired");
            }
            std::mem::forget(activity);
        }
    }

    /// Asks the activity for portrait (`true`) or landscape, following the sensor within it.
    pub fn set_portrait(portrait: bool) {
        #[cfg(target_os = "android")]
        {
            let Some(app) = APP.get() else { return };
            // ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT = 7, SENSOR_LANDSCAPE = 6.
            let value = if portrait { 7 } else { 6 };
            let Ok(vm) = (unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }) else { return };
            let Ok(mut env) = vm.attach_current_thread() else { return };
            let activity = unsafe { jni::objects::JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
            if env.call_method(&activity, "setRequestedOrientation", "(I)V", &[jni::objects::JValue::Int(value)]).is_err() {
                let _ = env.exception_clear();
                log::warn!("could not change the screen orientation");
            }
            // The activity reference belongs to the app: do not delete it.
            std::mem::forget(activity);
        }
        #[cfg(not(target_os = "android"))]
        let _ = portrait;
    }
}
