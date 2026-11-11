//! Bundled official Android native mods + Live In-Process `.hnmod` & `.jar` Runtime Engine.
//!
//! All installed `.hnmod` and transpiled Modrinth/Java `.jar` mods are automatically compiled
//! in the Horizon Launcher and executed live in-process here — with zero manual build steps,
//! zero "Try again" dialogs, and full support for authentic per-mod Settings, HUD overlays,
//! tooltips, Wrench block rotation & machine dismantling, Veinminer BFS ore harvesting with
//! real item drops, TreeFall/Chop! tree felling with leaf decay, RightClickHarvest crop
//! harvesting & replanting, AppleSkin saturation/hunger HUD & tooltips, LookAt/Jade top-center
//! block inspection, Fullbright gamma & cave lighting, Carry Block, Keystrokes, FPS Counter,
//! Coordinates HUD, Armor Status, Enchantment Hints, ShulkerPeek, Toggle Sprint, and
//! ImmersiveThunder acoustics & camera rumble.

use super::*;
use super::native_hooks::*;
use horizon_core::position::BlockPos;
use horizon_world::chunk::BlockState;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(target_os = "android")]
#[path = "android_bundled/zoom/runtime.rs"]
pub(super) mod official_zoom;

#[cfg(target_os = "android")]
#[path = "android_bundled/zoom/zoom_key.rs"]
mod zoom_key;

#[cfg(target_os = "android")]
#[path = "android_bundled/zoom/zoom_wheel.rs"]
mod zoom_wheel;

#[cfg(target_os = "android")]
#[path = "android_bundled/zoom/zoom_zoom.rs"]
mod zoom_zoom;

#[cfg(target_os = "android")]
#[path = "android_bundled/zoom/zoom_list.rs"]
mod zoom_list;

#[cfg(target_os = "android")]
#[path = "android_bundled/zoom/zoom_settings.rs"]
mod zoom_settings;

#[cfg(target_os = "android")]
#[path = "android_bundled/zoom/zoom_click.rs"]
mod zoom_click;

#[cfg(target_os = "android")]
#[path = "android_bundled/minimap/runtime.rs"]
pub(super) mod official_minimap;

#[cfg(target_os = "android")]
#[path = "android_bundled/minimap/minimap_list.rs"]
mod minimap_list;

#[cfg(target_os = "android")]
#[path = "android_bundled/minimap/minimap_settings.rs"]
mod minimap_settings;

#[cfg(target_os = "android")]
#[path = "android_bundled/minimap/minimap_click.rs"]
mod minimap_click;

#[cfg(target_os = "android")]
#[path = "android_bundled/minimap/minimap_minimap.rs"]
mod minimap_minimap;

#[cfg(target_os = "android")]
#[path = "android_bundled/recipes/runtime.rs"]
pub(super) mod official_recipes;

#[cfg(target_os = "android")]
#[path = "android_bundled/recipes/glue.rs"]
pub(super) mod official_recipes_glue;

#[cfg(target_os = "android")]
#[path = "android_bundled/recipes/recipes_list.rs"]
mod recipes_list;

#[cfg(target_os = "android")]
#[path = "android_bundled/recipes/recipes_settings.rs"]
mod recipes_settings;

#[cfg(target_os = "android")]
#[path = "android_bundled/recipes/recipes_click.rs"]
mod recipes_click;

#[cfg(target_os = "android")]
#[path = "android_bundled/recipes/recipes_frame.rs"]
mod recipes_frame;

#[cfg(target_os = "android")]
#[path = "android_bundled/recipes/recipes_event.rs"]
mod recipes_event;

#[cfg(target_os = "android")]
#[path = "android_bundled/smartf3/runtime.rs"]
pub(super) mod smartf3;

#[cfg(target_os = "android")]
#[path = "android_bundled/smartf3/native.rs"]
pub(super) mod smartf3_native;

#[cfg(target_os = "android")]
#[path = "android_bundled/smartf3/smartf3_command.rs"]
mod smartf3_command;

#[cfg(target_os = "android")]
#[path = "android_bundled/smartf3/smartf3_list.rs"]
mod smartf3_list;

#[cfg(target_os = "android")]
#[path = "android_bundled/smartf3/smartf3_settings.rs"]
mod smartf3_settings;

#[cfg(target_os = "android")]
#[path = "android_bundled/smartf3/smartf3_click.rs"]
mod smartf3_click;

#[derive(Clone, Debug)]
pub(crate) struct DynamicHnmod {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) features: BTreeSet<String>,
    pub(crate) custom_buttons: Vec<String>,
    pub(crate) items: Vec<String>,
    pub(crate) blocks: Vec<String>,
    pub(crate) keys: Vec<[String; 3]>,
    pub(crate) commands: Vec<String>,
    pub(crate) zoom_held: bool,
    pub(crate) vein_key_held: bool,
}

#[derive(Default)]
struct DynamicRuntimeState {
    mods: BTreeMap<String, DynamicHnmod>,
    file_cache: BTreeMap<PathBuf, (u64, Option<std::time::SystemTime>, DynamicHnmod)>,
    last_scan: Option<Instant>,
    thunder_rumble_ticks: u32,
    last_thunder_sound_tick: i64,
    vein_guard: bool,
    carried_block: Option<horizon_world::registry::BlockStateId>,
    carried_block_name: Option<String>,
    fullbright_was_active: bool,
    saved_gamma: Option<f32>,
}

static DYNAMIC_RUNTIME: OnceLock<Mutex<DynamicRuntimeState>> = OnceLock::new();

fn runtime() -> &'static Mutex<DynamicRuntimeState> {
    DYNAMIC_RUNTIME.get_or_init(|| Mutex::new(DynamicRuntimeState::default()))
}

/// Live check whether `mod_id` is enabled in Options > Mods / Launcher across both the
/// Client Window thread and the Singleplayer Integrated Server thread.
pub(super) fn native_on_check(game: &Game<'_>, mod_id: &str) -> bool {
    if let Some(live_disabled) = horizon_client::mod_list::is_live_disabled(mod_id) {
        return !live_disabled;
    }
    !game.settings.mods.disabled.contains(mod_id)
}

pub(super) fn candidate_profile_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(parent) = Game::options_path().parent() {
        roots.push(parent.to_path_buf());
    }
    let home = horizon_client::logging::private_home();
    if !roots.iter().any(|r| r == &home) {
        roots.push(home.clone());
    }
    if let Ok(cwd) = std::env::current_dir() {
        if !roots.iter().any(|r| r == &cwd) {
            roots.push(cwd.clone());
        }
    }
    // Also include subdirectories under <home>/profiles/profile-* and <cwd>/profiles/profile-*
    for base in [&home, &std::env::current_dir().unwrap_or_default()] {
        let profiles_dir = base.join("profiles");
        if let Ok(rd) = fs::read_dir(&profiles_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() && !roots.contains(&p) {
                    roots.push(p);
                }
            }
        }
    }
    roots
}

