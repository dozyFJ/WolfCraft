//! Open the private repository's releases with the user's browser session.
//! Native Android code is updated through a signed APK; resource packs use the existing ZIP downloader.
const RELEASE_URL: &str = "https://github.com/dozyFJ/WolfCraft/releases/latest";

pub fn open_release() {
    #[cfg(not(target_os = "android"))]
    { let _ = std::process::Command::new("xdg-open").arg(RELEASE_URL).spawn(); }
    #[cfg(target_os = "android")]
    if let Err(e) = open_android() { log::warn!("Could not open updates: {e}"); }
}

#[cfg(target_os = "android")]
fn open_android() -> Result<(), Box<dyn std::error::Error>> {
    use jni::objects::{JObject, JValue};
    let app = crate::orientation::app().ok_or("Android activity unavailable")?;
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM)? };
    let mut env = vm.attach_current_thread()?;
    env.with_local_frame(16, |env| -> Result<(), jni::errors::Error> {
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
        let result = (|| {
            let url = JObject::from(env.new_string(RELEASE_URL)?);
            let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&url)])?.l()?;
            let action = JObject::from(env.new_string("android.intent.action.VIEW")?);
            let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V", &[JValue::Object(&action), JValue::Object(&uri)])?;
            env.call_method(&activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)])?;
            Ok(())
        })();
        std::mem::forget(activity);
        result
    })?;
    Ok(())
}
