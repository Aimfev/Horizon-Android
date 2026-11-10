//! SAF selection, bounded background imports and explicit collision choices.
use horizon_client::{
    Result,
    content_files::{self, Kind},
    hud::{HudAtlas, HudBatch, rgba},
    ui_theme::{Face, Ui, color},
};
use jni::{
    JNIEnv,
    objects::{JObject, JString},
};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

struct Request {
    kind: Kind,
    appearance: Option<crate::skins::library::Kind>,
    target_dir: Option<PathBuf>,
    control_import: bool,
    export_bytes: Option<(Vec<u8>, String)>,
    export_source_path: Option<(PathBuf, String)>,
    home: PathBuf,
    cache: PathBuf,
}
struct Conflict {
    kind: Kind,
    staged: PathBuf,
    target: PathBuf,
}
#[derive(Clone)]
pub struct Notice {
    pub text: String,
    pub changed: bool,
    pub kind: Option<Kind>,
}
#[derive(Default)]
struct State {
    request: Option<Request>,
    working: bool,
    conflict: Option<Conflict>,
    notice: Option<Notice>,
    hits: Vec<([f32; 4], Choice)>,
    toast: Option<(String, std::time::Instant)>,
    cursor: (f32, f32),
    status_title: String,
    status_detail: String,
    progress: Option<f32>,
}
#[derive(Clone, Copy)]
pub enum Choice {
    KeepBoth,
    Replace,
    Cancel,
}
static STATE: Mutex<State> = Mutex::new(State {
    request: None,
    working: false,
    conflict: None,
    notice: None,
    hits: Vec::new(),
    toast: None,
    cursor: (0.0, 0.0),
    status_title: String::new(),
    status_detail: String::new(),
    progress: None,
});
static CANCEL: AtomicBool = AtomicBool::new(false);

pub fn busy() -> bool {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.request.is_some() || s.working || s.conflict.is_some()
}

pub fn conflict() -> bool {
    STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .conflict
        .is_some()
}

fn set_progress(title: impl Into<String>, detail: impl Into<String>, progress: Option<f32>) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.status_title = title.into();
    s.status_detail = detail.into();
    s.progress = progress;
}

fn notice(text: impl Into<String>, changed: bool) {
    let text = text.into();
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.toast = Some((text.clone(), std::time::Instant::now()));
    s.notice = Some(Notice {
        text,
        changed,
        kind: None,
    });
}

fn imported(text: impl Into<String>, kind: Kind) {
    let text = text.into();
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.toast = Some((text.clone(), std::time::Instant::now()));
    s.notice = Some(Notice {
        text,
        changed: true,
        kind: Some(kind),
    });
}

pub fn poll() -> Option<Notice> {
    if crate::document_provider::take_changes() {
        return Some(Notice {
            text: "Content files changed. Reload packs to apply.".into(),
            changed: true,
            kind: None,
        });
    }
    STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .notice
        .take()
}

pub fn begin(kind: Kind, home: PathBuf) -> std::result::Result<(), String> {
    if busy() {
        return Err("Finish or cancel the current import first".into());
    }
    CANCEL.store(false, Ordering::Relaxed);
    crate::platform::hide();
    let cache = crate::platform::cache_dir()
        .ok_or("Could not access the Android import cache")?
        .join("content-imports");
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.status_title = format!("Select {} file...", kind.label());
        s.status_detail = "Waiting for Android file picker...".into();
        s.progress = Some(0.08);
        s.request = Some(Request {
            kind,
            appearance: None,
            target_dir: None,
            control_import: false,
            export_bytes: None,
            export_source_path: None,
            home,
            cache,
        });
    }
    if !crate::native_ime::pick_document() {
        picker_failed("Android document picker is unavailable");
        return Err("Android document picker is unavailable".into());
    }
    notice(format!("Choose a {} file", kind.label()), false);
    Ok(())
}

pub fn picker_failed(error: &str) {
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.request = None;
        s.working = false;
        s.progress = None;
    }
    notice(error, false);
}

pub fn begin_appearance(
    home: PathBuf,
    kind: crate::skins::library::Kind,
) -> std::result::Result<(), String> {
    if busy() {
        return Err("Finish or cancel the current import first".into());
    }
    CANCEL.store(false, Ordering::Relaxed);
    crate::platform::hide();
    let cache = crate::platform::cache_dir()
        .ok_or("Could not access Android's import cache")?
        .join("appearance-imports");
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.status_title = "Select skin or cape PNG...".into();
        s.status_detail = "Waiting for Android file picker...".into();
        s.progress = Some(0.08);
        s.request = Some(Request {
            kind: Kind::Packs,
            appearance: Some(kind),
            target_dir: None,
            control_import: false,
            export_bytes: None,
            export_source_path: None,
            home,
            cache,
        });
    }
    if !crate::native_ime::pick_document() {
        picker_failed("Android document picker is unavailable");
        return Err("Android document picker is unavailable".into());
    }
    notice("Choose a skin or cape PNG", false);
    Ok(())
}

/// Opens Android's `ACTION_OPEN_DOCUMENT` picker to import a file directly into `target_dir`.
pub fn begin_into_dir(target_dir: PathBuf) -> std::result::Result<(), String> {
    if busy() {
        return Err("Finish or cancel the current import first".into());
    }
    CANCEL.store(false, Ordering::Relaxed);
    crate::platform::hide();
    let cache = crate::platform::cache_dir()
        .ok_or("Could not access the Android import cache")?
        .join("file-manager-imports");
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.status_title = "Importing file...".into();
        s.status_detail = "Choose a file in Android file picker (or tap Cancel)...".into();
        s.progress = Some(0.08);
        s.request = Some(Request {
            kind: Kind::Packs,
            appearance: None,
            target_dir: Some(target_dir),
            control_import: false,
            export_bytes: None,
            export_source_path: None,
            home: PathBuf::new(),
            cache,
        });
    }
    if !crate::native_ime::pick_document() {
        picker_failed("Android document picker is unavailable");
        return Err("Android document picker is unavailable".into());
    }
    notice("Choose a file to import into this folder", false);
    Ok(())
}

