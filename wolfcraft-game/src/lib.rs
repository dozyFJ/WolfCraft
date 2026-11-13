//! WolfCraft game library: desktop `main` and the Android entry point share this code.

mod app;
mod updates;
mod audio;
mod commands;
mod loader;
mod lua;
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
        platform::message_box("WolfCraft — ошибка", &format!("{e}"));
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
            platform::message_box("WolfCraft — ошибка запуска", &format!("Could not create the event loop: {e}"));
        }
    }
}

/// The game APK's entry point (off in the WolfStudio APK, which has its own).
#[cfg(all(target_os = "android", feature = "game-entry"))]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_entry(app, false);
}

/// Starts the game (`studio` = false) or the WolfStudio app on Android.
#[cfg(target_os = "android")]
pub fn android_entry(app: winit::platform::android::activity::AndroidApp, studio: bool) {
    use winit::event_loop::EventLoopBuilder;
    use winit::platform::android::EventLoopBuilderExtAndroid;
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag(if studio { "WolfStudio" } else { "WolfCraft" }));
    wolfcraft_ui::launcher::set_studio_app(studio);
    orientation::set_app(app.clone());
    let data_dir = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp/WolfCraft"));
    let args = LaunchArgs { data_dir: Some(data_dir), game_version: None, from_launcher: false };
    match EventLoopBuilder::default().with_android_app(app).build() {
        Ok(e) => run(args, e),
        Err(e) => log::error!("event loop: {e}"),
    }
    // The studio runs in its own process inside the WolfStudio app: end it, so the next
    // «Студия» starts fresh (an event loop can be built only once per process).
    if studio {
        std::process::exit(0);
    }
}
/// Text input on phones. The game runs in a NativeActivity, which has no input connection:
/// Gboard and other keyboards cannot type into it (non-Latin letters never arrive at all).
/// So a text field opens a small system dialog with a real Android text box; its text goes
/// into the game's field when the dialog closes.
pub mod text_input {
    #[cfg(target_os = "android")]
    struct Open {
        dialog: jni::objects::GlobalRef,
        edit: jni::objects::GlobalRef,
        looper: jni::objects::GlobalRef,
    }

    #[cfg(target_os = "android")]
    static STATE: std::sync::Mutex<(bool, Option<Open>)> = std::sync::Mutex::new((false, None));

    /// A dialog is being shown (or about to be).
    pub fn is_open() -> bool {
        #[cfg(target_os = "android")]
        {
            STATE.lock().map(|s| s.0).unwrap_or(false)
        }
        #[cfg(not(target_os = "android"))]
        false
    }

    /// Opens the system text box (no-op on desktop).
    pub fn open(title: &str) {
        #[cfg(target_os = "android")]
        {
            {
                let Ok(mut s) = STATE.lock() else { return };
                if s.0 {
                    return;
                }
                s.0 = true;
            }
            let title = title.to_string();
            std::thread::spawn(move || {
                if let Err(e) = run_dialog(&title) {
                    log::warn!("text input dialog: {e}");
                    if let Ok(mut s) = STATE.lock() {
                        *s = (false, None);
                    }
                }
            });
        }
        #[cfg(not(target_os = "android"))]
        let _ = title;
    }

    /// The dialog's own thread: a Looper, the dialog, then the loop until the game quits it.
    #[cfg(target_os = "android")]
    fn run_dialog(title: &str) -> Result<(), String> {
        use jni::objects::{JObject, JValue};
        let app = super::orientation::app().ok_or("no app")?;
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }.map_err(|e| e.to_string())?;
        let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
        let r = (|| -> jni::errors::Result<()> {
            env.call_static_method("android/os/Looper", "prepare", "()V", &[])?;
            let looper = env.call_static_method("android/os/Looper", "myLooper", "()Landroid/os/Looper;", &[])?.l()?;
            let edit = env.new_object("android/widget/EditText", "(Landroid/content/Context;)V", &[JValue::Object(&activity)])?;
            env.call_method(&edit, "setSingleLine", "()V", &[])?;
            let builder = env.new_object("android/app/AlertDialog$Builder", "(Landroid/content/Context;)V", &[JValue::Object(&activity)])?;
            let t = env.new_string(title)?;
            env.call_method(&builder, "setTitle", "(Ljava/lang/CharSequence;)Landroid/app/AlertDialog$Builder;", &[JValue::Object(&t)])?;
            env.call_method(&builder, "setView", "(Landroid/view/View;)Landroid/app/AlertDialog$Builder;", &[JValue::Object(&edit)])?;
            let ok = env.new_string("Готово")?;
            let none = JObject::null();
            env.call_method(&builder, "setPositiveButton", "(Ljava/lang/CharSequence;Landroid/content/DialogInterface$OnClickListener;)Landroid/app/AlertDialog$Builder;", &[JValue::Object(&ok), JValue::Object(&none)])?;
            let dialog = env.call_method(&builder, "create", "()Landroid/app/AlertDialog;", &[])?.l()?;
            let window = env.call_method(&dialog, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
            if !window.is_null() {
                // SOFT_INPUT_STATE_ALWAYS_VISIBLE: the keyboard comes up with the dialog.
                env.call_method(&window, "setSoftInputMode", "(I)V", &[JValue::Int(5)])?;
            }
            env.call_method(&dialog, "show", "()V", &[])?;
            env.call_method(&edit, "requestFocus", "()Z", &[])?;
            let open = Open { dialog: env.new_global_ref(&dialog)?, edit: env.new_global_ref(&edit)?, looper: env.new_global_ref(&looper)? };
            if let Ok(mut s) = STATE.lock() {
                s.1 = Some(open);
            }
            env.call_static_method("android/os/Looper", "loop", "()V", &[])?;
            Ok(())
        })();
        std::mem::forget(activity);
        if r.is_err() {
            let _ = env.exception_clear();
        }
        r.map_err(|e| e.to_string())
    }

    /// Text of a dialog that has just closed (once).
    pub fn poll() -> Option<String> {
        #[cfg(target_os = "android")]
        {
            let mut s = STATE.lock().ok()?;
            let open = s.1.as_ref()?;
            let app = super::orientation::app()?;
            let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }.ok()?;
            let mut env = vm.attach_current_thread().ok()?;
            let showing = env.call_method(open.dialog.as_obj(), "isShowing", "()Z", &[]).and_then(|v| v.z());
            match showing {
                Ok(true) => return None,
                Ok(false) => {}
                Err(_) => {
                    let _ = env.exception_clear();
                }
            }
            let text = (|| -> jni::errors::Result<String> {
                let editable = env.call_method(open.edit.as_obj(), "getText", "()Landroid/text/Editable;", &[])?.l()?;
                let js = env.call_method(&editable, "toString", "()Ljava/lang/String;", &[])?.l()?;
                Ok(env.get_string(&jni::objects::JString::from(js))?.into())
            })()
            .unwrap_or_default();
            let _ = env.call_method(open.looper.as_obj(), "quitSafely", "()V", &[]);
            let _ = env.exception_clear();
            *s = (false, None);
            Some(text)
        }
        #[cfg(not(target_os = "android"))]
        None
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

    #[cfg(target_os = "android")]
    pub(crate) fn app() -> Option<&'static winit::platform::android::activity::AndroidApp> {
        APP.get()
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

