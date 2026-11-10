//! Native Android in-process execution of `horizon-port-pack` for Java `.jar` mods,
//! Modrinth `.mrpack` / CurseForge `.zip` modpacks, and 26.3 data pack upgrades.
//!
//! Runs the full `horizon-port-pack` pipeline (`jar::read`, `classes::census`, `carry::carry`,
//! `check::check`, `events::report`, `pipeline::port`, and `upgrade::upgrade_into`) directly
//! on Android without spawning external desktop CLI processes, and packages the carried-over
//! folder mod into both `<profile>/horizon-mods/<id>/` and `<profile>/horizon-mods/<id>.hmod`.

use crate::{
    carry,
    check::{self, Check, Verdict},
    events, import, jar, pipeline, report, upgrade,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// Complete diagnostic and artifact summary returned when porting a Java `.jar` natively on Android.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AndroidPortResult {
    pub mod_id: String,
    pub name: String,
    pub version: String,
    pub loader: String,
    pub license: String,
    pub verdict: Verdict,
    pub summary: String,
    pub ports: Vec<String>,
    pub skipped: Vec<String>,
    pub depends: Vec<String>,
    pub namespaces: Vec<String>,
    pub mapped_hooks: Vec<String>,
    pub carry_dir: PathBuf,
    pub hmod_path: PathBuf,
    pub check_json_path: PathBuf,
    pub blocks_ported: usize,
    pub items_ported: usize,
    pub recipes_upgraded: usize,
    pub loot_tables_upgraded: usize,
    pub worldgen_ported: usize,
    pub classes_scanned: usize,
}

/// Recursively copies a directory tree from `from` to `to`.
fn copy_dir_all(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_all(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        }
    }
    Ok(())
}

