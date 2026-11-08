// Wrench's rules, with nothing of the game in them: which states a block may
// turn to and which blocks come apart. Injected as `crate::game::wrench`;
// native.rs applies them to the world.
//
// Follows the Wrench of Create (by simibubi and the Create team): a right
// click turns the block aimed at, a sneaking right click takes a machine
// apart. Create's items and blocks are not here; the wrench is a vanilla item
// (a stick unless the player picks another) and it works on vanilla blocks.
// The code is this mod's own.
use std::collections::BTreeMap;

pub const ID: &str = "wrench";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str = "A stick turns blocks: a right click turns the block you aim at a \
     quarter turn, a sneaking right click takes a machine apart. After Create's Wrench.";

/// The setting naming the wrench (an item id).
pub const ITEM_SETTING: &str = "item";
/// The setting that lets sneaking take machines apart (`on`, `off`).
pub const TAKE_APART_SETTING: &str = "take-apart";
/// The items the settings page cycles through; any other id can be written in
/// the setting's file.
pub const ITEMS: [&str; 4] = [
    "minecraft:stick",
    "minecraft:blaze_rod",
    "minecraft:breeze_rod",
    "minecraft:bone",
];

pub type Properties = BTreeMap<String, String>;

/// A direction as a unit vector.
pub type Dir = [i32; 3];
const UP: Dir = [0, 1, 0];
const DIRECTIONS: [(&str, Dir); 6] = [
    ("down", [0, -1, 0]),
    ("up", [0, 1, 0]),
    ("north", [0, 0, -1]),
    ("south", [0, 0, 1]),
    ("west", [-1, 0, 0]),
    ("east", [1, 0, 0]),
];
const AXES: [(&str, Dir); 3] = [("x", [1, 0, 0]), ("y", [0, 1, 0]), ("z", [0, 0, 1])];

fn direction(name: &str) -> Option<Dir> {
    DIRECTIONS.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}
fn direction_name(d: Dir) -> Option<&'static str> {
    DIRECTIONS.iter().find(|(_, v)| *v == d).map(|(n, _)| *n)
}
fn axis(name: &str) -> Option<Dir> {
    AXES.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}
fn axis_name(d: Dir) -> Option<&'static str> {
    let d = d.map(i32::abs);
    AXES.iter().find(|(_, v)| *v == d).map(|(n, _)| *n)
}
fn opposite(d: Dir) -> Dir {
    d.map(|v| -v)
}

/// `v` a quarter turn clockwise as seen looking at the face whose outward
/// normal is `face` (`v` × `face`); `v` itself when it lies along `face`.
pub fn turned(v: Dir, face: Dir) -> Dir {
    let c = [
        v[1] * face[2] - v[2] * face[1],
        v[2] * face[0] - v[0] * face[2],
        v[0] * face[1] - v[1] * face[0],
    ];
    if c == [0, 0, 0] { v } else { c }
}

