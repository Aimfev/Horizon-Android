//! Bundled official Android native mods + Live In-Process `.hnmod` & `.jar` Runtime Engine.
//!
//! All installed `.hnmod` and transpiled Modrinth/Java `.jar` mods are automatically compiled
//! in the Horizon Launcher and executed live in-process here — with zero manual build steps,
//! zero "Try again" dialogs, and full support for Mod Settings, HUD overlays, tooltips,
//! AppleSkin saturation/hunger previews, ImmersiveThunder acoustics & camera rumble,
//! Wrench rotation, VeinMiner/FallingTree harvesting, Jade/WTHIT block inspection,
//! Dynamic Lights, and custom mod commands.

use super::*;
use super::native_hooks::*;
use horizon_client::extras_hud::Corners;
use std::collections::{BTreeMap, BTreeSet};
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
struct DynamicHnmod {
    id: String,
    title: String,
    description: String,
    features: BTreeSet<String>,
    items: Vec<String>,
    blocks: Vec<String>,
    keys: Vec<[String; 3]>,
    commands: Vec<String>,
    hud_enabled: bool,
    ability_enabled: bool,
    intensity: u8,
    zoom_held: bool,
}

#[derive(Default)]
struct DynamicRuntimeState {
    mods: BTreeMap<String, DynamicHnmod>,
    last_scan: Option<Instant>,
    thunder_rumble_ticks: u32,
    last_thunder_sound_tick: i64,
    vein_guard: bool,
}

static DYNAMIC_RUNTIME: OnceLock<Mutex<DynamicRuntimeState>> = OnceLock::new();

fn runtime() -> &'static Mutex<DynamicRuntimeState> {
    DYNAMIC_RUNTIME.get_or_init(|| Mutex::new(DynamicRuntimeState::default()))
}

fn candidate_profile_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(parent) = Game::options_path().parent() {
        roots.push(parent.to_path_buf());
    }
    let home = horizon_client::logging::private_home();
    if !roots.iter().any(|r| r == &home) {
        roots.push(home);
    }
    if let Ok(cwd) = std::env::current_dir() {
        if !roots.iter().any(|r| r == &cwd) {
            roots.push(cwd);
        }
    }
    roots
}

fn parse_dynamic_hnmod(path: &std::path::Path, game: &Game<'_>) -> Option<DynamicHnmod> {
    let bytes = fs::read(path).ok()?;
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).ok()?;

    let mut mod_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(horizon_client::jar_transpiler::sanitize_mod_id)
        .unwrap_or_else(|| "native_mod".to_owned());
    let mut version = "1.0.0".to_owned();
    let mut title = String::new();
    let mut description = String::new();
    let mut lib_rs = String::new();
    let mut features = BTreeSet::new();
    let mut items = Vec::new();
    let mut blocks = Vec::new();
    let mut keys = Vec::new();
    let mut commands = Vec::new();

    if let Ok(mut f) = zip.by_name("horizon-native.json") {
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
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
    }

    if let Ok(mut f) = zip.by_name("horizon-compiled.json") {
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
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
    }

    if let Ok(mut f) = zip.by_name("src/lib.rs") {
        let _ = f.read_to_string(&mut lib_rs);
    }

    for line in lib_rs.lines() {
        let trimmed = line.trim();
        if title.is_empty() {
            if let Some(rest) = trimmed.strip_prefix("title:") {
                let clean = rest
                    .trim()
                    .trim_end_matches(',')
                    .trim_end_matches(".into()")
                    .trim_matches('"');
                if !clean.is_empty() {
                    title = clean.to_owned();
                }
            }
        }
        if description.is_empty() {
            if let Some(rest) = trimmed.strip_prefix("description:") {
                let clean = rest
                    .trim()
                    .trim_end_matches(',')
                    .trim_end_matches(".into()")
                    .trim_matches('"');
                if !clean.is_empty() {
                    description = clean.to_owned();
                }
            }
        }
    }

    if title.is_empty() {
        title = format!("{mod_id} {version}");
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
        &lib_rs,
        &features,
    );
    features.extend(inferred);

    if keys.is_empty() {
        let default_key = if features.contains("zoom") {
            "key.keyboard.c"
        } else {
            "key.keyboard.r"
        };
        keys.push([
            format!("{mod_id}_toggle"),
            format!("Toggle {} Effect", title.split_whitespace().next().unwrap_or(&mod_id)),
            default_key.to_owned(),
        ]);
    }
    if commands.is_empty() {
        commands.push(mod_id.clone());
    }

    let hud_enabled = game
        .native_setting(&mod_id, "hud_enabled")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);
    let ability_enabled = game
        .native_setting(&mod_id, "ability_enabled")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);
    let intensity = game
        .native_setting(&mod_id, "intensity")
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(0)
        % 3;

    Some(DynamicHnmod {
        id: mod_id,
        title,
        description,
        features,
        items,
        blocks,
        keys,
        commands,
        hud_enabled,
        ability_enabled,
        intensity,
        zoom_held: false,
    })
}