/// Opens Android's `ACTION_OPEN_DOCUMENT_TREE` picker to import an entire folder with its name directly into `target_dir`.
pub fn begin_folder_into_dir(target_dir: PathBuf) -> std::result::Result<(), String> {
    if busy() {
        return Err("Finish or cancel the current import first".into());
    }
    CANCEL.store(false, Ordering::Relaxed);
    crate::platform::hide();
    let cache = crate::platform::cache_dir()
        .ok_or("Could not access the Android import cache")?
        .join("file-manager-imports");
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.status_title = "Importing folder...".into();
        s.status_detail = "Select world/folder in Android picker (or tap Cancel)...".into();
        s.progress = Some(0.08);
        s.request = Some(Request {
            kind: Kind::Packs,
            appearance: None,
            target_dir: Some(target_dir),
            control_import: false,
            export_bytes: None,
            export_source_path: None,
            home: PathBuf::new(),
            cache,
        });
    }
    if !crate::native_ime::pick_document_tree() {
        picker_failed("Android folder picker is unavailable");
        return Err("Android folder picker is unavailable".into());
    }
    notice("Choose a folder to import into this directory", false);
    Ok(())
}

/// Opens Android's `ACTION_OPEN_DOCUMENT` picker to import a control preset JSON file,
/// automatically detect whether it is an `MCPE` or `GoyControl` layout, and save it in the proper directory.
pub fn begin_control_import(game_home: PathBuf) -> std::result::Result<(), String> {
    if busy() {
        return Err("Finish or cancel the current import first".into());
    }
    CANCEL.store(false, Ordering::Relaxed);
    crate::platform::hide();
    let cache = crate::platform::cache_dir()
        .unwrap_or_else(|| game_home.clone())
        .join("control-imports");
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.status_title = "Importing control preset...".into();
        s.status_detail = "Select a control preset (.json) file...".into();
        s.progress = Some(0.08);
        s.request = Some(Request {
            kind: Kind::Packs,
            appearance: None,
            target_dir: None,
            control_import: true,
            export_bytes: None,
            export_source_path: None,
            home: game_home,
            cache,
        });
    }
    if !crate::native_ime::pick_document() {
        picker_failed("Android document picker is unavailable");
        return Err("Android document picker is unavailable".into());
    }
    notice(
        "Choose a control preset (.json) — auto-detects MCPE or GoyControl",
        false,
    );
    Ok(())
}

/// Opens Android's `ACTION_CREATE_DOCUMENT` system file manager so the user can pick the save location
/// for their exported control preset JSON.
pub fn begin_control_export(
    game_home: PathBuf,
    suggested_name: String,
    bytes: Vec<u8>,
) -> std::result::Result<(), String> {
    if busy() {
        return Err("Finish or cancel the current file operation first".into());
    }
    CANCEL.store(false, Ordering::Relaxed);
    crate::platform::hide();
    let cache = crate::platform::cache_dir()
        .unwrap_or_else(|| game_home.clone())
        .join("control-exports");
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.status_title = format!("Exporting '{suggested_name}'...");
        s.status_detail = "Choose destination in Android file manager...".into();
        s.progress = Some(0.1);
        s.request = Some(Request {
            kind: Kind::Packs,
            appearance: None,
            target_dir: None,
            control_import: false,
            export_bytes: Some((bytes, suggested_name.clone())),
            export_source_path: None,
            home: game_home,
            cache,
        });
    }
    if !crate::native_ime::create_document(&suggested_name, "application/json") {
        picker_failed("Android file save picker is unavailable");
        return Err("Android file save picker is unavailable".into());
    }
    notice(
        format!("Choose where to save '{suggested_name}' in file manager"),
        false,
    );
    Ok(())
}

/// Opens Android's `ACTION_CREATE_DOCUMENT` system file manager so the user can choose where to
/// save/paste any exported file or folder `.zip` archive.
pub fn begin_file_export(
    home: PathBuf,
    source_path: PathBuf,
    suggested_name: String,
    mime_type: String,
) -> std::result::Result<(), String> {
    if busy() {
        return Err("Finish or cancel the current file operation first".into());
    }
    CANCEL.store(false, Ordering::Relaxed);
    crate::platform::hide();
    let cache = crate::platform::cache_dir()
        .unwrap_or_else(|| home.clone())
        .join("file-exports");
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.status_title = format!("Exporting '{suggested_name}'...");
        s.status_detail = "Select save destination in Android file manager...".into();
        s.progress = Some(0.1);
        s.request = Some(Request {
            kind: Kind::Packs,
            appearance: None,
            target_dir: None,
            control_import: false,
            export_bytes: None,
            export_source_path: Some((source_path, suggested_name.clone())),
            home,
            cache,
        });
    }
    if !crate::native_ime::create_document(&suggested_name, &mime_type) {
        picker_failed("Android file save picker is unavailable");
        return Err("Android file save picker is unavailable".into());
    }
    notice(
        format!("Select where to save '{suggested_name}' in file manager"),
        false,
    );
    Ok(())
}