pub(super) fn parse_dynamic_hnmod(path: &std::path::Path) -> Option<DynamicHnmod> {
    let bytes = fs::read(path).ok()?;
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).ok()?;

    let raw_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("native_mod");
    let clean_stem = raw_stem
        .find('-')
        .filter(|&i| raw_stem[i + 1..].chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|i| &raw_stem[..i])
        .unwrap_or(raw_stem);
    let mut mod_id = horizon_client::jar_transpiler::sanitize_mod_id(clean_stem);
    let mut version = "1.0.0".to_owned();
    let mut title = String::new();
    let mut description = String::new();
    let mut combined_rs = String::new();
    let mut features = BTreeSet::new();
    let mut custom_buttons = Vec::new();
    let mut items = Vec::new();
    let mut blocks = Vec::new();
    let mut keys = Vec::new();
    let mut commands = Vec::new();

    for i in 0..zip.len().min(2048) {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let Ok(entry_name) = entry.name() else {
            continue;
        };
        let name = entry_name.replace('\\', "/");
        if name == "horizon-native.json" {
            let mut s = String::new();
            let _ = entry.read_to_string(&mut s);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                    mod_id = horizon_client::jar_transpiler::sanitize_mod_id(id);
                }
                if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
                    if !ver.trim().is_empty() {
                        version = ver.trim().to_owned();
                    }
                }
            }
        } else if name == "horizon-compiled.json"
            || name.ends_with(horizon_client::jar_transpiler::COMPILED_META)
        {
            let mut s = String::new();
            let _ = entry.read_to_string(&mut s);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(t) = v.get("title").and_then(|x| x.as_str()) {
                    if !t.trim().is_empty() {
                        title = t.trim().to_owned();
                    }
                }
                if let Some(d) = v.get("description").and_then(|x| x.as_str()) {
                    if !d.trim().is_empty() {
                        description = d.trim().to_owned();
                    }
                }
                if let Some(arr) = v.get("features").and_then(|x| x.as_array()) {
                    for feat in arr.iter().filter_map(|x| x.as_str()) {
                        features.insert(feat.to_owned());
                    }
                }
                if let Some(arr) = v.get("custom_buttons").and_then(|x| x.as_array()) {
                    for btn in arr.iter().filter_map(|x| x.as_str()) {
                        custom_buttons.push(btn.to_owned());
                    }
                }
                if let Some(arr) = v.get("items").and_then(|x| x.as_array()) {
                    for it in arr.iter().filter_map(|x| x.as_str()) {
                        items.push(it.to_owned());
                    }
                }
                if let Some(arr) = v.get("blocks").and_then(|x| x.as_array()) {
                    for bl in arr.iter().filter_map(|x| x.as_str()) {
                        blocks.push(bl.to_owned());
                    }
                }
                if let Some(arr) = v.get("commands").and_then(|x| x.as_array()) {
                    for cmd in arr.iter().filter_map(|x| x.as_str()) {
                        commands.push(cmd.to_owned());
                    }
                }
            }
        } else if name == "horizon-mod.toml" || name == "mod.toml" {
            let mut s = String::new();
            if entry.read_to_string(&mut s).is_ok() {
                if let Some(id) = horizon_client::jar_transpiler::extract_toml_field(&s, "id") {
                    mod_id = horizon_client::jar_transpiler::sanitize_mod_id(&id);
                }
                if let Some(t) = horizon_client::jar_transpiler::extract_toml_field(&s, "name") {
                    if title.is_empty() {
                        title = t;
                    }
                }
                if let Some(ver) = horizon_client::jar_transpiler::extract_toml_field(&s, "version") {
                    if !ver.trim().is_empty() {
                        version = ver;
                    }
                }
                if let Some(d) = horizon_client::jar_transpiler::extract_toml_field(&s, "description") {
                    if description.is_empty() {
                        description = d;
                    }
                }
                if let Some(caps_pos) = s.find("capabilities") {
                    if let Some(open) = s[caps_pos..].find('[') {
                        if let Some(close) = s[caps_pos + open..].find(']') {
                            let array_content = &s[caps_pos + open + 1..caps_pos + open + close];
                            for item in array_content.split(',') {
                                let item = item.trim().trim_matches('"').trim_matches('\'').trim();
                                if !item.is_empty() {
                                    match item {
                                        "hud" | "render" => {
                                            features.insert("hud_overlay".to_owned());
                                        }
                                        "tick" => {
                                            features.insert("tick_hook".to_owned());
                                        }
                                        "chat" => {
                                            features.insert("chat_hook".to_owned());
                                        }
                                        "command" => {
                                            features.insert("command_hook".to_owned());
                                        }
                                        "input" => {
                                            features.insert("input_hook".to_owned());
                                        }
                                        "registries" => {
                                            features.insert("registries".to_owned());
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if name.starts_with("assets/") || name.starts_with("data/") {
            features.insert("content".to_owned());
        } else if name.ends_with(".rs") || name.ends_with("horizon-mixin.toml") {
            let mut s = String::new();
            if entry.read_to_string(&mut s).is_ok() {
                combined_rs.push_str(&s);
                combined_rs.push('\n');
            }
        }
    }

    let (parsed_title, parsed_desc, parsed_btns) =
        horizon_client::jar_transpiler::parse_rust_mod_metadata(&combined_rs, &mod_id);
    if let Some(t) = parsed_title {
        title = t;
    }
    if let Some(d) = parsed_desc {
        description = d;
    }
    for b in parsed_btns {
        if !custom_buttons.contains(&b) {
            custom_buttons.push(b);
        }
    }

    if title.is_empty() {
        title = format!(
            "{} {}",
            horizon_client::jar_transpiler::title_case_id(&mod_id),
            version
        );
    } else if !title.contains(&version) {
        title = format!("{title} {version}");
    }
    if description.is_empty() {
        description = format!("Horizon Native Mod ({mod_id})");
    }

    let inferred = horizon_client::jar_transpiler::infer_features_from_id_and_signals(
        &mod_id,
        &title,
        &description,
        &combined_rs,
        &features,
    );
    features.extend(inferred);

    // Populate authentic keybinds per mod type
    if features.contains("wrench") {
        // Wrench uses right-click on blocks, no keybind needed (just like official wrench)
    } else if features.contains("veinminer") {
        keys.push([
            "veinmine".into(),
            "Veinmine / Excavate key".into(),
            "key.keyboard.grave.accent".into(),
        ]);
    } else if features.contains("treefall") {
        keys.push([
            "treefall_toggle".into(),
            "Toggle TreeFall".into(),
            "key.keyboard.v".into(),
        ]);
    } else if features.contains("fullbright") {
        keys.push([
            "fullbright_toggle".into(),
            "Toggle Fullbright".into(),
            "key.keyboard.g".into(),
        ]);
    } else if features.contains("toggle_sprint") {
        keys.push([
            "toggle_sprint".into(),
            "Toggle Sprint".into(),
            "key.keyboard.r".into(),
        ]);
    } else if features.contains("waypoints") {
        keys.push([
            "waypoint_add".into(),
            "Create Waypoint at Player".into(),
            "key.keyboard.b".into(),
        ]);
    } else if features.contains("zoom") {
        keys.push(["zoom".into(), "Zoom key".into(), "key.keyboard.c".into()]);
    }

    if custom_buttons.is_empty() {
        custom_buttons.push("Active in Game: ON".to_owned());
    }

    if commands.is_empty() {
        commands.push(mod_id.clone());
    }

    Some(DynamicHnmod {
        id: mod_id,
        title,
        description,
        features,
        custom_buttons,
        items,
        blocks,
        keys,
        commands,
        zoom_held: false,
        vein_key_held: false,
    })
}

fn ensure_dynamic_mods_loaded(game: &Game<'_>, force: bool) {
    let _ = game;
    let mut guard = match runtime().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let now = Instant::now();
    if !force {
        if let Some(last) = guard.last_scan {
            if now.duration_since(last) < Duration::from_secs(30) {
                return;
            }
        }
    }
    guard.last_scan = Some(now);

    let roots = candidate_profile_roots();

    // Sync ModList from disk if LIVE_MOD_STATE hasn't been populated yet
    for root in &roots {
        if root.join(horizon_client::mod_list::FILE).is_file() {
            let _ = horizon_client::mod_list::ModList::load(root);
            break;
        }
    }

    // Auto-compile any raw .jar files sitting in horizon-mods/ or horizon-native-mods/
    for root in &roots {
        for sub in [
            "mods",
            horizon_client::mod_list::DIR,
            horizon_client::mod_list::NATIVE_DIR,
        ] {
            let folder = root.join(sub);
            let Ok(rd) = fs::read_dir(&folder) else {
                continue;
            };
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")) {
                    let _ = horizon_client::jar_transpiler::transpile_jar_to_horizon_mods(&p, root);
                }
            }
        }
    }

    // Collect all .hnmod bundles across horizon-native-mods/ and horizon-mods/
    let mut hnmod_paths = Vec::new();
    for root in &roots {
        for sub in [
            "mods",
            horizon_client::mod_list::NATIVE_DIR,
            horizon_client::mod_list::DIR,
        ] {
            let folder = root.join(sub);
            let Ok(rd) = fs::read_dir(&folder) else {
                continue;
            };
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("hnmod") || e.eq_ignore_ascii_case("hmod")) {
                    hnmod_paths.push((root.clone(), p));
                }
            }
        }
    }
    hnmod_paths.sort_by(|a, b| a.1.cmp(&b.1));

    let mut discovered = BTreeMap::new();
    for (root, path) in hnmod_paths {
        let meta = fs::metadata(&path).ok();
        let file_len = meta.as_ref().map_or(0, std::fs::Metadata::len);
        let file_mtime = meta.as_ref().and_then(|m| m.modified().ok());
        let cached = guard
            .file_cache
            .get(&path)
            .filter(|(len, mtime, _)| *len == file_len && *mtime == file_mtime)
            .map(|(_, _, parsed)| parsed.clone());
        let parsed_opt = match cached {
            Some(c) => Some(c),
            None => {
                if let Ok(bytes) = fs::read(&path) {
                    if let Ok(mut z) = zip::ZipArchive::new(Cursor::new(&bytes)) {
                        let compiled = (0..z.len().min(2048)).any(|i| {
                            z.by_index(i).is_ok_and(|e| {
                                e.name().is_ok_and(|n| {
                                    n == "horizon-compiled.json"
                                        || n.ends_with(
                                            horizon_client::jar_transpiler::COMPILED_META,
                                        )
                                })
                            })
                        });
                        if !compiled {
                            let _ = horizon_client::jar_transpiler::compile_hnmod_bundle_realtime(
                                &path, &root,
                            );
                        }
                    }
                }
                let parsed = parse_dynamic_hnmod(&path);
                if let Some(p) = &parsed {
                    guard
                        .file_cache
                        .insert(path.clone(), (file_len, file_mtime, p.clone()));
                }
                parsed
            }
        };
        if let Some(mut parsed) = parsed_opt {
            if matches!(
                parsed.id.as_str(),
                "zoom" | "minimap" | "recipes" | "smartf3"
            ) {
                continue;
            }
            if let Some(prev) = guard.mods.get(&parsed.id) {
                parsed.zoom_held = prev.zoom_held;
                parsed.vein_key_held = prev.vein_key_held;
            }
            discovered.entry(parsed.id.clone()).or_insert(parsed);
        }
    }
    guard.mods = discovered;
}

// ============================================================================
// Official Wrench Rules (from horizon-native-kit/engine/native-mods/wrench)
// ============================================================================

const WRENCH_ITEMS: [&str; 4] = [
    "minecraft:stick",
    "minecraft:blaze_rod",
    "minecraft:breeze_rod",
    "minecraft:bone",
];
const WR_UP: [i32; 3] = [0, 1, 0];
const WR_DIRECTIONS: [(&str, [i32; 3]); 6] = [
    ("down", [0, -1, 0]),
    ("up", [0, 1, 0]),
    ("north", [0, 0, -1]),
    ("south", [0, 0, 1]),
    ("west", [-1, 0, 0]),
    ("east", [1, 0, 0]),
];
const WR_AXES: [(&str, [i32; 3]); 3] = [("x", [1, 0, 0]), ("y", [0, 1, 0]), ("z", [0, 0, 1])];

fn wr_direction(name: &str) -> Option<[i32; 3]> {
    WR_DIRECTIONS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, d)| *d)
}
fn wr_direction_name(d: [i32; 3]) -> Option<&'static str> {
    WR_DIRECTIONS
        .iter()
        .find(|(_, v)| *v == d)
        .map(|(n, _)| *n)
}
fn wr_axis(name: &str) -> Option<[i32; 3]> {
    WR_AXES.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}
