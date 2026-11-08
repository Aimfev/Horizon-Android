//! NativeActivity loads its library with dlopen, outside the Java class loader.
//! Bind the result callback explicitly instead of relying on JNI symbol discovery.
use jni::{
    JNIEnv, NativeMethod,
    objects::{JObject, JString},
};

pub type Callback = extern "system" fn(JNIEnv<'_>, JObject<'_>, i32, i32, JString<'_>);

pub fn register(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    callback: Callback,
) -> jni::errors::Result<()> {
    let class = env.get_object_class(activity)?;
    env.register_native_methods(
        class,
        &[NativeMethod {
            name: "nativeDocumentResult".into(),
            sig: "(IILjava/lang/String;)V".into(),
            fn_ptr: callback as *mut std::ffi::c_void,
        }],
    )
}

/// Open the granted document on a worker. No filesystem path is inferred from its URI.
pub fn open(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    fallback: &str,
) -> Result<(std::fs::File, String), Box<dyn std::error::Error + Send + Sync>> {
    use jni::objects::JValue;
    use std::os::fd::FromRawFd;
    env.with_local_frame(24, |env| {
        let resolver = env.call_method(activity, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()?;
        let text = JObject::from(env.new_string(uri)?);
        let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&text)])?.l()?;
        let projection = env.new_object_array(1, "java/lang/String", JObject::null())?;
        env.set_object_array_element(&projection, 0, env.new_string("_display_name")?)?;
        let null = JObject::null();
        let cursor = env.call_method(&resolver, "query", "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;", &[JValue::Object(&uri), JValue::Object(projection.as_ref()), JValue::Object(&null), JValue::Object(&null), JValue::Object(&null)])?.l()?;
        let name = if cursor.is_null() { None } else {
            let result = (|| -> jni::errors::Result<Option<String>> {
                if !env.call_method(&cursor, "moveToFirst", "()Z", &[])?.z()? { return Ok(None); }
                let value = env.call_method(&cursor, "getString", "(I)Ljava/lang/String;", &[JValue::Int(0)])?.l()?;
                if value.is_null() { Ok(None) } else { Ok(Some(env.get_string((&value).into())?.into())) }
            })();
            if result.is_err() { let _ = env.exception_clear(); }
            env.call_method(&cursor, "close", "()V", &[])?;
            result?
        }.unwrap_or_else(|| fallback.into());
        let mode = JObject::from(env.new_string("r")?);
        let fd = env.call_method(&resolver, "openFileDescriptor", "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;", &[JValue::Object(&uri), JValue::Object(&mode)])?.l()?;
        if fd.is_null() { return Err("Document provider returned no file".into()); }
        let raw = env.call_method(&fd, "detachFd", "()I", &[])?.i()?;
        if raw < 0 { return Err("Document provider returned an invalid descriptor".into()); }
        // detachFd transferred ownership to Rust.
        let file = unsafe { std::fs::File::from_raw_fd(raw) };
        env.call_method(&fd, "close", "()V", &[])?;
        Ok((file, name))
    })
}

/// Open a newly created document URI (`ACTION_CREATE_DOCUMENT`) for writing.
pub fn create_output(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
) -> Result<std::fs::File, Box<dyn std::error::Error + Send + Sync>> {
    use jni::objects::JValue;
    use std::os::fd::FromRawFd;
    env.with_local_frame(16, |env| {
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )?
            .l()?;
        let text = JObject::from(env.new_string(uri)?);
        let uri_obj = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&text)],
            )?
            .l()?;
        let mode = JObject::from(env.new_string("wt")?);
        let fd = env
            .call_method(
                &resolver,
                "openFileDescriptor",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;",
                &[JValue::Object(&uri_obj), JValue::Object(&mode)],
            )?
            .l()?;
        if fd.is_null() {
            return Err("Document provider returned no writable file".into());
        }
        let raw = env.call_method(&fd, "detachFd", "()I", &[])?.i()?;
        if raw < 0 {
            return Err("Document provider returned an invalid descriptor".into());
        }
        let file = unsafe { std::fs::File::from_raw_fd(raw) };
        env.call_method(&fd, "close", "()V", &[])?;
        Ok(file)
    })
}

