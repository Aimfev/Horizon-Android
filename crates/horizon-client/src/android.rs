//! Android native library hot-swap entry handoff, crash-recovery notice, and activity helpers.
#![cfg(target_os = "android")]

use jni::objects::JString;
use std::{
    ffi::{CString, c_char, c_int, c_void},
    os::unix::ffi::OsStringExt,
    sync::atomic::{AtomicBool, Ordering},
};

const RTLD_NOW: c_int = 2;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn ANativeActivity_onCreate(
        activity: *mut c_void,
        saved_state: *mut c_void,
        saved_state_size: usize,
    );
}

static FIRST_FRAME_NOTIFIED: AtomicBool = AtomicBool::new(false);

/// `AndroidManifest.xml`'s `android.app.func_name`. When `HorizonActivity.loadGame`
/// chose a native-mod build before `super.onCreate`, `HORIZON_GAME_LIBRARY` names it and
/// we hand `ANativeActivity_onCreate` to its entry so `NativeActivity` and JNI share one
/// library; otherwise `android-activity`'s `ANativeActivity_onCreate` in this library runs.
pub unsafe fn on_create(activity: *mut c_void, saved: *mut c_void, saved_size: usize) {
    if let Some(raw_path) = std::env::var_os("HORIZON_GAME_LIBRARY")
        && let path_ref = std::path::Path::new(&raw_path)
        && std::fs::symlink_metadata(path_ref).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
        && path_ref.to_str().is_some_and(|s| s.contains("/files/native/") && s.ends_with("/libhorizon_android.so") && !s.contains(".."))
        && let Ok(path) = CString::new(raw_path.into_vec())
    {
        let build = unsafe { dlopen(path.as_ptr(), RTLD_NOW) };
        if !build.is_null() {
            let entry = unsafe { dlsym(build, c"ANativeActivity_onCreate".as_ptr()) };
            if !entry.is_null() && entry as usize != ANativeActivity_onCreate as *const () as usize
            {
                let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) =
                    unsafe { std::mem::transmute(entry) };
                unsafe { f(activity, saved, saved_size) };
                return;
            }
        }
    }
    unsafe { ANativeActivity_onCreate(activity, saved, saved_size) };
}

/// Clears the 2-strike native-mod crash counter (`files/native/starts`) and requests any
/// runtime permissions declared in the active build's `build.json`.
pub fn first_frame_shown() {
    if FIRST_FRAME_NOTIFIED.swap(true, Ordering::Relaxed) {
        return;
    }
    let _ = crate::platform::with_context(|env, activity| {
        env.call_method(activity, "firstFrame", "()V", &[])?;
        Ok(())
    });
}

/// The APK's `versionName` and whether it is signed with the official release certificate.
pub fn app_version() -> Option<(String, bool)> {
    crate::platform::with_context(|env, activity| {
        let name = env
            .call_method(activity, "versionName", "()Ljava/lang/String;", &[])?
            .l()?;
        let name = String::from(env.get_string(&JString::from(name))?);
        let release = env
            .call_method(activity, "releaseBuild", "()Z", &[])?
            .z()?;
        Ok((name, release))
    })
    .filter(|(name, _)| !name.is_empty())
}

/// Relaunches `HorizonActivity` in a fresh process so a newly chosen or compiled
/// `libhorizon_android.so` takes effect on `onCreate`.
pub fn restart_app() {
    let _ = crate::platform::with_context(|env, activity| {
        env.call_method(activity, "restart", "()V", &[])?;
        Ok(())
    });
}