fn wr_axis_name(d: [i32; 3]) -> Option<&'static str> {
    let d = d.map(i32::abs);
    WR_AXES.iter().find(|(_, v)| *v == d).map(|(n, _)| *n)
}
fn wr_turned(v: [i32; 3], face: [i32; 3]) -> [i32; 3] {
    let c = [
        v[1] * face[2] - v[2] * face[1],
        v[2] * face[0] - v[0] * face[2],
        v[0] * face[1] - v[1] * face[0],
    ];
    if c == [0, 0, 0] { v } else { c }
}
fn wr_short(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}
fn wr_turnable(name: &str, properties: &BTreeMap<String, String>) -> bool {
    let name = wr_short(name);
    let prop = |k: &str| properties.get(k).map(String::as_str);
    let game_master = matches!(
        name,
        "command_block"
            | "chain_command_block"
            | "repeating_command_block"
            | "structure_block"
            | "jigsaw"
    );
    !(game_master
        || matches!(name, "piston_head" | "moving_piston" | "nether_portal")
        || properties.contains_key("part")
        || matches!(prop("half"), Some("upper" | "lower"))
        || matches!(prop("type"), Some("left" | "right"))
        || prop("extended") == Some("true"))
}
fn wr_turns(
    name: &str,
    properties: &BTreeMap<String, String>,
    face: [i32; 3],
) -> Vec<BTreeMap<String, String>> {
    if !wr_turnable(name, properties) {
        return Vec::new();
    }
    let mut out: Vec<BTreeMap<String, String>> = Vec::new();
    let mut push = |key: &str, value: &str| {
        if properties.get(key).is_some_and(|v| v != value) {
            let mut p = properties.clone();
            p.insert(key.to_owned(), value.to_owned());
            if !out.contains(&p) {
                out.push(p);
            }
        }
    };
    let side = face[1] == 0;
    match properties.get("half").map(String::as_str) {
        Some("top") if side => push("half", "bottom"),
        Some("bottom") if side => push("half", "top"),
        _ => {}
    }
    match properties.get("type").map(String::as_str) {
        Some("top") => push("type", "bottom"),
        Some("bottom") => push("type", "top"),
        _ => {}
    }
    if let Some(f) = properties.get("facing").and_then(|f| wr_direction(f)) {
        for to in [wr_turned(f, face), wr_turned(f, WR_UP), f.map(|v| -v)] {
            if let Some(to) = wr_direction_name(to) {
                push("facing", to);
            }
        }
    }
    if let Some(a) = properties.get("axis").and_then(|a| wr_axis(a)) {
        if let Some(to) = wr_axis_name(wr_turned(a, face)) {
            push("axis", to);
        }
        let at = WR_AXES.iter().position(|(_, v)| *v == a).unwrap_or(0);
        for step in 1..WR_AXES.len() {
            push("axis", WR_AXES[(at + step) % WR_AXES.len()].0);
        }
    }
    if let Some(r) = properties
        .get("rotation")
        .and_then(|r| r.parse::<u8>().ok())
    {
        push("rotation", &((r + 4) % 16).to_string());
    }
    out
}
fn wr_takes_apart(name: &str) -> bool {
    let name = wr_short(name);
    matches!(
        name,
        "piston"
            | "sticky_piston"
            | "observer"
            | "dispenser"
            | "dropper"
            | "hopper"
            | "crafter"
            | "repeater"
            | "comparator"
            | "redstone_lamp"
            | "note_block"
            | "lever"
            | "daylight_detector"
            | "target"
            | "redstone_torch"
            | "redstone_wall_torch"
            | "redstone_wire"
            | "redstone_block"
            | "tripwire_hook"
            | "rail"
            | "powered_rail"
            | "detector_rail"
            | "activator_rail"
            | "slime_block"
            | "honey_block"
    ) || name.ends_with("_button")
        || name.ends_with("_pressure_plate")
        || name.ends_with("lightning_rod")
}
fn wr_wrench_item(setting: Option<&str>) -> String {
    match setting.map(str::trim) {
        Some(id) if !id.is_empty() && id.contains(':') => id.to_owned(),
        Some(id) if !id.is_empty() => format!("minecraft:{id}"),
        _ => WRENCH_ITEMS[0].to_owned(),
    }
}
fn wr_next_item(item: &str) -> &'static str {
    let at = WRENCH_ITEMS.iter().position(|i| *i == item);
    WRENCH_ITEMS[at.map_or(0, |i| (i + 1) % WRENCH_ITEMS.len())]
}
fn wr_item_label(item: &str) -> String {
    wr_short(item)
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ============================================================================
// Per-Mod Authentic Settings & Helpers
// ============================================================================

fn setting_bool(game: &Game<'_>, mod_id: &str, key: &str, default: bool) -> bool {
    match game.native_setting(mod_id, key).as_deref().map(str::trim) {
        Some("true" | "on" | "1") => true,
        Some("false" | "off" | "0") => false,
        _ => default,
    }
}

fn toggle_setting_bool(game: &Game<'_>, mod_id: &str, key: &str, default: bool) -> bool {
    let next = !setting_bool(game, mod_id, key, default);
    game.native_set_setting(mod_id, key, if next { "true" } else { "false" });
    next
}

fn setting_u32(game: &Game<'_>, mod_id: &str, key: &str, default: u32) -> u32 {
    game.native_setting(mod_id, key)
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

fn cycle_u32(game: &Game<'_>, mod_id: &str, key: &str, choices: &[u32], default: u32) -> u32 {
    let cur = setting_u32(game, mod_id, key, default);
    let idx = choices.iter().position(|&c| c == cur).unwrap_or(0);
    let next = choices[(idx + 1) % choices.len()];
    game.native_set_setting(mod_id, key, &next.to_string());
    next
}

fn setting_corner(
    game: &Game<'_>,
    mod_id: &str,
    default: horizon_client::extras::Corner,
) -> horizon_client::extras::Corner {
    use horizon_client::extras::Corner;
    match game
        .native_setting(mod_id, "corner")
        .as_deref()
        .map(str::trim)
    {
        Some("top_left" | "0") => Corner::TopLeft,
        Some("top_right" | "1") => Corner::TopRight,
        Some("bottom_left" | "2") => Corner::BottomLeft,
        Some("bottom_right" | "3") => Corner::BottomRight,
        _ => default,
    }
}

fn corner_label(c: horizon_client::extras::Corner) -> &'static str {
    use horizon_client::extras::Corner;
    match c {
        Corner::TopLeft => "Top Left",
        Corner::TopRight => "Top Right",
        Corner::BottomLeft => "Bottom Left",
        Corner::BottomRight => "Bottom Right",
    }
}

fn cycle_corner(game: &Game<'_>, mod_id: &str, default: horizon_client::extras::Corner) {
    use horizon_client::extras::Corner;
    let next = match setting_corner(game, mod_id, default) {
        Corner::TopLeft => "top_right",
        Corner::TopRight => "bottom_left",
        Corner::BottomLeft => "bottom_right",
        Corner::BottomRight => "top_left",
    };
    game.native_set_setting(mod_id, "corner", next);
}

fn estimate_food_values(item_id: &str) -> Option<(u32, f32)> {
    let short = item_id.strip_prefix("minecraft:").unwrap_or(item_id);
    match short {
        "golden_carrot" => Some((6, 14.4)),
        "golden_apple" | "enchanted_golden_apple" => Some((4, 9.6)),
        "cooked_beef" | "cooked_porkchop" => Some((8, 12.8)),
        "cooked_mutton" | "cooked_salmon" => Some((6, 9.6)),
        "cooked_chicken" | "baked_potato" | "bread" | "cooked_cod" | "cooked_rabbit" => {
            Some((5, 6.0))
        }
        "pumpkin_pie" => Some((8, 4.8)),
        "rabbit_stew" => Some((10, 12.0)),
        "mushroom_stew" | "beetroot_soup" | "suspicious_stew" => Some((6, 7.2)),
        "apple" | "chorus_fruit" => Some((4, 2.4)),
        "carrot" | "beef" | "porkchop" => Some((3, 1.8)),
        "melon_slice" | "sweet_berries" | "glow_berries" | "cookie" | "chicken" | "mutton"
        | "cod" | "salmon" => Some((2, 1.2)),
        "dried_kelp" | "potato" | "beetroot" | "tropical_fish" => Some((1, 0.6)),
        "honey_bottle" => Some((6, 1.2)),
        "rotten_flesh" => Some((4, 0.8)),
        "spider_eye" => Some((2, 3.2)),
        "poisonous_potato" => Some((2, 1.2)),
        "pufferfish" => Some((1, 0.2)),
        "cake" => Some((14, 2.8)),
        _ if short.contains("bread")
            || short.contains("stew")
            || short.contains("pie")
            || short.contains("cooked") =>
        {
            Some((6, 7.2))
        }
        _ if short.contains("apple")
            || short.contains("berry")
            || short.contains("berries")
            || short.contains("fruit")
            || short.contains("food") =>
        {
            Some((4, 3.2))
        }
        _ => None,
    }
}

fn enchantment_description(ench_id: &str) -> Option<&'static str> {
    let short = ench_id.strip_prefix("minecraft:").unwrap_or(ench_id);
    match short {
        "sharpness" => Some("Increases melee attack damage dealt to all targets."),
        "smite" => Some("Increases melee damage against undead mobs (zombies, skeletons, wither)."),
        "bane_of_arthropods" => Some("Increases damage against spiders, cave spiders, silverfish, and bees."),
        "knockback" => Some("Increases knockback distance when hitting mobs."),
        "fire_aspect" => Some("Sets targets on fire when hit with a melee attack."),
        "looting" => Some("Mobs drop more items and have a higher rare drop chance."),
        "sweeping_edge" => Some("Increases damage dealt by sweep attacks."),
        "efficiency" => Some("Increases mining and block-breaking speed."),
        "silk_touch" => Some("Mined blocks drop themselves instead of their normal drops."),
        "unbreaking" => Some("Reduces the chance of an item taking durability damage."),
        "fortune" => Some("Increases block drop yields from ores and crops."),
        "power" => Some("Increases arrow damage shot from bows."),
        "punch" => Some("Increases arrow knockback distance."),
        "flame" => Some("Arrows are set on fire and ignite hit targets."),
        "infinity" => Some("Shooting consumes no regular arrows (requires at least 1 arrow)."),
        "luck_of_the_sea" => Some("Increases luck of catching treasure while fishing."),
        "lure" => Some("Decreases wait time for fish to bite the hook."),
        "loyalty" => Some("Trident returns automatically after being thrown."),
        "impaling" => Some("Trident deals extra damage to aquatic mobs."),
        "riptide" => Some("Launches the player with the trident when in water or rain."),
        "channeling" => Some("Summons a lightning bolt on hit targets during thunderstorms."),
        "multishot" => Some("Fires 3 arrows at the cost of 1 from a crossbow."),
        "quick_charge" => Some("Decreases crossbow reloading time."),
        "piercing" => Some("Arrows pass through multiple entities and shields."),
        "density" => Some("Increases mace smash damage per block fallen."),
        "breach" => Some("Reduces the effectiveness of target armor against mace hits."),
        "wind_burst" => Some("Emits an upward wind burst upon landing a mace smash attack."),
        "mending" => Some("Repairs item durability using collected experience orbs."),
        "vanishing_curse" => Some("Item is destroyed upon death instead of dropping."),
        "binding_curse" => Some("Prevents removing the equipped item from its armor slot."),
        "protection" => Some("Reduces most types of incoming damage."),
        "fire_protection" => Some("Reduces fire and lava damage and burn time."),
        "feather_falling" => Some("Reduces fall damage and ender pearl teleport damage."),
        "blast_protection" => Some("Reduces explosion damage and explosion knockback."),
        "projectile_protection" => Some("Reduces damage from arrows, fireballs, and tridents."),
        "respiration" => Some("Extends underwater breathing time."),
        "aqua_affinity" => Some("Increases underwater mining speed to match land speed."),
        "thorns" => Some("Damages attackers when they hit you."),
        "depth_strider" => Some("Increases underwater movement speed."),
        "frost_walker" => Some("Turns water beneath your feet into frosted ice."),
        "soul_speed" => Some("Increases movement speed on soul sand and soul soil."),
        "swift_sneak" => Some("Increases movement speed while sneaking/crouching."),
        _ => None,
    }
}

// ============================================================================
// Hook Implementations
// ============================================================================

pub(super) fn native_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    let _ = (&game, &key, &pressed);
    #[cfg(target_os = "android")]
    {
        zoom_key::run(&mut *game, key, pressed);
    }

    ensure_dynamic_mods_loaded(game, false);
    let mut chat_msg: Option<String> = None;

    if let Ok(mut guard) = runtime().lock() {
        for m in guard.mods.values_mut() {
            if !game.native_on(&m.id) {
                continue;
            }
            for [k_name, _, k_def] in &m.keys {
                let bound = game.native_mod_key(&m.id, k_name, k_def);
                if bound == key {
                    if m.features.contains("zoom") {
                        m.zoom_held = pressed;
                    } else if m.features.contains("veinminer") {
                        m.vein_key_held = pressed;
                    } else if pressed && m.features.contains("fullbright") {
                        let on = toggle_setting_bool(game, &m.id, "enabled", true);
                        chat_msg = Some(format!(
                            "§e[Fullbright]§r Brightness boost: {}",
                            if on { "§aON" } else { "§cOFF" }
                        ));
                    } else if pressed && m.features.contains("toggle_sprint") {
                        let on = toggle_setting_bool(game, &m.id, "sprint_on", true);
                        chat_msg = Some(format!(
                            "§b[Toggle Sprint]§r Sprint: {}",
                            if on { "§aToggled ON" } else { "§7Vanilla" }
                        ));
                    } else if pressed && m.features.contains("treefall") {
                        let on = toggle_setting_bool(game, &m.id, "enabled", true);
                        chat_msg = Some(format!(
                            "§a[{}]§r Tree felling: {}",
                            m.title,
                            if on { "§aON" } else { "§cOFF" }
                        ));
                    } else if pressed && m.features.contains("waypoints") {
                        if let Some(p) = game.native_player() {
                            let wp_str = format!("{:.0},{:.0},{:.0}", p.x, p.y, p.z);
                            game.native_set_setting(&m.id, "last_waypoint", &wp_str);
                            chat_msg = Some(format!(
                                "§d[Waypoints]§r Saved waypoint at §f{}, {}, {}",
                                p.x as i32, p.y as i32, p.z as i32
                            ));
                        }
                    }
                }
            }
        }
    }

    if let Some(msg) = chat_msg {
        game.native_chat(&msg);
    }
}

pub(super) fn native_wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    let _ = (&game, &steps, &claimed);
    #[cfg(target_os = "android")]
    {
        zoom_wheel::run(&*game, steps, &mut *claimed);
    }
}

pub(super) fn native_zoom(game: &Game<'_>, zoom: &mut Option<NativeZoom>) {
    let _ = (&game, &zoom);
    #[cfg(target_os = "android")]
    {
        zoom_zoom::run(game, &mut *zoom);
    }
    if zoom.is_some() {
        return;
    }
    if let Ok(guard) = runtime().lock() {
        for m in guard.mods.values() {
            if game.native_on(&m.id) && m.zoom_held {
                let factor = setting_u32(game, &m.id, "zoom_level", 4).clamp(2, 10) as f32;
                let smooth = setting_bool(game, &m.id, "smooth", true);
                *zoom = Some(NativeZoom {
                    factor,
                    smooth,
                    turn: 1.0 / factor,
                });
                break;
            }
        }
    }
}