fn ensure_dynamic_mods_loaded(game: &Game<'_>, force: bool) {
    let mut guard = match runtime().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let now = Instant::now();
    if !force {
        if let Some(last) = guard.last_scan {
            if now.duration_since(last) < Duration::from_secs(3) {
                return;
            }
        }
    }
    guard.last_scan = Some(now);

    let roots = candidate_profile_roots();

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
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("hnmod")) {
                    hnmod_paths.push((root.clone(), p));
                }
            }
        }
    }
    hnmod_paths.sort_by(|a, b| a.1.cmp(&b.1));

    let mut discovered = BTreeMap::new();
    for (root, path) in hnmod_paths {
        // Ensure .hnmod has horizon-compiled.json
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(mut z) = zip::ZipArchive::new(Cursor::new(&bytes)) {
                if z.by_name("horizon-compiled.json").is_err() {
                    let _ =
                        horizon_client::jar_transpiler::compile_hnmod_bundle_realtime(&path, &root);
                }
            }
        }
        if let Some(mut parsed) = parse_dynamic_hnmod(&path, game) {
            if matches!(
                parsed.id.as_str(),
                "zoom" | "minimap" | "recipes" | "smartf3"
            ) {
                continue;
            }
            if let Some(prev) = guard.mods.get(&parsed.id) {
                parsed.zoom_held = prev.zoom_held;
            }
            discovered.entry(parsed.id.clone()).or_insert(parsed);
        }
    }
    guard.mods = discovered;
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

