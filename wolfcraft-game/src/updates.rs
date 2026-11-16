//! Android OTA is owned by the signed APK shell; desktop retains the release-page link.
/// Where the PC build is published (the Windows exe lives in the «pc-test» release).
const RELEASE_URL: &str = "https://github.com/dozyFJ/WolfCraft/releases/tag/pc-test";

pub fn open_release() {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", RELEASE_URL]).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(RELEASE_URL).spawn();
    }
    #[cfg(all(not(target_os = "android"), not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(RELEASE_URL).spawn();
    }
    #[cfg(target_os = "android")]
    if let Err(e) = open_android() { log::warn!("Could not open updates: {e}"); }
}

/// The stable APK shell owns downloads and restarts; Rust never replaces its own loaded code.
#[cfg(target_os = "android")]
fn open_android() -> Result<(), Box<dyn std::error::Error>> {
    shell_call("checkForUpdates")
}

pub fn mark_ready() {
    #[cfg(target_os="android")]
    if let Err(e)=shell_call("markUpdateHealthy"){log::warn!("OTA boot marker: {e}");}
}

#[cfg(target_os="android")]
fn shell_call(method:&str)->Result<(),Box<dyn std::error::Error>>{
    let app=crate::orientation::app().ok_or("Android activity unavailable")?;
    let vm=unsafe{jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM)?};
    let mut env=vm.attach_current_thread()?;
    let activity=unsafe{jni::objects::JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject)};
    let result=env.call_method(&activity,method,"()V",&[]);
    std::mem::forget(activity);
    if result.is_err(){let _=env.exception_clear();}
    result?;Ok(())
}