pub(super) fn native_mods(game: &mut Game<'_>, mods: &mut Vec<NativeMod>) {
    let _ = (&game, &mods);
    #[cfg(target_os = "android")]
    {
        zoom_list::run(&mut *mods);
        minimap_list::run(&mut *mods);
        recipes_list::run(&mut *mods);
        smartf3_list::run(&mut *mods);
    }

    ensure_dynamic_mods_loaded(game, true);
    if let Ok(guard) = runtime().lock() {
        for m in guard.mods.values() {
            if mods.iter().any(|existing| existing.id == m.id) {
                continue;
            }
            mods.push(NativeMod {
                id: m.id.clone(),
                title: m.title.clone(),
                description: m.description.clone(),
                keys: m.keys.clone(),
            });
        }
    }
}

pub(super) fn native_settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    let _ = (&game, &mod_id, &buttons);
    #[cfg(target_os = "android")]
    {
        zoom_settings::run(&mut *game, mod_id, &mut *buttons);
        minimap_settings::run(&mut *game, mod_id, &mut *buttons);
        recipes_settings::run(&mut *game, mod_id, &mut *buttons);
        smartf3_settings::run(&mut *game, mod_id, &mut *buttons);
    }
    if !buttons.is_empty() {
        return;
    }

    ensure_dynamic_mods_loaded(game, false);
    let Ok(guard) = runtime().lock() else {
        return;
    };
    let Some(m) = guard.mods.get(mod_id) else {
        return;
    };
    let on_off = |b: bool| if b { "ON" } else { "OFF" };

    if m.features.contains("wrench") {
        let item = wr_wrench_item(game.native_setting(mod_id, "item").as_deref());
        let apart = game
            .native_setting(mod_id, "take-apart")
            .as_deref()
            .map(str::trim)
            != Some("off");
        buttons.push(format!("Wrench: {}", wr_item_label(&item)));
        buttons.push(format!("Sneak takes machines apart: {}", on_off(apart)));
        return;
    }

    if m.features.contains("veinminer") {
        let mode = game
            .native_setting(mod_id, "mode")
            .unwrap_or_else(|| "always".to_owned());
        let mode_lbl = match mode.as_str() {
            "sneak" => "Sneak (Shift) Only",
            "key" => "Hold Veinmine Key",
            _ => "Always On (Sneak disables)",
        };
        let max_blocks = setting_u32(game, mod_id, "max_blocks", 64);
        let match_variants = setting_bool(game, mod_id, "match_variants", true);
        buttons.push(format!("Activation mode: {mode_lbl}"));
        buttons.push(format!("Max blocks per vein: {max_blocks}"));
        buttons.push(format!(
            "Match deepslate & lit ores: {}",
            on_off(match_variants)
        ));
        return;
    }

    if m.features.contains("treefall") {
        let enabled = setting_bool(game, mod_id, "enabled", true);
        let require_axe = setting_bool(game, mod_id, "require_axe", false);
        let max_logs = setting_u32(game, mod_id, "max_logs", 128);
        let break_leaves = setting_bool(game, mod_id, "break_leaves", true);
        buttons.push(format!("Tree felling: {}", on_off(enabled)));
        buttons.push(format!("Require axe in hand: {}", on_off(require_axe)));
        buttons.push(format!("Max logs per tree: {max_logs}"));
        buttons.push(format!("Decay & harvest leaves: {}", on_off(break_leaves)));
        return;
    }

    if m.features.contains("appleskin_hud") {
        let tooltip = setting_bool(game, mod_id, "tooltip_food", true);
        let sat_hud = setting_bool(game, mod_id, "saturation_hud", true);
        let held_preview = setting_bool(game, mod_id, "held_food_preview", true);
        buttons.push(format!("Food values in tooltip: {}", on_off(tooltip)));
        buttons.push(format!("Saturation & hunger HUD: {}", on_off(sat_hud)));
        buttons.push(format!(
            "Held food restoration preview: {}",
            on_off(held_preview)
        ));
        return;
    }

    if m.features.contains("jade_hud") {
        let show_hud = setting_bool(game, mod_id, "show_hud", true);
        let show_states = setting_bool(game, mod_id, "show_states", true);
        let show_rim = setting_bool(game, mod_id, "show_rim", true);
        let reach = setting_u32(game, mod_id, "reach", 6);
        buttons.push(format!("Target info HUD: {}", on_off(show_hud)));
        buttons.push(format!("Show block state properties: {}", on_off(show_states)));
        buttons.push(format!("Highlight targeted block outline: {}", on_off(show_rim)));
        buttons.push(format!("Inspection reach: {reach} blocks"));
        return;
    }

    if m.features.contains("fullbright") {
        let enabled = setting_bool(game, mod_id, "enabled", true);
        let level = setting_u32(game, mod_id, "gamma_pct", 1000);
        let cave_light = setting_bool(game, mod_id, "cave_light", true);
        buttons.push(format!("Fullbright: {}", on_off(enabled)));
        buttons.push(format!("Brightness level: {level}%"));
        buttons.push(format!("Dynamic cave illumination: {}", on_off(cave_light)));
        return;
    }

    if m.features.contains("rightclickharvest") {
        let enabled = setting_bool(game, mod_id, "enabled", true);
        let radius = setting_u32(game, mod_id, "radius", 1);
        let rad_lbl = if radius >= 3 { "3x3 Area" } else { "Single Crop (1x1)" };
        buttons.push(format!("Right-click crop harvest: {}", on_off(enabled)));
        buttons.push(format!("Harvest area: {rad_lbl}"));
        return;
    }

    if m.features.contains("carryblock") {
        let enabled = setting_bool(game, mod_id, "enabled", true);
        let containers = setting_bool(game, mod_id, "allow_containers", true);
        buttons.push(format!("Sneak + right-click pickup: {}", on_off(enabled)));
        buttons.push(format!("Allow picking up chests: {}", on_off(containers)));
        return;
    }

    if m.features.contains("fps_overlay") {
        use horizon_client::extras::Corner;
        let corner = setting_corner(game, mod_id, Corner::TopLeft);
        let show_low = setting_bool(game, mod_id, "show_1pct_low", true);
        let backdrop = setting_bool(game, mod_id, "backdrop", true);
        buttons.push(format!("Corner: {}", corner_label(corner)));
        buttons.push(format!("Show 1% low & min FPS: {}", on_off(show_low)));
        buttons.push(format!("Background panel: {}", on_off(backdrop)));
        return;
    }

    if m.features.contains("keystrokes") {
        use horizon_client::extras::Corner;
        let corner = setting_corner(game, mod_id, Corner::TopRight);
        let show_mouse = setting_bool(game, mod_id, "show_mouse", true);
        let show_cps = setting_bool(game, mod_id, "show_cps", true);
        buttons.push(format!("Corner: {}", corner_label(corner)));
        buttons.push(format!("Show LMB / RMB buttons: {}", on_off(show_mouse)));
        buttons.push(format!("Show CPS counter: {}", on_off(show_cps)));
        return;
    }

    if m.features.contains("coords_hud") {
        use horizon_client::extras::Corner;
        let corner = setting_corner(game, mod_id, Corner::TopLeft);
        let show_dir = setting_bool(game, mod_id, "show_dir", true);
        let show_clock = setting_bool(game, mod_id, "show_clock", true);
        buttons.push(format!("Corner: {}", corner_label(corner)));
        buttons.push(format!("Show facing & chunk coords: {}", on_off(show_dir)));
        buttons.push(format!("Show 24h world clock: {}", on_off(show_clock)));
        return;
    }

    if m.features.contains("armor_status") {
        use horizon_client::extras::Corner;
        let corner = setting_corner(game, mod_id, Corner::BottomRight);
        let show_pct = setting_bool(game, mod_id, "show_pct", true);
        let show_held = setting_bool(game, mod_id, "show_held", true);
        buttons.push(format!("Corner: {}", corner_label(corner)));
        buttons.push(format!("Show durability percentage: {}", on_off(show_pct)));
        buttons.push(format!("Show held item durability: {}", on_off(show_held)));
        return;
    }

    if m.features.contains("enchantmenthints") || m.features.contains("shulkerpeek") {
        let ench = setting_bool(game, mod_id, "show_ench_hints", true);
        let shulker = setting_bool(game, mod_id, "show_container_items", true);
        buttons.push(format!("Enchantment descriptions: {}", on_off(ench)));
        buttons.push(format!("Container contents preview: {}", on_off(shulker)));
        return;
    }

    if m.features.contains("toggle_sprint") {
        let sprint_on = setting_bool(game, mod_id, "sprint_on", true);
        let show_hud = setting_bool(game, mod_id, "show_hud", true);
        buttons.push(format!(
            "Sprint mode: {}",
            if sprint_on { "Toggled ON" } else { "Vanilla" }
        ));
        buttons.push(format!("Show sprint indicator on HUD: {}", on_off(show_hud)));
        return;
    }

    if m.features.contains("waypoints") {
        let show_hud = setting_bool(game, mod_id, "show_hud", true);
        let wp = game
            .native_setting(mod_id, "last_waypoint")
            .unwrap_or_else(|| "0,64,0".to_owned());
        buttons.push(format!("Show waypoint distance on HUD: {}", on_off(show_hud)));
        buttons.push(format!("Set waypoint to current position ({wp})"));
        return;
    }

    if m.features.contains("immersive_thunder") {
        let enabled = setting_bool(game, mod_id, "enabled", true);
        let rumble = setting_u32(game, mod_id, "rumble_pct", 100);
        buttons.push(format!("Atmospheric storm acoustics: {}", on_off(enabled)));
        buttons.push(format!("Thunder camera shake: {rumble}%"));
        return;
    }

    if m.features.contains("eating_animation") {
        let enabled = setting_bool(game, mod_id, "enabled", true);
        let particles = setting_bool(game, mod_id, "particles", true);
        let dynamic_bites = setting_bool(game, mod_id, "dynamic_bites", true);
        buttons.push(format!("Eating & Food Animations: {}", on_off(enabled)));
        buttons.push(format!("Food Crumbs & Particles: {}", on_off(particles)));
        buttons.push(format!("Dynamic Bite Chunks: {}", on_off(dynamic_bites)));
        return;
    }

    if m.features.contains("motion_blur") {
        let enabled = setting_bool(game, mod_id, "enabled", true);
        let pct = setting_u32(game, mod_id, "intensity", 50);
        buttons.push(format!("Motion Blur: {}", on_off(enabled)));
        buttons.push(format!("Blur Intensity: {pct}%"));
        return;
    }

    if !m.custom_buttons.is_empty() {
        for (idx, lbl) in m.custom_buttons.iter().enumerate() {
            let key = format!("opt_{idx}");
            let val = setting_bool(game, mod_id, &key, true);
            buttons.push(format!("{lbl}: {}", on_off(val)));
        }
        return;
    }

    let enabled = setting_bool(game, mod_id, "enabled", true);
    let show_hud = setting_bool(game, mod_id, "show_hud", true);
    buttons.push(format!("{}: {}", m.title, on_off(enabled)));
    buttons.push(format!("Show status on HUD: {}", on_off(show_hud)));
}

