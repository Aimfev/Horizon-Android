//! The `libhorizon_android.so` `cdylib` entry points: the thin glue between Android's
//! `NativeActivity` / `HorizonActivity` and `horizon-client`.
#![cfg(target_os = "android")]

include!("android_main.rs");

/// `AndroidManifest.xml`'s `android.app.func_name`. When `HorizonActivity.loadGame`
/// chose a native-mod build before `super.onCreate`, `HORIZON_GAME_LIBRARY` names it and
/// we hand `ANativeActivity_onCreate` to its entry so `NativeActivity` and JNI share one
/// library; otherwise `android-activity`'s `ANativeActivity_onCreate` in this library runs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn horizon_on_create(
    activity: *mut std::ffi::c_void,
    saved: *mut std::ffi::c_void,
    saved_size: usize,
) {
    unsafe { crate::android::on_create(activity, saved, saved_size) }
}
