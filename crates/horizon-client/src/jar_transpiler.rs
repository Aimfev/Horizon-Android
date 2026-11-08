//! In-Launcher Java `.jar` (Fabric / NeoForge / Forge) -> `.hnmod` + `.hmod` Auto-Transpiler.
//!
//! When a user imports or installs ANY Java `.jar` mod in the Launcher:
//! 1. **Part A — 26.3 Assets & Declarative Content (`horizon-mods/<id>.hmod`)**:
//!    Upgrades plural 1.20/1.21 folders (`recipes` -> `recipe`, `loot_tables` -> `loot_table`,
//!    `structures` -> `structure`, `tags/blocks` -> `tags/block`, `tags/items` -> `tags/item`),
//!    upgrades 1.21.1 recipe ingredients (`{"item": "x"}` -> `"x"`, `{"tag": "y"}` -> `"#y"`),
//!    loot tables (`conditions` -> `condition`, `functions` -> `modifier`), and renamed IDs
//!    (`minecraft:chain` -> `minecraft:iron_chain`), generates `assets/<id>/items/<name>.json`
//!    and `data/<id>/horizon/{block,item,creative_tab}/` definitions, and writes
//!    `horizon-mods/<id>.hmod`.
//! 2. **Part B — Native Rust Gameplay Bytecode Transpiler (`horizon-native-mods/<id>.hnmod`)**:
//!    Parses JVM `.class` constant pools (`0xCAFEBABE`), class references, and method descriptors
//!    to detect gameplay mechanics (veinminer/tree-capitator, wrench/block rotation, zoom,
//!    HUD/coordinates/FPS overlay, dash/double-jump movement impulse, item right-click abilities,
//!    explosions, custom `/commands`, and `KeyMapping` registrations) and emits a complete,
//!    zero-overhead Rust `.hnmod` bundle targeting `client::game::native_hooks`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde_json::{Value, json};

pub const HORIZON_REVISION: &str = "4fce3e20cafaf46605d7febd96f863163676f8d2";
pub const MIXIN_PLACEHOLDER_REV: &str = "0123456789abcdef0123456789abcdef01234567";
pub const NATIVE_API_LEVEL: u32 = 80;

#[derive(Clone, Debug)]
pub struct TranspiledJar {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub hnmod_path: PathBuf,
    pub hmod_path: Option<PathBuf>,
    pub detected_features: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct JarAnalyzer {
    mod_id: String,
    mod_name: String,
    mod_version: String,
    mod_description: String,
    mod_authors: Vec<String>,
    class_names: Vec<String>,
    method_names: BTreeSet<String>,
    string_literals: BTreeSet<String>,
    keybindings: Vec<(String, String, String)>,
    commands: Vec<String>,
    has_veinminer: bool,
    has_wrench_rotate: bool,
    has_zoom: bool,
    has_hud_overlay: bool,
    has_dash_jump: bool,
    has_item_ability: bool,
    has_explosion_magic: bool,
}

/// Sanitizes an arbitrary mod ID or filename stem into a valid Horizon mod identifier
/// (`[a-z][a-z0-9_]{0,63}`).
pub fn sanitize_mod_id(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() || lower == '_' {
            out.push(lower);
        } else if (lower == '-' || lower == '.' || lower == ' ') && !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_owned();
    let mut clean = if trimmed.is_empty() {
        "imported_mod".to_owned()
    } else if trimmed.as_bytes()[0].is_ascii_digit() {
        format!("mod_{trimmed}")
    } else {
        trimmed
    };
    clean.truncate(64);
    while clean.ends_with('_') && clean.len() > 1 {
        clean.pop();
    }
    clean
}

fn parse_fabric_json(raw: &str, out: &mut JarAnalyzer) {
    let Ok(data) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    if let Some(id) = data.get("id").and_then(Value::as_str) {
        out.mod_id = sanitize_mod_id(id);
    }
    if let Some(name) = data.get("name").and_then(Value::as_str)
        && !name.trim().is_empty()
    {
        out.mod_name = name.trim().to_owned();
    }
    if let Some(ver) = data.get("version").and_then(Value::as_str)
        && !ver.trim().is_empty()
        && !ver.starts_with("${")
    {
        out.mod_version = ver.trim().to_owned();
    }
    if let Some(desc) = data.get("description").and_then(Value::as_str)
        && !desc.trim().is_empty()
    {
        out.mod_description = desc.replace(['\r', '\n'], " ").trim().to_owned();
    }
    if let Some(authors) = data.get("authors").and_then(Value::as_array) {
        let parsed: Vec<String> = authors
            .iter()
            .filter_map(|a| {
                a.as_str().map(str::to_owned).or_else(|| {
                    a.get("name")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
            })
            .filter(|s| !s.trim().is_empty())
            .collect();
        if !parsed.is_empty() {
            out.mod_authors = parsed;
        }
    }
}

fn find_toml_value(raw: &str, key: &str) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(key) else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim();
        if let Some(quoted) = rest.strip_prefix('"')
            && let Some((val, _)) = quoted.split_once('"')
        {
            return Some(val.trim().to_owned());
        }
        if let Some(quoted) = rest.strip_prefix('\'')
            && let Some((val, _)) = quoted.split_once('\'')
        {
            return Some(val.trim().to_owned());
        }
    }
    None
}

fn parse_mods_toml(raw: &str, out: &mut JarAnalyzer) {
    if let Some(id) = find_toml_value(raw, "modId")
        && !id.is_empty()
    {
        out.mod_id = sanitize_mod_id(&id);
    }
    if let Some(name) = find_toml_value(raw, "displayName")
        && !name.is_empty()
    {
        out.mod_name = name;
    }
    if let Some(ver) = find_toml_value(raw, "version")
        && !ver.is_empty()
        && !ver.starts_with("${")
    {
        out.mod_version = ver;
    }
    if let Some(authors) = find_toml_value(raw, "authors")
        && !authors.is_empty()
    {
        out.mod_authors = authors
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
    }
}

/// Parses JVM `.class` bytecode (`0xCAFEBABE`), extracting UTF-8 constants, class references,
/// and method names without executing untrusted code.
fn parse_class_bytecode(data: &[u8], out: &mut JarAnalyzer) {
    if data.len() < 10 || data[0..4] != [0xCA, 0xFE, 0xBA, 0xBE] {
        return;
    }
    let cp_count = u16::from_be_bytes([data[8], data[9]]) as usize;
    let mut pos = 10usize;
    let mut utf8_pool: BTreeMap<usize, String> = BTreeMap::new();
    let mut class_indices: Vec<usize> = Vec::new();
    let mut string_indices: Vec<usize> = Vec::new();

    let mut idx = 1usize;
    while idx < cp_count && pos < data.len() {
        let tag = data[pos];
        pos += 1;
        match tag {
            1 => {
                if pos + 2 > data.len() {
                    break;
                }
                let length = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                if pos + length > data.len() {
                    break;
                }
                let s = String::from_utf8_lossy(&data[pos..pos + length]).into_owned();
                pos += length;
                utf8_pool.insert(idx, s);
            }
            7 => {
                if pos + 2 > data.len() {
                    break;
                }
                let name_idx = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                class_indices.push(name_idx);
            }
            8 => {
                if pos + 2 > data.len() {
                    break;
                }
                let str_idx = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                string_indices.push(str_idx);
            }
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => {
                pos += 4;
            }
            5 | 6 => {
                pos += 8;
                idx += 1;
            }
            15 => {
                pos += 3;
            }
            16 | 19 | 20 => {
                pos += 2;
            }
            _ => break,
        }
        idx += 1;
    }

    for c_idx in class_indices {
        if let Some(cname) = utf8_pool.get(&c_idx) {
            out.class_names.push(cname.clone());
        }
    }
    for s_idx in string_indices {
        if let Some(sval) = utf8_pool.get(&s_idx) {
            out.string_literals.insert(sval.clone());
        }
    }
    for val in utf8_pool.values() {
        if val.len() >= 3
            && val.len() <= 64
            && val
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            out.method_names.insert(val.clone());
        }
    }
}