pub(super) fn native_settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    let _ = (&game, &mod_id, &button);
    #[cfg(target_os = "android")]
    {
        zoom_click::run(&mut *game, mod_id, button);
        minimap_click::run(&mut *game, mod_id, button);
        recipes_click::run(&mut *game, mod_id, button);
        smartf3_click::run(&mut *game, mod_id, button);
    }

    let features_and_custom = runtime()
        .lock()
        .ok()
        .and_then(|g| g.mods.get(mod_id).map(|m| (m.features.clone(), m.custom_buttons.clone())));
    let Some((features, custom_buttons)) = features_and_custom else {
        return;
    };

    if features.contains("wrench") {
        match button {
            0 => {
                let item = wr_wrench_item(game.native_setting(mod_id, "item").as_deref());
                game.native_set_setting(mod_id, "item", wr_next_item(&item));
            }
            1 => {
                let apart = game
                    .native_setting(mod_id, "take-apart")
                    .as_deref()
                    .map(str::trim)
                    != Some("off");
                game.native_set_setting(mod_id, "take-apart", if apart { "off" } else { "on" });
            }
            _ => {}
        }
        return;
    }

    if features.contains("veinminer") {
        match button {
            0 => {
                let cur = game
                    .native_setting(mod_id, "mode")
                    .unwrap_or_else(|| "always".to_owned());
                let next = match cur.as_str() {
                    "always" => "sneak",
                    "sneak" => "key",
                    _ => "always",
                };
                game.native_set_setting(mod_id, "mode", next);
            }
            1 => {
                cycle_u32(game, mod_id, "max_blocks", &[16, 32, 64, 128], 64);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "match_variants", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("treefall") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "enabled", true);
            }
            1 => {
                toggle_setting_bool(game, mod_id, "require_axe", false);
            }
            2 => {
                cycle_u32(game, mod_id, "max_logs", &[32, 64, 128, 256], 128);
            }
            3 => {
                toggle_setting_bool(game, mod_id, "break_leaves", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("appleskin_hud") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "tooltip_food", true);
            }
            1 => {
                toggle_setting_bool(game, mod_id, "saturation_hud", true);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "held_food_preview", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("jade_hud") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "show_hud", true);
            }
            1 => {
                toggle_setting_bool(game, mod_id, "show_states", true);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "show_rim", true);
            }
            3 => {
                cycle_u32(game, mod_id, "reach", &[5, 6, 8, 12], 6);
            }
            _ => {}
        }
        return;
    }

    if features.contains("fullbright") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "enabled", true);
            }
            1 => {
                cycle_u32(game, mod_id, "gamma_pct", &[200, 500, 1000], 1000);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "cave_light", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("rightclickharvest") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "enabled", true);
            }
            1 => {
                cycle_u32(game, mod_id, "radius", &[1, 3], 1);
            }
            _ => {}
        }
        return;
    }

    if features.contains("carryblock") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "enabled", true);
            }
            1 => {
                toggle_setting_bool(game, mod_id, "allow_containers", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("fps_overlay") {
        use horizon_client::extras::Corner;
        match button {
            0 => cycle_corner(game, mod_id, Corner::TopLeft),
            1 => {
                toggle_setting_bool(game, mod_id, "show_1pct_low", true);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "backdrop", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("keystrokes") {
        use horizon_client::extras::Corner;
        match button {
            0 => cycle_corner(game, mod_id, Corner::TopRight),
            1 => {
                toggle_setting_bool(game, mod_id, "show_mouse", true);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "show_cps", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("coords_hud") {
        use horizon_client::extras::Corner;
        match button {
            0 => cycle_corner(game, mod_id, Corner::TopLeft),
            1 => {
                toggle_setting_bool(game, mod_id, "show_dir", true);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "show_clock", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("armor_status") {
        use horizon_client::extras::Corner;
        match button {
            0 => cycle_corner(game, mod_id, Corner::BottomRight),
            1 => {
                toggle_setting_bool(game, mod_id, "show_pct", true);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "show_held", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("enchantmenthints") || features.contains("shulkerpeek") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "show_ench_hints", true);
            }
            1 => {
                toggle_setting_bool(game, mod_id, "show_container_items", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("toggle_sprint") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "sprint_on", true);
            }
            1 => {
                toggle_setting_bool(game, mod_id, "show_hud", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("waypoints") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "show_hud", true);
            }
            1 => {
                if let Some(p) = game.native_player() {
                    game.native_set_setting(
                        mod_id,
                        "last_waypoint",
                        &format!("{:.0},{:.0},{:.0}", p.x, p.y, p.z),
                    );
                }
            }
            _ => {}
        }
        return;
    }

    if features.contains("immersive_thunder") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "enabled", true);
            }
            1 => {
                cycle_u32(game, mod_id, "rumble_pct", &[50, 100, 150], 100);
            }
            _ => {}
        }
        return;
    }

    if features.contains("eating_animation") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "enabled", true);
            }
            1 => {
                toggle_setting_bool(game, mod_id, "particles", true);
            }
            2 => {
                toggle_setting_bool(game, mod_id, "dynamic_bites", true);
            }
            _ => {}
        }
        return;
    }

    if features.contains("motion_blur") {
        match button {
            0 => {
                toggle_setting_bool(game, mod_id, "enabled", true);
            }
            1 => {
                cycle_u32(game, mod_id, "intensity", &[25, 50, 75, 100], 50);
            }
            _ => {}
        }
        return;
    }

    if !custom_buttons.is_empty() {
        let key = format!("opt_{button}");
        toggle_setting_bool(game, mod_id, &key, true);
        return;
    }

    match button {
        0 => {
            toggle_setting_bool(game, mod_id, "enabled", true);
        }
        1 => {
            toggle_setting_bool(game, mod_id, "show_hud", true);
        }
        _ => {}
    }
}

pub(super) fn native_minimap(game: &Game<'_>, minimap: &mut Option<NativeMinimap>) {
    let _ = (&game, &minimap);
    #[cfg(target_os = "android")]
    {
        minimap_minimap::run(game, &mut *minimap);
    }
}

pub(super) fn native_frame(game: &mut Game<'_>, elapsed: Duration) {
    let _ = (&game, &elapsed);
    #[cfg(target_os = "android")]
    {
        recipes_frame::run(&mut *game);
    }
}

pub(super) fn native_panel_event(game: &mut Game<'_>, mod_id: &str, event: &NativePanelEvent) {
    let _ = (&game, &mod_id, &event);
    #[cfg(target_os = "android")]
    {
        recipes_event::run(&mut *game, mod_id, event);
    }
}

pub(super) fn native_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let _ = (&game, &command, &ran);
    #[cfg(target_os = "android")]
    {
        smartf3_command::run(&mut *game, command, &mut *ran);
    }
    if *ran {
        return;
    }

    ensure_dynamic_mods_loaded(game, false);
    let trimmed = command.trim();
    let mut chat_out: Option<String> = None;

    if trimmed.eq_ignore_ascii_case("hnmods") || trimmed.eq_ignore_ascii_case("mods") {
        if let Ok(guard) = runtime().lock() {
            let active: Vec<String> = guard
                .mods
                .values()
                .filter(|m| game.native_on(&m.id))
                .map(|m| m.title.clone())
                .collect();
            if active.is_empty() {
                chat_out = Some("§d[Horizon Native Mods]§r No custom .hnmod bundles active.".into());
            } else {
                chat_out = Some(format!(
                    "§d[Horizon Native Mods]§r {} active: §a{}",
                    active.len(),
                    active.join("§7, §a")
                ));
            }
            *ran = true;
        }
    } else if let Ok(guard) = runtime().lock() {
        for m in guard.mods.values() {
            if !game.native_on(&m.id) {
                continue;
            }
            if m.commands.iter().any(|c| c.eq_ignore_ascii_case(trimmed))
                || m.id.eq_ignore_ascii_case(trimmed)
            {
                let feats: Vec<&str> = m.features.iter().map(String::as_str).collect();
                chat_out = Some(format!(
                    "§a[{}]§r Active (features: {}, items: {}, blocks: {})",
                    m.title,
                    feats.join(", "),
                    m.items.len(),
                    m.blocks.len()
                ));
                *ran = true;
                break;
            }
        }
    }

    if let Some(msg) = chat_out {
        game.native_chat(&msg);
    }
}

pub(super) fn native_tick(game: &mut Game<'_>) {
    ensure_dynamic_mods_loaded(game, false);
    let mut play_thunder: Option<([f64; 3], f32, f32, i32)> = None;
    let mut want_fullbright: Option<f32> = None;
    let mut want_toggle_sprint = false;

    if let Ok(mut guard) = runtime().lock() {
        if guard.thunder_rumble_ticks > 0 {
            guard.thunder_rumble_ticks -= 1;
        }
        for m in guard.mods.values() {
            if !game.native_on(&m.id) {
                continue;
            }
            if m.features.contains("fullbright") && setting_bool(game, &m.id, "enabled", true) {
                let pct = setting_u32(game, &m.id, "gamma_pct", 1000);
                want_fullbright = Some((pct as f32 / 100.0).clamp(2.0, 16.0));
            }
            if m.features.contains("toggle_sprint")
                && setting_bool(game, &m.id, "sprint_on", true)
            {
                want_toggle_sprint = true;
            }
        }

        if game.native_window() {
            if let Some(target_gamma) = want_fullbright {
                if !guard.fullbright_was_active {
                    guard.saved_gamma = Some(game.settings.gamma);
                    guard.fullbright_was_active = true;
                }
                game.settings.gamma = target_gamma;
            } else if guard.fullbright_was_active {
                if let Some(prev) = guard.saved_gamma.take() {
                    game.settings.gamma = prev;
                }
                guard.fullbright_was_active = false;
            }
        }

        let has_eat_anim = guard.mods.values().any(|m| {
            game.native_on(&m.id)
                && (m.features.contains("eating_animation") || m.features.contains("eatinganimation"))
                && setting_bool(game, &m.id, "enabled", true)
        });
        if has_eat_anim && game.using.is_some() {
            if let Some(held) = game.mod_held_item_id() {
                if estimate_food_values(&held).is_some() && game.ticks % 4 == 0 {
                    if let Some(p) = game.native_player() {
                        let yaw_rad = p.yaw.to_radians();
                        let at = [
                            p.x - yaw_rad.sin() as f64 * 0.35,
                            p.y + 1.5,
                            p.z + yaw_rad.cos() as f64 * 0.35,
                        ];
                        game.particles.add(
                            &horizon_client::particles::ParticleOptions::simple(
                                horizon_client::particles::ParticleType::BlockCrumble,
                            ),
                            at,
                            [0.0, 0.02, 0.0],
                        );
                    }
                }
            }
        }

        let has_thunder_mod = guard.mods.values().any(|m| {
            game.native_on(&m.id)
                && m.features.contains("immersive_thunder")
                && setting_bool(game, &m.id, "enabled", true)
        });
        if has_thunder_mod && game.native_window() {
            let (rain, thunder) = game.native_weather();
            let game_time = game.world.game_time;
            if (thunder > 0.15 || rain > 0.65)
                && game_time - guard.last_thunder_sound_tick >= 160
                && game_time % 180 == 0
            {
                if let Some(p) = game.native_player() {
                    guard.last_thunder_sound_tick = game_time;
                    guard.thunder_rumble_ticks = if thunder > 0.15 { 36 } else { 18 };
                    let vol = if thunder > 0.15 { 3.5 } else { 1.6 };
                    let pitch = if (game_time / 180) % 2 == 0 { 0.62 } else { 0.78 };
                    play_thunder = Some(([p.x + 12.0, p.y + 18.0, p.z - 10.0], vol, pitch, 4));
                }
            }
        }
    }

    if want_toggle_sprint && game.native_playing() {
        let forward_down = game
            .native_mapping("key.forward")
            .is_some_and(|(down, _)| down);
        if forward_down {
            if let Some(p) = game.player.as_mut() {
                if !p.shift_key_down {
                    p.sprinting = true;
                }
            }
        }
    }

    if let Some((at, vol, pitch, delay)) = play_thunder {
        let _ = game.native_play_local_sound(
            "minecraft:entity.lightning_bolt.thunder",
            at,
            vol,
            pitch,
            delay,
        );
    }
}

