//! WolfCraft game library: desktop `main` and the Android entry point share this code.

mod app;
mod audio;
mod commands;
mod loader;
mod preview;
mod session;
mod touch;

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
    let data_dir = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp/WolfCraft"));
    let args = LaunchArgs { data_dir: Some(data_dir), game_version: None, from_launcher: false };
    match EventLoopBuilder::default().with_android_app(app).build() {
        Ok(e) => run(args, e),
        Err(e) => log::error!("event loop: {e}"),
    }
}