/// Exports a local file by opening Android's system file save picker (`ACTION_CREATE_DOCUMENT`)
/// so the user chooses where to save it (never dumping into `logs` or `reports`).
pub fn export_file(
    home: &std::path::Path,
    _profile: u32,
    file_path: &std::path::Path,
) -> std::result::Result<String, String> {
    let meta = std::fs::symlink_metadata(file_path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err("Only regular files can be exported.".into());
    }
    let file_name = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid filename")?;
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        let _ = crate::gallery::save(file_path);
    }
    let mime = if lower.ends_with(".zip")
        || lower.ends_with(".hmod")
        || lower.ends_with(".hnmod")
        || lower.ends_with(".jar")
        || lower.ends_with(".hshader")
    {
        "application/zip"
    } else if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".txt") || lower.ends_with(".toml") || lower.ends_with(".log") {
        "text/plain"
    } else {
        "application/octet-stream"
    };
    match begin_file_export(
        home.to_path_buf(),
        file_path.to_path_buf(),
        file_name.to_owned(),
        mime.to_owned(),
    ) {
        Ok(()) => Ok(format!(
            "Select where to save '{file_name}' (also copied to File Manager clipboard for Paste)."
        )),
        Err(_) => Ok(format!(
            "Copied '{file_name}' to clipboard — open destination folder and tap Paste."
        )),
    }
}

/// Zips a folder into `.export-staging/<folder>.zip`, opens Android's system file picker to choose
/// where to save it, and returns `(zip_path, zip_name, status_message)` so callers can also place
/// `zip_path` on the in-app File Manager clipboard for `Paste`.
pub fn export_folder_prepared(
    home: &std::path::Path,
    profile: u32,
    dir_path: &std::path::Path,
) -> std::result::Result<(PathBuf, String, String), String> {
    let (zip_path, zip_name) = crate::document_store::export_folder_zip(home, profile, dir_path)?;
    let msg = match begin_file_export(
        home.to_path_buf(),
        zip_path.clone(),
        zip_name.clone(),
        "application/zip".into(),
    ) {
        Ok(()) => format!(
            "Created '{zip_name}'. Choose where to save it, or open any folder here and tap Paste."
        ),
        Err(_) => format!(
            "Created '{zip_name}' on clipboard — select any destination folder and tap Paste."
        ),
    };
    Ok((zip_path, zip_name, msg))
}

/// Zips a folder and opens Android file system picker to choose where to save/paste the zip.
pub fn export_folder(
    home: &std::path::Path,
    profile: u32,
    dir_path: &std::path::Path,
) -> std::result::Result<String, String> {
    let (_, _, msg) = export_folder_prepared(home, profile, dir_path)?;
    Ok(msg)
}