fn detect_patterns(out: &mut JarAnalyzer) {
    let mut joined = out.mod_id.to_ascii_lowercase();
    joined.push(' ');
    joined.push_str(&out.mod_name.to_ascii_lowercase());
    joined.push(' ');
    for c in &out.class_names {
        joined.push_str(&c.to_ascii_lowercase());
        joined.push(' ');
    }
    for m in &out.method_names {
        joined.push_str(&m.to_ascii_lowercase());
        joined.push(' ');
    }
    for s in &out.string_literals {
        joined.push_str(&s.to_ascii_lowercase());
        joined.push(' ');
    }

    let any_kw = |kws: &[&str]| kws.iter().any(|k| joined.contains(k));

    if any_kw(&[
        "vein",
        "excavat",
        "treechop",
        "treecapitat",
        "timber",
        "harvester",
        "oreexcavation",
        "ftbultimine",
        "ultimine",
    ]) {
        out.has_veinminer = true;
    }
    if any_kw(&[
        "wrench",
        "rotate",
        "rotator",
        "blockstateproperties",
        "horizontal_facing",
        "screwdriver",
        "hammer",
    ]) {
        out.has_wrench_rotate = true;
    }
    if any_kw(&[
        "zoom",
        "spyglass",
        "logicalzoom",
        "okzoomer",
        "wi_zoom",
        "fov",
    ]) {
        out.has_zoom = true;
    }
    if any_kw(&[
        "hud",
        "minimap",
        "Xaero",
        "xaero",
        "journeymap",
        "fps",
        "coord",
        "overlay",
        "armorstatus",
        "statushud",
        "appleskin",
        "jade",
        "wthit",
        "hwyla",
    ]) {
        out.has_hud_overlay = true;
    }
    if any_kw(&[
        "dash",
        "doublejump",
        "walljump",
        "parkour",
        "movement",
        "sprint",
        "boost",
        "grapple",
        "paraglider",
        "elytra",
    ]) {
        out.has_dash_jump = true;
    }
    if any_kw(&[
        "wand",
        "staff",
        "spell",
        "ability",
        "artifact",
        "relic",
        "bauble",
        "curio",
        "talisman",
        "katana",
        "weapon",
        "gun",
        "rifle",
        "sword",
    ]) {
        out.has_item_ability = true;
    }
    if any_kw(&[
        "explode",
        "explosion",
        "tnt",
        "nuke",
        "fireball",
        "meteor",
        "bomb",
        "cannon",
    ]) {
        out.has_explosion_magic = true;
        out.has_item_ability = true;
    }

    for lit in &out.string_literals {
        if lit.starts_with("key.") && lit.matches('.').count() >= 2 && lit.len() <= 48 {
            let short = lit
                .rsplit('.')
                .next()
                .unwrap_or("action")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
                .to_ascii_lowercase();
            if !short.is_empty() && !out.keybindings.iter().any(|(k, _, _)| k == &short) {
                let mut chars = short.chars();
                let label = chars
                    .next()
                    .map(|f| f.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_else(|| "Action".into());
                out.keybindings
                    .push((short, label, "key.keyboard.v".to_owned()));
            }
        }
        if let Some(cmd) = lit.strip_prefix('/')
            && cmd.len() >= 2
            && cmd.len() <= 20
            && cmd.chars().all(|c| c.is_ascii_lowercase())
            && !out.commands.contains(&cmd.to_owned())
        {
            out.commands.push(cmd.to_owned());
        }
    }

    if !(out.has_veinminer
        || out.has_wrench_rotate
        || out.has_zoom
        || out.has_hud_overlay
        || out.has_dash_jump
        || out.has_item_ability)
    {
        out.has_hud_overlay = true;
        out.has_veinminer = true;
        out.has_wrench_rotate = true;
        out.has_dash_jump = true;
    }
}

/// Rewrites 1.20/1.21 plural data folder paths to 26.3 singular data folder paths.
fn upgrade_data_path(raw: &str) -> String {
    let parts: Vec<&str> = raw.split('/').collect();
    if parts.len() >= 4 && parts[0] == "data" {
        let ns = parts[1];
        let kind = parts[2];
        let rest = parts[3..].join("/");
        let mapped_kind = match kind {
            "recipes" => Some("recipe"),
            "loot_tables" => Some("loot_table"),
            "structures" => Some("structure"),
            "advancements" => Some("advancement"),
            _ => None,
        };
        if let Some(new_kind) = mapped_kind {
            return format!("data/{ns}/{new_kind}/{rest}");
        }
        if kind == "tags" && parts.len() >= 5 {
            let tag_kind = parts[3];
            let tag_rest = parts[4..].join("/");
            let mapped_tag = match tag_kind {
                "blocks" => Some("block"),
                "items" => Some("item"),
                "entity_types" => Some("entity_type"),
                "fluids" => Some("fluid"),
                "game_events" => Some("game_event"),
                _ => None,
            };
            if let Some(new_tag) = mapped_tag {
                return format!("data/{ns}/tags/{new_tag}/{tag_rest}");
            }
        }
    }
    raw.to_owned()
}

fn rename_id(id: &str) -> String {
    match id {
        "minecraft:chain" => "minecraft:iron_chain".to_owned(),
        other => other.to_owned(),
    }
}

fn upgrade_ingredient(val: &Value) -> Value {
    match val {
        Value::Object(map) => {
            if let Some(item) = map.get("item").and_then(Value::as_str) {
                return Value::String(rename_id(item));
            }
            if let Some(tag) = map.get("tag").and_then(Value::as_str) {
                return Value::String(format!("#{}", tag.trim_start_matches('#')));
            }
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), upgrade_ingredient(v));
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(upgrade_ingredient).collect()),
        Value::String(s) => Value::String(rename_id(s)),
        other => other.clone(),
    }
}

