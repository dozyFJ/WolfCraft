use jni::{
    JavaVM,
    objects::{GlobalRef, JObject, JString},
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock, RwLock},
};
use winit::platform::android::activity::AndroidApp;

struct Bridge {
    vm: JavaVM,
    activity: GlobalRef,
    session: PathBuf,
}

static BINARIES: OnceLock<PathBuf> = OnceLock::new();
static BRIDGE: RwLock<Option<Arc<Bridge>>> = RwLock::new(None);

pub enum ResultEvent {
    Import(Vec<PathBuf>, String),
    Export(String),
    Update(String, bool),
}

pub fn initialize(app: &AndroidApp) -> Result<(), Box<dyn std::error::Error>> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
    let (activity, binaries, session) = {
        let mut env = vm.attach_current_thread()?;
        let activity =
            env.new_global_ref(unsafe { JObject::from_raw(app.activity_as_ptr().cast()) })?;
        let binaries = env
            .call_method(&activity, "binaryDirectory", "()Ljava/lang/String;", &[])?
            .l()?;
        let binaries = PathBuf::from(String::from(env.get_string(&JString::from(binaries))?));
        let session = env
            .call_method(&activity, "sessionDirectory", "()Ljava/lang/String;", &[])?
            .l()?;
        let session = PathBuf::from(String::from(env.get_string(&JString::from(session))?));
        (activity, binaries, session)
    };
    BINARIES.get_or_init(|| binaries);
    *BRIDGE.write().expect("Android bridge lock") = Some(Arc::new(Bridge {
        vm,
        activity,
        session,
    }));
    Ok(())
}

pub fn clear() {
    *BRIDGE.write().expect("Android bridge lock") = None;
}

fn bridge() -> Arc<Bridge> {
    BRIDGE
        .read()
        .expect("Android bridge lock")
        .as_ref()
        .expect("Android bridge")
        .clone()
}

pub fn session() -> PathBuf {
    bridge().session.clone()
}

pub fn executable(name: &str) -> PathBuf {
    BINARIES
        .get()
        .expect("FFmpeg directory")
        .join(format!("lib{name}.so"))
}

fn launch(method: &str) -> Result<(), String> {
    let bridge = bridge();
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    env.call_method(&bridge.activity, method, "()V", &[])
        .map(|_| ())
        .map_err(|error| {
            let _ = env.exception_clear();
            error.to_string()
        })
}

pub fn pick_import(audio: bool) -> Result<(), String> {
    launch(if audio { "pickAudio" } else { "pickVideos" })
}

pub fn pick_export() -> Result<(), String> {
    launch("saveVideo")
}

pub fn update(install: bool) -> Result<(), String> {
    launch(if install {
        "installUpdate"
    } else {
        "checkUpdate"
    })
}

pub fn poll() -> Result<Option<ResultEvent>, String> {
    let bridge = bridge();
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let message = env
        .with_local_frame(16, |env| -> jni::errors::Result<Option<String>> {
            let value = env
                .call_method(&bridge.activity, "pollResult", "()Ljava/lang/String;", &[])?
                .l()?;
            if value.is_null() {
                return Ok(None);
            }
            Ok(Some(String::from(env.get_string(&JString::from(value))?)))
        })
        .map_err(|error| {
            let _ = env.exception_clear();
            error.to_string()
        })?;
    let Some(message) = message else {
        return Ok(None);
    };
    let value: Value = serde_json::from_str(&message).map_err(|error| error.to_string())?;
    let status = value["status"].as_str().unwrap_or_default().to_owned();
    let event = if value["type"] == "import" {
        let paths = value["paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(PathBuf::from)
            .collect();
        ResultEvent::Import(paths, status)
    } else if value["type"] == "update" {
        ResultEvent::Update(status, value["available"].as_bool().unwrap_or(false))
    } else {
        ResultEvent::Export(status)
    };
    Ok(Some(event))
}