/// Called by the approved Activity result bridge, never from a game tick.
#[unsafe(export_name = "Java_dev_horizon_android_HorizonActivity_nativeDocumentResult")]
pub extern "system" fn document_result(
    mut env: JNIEnv<'_>,
    activity: JObject<'_>,
    request: i32,
    result: i32,
    uri: JString<'_>,
) {
    if request != 4101 && request != 4102 && request != 4103 {
        return;
    }
    let pending = STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .request
        .take();
    let Some(pending) = pending else {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.working = false;
        st.progress = None;
        return;
    };
    if result != -1 || uri.is_null() {
        {
            let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
            st.working = false;
            st.progress = None;
        }
        if request == 4103 {
            if let Some((_, name)) = pending.export_source_path {
                notice(
                    format!("'{name}' is on clipboard — open any folder in File Manager and tap Paste."),
                    false,
                );
            } else {
                notice("Export cancelled", false);
            }
        } else {
            notice("Import cancelled", false);
        }
        return;
    }
    let setup = (|| -> jni::errors::Result<_> {
        let text: String = env.get_string(&uri)?.into();
        let activity = env.new_global_ref(activity)?;
        Ok((env.get_java_vm()?, activity, text))
    })();
    let Ok((vm, activity, uri)) = setup else {
        let _ = env.exception_clear();
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.working = false;
        st.progress = None;
        drop(st);
        notice("Could not read the selected document", false);
        return;
    };
    if !uri.starts_with("content://") || uri.len() > 8192 {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.working = false;
        st.progress = None;
        drop(st);
        notice("Unsupported document URI", false);
        return;
    }

    if request == 4103 {
        {
            let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
            st.working = true;
            st.status_title = "Exporting file...".into();
            st.status_detail = "Writing archive to chosen location...".into();
            st.progress = Some(0.35);
        }
        let spawned = std::thread::Builder::new()
            .name("File export".into())
            .spawn(move || {
                let result = (|| -> Result<()> {
                    use std::io::Write;
                    let mut env = vm.attach_current_thread()?;
                    let opened =
                        crate::document_bridge::create_output(&mut env, activity.as_obj(), &uri);
                    if opened.is_err() {
                        let _ = env.exception_clear();
                    }
                    let mut output = opened.map_err(|e| e.to_string())?;
                    if let Some((bytes, suggested_name)) = pending.export_bytes {
                        set_progress(
                            format!("Exporting '{suggested_name}'..."),
                            format!("Writing {} KiB...", bytes.len() / 1024),
                            Some(0.75),
                        );
                        output.write_all(&bytes)?;
                        output.sync_all()?;
                        notice(
                            format!("Saved '{suggested_name}' to chosen location."),
                            true,
                        );
                    } else if let Some((src_path, suggested_name)) = pending.export_source_path {
                        let mut src_file = std::fs::File::open(&src_path)?;
                        let total = src_file.metadata().map(|m| m.len()).unwrap_or(0);
                        content_files::copy_bounded(&mut src_file, &mut output, |copied| {
                            let ratio = if total > 0 {
                                (copied as f32 / total as f32).clamp(0.1, 0.98)
                            } else {
                                0.6
                            };
                            set_progress(
                                format!("Exporting '{suggested_name}'..."),
                                format!("Written {} KiB...", copied / 1024),
                                Some(ratio),
                            );
                            !CANCEL.load(Ordering::Relaxed)
                        })?;
                        output.sync_all()?;
                        notice(
                            format!("Saved '{suggested_name}' to chosen location."),
                            true,
                        );
                    } else {
                        return Err("No pending export data".into());
                    }
                    Ok(())
                })();
                {
                    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
                    st.working = false;
                    st.progress = None;
                }
                if let Err(error) = result {
                    notice(format!("Export failed: {error}"), false);
                }
            });
        if let Err(error) = spawned {
            let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
            st.working = false;
            st.progress = None;
            drop(st);
            notice(format!("Export failed: {error}"), false);
        }
        return;
    }

    if request == 4102 {
        {
            let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
            st.working = true;
            st.status_title = "Importing folder...".into();
            st.status_detail = "Scanning folder structure...".into();
            st.progress = Some(0.05);
        }
        let spawned = std::thread::Builder::new()
            .name("Folder import".into())
            .spawn(move || {
                let result = (|| -> Result<()> {
                    let mut env = vm.attach_current_thread()?;
                    let target_dir = pending
                        .target_dir
                        .ok_or_else(|| "No target directory for folder import")?;
                    let copied = crate::document_bridge::copy_tree(
                        &mut env,
                        activity.as_obj(),
                        &uri,
                        &target_dir,
                        |done, total, current_file| {
                            if CANCEL.load(Ordering::Relaxed) {
                                return false;
                            }
                            let ratio = if total > 0 {
                                (done as f32 / total as f32).clamp(0.05, 0.99)
                            } else {
                                0.15
                            };
                            let pct = (ratio * 100.0).round() as u32;
                            set_progress(
                                format!("Importing folder ({done}/{total} files • {pct}%)..."),
                                format!("Copying: {current_file}"),
                                Some(ratio),
                            );
                            !CANCEL.load(Ordering::Relaxed)
                        },
                    );
                    if copied.is_err() {
                        let _ = env.exception_clear();
                    }
                    let folder_name = copied.map_err(|e| format!("Folder copy failed: {e}"))?;
                    if let Some(err_msg) = folder_name.strip_prefix("ERROR:") {
                        return Err(err_msg.trim().to_owned().into());
                    }
                    notice(format!("Imported folder '{folder_name}' successfully!"), true);
                    Ok(())
                })();
                {
                    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
                    st.working = false;
                    st.progress = None;
                }
                if let Err(error) = result {
                    if !CANCEL.load(Ordering::Relaxed) {
                        notice(format!("Import failed: {error}"), false);
                    }
                }
            });
        if let Err(error) = spawned {
            let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
            st.working = false;
            st.progress = None;
            drop(st);
            notice(format!("Import failed: {error}"), false);
        }
        return;
    }

    {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.working = true;
        st.status_title = "Importing package...".into();
        st.status_detail = "Reading selected file from Android storage...".into();
        st.progress = Some(0.15);
    }
    let spawned = std::thread::Builder::new()
        .name("Content import".into())
        .spawn(move || {
            let result = (|| -> Result<()> {
                let mut env = vm.attach_current_thread()?;
                let fallback = if pending.kind == Kind::Mods {
                    "import.hmod"
                } else {
                    "import.zip"
                };
                let opened =
                    crate::document_bridge::open(&mut env, activity.as_obj(), &uri, fallback);
                if opened.is_err() {
                    let _ = env.exception_clear();
                }
                let (input, name) = opened.map_err(|e| e.to_string())?;
                stage(pending, input, name)
            })();
            {
                let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
                st.working = false;
                st.progress = None;
            }
            if let Err(error) = result {
                if !CANCEL.load(Ordering::Relaxed) {
                    notice(format!("Import failed: {error}"), false);
                }
            }
        });
    if let Err(error) = spawned {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.working = false;
        st.progress = None;
        drop(st);
        notice(format!("Import failed: {error}"), false);
    }
}

