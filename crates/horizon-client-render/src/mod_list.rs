//! Which of the installed Horizon mods are on (`docs/mods.md`), kept in
//! `horizon-mods.txt` beside `options.txt` as `mod.<id>:true` lines. Every
//! mod is off until the player turns it on in Options > Mods. A native mod is
//! built in, so on until turned off (`mod.<id>:false`); the launcher builds
//! only the native mods whose line is `true`.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
};

pub const FILE: &str = "horizon-mods.txt";

static LIVE_MOD_STATE: OnceLock<Mutex<(BTreeSet<String>, BTreeSet<String>)>> = OnceLock::new();

fn live_state() -> &'static Mutex<(BTreeSet<String>, BTreeSet<String>)> {
    LIVE_MOD_STATE.get_or_init(|| Mutex::new((BTreeSet::new(), BTreeSet::new())))
}

/// Updates the process-wide live mod enable/disable sets so both the Client Window thread
/// and the Singleplayer Integrated Server thread observe mod toggles immediately.
pub fn sync_live_state(enabled: &BTreeSet<String>, disabled: &BTreeSet<String>) {
    if let Ok(mut guard) = live_state().lock() {
        guard.0 = enabled.clone();
        guard.1 = disabled.clone();
    }
}

/// Checks if `mod_id` has an authoritative live state (`Some(true)` = disabled, `Some(false)` = enabled).
pub fn is_live_disabled(mod_id: &str) -> Option<bool> {
    let guard = live_state().lock().ok()?;
    if guard.0.contains(mod_id) {
        Some(false)
    } else if guard.1.contains(mod_id) {
        Some(true)
    } else {
        None
    }
}

/// Fixed native modules compiled into Android APKs, all opt-in.
pub const ANDROID_BUNDLED: [(&str, &str, &str); 4] = [
    (
        "zoom",
        "Zoom",
        "Hold Z to zoom; configurable in Options > Mods",
    ),
    (
        "minimap",
        "Minimap",
        "A configurable map in the screen corner",
    ),
    (
        "recipes",
        "Recipes",
        "Item recipes and uses beside the inventory",
    ),
    (
        "smartf3",
        "SmartF3",
        "Configurable modules for the F3 debug screen",
    ),
];

/// The folder holding `.hmod` files, beside `options.txt`.
pub const DIR: &str = "horizon-mods";
/// Where the launcher installs native mods (`<id>.hnmod`), beside [`DIR`]. They run only in a
/// game the launcher built with them.
pub const NATIVE_DIR: &str = "horizon-native-mods";
/// Why a native mod installed in [`NATIVE_DIR`] doesn't run in this game.
#[cfg(not(target_os = "android"))]
pub const NOT_BUILT: &str = "Installed, not built into this game: build the game with it in the launcher (Settings > Native mods), then play the instance that build is in.";
#[cfg(target_os = "android")]
pub const NOT_BUILT: &str =
    "Installed, not in this game yet: Compile builds the game with it on Horizon's build server.";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModList {
    /// Ids of the mods the player turned on.
    pub enabled: BTreeSet<String>,
    /// Ids of the native mods the player turned off: they rest
    /// (`native_on`) until the launcher's next build leaves them out.
    pub disabled: BTreeSet<String>,
    /// The keys the player chose for mods' keys, mod id → key name → key
    /// (`key.<id>.<name>:key.keyboard.x` lines).
    pub keys: BTreeMap<String, BTreeMap<String, String>>,
    /// What the Mods page shows, filled by the game from the mods it found.
    pub installed: Vec<Installed>,
}

/// One installed mod as the Mods page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    pub id: String,
    /// `Name 1.0` (the file name when the manifest is unreadable).
    pub title: String,
    pub description: String,
    /// Why it is not running, when it failed.
    pub problem: Option<String>,
    /// Loaded with `--dev-mod`: always on, reloaded when rebuilt.
    pub dev: bool,
    /// The running mod set a page of widgets (capability `ui`): its row
    /// gains a Settings button.
    pub has_settings: bool,
    /// A native mod (`game/native_hooks.rs`): built into the game, on until
    /// the player turns it off, and its settings page.
    pub native: Option<NativePage>,
}

/// A native mod's settings page: its keys as Key Binds rows, then its
/// buttons. The keys the player chooses are kept in `ModList::keys`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativePage {
    /// Each key's name, label and default key (`key.keyboard.z`).
    pub keys: Vec<[String; 3]>,
    pub buttons: Vec<String>,
}