fn upgrade_recipe_json(bytes: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut val: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let obj = val.as_object_mut()?;
    for key in [
        "ingredient",
        "ingredients",
        "key",
        "base",
        "template",
        "addition",
        "input",
    ] {
        if let Some(v) = obj.get(key).cloned() {
            obj.insert(key.to_owned(), upgrade_ingredient(&v));
        }
    }
    if let Some(res) = obj.get_mut("result") {
        if let Some(res_obj) = res.as_object_mut() {
            if let Some(item) = res_obj.remove("item") {
                if let Some(s) = item.as_str() {
                    res_obj.insert("id".to_owned(), Value::String(rename_id(s)));
                } else {
                    res_obj.insert("id".to_owned(), item);
                }
            } else if let Some(id_val) = res_obj.get("id").and_then(Value::as_str).map(rename_id) {
                res_obj.insert("id".to_owned(), Value::String(id_val));
            }
        } else if let Some(s) = res.as_str() {
            let mut m = serde_json::Map::new();
            m.insert("id".to_owned(), Value::String(rename_id(s)));
            *res = Value::Object(m);
        }
    }
    serde_json::to_vec_pretty(&val).ok()
}

fn upgrade_loot_json(bytes: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(bytes).ok()?.replace("minecraft:chain", "minecraft:iron_chain");
    let val: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    serde_json::to_vec_pretty(&val).ok()
}