fn stage(request: Request, mut input: std::fs::File, name: String) -> Result<()> {
    let total_file_bytes = input.metadata().map(|m| m.len()).unwrap_or(0);
    if request.control_import {
        use std::io::Read;
        set_progress(
            format!("Importing '{name}'..."),
            "Detecting control layout (MCPE / GoyControl)...",
            Some(0.5),
        );
        let mut bytes = Vec::new();
        input.take(2 * 1024 * 1024).read_to_end(&mut bytes)?;
        if CANCEL.load(Ordering::Relaxed) {
            return Err("Import cancelled".into());
        }
        let meta =
            horizon_client::touch::Settings::detect_and_import_preset(&request.home, &bytes, &name)?;
        notice(
            format!(
                "Imported {} preset '{}' into {}/{}.json",
                meta.system.label(),
                meta.name,
                meta.system.storage_subdir(),
                meta.id
            ),
            true,
        );
        return Ok(());
    }
    if let Some(target_dir) = request.target_dir {
        crate::document_store::validate_entry_name(&name)?;
        std::fs::create_dir_all(&target_dir)?;
        if std::fs::symlink_metadata(&target_dir)?
            .file_type()
            .is_symlink()
        {
            return Err("Refusing a redirected destination folder".into());
        }
        std::fs::create_dir_all(&request.cache)?;
        if std::fs::symlink_metadata(&request.cache)?
            .file_type()
            .is_symlink()
        {
            return Err("Refusing a redirected import cache".into());
        }
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let staged = request.cache.join(format!(".import-fm-{stamp}.part"));
        let res = (|| -> Result<()> {
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged)?;
            content_files::copy_bounded(&mut input, &mut output, |bytes| {
                let ratio = if total_file_bytes > 0 {
                    (bytes as f32 / total_file_bytes as f32).clamp(0.08, 0.85)
                } else {
                    0.5
                };
                set_progress(
                    format!("Importing '{name}'..."),
                    format!("Copied {} KiB...", bytes / 1024),
                    Some(ratio),
                );
                !CANCEL.load(Ordering::Relaxed)
            })?;
            output.sync_all()?;
            drop(output);
            if CANCEL.load(Ordering::Relaxed) {
                return Err("Import cancelled".into());
            }
            let lower_name = name.to_ascii_lowercase();
            let in_saves = target_dir
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s == "saves");
            if in_saves && (lower_name.ends_with(".zip") || lower_name.ends_with(".mcworld")) {
                set_progress(
                    format!("Extracting world '{name}'..."),
                    "Unpacking world archive into saves/...",
                    Some(0.92),
                );
                if let Ok(Some(world_folder)) =
                    crate::document_store::extract_world_zip_if_valid(&staged, &target_dir, &name)
                {
                    let _ = std::fs::remove_file(&staged);
                    notice(
                        format!("Imported world '{world_folder}' into Worlds."),
                        true,
                    );
                    return Ok(());
                }
            }
            let in_mods = target_dir
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s == "horizon-mods" || s == "horizon-native-mods");
            if in_mods
                && (lower_name.ends_with(".hnmod")
                    || lower_name.ends_with(".hmod")
                    || lower_name.ends_with(".hpack")
                    || lower_name.ends_with(".jar"))
            {
                set_progress(
                    format!("Compiling mod '{name}'..."),
                    "Running real-time Horizon Native Compiler...",
                    Some(0.92),
                );
                let (mod_id, ext) = content_files::read_mod_archive_id(&staged, &name)?;
                let canon_name = format!("{mod_id}.{ext}");
                let profile_root = target_dir.parent().unwrap_or(&target_dir);
                let mods_dir = profile_root.join("horizon-mods");
                std::fs::create_dir_all(&mods_dir)?;
                let dest = mods_dir.join(&canon_name);
                if std::fs::rename(&staged, &dest).is_err() {
                    std::fs::copy(&staged, &dest)?;
                    let _ = std::fs::remove_file(&staged);
                }
                let mut on = horizon_client::mod_list::ModList::load(profile_root);
                on.disabled.remove(&mod_id);
                on.enabled.insert(mod_id.clone());
                if ext == "mrpack" {
                    if let Ok((pack, rows, count)) = horizon_port_pack::android_port::port_modpack_natively(
                        &dest,
                        profile_root,
                        &|stage_msg, p| set_progress("Porting .mrpack Modpack...", stage_msg, Some(p)),
                    ) {
                        imported(
                            format!(
                                "horizon-port-pack: Ported '{}' ({count} mods carried over from {} files)!",
                                pack.name,
                                rows.len()
                            ),
                            Kind::Mods,
                        );
                        return Ok(());
                    }
                }
                if ext == "hpack" {
                    if let Ok(count) = horizon_client::jar_transpiler::install_hpack_modpack(&dest, profile_root) {
                        imported(
                            format!("Installed Horizon Modpack '{mod_id}' ({count} mods enabled)!"),
                            Kind::Mods,
                        );
                        return Ok(());
                    }
                }
                if ext == "hnmod" {
                    if let Ok(compiled) = horizon_client::jar_transpiler::compile_hnmod_bundle_realtime(&dest, profile_root) {
                        on.disabled.remove(&compiled.id);
                        on.enabled.insert(compiled.id.clone());
                        on.save(profile_root);
                        imported(
                            format!(
                                "Auto-compiled & enabled '{}' ({}.hnmod) — ready in game!",
                                compiled.title, compiled.id
                            ),
                            Kind::Mods,
                        );
                        return Ok(());
                    }
                }
                if ext == "jar"
                    && let Ok(transpiled) = horizon_client::jar_transpiler::transpile_jar_to_horizon_mods(&dest, profile_root)
                {
                    on.disabled.remove(&transpiled.id);
                    on.enabled.insert(transpiled.id.clone());
                    on.save(profile_root);
                    imported(
                        format!(
                            "Auto-compiled & enabled '{}' -> {}.hnmod + .hmod ({}) — ready in game!",
                            transpiled.title,
                            transpiled.id,
                            transpiled.features.join(", ")
                        ),
                        Kind::Mods,
                    );
                    return Ok(());
                }
                on.save(profile_root);
                imported(
                    format!("Imported & enabled mod '{canon_name}' ({mod_id})."),
                    Kind::Mods,
                );
                return Ok(());
            }
            let dest = crate::document_store::unique_destination_path(&target_dir, &name)?;
            let final_name = dest
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if std::fs::rename(&staged, &dest).is_err() {
                std::fs::copy(&staged, &dest)?;
                let _ = std::fs::remove_file(&staged);
            }
            notice(
                format!("Imported '{final_name}' into current folder."),
                true,
            );
            Ok(())
        })();
        if res.is_err() {
            let _ = std::fs::remove_file(&staged);
        }
        return res;
    }
    if let Some(kind) = request.appearance {
        use std::io::Read;
        if !name.to_ascii_lowercase().ends_with(".png") {
            return Err("Choose a PNG skin or cape".into());
        }
        let mut bytes = Vec::new();
        input
            .take(crate::skins::MAX_IMPORT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if CANCEL.load(Ordering::Relaxed) {
            return Err("Import cancelled".into());
        }
        let mut library = crate::skins::library::Library::load(&request.home)?;
        library.import(&request.home, &name, kind, &bytes)?;
        notice(
            "PNG added to the local gallery. Select it to apply locally.",
            false,
        );
        return Ok(());
    }
    if !content_files::plain_name(&name) || !request.kind.accepts(&name) {
        return Err("Selected file has the wrong extension for this folder".into());
    }
    let dir = request.home.join(request.kind.folder());
    std::fs::create_dir_all(&dir)?;
    if std::fs::symlink_metadata(&dir)?.file_type().is_symlink() {
        return Err("Refusing a redirected content directory".into());
    }
    std::fs::create_dir_all(&request.cache)?;
    if std::fs::symlink_metadata(&request.cache)?
        .file_type()
        .is_symlink()
    {
        return Err("Refusing a redirected import cache".into());
    }
    for entry in std::fs::read_dir(&request.cache)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".import-") && name.ends_with(".part") && entry.file_type()?.is_file() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staged = request.cache.join(format!(".import-{stamp}.part"));
    let result = (|| -> Result<()> {
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)?;
        content_files::copy_bounded(&mut input, &mut output, |bytes| {
            let ratio = if total_file_bytes > 0 {
                (bytes as f32 / total_file_bytes as f32).clamp(0.1, 0.85)
            } else {
                0.5
            };
            set_progress(
                format!("Importing {}...", request.kind.label()),
                format!("Copied {} KiB of '{name}'...", bytes / 1024),
                Some(ratio),
            );
            !CANCEL.load(Ordering::Relaxed)
        })?;
        output.sync_all()?;
        drop(output);
        set_progress(
            format!("Verifying {}...", request.kind.label()),
            "Validating package manifest and assets...",
            Some(0.9),
        );
        let native_shader = content_files::validate(&staged, request.kind, &name, || {
            CANCEL.load(Ordering::Relaxed)
        })?;
        let mut imported_mod_id: Option<(String, &'static str)> = None;
        let name = if request.kind == Kind::Mods {
            let (mod_id, ext) = content_files::read_mod_archive_id(&staged, &name)?;
            let fname = format!("{mod_id}.{ext}");
            imported_mod_id = Some((mod_id, ext));
            fname
        } else if native_shader || name.to_ascii_lowercase().ends_with(".hshader") {
            let pack = horizon_shader_api::Pack::read(&staged)?;
            horizon_shader_api::check(&pack).map_err(|e| format!("Invalid shader pack: {e:?}"))?;
            let mut path = PathBuf::from(name);
            path.set_extension("hshader");
            path.to_string_lossy().into_owned()
        } else {
            name
        };
        if !content_files::plain_name(&name) {
            return Err("Package has an invalid ID or filename".into());
        }
        if CANCEL.load(Ordering::Relaxed) {
            return Err("Import cancelled".into());
        }
        let target = dir.join(name);
        if target.exists() {
            STATE.lock().unwrap_or_else(|e| e.into_inner()).conflict = Some(Conflict {
                kind: request.kind,
                staged: staged.clone(),
                target,
            });
            notice(
                "File already exists: choose Keep both, Replace or Cancel",
                false,
            );
        } else {
            if std::fs::rename(&staged, &target).is_err() {
                std::fs::copy(&staged, &target)?;
                let _ = std::fs::remove_file(&staged);
            }
            let mut transpiled_msg: Option<String> = None;
            if let Some((mod_id, ext)) = imported_mod_id {
                let mut on = horizon_client::mod_list::ModList::load(&request.home);
                on.disabled.remove(&mod_id);
                on.enabled.insert(mod_id.clone());
                if ext == "mrpack"
                    && let Ok((pack, rows, count)) = horizon_port_pack::android_port::port_modpack_natively(
                        &target,
                        &request.home,
                        &|stage_msg, p| set_progress("Porting .mrpack Modpack...", stage_msg, Some(p)),
                    )
                {
                    transpiled_msg = Some(format!(
                        "horizon-port-pack: Ported '{}' ({count} mods carried over from {} files) — ready in game!",
                        pack.name,
                        rows.len()
                    ));
                } else if ext == "hpack"
                    && let Ok(count) = horizon_client::jar_transpiler::install_hpack_modpack(&target, &request.home)
                {
                    transpiled_msg = Some(format!(
                        "Installed Horizon Modpack '{mod_id}' ({count} mods enabled) — ready in game!"
                    ));
                } else if ext == "hnmod"
                    && let Ok(compiled) = horizon_client::jar_transpiler::compile_hnmod_bundle_realtime(&target, &request.home)
                {
                    on.disabled.remove(&compiled.id);
                    on.enabled.insert(compiled.id.clone());
                    transpiled_msg = Some(format!(
                        "Auto-compiled & enabled '{}' ({}.hnmod) — ready in game!",
                        compiled.title, compiled.id
                    ));
                } else if ext == "jar"
                    && let Ok(transpiled) = horizon_client::jar_transpiler::transpile_jar_to_horizon_mods(&target, &request.home)
                {
                    on.disabled.remove(&transpiled.id);
                    on.enabled.insert(transpiled.id.clone());
                    transpiled_msg = Some(format!(
                        "Auto-compiled & enabled '{}' -> {}.hnmod + .hmod ({}) — ready in game!",
                        transpiled.title,
                        transpiled.id,
                        transpiled.features.join(", ")
                    ));
                }
                on.save(&request.home);
            }
            imported(
                transpiled_msg.unwrap_or_else(|| {
                    format!(
                        "Imported {} into {} and enabled it.",
                        target.file_name().unwrap_or_default().to_string_lossy(),
                        request.kind.label()
                    )
                }),
                request.kind,
            );
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(staged);
    }
    result
}

pub fn resolve(choice: Choice) {
    let c = STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .conflict
        .take();
    let Some(mut c) = c else {
        return;
    };
    if matches!(choice, Choice::Cancel) {
        let _ = std::fs::remove_file(c.staged);
        notice("Import cancelled", false);
        return;
    }
    let result = (|| -> Result<()> {
        if matches!(choice, Choice::Replace)
            && std::fs::symlink_metadata(&c.target)
                .is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
        {
            return Err("Cannot replace a folder or symbolic link".into());
        }
        if matches!(choice, Choice::KeepBoth) {
            if c.kind == Kind::Mods {
                return Err(
                    "A mod ID cannot be installed twice; choose Replace to update it".into(),
                );
            }
            let mut stem = c
                .target
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            while stem.len() > 150 {
                stem.pop();
            }
            let ext = c
                .target
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let dir = c
                .target
                .parent()
                .ok_or("Invalid content folder")?
                .to_owned();
            for i in 1..=10000 {
                let candidate = dir.join(format!("{stem} ({i}).{ext}"));
                if !candidate.exists() {
                    c.target = candidate;
                    break;
                }
                if i == 10000 {
                    return Err("Too many filename conflicts".into());
                }
            }
        }
        if std::fs::rename(&c.staged, &c.target).is_err() {
            std::fs::copy(&c.staged, &c.target)?;
            let _ = std::fs::remove_file(&c.staged);
        }
        if c.kind == Kind::Mods
            && let Some(profile_root) = c.target.parent().and_then(|p| p.parent())
            && let Some(fname) = c.target.file_name().and_then(|s| s.to_str())
        {
            if fname.ends_with(".hnmod") {
                let native_dir = profile_root.join("horizon-native-mods");
                let _ = std::fs::create_dir_all(&native_dir);
                let _ = std::fs::copy(&c.target, native_dir.join(fname));
            }
            let mod_id = fname
                .strip_suffix(".hnmod")
                .or_else(|| fname.strip_suffix(".hmod"))
                .or_else(|| fname.strip_suffix(".jar"))
                .unwrap_or(fname);
            let mut on = horizon_client::mod_list::ModList::load(profile_root);
            on.disabled.remove(mod_id);
            on.enabled.insert(mod_id.to_owned());
            if fname.ends_with(".hnmod") {
                let _ = horizon_client::jar_transpiler::compile_hnmod_bundle_realtime(&c.target, profile_root);
            } else if fname.ends_with(".jar")
                && let Ok(transpiled) = horizon_client::jar_transpiler::transpile_jar_to_horizon_mods(&c.target, profile_root)
            {
                on.disabled.remove(&transpiled.id);
                on.enabled.insert(transpiled.id);
            }
            on.save(profile_root);
        }
        Ok(())
    })();
    match result {
        Ok(()) => imported(
            "Import completed. Enable it in Installed or game settings.",
            c.kind,
        ),
        Err(e) => {
            let _ = std::fs::remove_file(c.staged);
            notice(format!("Import failed: {e}"), false);
        }
    }
}

pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.request = None;
        s.working = false;
        s.progress = None;
        s.status_title.clear();
        s.status_detail.clear();
    }
    if conflict() {
        resolve(Choice::Cancel);
    } else {
        notice("Import cancelled", false);
    }
}