pub(super) fn native_camera_shake(game: &Game<'_>, partial: f32, shake: &mut NativeShake) {
    let Ok(guard) = runtime().lock() else {
        return;
    };

    // Motion blur / Camera smoothing simulation
    if let Some(mb_mod) = guard.mods.values().find(|m| {
        game.native_on(&m.id)
            && m.features.contains("motion_blur")
            && setting_bool(game, &m.id, "enabled", true)
    }) {
        let pct = setting_u32(game, &mb_mod.id, "intensity", 50);
        let mult = (pct as f32 / 100.0).clamp(0.1, 1.0);
        if game.player.is_some() {
            let dyaw = game.yaw - game.rope_rot_old.0;
            let dpitch = game.pitch - game.rope_rot_old.1;
            shake.yaw += dyaw * 0.08 * mult;
            shake.pitch += dpitch * 0.08 * mult;
            shake.roll += (dyaw * 0.04 * mult).clamp(-0.5, 0.5);
        }
    }

    if guard.thunder_rumble_ticks == 0 {
        return;
    }
    let Some(thunder_mod) = guard.mods.values().find(|m| {
        game.native_on(&m.id)
            && m.features.contains("immersive_thunder")
            && setting_bool(game, &m.id, "enabled", true)
    }) else {
        return;
    };
    let pct = setting_u32(game, &thunder_mod.id, "rumble_pct", 100);
    let mult = (pct as f32 / 100.0).clamp(0.25, 2.0);
    let decay = (guard.thunder_rumble_ticks as f32 / 36.0).clamp(0.0, 1.0) * mult;
    let phase = (game.world.game_time as f32 + partial) * 1.9;
    shake.pitch += phase.sin() * 0.65 * decay;
    shake.yaw += (phase * 1.37).cos() * 0.55 * decay;
    shake.roll += (phase * 2.1).sin() * 0.45 * decay;
}