/// Recursively copies an Android SAF document tree (`ACTION_OPEN_DOCUMENT_TREE`) into `dest_parent`
/// while reporting real file-by-file progress and honoring cancellation via `on_progress`.
pub fn copy_tree(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    tree_uri: &str,
    dest_parent: &std::path::Path,
    mut on_progress: impl FnMut(usize, usize, &str) -> bool,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use jni::objects::JValue;
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;

    std::fs::create_dir_all(dest_parent)?;

    if !on_progress(0, 1, "Resolving selected folder...") {
        return Err("Import cancelled".into());
    }

    // 1. Resolve root document ID and folder display name using bootstrap-classloader DocumentsContract
    let (root_doc_id, folder_name) = env.with_local_frame(
        24,
        |env| -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
            let resolver = env
                .call_method(
                    activity,
                    "getContentResolver",
                    "()Landroid/content/ContentResolver;",
                    &[],
                )?
                .l()?;
            let j_tree_str = JObject::from(env.new_string(tree_uri)?);
            let tree_uri_obj = env
                .call_static_method(
                    "android/net/Uri",
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(&j_tree_str)],
                )?
                .l()?;
            let doc_id_obj = match env.call_static_method(
                "android/provider/DocumentsContract",
                "getTreeDocumentId",
                "(Landroid/net/Uri;)Ljava/lang/String;",
                &[JValue::Object(&tree_uri_obj)],
            ) {
                Ok(v) => v.l()?,
                Err(_) => {
                    let _ = env.exception_clear();
                    env.call_static_method(
                        "android/provider/DocumentsContract",
                        "getDocumentId",
                        "(Landroid/net/Uri;)Ljava/lang/String;",
                        &[JValue::Object(&tree_uri_obj)],
                    )?
                    .l()?
                }
            };
            if doc_id_obj.is_null() {
                return Err("Could not resolve selected folder document ID".into());
            }
            let root_id: String = env.get_string((&doc_id_obj).into())?.into();

            let mut display_name: Option<String> = None;
            if let Ok(doc_uri) = env.call_static_method(
                "android/provider/DocumentsContract",
                "buildDocumentUriUsingTree",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&tree_uri_obj), JValue::Object(&doc_id_obj)],
            ) {
                if let Ok(doc_uri_obj) = doc_uri.l() {
                    let proj = env.new_object_array(1, "java/lang/String", JObject::null())?;
                    env.set_object_array_element(&proj, 0, env.new_string("_display_name")?)?;
                    let null = JObject::null();
                    if let Ok(cur_val) = env.call_method(
                        &resolver,
                        "query",
                        "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                        &[
                            JValue::Object(&doc_uri_obj),
                            JValue::Object(proj.as_ref()),
                            JValue::Object(&null),
                            JValue::Object(&null),
                            JValue::Object(&null),
                        ],
                    ) {
                        if let Ok(cursor) = cur_val.l() {
                            if !cursor.is_null() {
                                if env
                                    .call_method(&cursor, "moveToFirst", "()Z", &[])
                                    .and_then(|v| v.z())
                                    .unwrap_or(false)
                                {
                                    if let Ok(s_obj) = env
                                        .call_method(
                                            &cursor,
                                            "getString",
                                            "(I)Ljava/lang/String;",
                                            &[JValue::Int(0)],
                                        )
                                        .and_then(|v| v.l())
                                    {
                                        if !s_obj.is_null() {
                                            if let Ok(javastr) = env.get_string((&s_obj).into()) {
                                                let s: String = javastr.into();
                                                if !s.trim().is_empty() {
                                                    display_name = Some(s.trim().to_owned());
                                                }
                                            }
                                        }
                                    }
                                }
                                let _ = env.exception_clear();
                                let _ = env.call_method(&cursor, "close", "()V", &[]);
                            }
                        }
                    }
                }
            }
            let _ = env.exception_clear();

            let fallback_name = root_id
                .rsplit(['/', ':'])
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("imported_world")
                .to_owned();
            let raw_folder = display_name.unwrap_or(fallback_name);
            let safe_folder = sanitize_entry_name(&raw_folder, "imported_world");
            Ok((root_id, safe_folder))
        },
    )?;

    let mut dest_folder = dest_parent.join(&folder_name);
    let mut suffix = 1usize;
    while dest_folder.exists() {
        dest_folder = dest_parent.join(format!("{folder_name} ({suffix})"));
        suffix += 1;
    }
    std::fs::create_dir_all(&dest_folder)?;
    let final_folder_name = dest_folder
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or(folder_name);

    // 2. Traverse the SAF tree and collect all files while reporting live progress
    let mut stack = vec![(root_doc_id, dest_folder.clone())];
    let mut pending_files: Vec<(String, std::path::PathBuf, String)> = Vec::new();

    while let Some((parent_doc_id, current_dir)) = stack.pop() {
        if !on_progress(0, pending_files.len().max(1), "Scanning folder structure...") {
            let _ = std::fs::remove_dir_all(&dest_folder);
            return Err("Import cancelled".into());
        }
        std::fs::create_dir_all(&current_dir)?;
        let children: Vec<(String, String, String)> = env.with_local_frame(
            32,
            |env| -> Result<Vec<(String, String, String)>, Box<dyn std::error::Error + Send + Sync>> {
                let resolver = env
                    .call_method(
                        activity,
                        "getContentResolver",
                        "()Landroid/content/ContentResolver;",
                        &[],
                    )?
                    .l()?;
                let j_tree_str = JObject::from(env.new_string(tree_uri)?);
                let tree_uri_obj = env
                    .call_static_method(
                        "android/net/Uri",
                        "parse",
                        "(Ljava/lang/String;)Landroid/net/Uri;",
                        &[JValue::Object(&j_tree_str)],
                    )?
                    .l()?;
                let j_parent_id = JObject::from(env.new_string(&parent_doc_id)?);
                let children_uri = env
                    .call_static_method(
                        "android/provider/DocumentsContract",
                        "buildChildDocumentsUriUsingTree",
                        "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                        &[JValue::Object(&tree_uri_obj), JValue::Object(&j_parent_id)],
                    )?
                    .l()?;
                let proj = env.new_object_array(3, "java/lang/String", JObject::null())?;
                env.set_object_array_element(&proj, 0, env.new_string("document_id")?)?;
                env.set_object_array_element(&proj, 1, env.new_string("_display_name")?)?;
                env.set_object_array_element(&proj, 2, env.new_string("mime_type")?)?;
                let null = JObject::null();
                let cursor = match env.call_method(
                    &resolver,
                    "query",
                    "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                    &[
                        JValue::Object(&children_uri),
                        JValue::Object(proj.as_ref()),
                        JValue::Object(&null),
                        JValue::Object(&null),
                        JValue::Object(&null),
                    ],
                ) {
                    Ok(v) => v.l()?,
                    Err(_) => {
                        let _ = env.exception_clear();
                        return Ok(Vec::new());
                    }
                };
                if cursor.is_null() {
                    return Ok(Vec::new());
                }
                let mut list = Vec::new();
                while env
                    .call_method(&cursor, "moveToNext", "()Z", &[])
                    .and_then(|v| v.z())
                    .unwrap_or(false)
                {
                    let mut read_col = |col: i32| -> Option<String> {
                        let is_null = env
                            .call_method(&cursor, "isNull", "(I)Z", &[JValue::Int(col)])
                            .and_then(|v| v.z())
                            .unwrap_or(true);
                        if is_null {
                            let _ = env.exception_clear();
                            return None;
                        }
                        let val = env
                            .call_method(
                                &cursor,
                                "getString",
                                "(I)Ljava/lang/String;",
                                &[JValue::Int(col)],
                            )
                            .and_then(|v| v.l());
                        match val {
                            Ok(obj) if !obj.is_null() => {
                                env.get_string((&obj).into()).ok().map(Into::into)
                            }
                            _ => {
                                let _ = env.exception_clear();
                                None
                            }
                        }
                    };
                    if let (Some(cid), Some(cname)) = (read_col(0), read_col(1)) {
                        let cmime = read_col(2).unwrap_or_default();
                        list.push((cid, cname, cmime));
                    }
                }
                let _ = env.exception_clear();
                let _ = env.call_method(&cursor, "close", "()V", &[]);
                Ok(list)
            },
        )?;

        for (child_id, raw_name, mime) in children {
            let safe_name = sanitize_entry_name(&raw_name, "");
            if safe_name.is_empty() || safe_name == "." || safe_name == ".." {
                continue;
            }
            if mime == "vnd.android.document/directory" {
                let sub = current_dir.join(&safe_name);
                let _ = std::fs::create_dir_all(&sub);
                stack.push((child_id, sub));
            } else if pending_files.len() < 20000 {
                pending_files.push((child_id, current_dir.join(&safe_name), safe_name));
            }
        }
    }

    // If pure-JNI scan found 0 files, fall back to HorizonActivity.copyFolderTree
    if pending_files.is_empty() {
        let _ = std::fs::remove_dir_all(&dest_folder);
        let java_attempt: Result<Option<String>, jni::errors::Error> =
            env.with_local_frame(16, |env| {
                let activity_class = env.get_object_class(activity)?;
                let j_uri = JObject::from(env.new_string(tree_uri)?);
                let target_str = dest_parent.to_string_lossy().into_owned();
                let j_target = JObject::from(env.new_string(&target_str)?);
                let res = env.call_static_method(
                    &activity_class,
                    "copyFolderTree",
                    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    &[
                        JValue::Object(activity),
                        JValue::Object(&j_uri),
                        JValue::Object(&j_target),
                    ],
                )?;
                let obj = res.l()?;
                if obj.is_null() {
                    return Ok(None);
                }
                let out: String = env.get_string((&obj).into())?.into();
                Ok(Some(out))
            });
        match java_attempt {
            Ok(Some(fname)) if !fname.starts_with("ERROR:") && !fname.is_empty() => {
                let copied_dir = dest_parent.join(&fname);
                flatten_nested_world_dir_if_needed(dest_parent, &copied_dir);
                return Ok(fname);
            }
            Ok(Some(err)) => {
                return Err(err
                    .strip_prefix("ERROR:")
                    .unwrap_or(&err)
                    .trim()
                    .to_owned()
                    .into());
            }
            Err(e) => {
                let _ = env.exception_clear();
                return Err(format!("Could not read selected folder: {e}").into());
            }
            _ => return Err("Selected folder was empty or inaccessible".into()),
        }
    }

    let total_files = pending_files.len();
    let mut buf = [0u8; 65536];

    for (idx, (child_id, target_file, safe_name)) in pending_files.into_iter().enumerate() {
        if !on_progress(idx, total_files, &safe_name) {
            let _ = std::fs::remove_dir_all(&dest_folder);
            return Err("Import cancelled".into());
        }
        let opened: Result<Option<std::fs::File>, Box<dyn std::error::Error + Send + Sync>> =
            env.with_local_frame(16, |env| {
                let resolver = env
                    .call_method(
                        activity,
                        "getContentResolver",
                        "()Landroid/content/ContentResolver;",
                        &[],
                    )?
                    .l()?;
                let j_tree_str = JObject::from(env.new_string(tree_uri)?);
                let tree_uri_obj = env
                    .call_static_method(
                        "android/net/Uri",
                        "parse",
                        "(Ljava/lang/String;)Landroid/net/Uri;",
                        &[JValue::Object(&j_tree_str)],
                    )?
                    .l()?;
                let j_child_id = JObject::from(env.new_string(&child_id)?);
                let child_uri = env
                    .call_static_method(
                        "android/provider/DocumentsContract",
                        "buildDocumentUriUsingTree",
                        "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                        &[JValue::Object(&tree_uri_obj), JValue::Object(&j_child_id)],
                    )?
                    .l()?;
                let mode = JObject::from(env.new_string("r")?);
                let fd = match env.call_method(
                    &resolver,
                    "openFileDescriptor",
                    "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;",
                    &[JValue::Object(&child_uri), JValue::Object(&mode)],
                ) {
                    Ok(v) => v.l()?,
                    Err(_) => {
                        let _ = env.exception_clear();
                        return Ok(None);
                    }
                };
                if fd.is_null() {
                    return Ok(None);
                }
                let raw = env.call_method(&fd, "detachFd", "()I", &[])?.i()?;
                let _ = env.call_method(&fd, "close", "()V", &[]);
                if raw < 0 {
                    return Ok(None);
                }
                Ok(Some(unsafe { std::fs::File::from_raw_fd(raw) }))
            });
        match opened {
            Ok(Some(mut src)) => {
                if let Ok(mut dst) = std::fs::File::create(&target_file) {
                    loop {
                        let Ok(n) = src.read(&mut buf) else {
                            break;
                        };
                        if n == 0 {
                            break;
                        }
                        if dst.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        if !on_progress(idx + 1, total_files, &safe_name) {
                            drop(dst);
                            let _ = std::fs::remove_dir_all(&dest_folder);
                            return Err("Import cancelled".into());
                        }
                    }
                }
            }
            _ => {
                let _ = env.exception_clear();
                if safe_name.eq_ignore_ascii_case("session.lock") && !target_file.exists() {
                    let _ = std::fs::File::create(&target_file);
                }
            }
        }
    }

    let _ = on_progress(total_files, total_files, &final_folder_name);
    flatten_nested_world_dir_if_needed(dest_parent, &dest_folder);
    Ok(final_folder_name)
}