/// Packages a carried-over folder mod (`carry_dir` containing `horizon-mod.toml`, `assets/`, `data/`)
/// into a deterministic `.hmod` archive at `hmod_path`.
pub fn package_carry_dir_to_hmod(carry_dir: &Path, hmod_path: &Path) -> Result<(), String> {
    if !carry_dir.join(horizon_mods::manifest::MANIFEST).is_file() {
        return Err(format!(
            "{}: missing {}",
            carry_dir.display(),
            horizon_mods::manifest::MANIFEST
        ));
    }
    if let Some(parent) = hmod_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    fn collect_files(
        root: &Path,
        cur: &Path,
        out: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(cur).map_err(|e| format!("{}: {e}", cur.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                collect_files(root, &path, out)?;
            } else {
                let rel = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                out.insert(rel, bytes);
            }
        }
        Ok(())
    }
    collect_files(carry_dir, carry_dir, &mut files)?;

    let file = File::create(hmod_path).map_err(|e| format!("{}: {e}", hmod_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (rel, bytes) in files {
        zip.start_file(&rel, opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Runs the complete `horizon-port-pack` single-JAR check, carry-over, and NeoForge/Fabric event
/// hook mapping natively on Android, installing the carried-over content into `<profile_dir>/horizon-mods/<id>/`
/// and `<profile_dir>/horizon-mods/<id>.hmod`.
pub fn port_jar_natively(
    jar_path: &Path,
    profile_dir: &Path,
    progress: &dyn Fn(&str, f32),
) -> Result<AndroidPortResult, String> {
    progress("horizon-port-pack: Reading Java .jar metadata & census...", 0.12);
    let info = jar::read(jar_path)?;
    let mod_id = info.mod_id.clone();

    let port_work_dir = profile_dir
        .join("mod-browser")
        .join("port-pack")
        .join(&mod_id);
    let _ = fs::remove_dir_all(&port_work_dir);
    fs::create_dir_all(&port_work_dir)
        .map_err(|e| format!("{}: {e}", port_work_dir.display()))?;

    progress(
        &format!(
            "horizon-port-pack: Carrying over {} ({}) assets, recipes, loot & worldgen...",
            info.name, mod_id
        ),
        0.38,
    );
    let chk: Check = check::check(jar_path, &port_work_dir)?;

    progress(
        "horizon-port-pack: Mapping bytecode events to Horizon Hook Catalog (API 415)...",
        0.68,
    );
    let events_report = events::report(jar_path, events::CATALOG).unwrap_or_default();
    let _ = fs::write(port_work_dir.join("events.txt"), &events_report);
    let mapped_hooks: Vec<String> = events_report
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .take(16)
        .map(str::to_owned)
        .collect();

    let carry_report: Option<carry::Report> =
        pipeline::read_json(&port_work_dir.join("carry-report.json")).ok();
    let (blocks_ported, items_ported, recipes_upgraded, loot_tables_upgraded, worldgen_ported) =
        if let Some(ref r) = carry_report {
            (
                r.blocks.len(),
                r.items,
                r.recipes_upgraded,
                r.loot_tables_upgraded + r.loot_tables_added,
                r.worldgen,
            )
        } else {
            (
                info.census.blockstates,
                info.census.item_models,
                info.census.recipes,
                info.census.loot_tables,
                info.census.worldgen,
            )
        };

    let stage_carry = port_work_dir.join("carry");
    let mods_dir = profile_dir.join("horizon-mods");
    fs::create_dir_all(&mods_dir).map_err(|e| format!("{}: {e}", mods_dir.display()))?;
    let target_carry_dir = mods_dir.join(&mod_id);
    let target_hmod_path = mods_dir.join(format!("{mod_id}.hmod"));

    if stage_carry.join(horizon_mods::manifest::MANIFEST).is_file() {
        progress(
            &format!("horizon-port-pack: Installing carried mod {mod_id} into horizon-mods/..."),
            0.85,
        );
        let _ = fs::remove_dir_all(&target_carry_dir);
        copy_dir_all(&stage_carry, &target_carry_dir)?;
        let _ = package_carry_dir_to_hmod(&stage_carry, &target_hmod_path);
    }

    let check_json_path = port_work_dir.join("check.json");
    progress(
        &format!("horizon-port-pack: {} ({})", chk.name, chk.verdict.label()),
        1.0,
    );

    Ok(AndroidPortResult {
        mod_id: chk.mod_id,
        name: chk.name,
        version: chk.version,
        loader: chk.loader,
        license: chk.license,
        verdict: chk.verdict,
        summary: chk.summary,
        ports: chk.ports,
        skipped: chk.skipped,
        depends: chk.depends,
        namespaces: chk.namespaces,
        mapped_hooks,
        carry_dir: target_carry_dir,
        hmod_path: target_hmod_path,
        check_json_path,
        blocks_ported,
        items_ported,
        recipes_upgraded,
        loot_tables_upgraded,
        worldgen_ported,
        classes_scanned: chk.code.classes,
    })
}

/// Ports an entire Modrinth `.mrpack` or CurseForge `.zip` modpack natively on Android
/// using `horizon_port_pack::pipeline::{import, inventory, port, report}` and installs all
/// carried-over mods, resource packs, and data packs into `profile_dir`.
pub fn port_modpack_natively(
    pack_path: &Path,
    profile_dir: &Path,
    progress: &dyn Fn(&str, f32),
) -> Result<(import::Pack, Vec<report::Row>, usize), String> {
    let stem = pack_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("modpack");
    let work_dir = profile_dir
        .join("ports")
        .join(stem.to_ascii_lowercase().replace(' ', "-"));
    fs::create_dir_all(&work_dir).map_err(|e| format!("{}: {e}", work_dir.display()))?;

    let log_fn = |line: String| {
        progress(&format!("horizon-port-pack: {line}"), 0.45);
    };
    progress("horizon-port-pack: Importing modpack manifest & overrides...", 0.15);
    let pack = pipeline::import(pack_path, &work_dir, &log_fn)?;

    progress("horizon-port-pack: Building Modrinth inventory & dependency layers...", 0.35);
    let _ = pipeline::inventory(&work_dir, false, &log_fn)?;

    progress("horizon-port-pack: Fetching & carrying over portable mods...", 0.65);
    let (_, rows) = pipeline::port(&work_dir, None, None, &log_fn)?;

    // Copy carried-over folder mods into profile_dir/horizon-mods/ and package .hmod archives
    let mods_dir = profile_dir.join("horizon-mods");
    fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    let mut installed_mods = 0usize;
    if let Ok(entries) = fs::read_dir(work_dir.join("mods")) {
        for entry in entries.flatten() {
            let carry = entry.path().join("carry");
            if carry.join(horizon_mods::manifest::MANIFEST).is_file() {
                let key = entry.file_name().to_string_lossy().into_owned();
                let dst = mods_dir.join(&key);
                let _ = fs::remove_dir_all(&dst);
                if copy_dir_all(&carry, &dst).is_ok() {
                    let _ = package_carry_dir_to_hmod(&carry, &mods_dir.join(format!("{key}.hmod")));
                    installed_mods += 1;
                }
            }
        }
    }
    let _ = crate::resourcepacks::install(&work_dir, profile_dir);
    progress(
        &format!(
            "horizon-port-pack: Ported {} mod(s) from {}",
            installed_mods, pack.name
        ),
        1.0,
    );
    Ok((pack, rows, installed_mods))
}

/// Upgrades a 1.21 / 26.2 data pack (`.zip` or folder) to 26.3 worldgen format natively on Android.
pub fn upgrade_datapack_natively(pack_path: &Path, out_dir: &Path) -> Result<Vec<String>, String> {
    let report = upgrade::upgrade_into(pack_path, out_dir)?;
    Ok(report.lines())
}