pub fn tap(x: f32, y: f32) -> bool {
    if !busy() {
        return false;
    }
    let choice = STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .hits
        .iter()
        .find(|(r, _)| {
            x >= r[0] - 8.0
                && x <= r[0] + r[2] + 8.0
                && y >= r[1] - 8.0
                && y <= r[1] + r[3] + 8.0
        })
        .map(|(_, c)| *c);
    if let Some(choice) = choice {
        if matches!(choice, Choice::Cancel) {
            cancel();
        } else {
            resolve(choice);
        }
    }
    true
}

pub fn draw(batch: &mut HudBatch, atlas: &HudAtlas) {
    let mut ui = Ui::mobile(atlas, batch, (0.0, 0.0));
    draw_ui(&mut ui);
    ui.finish();
}

/// Launcher overlays must use its bundled font, including before game fonts exist.
pub fn draw_ui(ui: &mut Ui<'_>) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.hits.clear();
    if s.conflict.is_none() && !s.working && s.request.is_none() {
        return;
    }

    if s.conflict.is_none() && (s.working || s.request.is_some()) {
        // Material 3 [Importing...] Dialog with live progress bar and instant Cancel button
        let w = 460.0f32.min(ui.w - 28.0);
        let h = 160.0;
        let x = (ui.w - w) / 2.0;
        let y = ((ui.h - h) / 2.0).max(0.0);

        // Dim background scrim
        ui.rounded(0.0, 0.0, ui.w, ui.h, 0.0, rgba(0, 0, 0, 175));

        // Card container
        ui.rounded(x, y, w, h, 14.0, rgba(24, 22, 34, 252));
        ui.border(x, y, w, h, 14.0, rgba(255, 255, 255, 24));

        // [Importing...] badge chip
        let chip_w = 98.0;
        ui.rounded(
            x + 16.0,
            y + 16.0,
            chip_w,
            24.0,
            6.0,
            rgba(130, 110, 245, 45),
        );
        ui.centered_fit(
            Face::SemiBold,
            11.5,
            [x + 16.0, y + 16.0, chip_w, 24.0],
            rgba(195, 180, 255, 255),
            "[Importing...]",
        );

        let title_text = if !s.status_title.is_empty() {
            s.status_title.as_str()
        } else {
            "Processing package..."
        };
        let fitted_title = ui.fit_text(Face::SemiBold, 15.0, title_text, w - chip_w - 42.0);
        ui.text(
            Face::SemiBold,
            15.0,
            x + 16.0 + chip_w + 10.0,
            y + 19.0,
            color::TEXT,
            &fitted_title,
        );

        let detail_text = if !s.status_detail.is_empty() {
            s.status_detail.as_str()
        } else {
            "Please wait, copying files into profile directory..."
        };
        let detail_fitted = ui.fit_text(Face::Regular, 13.0, detail_text, w - 32.0);
        ui.text(
            Face::Regular,
            13.0,
            x + 18.0,
            y + 54.0,
            rgba(195, 190, 210, 240),
            &detail_fitted,
        );

        // Material 2026 Wavy Progress Indicator + Morphing Spinner
        let pb_x = x + 18.0;
        let pb_y = y + 80.0;
        let pb_w = w - 36.0;
        let elapsed = horizon_client::ui_theme::motion::ambient_time();
        let fill_ratio = s
            .progress
            .unwrap_or_else(|| ((elapsed * 0.45).sin().abs() * 0.55 + 0.35).clamp(0.15, 0.95));
        let _fill_w = (pb_w * fill_ratio).clamp(8.0, pb_w);
        ui.m3_wavy_progress(
            pb_x,
            pb_y,
            pb_w,
            10.0,
            fill_ratio.clamp(0.06, 1.0),
            (elapsed * 0.85) % 1.0,
            false,
        );
        ui.m3_wavy_circular_spinner(x + w - 28.0, y + 28.0, 8.0, (elapsed * 0.75) % 1.0);

        // Cancel button
        let btn_w = 104.0;
        let btn_x = x + w - btn_w - 18.0;
        let btn_y = y + 106.0;
        ui.rounded(btn_x, btn_y, btn_w, 36.0, 8.0, rgba(235, 85, 95, 38));
        ui.border(btn_x, btn_y, btn_w, 36.0, 8.0, rgba(255, 120, 130, 95));
        ui.centered_fit(
            Face::SemiBold,
            13.0,
            [btn_x, btn_y, btn_w, 36.0],
            rgba(255, 210, 215, 255),
            "Cancel",
        );
        s.hits.push((
            [
                btn_x * ui.density,
                btn_y * ui.density,
                btn_w * ui.density,
                36.0 * ui.density,
            ],
            Choice::Cancel,
        ));
        return;
    }

    let (name, choices) = if let Some(c) = &s.conflict {
        (
            c.target
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            if c.kind == Kind::Mods {
                vec![("Replace", Choice::Replace), ("Cancel", Choice::Cancel)]
            } else {
                vec![
                    ("Keep both", Choice::KeepBoth),
                    ("Replace", Choice::Replace),
                    ("Cancel", Choice::Cancel),
                ]
            },
        )
    } else {
        (
            s.toast.as_ref().map(|v| v.0.clone()).unwrap_or_default(),
            vec![("Cancel", Choice::Cancel)],
        )
    };
    let w = 440.0f32.min(ui.w - 24.0);
    let vertical = w < 320.0;
    let height = if vertical {
        92.0 + choices.len() as f32 * 44.0
    } else {
        144.0
    };
    let x = (ui.w - w) / 2.0;
    let y = ((ui.h - height) / 2.0).max(0.0);
    ui.rounded(0.0, 0.0, ui.w, ui.h, 0.0, rgba(0, 0, 0, 180));
    ui.rounded(x, y, w, height, 12.0, rgba(26, 24, 36, 252));
    ui.border(x, y, w, height, 12.0, rgba(255, 255, 255, 24));
    ui.text(
        Face::SemiBold,
        18.0,
        x + 16.0,
        y + 14.0,
        color::TEXT,
        if s.conflict.is_some() {
            "File or folder exists"
        } else {
            "Importing file"
        },
    );
    let name = ui.fit_text(Face::Regular, 14.0, &name, w - 32.0);
    ui.text(
        Face::Regular,
        14.0,
        x + 16.0,
        y + 46.0,
        rgba(195, 190, 210, 240),
        &name,
    );
    let width = (w - 32.0) / choices.len() as f32;
    for (i, (label, choice)) in choices.into_iter().enumerate() {
        let rect = if vertical {
            [x + 16.0, y + 86.0 + i as f32 * 44.0, w - 32.0, 36.0]
        } else {
            [x + 16.0 + i as f32 * width, y + 92.0, width - 4.0, 36.0]
        };
        ui.rounded(
            rect[0],
            rect[1],
            rect[2],
            rect[3],
            8.0,
            rgba(255, 255, 255, 18),
        );
        ui.border(
            rect[0],
            rect[1],
            rect[2],
            rect[3],
            8.0,
            rgba(255, 255, 255, 30),
        );
        ui.centered_fit(Face::Medium, 13.0, rect, color::TEXT, label);
        s.hits.push((
            [
                rect[0] * ui.density,
                rect[1] * ui.density,
                rect[2] * ui.density,
                rect[3] * ui.density,
            ],
            choice,
        ));
    }
}