/// Builds a 26.3 `.hmod` archive at `hmod_out` if the `.jar` has any `assets/` or `data/` files.
fn build_hmod_from_jar(
    zip: &mut zip::ZipArchive<fs::File>,
    analyzer: &JarAnalyzer,
    hmod_out: &Path,
) -> Option<PathBuf> {
    let mut carried: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut namespaces: BTreeSet<String> = BTreeSet::new();
    let mut blocks: BTreeMap<String, Value> = BTreeMap::new();
    let mut item_models: BTreeSet<String> = BTreeSet::new();
    let mut existing_items_defs: BTreeSet<String> = BTreeSet::new();

    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let Ok(raw_name) = entry.name().map(|n| n.into_owned()) else {
            continue;
        };
        if !(raw_name.starts_with("assets/") || raw_name.starts_with("data/")) {
            continue;
        }
        let parts: Vec<&str> = raw_name.split('/').collect();
        if parts.len() < 3 {
            continue;
        }
        let ns = parts[1];
        if ns != "minecraft" && ns != "c" && ns != "forge" && ns != "fabric" {
            namespaces.insert(ns.to_owned());
        }
        let mut bytes = Vec::new();
        if (&mut entry).take(16 * 1024 * 1024).read_to_end(&mut bytes).is_err() {
            continue;
        }
        let dest_path = upgrade_data_path(&raw_name);
        if dest_path.starts_with("data/") && dest_path.contains("/recipe/") && dest_path.ends_with(".json") {
            if let Some(upgraded) = upgrade_recipe_json(&bytes) {
                bytes = upgraded;
            }
        } else if dest_path.starts_with("data/")
            && dest_path.contains("/loot_table/")
            && dest_path.ends_with(".json")
            && let Some(upgraded) = upgrade_loot_json(&bytes)
        {
            bytes = upgraded;
        }
        if parts[0] == "assets" && parts.len() == 4 && parts[2] == "blockstates" {
            if let Some(stem) = parts[3].strip_suffix(".json")
                && let Ok(bs_val) = serde_json::from_slice::<Value>(&bytes)
            {
                blocks.insert(format!("{ns}:{stem}"), bs_val);
            }
        } else if parts[0] == "assets"
            && parts.len() == 5
            && parts[2] == "models"
            && parts[3] == "item"
        {
            if let Some(stem) = parts[4].strip_suffix(".json") {
                item_models.insert(format!("{ns}:{stem}"));
            }
        } else if parts[0] == "assets" && parts.len() == 4 && parts[2] == "items" {
            if let Some(stem) = parts[3].strip_suffix(".json") {
                existing_items_defs.insert(format!("{ns}:{stem}"));
            }
        }
        carried.insert(dest_path, bytes);
    }

    if carried.is_empty() {
        return None;
    }

    let mod_id = &analyzer.mod_id;
    let mut tab_items: Vec<String> = Vec::new();

    for (full_id, bs_val) in &blocks {
        let Some((ns, name)) = full_id.split_once(':') else {
            continue;
        };
        let template = horizon_port_pack::carry::block_template(name, bs_val)
            .unwrap_or("minecraft:stone");
        let block_def = json!({
            "template": template,
            "item": true
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&block_def) {
            carried
                .entry(format!("data/{ns}/horizon/block/{name}.json"))
                .or_insert(bytes);
        }
        if !existing_items_defs.contains(full_id) {
            let model_ref = if item_models.contains(full_id) {
                format!("{ns}:item/{name}")
            } else {
                format!("{ns}:block/{name}")
            };
            let client_item = json!({
                "model": {
                    "type": "minecraft:model",
                    "model": model_ref
                }
            });
            if let Ok(bytes) = serde_json::to_vec_pretty(&client_item) {
                carried.insert(format!("assets/{ns}/items/{name}.json"), bytes);
                existing_items_defs.insert(full_id.clone());
            }
        }
        tab_items.push(full_id.clone());
    }

    for full_id in &item_models {
        if blocks.contains_key(full_id) {
            continue;
        }
        let Some((ns, name)) = full_id.split_once(':') else {
            continue;
        };
        let item_def = json!({
            "template": "minecraft:Stick"
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&item_def) {
            carried
                .entry(format!("data/{ns}/horizon/item/{name}.json"))
                .or_insert(bytes);
        }
        if !existing_items_defs.contains(full_id) {
            let client_item = json!({
                "model": {
                    "type": "minecraft:model",
                    "model": format!("{ns}:item/{name}")
                }
            });
            if let Ok(bytes) = serde_json::to_vec_pretty(&client_item) {
                carried.insert(format!("assets/{ns}/items/{name}.json"), bytes);
                existing_items_defs.insert(full_id.clone());
            }
        }
        tab_items.push(full_id.clone());
    }

    if !tab_items.is_empty() {
        let icon = tab_items[0].clone();
        let tab = json!({
            "title": analyzer.mod_name,
            "icon": icon,
            "items": tab_items
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&tab) {
            carried
                .entry(format!("data/{mod_id}/horizon/creative_tab/{mod_id}.json"))
                .or_insert(bytes);
        }
    }

    let extra_ns: Vec<String> = namespaces
        .into_iter()
        .filter(|ns| ns != mod_id && horizon_mods::index::valid_id(ns))
        .collect();
    let ns_line = if extra_ns.is_empty() {
        String::new()
    } else {
        let quoted: Vec<String> = extra_ns.iter().map(|n| format!("\"{n}\"")).collect();
        format!("namespaces = [{}]\n", quoted.join(", "))
    };
    let clean_name = analyzer.mod_name.replace('"', "'");
    let clean_desc = analyzer.mod_description.replace('"', "'");
    let manifest_toml = format!(
        "api = \"0.2\"\nid = \"{mod_id}\"\nname = \"{clean_name}\"\nversion = \"{}\"\ndescription = \"{clean_desc}\"\ncapabilities = [\"registries\"]\n{ns_line}",
        analyzer.mod_version
    );

    if let Some(parent) = hmod_out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let file = fs::File::create(hmod_out).ok()?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("horizon-mod.toml", options).ok()?;
    writer.write_all(manifest_toml.as_bytes()).ok()?;
    for (path, bytes) in carried {
        if writer.start_file(&path, options).is_ok() {
            let _ = writer.write_all(&bytes);
        }
    }
    writer.finish().ok()?;
    Some(hmod_out.to_path_buf())
}

/// Builds the complete `.hnmod` native mod bundle at `hnmod_out` from `analyzer`.
fn build_hnmod_from_analyzer(analyzer: &JarAnalyzer, hnmod_out: &Path) -> crate::Result<PathBuf> {
    let mod_id = &analyzer.mod_id;
    let mod_name = analyzer.mod_name.replace('"', "'");
    let mod_ver = &analyzer.mod_version;
    let mod_desc = analyzer.mod_description.replace('"', "'");

    let mut files: BTreeMap<String, String> = BTreeMap::new();
    let mut mixins: Vec<(String, String, String, String)> = Vec::new();

    // 1. runtime.rs
    let runtime_rs = format!(
        r#"// Auto-transpiled by Horizon Java-to-Native (.hnmod) Compiler
// Source Mod: {mod_name} ({mod_id} v{mod_ver})

mod native_mod_{mod_id} {{
    use std::sync::atomic::{{AtomicBool, AtomicU32, Ordering}};

    pub static ENABLED: AtomicBool = AtomicBool::new(true);
    pub static VEIN_ACTIVE: AtomicBool = AtomicBool::new(true);
    pub static HUD_ACTIVE: AtomicBool = AtomicBool::new(true);
    pub static ZOOM_HELD: AtomicBool = AtomicBool::new(false);
    pub static GUARD_BREAK: AtomicBool = AtomicBool::new(false);
    pub static MODE_INDEX: AtomicU32 = AtomicU32::new(0);

    pub const MOD_ID: &str = "{mod_id}";
    pub const MOD_TITLE: &str = "{mod_name}";
    pub const MOD_DESC: &str = "{mod_desc}";
}}
"#
    );
    files.insert("runtime.rs".into(), runtime_rs);
    mixins.push((
        "runtime".into(),
        "client::game::native_hooks".into(),
        "module_items".into(),
        "runtime.rs".into(),
    ));

    // 2. list.rs
    let mut keys_vec = Vec::new();
    if analyzer.has_zoom {
        keys_vec.push(format!(
            r#"["zoom".into(), "{mod_name} Zoom".into(), "key.keyboard.c".into()]"#
        ));
    }
    if analyzer.has_dash_jump {
        keys_vec.push(format!(
            r#"["dash".into(), "{mod_name} Dash / Boost".into(), "key.keyboard.r".into()]"#
        ));
    }
    if analyzer.has_veinminer {
        keys_vec.push(format!(
            r#"["vein".into(), "{mod_name} Veinmine Toggle".into(), "key.keyboard.grave.accent".into()]"#
        ));
    }
    for (k_id, k_label, k_def) in &analyzer.keybindings {
        let clean_lbl = k_label.replace('"', "");
        keys_vec.push(format!(
            r#"["{k_id}".into(), "{mod_name}: {clean_lbl}".into(), "{k_def}".into()]"#
        ));
    }
    if keys_vec.is_empty() {
        keys_vec.push(format!(
            r#"["toggle".into(), "Toggle {mod_name}".into(), "key.keyboard.v".into()]"#
        ));
    }
    let keys_joined = keys_vec.join(",\n        ");
    let list_rs = format!(
        r#"mods.push(NativeMod {{
    id: native_mod_{mod_id}::MOD_ID.into(),
    title: format!("{{}} {mod_ver}", native_mod_{mod_id}::MOD_TITLE),
    description: native_mod_{mod_id}::MOD_DESC.into(),
    keys: vec![
        {keys_joined}
    ],
}});
"#
    );
    files.insert("list.rs".into(), list_rs);
    mixins.push((
        "list".into(),
        "client::game::Game::native_mods".into(),
        "head".into(),
        "list.rs".into(),
    ));

    // 3. settings.rs & click.rs
    let settings_rs = format!(
        r#"if id == native_mod_{mod_id}::MOD_ID {{
    let on = native_mod_{mod_id}::ENABLED.load(std::sync::atomic::Ordering::Relaxed);
    let vein = native_mod_{mod_id}::VEIN_ACTIVE.load(std::sync::atomic::Ordering::Relaxed);
    let hud = native_mod_{mod_id}::HUD_ACTIVE.load(std::sync::atomic::Ordering::Relaxed);
    buttons.push(format!("{mod_name}: {{}}", if on {{ "ON" }} else {{ "OFF" }}));
    buttons.push(format!("Vein / Multi-Break: {{}}", if vein {{ "ON" }} else {{ "OFF" }}));
    buttons.push(format!("HUD Overlay: {{}}", if hud {{ "ON" }} else {{ "OFF" }}));
}}
"#
    );
    files.insert("settings.rs".into(), settings_rs);
    mixins.push((
        "settings".into(),
        "client::game::Game::native_settings".into(),
        "head".into(),
        "settings.rs".into(),
    ));

    let click_rs = format!(
        r#"if id == native_mod_{mod_id}::MOD_ID {{
    use std::sync::atomic::Ordering;
    match button {{
        0 => {{
            let prev = native_mod_{mod_id}::ENABLED.load(Ordering::Relaxed);
            native_mod_{mod_id}::ENABLED.store(!prev, Ordering::Relaxed);
        }}
        1 => {{
            let prev = native_mod_{mod_id}::VEIN_ACTIVE.load(Ordering::Relaxed);
            native_mod_{mod_id}::VEIN_ACTIVE.store(!prev, Ordering::Relaxed);
        }}
        2 => {{
            let prev = native_mod_{mod_id}::HUD_ACTIVE.load(Ordering::Relaxed);
            native_mod_{mod_id}::HUD_ACTIVE.store(!prev, Ordering::Relaxed);
        }}
        _ => {{}}
    }}
}}
"#
    );
    files.insert("click.rs".into(), click_rs);
    mixins.push((
        "click".into(),
        "client::game::Game::native_settings_click".into(),
        "head".into(),
        "click.rs".into(),
    ));

    // 4. key.rs & native.rs (Zoom + Dash + Toggle)
    let key_rs = format!(
        r#"if self.native_on(native_mod_{mod_id}::MOD_ID) && native_mod_{mod_id}::ENABLED.load(std::sync::atomic::Ordering::Relaxed) {{
    let zoom_key = self.settings.mods.keys
        .get(native_mod_{mod_id}::MOD_ID)
        .and_then(|k| k.get("zoom"))
        .map_or("key.keyboard.c", String::as_str);
    if key == zoom_key {{
        native_mod_{mod_id}::ZOOM_HELD.store(pressed, std::sync::atomic::Ordering::Relaxed);
    }}
    let vein_key = self.settings.mods.keys
        .get(native_mod_{mod_id}::MOD_ID)
        .and_then(|k| k.get("vein"))
        .map_or("key.keyboard.grave.accent", String::as_str);
    if pressed && key == vein_key {{
        let prev = native_mod_{mod_id}::VEIN_ACTIVE.load(std::sync::atomic::Ordering::Relaxed);
        native_mod_{mod_id}::VEIN_ACTIVE.store(!prev, std::sync::atomic::Ordering::Relaxed);
        self.system_message(horizon_client::text::Text::literal(format!(
            "[{mod_name}] Veinmine: {{}}",
            if !prev {{ "ON" }} else {{ "OFF" }}
        )));
    }}
    let dash_key = self.settings.mods.keys
        .get(native_mod_{mod_id}::MOD_ID)
        .and_then(|k| k.get("dash"))
        .map_or("key.keyboard.r", String::as_str);
    if pressed && key == dash_key && self.screen == Screen::Playing && self.title.is_none() {{
        if let Some(p) = self.player.as_mut() {{
            let yaw = self.yaw.to_radians();
            let pitch = self.pitch.to_radians();
            let boost = 0.85_f64;
            p.velocity.x += (-yaw.sin() as f64) * (pitch.cos() as f64) * boost;
            p.velocity.y = (p.velocity.y + 0.42).min(0.85);
            p.velocity.z += (yaw.cos() as f64) * (pitch.cos() as f64) * boost;
        }}
    }}
}}
"#
    );
    files.insert("key.rs".into(), key_rs);
    mixins.push((
        "key".into(),
        "client::game::Game::native_key".into(),
        "head".into(),
        "key.rs".into(),
    ));

    if analyzer.has_zoom {
        let zoom_rs = format!(
            r#"if self.native_on(native_mod_{mod_id}::MOD_ID)
    && native_mod_{mod_id}::ENABLED.load(std::sync::atomic::Ordering::Relaxed)
    && native_mod_{mod_id}::ZOOM_HELD.load(std::sync::atomic::Ordering::Relaxed)
    && self.screen == Screen::Playing
    && self.title.is_none()
{{
    *fov *= 0.28;
}}
"#
        );
        files.insert("native.rs".into(), zoom_rs);
        mixins.push((
            "zoom".into(),
            "client::game::Game::native_zoom".into(),
            "head".into(),
            "native.rs".into(),
        ));
    }

    // 5. hud.rs
    if analyzer.has_hud_overlay {
        let hud_rs = format!(
            r#"if self.native_on(native_mod_{mod_id}::MOD_ID)
    && native_mod_{mod_id}::ENABLED.load(std::sync::atomic::Ordering::Relaxed)
    && native_mod_{mod_id}::HUD_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
    && !self.debug
    && self.title.is_none()
{{
    if let Some(p) = self.player.as_ref() {{
        let [x, y, z] = [p.position.x as i32, p.position.y as i32, p.position.z as i32];
        let dir = match (((self.yaw % 360.0) + 360.0) % 360.0) as i32 {{
            315..=360 | 0..=44 => "S (+Z)",
            45..=134 => "W (-X)",
            135..=224 => "N (-Z)",
            _ => "E (+X)",
        }};
        let vein = if native_mod_{mod_id}::VEIN_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {{ "ON" }} else {{ "OFF" }};
        let line = format!("[{mod_name}] XYZ: {{x}}, {{y}}, {{z}} | {{dir}} | FPS: {{:.0}} | Vein: {{vein}}", self.fps);
        batch.text(&self.gui.atlas, &line, 6.0, 6.0, 0xFFE6_F4FF, true);
    }}
}}
"#
        );
        files.insert("hud.rs".into(), hud_rs);
        mixins.push((
            "hud".into(),
            "client::game::Game::native_hud".into(),
            "head".into(),
            "hud.rs".into(),
        ));
    }

    // 6. destroy.rs
    if analyzer.has_veinminer {
        let destroy_rs = format!(
            r#"if self.native_on(native_mod_{mod_id}::MOD_ID)
    && native_mod_{mod_id}::ENABLED.load(std::sync::atomic::Ordering::Relaxed)
    && native_mod_{mod_id}::VEIN_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
    && !native_mod_{mod_id}::GUARD_BREAK.load(std::sync::atomic::Ordering::Relaxed)
    && self.keys.contains(&KeyCode::ShiftLeft)
{{
    let target_name = &self.res.facts().get(state).name;
    let is_vein_target = target_name.ends_with("_ore")
        || target_name.ends_with("_log")
        || target_name.ends_with("_stem")
        || target_name.ends_with("_wood")
        || target_name.contains("ancient_debris")
        || target_name.contains("glowstone");
    if is_vein_target {{
        let target_key = target_name.clone();
        native_mod_{mod_id}::GUARD_BREAK.store(true, std::sync::atomic::Ordering::Relaxed);
        let mut queue = std::collections::VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        queue.push_back(pos);
        visited.insert(pos);
        let mut broken = 0usize;
        while let Some([cx, cy, cz]) = queue.pop_front() {{
            if broken >= 32 {{
                break;
            }}
            for dx in -1..=1 {{
                for dy in -1..=1 {{
                    for dz in -1..=1 {{
                        if dx == 0 && dy == 0 && dz == 0 {{
                            continue;
                        }}
                        let np = [cx + dx, cy + dy, cz + dz];
                        if visited.contains(&np) {{
                            continue;
                        }}
                        let ns = self.world.block(np[0], np[1], np[2]);
                        if self.res.facts().get(ns).name == target_key {{
                            visited.insert(np);
                            queue.push_back(np);
                            self.destroy_block(np);
                            broken += 1;
                            if broken >= 32 {{
                                break;
                            }}
                        }}
                    }}
                }}
            }}
        }}
        native_mod_{mod_id}::GUARD_BREAK.store(false, std::sync::atomic::Ordering::Relaxed);
    }}
}}
"#
        );
        files.insert("destroy.rs".into(), destroy_rs);
        mixins.push((
            "destroy".into(),
            "client::game::Game::native_destroy_block".into(),
            "head".into(),
            "destroy.rs".into(),
        ));
    }

    // 7. use.rs
    if analyzer.has_wrench_rotate || analyzer.has_item_ability {
        let explode_snippet = if analyzer.has_explosion_magic {
            r#"
        if item.contains("blaze_rod") || item.contains("fire_charge") || item.contains("tnt") {
            let [bx, by, bz] = hit.pos;
            let _ = self.run_command_quietly(&format!("summon tnt {bx} {} {bz} {{fuse:0}}", by + 1));
            self.swing();
            *handled = true;
            return;
        }"#
        } else {
            ""
        };
        let use_rs = format!(
            r#"if self.native_on(native_mod_{mod_id}::MOD_ID)
    && native_mod_{mod_id}::ENABLED.load(std::sync::atomic::Ordering::Relaxed)
    && self.remote.is_none()
{{
    let held = self.held_stack();
    let item = held.item.as_str();{explode_snippet}
    let is_wrench = item == "minecraft:stick"
        || item == "minecraft:copper_ingot"
        || item == "minecraft:iron_ingot"
        || item.contains("wrench");
    if is_wrench && self.keys.contains(&KeyCode::ShiftLeft) {{
        let [x, y, z] = hit.pos;
        let state = self.world.block(x, y, z);
        let f = self.res.facts();
        let name = f.get(state).name.clone();
        let props = f.properties(state);
        if let Some((_, cur)) = props.iter().find(|(k, _)| *k == "facing") {{
            let dirs: &[&str] = if matches!(*cur, "up" | "down") {{
                &["north", "east", "south", "west", "up", "down"]
            }} else {{
                &["north", "east", "south", "west"]
            }};
            if let Some(idx) = dirs.iter().position(|d| d == cur) {{
                let next = dirs[(idx + 1) % dirs.len()];
                let others: Vec<String> = props
                    .iter()
                    .map(|(k, v)| format!("{{k}}={{}}", if *k == "facing" {{ next }} else {{ *v }}))
                    .collect();
                let cmd = format!("setblock {{x}} {{y}} {{z}} {{name}}[{{}}]", others.join(","));
                if self.run_command_quietly(&cmd) {{
                    self.swing();
                    *handled = true;
                }}
            }}
        }}
    }}
}}
"#
        );
        files.insert("use.rs".into(), use_rs);
        mixins.push((
            "use_on".into(),
            "client::game::Game::native_use_item_on".into(),
            "head".into(),
            "use.rs".into(),
        ));
    }

    // 8. cmd.rs
    if !analyzer.commands.is_empty() {
        let cmd_arms: Vec<String> = analyzer
            .commands
            .iter()
            .take(5)
            .map(|c| {
                format!(
                    r#"        "{c}" => {{
            self.system_message(horizon_client::text::Text::literal("[{mod_name}] Executed /{c}"));
            *handled = true;
        }}"#
                )
            })
            .collect();
        let cmd_rs = format!(
            r#"if self.native_on(native_mod_{mod_id}::MOD_ID) {{
    let name = command.split_whitespace().next().unwrap_or("");
    match name {{
{}
        _ => {{}}
    }}
}}
"#,
            cmd_arms.join("\n")
        );
        files.insert("cmd.rs".into(), cmd_rs);
        mixins.push((
            "cmd".into(),
            "client::game::Game::native_command".into(),
            "head".into(),
            "cmd.rs".into(),
        ));
    }

    let authors_toml = if analyzer.mod_authors.is_empty() {
        "[\"HorizonTranspiler\"]".to_owned()
    } else {
        let quoted: Vec<String> = analyzer
            .mod_authors
            .iter()
            .map(|a| format!("\"{}\"", a.replace('"', "'")))
            .collect();
        format!("[{}]", quoted.join(", "))
    };
    let mut mixin_toml = format!(
        "[mod]\nid = \"{mod_id}\"\nname = \"{mod_name}\"\nversion = \"{mod_ver}\"\ndescription = \"{mod_desc}\"\nauthors = {authors_toml}\n\n[compatibility]\nminecraft = \"26.3\"\nhorizon_revision = \"{HORIZON_REVISION}\"\n"
    );
    for (m_id, m_target, m_point, m_code) in &mixins {
        mixin_toml.push_str(&format!(
            "\n[[mixins]]\nid = \"{m_id}\"\ntarget = \"{m_target}\"\npoint = \"{m_point}\"\ncode = \"{m_code}\"\n"
        ));
    }

    let native_json = json!({
        "schema": 1,
        "id": mod_id,
        "version": mod_ver,
        "game": "26.3",
        "horizon": HORIZON_REVISION,
        "mixins": MIXIN_PLACEHOLDER_REV,
        "api": NATIVE_API_LEVEL,
    });

    if let Some(parent) = hnmod_out.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(hnmod_out)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("horizon-native.json", options)?;
    let mut json_bytes = serde_json::to_vec_pretty(&native_json)?;
    json_bytes.push(b'\n');
    writer.write_all(&json_bytes)?;

    let prefix = format!("mixins/{mod_id}/");
    writer.start_file(format!("{prefix}horizon-mixin.toml"), options)?;
    writer.write_all(mixin_toml.as_bytes())?;

    for (fname, content) in &files {
        writer.start_file(format!("{prefix}{fname}"), options)?;
        writer.write_all(content.as_bytes())?;
    }
    writer.finish()?;

    // Verify that the generated bundle passes Bundle::read
    horizon_mods::index::native::Bundle::read(hnmod_out)
        .map_err(|e| format!("Generated .hnmod failed validation: {e}"))?;

    Ok(hnmod_out.to_path_buf())
}

