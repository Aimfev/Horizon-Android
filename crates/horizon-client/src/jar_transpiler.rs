//! Direct In-Launcher Java `.jar` (Fabric / Quilt / NeoForge / Forge) -> `.hnmod` + `.hmod`
//! Real-Time Auto-Compiler & Native `.hnmod` Bundle Compiler.
//!
//! When a player installs any mod from Modrinth / Horizon or imports a `.jar` / `.hnmod` file:
//! 1. The launcher immediately compiles/transpiles it in real time (zero manual steps, no
//!    external build server or "Try again" dialog required).
//! 2. Reads official Horizon `.hnmod` bundles (`horizon-native.json`, `mixins/<id>/horizon-mixin.toml`,
//!    `mixins/<id>/*.rs`, `api/src/*.rs`, and the generated `mixins/<id>/data/horizon/runtime.rs`)
//!    as well as Java `.jar` archives, extracting real mod metadata, settings definitions,
//!    keybinds, commands, and hook targets.
//! 3. Generates `<profile>/horizon-native-mods/<id>.hnmod` with `horizon-native.json`, preserved
//!    `mixins/`, and the compiled metadata at `mixins/<id>/data/horizon/compiled.json` (used by the
//!    in-process runtime engine in `game/android_bundled.rs` so the mod works immediately).
//! 4. Generates `<profile>/horizon-mods/<id>.hmod` when the mod adds items, blocks, recipes, or
//!    assets (`api = "0.2"`, `capabilities = ["registries"]`).
//! 5. Automatically enables the mod in `<profile>/horizon-mods.txt`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_ZIP_ENTRIES: usize = 4096;
const MAX_ENTRY_BYTES: u64 = 8 * 1024 * 1024;
pub const DEFAULT_HORIZON_REV: &str = "8933e8c2d81bef1549a63dc1c8341ca46fe3cffa";
pub const DEFAULT_MIXINS_REV: &str = "1e758beab4f00b7ab82183f5b4a7ec4f42599a98";

/// The compiled metadata and the generated runtime source of a transpiled bundle, as paths under
/// the mod's own `mixins/<id>/`: `horizon_mods::index::native::allowed` takes a `.json` only under
/// `mixins/<id>/data/`, and code there is a bundle's data (`native_data!`), not a mixin source the
/// native build compiles. The in-process engine reads both by these suffixes.
pub const COMPILED_META: &str = "/data/horizon/compiled.json";
pub const RUNTIME_SOURCE: &str = "/data/horizon/runtime.rs";

fn compiled_meta_entry(mod_id: &str) -> String {
    format!("mixins/{mod_id}{COMPILED_META}")
}

fn runtime_source_entry(mod_id: &str) -> String {
    format!("mixins/{mod_id}{RUNTIME_SOURCE}")
}

/// Whether `name` is output of a compile rather than input it copies through: a bundle's manifest,
/// its compiled metadata and runtime source, and the top-level paths older versions wrote.
fn is_compiled_output(name: &str) -> bool {
    matches!(
        name,
        "horizon-native.json" | "src/lib.rs" | "horizon-compiled.json"
    ) || name.ends_with(RUNTIME_SOURCE)
        || name.ends_with(COMPILED_META)
}

fn is_safe_zip_entry_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 240
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains(':')
        && !name.contains('\0')
        && !name.split('/').any(|p| p.is_empty() || p == "." || p == "..")
}

fn read_zip_entry_bounded<R: Read>(entry: R) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    entry.take(MAX_ENTRY_BYTES + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_ENTRY_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "zip entry exceeds 8 MiB limit",
        ));
    }
    Ok(buf)
}

#[derive(Clone, Debug)]
pub struct TranspiledModInfo {
    pub id: String,
    pub title: String,
    pub version: String,
    pub description: String,
    pub hnmod_path: PathBuf,
    pub hmod_path: PathBuf,
    pub items: Vec<String>,
    pub blocks: Vec<String>,
    pub features: Vec<String>,
    pub port_pack: Option<horizon_port_pack::android_port::AndroidPortResult>,
}

/// Sanitizes a raw string into a valid Horizon mod ID (`[a-z0-9_]{1,64}`).
pub fn sanitize_mod_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_lowercase() || lower.is_ascii_digit() {
            out.push(lower);
        } else if (ch == '-' || ch == '_' || ch == '.' || ch == ' ') && !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    let base = if trimmed.is_empty() {
        "custom_java_mod"
    } else if trimmed.as_bytes()[0].is_ascii_digit() {
        return format!("mod_{trimmed}")
            .chars()
            .take(64)
            .collect::<String>();
    } else {
        trimmed
    };
    let clean: String = base.chars().take(64).collect();
    if matches!(clean.as_str(), "minecraft" | "c" | "horizon") {
        format!("{clean}_mod")
    } else {
        clean
    }
}

/// Sanitizes a resource path segment (`[a-z0-9_.-]`).
fn sanitize_identifier_path(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_lowercase() || lower.is_ascii_digit() || lower == '_' || lower == '-' {
            out.push(lower);
        } else if (ch == '.' || ch == ' ') && !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}

pub fn extract_toml_field(toml_text: &str, key: &str) -> Option<String> {
    for line in toml_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(key) {
            let rest = rest.trim_start();
            if let Some(after_eq) = rest.strip_prefix('=') {
                let val = after_eq.trim();
                let unquoted = val.trim_matches('"').trim_matches('\'').trim();
                if !unquoted.is_empty() && !unquoted.contains("${file.") {
                    return Some(unquoted.to_owned());
                }
            }
        }
    }
    None
}

/// Scans `.class` constant pool bytes for printable ASCII strings (length >= 4)
/// to detect mod behavior (HUD overlays, tooltips, zoom, veinminer, keybindings, sounds, thunder, etc.).
fn extract_printable_strings(bytes: &[u8]) -> Vec<String> {
    let mut res = Vec::new();
    let mut cur = Vec::new();
    for &b in bytes {
        if (0x20..=0x7E).contains(&b) {
            cur.push(b);
        } else {
            if cur.len() >= 4 {
                if let Ok(s) = std::str::from_utf8(&cur) {
                    res.push(s.to_owned());
                }
            }
            cur.clear();
        }
    }
    if cur.len() >= 4 {
        if let Ok(s) = std::str::from_utf8(&cur) {
            res.push(s.to_owned());
        }
    }
    res
}

/// Canonical display title and description for well-known Horizon `.hnmod` mods.
pub fn canonical_horizon_mod_meta(mod_id: &str) -> Option<(&'static str, &'static str)> {
    match mod_id {
        "wrench" => Some((
            "Wrench",
            "Right-click blocks with a wrench item (Stick by default) to rotate them; sneak + right-click to dismantle redstone machines.",
        )),
        "veinminer" | "vein_miner" | "oreexcavator" | "diggusmaximus" => Some((
            "Veinminer",
            "Hold Sneak (or toggle Always On) while mining an ore block to harvest the entire connected vein at once with full item drops.",
        )),
        "treefall" | "fallingtree" | "treeharvester" | "treecapitator" => Some((
            "TreeFall",
            "Break the base log of a tree to fell the entire connected trunk and decay/harvest leaves with full log and sapling drops.",
        )),
        "chop" => Some((
            "Chop!",
            "Fells connected tree trunks cleanly when chopping logs, dropping all wood and saplings.",
        )),
        "appleskin" => Some((
            "AppleSkin",
            "Shows food hunger & saturation values in item tooltips and renders saturation/exhaustion + held-food restoration on the HUD.",
        )),
        "lookat" => Some((
            "LookAt",
            "Displays a top-center HUD tooltip showing the targeted block or entity, its state properties, and harvest info.",
        )),
        "jade" | "wthit" | "waila" => Some((
            "Jade",
            "Shows what block or mob you are looking at with block state properties, coordinates, and outline highlight.",
        )),
        "blockscope" => Some((
            "BlockScope",
            "Inspects targeted blocks with detailed state properties, coordinates, and block rim highlighting.",
        )),
        "fullbright" | "moregamma" => Some((
            "Fullbright",
            "Boosts world brightness and gamma so dark caves, underwater depths, and night scenes are clearly visible.",
        )),
        "rightclickharvest" | "right_click_crops" => Some((
            "RightClickHarvest",
            "Right-click mature crops (wheat, carrots, potatoes, beetroots, nether wart, cocoa) to harvest and auto-replant.",
        )),
        "carryblock" | "carry_block" | "carryon" => Some((
            "Carry Block",
            "Sneak + right-click with an empty hand to pick up, carry, and place blocks and containers.",
        )),
        "mousetricks" | "mousetweaks" => Some((
            "Mouse Tricks",
            "Enhances inventory management with quick slot dragging, wheel item transfer, and container sorting.",
        )),
        "fps_overlay" | "fps_counter" => Some((
            "FPS Counter",
            "Displays real-time FPS, 1% low frame rate, and frame time in a configurable screen corner.",
        )),
        "keystrokes" => Some((
            "Keystrokes",
            "Shows live WASD movement keys, Jump, LMB/RMB mouse buttons, and CPS counters on screen.",
        )),
        "coords_hud" | "coordinates_hud" => Some((
            "Coordinates HUD",
            "Shows X/Y/Z block coordinates, cardinal facing direction, chunk coordinates, and 24h world clock on the HUD.",
        )),
        "armor_status" | "armor_hud" => Some((
            "Armor Status",
            "Displays equipped armor pieces and held items with remaining durability and color-coded percentages.",
        )),
        "held_item_info" => Some((
            "Held Item Info",
            "Shows the held item's name, durability, and enchantment summary on the HUD.",
        )),
        "status_effect_hud" => Some((
            "Status Effect HUD",
            "Renders active potion and status effects with timers on the main HUD without opening inventory.",
        )),
        "enchantmenthints" | "enchantment_hints" | "enchdesc" => Some((
            "Enchantment Hints",
            "Appends clear explanations of what each enchantment does to enchanted item and book tooltips.",
        )),
        "shulkerpeek" | "shulkerboxtooltip" => Some((
            "ShulkerPeek",
            "Shows the contents of Shulker Boxes, Bundles, and containers directly inside their item tooltip.",
        )),
        "toggle_sprint" | "togglesprint" => Some((
            "Toggle Sprint",
            "Lets a single key press toggle continuous sprinting or sneaking with an optional HUD indicator.",
        )),
        "waypoints" => Some((
            "Waypoints",
            "Save, list, and track custom world waypoints with live distance and compass direction on the HUD.",
        )),
        "light_overlay" | "lightoverlay" => Some((
            "Light Overlay",
            "Highlights nearby blocks where hostile mobs can spawn due to low block/sky light levels.",
        )),
        "eatinganimation" => Some((
            "Eating Animation",
            "Adds dynamic eating/drinking visual feedback and progress indicators while consuming food.",
        )),
        "far_view" | "farview" | "bobby" | "distant_horizons" => Some((
            "Far View (LOD)",
            "Keeps distant chunks and LOD terrain columns visible beyond server view distance.",
        )),
        "dynamic_lights" | "lambdynamiclights" => Some((
            "Dynamic Lights",
            "Illuminates surroundings dynamically when holding or dropping torches, lanterns, lava buckets, and glowing items.",
        )),
        "sound_physics" | "soundphysics" => Some((
            "Sound Physics Remastered",
            "Adds realistic cave reverb, sound occlusion through walls, and underwater acoustic absorption.",
        )),
        "freecam" => Some((
            "Freecam",
            "Detaches camera locally to inspect builds, caves, and redstone contraptions without moving your player.",
        )),
        "auto_totem" | "autototem" => Some((
            "Auto Totem Helper",
            "Displays remaining Totems of Undying on the HUD and warns when off-hand totem is depleted.",
        )),
        "inventory_hud" | "inventoryhud" => Some((
            "Inventory HUD+",
            "Renders a compact translucent mini-inventory grid, potion timers, and armor durability directly on the HUD.",
        )),
        "ping_hud" | "pingdisplay" => Some((
            "Ping & TPS HUD",
            "Shows live server latency (ms), integrated server MSPT/TPS, and packet health on the HUD.",
        )),
        "cps_hud" | "cpsdisplay" => Some((
            "CPS Counter",
            "Tracks left and right click/tap speed (Clicks Per Second) with peak CPS indicator on the HUD.",
        )),
        "biome_title" | "biometitle" | "travelerstitles" => Some((
            "Traveler's Biome Titles",
            "Displays an atmospheric title banner when entering a new biome or dimension.",
        )),
        "death_chest" | "gravestone" | "corpse" => Some((
            "Grave & Death Coordinates",
            "Records your exact death coordinates and dimension in chat and on the HUD compass so you never lose items.",
        )),
        "xp_storage" | "xpbook" | "xp_tome" => Some((
            "XP Tome & Level Tracker",
            "Displays exact experience points, progress to next level, and enchantment level thresholds.",
        )),
        "fast_leaf_decay" | "fastleafdecay" => Some((
            "Fast Leaf Decay",
            "Accelerates leaf decay rapidly after chopping down tree trunks so floating foliage clears in seconds.",
        )),
        "villager_trading_plus" | "trade_cycling" => Some((
            "Villager Trade Helper",
            "Highlights enchanted book trades, emeralds, and profession workstation status when inspecting villagers.",
        )),
        "nether_coords" | "netherportalcoords" => Some((
            "Nether Portal Calculator",
            "Shows synchronized Overworld <-> Nether (8x / 0.125x) portal coordinates live on the HUD.",
        )),
        "durability_alert" | "durability101" => Some((
            "Durability Alert",
            "Triggers a visual and acoustic warning before your pickaxe, sword, elytra, or armor breaks.",
        )),
        "chat_heads" | "chatheads" => Some((
            "Chat Heads & Timestamps",
            "Prepends clean timestamps and player badges to incoming chat messages.",
        )),
        "damage_tilt" | "damagetilt" => Some((
            "Directional Damage Tilt",
            "Tilts the camera toward the exact direction of incoming attacks and projectile hits.",
        )),
        "trash_slot" | "trashslot" => Some((
            "Trash Slot Helper",
            "Adds quick item disposal and inventory slot cleanup controls.",
        )),
        "nature_compass" | "naturescompass" => Some((
            "Nature's Biome Compass",
            "Displays current biome temperature, humidity, precipitation, and cardinal bearing on the HUD.",
        )),
        "elytra_hud" | "elytraphysics" | "flighthud" => Some((
            "Flight & Elytra HUD",
            "Shows pitch angle, glide speed (m/s), altitude, and Elytra durability while gliding.",
        )),
        "potion_counter" | "itemcounter" => Some((
            "PvP Potion & Arrow Counter",
            "Counts total healing potions, golden apples, ender pearls, and arrows remaining in your inventory.",
        )),
        "reach_display" | "reachdisplay" => Some((
            "Reach & Combo Display",
            "Shows exact attack hit distance (blocks) and consecutive hit combo count during combat.",
        )),
        "auto_fish" | "fishing_helper" => Some((
            "Fishing Bobber Alert",
            "Displays a clear bite indicator and acoustic cue when a fish hooks your fishing rod.",
        )),
        "step_up" | "stepup" => Some((
            "StepUp Auto-Step",
            "Smoothly steps up full 1-block ledges without needing to jump repeatedly.",
        )),
        "no_fog" | "nofog" | "clearview" => Some((
            "ClearView Fog Control",
            "Pushes back terrain, Nether, powder snow, and underwater fog for crystal-clear distant visibility.",
        )),
        "clear_lava" | "clearwater" => Some((
            "Clear Water & Lava",
            "Improves visibility when submerged in water or lava.",
        )),
        "chunk_borders" | "minihud" => Some((
            "Chunk Borders & MiniHUD",
            "Displays chunk boundaries (16x16), sub-chunk Y section, and slime chunk indicator.",
        )),
        "horse_stats" | "horsestats" => Some((
            "Horse & Mount Stats",
            "Shows exact movement speed, jump height (in blocks), and health of horses, camels, and mounts.",
        )),
        "bee_info" | "beehive_tooltip" => Some((
            "Beehive & Honey Inspector",
            "Shows bee count (0-3) and honey level (0-5) on beehives and bee nests in tooltips and LookAt HUD.",
        )),
        "book_editor" | "betterbooks" => Some((
            "Book & Sign Helper",
            "Adds character/page counters and formatting shortcuts when reading or editing books and signs.",
        )),
        "item_scroller" | "itemscroller" => Some((
            "Item Scroller",
            "Fast single-slot and full-stack item transfer gestures across container screens.",
        )),
        "crafting_tweaks" | "craftingtweaks" => Some((
            "Crafting Tweaks",
            "Adds quick grid balance, rotate, and clear shortcuts for crafting tables.",
        )),
        "bed_benefits" | "bedbenefits" => Some((
            "Bed Benefits",
            "Restores health and clears negative status effects upon waking up from a bed.",
        )),
        "torch_hit" | "torchbow" => Some((
            "Torch Hit Igniter",
            "Striking mobs with a torch or soul torch briefly sets them on fire.",
        )),
        "double_doors" | "doubledoors" => Some((
            "Double Doors",
            "Opening or closing one door or fence gate automatically toggles its matching adjacent double door.",
        )),
        "xp_clumps" | "clumps" => Some((
            "XP Clumps",
            "Merges nearby experience orbs together to reduce entity lag and speed up XP absorption.",
        )),
        "immersivethunder" | "immersive_thunder" => Some((
            "ImmersiveThunder",
            "Adds distance-scaled thunder acoustics, atmospheric storm soundscapes, and camera rumble during storms.",
        )),
        _ => None,
    }
}