fn sanitize_entry_name(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// If importing into `saves` produced `saves/<Outer>/<Inner>/level.dat` where `<Outer>` has no
/// `level.dat` directly, promote `<Inner>`'s contents into `<Outer>` so Minecraft recognizes the world.
fn flatten_nested_world_dir_if_needed(dest_parent: &std::path::Path, copied_dir: &std::path::Path) {
    let in_saves = dest_parent
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s == "saves");
    if !in_saves || !copied_dir.is_dir() || copied_dir.join("level.dat").exists() {
        return;
    }
    for _ in 0..2 {
        if copied_dir.join("level.dat").exists() {
            break;
        }
        let Ok(entries) = std::fs::read_dir(copied_dir) else {
            break;
        };
        let subdirs: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        if let Some(world_sub) = subdirs
            .iter()
            .find(|p| p.join("level.dat").exists() || p.join("region").is_dir())
            .or_else(|| (subdirs.len() == 1).then(|| &subdirs[0]))
        {
            if let Ok(inner_entries) = std::fs::read_dir(world_sub) {
                for entry in inner_entries.flatten() {
                    let from = entry.path();
                    let to = copied_dir.join(entry.file_name());
                    if !to.exists() {
                        let _ = std::fs::rename(&from, &to);
                    }
                }
            }
            let _ = std::fs::remove_dir(world_sub);
        } else {
            break;
        }
    }
}