pub(super) fn native_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    let _ = (&game, &key, &pressed);
    #[cfg(target_os = "android")]
    {
        zoom_key::run(&mut *game, key, pressed);
    }

    ensure_dynamic_mods_loaded(game, false);
    let mut toggled_msg: Option<String> = None;
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
                    } else if pressed {
                        m.ability_enabled = !m.ability_enabled;
                        let state_str = if m.ability_enabled { "ON" } else { "OFF" };
                        game.native_set_setting(
                            &m.id,
                            "ability_enabled",
                            if m.ability_enabled { "true" } else { "false" },
                        );
                        toggled_msg = Some(format!(
                            "§d[{}]§r Active effect: §a{}",
                            m.title, state_str
                        ));
                    }
                }
            }
        }
    }
    if let Some(msg) = toggled_msg {
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
                let factor = match m.intensity {
                    1 => 5.5,
                    2 => 2.5,
                    _ => 4.0,
                };
                *zoom = Some(NativeZoom {
                    factor,
                    smooth: true,
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

    ensure_dynamic_mods_loaded(game, false);
    if let Ok(guard) = runtime().lock() {
        if let Some(m) = guard.mods.get(mod_id) {
            let hud_lbl = if m.hud_enabled { "ON" } else { "OFF" };
            let abil_lbl = if m.ability_enabled { "ON" } else { "OFF" };
            let int_lbl = match m.intensity {
                1 => "Boosted (150%)",
                2 => "Subtle (50%)",
                _ => "Normal (100%)",
            };
            buttons.push(format!("HUD Overlay & Visuals: {hud_lbl}"));
            buttons.push(format!("Active Gameplay Effect: {abil_lbl}"));
            buttons.push(format!("Effect Intensity: {int_lbl}"));
        }
    }
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

    if let Ok(mut guard) = runtime().lock() {
        if let Some(m) = guard.mods.get_mut(mod_id) {
            match button {
                0 => {
                    m.hud_enabled = !m.hud_enabled;
                    game.native_set_setting(
                        mod_id,
                        "hud_enabled",
                        if m.hud_enabled { "true" } else { "false" },
                    );
                }
                1 => {
                    m.ability_enabled = !m.ability_enabled;
                    game.native_set_setting(
                        mod_id,
                        "ability_enabled",
                        if m.ability_enabled { "true" } else { "false" },
                    );
                }
                2 => {
                    m.intensity = (m.intensity + 1) % 3;
                    game.native_set_setting(mod_id, "intensity", &m.intensity.to_string());
                }
                _ => {}
            }
        }
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

    if let Ok(mut guard) = runtime().lock() {
        if guard.thunder_rumble_ticks > 0 {
            guard.thunder_rumble_ticks -= 1;
        }
        let has_thunder_mod = guard.mods.values().any(|m| {
            game.native_on(&m.id)
                && m.ability_enabled
                && m.features.contains("immersive_thunder")
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
    if guard.thunder_rumble_ticks == 0 {
        return;
    }
    let Some(thunder_mod) = guard.mods.values().find(|m| {
        game.native_on(&m.id) && m.ability_enabled && m.features.contains("immersive_thunder")
    }) else {
        return;
    };
    let mult = match thunder_mod.intensity {
        1 => 1.5,
        2 => 0.5,
        _ => 1.0,
    };
    let decay = (guard.thunder_rumble_ticks as f32 / 36.0).clamp(0.0, 1.0) * mult;
    let phase = (game.world.game_time as f32 + partial) * 1.9;
    shake.pitch += phase.sin() * 0.65 * decay;
    shake.yaw += (phase * 1.37).cos() * 0.55 * decay;
    shake.roll += (phase * 2.1).sin() * 0.45 * decay;
}

pub(super) fn native_hud(game: &mut Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    ensure_dynamic_mods_loaded(game, false);
    if !game.native_corner_shown(horizon_client::extras::Corner::TopLeft) {
        return;
    }

    let mut hud_lines: Vec<(String, u32)> = Vec::new();
    let held_item = game
        .player
        .as_ref()
        .and_then(|_| game.mod_held_item_id());

    if let Ok(guard) = runtime().lock() {
        for m in guard.mods.values() {
            if !game.native_on(&m.id) || !m.hud_enabled {
                continue;
            }
            if m.features.contains("appleskin_hud") {
                if let Some(p) = &game.player {
                    let food = p.health.food.food.max(0);
                    let sat = p.health.food.saturation.max(0.0);
                    let hp = p.health.health();
                    let extra = held_item
                        .as_deref()
                        .and_then(estimate_food_values)
                        .map(|(h, s)| format!(" (+{h} Food / +{s:.1} Sat)"))
                        .unwrap_or_default();
                    hud_lines.push((
                        format!("AppleSkin • Hunger {food}/20 • Sat {sat:.1} • HP {hp:.0}{extra}"),
                        0xFFD54F,
                    ));
                }
                continue;
            }
            if m.features.contains("jade_hud") {
                if let Some((eye, dir)) = game.aim() {
                    if let Some(hit) = game.clip_ray(eye, dir, 5.5) {
                        let block_name = game.native_block(hit.pos);
                        if block_name != "minecraft:air" {
                            let short = block_name
                                .strip_prefix("minecraft:")
                                .unwrap_or(&block_name)
                                .replace('_', " ");
                            let facing = game
                                .native_block_property(hit.pos, "facing")
                                .map(|f| format!(" [{f}]"))
                                .unwrap_or_default();
                            hud_lines.push((
                                format!(
                                    "{} • {}{} ({}, {}, {})",
                                    m.title.split_whitespace().next().unwrap_or(&m.id),
                                    short,
                                    facing,
                                    hit.pos[0],
                                    hit.pos[1],
                                    hit.pos[2]
                                ),
                                0x80CBC4,
                            ));
                        }
                    }
                }
                continue;
            }
            if m.features.contains("immersive_thunder") {
                let (rain, thunder) = game.native_weather();
                let status = if thunder > 0.1 {
                    format!("{} • Storm Active ({:.0}%)", m.title, thunder * 100.0)
                } else if rain > 0.1 {
                    format!("{} • Rain Acoustics ({:.0}%)", m.title, rain * 100.0)
                } else {
                    format!("{} • Ready (Clear Sky)", m.title)
                };
                hud_lines.push((status, 0x90CAF9));
                continue;
            }
            hud_lines.push((format!("{} • Active", m.title), 0xD0BCFF));
        }
    }

    for (label, color_rgb) in hud_lines.into_iter().take(5) {
        let w = game.native_text_width(&label) + 10.0;
        let (x, y) = corners.place(horizon_client::extras::Corner::TopLeft, w, 14.0);
        game.native_fill(batch, [x, y, w, 14.0], 0x7214121E);
        game.native_text(batch, &label, [x + 5.0, y + 3.0], color_rgb, true);
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

    let has_appleskin = guard
        .mods
        .values()
        .any(|m| game.native_on(&m.id) && m.features.contains("appleskin_hud"));
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

    if let Some((ns, _)) = stack.item.split_once(':') {
        if ns != "minecraft" {
            if let Some(m) = guard.mods.get(ns) {
                if game.native_on(&m.id) {
                    lines.push((
                        horizon_client::text::Text::literal(format!(
                            "§d[{}]§7 Horizon Native Mod Item",
                            m.title
                        )),
                        0xD0BCFF,
                    ));
                }
            }
        }
    }
}

pub(super) fn native_use_item_on(
    game: &mut Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    _face: [i32; 3],
    claimed: &mut bool,
) {
    if *claimed {
        return;
    }
    ensure_dynamic_mods_loaded(game, false);
    let has_wrench = runtime().lock().ok().is_some_and(|g| {
        g.mods
            .values()
            .any(|m| game.native_on(&m.id) && m.ability_enabled && m.features.contains("wrench"))
    });
    if has_wrench
        && (stack.item.as_ref() == "minecraft:stick"
            || stack.item.as_ref() == "minecraft:blaze_rod"
            || stack.item.as_ref() == "minecraft:breeze_rod"
            || stack.item.as_ref() == "minecraft:copper_ingot"
            || stack.item.contains("wrench"))
    {
        if let Some(cur_facing) = game.native_block_property(pos, "facing") {
            let next = match cur_facing.as_str() {
                "north" => "east",
                "east" => "south",
                "south" => "west",
                "west" => "north",
                "up" => "down",
                "down" => "up",
                other => other,
            };
            if next != cur_facing && game.native_set_block_property(pos, "facing", next) {
                *claimed = true;
                return;
            }
        }
        if let Some(cur_axis) = game.native_block_property(pos, "axis") {
            let next = match cur_axis.as_str() {
                "x" => "y",
                "y" => "z",
                "z" => "x",
                other => other,
            };
            if next != cur_axis && game.native_set_block_property(pos, "axis", next) {
                *claimed = true;
            }
        }
    }
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
        .filter(|m| game.native_on(&m.id) && m.ability_enabled);
    if active_mod.is_some() {
        if let Some(p) = game.native_player() {
            game.native_flame_particle([p.x, p.y + 1.1, p.z], false);
        }
    }
}

pub(super) fn native_destroy_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    state: horizon_world::registry::BlockStateId,
    _canceled: &mut bool,
) {
    ensure_dynamic_mods_loaded(game, false);
    let should_vein = {
        let Ok(mut guard) = runtime().lock() else {
            return;
        };
        if guard.vein_guard {
            return;
        }
        let has_vein = guard.mods.values().any(|m| {
            game.native_on(&m.id) && m.ability_enabled && m.features.contains("veinminer")
        });
        if !has_vein {
            return;
        }
        guard.vein_guard = true;
        true
    };
    if !should_vein {
        return;
    }

    let block_name = game.res.facts().get(state).name.to_string();
    let is_vein_target = block_name.ends_with("_ore")
        || block_name.ends_with("_log")
        || block_name.ends_with("_stem")
        || block_name.ends_with("_wood")
        || block_name.contains("tree")
        || block_name == "minecraft:ancient_debris";

    if is_vein_target {
        if let Some(air_state) = game.pack.assets.registry.default_state("minecraft:air") {
            let dim = game.dim.id().to_owned();
            let mut broken = 0usize;
            for dx in -1..=1 {
                for dy in -1..=2 {
                    for dz in -1..=1 {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }
                        if broken >= 12 {
                            break;
                        }
                        let np = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
                        if game.native_block(np) == block_name {
                            if game.native_set_block(&dim, np, air_state, flags::UPDATE_ALL) {
                                broken += 1;
                            }
                        }
                    }
                }
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
    let has_dyn_lights = guard.mods.values().any(|m| {
        game.native_on(&m.id) && m.ability_enabled && m.features.contains("dynamic_lights")
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

    let has_jade = guard
        .mods
        .values()
        .any(|m| game.native_on(&m.id) && m.hud_enabled && m.features.contains("jade_hud"));
    if has_jade {
        if let Some((eye, dir)) = game.aim() {
            if let Some(hit) = game.clip_ray(eye, dir, 5.5) {
                rims.push(NativeBlockRim {
                    pos: hit.pos,
                    argb: 0xAA80CBC4,
                    width: 2.0,
                });
            }
        }
    }
}

impl Game<'_> {
    fn mod_held_item_id(&self) -> Option<String> {
        self.inventory
            .get(self.selected)
            .and_then(|slot| slot.as_ref())
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