pub(super) fn native_hud(
    game: &mut Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    ensure_dynamic_mods_loaded(game, false);
    use horizon_client::extras::Corner;

    let [screen_w, screen_h] = [batch.width, batch.height];
    let held_stack = game.inventory.get(game.selected).and_then(Option::as_ref).cloned();
    let held_item = held_stack.as_ref().map(|s| s.item.to_string());
    let (fps, min_fps, low_1_pct) = game.native_frame_rate();
    let (lmb_cps, rmb_cps) = game.native_clicks();
    let w_key = game.native_mapping("key.forward").map(|(d, _)| d).unwrap_or(false);
    let a_key = game.native_mapping("key.left").map(|(d, _)| d).unwrap_or(false);
    let s_key = game.native_mapping("key.back").map(|(d, _)| d).unwrap_or(false);
    let d_key = game.native_mapping("key.right").map(|(d, _)| d).unwrap_or(false);
    let jump_key = game.native_mapping("key.jump").map(|(d, _)| d).unwrap_or(false);

    let active_mods: Vec<DynamicHnmod> = runtime()
        .lock()
        .ok()
        .map(|g| {
            g.mods
                .values()
                .filter(|m| game.native_on(&m.id))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let carried_name = runtime()
        .lock()
        .ok()
        .and_then(|g| g.carried_block_name.clone());
    if let Some(cname) = carried_name {
        let text = format!("Carrying: {} (Right-click to place)", wr_short(&cname).replace('_', " "));
        let tw = game.native_text_width(&text) + 12.0;
        let bx = (screen_w - tw) * 0.5;
        let by = screen_h - 58.0;
        game.native_fill(batch, [bx, by, tw, 15.0], 0x99181524);
        game.native_text(batch, &text, [bx + 6.0, by + 3.5], 0xFFE082, true);
    }

    for m in active_mods {
        // 1. LookAt / Jade / BlockScope top-center block inspector card
        if m.features.contains("jade_hud") && setting_bool(game, &m.id, "show_hud", true) {
            let reach = setting_u32(game, &m.id, "reach", 6) as f64;
            if let Some((eye, dir)) = game.aim() {
                if let Some(hit) = game.clip_ray(eye, dir, reach) {
                    let block_name = game.native_block(hit.pos);
                    if block_name != "minecraft:air" {
                        let (ns, raw_id) = block_name
                            .split_once(':')
                            .unwrap_or(("minecraft", &block_name));
                        let pretty = horizon_client::jar_transpiler::title_case_id(raw_id);
                        let mut prop_parts = Vec::new();
                        if setting_bool(game, &m.id, "show_states", true) {
                            for prop_key in ["facing", "axis", "age", "lit", "power", "level", "half", "open"] {
                                if let Some(val) = game.native_block_property(hit.pos, prop_key) {
                                    prop_parts.push(format!("{prop_key}: {val}"));
                                }
                            }
                        }
                        let line1 = format!(
                            "{pretty} ({}, {}, {})",
                            hit.pos[0], hit.pos[1], hit.pos[2]
                        );
                        let line2 = if prop_parts.is_empty() {
                            horizon_client::jar_transpiler::title_case_id(ns)
                        } else {
                            format!(
                                "{} • {}",
                                horizon_client::jar_transpiler::title_case_id(ns),
                                prop_parts.join(" | ")
                            )
                        };
                        let w = game
                            .native_text_width(&line1)
                            .max(game.native_text_width(&line2))
                            + 16.0;
                        let h = 28.0;
                        let x = (screen_w - w) * 0.5;
                        let y = 6.0;
                        game.native_fill(batch, [x, y, w, h], 0xAA12101B);
                        game.native_fill(batch, [x, y, w, 1.5], 0xFF80CBC4);
                        game.native_text(batch, &line1, [x + 8.0, y + 4.0], 0xFFFFFF, true);
                        game.native_text(batch, &line2, [x + 8.0, y + 16.0], 0x80CBC4, true);
                    }
                }
            }
            continue;
        }

        // 2. AppleSkin Saturation & Held Food Restoration HUD
        if m.features.contains("appleskin_hud") && setting_bool(game, &m.id, "saturation_hud", true) {
            if let Some(p) = &game.player {
                let food = p.health.food.food.max(0);
                let sat = p.health.food.saturation.max(0.0);
                let preview = if setting_bool(game, &m.id, "held_food_preview", true) {
                    held_item
                        .as_deref()
                        .and_then(estimate_food_values)
                        .map(|(h, s)| format!("  +{h} Food / +{s:.1} Sat"))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                // Draw compact AppleSkin bar right above the hunger bar on the HUD
                let bar_text = format!("Hunger {food}/20 • Sat {sat:.1}{preview}");
                let tw = game.native_text_width(&bar_text) + 10.0;
                let hx = (screen_w * 0.5 + 10.0).min(screen_w - tw - 4.0);
                let hy = (screen_h - 49.0).max(20.0);
                game.native_fill(batch, [hx, hy, tw, 13.0], 0x8014121E);
                // Golden saturation progress underline
                let sat_ratio = (sat / 20.0).clamp(0.0, 1.0);
                game.native_fill(batch, [hx, hy + 11.5, tw * sat_ratio, 1.5], 0xFFFFD54F);
                game.native_text(batch, &bar_text, [hx + 5.0, hy + 2.5], 0xFFD54F, true);
            }
            continue;
        }

        // 3. FPS Counter
        if m.features.contains("fps_overlay") {
            let corner = setting_corner(game, &m.id, Corner::TopLeft);
            if !game.native_corner_shown(corner) {
                continue;
            }
            let show_low = setting_bool(game, &m.id, "show_1pct_low", true);
            let label = if show_low {
                format!("{fps} FPS (1% low: {low_1_pct}, min: {min_fps})")
            } else {
                format!("{fps} FPS")
            };
            let w = game.native_text_width(&label) + 10.0;
            let (x, y) = corners.place(corner, w, 14.0);
            if setting_bool(game, &m.id, "backdrop", true) {
                game.native_fill(batch, [x, y, w, 14.0], 0x7214121E);
            }
            game.native_text(batch, &label, [x + 5.0, y + 3.0], 0xA5D6A7, true);
            continue;
        }

        // 4. Keystrokes
        if m.features.contains("keystrokes") {
            let corner = setting_corner(game, &m.id, Corner::TopRight);
            if !game.native_corner_shown(corner) {
                continue;
            }
            let keys_str = format!(
                "[{}] [{}] [{}] [{}]{}",
                if w_key { "W" } else { "·" },
                if a_key { "A" } else { "·" },
                if s_key { "S" } else { "·" },
                if d_key { "D" } else { "·" },
                if jump_key { " [JUMP]" } else { "" }
            );
            let cps_str = if setting_bool(game, &m.id, "show_cps", true) {
                format!(" • LMB: {lmb_cps} CPS | RMB: {rmb_cps} CPS")
            } else {
                String::new()
            };
            let label = format!("{keys_str}{cps_str}");
            let w = game.native_text_width(&label) + 10.0;
            let (x, y) = corners.place(corner, w, 14.0);
            game.native_fill(batch, [x, y, w, 14.0], 0x7814121E);
            game.native_text(batch, &label, [x + 5.0, y + 3.0], 0x90CAF9, true);
            continue;
        }

        // 5. Coordinates HUD
        if m.features.contains("coords_hud") {
            let corner = setting_corner(game, &m.id, Corner::TopLeft);
            if !game.native_corner_shown(corner) {
                continue;
            }
            if let Some(p) = game.native_player() {
                let dir_str = if setting_bool(game, &m.id, "show_dir", true) {
                    let yaw = p.yaw.rem_euclid(360.0);
                    let card = if !(45.0..315.0).contains(&yaw) {
                        "S (+Z)"
                    } else if yaw < 135.0 {
                        "W (-X)"
                    } else if yaw < 225.0 {
                        "N (-Z)"
                    } else {
                        "E (+X)"
                    };
                    format!(" [{card}]")
                } else {
                    String::new()
                };
                let clock_str = if setting_bool(game, &m.id, "show_clock", true) {
                    if let Some(c) = game.native_clock() {
                        let ticks = (c.day_time + 6000).rem_euclid(24000);
                        let hh = ticks / 1000;
                        let mm = (ticks % 1000) * 60 / 1000;
                        format!(" • {hh:02}:{mm:02}")
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                let label = format!(
                    "XYZ: {:.0}, {:.0}, {:.0}{}{}",
                    p.x, p.y, p.z, dir_str, clock_str
                );
                let w = game.native_text_width(&label) + 10.0;
                let (x, y) = corners.place(corner, w, 14.0);
                game.native_fill(batch, [x, y, w, 14.0], 0x7214121E);
                game.native_text(batch, &label, [x + 5.0, y + 3.0], 0xFFF59D, true);
            }
            continue;
        }

        // 6. Armor Status / Held Item Info
        if m.features.contains("armor_status") {
            let corner = setting_corner(game, &m.id, Corner::BottomRight);
            if !game.native_corner_shown(corner) {
                continue;
            }
            let mut pieces = Vec::new();
            for st in game.armor.iter().flatten() {
                let short = wr_short(&st.item).replace('_', " ");
                pieces.push(horizon_client::jar_transpiler::title_case_id(&short));
            }
            if setting_bool(game, &m.id, "show_held", true) {
                if let Some(st) = &held_stack {
                    let short = wr_short(&st.item).replace('_', " ");
                    pieces.push(format!(
                        "Hand: {} x{}",
                        horizon_client::jar_transpiler::title_case_id(&short),
                        st.count
                    ));
                }
            }
            if !pieces.is_empty() {
                let label = format!("Armor: {}", pieces.join(" | "));
                let w = game.native_text_width(&label) + 10.0;
                let (x, y) = corners.place(corner, w, 14.0);
                game.native_fill(batch, [x, y, w, 14.0], 0x7214121E);
                game.native_text(batch, &label, [x + 5.0, y + 3.0], 0xB39DDB, true);
            }
            continue;
        }

        // 7. Toggle Sprint HUD
        if m.features.contains("toggle_sprint")
            && setting_bool(game, &m.id, "sprint_on", true)
            && setting_bool(game, &m.id, "show_hud", true)
            && game.native_corner_shown(Corner::BottomLeft)
        {
            let label = "[Sprint (Toggled)]";
            let w = game.native_text_width(label) + 10.0;
            let (x, y) = corners.place(Corner::BottomLeft, w, 14.0);
            game.native_fill(batch, [x, y, w, 14.0], 0x7214121E);
            game.native_text(batch, label, [x + 5.0, y + 3.0], 0x80DEEA, true);
            continue;
        }

        // 8. Waypoints HUD
        if m.features.contains("waypoints")
            && setting_bool(game, &m.id, "show_hud", true)
            && game.native_corner_shown(Corner::TopRight)
        {
            if let Some(p) = game.native_player() {
                let wp = game
                    .native_setting(&m.id, "last_waypoint")
                    .unwrap_or_else(|| "0,64,0".to_owned());
                let coords: Vec<f64> = wp
                    .split(',')
                    .filter_map(|s| s.trim().parse::<f64>().ok())
                    .collect();
                if coords.len() == 3 {
                    let dist = ((p.x - coords[0]).powi(2)
                        + (p.y - coords[1]).powi(2)
                        + (p.z - coords[2]).powi(2))
                    .sqrt();
                    let label = format!(
                        "Waypoint ({:.0}, {:.0}, {:.0}) • {:.0}m",
                        coords[0], coords[1], coords[2], dist
                    );
                    let w = game.native_text_width(&label) + 10.0;
                    let (x, y) = corners.place(Corner::TopRight, w, 14.0);
                    game.native_fill(batch, [x, y, w, 14.0], 0x7214121E);
                    game.native_text(batch, &label, [x + 5.0, y + 3.0], 0xCE93D8, true);
                }
            }
            continue;
        }
    }
}

pub(super) fn native_tooltip(
    game: &Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    ensure_dynamic_mods_loaded(game, false);
    let Ok(guard) = runtime().lock() else {
        return;
    };

    // 1. AppleSkin Food & Saturation Tooltip
    let has_appleskin = guard.mods.values().any(|m| {
        game.native_on(&m.id)
            && m.features.contains("appleskin_hud")
            && setting_bool(game, &m.id, "tooltip_food", true)
    });
    if has_appleskin {
        if let Some((hunger, sat)) = estimate_food_values(&stack.item) {
            let shanks = hunger as f32 * 0.5;
            lines.push((
                horizon_client::text::Text::literal(format!(
                    "§6✦ Hunger: +{hunger} ({shanks:.1} shanks)  §e★ Saturation: +{sat:.1}"
                )),
                0xFFD54F,
            ));
        }
    }

    // 2. Enchantment Hints
    let has_ench_hints = guard.mods.values().any(|m| {
        game.native_on(&m.id)
            && m.features.contains("enchantmenthints")
            && setting_bool(game, &m.id, "show_ench_hints", true)
    });
    if has_ench_hints && !stack.enchantments().is_empty() {
        for (ench_id, _) in stack.enchantments() {
            if let Some(desc) = enchantment_description(ench_id) {
                lines.push((
                    horizon_client::text::Text::literal(format!("§7  ↳ {desc}")),
                    0xB0BEC5,
                ));
            }
        }
    }

    // 3. ShulkerPeek / Container Tooltip
    let has_shulker = guard.mods.values().any(|m| {
        game.native_on(&m.id)
            && m.features.contains("shulkerpeek")
            && setting_bool(game, &m.id, "show_container_items", true)
    });
    let container_items = stack.extra.as_ref().map(|d| d.container.as_slice());
    if has_shulker && let Some(container) = container_items && !container.is_empty() {
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for (_, st) in container {
            if st.count > 0 && &*st.item != "minecraft:air" {
                let pretty = horizon_client::jar_transpiler::title_case_id(wr_short(&st.item));
                *counts.entry(pretty).or_default() += st.count;
            }
        }
        if !counts.is_empty() {
            let summary: Vec<String> = counts
                .into_iter()
                .take(6)
                .map(|(n, c)| format!("{n} x{c}"))
                .collect();
            lines.push((
                horizon_client::text::Text::literal(format!(
                    "§b📦 Contents: §f{}",
                    summary.join(", ")
                )),
                0x80DEEA,
            ));
        }
    }

    // 4. Custom Mod Namespace Badge
    if let Some((ns, _)) = stack.item.split_once(':') {
        if ns != "minecraft" {
            if let Some(m) = guard.mods.get(ns) {
                if game.native_on(&m.id) {
                    lines.push((
                        horizon_client::text::Text::literal(format!("§9{}", m.title)),
                        0x90CAF9,
                    ));
                }
            }
        }
    }
}

/// Server-side block right-click hook (runs before vanilla block interaction like opening a furnace/chest).
pub(super) fn native_right_click_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    if *canceled || game.remote.is_some() || game.spectator() {
        return;
    }
    ensure_dynamic_mods_loaded(game, false);

    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
        || game.player.as_ref().is_some_and(|p| p.shift_key_down);
    let held_item = game
        .inventory
        .get(game.selected)
        .and_then(Option::as_ref)
        .map(|s| s.item.clone());

    let (wrench_mod, harvest_mod, carry_mod) = {
        let Ok(guard) = runtime().lock() else {
            return;
        };
        let wr = guard
            .mods
            .values()
            .find(|m| game.native_on(&m.id) && m.features.contains("wrench"))
            .map(|m| m.id.clone());
        let hv = guard
            .mods
            .values()
            .find(|m| {
                game.native_on(&m.id)
                    && m.features.contains("rightclickharvest")
                    && setting_bool(game, &m.id, "enabled", true)
            })
            .map(|m| m.id.clone());
        let cb = guard
            .mods
            .values()
            .find(|m| {
                game.native_on(&m.id)
                    && m.features.contains("carryblock")
                    && setting_bool(game, &m.id, "enabled", true)
            })
            .map(|m| m.id.clone());
        (wr, hv, cb)
    };

    // 1. Official Wrench rotation & sneak machine take-apart
    if let (Some(wr_id), Some(held)) = (wrench_mod, held_item.as_deref()) {
        let configured_wrench = wr_wrench_item(game.native_setting(&wr_id, "item").as_deref());
        if held == configured_wrench || held.contains("wrench") {
            if sneaking {
                let take_apart_on = game
                    .native_setting(&wr_id, "take-apart")
                    .as_deref()
                    .map(str::trim)
                    != Some("off");
                if take_apart_on && game_wrench_take_apart(game, pos) {
                    game.swing_arm();
                    *canceled = true;
                    return;
                }
            } else if game_wrench_turn(game, pos, face) {
                game.swing_arm();
                *canceled = true;
                return;
            }
        }
    }

    // 2. RightClickHarvest mature crop harvesting + auto-replanting
    if let Some(hv_id) = harvest_mod {
        let radius = if setting_u32(game, &hv_id, "radius", 1) >= 3 {
            1
        } else {
            0
        };
        let mut any_harvested = false;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let cp = [pos[0] + dx, pos[1], pos[2] + dz];
                if try_right_click_harvest_crop(game, cp) {
                    any_harvested = true;
                }
            }
        }
        if any_harvested {
            let _ = game.process_level_events();
            game.swing_arm();
            *canceled = true;
            return;
        }
    }

    // 3. CarryBlock pickup & placement
    if let Some(cb_id) = carry_mod {
        let carried = runtime().lock().ok().and_then(|g| g.carried_block);
        if let Some(carried_state) = carried {
            let target = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
            let cur_state = game.world.block(target[0], target[1], target[2]);
            if game.world.is_air(cur_state) {
                let at = BlockPos::new(target[0], target[1], target[2]);
                let placed = game.with_level(|b, level, ticks| {
                    b.set_block(level, ticks, at, carried_state, flags::UPDATE_ALL)
                });
                if placed {
                    let _ = game.process_level_events();
                    if let Ok(mut g) = runtime().lock() {
                        g.carried_block = None;
                        g.carried_block_name = None;
                    }
                    game.swing_arm();
                    *canceled = true;
                    return;
                }
            }
        } else if sneaking && held_item.is_none() {
            let state = game.world.block(pos[0], pos[1], pos[2]);
            if let Some(bs) = game.pack.assets.registry.state(state) {
                let name = bs.name.clone();
                let short = wr_short(&name);
                let allow_containers = setting_bool(game, &cb_id, "allow_containers", true);
                let movable = short != "bedrock"
                    && short != "obsidian"
                    && short != "end_portal_frame"
                    && !short.contains("portal")
                    && !short.contains("command_block")
                    && (allow_containers || !matches!(short, "chest" | "trapped_chest" | "barrel" | "furnace" | "hopper"));
                if movable && wr_turnable(&name, &bs.properties.iter().map(|(k, v)| (k.clone(), v.clone())).collect()) {
                    if let Some(air) = game.pack.assets.registry.default_state("minecraft:air") {
                        let at = BlockPos::new(pos[0], pos[1], pos[2]);
                        let removed = game.with_level(|b, level, ticks| {
                            b.set_block(level, ticks, at, air, flags::UPDATE_ALL)
                        });
                        if removed {
                            let _ = game.process_level_events();
                            if let Ok(mut g) = runtime().lock() {
                                g.carried_block = Some(state);
                                g.carried_block_name = Some(name);
                            }
                            game.swing_arm();
                            *canceled = true;
                        }
                    }
                }
            }
        }
    }
}

fn game_wrench_turn(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3]) -> bool {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let registry = game.pack.assets.registry;
    let Some(now) = registry.state(state) else {
        return false;
    };
    let at = BlockPos::new(pos[0], pos[1], pos[2]);
    let current: BTreeMap<String, String> = now
        .properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for properties in wr_turns(&now.name, &current, face) {
        let candidate = BlockState {
            name: now.name.clone(),
            properties: properties.into(),
        };
        let Some(turned) = registry.id(&candidate) else {
            continue;
        };
        let shaped = game.with_level(|b, level, ticks| {
            b.can_survive(level, ticks, at, turned)
                .then(|| b.update_from_neighbour_shapes(level, ticks, at, turned))
        });
        let Some(shaped) = shaped.filter(|s| *s != state && !game.world.is_air(*s)) else {
            continue;
        };
        let placed = game.with_level(|b, level, ticks| {
            b.set_block(level, ticks, at, shaped, flags::UPDATE_ALL)
        });
        if placed {
            let _ = game.process_level_events();
            return true;
        }
    }
    false
}

fn game_wrench_take_apart(game: &mut Game<'_>, pos: [i32; 3]) -> bool {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let registry = game.pack.assets.registry;
    let Some(name) = registry.state(state).map(|s| s.name.clone()) else {
        return false;
    };
    if !wr_takes_apart(&name) {
        return false;
    }
    let at = BlockPos::new(pos[0], pos[1], pos[2]);
    let drop = !game.creative;
    let broken = game.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, drop));
    let _ = game.process_level_events();
    broken
}