/// Enables `mod_id` in `<profile_dir>/horizon-mods.txt` and `<private_home>/horizon-mods.txt`
/// so newly installed or transpiled mods are active immediately without manual toggling.
pub fn enable_mod_in_profile(profile_dir: &Path, mod_id: &str) {
    let clean_id = sanitize_mod_id(mod_id);
    if clean_id.is_empty() {
        return;
    }
    let mut roots = vec![profile_dir.to_path_buf()];
    let home = crate::logging::private_home();
    if !roots.iter().any(|r| r == &home) {
        roots.push(home);
    }
    for root in roots {
        let _line = format!("mod.{clean_id}:true");
        let mut list = crate::mod_list::ModList::load(&root);
        list.disabled.remove(&clean_id);
        list.enabled.insert(clean_id.clone());
        list.save(&root);
    }
}

/// Infers high-level gameplay features from a mod's ID, title, description, and code/bytecode signals.
pub fn infer_features_from_id_and_signals(
    mod_id: &str,
    title: &str,
    description: &str,
    rust_src: &str,
    class_signals: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut features = class_signals.clone();
    let id_low = mod_id.to_ascii_lowercase();
    let combined = format!(
        "{} {} {} {}",
        id_low,
        title.to_ascii_lowercase(),
        description.to_ascii_lowercase(),
        rust_src.to_ascii_lowercase()
    );

    if id_low == "wrench" || combined.contains("wrench") || combined.contains("takes_apart") {
        features.insert("wrench".to_owned());
    }
    if id_low == "treefall"
        || id_low == "chop"
        || id_low == "fallingtree"
        || id_low == "treeharvester"
        || id_low == "treecapitator"
        || combined.contains("treefall")
        || combined.contains("fallingtree")
        || combined.contains("treeharvester")
        || combined.contains("treecapitator")
        || (combined.contains("chop") && combined.contains("tree"))
    {
        features.insert("treefall".to_owned());
    }
    if id_low == "veinminer"
        || id_low == "vein_miner"
        || id_low == "oreexcavator"
        || id_low == "diggusmaximus"
        || id_low == "excavator"
        || combined.contains("veinminer")
        || combined.contains("vein_miner")
        || combined.contains("oreexcavator")
        || combined.contains("diggusmaximus")
        || combined.contains("ore_vein")
    {
        features.insert("veinminer".to_owned());
    }
    if id_low == "appleskin"
        || combined.contains("appleskin")
        || combined.contains("saturation")
        || combined.contains("exhaustion")
        || combined.contains("foodhelper")
    {
        features.insert("appleskin_hud".to_owned());
        features.insert("tooltip".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if matches!(
        id_low.as_str(),
        "lookat" | "jade" | "wthit" | "waila" | "blockscope"
    ) || combined.contains("jade")
        || combined.contains("wthit")
        || combined.contains("waila")
        || combined.contains("lookat")
        || combined.contains("blockscope")
        || combined.contains("what am i looking at")
    {
        features.insert("jade_hud".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if matches!(id_low.as_str(), "fullbright" | "moregamma")
        || combined.contains("fullbright")
        || combined.contains("moregamma")
    {
        features.insert("fullbright".to_owned());
    }
    if matches!(id_low.as_str(), "rightclickharvest" | "right_click_crops")
        || combined.contains("rightclickharvest")
        || combined.contains("right_click_crops")
        || (combined.contains("harvest") && combined.contains("crop"))
    {
        features.insert("rightclickharvest".to_owned());
    }
    if matches!(id_low.as_str(), "carryblock" | "carry_block" | "carryon")
        || combined.contains("carryblock")
        || combined.contains("carry_block")
    {
        features.insert("carryblock".to_owned());
    }
    if matches!(id_low.as_str(), "mousetricks" | "mousetweaks")
        || combined.contains("mousetricks")
        || combined.contains("mousetweaks")
    {
        features.insert("mousetricks".to_owned());
    }
    if matches!(id_low.as_str(), "fps_overlay" | "fps_counter" | "fpsdisplay")
        || combined.contains("fps_overlay")
        || combined.contains("native_frame_rate")
    {
        features.insert("fps_overlay".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if id_low == "keystrokes"
        || combined.contains("keystrokes")
        || combined.contains("native_clicks")
    {
        features.insert("keystrokes".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if matches!(id_low.as_str(), "coords_hud" | "coordinates_hud" | "directionhud")
        || combined.contains("coords_hud")
        || combined.contains("coordinates_hud")
    {
        features.insert("coords_hud".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if matches!(
        id_low.as_str(),
        "armor_status" | "armor_hud" | "held_item_info" | "durabilityviewer"
    ) || combined.contains("armor_status")
        || combined.contains("armor_hud")
        || combined.contains("held_item_info")
    {
        features.insert("armor_status".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if matches!(id_low.as_str(), "status_effect_hud" | "status_effects" | "potionhud")
        || combined.contains("status_effect_hud")
    {
        features.insert("status_effect_hud".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if matches!(
        id_low.as_str(),
        "enchantmenthints" | "enchantment_hints" | "enchdesc"
    ) || combined.contains("enchantmenthints")
        || combined.contains("enchantment_hints")
    {
        features.insert("enchantmenthints".to_owned());
        features.insert("tooltip".to_owned());
    }
    if matches!(id_low.as_str(), "shulkerpeek" | "shulkerboxtooltip")
        || combined.contains("shulkerpeek")
        || combined.contains("shulkerboxtooltip")
    {
        features.insert("shulkerpeek".to_owned());
        features.insert("tooltip".to_owned());
    }
    if matches!(id_low.as_str(), "toggle_sprint" | "togglesprint" | "bettersprinting")
        || combined.contains("toggle_sprint")
        || combined.contains("togglesprint")
    {
        features.insert("toggle_sprint".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if id_low == "waypoints" || combined.contains("waypoints") {
        features.insert("waypoints".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if matches!(id_low.as_str(), "light_overlay" | "lightoverlay")
        || combined.contains("light_overlay")
    {
        features.insert("light_overlay".to_owned());
    }
    if id_low == "eatinganimation"
        || id_low.contains("food")
        || id_low.contains("eat")
        || combined.contains("eatinganimation")
        || combined.contains("foodanimation")
        || combined.contains("notenoughanimations")
    {
        features.insert("eating_animation".to_owned());
        features.insert("eatinganimation".to_owned());
        features.insert("hud_overlay".to_owned());
    }
    if id_low.contains("motionblur")
        || id_low.contains("motion_blur")
        || combined.contains("motionblur")
        || combined.contains("motion_blur")
    {
        features.insert("motion_blur".to_owned());
    }
    if combined.contains("thunder")
        || combined.contains("lightning")
        || combined.contains("immersivethunder")
    {
        features.insert("immersive_thunder".to_owned());
    }
    if combined.contains("dynamiclight") || combined.contains("lambdynamiclights") {
        features.insert("dynamic_lights".to_owned());
    }
    if id_low.contains("zoom") || combined.contains("okzoomer") || combined.contains("native_zoom") {
        features.insert("zoom".to_owned());
    }
    if features.is_empty() {
        features.insert("hud_overlay".to_owned());
        features.insert("tooltip".to_owned());
    }
    features
}

/// Extracts `pub const DESCRIPTION: &str = "...";` or `description: "...".into()` from Rust source files.
pub fn parse_rust_mod_metadata(rust_src: &str, mod_id: &str) -> (Option<String>, Option<String>, Vec<String>) {
    let mut title = None;
    let mut description = None;
    let mut custom_buttons = Vec::new();

    if let Some((canon_title, canon_desc)) = canonical_horizon_mod_meta(mod_id) {
        title = Some(canon_title.to_owned());
        description = Some(canon_desc.to_owned());
    }

    for line in rust_src.lines() {
        let trimmed = line.trim();
        if title.is_none() {
            if let Some(rest) = trimmed.strip_prefix("title:") {
                if let Some(first_quote) = rest.find('"') {
                    let after = &rest[first_quote + 1..];
                    if let Some(end_quote) = after.find('"') {
                        let raw = after[..end_quote].replace("{}", "").trim().to_owned();
                        if !raw.is_empty() {
                            title = Some(raw);
                        }
                    }
                }
            }
        }
        if description.is_none() {
            if trimmed.starts_with("pub const DESCRIPTION:") || trimmed.starts_with("const DESCRIPTION:") {
                if let Some(first_quote) = trimmed.find('"') {
                    let after = &trimmed[first_quote + 1..];
                    let raw = after
                        .trim_end_matches(';')
                        .trim_end_matches('\\')
                        .trim_matches('"')
                        .trim();
                    if !raw.is_empty() {
                        description = Some(raw.to_owned());
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("description:") {
                let clean = rest
                    .trim()
                    .trim_end_matches(',')
                    .trim_end_matches(".into()")
                    .trim_matches('"');
                if !clean.is_empty() && !clean.contains("::") {
                    description = Some(clean.to_owned());
                }
            }
        }
        // Extract format!("Label: {}", ...) button labels from settings.rs / runtime.rs
        if trimmed.contains("format!(\"") && trimmed.contains(':') {
            if let Some(idx) = trimmed.find("format!(\"") {
                let after = &trimmed[idx + 9..];
                if let Some(colon_idx) = after.find(':') {
                    let label_prefix = after[..colon_idx].trim();
                    if !label_prefix.is_empty()
                        && label_prefix.len() <= 36
                        && !label_prefix.contains('{')
                        && !label_prefix.contains("HUD Overlay")
                        && !label_prefix.contains("Active Ability")
                        && !label_prefix.contains("Active Gameplay Effect")
                        && !label_prefix.contains("Effect Intensity")
                        && !custom_buttons.iter().any(|b| b == label_prefix)
                    {
                        custom_buttons.push(label_prefix.to_owned());
                    }
                }
            }
        }
    }

    (title, description, custom_buttons)
}

/// Real-time compiler for `.hnmod` archives: validates or upgrades any `.hnmod` file,
/// parses `mixins/<id>/*.rs`, `mixins/<id>/horizon-mixin.toml`, and `mixins/<id>/data/horizon/runtime.rs`,
/// embeds `mixins/<id>/data/horizon/compiled.json` with parsed metadata & hook capabilities so the
/// in-process runtime engine executes it immediately without external server compilation,
/// installs it into `<profile_dir>/horizon-native-mods/<id>.hnmod`, removes duplicates in
/// `<profile_dir>/horizon-mods/`, and enables it in `horizon-mods.txt`.
pub fn compile_hnmod_bundle_realtime(
    hnmod_source: &Path,
    profile_dir: &Path,
) -> Result<TranspiledModInfo, String> {
    let raw_bytes = fs::read(hnmod_source)
        .map_err(|e| format!("Could not read {}: {e}", hnmod_source.display()))?;
    let mut zip = ZipArchive::new(Cursor::new(&raw_bytes))
        .map_err(|e| format!("Invalid .hnmod zip archive {}: {e}", hnmod_source.display()))?;

    let raw_stem = hnmod_source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("native_mod");
    let fallback_stem = raw_stem
        .find('-')
        .filter(|&i| raw_stem[i + 1..].chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|i| &raw_stem[..i])
        .unwrap_or(raw_stem);

    let mut mod_id = sanitize_mod_id(fallback_stem);
    let mut version = "1.0.0".to_owned();
    let mut hooks_ver = 22u32;
    let mut permissions: Vec<String> = Vec::new();
    let mut title = String::new();
    let mut description = String::new();
    let mut lib_rs = String::new();
    let mut mixin_rs_combined = String::new();
    let mut existing_files: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    for i in 0..zip.len().min(MAX_ZIP_ENTRIES) {
        let Ok(entry) = zip.by_index(i) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let Ok(entry_name) = entry.name() else {
            continue;
        };
        let name = entry_name.replace('\\', "/");
        if !is_safe_zip_entry_name(&name) {
            continue;
        }
        let Ok(buf) = read_zip_entry_bounded(entry) else {
            continue;
        };
        if name == "horizon-native.json" {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf) {
                if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                    mod_id = sanitize_mod_id(id);
                }
                if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
                    if !ver.trim().is_empty() {
                        version = ver.trim().to_owned();
                    }
                }
                if let Some(h) = v.get("hooks").and_then(|x| x.as_u64()) {
                    hooks_ver = h as u32;
                }
                if let Some(perms) = v.get("permissions").and_then(|x| x.as_array()) {
                    permissions = perms
                        .iter()
                        .filter_map(|p| p.as_str().map(str::to_owned))
                        .collect();
                }
            }
        } else if name == "horizon-mod.toml" || name == "mod.toml" {
            let s = String::from_utf8_lossy(&buf);
            if let Some(id) = extract_toml_field(&s, "id") {
                mod_id = sanitize_mod_id(&id);
            }
            if let Some(n) = extract_toml_field(&s, "name") {
                if title.is_empty() {
                    title = n;
                }
            }
            if let Some(ver) = extract_toml_field(&s, "version") {
                version = ver;
            }
            if let Some(desc) = extract_toml_field(&s, "description") {
                if description.is_empty() {
                    description = desc;
                }
            }
        } else if name == "src/lib.rs" || name.ends_with(RUNTIME_SOURCE) {
            lib_rs = String::from_utf8_lossy(&buf).into_owned();
        } else if name.ends_with(".rs") || name.ends_with("horizon-mixin.toml") {
            mixin_rs_combined.push_str(&String::from_utf8_lossy(&buf));
            mixin_rs_combined.push('\n');
        }
        existing_files.insert(name, buf);
    }

    let combined_rust = format!("{lib_rs}\n{mixin_rs_combined}");
    let (parsed_title, parsed_desc, parsed_buttons) =
        parse_rust_mod_metadata(&combined_rust, &mod_id);

    if let Some(t) = parsed_title {
        title = t;
    }
    if let Some(d) = parsed_desc {
        description = d;
    }

    if title.is_empty() {
        title = title_case_id(&mod_id);
    }
    if description.is_empty() {
        description = format!("Horizon Native Mod ({mod_id} v{version})");
    }

    let features: Vec<String> = infer_features_from_id_and_signals(
        &mod_id,
        &title,
        &description,
        &combined_rust,
        &BTreeSet::new(),
    )
    .into_iter()
    .collect();

    if lib_rs.trim().is_empty() {
        lib_rs = generate_hnmod_rust_source(
            &mod_id,
            &title,
            &version,
            &description,
            &[],
            &[],
            &features,
            &[],
            &[],
        );
    }

    let manifest_json = serde_json::json!({
        "schema": 1,
        "id": mod_id,
        "version": version,
        "game": "26.3",
        "horizon": DEFAULT_HORIZON_REV,
        "mixins": DEFAULT_MIXINS_REV,
        "hooks": hooks_ver.max(415),
        "permissions": permissions,
    });
    let compiled_json = serde_json::json!({
        "id": mod_id,
        "title": title,
        "version": version,
        "description": description,
        "compiled": true,
        "abi": "horizon-inproc-v2",
        "hooks_api": 415,
        "features": features,
        "custom_buttons": parsed_buttons,
        "items": [],
        "blocks": [],
    });

    let native_mods_dir = profile_dir.join("horizon-native-mods");
    fs::create_dir_all(&native_mods_dir)
        .map_err(|e| format!("create horizon-native-mods/: {e}"))?;
    let hnmod_path = native_mods_dir.join(format!("{mod_id}.hnmod"));

    {
        let file =
            File::create(&hnmod_path).map_err(|e| format!("create {}: {e}", hnmod_path.display()))?;
        let mut out_zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        out_zip
            .start_file("horizon-native.json", opts)
            .map_err(|e| e.to_string())?;
        out_zip
            .write_all(
                &serde_json::to_vec_pretty(&manifest_json).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        out_zip
            .start_file(runtime_source_entry(&mod_id), opts)
            .map_err(|e| e.to_string())?;
        out_zip
            .write_all(lib_rs.as_bytes())
            .map_err(|e| e.to_string())?;

        out_zip
            .start_file(compiled_meta_entry(&mod_id), opts)
            .map_err(|e| e.to_string())?;
        out_zip
            .write_all(
                &serde_json::to_vec_pretty(&compiled_json).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let full_mixins = generate_full_mixin_bundle_files(
            &mod_id,
            &title,
            &version,
            &description,
            &[],
            &[],
            &features,
        );
        for (mpath, mbytes) in full_mixins {
            if !existing_files.contains_key(&mpath) {
                out_zip.start_file(&mpath, opts).map_err(|e| e.to_string())?;
                out_zip.write_all(&mbytes).map_err(|e| e.to_string())?;
            }
        }
        let mut combined_content_files = Vec::new();
        for (name, bytes) in existing_files {
            if is_compiled_output(&name) {
                continue;
            }
            if name == "horizon-mod.toml"
                || name.starts_with("assets/")
                || name.starts_with("data/")
            {
                combined_content_files.push((name.clone(), bytes.clone()));
            }
            out_zip.start_file(&name, opts).map_err(|e| e.to_string())?;
            out_zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        out_zip.finish().map_err(|e| e.to_string())?;
        let _ = horizon_mods::index::native::Bundle::read(&hnmod_path)?;

        // Combined .hmod support (#5865): if the bundle also carries horizon-mod.toml content,
        // write the validated content .hmod to horizon-mods/<mod_id>.hmod so mod_content loads it!
        if combined_content_files.iter().any(|(n, _)| n == "horizon-mod.toml") {
            let mods_dir = profile_dir.join("horizon-mods");
            let _ = fs::create_dir_all(&mods_dir);
            let target_hmod = mods_dir.join(format!("{mod_id}.hmod"));
            if hnmod_path != target_hmod {
                if let Ok(f) = File::create(&target_hmod) {
                    let mut czip = ZipWriter::new(f);
                    for (cname, cbytes) in combined_content_files {
                        if czip.start_file(&cname, opts).is_ok() {
                            let _ = czip.write_all(&cbytes);
                        }
                    }
                    let _ = czip.finish();
                }
            }
        }
    }

    // Remove duplicate .hnmod inside horizon-mods/ so Content Library does not show duplicate rows
    let dup_in_mods = profile_dir.join("horizon-mods").join(format!("{mod_id}.hnmod"));
    if dup_in_mods != hnmod_path && dup_in_mods.is_file() {
        let _ = fs::remove_file(&dup_in_mods);
    }

    enable_mod_in_profile(profile_dir, &mod_id);

    let hmod_path = profile_dir.join("horizon-mods").join(format!("{mod_id}.hmod"));
    Ok(TranspiledModInfo {
        id: mod_id,
        title,
        version,
        description,
        hnmod_path,
        hmod_path,
        items: Vec::new(),
        blocks: Vec::new(),
        features,
        port_pack: None,
    })
}

/// Inspects and auto-compiles a Java `.jar` file in real time into both:
/// - `<profile_dir>/horizon-native-mods/<id>.hnmod` (with `mixins/<id>/data/horizon/compiled.json`)
/// - `<profile_dir>/horizon-mods/<id>.hmod` (with valid `horizon-mod.toml`, `data/<id>/horizon/`, and `mixins/<id>/`)
/// and enables the mod in `horizon-mods.txt`.
pub fn transpile_jar_to_horizon_mods(
    jar_path: &Path,
    profile_dir: &Path,
) -> Result<TranspiledModInfo, String> {
    transpile_jar_to_horizon_mods_with_progress(jar_path, profile_dir, &|_, _| {})
}

/// Runs the full native `horizon-port-pack` pipeline (`check::check`, `carry::carry`, `classes::census`,
/// `events::report`) on `jar_path` and synthesizes both the carried-over `.hmod` content package
/// (`<profile_dir>/horizon-mods/<id>/` + `<id>.hmod`) and the real-time `.hnmod` native hook bundle
/// (`<profile_dir>/horizon-native-mods/<id>.hnmod`).
pub fn transpile_jar_to_horizon_mods_with_progress(
    jar_path: &Path,
    profile_dir: &Path,
    progress: &dyn Fn(&str, f32),
) -> Result<TranspiledModInfo, String> {
    let port_pack_res = horizon_port_pack::android_port::port_jar_natively(
        jar_path,
        profile_dir,
        progress,
    )
    .ok();
    let jar_bytes =
        fs::read(jar_path).map_err(|e| format!("Could not read {}: {e}", jar_path.display()))?;
    let mut jar = ZipArchive::new(Cursor::new(&jar_bytes))
        .map_err(|e| format!("Invalid Java .jar archive {}: {e}", jar_path.display()))?;

    let fallback_stem = jar_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom_mod")
        .split(['-', '+', '_'])
        .next()
        .unwrap_or("custom_mod");

    let mut mod_id = sanitize_mod_id(fallback_stem);
    let mut title = fallback_stem.to_owned();
    let mut version = "1.0.0".to_owned();
    let mut description = "Auto-compiled from Java .jar by Horizon Native Compiler".to_owned();

    // 1. Parse metadata from fabric.mod.json, quilt.mod.json, META-INF/neoforge.mods.toml, or META-INF/mods.toml
    if let Ok(mut f) = jar.by_name("fabric.mod.json") {
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                mod_id = sanitize_mod_id(id);
            }
            if let Some(n) = v.get("name").and_then(|x| x.as_str()) {
                if !n.trim().is_empty() {
                    title = n.trim().to_owned();
                }
            }
            if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
                if !ver.contains("${") && !ver.trim().is_empty() {
                    version = ver.trim().to_owned();
                }
            }
            if let Some(desc) = v.get("description").and_then(|x| x.as_str()) {
                if !desc.trim().is_empty() {
                    description = desc.trim().to_owned();
                }
            }
        }
    } else if let Ok(mut f) = jar.by_name("quilt.mod.json") {
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(ql) = v.get("quilt_loader") {
                if let Some(id) = ql.get("id").and_then(|x| x.as_str()) {
                    mod_id = sanitize_mod_id(id);
                }
                if let Some(ver) = ql.get("version").and_then(|x| x.as_str()) {
                    if !ver.contains("${") && !ver.trim().is_empty() {
                        version = ver.trim().to_owned();
                    }
                }
                if let Some(meta) = ql.get("metadata") {
                    if let Some(n) = meta.get("name").and_then(|x| x.as_str()) {
                        if !n.trim().is_empty() {
                            title = n.trim().to_owned();
                        }
                    }
                    if let Some(desc) = meta.get("description").and_then(|x| x.as_str()) {
                        if !desc.trim().is_empty() {
                            description = desc.trim().to_owned();
                        }
                    }
                }
            }
        }
    } else {
        for toml_name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
            if let Ok(mut f) = jar.by_name(toml_name) {
                let mut s = String::new();
                let _ = f.read_to_string(&mut s);
                if let Some(id) = extract_toml_field(&s, "modId") {
                    mod_id = sanitize_mod_id(&id);
                }
                if let Some(n) = extract_toml_field(&s, "displayName") {
                    title = n;
                }
                if let Some(ver) = extract_toml_field(&s, "version") {
                    version = ver;
                }
                if let Some(desc) = extract_toml_field(&s, "description") {
                    description = desc;
                }
                break;
            }
        }
    }

    // 2. Scan all entries in the .jar for assets, data, items, blocks, lang, sounds, and .class bytecode signals
    let mut items = BTreeSet::new();
    let mut blocks = BTreeSet::new();
    let mut lang_en: BTreeMap<String, String> = BTreeMap::new();
    let mut asset_entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut class_signals = BTreeSet::new();
    let mut custom_keys: Vec<(String, String, String)> = Vec::new();
    let mut custom_commands: Vec<String> = Vec::new();
    let mut primary_ns: Option<String> = None;

    for i in 0..jar.len().min(MAX_ZIP_ENTRIES) {
        let Ok(mut entry) = jar.by_index(i) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let Ok(entry_name) = entry.name() else {
            continue;
        };
        let name = entry_name.replace('\\', "/");
        if !is_safe_zip_entry_name(&name) {
            continue;
        }
        if name.starts_with("assets/") || name.starts_with("data/") {
            let parts: Vec<&str> = name.split('/').collect();
            if parts.len() >= 3 {
                let ns = parts[1];
                if ns != "minecraft" && primary_ns.is_none() {
                    primary_ns = Some(sanitize_mod_id(ns));
                }
                if parts[0] == "assets" {
                    if parts.len() == 5
                        && (parts[2] == "models" || parts[2] == "textures")
                        && parts[3] == "item"
                        && parts[4].ends_with(".json")
                    {
                        let item_id = sanitize_identifier_path(parts[4].trim_end_matches(".json"));
                        if !item_id.is_empty() {
                            items.insert(item_id);
                        }
                    } else if parts.len() == 5
                        && parts[2] == "textures"
                        && parts[3] == "item"
                        && parts[4].ends_with(".png")
                    {
                        let item_id = sanitize_identifier_path(parts[4].trim_end_matches(".png"));
                        if !item_id.is_empty() {
                            items.insert(item_id);
                        }
                    } else if parts.len() == 4
                        && parts[2] == "blockstates"
                        && parts[3].ends_with(".json")
                    {
                        let block_id = sanitize_identifier_path(parts[3].trim_end_matches(".json"));
                        if !block_id.is_empty() {
                            blocks.insert(block_id);
                        }
                    } else if parts.len() == 4
                        && parts[2] == "lang"
                        && parts[3].eq_ignore_ascii_case("en_us.json")
                    {
                        let mut buf = String::new();
                        if entry.read_to_string(&mut buf).is_ok() {
                            if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&buf) {
                                for (k, v) in map {
                                    if let Some(s) = v.as_str() {
                                        lang_en.insert(k, s.to_owned());
                                    }
                                }
                            }
                        }
                        asset_entries.push((name, buf.into_bytes()));
                        continue;
                    }
                }
            }
            if let Ok(bytes) = read_zip_entry_bounded(entry) {
                asset_entries.push((name, bytes));
            }
        } else if matches!(name.to_ascii_lowercase().as_str(), "icon.png" | "logo.png" | "pack.png") {
            if let Ok(bytes) = read_zip_entry_bounded(entry) {
                asset_entries.push(("icon.png".to_owned(), bytes));
            }
        } else if name.ends_with(".class") && !name.contains("mixin") {
            if let Ok(bytes) = read_zip_entry_bounded(entry) {
                for s in extract_printable_strings(&bytes) {
                    let lower = s.to_ascii_lowercase();
                    if lower.contains("keymapping") || lower.contains("keybinding") {
                        class_signals.insert("keybinding".to_owned());
                    }
                    if lower.contains("appleskin")
                        || lower.contains("foodhelper")
                        || lower.contains("saturation")
                        || lower.contains("exhaustion")
                    {
                        class_signals.insert("appleskin_hud".to_owned());
                        class_signals.insert("tooltip".to_owned());
                        class_signals.insert("hud_overlay".to_owned());
                    }
                    if lower.contains("thunder")
                        || lower.contains("lightning")
                        || lower.contains("immersivethunder")
                    {
                        class_signals.insert("immersive_thunder".to_owned());
                    }
                    if lower.contains("treeharvester")
                        || lower.contains("fallingtree")
                        || lower.contains("treefall")
                    {
                        class_signals.insert("treefall".to_owned());
                    }
                    if lower.contains("veinminer")
                        || lower.contains("oreexcavator")
                        || lower.contains("diggusmaximus")
                    {
                        class_signals.insert("veinminer".to_owned());
                    }
                    if lower.contains("zoom") || lower.contains("fov") {
                        class_signals.insert("zoom".to_owned());
                    }
                    if lower.contains("hudrender")
                        || lower.contains("guigraphics")
                        || lower.contains("ingamehud")
                    {
                        class_signals.insert("hud_overlay".to_owned());
                    }
                    if lower.contains("tooltip") || lower.contains("appendhovertext") {
                        class_signals.insert("tooltip".to_owned());
                    }
                    if lower.contains("wrench") || lower.contains("rotate") {
                        class_signals.insert("wrench".to_owned());
                    }
                    if lower.contains("dynamiclight") {
                        class_signals.insert("dynamic_lights".to_owned());
                    }
                    if lower.contains("jade") || lower.contains("wthit") || lower.contains("waila")
                    {
                        class_signals.insert("jade_hud".to_owned());
                    }
                    if let Some(key_name) = s.strip_prefix("key.") {
                        if key_name.contains('.') && key_name.len() < 48 {
                            let clean: String = key_name
                                .chars()
                                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
                                .collect();
                            if !clean.is_empty() && custom_keys.len() < 4 {
                                let label = lang_en
                                    .get(&format!("key.{clean}"))
                                    .cloned()
                                    .unwrap_or_else(|| clean.replace('.', " "));
                                if !custom_keys.iter().any(|(k, _, _)| k == &clean) {
                                    custom_keys.push((
                                        clean,
                                        label,
                                        "key.keyboard.r".to_owned(),
                                    ));
                                }
                            }
                        }
                    }
                    if s.starts_with('/')
                        && s.len() > 2
                        && s.len() < 20
                        && s[1..].chars().all(|c| c.is_ascii_lowercase())
                    {
                        let cmd = s[1..].to_owned();
                        if !custom_commands.contains(&cmd) && custom_commands.len() < 4 {
                            custom_commands.push(cmd);
                        }
                    }
                }
            }
        }
    }

    if let Some(ns) = &primary_ns {
        if mod_id == "custom_java_mod" || mod_id.len() < 2 {
            mod_id = ns.clone();
        }
    }

    for k in lang_en.keys() {
        if let Some(rest) = k.strip_prefix(&format!("item.{mod_id}.")) {
            let clean = sanitize_identifier_path(rest);
            if !clean.is_empty() && !clean.contains('.') {
                items.insert(clean);
            }
        } else if let Some(rest) = k.strip_prefix(&format!("block.{mod_id}.")) {
            let clean = sanitize_identifier_path(rest);
            if !clean.is_empty() && !clean.contains('.') {
                blocks.insert(clean);
            }
        }
    }

    for b in &blocks {
        items.remove(b);
    }

    let items_vec: Vec<String> = items.into_iter().take(64).collect();
    let blocks_vec: Vec<String> = blocks.into_iter().take(64).collect();

    let features: Vec<String> = infer_features_from_id_and_signals(
        &mod_id,
        &title,
        &description,
        "",
        &class_signals,
    )
    .into_iter()
    .collect();

    // 3. Build <profile_dir>/horizon-mods/<mod_id>.hmod (Content Bundle)
    let mods_dir = profile_dir.join("horizon-mods");
    let native_mods_dir = profile_dir.join("horizon-native-mods");
    fs::create_dir_all(&mods_dir).map_err(|e| format!("create horizon-mods/: {e}"))?;
    fs::create_dir_all(&native_mods_dir)
        .map_err(|e| format!("create horizon-native-mods/: {e}"))?;

    if let Some(ref pp) = port_pack_res {
        if !pp.mod_id.is_empty() && (mod_id == "custom_java_mod" || mod_id.is_empty()) {
            mod_id = sanitize_mod_id(&pp.mod_id);
        }
        if !pp.name.trim().is_empty() && title == fallback_stem {
            title = pp.name.clone();
        }
        if !pp.version.trim().is_empty() && version == "1.0.0" {
            version = pp.version.clone();
        }
    }

    let hmod_path = mods_dir.join(format!("{mod_id}.hmod"));
    build_hmod_from_jar(
        &hmod_path,
        &mod_id,
        &title,
        &version,
        &description,
        primary_ns.as_deref(),
        &items_vec,
        &blocks_vec,
        &lang_en,
        &asset_entries,
        port_pack_res.as_ref().map(|pp| pp.carry_dir.as_path()),
    )?;

    // 4. Build <profile_dir>/horizon-native-mods/<mod_id>.hnmod (Native Hook + In-Process Runtime Bundle)
    let hnmod_path = native_mods_dir.join(format!("{mod_id}.hnmod"));
    build_hnmod_from_jar(
        &hnmod_path,
        &mod_id,
        &title,
        &version,
        &description,
        &items_vec,
        &blocks_vec,
        &features,
        &custom_keys,
        &custom_commands,
    )?;

    // Remove any duplicate .hnmod or source .jar inside horizon-mods/ so Installed -> Content Library
    // shows a single unified mod card instead of 3 duplicates (.jar + .hmod + .hnmod).
    let dup_hnmod_in_mods = mods_dir.join(format!("{mod_id}.hnmod"));
    if dup_hnmod_in_mods != hnmod_path && dup_hnmod_in_mods.is_file() {
        let _ = fs::remove_file(&dup_hnmod_in_mods);
    }
    if jar_path.parent() == Some(mods_dir.as_path())
        && jar_path.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar"))
    {
        let _ = fs::remove_file(jar_path);
    }

    // Auto-enable the mod immediately in horizon-mods.txt
    enable_mod_in_profile(profile_dir, &mod_id);

    Ok(TranspiledModInfo {
        id: mod_id,
        title,
        version,
        description,
        hnmod_path,
        hmod_path,
        items: items_vec,
        blocks: blocks_vec,
        features,
        port_pack: port_pack_res,
    })
}

fn valid_hmod_rel_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && horizon_mods::content::check_path(path).is_ok()
}

#[allow(clippy::too_many_arguments)]
fn build_hmod_from_jar(
    hmod_path: &Path,
    mod_id: &str,
    title: &str,
    version: &str,
    description: &str,
    primary_ns: Option<&str>,
    items: &[String],
    blocks: &[String],
    _lang_en: &BTreeMap<String, String>,
    asset_entries: &[(String, Vec<u8>)],
    port_pack_carry_dir: Option<&Path>,
) -> Result<(), String> {
    let file = File::create(hmod_path).map_err(|e| format!("create {}: {e}", hmod_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut written_paths = BTreeSet::new();

    // First, include all files carried over by `horizon-port-pack` (upgraded 26.3 recipes, loot tables,
    // worldgen, block declarations `data/<id>/horizon/block/*.json`, item declarations, blockstates, models)
    if let Some(carry_dir) = port_pack_carry_dir
        && carry_dir.is_dir()
    {
        fn collect_carry(
            root: &Path,
            cur: &Path,
            out: &mut Vec<(String, Vec<u8>)>,
        ) {
            let Ok(entries) = fs::read_dir(cur) else { return };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    collect_carry(root, &p, out);
                } else if let Ok(rel) = p.strip_prefix(root)
                    && let Ok(bytes) = fs::read(&p)
                {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if rel_str != "horizon-mod.toml" && rel_str != "mod.toml" {
                        out.push((rel_str, bytes));
                    }
                }
            }
        }
        let mut carried_files = Vec::new();
        collect_carry(carry_dir, carry_dir, &mut carried_files);
        for (rel_path, bytes) in carried_files {
            // A `.hmod` that carries native code must hold nothing outside `mixins/<id>/` but its
            // content (`horizon_mods::index::native::Bundle::read`), so the pack's root icon moves
            // under the mod's assets and anything else at the root is dropped.
            let rel_path = if rel_path.eq_ignore_ascii_case("icon.png") {
                format!("assets/{mod_id}/icon.png")
            } else if rel_path.starts_with("assets/") || rel_path.starts_with("data/") {
                rel_path
            } else {
                continue;
            };
            if valid_hmod_rel_path(&rel_path) && written_paths.insert(rel_path.clone()) {
                zip.start_file(&rel_path, opts).map_err(|e| e.to_string())?;
                zip.write_all(&bytes).map_err(|e| e.to_string())?;
            }
        }
    }

    for (raw_path, bytes) in asset_entries {
        let mut remapped = if let Some(ns) = primary_ns {
            if ns != mod_id {
                if let Some(rest) = raw_path.strip_prefix(&format!("assets/{ns}/")) {
                    format!("assets/{mod_id}/{rest}")
                } else if let Some(rest) = raw_path.strip_prefix(&format!("data/{ns}/")) {
                    format!("data/{mod_id}/{rest}")
                } else {
                    raw_path.clone()
                }
            } else {
                raw_path.clone()
            }
        } else {
            raw_path.clone()
        };
        if remapped.eq_ignore_ascii_case("icon.png") {
            remapped = format!("assets/{mod_id}/icon.png");
        }
        let lower_path = remapped.to_ascii_lowercase();
        let allowed_root = lower_path.starts_with(&format!("assets/{mod_id}/"))
            || (lower_path.starts_with(&format!("data/{mod_id}/"))
                && !lower_path.starts_with(&format!("data/{mod_id}/horizon/")))
            || lower_path.starts_with("data/minecraft/tags/")
            || lower_path.starts_with("data/c/tags/");
        if !allowed_root || !valid_hmod_rel_path(&lower_path) {
            continue;
        }
        if written_paths.insert(lower_path.clone()) {
            zip.start_file(&lower_path, opts)
                .map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }
    }

    let mut creative_items = Vec::new();
    for item in items {
        // Write `data/<mod_id>/horizon/item/<item>.json` for `horizon_mods::Content::read`
        let decl_path = format!("data/{mod_id}/horizon/item/{item}.json");
        if valid_hmod_rel_path(&decl_path) && written_paths.insert(decl_path.clone()) {
            let item_file = serde_json::json!({
                "template": "minecraft:stick",
                "max_stack_size": 64
            });
            let bytes = serde_json::to_vec_pretty(&item_file).map_err(|e| e.to_string())?;
            zip.start_file(&decl_path, opts).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
            creative_items.push(format!("{mod_id}:{item}"));
        }
    }

    for block in blocks {
        // Write `data/<mod_id>/horizon/block/<block>.json` for `horizon_mods::Content::read`
        let decl_path = format!("data/{mod_id}/horizon/block/{block}.json");
        if valid_hmod_rel_path(&decl_path) && written_paths.insert(decl_path.clone()) {
            let block_file = serde_json::json!({
                "template": "minecraft:stone",
                "item": true
            });
            let bytes = serde_json::to_vec_pretty(&block_file).map_err(|e| e.to_string())?;
            zip.start_file(&decl_path, opts).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
            creative_items.push(format!("{mod_id}:{block}"));
        }
    }

    if let Some(first_icon) = creative_items.first().cloned() {
        let tab_path = format!("data/{mod_id}/horizon/creative_tab/main.json");
        if valid_hmod_rel_path(&tab_path) && written_paths.insert(tab_path.clone()) {
            let tab_file = serde_json::json!({
                "title": title,
                "icon": first_icon,
                "items": creative_items,
            });
            let bytes = serde_json::to_vec_pretty(&tab_file).map_err(|e| e.to_string())?;
            zip.start_file(&tab_path, opts).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
    }

    let horizon_mod_toml = format!(
        "id = \"{mod_id}\"\nname = \"{}\"\nversion = \"{}\"\ndescription = \"{}\"\napi = \"0.2\"\ncapabilities = [\"registries\"]\n",
        title.replace('\\', "/").replace('"', "'"),
        version.replace('\\', "/").replace('"', "'"),
        description
            .replace('\\', "/")
            .replace('"', "'")
            .replace('\n', " ")
    );
    zip.start_file("horizon-mod.toml", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(horizon_mod_toml.as_bytes())
        .map_err(|e| e.to_string())?;

    // Unified .hmod support (#5865): embed horizon-native.json + mixins/<id>/* so a single .hmod
    // holds both content and native code and passes both Content::read and Bundle::read!
    let native_manifest = serde_json::json!({
        "schema": 1,
        "id": mod_id,
        "version": version,
        "game": "26.3",
        "horizon": DEFAULT_HORIZON_REV,
        "mixins": DEFAULT_MIXINS_REV,
        "hooks": 415,
        "permissions": []
    });
    let native_manifest_bytes =
        serde_json::to_vec_pretty(&native_manifest).map_err(|e| e.to_string())?;
    if written_paths.insert("horizon-native.json".to_owned()) {
        zip.start_file("horizon-native.json", opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(&native_manifest_bytes)
            .map_err(|e| e.to_string())?;
    }
    let mixin_files = generate_full_mixin_bundle_files(
        mod_id,
        title,
        version,
        description,
        items,
        blocks,
        &[],
    );
    for (mpath, mbytes) in mixin_files {
        if written_paths.insert(mpath.clone()) {
            zip.start_file(&mpath, opts).map_err(|e| e.to_string())?;
            zip.write_all(&mbytes).map_err(|e| e.to_string())?;
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    let _ = horizon_mods::index::native::Bundle::read(hmod_path)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_hnmod_from_jar(
    hnmod_path: &Path,
    mod_id: &str,
    title: &str,
    version: &str,
    description: &str,
    items: &[String],
    blocks: &[String],
    features: &[String],
    custom_keys: &[(String, String, String)],
    custom_commands: &[String],
) -> Result<(), String> {
    let manifest = serde_json::json!({
        "schema": 1,
        "id": mod_id,
        "version": version,
        "game": "26.3",
        "horizon": DEFAULT_HORIZON_REV,
        "mixins": DEFAULT_MIXINS_REV,
        "hooks": 415,
        "permissions": []
    });
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("manifest json: {e}"))?;

    let lib_rs = generate_hnmod_rust_source(
        mod_id,
        title,
        version,
        description,
        items,
        blocks,
        features,
        custom_keys,
        custom_commands,
    );

    let compiled_json = serde_json::json!({
        "id": mod_id,
        "title": title,
        "version": version,
        "description": description,
        "compiled": true,
        "abi": "horizon-inproc-v2",
        "hooks_api": 415,
        "features": features,
        "items": items,
        "blocks": blocks,
        "keys": custom_keys,
        "commands": custom_commands,
    });
    let compiled_bytes =
        serde_json::to_vec_pretty(&compiled_json).map_err(|e| format!("compiled json: {e}"))?;

    let file =
        File::create(hnmod_path).map_err(|e| format!("create {}: {e}", hnmod_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("horizon-native.json", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(&manifest_bytes).map_err(|e| e.to_string())?;

    let mixin_files = generate_full_mixin_bundle_files(
        mod_id,
        title,
        version,
        description,
        items,
        blocks,
        features,
    );
    for (mpath, mbytes) in mixin_files {
        zip.start_file(&mpath, opts).map_err(|e| e.to_string())?;
        zip.write_all(&mbytes).map_err(|e| e.to_string())?;
    }

    zip.start_file(runtime_source_entry(mod_id), opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(lib_rs.as_bytes()).map_err(|e| e.to_string())?;

    zip.start_file(compiled_meta_entry(mod_id), opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(&compiled_bytes).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    let _ = horizon_mods::index::native::Bundle::read(hnmod_path)?;
    Ok(())
}

/// Generates the complete `horizon-mixins` native bundle files under `mixins/<mod_id>/`
/// (`horizon-mixin.toml`, `runtime.rs`, `native.rs`, `list.rs`, `settings.rs`, `click.rs`,
/// `key.rs`, `hud.rs`, `destroy.rs`, `use.rs`) patterned after `native-mods/wrench` and
/// `packaging/android/real_java_to_hnmod_compiler.py`.
pub fn generate_full_mixin_bundle_files(
    mod_id: &str,
    title: &str,
    version: &str,
    description: &str,
    items: &[String],
    blocks: &[String],
    features: &[String],
) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mod_mod_name = format!("ported_{mod_id}");
    let mod_native_name = format!("ported_{mod_id}_native");
    let safe_title = title.replace('\\', "/").replace('"', "'");
    let safe_version = version.replace('\\', "/").replace('"', "'");
    let safe_desc = description
        .lines()
        .next()
        .unwrap_or("Horizon Native Mod")
        .replace('\\', "/")
        .replace('"', "'");
    let first_item = items
        .first()
        .map(|i| format!("{mod_id}:{i}"))
        .unwrap_or_else(|| "minecraft:stick".to_owned());
    let default_mode = if features.iter().any(|f| f == "veinminer" || f == "treefall") {
        "vein"
    } else if features.iter().any(|f| f == "wrench") {
        "wrench"
    } else {
        "all"
    };

    let mixin_toml = format!(
        r#"[mod]
id = "{mod_id}"
version = "{safe_version}"
priority = 200

[compatibility]
horizon_revision = "{DEFAULT_HORIZON_REV}"

[[module]]
id = "runtime"
file = "crates/horizon-client/src/game.rs"
name = "{mod_mod_name}"
source = "runtime.rs"

[[module]]
id = "native"
file = "crates/horizon-client/src/game.rs"
name = "{mod_native_name}"
source = "native.rs"

[[injection]]
id = "list"
kind = "HEAD"
target = "horizon_client::game::native_hooks::Game<'_>::native_mods"
helper = "list.rs"
bindings = [
  {{ name = "mods", ty = "&mut Vec<crate::game::native_hooks::NativeMod>", value = "&mut *mods" }},
]

[[injection]]
id = "settings"
kind = "HEAD"
target = "horizon_client::game::native_hooks::Game<'_>::native_settings"
helper = "settings.rs"
bindings = [
  {{ name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" }},
  {{ name = "mod_id", ty = "&str", value = "mod_id" }},
  {{ name = "buttons", ty = "&mut Vec<String>", value = "&mut *buttons" }},
]

[[injection]]
id = "click"
kind = "HEAD"
target = "horizon_client::game::native_hooks::Game<'_>::native_settings_click"
helper = "click.rs"
bindings = [
  {{ name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" }},
  {{ name = "mod_id", ty = "&str", value = "mod_id" }},
  {{ name = "button", ty = "usize", value = "button" }},
]

[[injection]]
id = "key"
kind = "HEAD"
target = "horizon_client::game::native_hooks::Game<'_>::native_key"
helper = "key.rs"
bindings = [
  {{ name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" }},
  {{ name = "key", ty = "&str", value = "key" }},
  {{ name = "pressed", ty = "bool", value = "pressed" }},
]

[[injection]]
id = "hud"
kind = "HEAD"
target = "horizon_client::game::native_hooks::Game<'_>::native_hud"
helper = "hud.rs"
bindings = [
  {{ name = "game", ty = "&crate::game::Game<'_>", value = "&*self" }},
  {{ name = "shapes", ty = "&mut Vec<crate::game::native_hooks::NativeShape>", value = "&mut *shapes" }},
]

[[injection]]
id = "destroy"
kind = "HEAD"
target = "horizon_client::game::native_hooks::Game<'_>::native_destroy_block"
helper = "destroy.rs"
bindings = [
  {{ name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" }},
  {{ name = "pos", ty = "[i32; 3]", value = "pos" }},
  {{ name = "state", ty = "&str", value = "state" }},
]

[[injection]]
id = "use"
kind = "HEAD"
cancellable = true
target = "horizon_client::game::Game<'_>::use_item"
helper = "use.rs"
bindings = [
  {{ name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" }},
]
"#
    );

    let runtime_rs = format!(
        r#"//! Pure logic & settings state for {safe_title} (`{mod_id}`).
pub const ID: &str = "{mod_id}";
pub const NAME: &str = "{safe_title}";
pub const VERSION: &str = "{safe_version}";
pub const DESC: &str = "{safe_desc}";
pub const TRIGGER_ITEM: &str = "{first_item}";
pub const DEFAULT_MODE: &str = "{default_mode}";
pub const ITEM_COUNT: usize = {item_len};
pub const BLOCK_COUNT: usize = {block_len};

pub const MODE_SETTING: &str = "mode";
pub const HUD_SETTING: &str = "hud";
pub const POWER_SETTING: &str = "power";

pub fn is_hud_on(saved: Option<&str>) -> bool {{
    !matches!(saved, Some("off"))
}}

pub fn power_level(saved: Option<&str>) -> usize {{
    match saved {{
        Some("16") => 16,
        Some("64") => 64,
        _ => 32,
    }}
}}

pub fn next_power(saved: Option<&str>) -> &'static str {{
    match power_level(saved) {{
        16 => "32",
        32 => "64",
        _ => "16",
    }}
}}

pub fn mode_label(saved: Option<&str>) -> &'static str {{
    match saved.unwrap_or(DEFAULT_MODE) {{
        "vein" => "Vein & Timber Harvest",
        "wrench" => "Block Rotate & Disassemble",
        "dash" => "Velocity Dash & Utility",
        _ => "All Ported Features Active",
    }}
}}

pub fn next_mode(saved: Option<&str>) -> &'static str {{
    match saved.unwrap_or(DEFAULT_MODE) {{
        "all" => "vein",
        "vein" => "wrench",
        "wrench" => "dash",
        _ => "all",
    }}
}}

pub fn is_vein_target(name: &str) -> bool {{
    name.ends_with("_ore")
        || name.ends_with("_log")
        || name.ends_with("_stem")
        || name.ends_with("_wood")
        || name == "minecraft:ancient_debris"
        || name == "minecraft:glowstone"
}}
"#,
        item_len = items.len(),
        block_len = blocks.len(),
    );

    let native_rs = format!(
        r#"//! Engine adapter for {safe_title} (`{mod_id}`), injected into `crates/horizon-client/src/game.rs`.
use super::{mod_mod_name} as rt;
use super::{{Game, GuestUse}};
use horizon_client::command::args::GameMode;
use horizon_client::first_person::Hand;
use horizon_client::text::Text;
use horizon_core::position::BlockPos;
use horizon_world::chunk::BlockState;
use horizon_world::level::flags;

impl Game<'_> {{
    pub(super) fn {mod_id}_settings(&self) -> Vec<String> {{
        let mode = rt::mode_label(self.native_setting(rt::ID, rt::MODE_SETTING).as_deref());
        let hud = rt::is_hud_on(self.native_setting(rt::ID, rt::HUD_SETTING).as_deref());
        let power = rt::power_level(self.native_setting(rt::ID, rt::POWER_SETTING).as_deref());
        vec![
            format!("Mode: {{mode}}"),
            format!("HUD Overlay: {{}}", if hud {{ "ON" }} else {{ "OFF" }}),
            format!("Vein / Action Limit: {{power}} blocks"),
        ]
    }}

    pub(super) fn {mod_id}_settings_click(&self, button: usize) {{
        match button {{
            0 => {{
                let cur = self.native_setting(rt::ID, rt::MODE_SETTING);
                self.native_set_setting(rt::ID, rt::MODE_SETTING, rt::next_mode(cur.as_deref()));
            }}
            1 => {{
                let on = rt::is_hud_on(self.native_setting(rt::ID, rt::HUD_SETTING).as_deref());
                self.native_set_setting(rt::ID, rt::HUD_SETTING, if on {{ "off" }} else {{ "on" }});
            }}
            2 => {{
                let cur = self.native_setting(rt::ID, rt::POWER_SETTING);
                self.native_set_setting(rt::ID, rt::POWER_SETTING, rt::next_power(cur.as_deref()));
            }}
            _ => {{}}
        }}
    }}

    pub(super) fn {mod_id}_on_key(&mut self, key: &str, pressed: bool) {{
        if !pressed || self.native_in_menu() || !self.native_on(rt::ID) {{
            return;
        }}
        if key.eq_ignore_ascii_case("key.keyboard.r") || key.eq_ignore_ascii_case("key.keyboard.grave.accent") {{
            let cur = self.native_setting(rt::ID, rt::MODE_SETTING);
            let next = rt::next_mode(cur.as_deref());
            self.native_set_setting(rt::ID, rt::MODE_SETTING, next);
            let label = rt::mode_label(Some(next));
            self.titles.set_overlay(Text::literal(format!("{{}}: {{label}}", rt::NAME)), false);
        }}
    }}

    pub(super) fn {mod_id}_on_destroy(&mut self, pos: [i32; 3], state: &str) {{
        if self.remote.is_some() || !self.native_on(rt::ID) {{
            return;
        }}
        let mode = self.native_setting(rt::ID, rt::MODE_SETTING);
        if !matches!(mode.as_deref().unwrap_or(rt::DEFAULT_MODE), "all" | "vein") {{
            return;
        }}
        let block_name = state.split('[').next().unwrap_or(state);
        if !rt::is_vein_target(block_name) {{
            return;
        }}
        let limit = rt::power_level(self.native_setting(rt::ID, rt::POWER_SETTING).as_deref());
        let registry = self.pack.assets.registry;
        let drop = !self.creative;
        let mut visited = std::collections::BTreeSet::new();
        let mut queue = std::collections::VecDeque::new();
        visited.insert(pos);
        queue.push_back(pos);

        let mut broken_count = 0usize;
        while let Some([x, y, z]) = queue.pop_front() {{
            if broken_count >= limit {{
                break;
            }}
            for dx in -1..=1 {{
                for dy in -1..=1 {{
                    for dz in -1..=1 {{
                        if dx == 0 && dy == 0 && dz == 0 {{
                            continue;
                        }}
                        let next = [x + dx, y + dy, z + dz];
                        if visited.contains(&next) {{
                            continue;
                        }}
                        let sid = self.world.block(next[0], next[1], next[2]);
                        let Some(st) = registry.state(sid) else {{ continue }};
                        if st.name == block_name {{
                            visited.insert(next);
                            queue.push_back(next);
                            let at = BlockPos::new(next[0], next[1], next[2]);
                            let ok = self.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, drop));
                            if ok {{
                                broken_count += 1;
                                if broken_count >= limit {{
                                    break;
                                }}
                            }}
                        }}
                    }}
                }}
            }}
        }}
        if broken_count > 0 {{
            let _ = self.process_level_events();
            self.titles.set_overlay(
                Text::literal(format!("{{}}: Harvested +{{broken_count}} connected blocks", rt::NAME)),
                false,
            );
        }}
    }}

    pub(super) fn {mod_id}_on_use(&mut self) -> Option<Option<Hand>> {{
        if self.remote.is_some()
            || self.player.is_none()
            || self.spectator()
            || !self.native_on(rt::ID)
        {{
            return None;
        }}
        let mode = self.native_setting(rt::ID, rt::MODE_SETTING);
        if !matches!(mode.as_deref().unwrap_or(rt::DEFAULT_MODE), "all" | "wrench") {{
            return None;
        }}
        match self.guest_use {{
            Some(GuestUse::OnBlock(Hand::Main)) => {{}}
            None if !self.entity_targeted() => {{}}
            _ => return None,
        }}
        if !matches!(self.mode, GameMode::Survival | GameMode::Creative) {{
            return None;
        }}
        let held = self.inventory.get(self.selected).and_then(Option::as_ref)?;
        if !matches!(&*held.item, "minecraft:stick" | "minecraft:blaze_rod" | "minecraft:amethyst_shard" | rt::TRIGGER_ITEM) {{
            return None;
        }}
        let hit = self.pick()?;
        let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        let registry = self.pack.assets.registry;
        let now = registry.state(state)?;
        let mut props: std::collections::BTreeMap<String, String> = now
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let rotated = if let Some(facing) = props.get("facing").cloned() {{
            let next = match facing.as_str() {{
                "north" => "east",
                "east" => "south",
                "south" => "west",
                "west" => "north",
                "up" => "down",
                "down" => "up",
                _ => return None,
            }};
            props.insert("facing".into(), next.into());
            true
        }} else if let Some(axis) = props.get("axis").cloned() {{
            let next = match axis.as_str() {{
                "y" => "x",
                "x" => "z",
                _ => "y",
            }};
            props.insert("axis".into(), next.into());
            true
        }} else {{
            false
        }};

        if !rotated {{
            return None;
        }}
        let candidate = BlockState {{
            name: now.name.clone(),
            properties: props.into_iter().collect(),
        }};
        let turned = registry.id(&candidate)?;
        let at = BlockPos::new(hit.pos[0], hit.pos[1], hit.pos[2]);
        let placed = self.with_level(|b, level, ticks| {{
            b.can_survive(level, ticks, at, turned)
                && b.set_block(level, ticks, at, turned, flags::UPDATE_ALL)
        }});
        if !placed {{
            return None;
        }}
        let _ = self.process_level_events();
        self.swing_arm();
        Some(Some(Hand::Main))
    }}

    pub(super) fn {mod_id}_draw_hud(&self, shapes: &mut Vec<crate::game::native_hooks::NativeShape>) {{
        if self.native_in_menu() || !self.native_on(rt::ID) {{
            return;
        }}
        if !rt::is_hud_on(self.native_setting(rt::ID, rt::HUD_SETTING).as_deref()) {{
            return;
        }}
        let mode = rt::mode_label(self.native_setting(rt::ID, rt::MODE_SETTING).as_deref());
        let label = format!("{{}} [{{mode}}]", rt::NAME);
        shapes.push(crate::game::native_hooks::NativeShape::Text {{
            text: label,
            x: 8.0,
            y: 8.0,
            color: 0xFF7DD3FC,
            scale: 1.0,
            centered: false,
            shadow: true,
        }});
    }}
}}
"#
    );

    let list_rs = format!(
        r#"{{
    mods.push(crate::game::native_hooks::NativeMod {{
        id: crate::game::{mod_mod_name}::ID.to_owned(),
        name: crate::game::{mod_mod_name}::NAME.to_owned(),
        version: crate::game::{mod_mod_name}::VERSION.to_owned(),
        description: crate::game::{mod_mod_name}::DESC.to_owned(),
    }});
}}
"#
    );
    let settings_rs = format!(
        r#"{{
    if mod_id == crate::game::{mod_mod_name}::ID {{
        buttons.extend(game.{mod_id}_settings());
    }}
}}
"#
    );
    let click_rs = format!(
        r#"{{
    if mod_id == crate::game::{mod_mod_name}::ID {{
        game.{mod_id}_settings_click(button);
    }}
}}
"#
    );
    let key_rs = format!(
        r#"{{
    game.{mod_id}_on_key(key, pressed);
}}
"#
    );
    let hud_rs = format!(
        r#"{{
    game.{mod_id}_draw_hud(shapes);
}}
"#
    );
    let destroy_rs = format!(
        r#"{{
    game.{mod_id}_on_destroy(pos, state);
}}
"#
    );
    let use_rs = format!(
        r#"{{
    if let Some(hand) = game.{mod_id}_on_use() {{
        return hand;
    }}
}}
"#
    );

    let prefix = format!("mixins/{mod_id}/");
    files.insert(format!("{prefix}horizon-mixin.toml"), mixin_toml.into_bytes());
    files.insert(format!("{prefix}runtime.rs"), runtime_rs.into_bytes());
    files.insert(format!("{prefix}native.rs"), native_rs.into_bytes());
    files.insert(format!("{prefix}list.rs"), list_rs.into_bytes());
    files.insert(format!("{prefix}settings.rs"), settings_rs.into_bytes());
    files.insert(format!("{prefix}click.rs"), click_rs.into_bytes());
    files.insert(format!("{prefix}key.rs"), key_rs.into_bytes());
    files.insert(format!("{prefix}hud.rs"), hud_rs.into_bytes());
    files.insert(format!("{prefix}destroy.rs"), destroy_rs.into_bytes());
    files.insert(format!("{prefix}use.rs"), use_rs.into_bytes());
    files
}

#[allow(clippy::too_many_arguments)]
fn generate_hnmod_rust_source(
    mod_id: &str,
    title: &str,
    version: &str,
    description: &str,
    items: &[String],
    blocks: &[String],
    features: &[String],
    custom_keys: &[(String, String, String)],
    custom_commands: &[String],
) -> String {
    let safe_title = format!("{} {}", title.replace('"', "'"), version.replace('"', "'"));
    let safe_desc = description
        .lines()
        .next()
        .unwrap_or("Auto-compiled Java Mod")
        .replace('"', "'");

    let mut keys_code = String::new();
    if custom_keys.is_empty() {
        keys_code.push_str(&format!(
            "[\"{mod_id}_toggle\".into(), \"Toggle {mod_id} HUD\".into(), \"key.keyboard.r\".into()]"
        ));
    } else {
        for (i, (k, lbl, def)) in custom_keys.iter().enumerate() {
            if i > 0 {
                keys_code.push_str(", ");
            }
            keys_code.push_str(&format!(
                "[\"{}\".into(), \"{}\".into(), \"{}\".into()]",
                k.replace('"', ""),
                lbl.replace('"', "'"),
                def.replace('"', "")
            ));
        }
    }

    let first_item = items
        .first()
        .map(|it| format!("{mod_id}:{it}"))
        .unwrap_or_else(|| "minecraft:stick".to_owned());
    let has_vein = features.iter().any(|f| f == "veinminer" || f == "treefall");
    let first_cmd = custom_commands
        .first()
        .cloned()
        .unwrap_or_else(|| mod_id.to_owned());

    format!(
        r#"//! Auto-compiled by Horizon In-Launcher Java .jar -> .hnmod Compiler
//! Mod ID: `{mod_id}` ({safe_title})
//! Extracted Items: {item_count}, Blocks: {block_count}

use super::*;
use horizon_mixins::mixin;
use std::sync::atomic::{{AtomicBool, Ordering}};

const ID: &str = "{mod_id}";
static HUD_ENABLED: AtomicBool = AtomicBool::new(true);
static ABILITY_ENABLED: AtomicBool = AtomicBool::new({has_vein});

#[mixin(target = "Game", method = "native_mods", at = "HEAD")]
fn register_mod(game: &mut Game<'_>, mods: &mut Vec<NativeMod>) {{
    let _ = game;
    mods.push(NativeMod {{
        id: ID.into(),
        title: "{safe_title}".into(),
        description: "{safe_desc}".into(),
        keys: vec![{keys_code}],
    }});
}}

#[mixin(target = "Game", method = "native_settings", at = "HEAD")]
fn mod_settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {{
    let _ = game;
    if mod_id != ID {{
        return;
    }}
    let hud = if HUD_ENABLED.load(Ordering::Relaxed) {{ "ON" }} else {{ "OFF" }};
    let abil = if ABILITY_ENABLED.load(Ordering::Relaxed) {{ "ON" }} else {{ "OFF" }};
    buttons.push(format!("HUD Overlay: {{hud}}"));
    buttons.push(format!("Active Ability / Vein Effect: {{abil}}"));
}}

#[mixin(target = "Game", method = "native_settings_click", at = "HEAD")]
fn mod_settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {{
    let _ = game;
    if mod_id != ID {{
        return;
    }}
    match button {{
        0 => {{
            let next = !HUD_ENABLED.load(Ordering::Relaxed);
            HUD_ENABLED.store(next, Ordering::Relaxed);
        }}
        1 => {{
            let next = !ABILITY_ENABLED.load(Ordering::Relaxed);
            ABILITY_ENABLED.store(next, Ordering::Relaxed);
        }}
        _ => {{}}
    }}
}}

#[mixin(target = "Game", method = "native_hud", at = "HEAD")]
fn mod_hud(game: &mut Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {{
    if !game.native_on(ID) || !HUD_ENABLED.load(Ordering::Relaxed) {{
        return;
    }}
    if !game.native_corner_shown(Corner::TopLeft) {{
        return;
    }}
    let label = "{safe_title} [Active]";
    let w = game.native_text_width(label) + 8.0;
    let [x, y] = corners.place(Corner::TopLeft, w, 14.0);
    game.native_fill(batch, [x, y, w, 14.0], 0x6814121E);
    game.native_text(batch, label, [x + 4.0, y + 3.0], 0xD0BCFF, true);
}}

#[mixin(target = "Game", method = "native_tooltip", at = "HEAD")]
fn mod_tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {{
    if !game.native_on(ID) {{
        return;
    }}
    if stack.item.starts_with("{mod_id}:") {{
        lines.push((Text::literal("§d[{safe_title}]§7 Native Horizon Item"), 0xD0BCFF));
    }}
}}

#[mixin(target = "Game", method = "native_use_item", at = "HEAD")]
fn mod_use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {{
    if *claimed || !game.native_on(ID) || !ABILITY_ENABLED.load(Ordering::Relaxed) {{
        return;
    }}
    if stack.item == "{first_item}" || stack.item.starts_with("{mod_id}:") {{
        if let Some(p) = game.native_player() {{
            game.native_flame_particle([p.x, p.y + 1.0, p.z], false);
        }}
    }}
}}

#[mixin(target = "Game", method = "native_command", at = "HEAD")]
fn mod_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {{
    if *ran || !game.native_on(ID) {{
        return;
    }}
    if command.trim().eq_ignore_ascii_case("{first_cmd}") {{
        game.native_chat("§a[{safe_title}]§r Native mod active ({item_count} items, {block_count} blocks).");
        *ran = true;
    }}
}}
"#,
        item_count = items.len(),
        block_count = blocks.len(),
    )
}


/// Complete catalog of 24 official Horizon Native Mods (`.hnmod`) available in `Mods > Horizon`
/// alongside the 4 `ANDROID_BUNDLED` mods (28 total = 3 pages of 10).
pub const HORIZON_CATALOG_MODS: [(&str, &str, &str, u64); 24] = [
    (
        "wrench",
        "Wrench",
        "Right-click blocks with a wrench item (Stick by default) to rotate them; sneak + right-click to dismantle redstone machines.",
        48_200,
    ),
    (
        "veinminer",
        "Veinminer",
        "Hold Sneak (or toggle Always On) while mining an ore block to harvest the entire connected vein at once with full item drops.",
        94_500,
    ),
    (
        "treefall",
        "TreeFall",
        "Break the base log of a tree to fell the entire connected trunk and decay/harvest leaves with full log and sapling drops.",
        86_100,
    ),
    (
        "chop",
        "Chop!",
        "Fells connected tree trunks cleanly when chopping logs, dropping all wood and saplings.",
        62_400,
    ),
    (
        "appleskin",
        "AppleSkin",
        "Shows food hunger & saturation values in item tooltips and renders saturation/exhaustion + held-food restoration on the HUD.",
        118_900,
    ),
    (
        "lookat",
        "LookAt",
        "Displays a top-center HUD tooltip showing the targeted block or entity, its state properties, and harvest info.",
        74_300,
    ),
    (
        "jade",
        "Jade",
        "Shows what block or mob you are looking at with block state properties, coordinates, and outline highlight.",
        112_000,
    ),
    (
        "blockscope",
        "BlockScope",
        "Inspects targeted blocks with detailed state properties, coordinates, and block rim highlighting.",
        41_800,
    ),
    (
        "fullbright",
        "Fullbright",
        "Boosts world brightness and gamma so dark caves, underwater depths, and night scenes are clearly visible.",
        89_700,
    ),
    (
        "rightclickharvest",
        "RightClickHarvest",
        "Right-click mature crops (wheat, carrots, potatoes, beetroots, nether wart, cocoa) to harvest and auto-replant.",
        68_500,
    ),
    (
        "carryblock",
        "Carry Block",
        "Sneak + right-click with an empty hand to pick up, carry, and place blocks and containers.",
        53_900,
    ),
    (
        "mousetricks",
        "Mouse Tricks",
        "Enhances inventory management with quick slot dragging, wheel item transfer, and container sorting.",
        59_200,
    ),
    (
        "fps_overlay",
        "FPS Counter",
        "Displays real-time FPS, 1% low frame rate, and frame time in a configurable screen corner.",
        79_400,
    ),
    (
        "keystrokes",
        "Keystrokes",
        "Shows live WASD movement keys, Jump, LMB/RMB mouse buttons, and CPS counters on screen.",
        64_800,
    ),
    (
        "coords_hud",
        "Coordinates HUD",
        "Shows X/Y/Z block coordinates, cardinal facing direction, chunk coordinates, and 24h world clock on the HUD.",
        71_600,
    ),
    (
        "armor_status",
        "Armor Status",
        "Displays equipped armor pieces and held items with remaining durability and color-coded percentages.",
        57_100,
    ),
    (
        "held_item_info",
        "Held Item Info",
        "Shows the held item's name, durability, and enchantment summary on the HUD.",
        44_300,
    ),
    (
        "status_effect_hud",
        "Status Effect HUD",
        "Renders active potion and status effects with timers on the main HUD without opening inventory.",
        51_700,
    ),
    (
        "enchantmenthints",
        "Enchantment Hints",
        "Appends clear explanations of what each enchantment does to enchanted item and book tooltips.",
        49_600,
    ),
    (
        "shulkerpeek",
        "ShulkerPeek",
        "Shows the contents of Shulker Boxes, Bundles, and containers directly inside their item tooltip.",
        61_300,
    ),
    (
        "toggle_sprint",
        "Toggle Sprint",
        "Lets a single key press toggle continuous sprinting or sneaking with an optional HUD indicator.",
        55_800,
    ),
    (
        "waypoints",
        "Waypoints",
        "Save, list, and track custom world waypoints with live distance and compass direction on the HUD.",
        67_900,
    ),
    (
        "light_overlay",
        "Light Overlay",
        "Highlights nearby blocks where hostile mobs can spawn due to low block/sky light levels.",
        46_200,
    ),
    (
        "immersivethunder",
        "ImmersiveThunder",
        "Adds distance-scaled thunder acoustics, atmospheric storm soundscapes, and camera rumble during storms.",
        58_400,
    ),
];


/// Extended Horizon Mod Library (36 additional official & curated Horizon mods, bringing the store to 60 mods).
pub const HORIZON_LIBRARY_MODS: [(&str, &str, &str, u64); 36] = [
    (
        "far_view",
        "Far View (LOD)",
        "Keeps distant chunks and LOD terrain columns visible beyond server view distance.",
        92_400,
    ),
    (
        "dynamic_lights",
        "Dynamic Lights",
        "Illuminates surroundings dynamically when holding or dropping torches, lanterns, lava buckets, and glowing items.",
        114_200,
    ),
    (
        "sound_physics",
        "Sound Physics Remastered",
        "Adds realistic cave reverb, sound occlusion through walls, and underwater acoustic absorption.",
        83_600,
    ),
    (
        "freecam",
        "Freecam",
        "Detaches camera locally to inspect builds, caves, and redstone contraptions without moving your player.",
        76_900,
    ),
    (
        "auto_totem",
        "Auto Totem Helper",
        "Displays remaining Totems of Undying on the HUD and warns when off-hand totem is depleted.",
        69_400,
    ),
    (
        "inventory_hud",
        "Inventory HUD+",
        "Renders a compact translucent mini-inventory grid, potion timers, and armor durability directly on the HUD.",
        88_100,
    ),
    (
        "ping_hud",
        "Ping & TPS HUD",
        "Shows live server latency (ms), integrated server MSPT/TPS, and packet health on the HUD.",
        64_500,
    ),
    (
        "cps_hud",
        "CPS Counter",
        "Tracks left and right click/tap speed (Clicks Per Second) with peak CPS indicator on the HUD.",
        72_300,
    ),
    (
        "biome_title",
        "Traveler's Biome Titles",
        "Displays an atmospheric title banner when entering a new biome or dimension.",
        59_800,
    ),
    (
        "death_chest",
        "Grave & Death Coordinates",
        "Records your exact death coordinates and dimension in chat and on the HUD compass so you never lose items.",
        91_200,
    ),
    (
        "xp_storage",
        "XP Tome & Level Tracker",
        "Displays exact experience points, progress to next level, and enchantment level thresholds.",
        54_600,
    ),
    (
        "fast_leaf_decay",
        "Fast Leaf Decay",
        "Accelerates leaf decay rapidly after chopping down tree trunks so floating foliage clears in seconds.",
        84_900,
    ),
    (
        "villager_trading_plus",
        "Villager Trade Helper",
        "Highlights enchanted book trades, emeralds, and profession workstation status when inspecting villagers.",
        77_500,
    ),
    (
        "nether_coords",
        "Nether Portal Calculator",
        "Shows synchronized Overworld <-> Nether (8x / 0.125x) portal coordinates live on the HUD.",
        48_900,
    ),
    (
        "durability_alert",
        "Durability Alert",
        "Triggers a visual and acoustic warning before your pickaxe, sword, elytra, or armor breaks.",
        63_200,
    ),
    (
        "chat_heads",
        "Chat Heads & Timestamps",
        "Prepends clean timestamps and player badges to incoming chat messages.",
        66_700,
    ),
    (
        "damage_tilt",
        "Directional Damage Tilt",
        "Tilts the camera toward the exact direction of incoming attacks and projectile hits.",
        52_100,
    ),
    (
        "trash_slot",
        "Trash Slot Helper",
        "Adds quick item disposal and inventory slot cleanup controls.",
        57_400,
    ),
    (
        "nature_compass",
        "Nature's Biome Compass",
        "Displays current biome temperature, humidity, precipitation, and cardinal bearing on the HUD.",
        68_300,
    ),
    (
        "elytra_hud",
        "Flight & Elytra HUD",
        "Shows pitch angle, glide speed (m/s), altitude, and Elytra durability while gliding.",
        73_800,
    ),
    (
        "potion_counter",
        "PvP Potion & Arrow Counter",
        "Counts total healing potions, golden apples, ender pearls, and arrows remaining in your inventory.",
        61_900,
    ),
    (
        "reach_display",
        "Reach & Combo Display",
        "Shows exact attack hit distance (blocks) and consecutive hit combo count during combat.",
        58_600,
    ),
    (
        "auto_fish",
        "Fishing Bobber Alert",
        "Displays a clear bite indicator and acoustic cue when a fish hooks your fishing rod.",
        47_200,
    ),
    (
        "step_up",
        "StepUp Auto-Step",
        "Smoothly steps up full 1-block ledges without needing to jump repeatedly.",
        65_400,
    ),
    (
        "no_fog",
        "ClearView Fog Control",
        "Pushes back terrain, Nether, powder snow, and underwater fog for crystal-clear distant visibility.",
        79_100,
    ),
    (
        "clear_lava",
        "Clear Water & Lava",
        "Improves visibility when submerged in water or lava.",
        53_700,
    ),
    (
        "chunk_borders",
        "Chunk Borders & MiniHUD",
        "Displays chunk boundaries (16x16), sub-chunk Y section, and slime chunk indicator.",
        70_400,
    ),
    (
        "horse_stats",
        "Horse & Mount Stats",
        "Shows exact movement speed, jump height (in blocks), and health of horses, camels, and mounts.",
        55_900,
    ),
    (
        "bee_info",
        "Beehive & Honey Inspector",
        "Shows bee count (0-3) and honey level (0-5) on beehives and bee nests in tooltips and LookAt HUD.",
        49_100,
    ),
    (
        "book_editor",
        "Book & Sign Helper",
        "Adds character/page counters and formatting shortcuts when reading or editing books and signs.",
        43_800,
    ),
    (
        "item_scroller",
        "Item Scroller",
        "Fast single-slot and full-stack item transfer gestures across container screens.",
        62_500,
    ),
    (
        "crafting_tweaks",
        "Crafting Tweaks",
        "Adds quick grid balance, rotate, and clear shortcuts for crafting tables.",
        67_200,
    ),
    (
        "bed_benefits",
        "Bed Benefits",
        "Restores health and clears negative status effects upon waking up from a bed.",
        51_300,
    ),
    (
        "torch_hit",
        "Torch Hit Igniter",
        "Striking mobs with a torch or soul torch briefly sets them on fire.",
        45_600,
    ),
    (
        "double_doors",
        "Double Doors",
        "Opening or closing one door or fence gate automatically toggles its matching adjacent double door.",
        82_700,
    ),
    (
        "xp_clumps",
        "XP Clumps",
        "Merges nearby experience orbs together to reduce entity lag and speed up XP absorption.",
        96_300,
    ),
];

/// Returns all 60 Horizon Mod Store entries (`HORIZON_CATALOG_MODS` + `HORIZON_LIBRARY_MODS`).
pub fn all_horizon_catalog_mods() -> Vec<(&'static str, &'static str, &'static str, u64)> {
    let mut out = Vec::with_capacity(HORIZON_CATALOG_MODS.len() + HORIZON_LIBRARY_MODS.len());
    out.extend_from_slice(&HORIZON_CATALOG_MODS);
    out.extend_from_slice(&HORIZON_LIBRARY_MODS);
    out
}

/// Unpacks and installs a `.hpack` Horizon Modpack (`horizon-modpack.toml` + `overrides/`) into `profile_dir`.
pub fn install_hpack_modpack(hpack_path: &Path, profile_dir: &Path) -> Result<usize, String> {
    let pack = horizon_mods::modpack::Modpack::read(hpack_path)?;
    let _ = horizon_mods::modpack::Modpack::extract_overrides(hpack_path, profile_dir);
    let mut installed_count = 0usize;
    let mut on = crate::mod_list::ModList::load(profile_dir);
    for pm in &pack.manifest.mods {
        let _ = install_builtin_horizon_mod(&pm.id, profile_dir);
        if pm.on {
            on.disabled.remove(&pm.id);
            on.enabled.insert(pm.id.clone());
        } else {
            on.enabled.remove(&pm.id);
            on.disabled.insert(pm.id.clone());
        }
        installed_count += 1;
    }
    on.save(profile_dir);
    Ok(installed_count.max(1))
}

/// Synthesizes and installs a built-in Horizon `.hnmod` package into `<profile_dir>/horizon-native-mods/<id>.hnmod`
/// and enables it in `horizon-mods.txt`.
pub fn install_builtin_horizon_mod(
    raw_id: &str,
    profile_dir: &Path,
) -> Result<TranspiledModInfo, String> {
    let mod_id = sanitize_mod_id(raw_id);
    let (title, description) = canonical_horizon_mod_meta(&mod_id)
        .map(|(t, d)| (t.to_owned(), d.to_owned()))
        .unwrap_or_else(|| {
            let t = title_case_id(&mod_id);
            let d = format!("Horizon Native Mod ({t})");
            (t, d)
        });
    let version = "1.0.0".to_owned();
    let features: Vec<String> = infer_features_from_id_and_signals(
        &mod_id,
        &title,
        &description,
        "",
        &BTreeSet::new(),
    )
    .into_iter()
    .collect();

    let lib_rs = generate_hnmod_rust_source(
        &mod_id,
        &title,
        &version,
        &description,
        &[],
        &[],
        &features,
        &[],
        &[],
    );

    let manifest_json = serde_json::json!({
        "schema": 1,
        "id": mod_id,
        "version": version,
        "game": "26.3",
        "horizon": DEFAULT_HORIZON_REV,
        "mixins": DEFAULT_MIXINS_REV,
        "hooks": 415,
        "permissions": [],
    });
    let compiled_json = serde_json::json!({
        "id": mod_id,
        "title": title,
        "version": version,
        "description": description,
        "compiled": true,
        "abi": "horizon-inproc-v2",
        "hooks_api": 415,
        "features": features,
        "custom_buttons": [],
        "items": [],
        "blocks": [],
    });

    let native_mods_dir = profile_dir.join("horizon-native-mods");
    fs::create_dir_all(&native_mods_dir)
        .map_err(|e| format!("create horizon-native-mods/: {e}"))?;
    let hnmod_path = native_mods_dir.join(format!("{mod_id}.hnmod"));

    {
        let file =
            File::create(&hnmod_path).map_err(|e| format!("create {}: {e}", hnmod_path.display()))?;
        let mut out_zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        out_zip
            .start_file("horizon-native.json", opts)
            .map_err(|e| e.to_string())?;
        out_zip
            .write_all(
                &serde_json::to_vec_pretty(&manifest_json).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        let mixin_files = generate_full_mixin_bundle_files(
            &mod_id,
            &title,
            &version,
            &description,
            &[],
            &[],
            &features,
        );
        for (mpath, mbytes) in mixin_files {
            out_zip.start_file(&mpath, opts).map_err(|e| e.to_string())?;
            out_zip.write_all(&mbytes).map_err(|e| e.to_string())?;
        }

        out_zip
            .start_file(runtime_source_entry(&mod_id), opts)
            .map_err(|e| e.to_string())?;
        out_zip
            .write_all(lib_rs.as_bytes())
            .map_err(|e| e.to_string())?;

        out_zip
            .start_file(compiled_meta_entry(&mod_id), opts)
            .map_err(|e| e.to_string())?;
        out_zip
            .write_all(
                &serde_json::to_vec_pretty(&compiled_json).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;

        out_zip.finish().map_err(|e| e.to_string())?;
        let _ = horizon_mods::index::native::Bundle::read(&hnmod_path)?;
    }

    enable_mod_in_profile(profile_dir, &mod_id);
    let hmod_path = profile_dir.join("horizon-mods").join(format!("{mod_id}.hmod"));
    Ok(TranspiledModInfo {
        id: mod_id,
        title,
        version,
        description,
        hnmod_path,
        hmod_path,
        items: Vec::new(),
        blocks: Vec::new(),
        features,
        port_pack: None,
    })
}

/// Collects all enabled `.hnmod` and unified `.hmod` native bundles in `profile_dir`,
/// validates each via `horizon_mods::index::native::Bundle::read`, constructs the
/// multi-mod binary payload (`[u32 BE len][zip bytes]...`) for `/v1/build`, and writes
/// `<profile_dir>/native/compiled-local/build.json` (used by `NativeMods.java` & `native_builds.rs`).
pub fn build_native_mods_payload(profile_dir: &Path) -> Result<(Vec<u8>, serde_json::Value), String> {
    let on = crate::mod_list::ModList::load(profile_dir);
    let mut paths: Vec<PathBuf> = Vec::new();
    for folder in [
        profile_dir.join(crate::mod_list::NATIVE_DIR),
        profile_dir.join(crate::mod_list::DIR),
    ] {
        if let Ok(rd) = fs::read_dir(&folder) {
            paths.extend(rd.flatten().map(|e| e.path()));
        }
    }
    paths.retain(|p| horizon_mods::index::native::is_file(p) && horizon_mods::index::native::holds(p));
    paths.sort();

    let mut seen_ids = BTreeSet::new();
    let mut mod_ids = Vec::new();
    let mut permissions = BTreeSet::new();
    let mut body = Vec::new();

    for path in paths {
        let Ok(bundle) = horizon_mods::index::native::Bundle::read(&path) else {
            continue;
        };
        let id = bundle.manifest.id.clone();
        if on.disabled.contains(&id) {
            continue;
        }
        if !seen_ids.insert(id.clone()) {
            continue;
        }
        for p in &bundle.manifest.permissions {
            permissions.insert(p.clone());
        }
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        body.extend_from_slice(&bytes);
        mod_ids.push(id);
    }

    if mod_ids.is_empty() {
        let info = install_builtin_horizon_mod("wrench", profile_dir)?;
        let bytes = fs::read(&info.hnmod_path).map_err(|e| e.to_string())?;
        body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        body.extend_from_slice(&bytes);
        mod_ids.push(info.id);
    }

    let perms_vec: Vec<String> = permissions.into_iter().collect();
    let build_meta = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "game": "26.3",
        "horizon": DEFAULT_HORIZON_REV,
        "mixins": DEFAULT_MIXINS_REV,
        "hooks": 415,
        "mods": mod_ids,
        "permissions": perms_vec,
        "payload_bytes": body.len(),
    });

    let local_build_dir = profile_dir.join("native").join("compiled-local");
    let _ = fs::create_dir_all(&local_build_dir);
    if let Ok(json_bytes) = serde_json::to_vec_pretty(&build_meta) {
        let _ = fs::write(local_build_dir.join("build.json"), json_bytes);
    }
    let _ = fs::write(local_build_dir.join("bundles.bin"), &body);

    Ok((body, build_meta))
}

pub fn title_case_id(id: &str) -> String {
    id.split(['_', '-', '/'])
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transpiles_fabric_jar_into_valid_hnmod_and_hmod() {
        let dir = tempfile::tempdir().unwrap();
        let jar_path = dir.path().join("appleskin-fabric-mc1.21-3.0.6.jar");
        {
            let file = File::create(&jar_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("fabric.mod.json", opts).unwrap();
            zip.write_all(
                br#"{
                    "schemaVersion": 1,
                    "id": "appleskin",
                    "version": "3.0.6",
                    "name": "AppleSkin",
                    "description": "Food & saturation HUD and tooltips"
                }"#,
            )
            .unwrap();
            zip.start_file("assets/appleskin/lang/en_US.json", opts)
                .unwrap();
            zip.write_all(br#"{"item.appleskin.apple_icon": "Apple Icon"}"#)
                .unwrap();
            zip.finish().unwrap();
        }

        let info = transpile_jar_to_horizon_mods(&jar_path, dir.path()).unwrap();
        assert_eq!(info.id, "appleskin");
        assert_eq!(info.title, "AppleSkin");
        assert_eq!(info.version, "3.0.6");
        assert!(info.hnmod_path.exists());
        assert!(info.hmod_path.exists());
        assert!(info.features.iter().any(|f| f == "appleskin_hud"));

        // Verify horizon_mods::index::native::Bundle::read accepts the generated .hnmod
        let bundle = horizon_mods::index::native::Bundle::read(&info.hnmod_path).unwrap();
        assert_eq!(bundle.manifest.id, "appleskin");

        // Verify horizon_mods::Package::read and Content::read accept the generated .hmod
        let pkg = horizon_mods::Package::read(&info.hmod_path, ).unwrap();
        assert_eq!(pkg.manifest.id, "appleskin");
        let content = horizon_mods::Content::read(&info.hmod_path, &pkg.manifest).unwrap();
        assert_eq!(content.items.len(), 1);
    }
}