impl ModList {
    pub fn load(dir: &Path) -> Self {
        let loaded = Self::parse(&fs::read_to_string(dir.join(FILE)).unwrap_or_default());
        sync_live_state(&loaded.enabled, &loaded.disabled);
        loaded
    }
    pub fn parse(text: &str) -> Self {
        let mut enabled = BTreeSet::new();
        let mut disabled = BTreeSet::new();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            if let Some((k, v)) = line.split_once(':') {
                let k = k.trim();
                let v = v.trim();
                if let Some(id) = k.strip_prefix("mod.") {
                    if v == "true" {
                        enabled.insert(id.to_owned());
                    } else if v == "false" {
                        disabled.insert(id.to_owned());
                    }
                }
            } else {
                let id = line.strip_prefix("mod.").unwrap_or(line);
                enabled.insert(id.to_owned());
            }
        }
        #[cfg(target_os = "android")]
        let disabled = {
            let mut disabled = disabled;
            for (id, _, _) in ANDROID_BUNDLED {
                if !enabled.contains(id) {
                    disabled.insert(id.to_owned());
                }
            }
            disabled
        };
        let mut keys: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for (k, v) in text.lines().filter_map(|l| l.split_once(':')) {
            // Mod ids have no dots; a key's name may.
            if let Some((id, name)) = k
                .trim()
                .strip_prefix("key.")
                .and_then(|r| r.split_once('.'))
            {
                let entry = keys.entry(id.to_owned()).or_default();
                entry.insert(name.to_owned(), v.trim().to_owned());
            }
        }
        Self {
            enabled,
            disabled,
            keys,
            installed: Vec::new(),
        }
    }
    pub fn serialize(&self) -> String {
        let on = self.enabled.iter().map(|id| format!("mod.{id}:true\n"));
        let off = self.disabled.iter().map(|id| format!("mod.{id}:false\n"));
        let keys = self.keys.iter().flat_map(|(id, keys)| {
            keys.iter()
                .map(move |(name, key)| format!("key.{id}.{name}:{key}\n"))
        });
        on.chain(off).chain(keys).collect()
    }
    /// Mod `id`'s chosen keys, for `Mods::set_bindings`.
    pub fn keys_of(&self, id: &str) -> Vec<(&str, &str)> {
        self.keys.get(id).map_or_else(Vec::new, |k| {
            k.iter().map(|(n, k)| (n.as_str(), k.as_str())).collect()
        })
    }
    pub fn save(&self, dir: &Path) {
        sync_live_state(&self.enabled, &self.disabled);
        let path = dir.join(FILE);
        if self.enabled.is_empty()
            && self.disabled.is_empty()
            && self.keys.is_empty()
            && !path.exists()
        {
            return;
        }
        let _ = fs::write(path, self.serialize());
    }
    pub fn is_on(&self, i: usize) -> bool {
        self.installed.get(i).is_some_and(|m| {
            m.dev
                || if m.native.is_some() {
                    !self.disabled.contains(&m.id)
                } else {
                    self.enabled.contains(&m.id)
                }
        })
    }
    /// Turns installed mod `i` on or off (a dev mod stays on). A native mod's
    /// line says `false` when off, so that the launcher's next build leaves
    /// it out, and `true` when on again.
    /// Turn the installed mod `id` on (nothing when it isn't installed).
    pub fn turn_on(&mut self, id: &str) {
        let Some(m) = self.installed.iter().find(|m| m.id == id && !m.dev) else {
            return;
        };
        if m.native.is_some() {
            self.disabled.remove(&m.id);
        }
        self.enabled.insert(m.id.clone());
        sync_live_state(&self.enabled, &self.disabled);
    }
    pub fn toggle(&mut self, i: usize) {
        let Some(m) = self.installed.get(i).filter(|m| !m.dev) else {
            return;
        };
        if m.native.is_some() {
            if self.disabled.remove(&m.id) {
                self.enabled.insert(m.id.clone());
            } else {
                self.enabled.remove(&m.id);
                self.disabled.insert(m.id.clone());
            }
        } else if !self.enabled.remove(&m.id) {
            self.enabled.insert(m.id.clone());
        }
        sync_live_state(&self.enabled, &self.disabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_ignores_the_rest() {
        let list = ModList::parse("mod.coords_hud:true\nmod.other:false\nzoom:true\n");
        assert_eq!(list.enabled.iter().collect::<Vec<_>>(), ["coords_hud"]);
        assert_eq!(ModList::parse(&list.serialize()), list);
        assert_eq!(ModList::default().serialize(), "");
    }

    #[test]
    fn keeps_the_keys_players_chose() {
        let text = "mod.zoom:true\nkey.zoom.zoom.in:key.mouse.4\nkey.zoom.out:key.keyboard.x\n";
        let list = ModList::parse(text);
        assert_eq!(
            list.keys_of("zoom"),
            [("out", "key.keyboard.x"), ("zoom.in", "key.mouse.4")]
        );
        assert!(list.keys_of("other").is_empty());
        assert_eq!(ModList::parse(&list.serialize()), list);
    }

    #[test]
    fn toggles_installed_and_native_mods_but_not_dev_ones() {
        let installed = |id: &str, dev| Installed {
            id: id.into(),
            title: id.into(),
            description: String::new(),
            problem: None,
            dev,
            has_settings: false,
            native: None,
        };
        let mut list = ModList {
            installed: vec![installed("a", false), installed("b", true)],
            ..Default::default()
        };
        assert!(!list.is_on(0) && list.is_on(1));
        list.toggle(0);
        list.toggle(1);
        assert!(list.is_on(0) && list.is_on(1));
        assert_eq!(list.serialize(), "mod.a:true\n");
        list.toggle(0);
        assert!(!list.is_on(0));
        list.installed.push(Installed {
            native: Some(NativePage::default()),
            ..installed("c", false)
        });
        // On until turned off; off, the launcher leaves it out.
        assert!(list.is_on(2));
        list.toggle(2);
        assert!(!list.is_on(2));
        assert_eq!(list.serialize(), "mod.c:false\n");
        assert_eq!(ModList::parse(&list.serialize()).disabled, list.disabled);
        list.toggle(2);
        assert!(list.is_on(2));
        assert_eq!(list.serialize(), "mod.c:true\n");
    }
}