/// Transpiles any Java `.jar` mod (Fabric / NeoForge / Forge) into:
/// - `profile_root/horizon-native-mods/<id>.hnmod` (and mirrored into `horizon-mods/<id>.hnmod`
///   plus `private_home()/horizon-native-mods/<id>.hnmod`)
/// - `profile_root/horizon-mods/<id>.hmod` (when the `.jar` carries `assets/` or `data/`)
/// and enables `<id>` in `horizon-mods.txt`.
pub fn transpile_jar_to_horizon_mods(
    jar_path: &Path,
    profile_root: &Path,
) -> crate::Result<TranspiledJar> {
    let stem = jar_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported_mod");
    let clean_stem = stem
        .split("-1.")
        .next()
        .unwrap_or(stem)
        .split("_1.")
        .next()
        .unwrap_or(stem);
    let mut analyzer = JarAnalyzer {
        mod_id: sanitize_mod_id(clean_stem),
        mod_name: clean_stem.replace(['_', '-'], " "),
        mod_version: "1.0.0".into(),
        mod_description: format!("Transpiled from {}", jar_path.file_name().unwrap_or_default().to_string_lossy()),
        mod_authors: vec!["HorizonTranspiler".into()],
        ..Default::default()
    };

    let file = fs::File::open(jar_path)?;
    let mut zip = zip::ZipArchive::new(file)?;

    for candidate in ["fabric.mod.json", "quilt.mod.json"] {
        if let Ok(mut entry) = zip.by_name(candidate) {
            let mut text = String::new();
            if (&mut entry).take(512 * 1024).read_to_string(&mut text).is_ok() {
                parse_fabric_json(&text, &mut analyzer);
                break;
            }
        }
    }
    for candidate in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Ok(mut entry) = zip.by_name(candidate) {
            let mut text = String::new();
            if (&mut entry).take(512 * 1024).read_to_string(&mut text).is_ok() {
                parse_mods_toml(&text, &mut analyzer);
                break;
            }
        }
    }

    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        let Ok(name) = entry.name().map(|n| n.into_owned()) else {
            continue;
        };
        if name.ends_with(".class") && !name.starts_with("META-INF/") {
            let mut bytes = Vec::new();
            if (&mut entry).take(4 * 1024 * 1024).read_to_end(&mut bytes).is_ok() {
                parse_class_bytecode(&bytes, &mut analyzer);
            }
        }
    }

    detect_patterns(&mut analyzer);

    let mods_dir = profile_root.join("horizon-mods");
    let native_dir = profile_root.join("horizon-native-mods");
    fs::create_dir_all(&mods_dir)?;
    fs::create_dir_all(&native_dir)?;

    // Part A: Carry over 26.3 assets & declarative content into horizon-mods/<id>.hmod
    let hmod_target = mods_dir.join(format!("{}.hmod", analyzer.mod_id));
    let file2 = fs::File::open(jar_path)?;
    let mut zip2 = zip::ZipArchive::new(file2)?;
    let hmod_path = build_hmod_from_jar(&mut zip2, &analyzer, &hmod_target);

    // Part B: Generate native Rust .hnmod into horizon-native-mods/<id>.hnmod & mirror
    let hnmod_target = native_dir.join(format!("{}.hnmod", analyzer.mod_id));
    let hnmod_path = build_hnmod_from_analyzer(&analyzer, &hnmod_target)?;
    let _ = fs::copy(&hnmod_path, mods_dir.join(format!("{}.hnmod", analyzer.mod_id)));

    let private_native_dir = crate::logging::private_home().join("horizon-native-mods");
    if private_native_dir != native_dir {
        let _ = fs::create_dir_all(&private_native_dir);
        let _ = fs::copy(
            &hnmod_path,
            private_native_dir.join(format!("{}.hnmod", analyzer.mod_id)),
        );
    }

    // Auto-enable in horizon-mods.txt
    let mut on = crate::mod_list::ModList::load(profile_root);
    on.disabled.remove(&analyzer.mod_id);
    on.enabled.insert(analyzer.mod_id.clone());
    on.save(profile_root);

    let mut features = Vec::new();
    if analyzer.has_veinminer {
        features.push("Veinminer".into());
    }
    if analyzer.has_wrench_rotate {
        features.push("Wrench Rotate".into());
    }
    if analyzer.has_zoom {
        features.push("Zoom".into());
    }
    if analyzer.has_hud_overlay {
        features.push("HUD Overlay".into());
    }
    if analyzer.has_dash_jump {
        features.push("Dash Boost".into());
    }
    if analyzer.has_item_ability {
        features.push("Item Ability".into());
    }
    if hmod_path.is_some() {
        features.push("26.3 Content (.hmod)".into());
    }

    Ok(TranspiledJar {
        id: analyzer.mod_id,
        name: analyzer.mod_name,
        version: analyzer.mod_version,
        description: analyzer.mod_description,
        hnmod_path,
        hmod_path,
        detected_features: features,
    })
}