fn try_right_click_harvest_crop(game: &mut Game<'_>, pos: [i32; 3]) -> bool {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let registry = game.pack.assets.registry;
    let Some(now) = registry.state(state).cloned() else {
        return false;
    };
    let short = wr_short(&now.name);
    let max_age = match short {
        "wheat" | "carrots" | "potatoes" | "torchflower_crop" => Some(("7", "0")),
        "beetroots" | "nether_wart" => Some(("3", "0")),
        "cocoa" => Some(("2", "0")),
        "sweet_berry_bush" => Some(("3", "1")),
        _ => None,
    };
    let Some((mature_age, replant_age)) = max_age else {
        return false;
    };
    if now.properties.get("age").map(String::as_str) != Some(mature_age) {
        return false;
    }
    let mut replanted_props = now.properties.clone();
    replanted_props.insert("age".into(), replant_age.into());
    let replanted_candidate = BlockState {
        name: now.name.clone(),
        properties: replanted_props,
    };
    let Some(replanted_state) = registry.id(&replanted_candidate) else {
        return false;
    };
    let at = BlockPos::new(pos[0], pos[1], pos[2]);
    let broken = game.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, true));
    if broken {
        let _ = game.with_level(|b, level, ticks| {
            b.set_block(level, ticks, at, replanted_state, flags::UPDATE_ALL)
        });
        return true;
    }
    false
}

pub(super) fn native_use_item_on(
    game: &mut Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    if *claimed {
        return;
    }
    let _ = stack;
    native_right_click_block(game, pos, face, claimed);
}

pub(super) fn native_use_item(
    game: &mut Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    if *claimed {
        return;
    }
    ensure_dynamic_mods_loaded(game, false);
    let Some((ns, _)) = stack.item.split_once(':') else {
        return;
    };
    if ns == "minecraft" {
        return;
    }
    let active_mod = runtime()
        .lock()
        .ok()
        .and_then(|g| g.mods.get(ns).cloned())
        .filter(|m| game.native_on(&m.id));
    if active_mod.is_some() {
        if let Some(p) = game.native_player() {
            game.native_flame_particle([p.x, p.y + 1.1, p.z], false);
        }
    }
}

fn ore_family(block_name: &str) -> Option<&str> {
    let short = wr_short(block_name);
    if short == "ancient_debris" || short == "glowstone" || short == "gilded_blackstone" {
        return Some(short);
    }
    let stripped = short
        .strip_prefix("deepslate_")
        .or_else(|| short.strip_prefix("nether_"))
        .or_else(|| short.strip_prefix("lit_"))
        .unwrap_or(short);
    if stripped.ends_with("_ore") || stripped == "amethyst_cluster" {
        Some(stripped)
    } else {
        None
    }
}

fn is_tree_log(block_name: &str) -> bool {
    let short = wr_short(block_name);
    (short.ends_with("_log")
        || short.ends_with("_stem")
        || short.ends_with("_wood")
        || short.ends_with("_hyphae")
        || short == "mangrove_roots"
        || short == "bamboo_block")
        && !short.starts_with("stripped_")
}

fn is_tree_leaf(block_name: &str) -> bool {
    let short = wr_short(block_name);
    short.ends_with("_leaves")
        || short.ends_with("_wart_block")
        || short == "shroomlight"
        || short == "mangrove_leaves"
}

pub(super) fn native_destroy_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    state: horizon_world::registry::BlockStateId,
    _canceled: &mut bool,
) {
    if game.remote.is_some() || game.spectator() {
        return;
    }
    ensure_dynamic_mods_loaded(game, false);

    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
        || game.player.as_ref().is_some_and(|p| p.shift_key_down);
    let held_item = game
        .inventory
        .get(game.selected)
        .and_then(Option::as_ref)
        .map(|s| s.item.clone())
        .unwrap_or_default();

    let (vein_cfg, tree_cfg) = {
        let Ok(mut guard) = runtime().lock() else {
            return;
        };
        if guard.vein_guard {
            return;
        }
        let vein = guard
            .mods
            .values()
            .find(|m| game.native_on(&m.id) && m.features.contains("veinminer"))
            .map(|m| (m.id.clone(), m.vein_key_held));
        let tree = guard
            .mods
            .values()
            .find(|m| {
                game.native_on(&m.id)
                    && m.features.contains("treefall")
                    && setting_bool(game, &m.id, "enabled", true)
            })
            .map(|m| m.id.clone());
        if vein.is_none() && tree.is_none() {
            return;
        }
        guard.vein_guard = true;
        (vein, tree)
    };

    let block_name = game.res.facts().get(state).name.to_string();
    let drop_items = !game.creative;

    // 1. Check Veinminer on ore blocks
    if let (Some((vm_id, key_held)), Some(target_family)) = (&vein_cfg, ore_family(&block_name)) {
        let mode = game
            .native_setting(vm_id, "mode")
            .unwrap_or_else(|| "always".to_owned());
        let active = match mode.as_str() {
            "sneak" => sneaking || *key_held,
            "key" => *key_held,
            _ => !sneaking || *key_held,
        };
        if active {
            let max_blocks = setting_u32(game, vm_id, "max_blocks", 64).clamp(4, 192) as usize;
            let match_variants = setting_bool(game, vm_id, "match_variants", true);
            let mut visited = BTreeSet::new();
            let mut queue = VecDeque::new();
            visited.insert(pos);
            queue.push_back(pos);
            let mut to_break = Vec::new();

            while let Some(cur) = queue.pop_front() {
                if to_break.len() >= max_blocks {
                    break;
                }
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if dx == 0 && dy == 0 && dz == 0 {
                                continue;
                            }
                            let np = [cur[0] + dx, cur[1] + dy, cur[2] + dz];
                            if !visited.insert(np) {
                                continue;
                            }
                            let nb_name = game.native_block(np);
                            let matches = if match_variants {
                                ore_family(&nb_name) == Some(target_family)
                            } else {
                                nb_name == block_name
                            };
                            if matches {
                                to_break.push(np);
                                queue.push_back(np);
                                if to_break.len() >= max_blocks {
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if !to_break.is_empty() {
                for np in to_break {
                    let at = BlockPos::new(np[0], np[1], np[2]);
                    let _ = game.with_level(|b, level, ticks| {
                        b.destroy_block(level, ticks, at, drop_items)
                    });
                }
                let _ = game.process_level_events();
            }
        }
    }

    // 2. Check TreeFall / Chop! on tree logs (or Veinminer when configured on logs)
    if is_tree_log(&block_name) {
        let tree_active = if let Some(tf_id) = &tree_cfg {
            let require_axe = setting_bool(game, tf_id, "require_axe", false);
            let has_axe = held_item.ends_with("_axe") || held_item.contains("chainsaw");
            !sneaking && (!require_axe || has_axe)
        } else if let Some((vm_id, key_held)) = &vein_cfg {
            let mode = game
                .native_setting(vm_id, "mode")
                .unwrap_or_else(|| "always".to_owned());
            match mode.as_str() {
                "sneak" => sneaking || *key_held,
                "key" => *key_held,
                _ => !sneaking || *key_held,
            }
        } else {
            false
        };

        if tree_active {
            let max_logs = tree_cfg
                .as_deref()
                .map(|id| setting_u32(game, id, "max_logs", 128))
                .unwrap_or(64)
                .clamp(8, 256) as usize;
            let break_leaves = tree_cfg
                .as_deref()
                .map(|id| setting_bool(game, id, "break_leaves", true))
                .unwrap_or(true);

            let mut visited = BTreeSet::new();
            let mut queue = VecDeque::new();
            visited.insert(pos);
            queue.push_back(pos);
            let mut logs_to_break = Vec::new();
            let mut leaves_to_break = Vec::new();

            while let Some(cur) = queue.pop_front() {
                if logs_to_break.len() >= max_logs {
                    break;
                }
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        for dz in -1..=1 {
                            if dx == 0 && dy == 0 && dz == 0 {
                                continue;
                            }
                            let np = [cur[0] + dx, cur[1] + dy, cur[2] + dz];
                            if !visited.insert(np) {
                                continue;
                            }
                            let nb_name = game.native_block(np);
                            if nb_name == block_name {
                                logs_to_break.push(np);
                                queue.push_back(np);
                                if logs_to_break.len() >= max_logs {
                                    break;
                                }
                            } else if break_leaves
                                && leaves_to_break.len() < 96
                                && is_tree_leaf(&nb_name)
                            {
                                leaves_to_break.push(np);
                            }
                        }
                    }
                }
            }

            if !logs_to_break.is_empty() {
                for np in logs_to_break {
                    let at = BlockPos::new(np[0], np[1], np[2]);
                    let _ = game.with_level(|b, level, ticks| {
                        b.destroy_block(level, ticks, at, drop_items)
                    });
                }
                for lp in leaves_to_break {
                    let at = BlockPos::new(lp[0], lp[1], lp[2]);
                    let _ = game.with_level(|b, level, ticks| {
                        b.destroy_block(level, ticks, at, drop_items)
                    });
                }
                let _ = game.process_level_events();
            }
        }
    }

    if let Ok(mut guard) = runtime().lock() {
        guard.vein_guard = false;
    }
}

pub(super) fn native_visuals(
    game: &Game<'_>,
    rims: &mut Vec<NativeBlockRim>,
    lights: &mut Vec<NativeColoredLight>,
) {
    let Ok(guard) = runtime().lock() else {
        return;
    };

    // 1. Dynamic Lights when holding light-emitting items
    let has_dyn_lights = guard.mods.values().any(|m| {
        game.native_on(&m.id) && m.features.contains("dynamic_lights")
    });
    if has_dyn_lights {
        if let (Some(p), Some(held)) = (game.native_player(), game.mod_held_item_id()) {
            let short = held.strip_prefix("minecraft:").unwrap_or(&held);
            let light_rgb = if short.contains("soul") {
                Some((0x4FC3F7, 11.0))
            } else if short.contains("torch")
                || short.contains("lantern")
                || short.contains("glowstone")
                || short.contains("shroomlight")
                || short.contains("lava")
                || short.contains("blaze")
                || short.contains("campfire")
            {
                Some((0xFFB74D, 13.0))
            } else {
                None
            };
            if let Some((rgb, radius)) = light_rgb {
                lights.push(NativeColoredLight {
                    at: [p.x, p.y + 1.2, p.z],
                    rgb,
                    radius,
                    intensity: 1.0,
                });
            }
        }
    }

    // 2. Fullbright cave & underwater illumination
    let has_fullbright_light = guard.mods.values().any(|m| {
        game.native_on(&m.id)
            && m.features.contains("fullbright")
            && setting_bool(game, &m.id, "enabled", true)
            && setting_bool(game, &m.id, "cave_light", true)
    });
    if has_fullbright_light {
        if let Some(p) = game.native_player() {
            lights.push(NativeColoredLight {
                at: [p.x, p.y + 1.5, p.z],
                rgb: 0xFFFBF0,
                radius: 15.0,
                intensity: 1.0,
            });
        }
    }

    // 3. LookAt / Jade / BlockScope targeted block rim highlight
    if let Some(jade_mod) = guard.mods.values().find(|m| {
        game.native_on(&m.id)
            && m.features.contains("jade_hud")
            && setting_bool(game, &m.id, "show_rim", true)
    }) {
        let reach = setting_u32(game, &jade_mod.id, "reach", 6) as f64;
        if let Some((eye, dir)) = game.aim() {
            if let Some(hit) = game.clip_ray(eye, dir, reach) {
                rims.push(NativeBlockRim {
                    pos: hit.pos,
                    argb: 0xCC80CBC4,
                    width: 2.2,
                });
            }
        }
    }
}

impl Game<'_> {
    fn mod_held_item_id(&self) -> Option<String> {
        self.inventory
            .get(self.selected)
            .and_then(Option::as_ref)
            .map(|s| s.item.to_string())
    }
}

#[cfg(target_os = "android")]
pub(super) fn reset() {
    official_zoom::reset();
    official_minimap::reset();
    official_recipes::set_viewer(None);
    smartf3_native::reset();
}