pub fn handle_event(event: &winit::event::WindowEvent) -> bool {
    use winit::{
        event::{ElementState, MouseButton, TouchPhase, WindowEvent},
        keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    };
    if !busy() {
        return false;
    }
    match event {
        WindowEvent::CursorMoved { position, .. } => {
            STATE.lock().unwrap_or_else(|e| e.into_inner()).cursor =
                (position.x as f32, position.y as f32);
            true
        }
        WindowEvent::MouseInput {
            button: MouseButton::Left,
            state,
            ..
        } => {
            if *state == ElementState::Released || *state == ElementState::Pressed {
                let (x, y) = STATE.lock().unwrap_or_else(|e| e.into_inner()).cursor;
                tap(x, y);
            }
            true
        }
        WindowEvent::Touch(t) => {
            if matches!(t.phase, TouchPhase::Started | TouchPhase::Ended) {
                tap(t.location.x as f32, t.location.y as f32);
            }
            true
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state == ElementState::Pressed {
                if event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                    || event.logical_key == Key::Named(NamedKey::BrowserBack)
                {
                    cancel();
                } else if event.physical_key == PhysicalKey::Code(KeyCode::Enter) && conflict() {
                    let native = STATE
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .conflict
                        .as_ref()
                        .is_some_and(|c| c.kind == Kind::Mods);
                    resolve(if native {
                        Choice::Cancel
                    } else {
                        Choice::KeepBoth
                    });
                }
            }
            true
        }
        _ => false,
    }
}