fn short(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

/// Whether the wrench may turn this block at all: never half of a bigger
/// thing (beds, doors, tall plants, double chests, an extended piston and its
/// head), a portal, or a game master block.
pub fn turnable(name: &str, properties: &Properties) -> bool {
    let name = short(name);
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

/// The states to try, best first, when face `face` of this block is clicked:
/// each is the block's properties with one change. The game keeps the first
/// that exists and can stay where it is. Empty when nothing of the block
/// turns.
///
/// - Stairs and trapdoors (`half` top or bottom) flip upside down when a side
///   is clicked; slabs (`type` top or bottom) flip from any face.
/// - `facing` turns a quarter clockwise around the clicked face; when that
///   cannot be (it points along that face, or the block only faces sideways)
///   it turns around the vertical, then turns back to front.
/// - `axis` (logs, pillars, chains) turns around the clicked face, else goes
///   on to the next axis.
/// - `rotation` (standing signs, banners, heads) turns a quarter.
pub fn turns(name: &str, properties: &Properties, face: Dir) -> Vec<Properties> {
    if !turnable(name, properties) {
        return Vec::new();
    }
    let mut out: Vec<Properties> = Vec::new();
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
    if let Some(f) = properties.get("facing").and_then(|f| direction(f)) {
        for to in [turned(f, face), turned(f, UP), opposite(f)] {
            if let Some(to) = direction_name(to) {
                push("facing", to);
            }
        }
    }
    if let Some(a) = properties.get("axis").and_then(|a| axis(a)) {
        if let Some(to) = axis_name(turned(a, face)) {
            push("axis", to);
        }
        let at = AXES.iter().position(|(_, v)| *v == a).unwrap_or(0);
        for step in 1..AXES.len() {
            push("axis", AXES[(at + step) % AXES.len()].0);
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

/// Whether a sneaking right click takes this block apart: redstone parts and
/// machines, which Create's wrench is for, never blocks a pickaxe is for.
pub fn takes_apart(name: &str) -> bool {
    let name = short(name);
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

/// The wrench's item from its setting (an id, `stick` meaning
/// `minecraft:stick`); a stick when unset.
pub fn wrench_item(setting: Option<&str>) -> String {
    match setting.map(str::trim) {
        Some(id) if !id.is_empty() && id.contains(':') => id.to_owned(),
        Some(id) if !id.is_empty() => format!("minecraft:{id}"),
        _ => ITEMS[0].to_owned(),
    }
}
/// The next item of [`ITEMS`] after `item` (the first after one not listed).
pub fn next_item(item: &str) -> &'static str {
    let at = ITEMS.iter().position(|i| *i == item);
    ITEMS[at.map_or(0, |i| (i + 1) % ITEMS.len())]
}
/// `minecraft:blaze_rod` as a button says it: "Blaze Rod".
pub fn item_label(item: &str) -> String {
    short(item)
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
/// Taking apart is on unless its setting says `off`.
pub fn take_apart_on(setting: Option<&str>) -> bool {
    setting.map(str::trim) != Some("off")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> Properties {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }
    fn first(name: &str, pairs: &[(&str, &str)], face: Dir) -> Option<Properties> {
        turns(name, &props(pairs), face).into_iter().next()
    }

    #[test]
    fn a_furnace_turns_clockwise_seen_from_above() {
        let after = first(
            "minecraft:furnace",
            &[("facing", "north"), ("lit", "false")],
            UP,
        );
        assert_eq!(after, Some(props(&[("facing", "east"), ("lit", "false")])));
    }

    #[test]
    fn a_side_click_on_a_sideways_block_still_turns_it_around_the_vertical() {
        // Turning north around the west face gives up, which a furnace lacks:
        // the game skips it and takes the next, around the vertical.
        let all = turns(
            "minecraft:furnace",
            &props(&[("facing", "north")]),
            [-1, 0, 0],
        );
        assert_eq!(all[0], props(&[("facing", "up")]));
        assert_eq!(all[1], props(&[("facing", "east")]));
    }

    #[test]
    fn a_piston_facing_up_clicked_on_top_turns_back_to_front() {
        let after = first(
            "minecraft:piston",
            &[("facing", "up"), ("extended", "false")],
            UP,
        );
        assert_eq!(
            after,
            Some(props(&[("facing", "down"), ("extended", "false")]))
        );
    }

    #[test]
    fn stairs_flip_from_a_side_and_turn_from_the_top() {
        let base = [
            ("facing", "north"),
            ("half", "bottom"),
            ("shape", "straight"),
        ];
        let side = first("minecraft:oak_stairs", &base, [0, 0, -1]).unwrap();
        assert_eq!(side["half"], "top");
        let top = first("minecraft:oak_stairs", &base, UP).unwrap();
        assert_eq!(
            (top["facing"].as_str(), top["half"].as_str()),
            ("east", "bottom")
        );
    }

    #[test]
    fn a_log_turns_around_the_clicked_face() {
        let after = first("minecraft:oak_log", &[("axis", "y")], [0, 0, 1]).unwrap();
        assert_eq!(after["axis"], "x");
        // Clicked on its end, it goes on to the next axis.
        let end = first("minecraft:oak_log", &[("axis", "y")], UP).unwrap();
        assert_eq!(end["axis"], "z");
    }

    #[test]
    fn halves_of_bigger_things_never_turn() {
        let none = |name: &str, extra: (&str, &str)| {
            turns(name, &props(&[("facing", "north"), extra]), UP).is_empty()
        };
        assert!(none("minecraft:oak_door", ("half", "lower")));
        assert!(none("minecraft:red_bed", ("part", "head")));
        assert!(none("minecraft:chest", ("type", "left")));
        assert!(none("minecraft:piston", ("extended", "true")));
        assert!(none("minecraft:command_block", ("powered", "false")));
        assert!(!none("minecraft:chest", ("type", "single")));
    }

    #[test]
    fn machines_come_apart_and_ores_do_not() {
        assert!(takes_apart("minecraft:observer"));
        assert!(takes_apart("minecraft:stone_button"));
        assert!(takes_apart("minecraft:weathered_lightning_rod"));
        assert!(!takes_apart("minecraft:diamond_ore"));
        assert!(!takes_apart("minecraft:obsidian"));
    }

    #[test]
    fn the_item_setting() {
        assert_eq!(wrench_item(None), "minecraft:stick");
        assert_eq!(wrench_item(Some("blaze_rod")), "minecraft:blaze_rod");
        assert_eq!(next_item("minecraft:bone"), "minecraft:stick");
        assert_eq!(item_label("minecraft:breeze_rod"), "Breeze Rod");
        assert!(take_apart_on(None) && !take_apart_on(Some("off")));
    }
}
