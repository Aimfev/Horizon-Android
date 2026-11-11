//! The design system's widgets drawn in the game (`crates/horizon-theme`:
//! palette, type scale, radii, control sizes, motion and the bundled fonts;
//! `docs/design-system.md`). Every menu is vanilla's (`ui`); what is left
//! here draws Horizon's own overlays that vanilla has no screen for: the
//! on-screen keyboard a controller types with (`osk`), plus the
//! `horizon.txt` preferences.
//!
//! Build a [`Ui`] over the frame's `HudBatch` with [`Ui::new`], lay out in dp
//! (the launcher's logical pixels) and give every interactive widget a stable
//! `id`. Clicks run the same layout with [`Ui::dry`] and read [`Ui::hit`]: one
//! function draws and hit-tests, so they never disagree.
use crate::hud::{HudAtlas, HudBatch, Rect};
use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};
use std::{cell::RefCell, collections::HashMap, path::Path};

/// A Horizon-only preference from `horizon.txt` beside `options.txt`
/// (`key:value` lines), so vanilla's options file stays exactly as vanilla
/// writes it.
pub fn preference(dir: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("horizon.txt")).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_owned())
    })
}
/// Set (or with None remove) one preference, keeping the others.
pub(crate) fn set_preference(dir: &Path, key: &str, value: Option<&str>) {
    let text = std::fs::read_to_string(dir.join("horizon.txt")).unwrap_or_default();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| l.split_once(':').is_none_or(|(k, _)| k.trim() != key))
        .map(str::to_owned)
        .collect();
    if let Some(v) = value {
        lines.push(format!("{key}:{v}"));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    let _ = std::fs::write(dir.join("horizon.txt"), out);
}

/// The dark palette (`horizon_theme::DARK`) in the HUD's colour encoding, plus the
/// translucent plates the overlays draw over the game.
pub mod color {
    use horizon_theme::DARK as P;
    pub const fn hex(c: u32, a: u8) -> u32 {
        (a as u32) << 24 | (c & 0xff) << 16 | (c >> 8 & 0xff) << 8 | (c >> 16 & 0xff)
    }
    /// The page, under every veil.
    pub const BG: u32 = hex(P.page, 255);
    /// Hover plates, secondary buttons, fields, the segmented track: the
    /// dark palette's soft plate, as ink over the veil so the plates read the
    /// same over any backdrop (over the page colour they are `plate_soft`).
    pub const RAISED: u32 = hex(P.ink, 20);
    /// Pressed plates and a switch that is off (`plate`, the same way).
    pub const HOVER: u32 = hex(P.ink, 36);
    /// One-pixel rules between rows, slider tracks.
    pub const BORDER: u32 = hex(P.rule, 255);
    /// Text and selection (`ink`).
    pub const TEXT: u32 = hex(P.ink, 255);
    /// Values and secondary text (`ink_soft`).
    pub const MUTED: u32 = hex(P.ink_soft, 255);
    pub const ACCENT: u32 = hex(P.accent, 255);
    pub const ON_ACCENT: u32 = hex(P.on_accent, 255);
    pub fn with_alpha(c: u32, a: f32) -> u32 {
        let base = (c >> 24) as f32;
        (c & 0x00ff_ffff) | ((base * a.clamp(0.0, 1.0)).round() as u32) << 24
    }
}

pub type Color = u32;

/// The launcher's type scale and shapes (`horizon_theme`), in dp.
pub(crate) const SMALL: f32 = horizon_theme::CAPTION.px;
pub(crate) const BODY: f32 = horizon_theme::LABEL.px;
/// The boot screen's wordmark (`DISPLAY`) and console lines (`MICRO`).
pub(crate) const DISPLAY: f32 = horizon_theme::DISPLAY.px;
pub(crate) const MICRO: f32 = horizon_theme::MICRO.px;
/// Fields; plates (hover rows, selection); pills (buttons, segments).
pub(crate) const RADIUS: f32 = horizon_theme::RADIUS;
pub(crate) const RADIUS_PLATE: f32 = horizon_theme::RADIUS_PLATE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Regular,
    Medium,
    /// The wordmark and page titles (the design system's 500).
    SemiBold,
    /// Counts, timestamps and the boot screen's console lines.
    Mono,
}
impl Face {
    #[allow(non_upper_case_globals)]
    pub const Semibold: Self = Self::SemiBold;
}

/// Vector icons used by the Material You 3 Android launcher navigation rail,
/// top bar, and progress controls.
static ANDROID_DPI_BITS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static UI_SCALE_PERCENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);

pub fn set_android_dpi(dpi: f32) {
    if dpi.is_finite() && dpi >= 1.0 {
        ANDROID_DPI_BITS.store(
            dpi.clamp(1.0, 5.0).to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
    }
}

pub fn set_ui_scale_percent(percent: u32) {
    let clamped = if percent == 0 {
        100
    } else {
        percent.clamp(75, 165)
    };
    UI_SCALE_PERCENT.store(clamped, std::sync::atomic::Ordering::Relaxed);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    HorizonLogo,
    ChevronDown,
    Play,
    Mods,
    Packs,
    Shaders,
    Installed,
    Settings,
    Pause,
    Close,
    Search,
    Check,
    Monitor,
    Gear,
    Flask,
    Terminal,
    MicrosoftGrid,
    Gamepad,
    ArrowLeft,
    MenuEquals,
    Undo,
    Redo,
    Plus,
    Crosshair,
    Pencil,
    Copy,
    ArrowRight,
    Folder,
    FileText,
    Trash,
    Share,
    Skins,
}

struct Fonts {
    faces: [FontRef<'static>; 4],
}
impl Fonts {
    fn load() -> Self {
        let f = |b: &'static [u8]| FontRef::try_from_slice(b).expect("bundled font");
        let _fallback_bricolage = (
            f(horizon_theme::fonts::BRICOLAGE_REGULAR),
            f(horizon_theme::fonts::BRICOLAGE_MEDIUM),
            f(horizon_theme::fonts::BRICOLAGE_SEMIBOLD),
        );
        Self {
            faces: [
                f(horizon_theme::fonts::GOOGLE_SANS_REGULAR),
                f(horizon_theme::fonts::GOOGLE_SANS_MEDIUM),
                f(horizon_theme::fonts::GOOGLE_SANS_SEMIBOLD),
                f(horizon_theme::fonts::GEIST_MONO),
            ],
        }
    }
    fn face(&self, f: Face) -> &FontRef<'static> {
        &self.faces[f as usize]
    }
    /// The ab_glyph scale for an em size in pixels.
    fn scale(&self, f: Face, px: f32) -> PxScale {
        let font = self.face(f);
        let upem = font.units_per_em().unwrap_or(1000.0);
        PxScale::from(px * font.height_unscaled() / upem)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Glyph(Face, u16, u16),
    /// A quarter disc (`0`) or a quarter ring of `(radius_px, thickness_px)`.
    Corner(u16, u16),
    /// A supersampled vector icon at this pixel size.
    Vector(Icon, u16),
}
#[derive(Clone, Copy)]
struct Cached {
    rect: Option<Rect>,
    /// Offset of the bitmap from the pen (glyphs) in pixels.
    left: f32,
    top: f32,
}
/// Glyphs and corner masks rasterized into the atlas region `horizon/ui-text`.
struct Cache {
    fonts: Fonts,
    entries: HashMap<Key, Cached>,
    images: HashMap<String, Rect>,
    shelf: (u32, u32, u32),
    image_shelf: (u32, u32, u32),
    pending: Vec<crate::font::Patch>,
    glyph_scratch: Vec<(f32, f32, Rect)>,
    /// The atlas the entries were uploaded to (a new atlas starts over).
    atlas: usize,
}
thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}
/// Reloaded atlases can occupy the same address with empty run-time slots.
pub fn reset_atlas() {
    CACHE.with(|cache| *cache.borrow_mut() = None);
}
fn with_cache<R>(atlas: &HudAtlas, f: impl FnOnce(&mut Cache) -> R) -> R {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let id = atlas as *const HudAtlas as usize;
        let cache = c.get_or_insert_with(|| Cache {
            fonts: Fonts::load(),
            entries: HashMap::with_capacity(512),
            images: HashMap::with_capacity(64),
            shelf: (0, 0, 0),
            image_shelf: (0, 320, 0),
            pending: Vec::with_capacity(64),
            glyph_scratch: Vec::with_capacity(128),
            atlas: id,
        });
        if cache.atlas != id {
            cache.atlas = id;
            cache.entries.clear();
            cache.images.clear();
            cache.shelf = (0, 0, 0);
            cache.image_shelf = (0, 320, 0);
            cache.pending.clear();
        }
        f(cache)
    })
}

fn seg_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 1e-6 {
        ((px - ax) * dx + (py - ay) * dy) / len2
    } else {
        0.0
    }
    .clamp(0.0, 1.0);
    ((px - (ax + t * dx)).powi(2) + (py - (ay + t * dy)).powi(2)).sqrt()
}

/// The colour Horizon sunset logo as a 96x96 pixel grid, run-length encoded.
/// Palette entries are `0xRRGGBB`; transparent cells (the rounded corners) are absent.
const HORIZON_LOGO_GRID: u32 = 96;
static HORIZON_LOGO_PALETTE: [u32; 18] = [0xFFE6A5, 0xDA9A72, 0x573474, 0x402D6F, 0x3F2972, 0x2F2465, 0x222054, 0x1D1C4E, 0x1B1951, 0x171748, 0x141543, 0x12163A, 0x101439, 0x101337, 0x0E1234, 0x0D102F, 0x0B0E29, 0x03040B];
/// `(first_row, row_count, first_col, col_count, palette_index)`.
static HORIZON_LOGO_RUNS: [(u8, u8, u8, u8, u8); 998] = [
    (0,1,16,2,7), (0,1,18,60,11), (0,1,78,2,7), (1,1,13,1,6), (1,1,14,1,7), (1,1,15,1,11),
    (1,1,16,1,14), (1,1,17,62,15), (1,1,79,1,14), (1,1,80,1,11), (1,1,81,1,7), (1,1,82,1,6),
    (2,1,11,1,7), (2,1,12,1,11), (2,1,13,1,13), (2,1,14,68,15), (2,1,82,1,13), (2,1,83,1,11),
    (2,1,84,1,7), (3,1,9,1,7), (3,1,10,1,10), (3,1,11,1,13), (3,1,12,73,14), (3,1,85,1,10),
    (3,1,86,1,7), (4,1,8,1,6), (4,1,9,1,11), (4,1,10,76,14), (4,1,86,1,11), (4,1,87,1,6),
    (5,1,7,1,7), (5,1,8,1,11), (5,1,9,1,14), (5,1,10,1,13), (5,1,11,2,14), (5,1,13,1,13),
    (5,1,14,2,14), (5,1,16,1,13), (5,1,17,2,14), (5,1,19,1,13), (5,1,20,2,14), (5,1,22,1,13),
    (5,1,23,2,14), (5,1,25,1,13), (5,1,26,2,14), (5,1,28,1,13), (5,1,29,2,14), (5,1,31,1,13),
    (5,1,32,2,14), (5,1,34,1,13), (5,1,35,2,14), (5,1,37,1,13), (5,1,38,2,14), (5,1,40,1,13),
    (5,1,41,2,14), (5,1,43,1,13), (5,1,44,2,14), (5,1,46,1,13), (5,1,47,2,14), (5,1,49,1,13),
    (5,1,50,2,14), (5,1,52,1,13), (5,1,53,2,14), (5,1,55,1,13), (5,1,56,2,14), (5,1,58,1,13),
    (5,1,59,2,14), (5,1,61,1,13), (5,1,62,2,14), (5,1,64,1,13), (5,1,65,2,14), (5,1,67,1,13),
    (5,1,68,2,14), (5,1,70,1,13), (5,1,71,2,14), (5,1,73,1,13), (5,1,74,2,14), (5,1,76,1,13),
    (5,1,77,2,14), (5,1,79,1,13), (5,1,80,2,14), (5,1,82,1,13), (5,1,83,2,14), (5,1,85,1,13),
    (5,1,86,1,14), (5,1,87,1,11), (5,1,88,1,7), (6,1,6,1,7), (6,1,7,82,13), (6,1,89,1,7),
    (7,1,5,1,6), (7,1,6,84,12), (7,1,90,1,6), (8,1,4,1,6), (8,1,5,1,10), (8,1,6,84,12),
    (8,1,90,1,10), (8,1,91,1,6), (9,1,3,1,6), (9,1,4,1,10), (9,1,5,1,11), (9,1,6,1,12),
    (9,1,7,2,11), (9,1,9,1,12), (9,1,10,2,11), (9,1,12,1,12), (9,1,13,2,11), (9,1,15,1,12),
    (9,1,16,2,11), (9,1,18,1,12), (9,1,19,2,11), (9,1,21,1,12), (9,1,22,2,11), (9,1,24,1,12),
    (9,1,25,2,11), (9,1,27,1,12), (9,1,28,2,11), (9,1,30,1,12), (9,1,31,2,11), (9,1,33,1,12),
    (9,1,34,2,11), (9,1,36,1,12), (9,1,37,2,11), (9,1,39,1,12), (9,1,40,2,11), (9,1,42,1,12),
    (9,1,43,2,11), (9,1,45,1,12), (9,1,46,2,11), (9,1,48,1,12), (9,1,49,2,11), (9,1,51,1,12),
    (9,1,52,2,11), (9,1,54,1,12), (9,1,55,2,11), (9,1,57,1,12), (9,1,58,2,11), (9,1,60,1,12),
    (9,1,61,2,11), (9,1,63,1,12), (9,1,64,2,11), (9,1,66,1,12), (9,1,67,2,11), (9,1,69,1,12),
    (9,1,70,2,11), (9,1,72,1,12), (9,1,73,2,11), (9,1,75,1,12), (9,1,76,2,11), (9,1,78,1,12),
    (9,1,79,2,11), (9,1,81,1,12), (9,1,82,2,11), (9,1,84,1,12), (9,1,85,2,11), (9,1,87,1,12),
    (9,1,88,2,11), (9,1,90,1,12), (9,1,91,1,10), (9,1,92,1,6), (10,1,3,1,7), (10,1,4,1,10),
    (10,1,5,2,11), (10,1,7,1,10), (10,1,8,2,11), (10,1,10,1,10), (10,1,11,2,11), (10,1,13,1,10),
    (10,1,14,2,11), (10,1,16,1,10), (10,1,17,2,11), (10,1,19,1,10), (10,1,20,2,11), (10,1,22,1,10),
    (10,1,23,2,11), (10,1,25,1,10), (10,1,26,2,11), (10,1,28,1,10), (10,1,29,2,11), (10,1,31,1,10),
    (10,1,32,2,11), (10,1,34,1,10), (10,1,35,2,11), (10,1,37,1,10), (10,1,38,2,11), (10,1,40,1,10),
    (10,1,41,2,11), (10,1,43,1,10), (10,1,44,2,11), (10,1,46,1,10), (10,1,47,2,11), (10,1,49,1,10),
    (10,1,50,2,11), (10,1,52,1,10), (10,1,53,2,11), (10,1,55,1,10), (10,1,56,2,11), (10,1,58,1,10),
    (10,1,59,2,11), (10,1,61,1,10), (10,1,62,2,11), (10,1,64,1,10), (10,1,65,2,11), (10,1,67,1,10),
    (10,1,68,2,11), (10,1,70,1,10), (10,1,71,2,11), (10,1,73,1,10), (10,1,74,2,11), (10,1,76,1,10),
    (10,1,77,2,11), (10,1,79,1,10), (10,1,80,2,11), (10,1,82,1,10), (10,1,83,2,11), (10,1,85,1,10),
    (10,1,86,2,11), (10,1,88,1,10), (10,1,89,2,11), (10,1,91,1,10), (10,1,92,1,7), (11,1,2,1,6),
    (11,1,3,90,10), (11,1,93,1,6), (12,1,2,1,7), (12,1,3,90,10), (12,1,93,1,7), (13,1,1,1,5),
    (13,1,2,1,9), (13,1,3,90,10), (13,1,93,1,9), (13,1,94,1,5), (14,1,1,1,6), (14,1,2,92,10),
    (14,1,94,1,6), (15,1,1,1,7), (15,1,2,92,9), (15,1,94,1,6), (16,1,0,1,5), (16,1,1,1,7),
    (16,1,2,92,9), (16,1,94,1,7), (16,1,95,1,5), (17,1,0,1,5), (17,1,1,94,9), (17,1,95,1,5),
    (18,1,0,1,6), (18,1,1,94,9), (18,1,95,1,6), (19,2,0,1,6), (19,2,1,94,8), (19,2,95,1,6),
    (21,2,0,1,5), (21,2,1,94,8), (21,2,95,1,5), (23,1,0,1,5), (23,1,1,39,8), (23,1,40,2,6),
    (23,1,42,1,8), (23,1,43,2,6), (23,1,45,1,8), (23,1,46,2,6), (23,1,48,1,8), (23,1,49,2,6),
    (23,1,51,1,8), (23,1,52,2,6), (23,1,54,1,8), (23,1,55,1,6), (23,1,56,39,8), (23,1,95,1,5),
    (24,1,0,1,5), (24,1,1,27,8), (24,1,28,2,6), (24,1,30,1,8), (24,1,31,35,6), (24,1,66,1,8),
    (24,1,67,1,6), (24,1,68,27,8), (24,1,95,1,5), (25,1,0,1,5), (25,1,1,2,6), (25,1,3,1,8),
    (25,1,4,2,6), (25,1,6,1,8), (25,1,7,2,6), (25,1,9,1,8), (25,1,10,2,6), (25,1,12,1,8),
    (25,1,13,2,6), (25,1,15,1,8), (25,1,16,2,6), (25,1,18,1,8), (25,1,19,2,6), (25,1,21,1,8),
    (25,1,22,2,6), (25,1,24,1,8), (25,1,25,47,6), (25,1,72,1,8), (25,1,73,2,6), (25,1,75,1,8),
    (25,1,76,2,6), (25,1,78,1,8), (25,1,79,2,6), (25,1,81,1,8), (25,1,82,2,6), (25,1,84,1,8),
    (25,1,85,2,6), (25,1,87,1,8), (25,1,88,2,6), (25,1,90,1,8), (25,1,91,2,6), (25,1,93,1,8),
    (25,1,94,1,6), (25,1,95,1,5), (26,2,0,1,5), (26,2,1,94,6), (26,2,95,1,5), (28,1,0,1,5),
    (28,1,1,33,6), (28,1,34,2,5), (28,1,36,1,6), (28,1,37,20,5), (28,1,57,1,6), (28,1,58,2,5),
    (28,1,60,1,6), (28,1,61,1,5), (28,1,62,33,6), (28,1,95,1,5), (29,1,0,1,5), (29,1,1,25,6),
    (29,1,26,43,5), (29,1,69,26,6), (29,1,95,1,5), (30,1,0,1,5), (30,1,1,15,6), (30,1,16,28,5),
    (30,1,44,1,3), (30,1,45,6,2), (30,1,51,1,3), (30,1,52,28,5), (30,1,80,15,6), (30,1,95,1,5),
    (31,1,0,3,5), (31,1,3,1,6), (31,1,4,2,5), (31,1,6,1,6), (31,1,7,2,5), (31,1,9,1,6),
    (31,1,10,30,5), (31,1,40,1,3), (31,1,41,1,2), (31,1,42,12,0), (31,1,54,1,2), (31,1,55,1,3),
    (31,1,56,28,5), (31,1,84,1,6), (31,1,85,2,5), (31,1,87,1,6), (31,1,88,2,5), (31,1,90,1,6),
    (31,1,91,2,5), (31,1,93,1,6), (31,1,94,2,5), (32,1,0,38,5), (32,1,38,1,2), (32,1,39,1,1),
    (32,1,40,16,0), (32,1,56,1,1), (32,1,57,1,2), (32,1,58,38,5), (33,1,0,1,3), (33,1,1,34,5),
    (33,1,35,1,3), (33,1,36,1,1), (33,1,37,22,0), (33,1,59,1,1), (33,1,60,1,3), (33,1,61,34,5),
    (33,1,95,1,3), (34,1,0,1,4), (34,1,1,33,5), (34,1,34,1,2), (34,1,35,26,0), (34,1,61,1,2),
    (34,1,62,33,5), (34,1,95,1,4), (35,1,0,1,4), (35,1,1,31,5), (35,1,32,1,3), (35,1,33,1,1),
    (35,1,34,28,0), (35,1,62,1,1), (35,1,63,1,3), (35,1,64,31,5), (35,1,95,1,4), (36,1,0,1,4),
    (36,1,1,30,5), (36,1,31,1,2), (36,1,32,32,0), (36,1,64,1,2), (36,1,65,1,3), (36,1,66,29,5),
    (36,1,95,1,4), (37,1,0,1,4), (37,1,1,18,5), (37,1,19,3,4), (37,1,22,8,3), (37,1,30,1,2),
    (37,1,31,34,0), (37,1,65,1,2), (37,1,66,9,3), (37,1,75,20,5), (37,1,95,1,4), (38,1,0,1,3),
    (38,1,1,12,5), (38,1,13,5,4), (38,1,18,11,3), (38,1,29,1,2), (38,1,30,36,0), (38,1,66,1,2),
    (38,1,67,11,3), (38,1,78,6,4), (38,1,84,11,5), (38,1,95,1,3), (39,1,0,1,3), (39,1,1,3,5),
    (39,1,4,11,4), (39,1,15,13,3), (39,1,28,1,2), (39,1,29,38,0), (39,1,67,1,2), (39,1,68,12,3),
    (39,1,80,12,4), (39,1,92,3,5), (39,1,95,1,3), (40,1,0,1,3), (40,1,1,11,4), (40,1,12,15,3),
    (40,1,27,1,2), (40,1,28,40,0), (40,1,68,1,2), (40,1,69,15,3), (40,1,84,11,4), (40,1,95,1,3),
    (41,1,0,1,3), (41,1,1,8,4), (41,1,9,17,3), (41,1,26,1,2), (41,1,27,1,1), (41,1,28,40,0),
    (41,1,68,1,1), (41,1,69,1,2), (41,1,70,16,3), (41,1,86,9,4), (41,1,95,1,2), (42,1,0,1,2),
    (42,1,1,4,4), (42,1,5,13,3), (42,1,18,8,2), (42,1,26,1,1), (42,1,27,42,0), (42,1,69,1,1),
    (42,1,70,8,2), (42,1,78,13,3), (42,1,91,4,4), (42,1,95,1,2), (43,1,0,1,2), (43,1,1,2,4),
    (43,1,3,8,3), (43,1,11,15,2), (43,1,26,44,0), (43,1,70,14,2), (43,1,84,9,3), (43,1,93,2,4),
    (43,1,95,1,2), (44,1,0,1,2), (44,1,1,5,3), (44,1,6,19,2), (44,1,25,1,1), (44,1,26,44,0),
    (44,1,70,1,1), (44,1,71,19,2), (44,1,90,5,3), (44,1,95,1,2), (45,1,0,25,2), (45,1,25,46,0),
    (45,1,71,25,2), (46,1,0,24,2), (46,1,24,1,1), (46,1,25,46,0), (46,1,71,1,1), (46,1,72,24,2),
    (47,2,0,24,2), (47,2,24,48,0), (47,2,72,24,2), (49,2,0,23,2), (49,2,23,50,1), (49,2,73,5,2),
    (49,2,78,17,4), (49,2,95,1,2), (51,2,0,23,2), (51,2,23,50,0), (51,2,73,5,2), (51,2,78,17,4),
    (51,2,95,1,2), (53,1,0,22,2), (53,1,22,52,1), (53,1,74,4,2), (53,1,78,17,4), (53,1,95,1,2),
    (54,2,0,12,2), (54,2,12,12,4), (54,2,24,42,1), (54,2,66,29,4), (54,2,95,1,2), (56,2,0,12,2),
    (56,2,12,12,4), (56,2,24,42,0), (56,2,66,29,4), (56,2,95,1,2), (58,2,0,12,2), (58,2,12,12,4),
    (58,2,24,42,1), (58,2,66,29,4), (58,2,95,1,2), (60,1,0,1,2), (60,1,1,71,4), (60,1,72,13,2),
    (60,1,85,10,4), (60,1,95,1,2), (61,3,0,1,2), (61,3,1,71,4), (61,3,72,1,7), (61,3,73,11,10),
    (61,3,84,1,2), (61,3,85,10,4), (61,3,95,1,2), (64,2,0,1,2), (64,2,1,71,4), (64,2,72,1,7),
    (64,2,73,11,10), (64,2,84,1,3), (64,2,85,10,4), (64,2,95,1,2), (66,1,0,1,2), (66,1,1,11,4),
    (66,1,12,1,2), (66,1,13,24,3), (66,1,37,23,4), (66,1,60,1,2), (66,1,61,11,3), (66,1,72,1,7),
    (66,1,73,11,10), (66,1,84,1,2), (66,1,85,10,3), (66,1,95,1,2), (67,1,0,1,2), (67,1,1,11,4),
    (67,1,12,1,7), (67,1,13,23,11), (67,1,36,1,3), (67,1,37,23,4), (67,1,60,1,7), (67,1,61,11,11),
    (67,1,72,1,7), (67,1,73,11,11), (67,1,84,1,7), (67,1,85,10,11), (67,1,95,1,6), (68,3,0,1,2),
    (68,3,1,11,4), (68,3,12,1,7), (68,3,13,23,11), (68,3,36,1,3), (68,3,37,23,4), (68,3,60,1,7),
    (68,3,61,11,11), (68,3,72,1,7), (68,3,73,11,11), (68,3,84,1,7), (68,3,85,10,11), (68,3,95,1,7),
    (71,1,0,1,3), (71,1,1,11,4), (71,1,12,1,9), (71,1,13,23,11), (71,1,36,1,3), (71,1,37,23,4),
    (71,1,60,1,9), (71,1,61,11,11), (71,1,72,1,9), (71,1,73,11,11), (71,1,84,1,9), (71,1,85,10,11),
    (71,1,95,1,7), (72,1,0,1,3), (72,1,1,11,6), (72,1,12,1,9), (72,1,13,23,12), (72,1,36,1,5),
    (72,1,37,23,6), (72,1,60,1,9), (72,1,61,11,12), (72,1,72,1,9), (72,1,73,11,12), (72,1,84,1,9),
    (72,1,85,10,12), (72,1,95,1,7), (73,1,0,1,7), (73,1,1,11,12), (73,1,12,1,9), (73,1,13,23,12),
    (73,1,36,1,9), (73,1,37,23,12), (73,1,60,1,9), (73,1,61,11,12), (73,1,72,1,9), (73,1,73,11,12),
    (73,1,84,1,9), (73,1,85,10,12), (73,1,95,1,7), (74,1,0,1,7), (74,1,1,11,13), (74,1,12,1,9),
    (74,1,13,23,13), (74,1,36,1,9), (74,1,37,23,13), (74,1,60,1,9), (74,1,61,11,13), (74,1,72,1,9),
    (74,1,73,11,13), (74,1,84,1,9), (74,1,85,10,13), (74,1,95,1,7), (75,1,0,1,7), (75,1,1,11,13),
    (75,1,12,1,10), (75,1,13,23,13), (75,1,36,1,10), (75,1,37,23,13), (75,1,60,1,10), (75,1,61,11,13),
    (75,1,72,1,10), (75,1,73,11,13), (75,1,84,1,10), (75,1,85,10,13), (75,1,95,1,7), (76,1,0,1,7),
    (76,1,1,1,13), (76,1,2,2,14), (76,1,4,1,13), (76,1,5,2,14), (76,1,7,1,13), (76,1,8,2,14),
    (76,1,10,1,13), (76,1,11,1,14), (76,1,12,1,10), (76,1,13,1,13), (76,1,14,2,14), (76,1,16,1,13),
    (76,1,17,2,14), (76,1,19,1,13), (76,1,20,2,14), (76,1,22,1,13), (76,1,23,2,14), (76,1,25,1,13),
    (76,1,26,2,14), (76,1,28,1,13), (76,1,29,2,14), (76,1,31,1,13), (76,1,32,2,14), (76,1,34,1,13),
    (76,1,35,1,14), (76,1,36,1,10), (76,1,37,1,13), (76,1,38,2,14), (76,1,40,1,13), (76,1,41,2,14),
    (76,1,43,1,13), (76,1,44,2,14), (76,1,46,1,13), (76,1,47,2,14), (76,1,49,1,13), (76,1,50,2,14),
    (76,1,52,1,13), (76,1,53,2,14), (76,1,55,1,13), (76,1,56,2,14), (76,1,58,1,13), (76,1,59,1,14),
    (76,1,60,1,10), (76,1,61,1,13), (76,1,62,2,14), (76,1,64,1,13), (76,1,65,2,14), (76,1,67,1,13),
    (76,1,68,2,14), (76,1,70,1,13), (76,1,71,1,14), (76,1,72,1,10), (76,1,73,1,13), (76,1,74,2,14),
    (76,1,76,1,13), (76,1,77,2,14), (76,1,79,1,13), (76,1,80,2,14), (76,1,82,1,13), (76,1,83,1,14),
    (76,1,84,1,10), (76,1,85,1,13), (76,1,86,2,14), (76,1,88,1,13), (76,1,89,2,14), (76,1,91,1,13),
    (76,1,92,2,14), (76,1,94,1,13), (76,1,95,1,7), (77,2,0,1,6), (77,2,1,11,14), (77,2,12,1,11),
    (77,2,13,23,14), (77,2,36,1,11), (77,2,37,23,14), (77,2,60,1,11), (77,2,61,11,14), (77,2,72,1,11),
    (77,2,73,11,14), (77,2,84,1,11), (77,2,85,10,14), (77,2,95,1,6), (79,1,0,1,6), (79,1,1,1,13),
    (79,1,2,10,14), (79,1,12,1,11), (79,1,13,23,14), (79,1,36,1,11), (79,1,37,23,14), (79,1,60,1,11),
    (79,1,61,11,14), (79,1,72,1,11), (79,1,73,11,14), (79,1,84,1,11), (79,1,85,9,14), (79,1,94,1,13),
    (79,1,95,1,6), (80,1,1,1,11), (80,1,2,1,14), (80,1,3,1,15), (80,1,4,2,14), (80,1,6,1,15),
    (80,1,7,2,14), (80,1,9,1,15), (80,1,10,2,14), (80,1,12,1,11), (80,1,13,2,14), (80,1,15,1,15),
    (80,1,16,2,14), (80,1,18,1,15), (80,1,19,2,14), (80,1,21,1,15), (80,1,22,2,14), (80,1,24,1,15),
    (80,1,25,2,14), (80,1,27,1,15), (80,1,28,2,14), (80,1,30,1,15), (80,1,31,2,14), (80,1,33,1,15),
    (80,1,34,2,14), (80,1,36,1,11), (80,1,37,2,14), (80,1,39,1,15), (80,1,40,2,14), (80,1,42,1,15),
    (80,1,43,2,14), (80,1,45,1,15), (80,1,46,2,14), (80,1,48,1,15), (80,1,49,2,14), (80,1,51,1,15),
    (80,1,52,2,14), (80,1,54,1,15), (80,1,55,2,14), (80,1,57,1,15), (80,1,58,2,14), (80,1,60,1,11),
    (80,1,61,2,14), (80,1,63,1,15), (80,1,64,2,14), (80,1,66,1,15), (80,1,67,2,14), (80,1,69,1,15),
    (80,1,70,2,14), (80,1,72,1,11), (80,1,73,2,14), (80,1,75,1,15), (80,1,76,2,14), (80,1,78,1,15),
    (80,1,79,2,14), (80,1,81,1,15), (80,1,82,2,14), (80,1,84,1,11), (80,1,85,2,14), (80,1,87,1,15),
    (80,1,88,2,14), (80,1,90,1,15), (80,1,91,2,14), (80,1,93,1,15), (80,1,94,1,11), (81,1,1,1,7),
    (81,1,2,10,15), (81,1,12,1,11), (81,1,13,23,15), (81,1,36,1,11), (81,1,37,23,15), (81,1,60,1,11),
    (81,1,61,11,15), (81,1,72,1,11), (81,1,73,11,15), (81,1,84,1,11), (81,1,85,9,15), (81,1,94,1,7),
    (82,1,1,1,6), (82,1,2,1,13), (82,1,3,9,15), (82,1,12,1,11), (82,1,13,23,15), (82,1,36,1,11),
    (82,1,37,23,15), (82,1,60,1,11), (82,1,61,11,15), (82,1,72,1,11), (82,1,73,11,15), (82,1,84,1,11),
    (82,1,85,8,15), (82,1,93,1,13), (82,1,94,1,6), (83,1,2,1,11), (83,1,3,9,15), (83,1,12,1,11),
    (83,1,13,23,15), (83,1,36,1,11), (83,1,37,23,15), (83,1,60,1,11), (83,1,61,11,15), (83,1,72,1,11),
    (83,1,73,11,15), (83,1,84,1,11), (83,1,85,8,15), (83,1,93,1,11), (84,1,2,1,5), (84,1,3,90,11),
    (84,1,93,1,5), (85,1,3,1,11), (85,1,4,8,15), (85,1,12,1,11), (85,1,13,23,15), (85,1,36,1,11),
    (85,1,37,23,15), (85,1,60,1,11), (85,1,61,11,15), (85,1,72,1,11), (85,1,73,11,15), (85,1,84,1,11),
    (85,1,85,7,15), (85,1,92,1,11), (86,1,3,1,6), (86,1,4,1,14), (86,1,5,7,16), (86,1,12,1,11),
    (86,1,13,23,16), (86,1,36,1,11), (86,1,37,23,16), (86,1,60,1,11), (86,1,61,11,16), (86,1,72,1,11),
    (86,1,73,11,16), (86,1,84,1,11), (86,1,85,6,16), (86,1,91,1,14), (86,1,92,1,6), (87,1,4,1,7),
    (87,1,5,1,15), (87,1,6,6,16), (87,1,12,1,11), (87,1,13,23,16), (87,1,36,1,11), (87,1,37,23,16),
    (87,1,60,1,11), (87,1,61,11,16), (87,1,72,1,11), (87,1,73,11,16), (87,1,84,1,11), (87,1,85,5,16),
    (87,1,90,1,15), (87,1,91,1,7), (88,1,5,1,7), (88,1,6,6,16), (88,1,12,1,11), (88,1,13,23,16),
    (88,1,36,1,11), (88,1,37,23,16), (88,1,60,1,11), (88,1,61,11,16), (88,1,72,1,11), (88,1,73,11,16),
    (88,1,84,1,11), (88,1,85,5,16), (88,1,90,1,7), (89,1,6,1,11), (89,1,7,5,16), (89,1,12,1,11),
    (89,1,13,23,16), (89,1,36,1,11), (89,1,37,23,16), (89,1,60,1,11), (89,1,61,11,16), (89,1,72,1,11),
    (89,1,73,11,16), (89,1,84,1,11), (89,1,85,4,16), (89,1,89,1,11), (90,1,7,1,7), (90,1,8,1,15),
    (90,1,9,3,16), (90,1,12,1,13), (90,1,13,23,16), (90,1,36,1,13), (90,1,37,23,16), (90,1,60,1,13),
    (90,1,61,11,16), (90,1,72,1,13), (90,1,73,11,16), (90,1,84,1,13), (90,1,85,2,16), (90,1,87,1,15),
    (90,1,88,1,7), (91,1,8,1,7), (91,1,9,1,15), (91,1,10,2,16), (91,1,12,1,13), (91,1,13,23,16),
    (91,1,36,1,13), (91,1,37,23,16), (91,1,60,1,13), (91,1,61,11,16), (91,1,72,1,13), (91,1,73,11,16),
    (91,1,84,1,13), (91,1,85,1,16), (91,1,86,1,15), (91,1,87,1,7), (92,1,9,1,6), (92,1,10,1,11),
    (92,1,11,1,16), (92,1,12,1,14), (92,1,13,23,16), (92,1,36,1,14), (92,1,37,23,16), (92,1,60,1,14),
    (92,1,61,11,16), (92,1,72,1,14), (92,1,73,11,16), (92,1,84,2,11), (92,1,86,1,6), (93,1,11,1,6),
    (93,1,12,1,11), (93,1,13,23,16), (93,1,36,1,14), (93,1,37,23,16), (93,1,60,1,14), (93,1,61,11,16),
    (93,1,72,1,14), (93,1,73,10,16), (93,1,83,1,11), (93,1,84,1,6), (94,1,13,1,6), (94,1,14,1,11),
    (94,1,15,1,14), (94,1,16,20,16), (94,1,36,1,14), (94,1,37,23,16), (94,1,60,1,14), (94,1,61,11,16),
    (94,1,72,1,14), (94,1,73,7,16), (94,1,80,1,14), (94,1,81,1,11), (94,1,82,1,6), (95,1,16,2,7),
    (95,1,18,18,11), (95,1,36,1,7), (95,1,37,23,11), (95,1,60,1,7), (95,1,61,11,11), (95,1,72,1,7),
    (95,1,73,5,11), (95,1,78,2,7),
];

fn icon_hit(icon: Icon, u: f32, v: f32) -> bool {
    match icon {
        Icon::HorizonLogo => {
            // Single-colour Horizon sunset mark: a half sun with two gaps over a city skyline.
            let dx = u - 0.50;
            let dy = v - 0.56;
            let sun = dy <= 0.0
                && (dx * dx + dy * dy).sqrt() <= 0.31
                && !(v >= 0.43 && v < 0.47)
                && !(v >= 0.50 && v < 0.54);
            let tower_a = u >= 0.08 && u <= 0.30 && v >= 0.66 && v <= 0.90;
            let tower_b = u >= 0.34 && u <= 0.66 && v >= 0.72 && v <= 0.90;
            let tower_c = u >= 0.70 && u <= 0.92 && v >= 0.60 && v <= 0.90;
            sun || tower_a || tower_b || tower_c
        }
        Icon::ChevronDown => {
            seg_dist(u, v, 0.24, 0.38, 0.50, 0.64) <= 0.075
                || seg_dist(u, v, 0.50, 0.64, 0.76, 0.38) <= 0.075
        }
        Icon::Play => {
            // Rounded right-pointing play triangle.
            let x0 = 0.26;
            let x1 = 0.80;
            if u < x0 - 0.04 || u > x1 + 0.04 {
                return false;
            }
            let t = ((u - x0) / (x1 - x0)).clamp(0.0, 1.0);
            let half_h = (1.0 - t) * 0.31 + 0.025;
            (v - 0.50).abs() <= half_h
        }
        Icon::Mods => {
            // 4-lobe puzzle / clover grid mark matching the screenshot's Mods icon.
            let centers = [(0.33, 0.33), (0.67, 0.33), (0.33, 0.67), (0.67, 0.67)];
            let in_lobe = centers.iter().any(|&(cx, cy)| {
                let dx = (u - cx).abs();
                let dy = (v - cy).abs();
                let r = 0.07;
                let qx = (dx - 0.075).max(0.0);
                let qy = (dy - 0.075).max(0.0);
                (qx * qx + qy * qy).sqrt() <= r
            });
            let center_bridge = (u - 0.50).abs() <= 0.09 && (v - 0.50).abs() <= 0.09;
            let hole = ((u - 0.50).powi(2) + (v - 0.50).powi(2)).sqrt() <= 0.065;
            (in_lobe || center_bridge) && !hole
        }
        Icon::Packs => {
            // Layered isometric diamond/package stack matching the screenshot's Packs icon.
            let top_diamond = ((u - 0.50).abs() * 0.62 + (v - 0.34).abs()).abs() <= 0.22
                && ((u - 0.50).abs() * 0.62 + (v - 0.34).abs()).abs() >= 0.11;
            let top_fill = (u - 0.50).abs() * 0.65 + (v - 0.33).abs() <= 0.19;
            let mid_layer = seg_dist(u, v, 0.18, 0.53, 0.50, 0.70) <= 0.055
                || seg_dist(u, v, 0.50, 0.70, 0.82, 0.53) <= 0.055;
            let bot_layer = seg_dist(u, v, 0.18, 0.69, 0.50, 0.86) <= 0.055
                || seg_dist(u, v, 0.50, 0.86, 0.82, 0.69) <= 0.055;
            top_diamond || top_fill || mid_layer || bot_layer
        }
        Icon::Shaders => {
            // Sunburst / 8-ray sparkle icon matching the screenshot's Shaders icon.
            let (dx, dy) = (u - 0.50, v - 0.50);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= 0.15 {
                return true;
            }
            let dirs: [(f32, f32); 8] = [
                (1.0, 0.0),
                (-1.0, 0.0),
                (0.0, 1.0),
                (0.0, -1.0),
                (0.7071, 0.7071),
                (-0.7071, 0.7071),
                (0.7071, -0.7071),
                (-0.7071, -0.7071),
            ];
            dirs.iter().any(|&(rx, ry)| {
                seg_dist(
                    u,
                    v,
                    0.50 + rx * 0.24,
                    0.50 + ry * 0.24,
                    0.50 + rx * 0.39,
                    0.50 + ry * 0.39,
                ) <= 0.048
            })
        }
        Icon::Installed => {
            // Folder outline/fill matching the screenshot's Installed icon.
            let in_tab = (0.14..=0.45).contains(&u) && (0.20..=0.34).contains(&v);
            let in_body = (0.14..=0.86).contains(&u) && (0.30..=0.80).contains(&v);
            let in_inner = (0.22..=0.78).contains(&u) && (0.40..=0.72).contains(&v);
            (in_tab || in_body) && !in_inner
        }
        Icon::Settings => {
            // Horizontal tuning sliders with circular knobs matching the screenshot's Settings icon.
            let track1 = seg_dist(u, v, 0.16, 0.33, 0.84, 0.33) <= 0.048;
            let track2 = seg_dist(u, v, 0.16, 0.67, 0.84, 0.67) <= 0.048;
            let knob1 = ((u - 0.36).powi(2) + (v - 0.33).powi(2)).sqrt() <= 0.125;
            let knob1_hole = ((u - 0.36).powi(2) + (v - 0.33).powi(2)).sqrt() <= 0.05;
            let knob2 = ((u - 0.64).powi(2) + (v - 0.67).powi(2)).sqrt() <= 0.125;
            let knob2_hole = ((u - 0.64).powi(2) + (v - 0.67).powi(2)).sqrt() <= 0.05;
            ((track1 || knob1) && !knob1_hole) || ((track2 || knob2) && !knob2_hole)
        }
        Icon::Pause => {
            // Two vertical pause bars ||
            seg_dist(u, v, 0.34, 0.24, 0.34, 0.76) <= 0.068
                || seg_dist(u, v, 0.66, 0.24, 0.66, 0.76) <= 0.068
        }
        Icon::Close => {
            // Crisp X cross
            seg_dist(u, v, 0.27, 0.27, 0.73, 0.73) <= 0.062
                || seg_dist(u, v, 0.73, 0.27, 0.27, 0.73) <= 0.062
        }
        Icon::Search => {
            let d = ((u - 0.43).powi(2) + (v - 0.43).powi(2)).sqrt();
            let ring = (0.19..=0.28).contains(&d);
            let handle = seg_dist(u, v, 0.59, 0.59, 0.82, 0.82) <= 0.055;
            ring || handle
        }
        Icon::Check => {
            seg_dist(u, v, 0.22, 0.54, 0.42, 0.74) <= 0.068
                || seg_dist(u, v, 0.42, 0.74, 0.80, 0.28) <= 0.068
        }
        Icon::Monitor => {
            // Desktop monitor outline + stand + base matching Settings -> Video Settings.
            let outer = (0.14..=0.86).contains(&u) && (0.18..=0.66).contains(&v);
            let inner = (0.21..=0.79).contains(&u) && (0.25..=0.59).contains(&v);
            let screen = outer && !inner;
            let neck = (0.46..=0.54).contains(&u) && (0.66..=0.80).contains(&v);
            let base = seg_dist(u, v, 0.33, 0.80, 0.67, 0.80) <= 0.04;
            screen || neck || base
        }
        Icon::Gear => {
            // 8-tooth cogwheel with hollow center matching Settings -> Launcher Settings.
            let (dx, dy) = (u - 0.50, v - 0.50);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= 0.14 || d >= 0.41 {
                return false;
            }
            if (0.14..=0.28).contains(&d) {
                return true;
            }
            let dirs: [(f32, f32); 8] = [
                (1.0, 0.0),
                (-1.0, 0.0),
                (0.0, 1.0),
                (0.0, -1.0),
                (0.7071, 0.7071),
                (-0.7071, 0.7071),
                (0.7071, -0.7071),
                (-0.7071, -0.7071),
            ];
            dirs.iter().any(|&(rx, ry)| {
                let proj = dx * rx + dy * ry;
                let perp = (dx * ry - dy * rx).abs();
                (0.24..=0.39).contains(&proj) && perp <= 0.075
            })
        }
        Icon::Flask => {
            // Erlenmeyer laboratory flask with liquid level matching Settings -> Experimental Stuff.
            let rim = seg_dist(u, v, 0.36, 0.17, 0.64, 0.17) <= 0.038;
            let neck_l = seg_dist(u, v, 0.42, 0.17, 0.42, 0.40) <= 0.036;
            let neck_r = seg_dist(u, v, 0.58, 0.17, 0.58, 0.40) <= 0.036;
            let slope_l = seg_dist(u, v, 0.42, 0.40, 0.18, 0.80) <= 0.038;
            let slope_r = seg_dist(u, v, 0.58, 0.40, 0.82, 0.80) <= 0.038;
            let bottom = seg_dist(u, v, 0.18, 0.82, 0.82, 0.82) <= 0.038;
            let liquid = seg_dist(u, v, 0.30, 0.63, 0.70, 0.63) <= 0.032;
            rim || neck_l || neck_r || slope_l || slope_r || bottom || liquid
        }
        Icon::Terminal => {
            // ">_" terminal prompt matching Settings -> Diagnostics.
            let chev_top = seg_dist(u, v, 0.18, 0.30, 0.44, 0.50) <= 0.045;
            let chev_bot = seg_dist(u, v, 0.44, 0.50, 0.18, 0.70) <= 0.045;
            let underscore = seg_dist(u, v, 0.52, 0.72, 0.82, 0.72) <= 0.045;
            chev_top || chev_bot || underscore
        }
        Icon::MicrosoftGrid => {
            // 2x2 four-pane window grid matching Settings -> Account / Buttons.
            let in_x = (0.18..=0.46).contains(&u) || (0.54..=0.82).contains(&u);
            let in_y = (0.18..=0.46).contains(&v) || (0.54..=0.82).contains(&v);
            in_x && in_y
        }
        Icon::Gamepad => {
            // Controller / D-pad + buttons icon for Settings -> Controls.
            let body = ((u - 0.50).abs() <= 0.32 && (v - 0.52).abs() <= 0.20)
                || ((u - 0.24).powi(2) + (v - 0.58).powi(2)).sqrt() <= 0.15
                || ((u - 0.76).powi(2) + (v - 0.58).powi(2)).sqrt() <= 0.15;
            let dpad_h = (0.24..=0.42).contains(&u) && (0.48..=0.55).contains(&v);
            let dpad_v = (0.295..=0.365).contains(&u) && (0.42..=0.61).contains(&v);
            let btn1 = ((u - 0.68).powi(2) + (v - 0.46).powi(2)).sqrt() <= 0.045;
            let btn2 = ((u - 0.76).powi(2) + (v - 0.55).powi(2)).sqrt() <= 0.045;
            body && !(dpad_h || dpad_v || btn1 || btn2)
        }
        Icon::ArrowLeft => {
            let top = seg_dist(u, v, 0.62, 0.24, 0.34, 0.50) <= 0.055;
            let bot = seg_dist(u, v, 0.34, 0.50, 0.62, 0.76) <= 0.055;
            top || bot
        }
        Icon::MenuEquals => {
            // Sleek floating control-hub icon: two horizontal tracks with circular slider knobs.
            let top_bar = seg_dist(u, v, 0.16, 0.35, 0.84, 0.35) <= 0.058;
            let bot_bar = seg_dist(u, v, 0.16, 0.65, 0.84, 0.65) <= 0.058;
            let top_knob = ((u - 0.64).powi(2) + (v - 0.35).powi(2)).sqrt() <= 0.135;
            let top_hole = ((u - 0.64).powi(2) + (v - 0.35).powi(2)).sqrt() <= 0.052;
            let bot_knob = ((u - 0.36).powi(2) + (v - 0.65).powi(2)).sqrt() <= 0.135;
            let bot_hole = ((u - 0.36).powi(2) + (v - 0.65).powi(2)).sqrt() <= 0.052;
            (top_bar || bot_bar || top_knob || bot_knob) && !top_hole && !bot_hole
        }
        Icon::Undo => {
            let arc = {
                let dx = u - 0.54;
                let dy = v - 0.56;
                let r = (dx * dx + dy * dy).sqrt();
                (0.20..=0.31).contains(&r) && !(dx < -0.05 && dy > 0.08)
            };
            let arrow1 = seg_dist(u, v, 0.22, 0.42, 0.38, 0.26) <= 0.058;
            let arrow2 = seg_dist(u, v, 0.22, 0.42, 0.40, 0.52) <= 0.058;
            arc || arrow1 || arrow2
        }
        Icon::Redo => {
            let mu = 1.0 - u;
            let arc = {
                let dx = mu - 0.54;
                let dy = v - 0.56;
                let r = (dx * dx + dy * dy).sqrt();
                (0.20..=0.31).contains(&r) && !(dx < -0.05 && dy > 0.08)
            };
            let arrow1 = seg_dist(mu, v, 0.22, 0.42, 0.38, 0.26) <= 0.058;
            let arrow2 = seg_dist(mu, v, 0.22, 0.42, 0.40, 0.52) <= 0.058;
            arc || arrow1 || arrow2
        }
        Icon::Plus => {
            let h_bar = seg_dist(u, v, 0.22, 0.50, 0.78, 0.50) <= 0.068;
            let v_bar = seg_dist(u, v, 0.50, 0.22, 0.50, 0.78) <= 0.068;
            h_bar || v_bar
        }
        Icon::Crosshair => {
            let (dx, dy) = (u - 0.50, v - 0.50);
            let d = (dx * dx + dy * dy).sqrt();
            let outer_ring = (0.23..=0.30).contains(&d);
            let center_dot = d <= 0.065;
            let h_tick = (v - 0.50).abs() <= 0.042 && (0.12..=0.37).contains(&u.min(1.0 - u));
            let v_tick = (u - 0.50).abs() <= 0.042 && (0.12..=0.37).contains(&v.min(1.0 - v));
            outer_ring || center_dot || h_tick || v_tick
        }
        Icon::Pencil => {
            let shaft = seg_dist(u, v, 0.30, 0.70, 0.75, 0.25) <= 0.082;
            let tip = seg_dist(u, v, 0.19, 0.81, 0.28, 0.72) <= 0.048;
            let gap = seg_dist(u, v, 0.62, 0.38, 0.65, 0.35) <= 0.095;
            (shaft && !gap) || tip
        }
        Icon::Copy => {
            let front_outer = (0.32..=0.82).contains(&u) && (0.32..=0.84).contains(&v);
            let front_inner = (0.39..=0.75).contains(&u) && (0.39..=0.77).contains(&v);
            let front = front_outer && !front_inner;
            let back_outer = (0.18..=0.68).contains(&u)
                && (0.16..=0.68).contains(&v)
                && !((0.28..=0.86).contains(&u) && (0.28..=0.88).contains(&v));
            let back_inner = (0.25..=0.61).contains(&u) && (0.23..=0.61).contains(&v);
            let back = back_outer && !back_inner;
            front || back
        }
        Icon::ArrowRight => {
            let top = seg_dist(u, v, 0.38, 0.24, 0.66, 0.50) <= 0.055;
            let bot = seg_dist(u, v, 0.66, 0.50, 0.38, 0.76) <= 0.055;
            top || bot
        }
        Icon::Folder => {
            let in_tab = (0.14..=0.46).contains(&u) && (0.20..=0.34).contains(&v);
            let in_body = (0.14..=0.86).contains(&u) && (0.30..=0.80).contains(&v);
            let top_seam = (0.18..=0.82).contains(&u) && (0.35..=0.40).contains(&v);
            (in_tab || in_body) && !top_seam
        }
        Icon::FileText => {
            let outer = (0.22..=0.78).contains(&u) && (0.16..=0.84).contains(&v);
            let inner = (0.29..=0.71).contains(&u) && (0.23..=0.77).contains(&v);
            let line1 = seg_dist(u, v, 0.35, 0.38, 0.65, 0.38) <= 0.035;
            let line2 = seg_dist(u, v, 0.35, 0.52, 0.65, 0.52) <= 0.035;
            let line3 = seg_dist(u, v, 0.35, 0.66, 0.55, 0.66) <= 0.035;
            (outer && !inner) || line1 || line2 || line3
        }
        Icon::Trash => {
            let lid = seg_dist(u, v, 0.22, 0.26, 0.78, 0.26) <= 0.045;
            let handle = (0.40..=0.60).contains(&u) && (0.17..=0.26).contains(&v);
            let body_outer = (0.28..=0.72).contains(&u) && (0.30..=0.84).contains(&v);
            let body_inner = (0.35..=0.65).contains(&u) && (0.36..=0.77).contains(&v);
            let slat1 = seg_dist(u, v, 0.44, 0.42, 0.44, 0.71) <= 0.032;
            let slat2 = seg_dist(u, v, 0.56, 0.42, 0.56, 0.71) <= 0.032;
            lid || handle || (body_outer && !body_inner) || slat1 || slat2
        }
        Icon::Share => {
            let tray_l = seg_dist(u, v, 0.22, 0.56, 0.22, 0.80) <= 0.048;
            let tray_r = seg_dist(u, v, 0.78, 0.56, 0.78, 0.80) <= 0.048;
            let tray_b = seg_dist(u, v, 0.22, 0.80, 0.78, 0.80) <= 0.048;
            let stem = seg_dist(u, v, 0.50, 0.20, 0.50, 0.62) <= 0.048;
            let head_l = seg_dist(u, v, 0.34, 0.36, 0.50, 0.20) <= 0.048;
            let head_r = seg_dist(u, v, 0.66, 0.36, 0.50, 0.20) <= 0.048;
            tray_l || tray_r || tray_b || stem || head_l || head_r
        }
        Icon::Skins => {
            // Crisp player tunic / t-shirt icon for Skins tab & Wardrobe.
            let body = (0.28..=0.72).contains(&u) && (0.22..=0.84).contains(&v);
            let left_sleeve = (0.12..=0.30).contains(&u)
                && (0.22..=0.52).contains(&v)
                && (v - 0.22) >= (0.28 - u) * 0.55;
            let right_sleeve = (0.70..=0.88).contains(&u)
                && (0.22..=0.52).contains(&v)
                && (v - 0.22) >= (u - 0.72) * 0.55;
            let neck_cut = ((u - 0.50).powi(2) + (v - 0.20).powi(2)).sqrt() <= 0.12;
            (body || left_sleeve || right_sleeve) && !neck_cut
        }
    }
}

impl Cache {
    fn region(atlas: &HudAtlas) -> Option<Rect> {
        atlas.sprites.get("horizon/ui-text").copied()
    }
    /// Reserve `w x h` pixels in the top glyph/icon band (`y: 0..320`); never evicts `self.images`
    /// and immediately allocates after wrapping so no icon or glyph is dropped mid-frame.
    fn place(&mut self, atlas: &HudAtlas, w: u32, h: u32) -> Option<(u32, u32, Rect)> {
        let region = Self::region(atlas)?;
        let text_band_h = region.h.min(320);
        if w > region.w || h > text_band_h {
            return None;
        }
        let (mut x, mut y, mut row) = self.shelf;
        if x + w + 1 > region.w {
            x = 0;
            y += row + 1;
            row = 0;
        }
        if y + h > text_band_h {
            self.entries.clear();
            x = 0;
            y = 0;
            row = 0;
        }
        self.shelf = (x + w + 1, y, row.max(h));
        Some((x, y, region))
    }
    /// Reserve `w x h` pixels in the dedicated bottom image band (`y: 320..region.h`) so project
    /// thumbnails and gallery screenshots never overwrite or evict UI text, corners, or vector icons.
    fn place_image(&mut self, atlas: &HudAtlas, w: u32, h: u32) -> Option<(u32, u32, Rect)> {
        let region = Self::region(atlas)?;
        let band_top = 320u32.min(region.h.saturating_sub(64));
        let band_bottom = region.h;
        if w > region.w || band_top + h > band_bottom {
            return None;
        }
        let (mut x, mut y, mut row) = self.image_shelf;
        if y < band_top {
            y = band_top;
        }
        if x + w + 1 > region.w {
            x = 0;
            y += row + 1;
            row = 0;
        }
        if y + h > band_bottom {
            self.images.clear();
            x = 0;
            y = band_top;
            row = 0;
        }
        self.image_shelf = (x + w + 1, y, row.max(h));
        Some((x, y, region))
    }
    fn upload_image_rgba(&mut self, atlas: &HudAtlas, w: u32, h: u32, rgba: Vec<u8>) -> Option<Rect> {
        let (x, y, region) = self.place_image(atlas, w, h)?;
        let (ax, ay) = (
            (region.u0 * atlas.width as f32).round() as u32 + x,
            (region.v0 * atlas.height as f32).round() as u32 + y,
        );
        self.pending.push(crate::font::Patch {
            x: ax,
            y: ay,
            width: w,
            height: h,
            rgba,
        });
        let (aw, ah) = (atlas.width as f32, atlas.height as f32);
        Some(Rect {
            u0: ax as f32 / aw,
            v0: ay as f32 / ah,
            u1: (ax + w) as f32 / aw,
            v1: (ay + h) as f32 / ah,
            w,
            h,
        })
    }
    fn upload(&mut self, atlas: &HudAtlas, w: u32, h: u32, alpha: &[f32]) -> Option<Rect> {
        let rgba: Vec<u8> = alpha
            .iter()
            .flat_map(|a| [255, 255, 255, (a.clamp(0.0, 1.0) * 255.0).round() as u8])
            .collect();
        self.upload_rgba(atlas, w, h, rgba)
    }
    fn upload_rgba(&mut self, atlas: &HudAtlas, w: u32, h: u32, rgba: Vec<u8>) -> Option<Rect> {
        let (x, y, region) = self.place(atlas, w, h)?;
        let (ax, ay) = (
            (region.u0 * atlas.width as f32).round() as u32 + x,
            (region.v0 * atlas.height as f32).round() as u32 + y,
        );
        self.pending.push(crate::font::Patch {
            x: ax,
            y: ay,
            width: w,
            height: h,
            rgba,
        });
        let (aw, ah) = (atlas.width as f32, atlas.height as f32);
        Some(Rect {
            u0: ax as f32 / aw,
            v0: ay as f32 / ah,
            u1: (ax + w) as f32 / aw,
            v1: (ay + h) as f32 / ah,
            w,
            h,
        })
    }
    fn glyph(&mut self, atlas: &HudAtlas, face: Face, id: ab_glyph::GlyphId, px: f32) -> Cached {
        let size = (px * 4.0).round() as u16;
        let key = Key::Glyph(face, id.0, size);
        if let Some(c) = self.entries.get(&key) {
            return *c;
        }
        let scale = self.fonts.scale(face, f32::from(size) / 4.0);
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
        let made = match self.fonts.face(face).outline_glyph(glyph) {
            Some(outline) => {
                let b = outline.px_bounds();
                let (w, h) = (b.width() as u32, b.height() as u32);
                let mut alpha = vec![0.0; (w * h) as usize];
                outline.draw(|x, y, c| {
                    if x < w && y < h {
                        alpha[(y * w + x) as usize] = c;
                    }
                });
                // Light text on dark backgrounds reads thin: a gentle boost.
                for a in &mut alpha {
                    *a = a.powf(0.85);
                }
                Cached {
                    rect: self.upload(atlas, w, h, &alpha),
                    left: b.min.x,
                    top: b.min.y,
                }
            }
            None => Cached {
                rect: None,
                left: 0.0,
                top: 0.0,
            },
        };
        if made.rect.is_some() || made.left == 0.0 {
            self.entries.insert(key, made);
        }
        made
    }
    fn corner(&mut self, atlas: &HudAtlas, r: u16, thickness: u16) -> Option<Rect> {
        // Cap rasterized corner resolution to 56px so large circles (e.g. joysticks)
        // never overflow the 1024x512 UI text atlas shelf.
        let capped_r = r.clamp(1, 56);
        let scaled_t = if thickness == 0 {
            0
        } else {
            ((u32::from(thickness) * u32::from(capped_r))
                .div_ceil(u32::from(r.max(1)))
                .max(1)) as u16
        };
        let key = Key::Corner(capped_r, scaled_t);
        if let Some(c) = self.entries.get(&key) {
            return c.rect;
        }
        // The top-left quadrant of a circle centred at (r, r), 4x4 supersampled.
        let n = u32::from(capped_r);
        let rf = f32::from(capped_r);
        let tf = f32::from(scaled_t);
        let mut alpha = vec![0.0; (n * n) as usize];
        for y in 0..n {
            for x in 0..n {
                let mut hits = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                        let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                        let d = ((px - rf).powi(2) + (py - rf).powi(2)).sqrt();
                        if d <= rf && (scaled_t == 0 || d >= (rf - tf).max(0.0)) {
                            hits += 1;
                        }
                    }
                }
                alpha[(y * n + x) as usize] = hits as f32 / 16.0;
            }
        }
        let rect = self.upload(atlas, n, n, &alpha);
        if rect.is_some() {
            self.entries.insert(
                key,
                Cached {
                    rect,
                    left: 0.0,
                    top: 0.0,
                },
            );
        }
        rect
    }
    fn vector_icon(&mut self, atlas: &HudAtlas, icon: Icon, px: u16) -> Option<Rect> {
        let n = u32::from(px.clamp(8, 48));
        let key = Key::Vector(icon, n as u16);
        if let Some(c) = self.entries.get(&key) {
            return c.rect;
        }
        let nf = n as f32;
        let mut alpha = vec![0.0; (n * n) as usize];
        for y in 0..n {
            for x in 0..n {
                let mut hits = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let u = (x as f32 + (sx as f32 + 0.5) / 4.0) / nf;
                        let v = (y as f32 + (sy as f32 + 0.5) / 4.0) / nf;
                        if icon_hit(icon, u, v) {
                            hits += 1;
                        }
                    }
                }
                alpha[(y * n + x) as usize] = hits as f32 / 16.0;
            }
        }
        let rect = self.upload(atlas, n, n, &alpha);
        if rect.is_some() {
            self.entries.insert(
                key,
                Cached {
                    rect,
                    left: 0.0,
                    top: 0.0,
                },
            );
        }
        rect
    }
    fn image(&mut self, atlas: &HudAtlas, key: &str, w: u32, h: u32, rgba: &[u8]) -> Option<Rect> {
        if let Some(&rect) = self.images.get(key) {
            return Some(rect);
        }
        if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
            return None;
        }
        // Fit project icons (<= 48x48) and wide gallery screenshots (<= 96x54) into the dedicated
        // bottom image band (`y: 320..512`) so they never overflow the atlas shelf.
        let (max_w, max_h) = if w > h * 4 / 3 { (96u32, 54u32) } else { (48u32, 48u32) };
        let (tw, th, mut masked) = if w > max_w || h > max_h {
            let scale = (max_w as f32 / w as f32).min(max_h as f32 / h as f32);
            let nw = ((w as f32 * scale).round() as u32).clamp(1, max_w);
            let nh = ((h as f32 * scale).round() as u32).clamp(1, max_h);
            let mut out = vec![0u8; (nw * nh * 4) as usize];
            for ny in 0..nh {
                let sy = ((ny as u64 * h as u64) / nh as u64).min((h - 1) as u64) as u32;
                for nx in 0..nw {
                    let sx = ((nx as u64 * w as u64) / nw as u64).min((w - 1) as u64) as u32;
                    let src_i = ((sy * w + sx) * 4) as usize;
                    let dst_i = ((ny * nw + nx) * 4) as usize;
                    out[dst_i..dst_i + 4].copy_from_slice(&rgba[src_i..src_i + 4]);
                }
            }
            (nw, nh, out)
        } else {
            (w, h, rgba.to_vec())
        };
        // Apply a soft rounded-corner mask to project icons so they sit cleanly in cards.
        let r = (tw.min(th) as f32 * 0.22).max(2.0);
        let (wf, hf) = (tw as f32, th as f32);
        for y in 0..th {
            for x in 0..tw {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let dx = if px < r {
                    r - px
                } else if px > wf - r {
                    px - (wf - r)
                } else {
                    0.0
                };
                let dy = if py < r {
                    r - py
                } else if py > hf - r {
                    py - (hf - r)
                } else {
                    0.0
                };
                if dx > 0.0 && dy > 0.0 {
                    let d = (dx * dx + dy * dy).sqrt();
                    let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                    let idx = ((y * tw + x) * 4 + 3) as usize;
                    masked[idx] = (masked[idx] as f32 * cov).round() as u8;
                }
            }
        }
        let rect = self.upload_image_rgba(atlas, tw, th, masked)?;
        self.images.insert(key.to_owned(), rect);
        Some(rect)
    }
}

/// One frame of design-system drawing, in dp: the
/// launcher's logical pixels, scaled so the screen reads like the launcher's
/// 1040x700 window at any size.
pub struct Ui<'a> {
    pub atlas: &'a HudAtlas,
    pub batch: &'a mut HudBatch,
    /// The screen in dp.
    pub w: f32,
    pub h: f32,
    /// The cursor in dp.
    pub cursor: (f32, f32),
    /// Physical pixels per dp.
    pub density: f32,
    scale: f32,
    clip: Option<[f32; 4]>,
    /// Optional page/view entrance progress (`0.0..=1.0`) for spatial staggered choreography.
    pub page_anim: f32,
    /// Top-left `(x, y)` in dp of the animated content region.
    pub stagger_origin: [f32; 2],
}
impl<'a> Ui<'a> {
    /// `cursor` is in GUI pixels, like every screen's input.
    pub fn new(atlas: &'a HudAtlas, batch: &'a mut HudBatch, cursor: (f32, f32)) -> Self {
        let scale = batch.scale;
        let (pw, ph) = (batch.width * scale, batch.height * scale);
        let density = (pw / 1040.0).min(ph / 700.0).clamp(1.0, 4.0);
        Self {
            atlas,
            w: pw / density,
            h: ph / density,
            cursor: (cursor.0 * scale / density, cursor.1 * scale / density),
            density,
            scale,
            clip: None,
            page_anim: 1.0,
            stagger_origin: [0.0, 0.0],
            batch,
        }
    }

    /// Configures spatial staggered entrance choreography for elements inside `(origin_x.., origin_y..)`.
    pub fn set_stagger_transition(&mut self, progress: f32, origin_x: f32, origin_y: f32) {
        self.page_anim = if motion::reduced_motion() {
            1.0
        } else {
            progress.clamp(0.0, 1.0)
        };
        self.stagger_origin = [origin_x, origin_y];
    }

    #[inline]
    fn stagger_transform(&self, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
        if self.page_anim >= 0.999
            || (x <= 1.0 && w >= self.w - 2.0 && h >= self.h - 2.0)
            || x < self.stagger_origin[0] - 4.0
            || y < self.stagger_origin[1] - 4.0
        {
            return (0.0, 1.0);
        }
        if motion::reduced_motion() {
            // Tier 2 Accessible Reduced Motion: strip spatial displacement, keep fast opacity fade
            let alpha = (0.35 + 0.65 * motion::spring_expressive_effects(self.page_anim)).clamp(0.0, 1.0);
            return (0.0, alpha);
        }
        let rel_y = (y - self.stagger_origin[1]).max(0.0);
        let rel_x = (x - self.stagger_origin[0]).max(0.0);
        let delay = ((rel_y / 520.0) * 0.34 + (rel_x / 920.0) * 0.10).clamp(0.0, 0.42);
        let local_t = ((self.page_anim - delay) / (1.0 - delay).max(0.25)).clamp(0.0, 1.0);
        let dy = (1.0 - motion::spring_expressive_default(local_t)) * 14.0;
        let alpha = (0.16 + 0.84 * motion::spring_expressive_effects(local_t)).clamp(0.0, 1.0);
        (dy, alpha)
    }
    /// Native phone DPI-adaptive launcher density; game HUD scale is untouched.
    pub fn mobile(atlas: &'a HudAtlas, batch: &'a mut HudBatch, cursor: (f32, f32)) -> Self {
        let mut ui = Self::new(atlas, batch, cursor);
        let pw = ui.batch.width * ui.scale;
        let ph = ui.batch.height * ui.scale;
        let hw_dpi = f32::from_bits(ANDROID_DPI_BITS.load(std::sync::atomic::Ordering::Relaxed));
        let user_scale = (UI_SCALE_PERCENT.load(std::sync::atomic::Ordering::Relaxed) as f32
            / 100.0)
            .clamp(0.75, 1.65);
        let base_density = if ph > pw {
            // Portrait phone screen: target ~410dp width so cards and touch buttons are comfortable.
            let target_dpi = (pw / 410.0).min(ph / 640.0);
            if hw_dpi >= 1.0 {
                (target_dpi * 0.7 + hw_dpi * 0.3).clamp(1.0, 4.5)
            } else {
                target_dpi.clamp(1.0, 4.5)
            }
        } else {
            // Landscape phone/tablet screen: target ~440dp height (and ~880dp width) so the
            // left navigation rail, header, and 2-column cards match phone DPI with breathing room.
            let fit_phone = (pw / 880.0).min(ph / 440.0);
            let dpi = if hw_dpi >= 1.0 {
                (hw_dpi * 0.78).min(ph / 435.0).max(fit_phone)
            } else {
                fit_phone
            };
            dpi.clamp(0.65, 4.5)
        };
        // Ensure landscape viewports always have at least 410-440dp of vertical breathing room
        // and 760-880dp of horizontal width even on high-density (3.0-3.5x) screens.
        let max_safe_density = if ph > pw {
            (pw / 340.0).max(1.0)
        } else {
            (ph / 410.0).min(pw / 760.0).max(1.0)
        };
        ui.density = (base_density * user_scale)
            .min(max_safe_density)
            .clamp(0.5, 4.5);
        ui.w = pw / ui.density;
        ui.h = ph / ui.density;
        ui.cursor = (
            cursor.0 * ui.scale / ui.density,
            cursor.1 * ui.scale / ui.density,
        );
        ui
    }

    pub fn fit_text(&self, face: Face, size: f32, text: &str, width: f32) -> String {
        if text.is_empty() || width <= 0.0 {
            return String::new();
        }
        let px = (size * self.density * 4.0).round() / 4.0;
        let max_px = width * self.density;
        with_cache(self.atlas, |cache| {
            let font = cache.fonts.face(face);
            let scaled = font.as_scaled(cache.fonts.scale(face, px));
            let mut total = 0.0;
            let mut prev = None;
            for ch in text.chars() {
                let id = font.glyph_id(ch);
                if let Some(p) = prev {
                    total += scaled.kern(p, id);
                }
                total += scaled.h_advance(id);
                prev = Some(id);
            }
            if total <= max_px {
                return text.to_owned();
            }
            crate::ui_text::fit(text, max_px, |previous, ch| {
                let id = font.glyph_id(ch);
                previous.map_or(0.0, |p| scaled.kern(font.glyph_id(p), id)) + scaled.h_advance(id)
            })
        })
    }
    /// Move rasterized glyphs into the batch's atlas uploads (real frames only).
    pub fn finish(self) {
        let pending = with_cache(self.atlas, |c| std::mem::take(&mut c.pending));
        self.batch.patches.extend(pending);
    }

    /// Set or clear a scissor clip rectangle `[x, y, w, h]` in dp coordinates.
    pub fn set_clip(&mut self, rect: Option<[f32; 4]>) {
        self.clip = rect.map(|[x, y, w, h]| {
            [
                self.px_x(x),
                self.px_y(y),
                self.px_x(x + w.max(0.0)),
                self.px_y(y + h.max(0.0)),
            ]
        });
    }

    /// Runs `draw` with the clip narrowed to `rect` (dp), restoring the outer clip afterwards.
    pub fn clipped(&mut self, rect: [f32; 4], draw: impl FnOnce(&mut Self)) {
        let previous = self.clip;
        if let Some(narrowed) = self.clip_rect(rect) {
            self.set_clip(Some(narrowed));
        }
        draw(self);
        self.clip = previous;
    }

    pub fn clip_rect(&self, rect: [f32; 4]) -> Option<[f32; 4]> {
        let [x, y, w, h] = rect;
        if let Some([cx0, cy0, cx1, cy1]) = self.clip {
            let d = self.density.max(0.001);
            let x0 = x.max(cx0 / d);
            let y0 = y.max(cy0 / d);
            let x1 = (x + w).min(cx1 / d);
            let y1 = (y + h).min(cy1 / d);
            (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
        } else {
            (w > 0.0 && h > 0.0).then_some(rect)
        }
    }

    // ---- geometry, snapped to physical pixels ----
    fn px_x(&self, v: f32) -> f32 {
        (v * self.density).round()
    }
    fn px_y(&self, v: f32) -> f32 {
        (v * self.density).round()
    }
    fn raw_fill(&mut self, mut x0: f32, mut y0: f32, mut x1: f32, mut y1: f32, color: u32) {
        if let Some([cx0, cy0, cx1, cy1]) = self.clip {
            x0 = x0.max(cx0);
            y0 = y0.max(cy0);
            x1 = x1.min(cx1);
            y1 = y1.min(cy1);
        }
        if x1 > x0 && y1 > y0 {
            let s = self.scale;
            self.batch.fill(
                self.atlas,
                x0 / s,
                y0 / s,
                (x1 - x0) / s,
                (y1 - y0) / s,
                color,
            );
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn emit_clipped_uv(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: u32,
    ) {
        let w = x1 - x0;
        let h = y1 - y0;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (mut nx0, mut ny0, mut nx1, mut ny1) = (x0, y0, x1, y1);
        let (mut nu0, mut nv0, mut nu1, mut nv1) = (u0, v0, u1, v1);
        if let Some([cx0, cy0, cx1, cy1]) = self.clip {
            nx0 = x0.max(cx0);
            ny0 = y0.max(cy0);
            nx1 = x1.min(cx1);
            ny1 = y1.min(cy1);
            if nx1 <= nx0 || ny1 <= ny0 {
                return;
            }
            let du = u1 - u0;
            let dv = v1 - v0;
            nu0 = u0 + du * ((nx0 - x0) / w);
            nu1 = u0 + du * ((nx1 - x0) / w);
            nv0 = v0 + dv * ((ny0 - y0) / h);
            nv1 = v0 + dv * ((ny1 - y0) / h);
        }
        let s = self.scale;
        let (gx0, gy0, gx1, gy1) = (nx0 / s, ny0 / s, nx1 / s, ny1 / s);
        self.batch.quad(
            [[gx0, gy0], [gx1, gy0], [gx1, gy1], [gx0, gy1]],
            [[nu0, nv0], [nu1, nv0], [nu1, nv1], [nu0, nv1]],
            color,
        );
    }
    fn corner_quad(&mut self, rect: Rect, x: f32, y: f32, r: f32, flip: (bool, bool), color: u32) {
        let (u0, u1) = if flip.0 {
            (rect.u1, rect.u0)
        } else {
            (rect.u0, rect.u1)
        };
        let (v0, v1) = if flip.1 {
            (rect.v1, rect.v0)
        } else {
            (rect.v0, rect.v1)
        };
        self.emit_clipped_uv(x, y, x + r, y + r, u0, v0, u1, v1, color);
    }
    fn corners(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, thickness: u16, color: u32) {
        if r < 1.0 {
            return;
        }
        let atlas = self.atlas;
        if let Some(rect) = with_cache(atlas, |c| c.corner(atlas, r as u16, thickness)) {
            self.corner_quad(rect, x0, y0, r, (false, false), color);
            self.corner_quad(rect, x1 - r, y0, r, (true, false), color);
            self.corner_quad(rect, x1 - r, y1 - r, r, (true, true), color);
            self.corner_quad(rect, x0, y1 - r, r, (false, true), color);
        }
    }
    fn raw_rounded_px(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
        if color >> 24 == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let (x0, y0, x1, y1) = (
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + w),
            self.px_y(y + h),
        );
        let r = (radius * self.density)
            .round()
            .min(((x1 - x0) / 2.0).floor())
            .min(((y1 - y0) / 2.0).floor())
            .max(0.0);
        if r < 1.0 {
            self.raw_fill(x0, y0, x1, y1, color);
            return;
        }
        self.raw_fill(x0 + r, y0, x1 - r, y1, color);
        self.raw_fill(x0, y0 + r, x0 + r, y1 - r, color);
        self.raw_fill(x1 - r, y0 + r, x1, y1 - r, color);
        self.corners(x0, y0, x1, y1, r, 0, color);
    }

    /// A filled rectangle with rounded corners (pure, artifact-free geometry).
    pub fn rounded(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
        let (dy, alpha) = self.stagger_transform(x, y, w, h);
        let y = y + dy;
        let color = if alpha < 0.999 {
            color::with_alpha(color, ((color >> 24) as f32 / 255.0 * alpha).clamp(0.0, 1.0))
        } else {
            color
        };
        self.raw_rounded_px(x, y, w, h, radius, color);
    }
    /// A 1px rounded border.
    pub fn border(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
        if color >> 24 == 0 {
            return;
        }
        let (dy, alpha) = self.stagger_transform(x, y, w, h);
        let y = y + dy;
        let color = if alpha < 0.999 {
            color::with_alpha(color, ((color >> 24) as f32 / 255.0 * alpha).clamp(0.0, 1.0))
        } else {
            color
        };
        let (x0, y0, x1, y1) = (
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + w),
            self.px_y(y + h),
        );
        let r = (radius * self.density)
            .round()
            .min(((x1 - x0) / 2.0).floor())
            .min(((y1 - y0) / 2.0).floor())
            .max(0.0);
        self.raw_fill(x0 + r, y0, x1 - r, y0 + 1.0, color);
        self.raw_fill(x0 + r, y1 - 1.0, x1 - r, y1, color);
        self.raw_fill(x0, y0 + r, x0 + 1.0, y1 - r, color);
        self.raw_fill(x1 - 1.0, y0 + r, x1, y1 - r, color);
        self.corners(x0, y0, x1, y1, r, 1, color);
    }

    /// Draw a supersampled vector icon in `size x size` dp.
    pub fn icon(&mut self, icon: Icon, x: f32, y: f32, size: f32, color: u32) {
        if color >> 24 == 0 || size <= 0.0 {
            return;
        }
        let (dy, alpha) = self.stagger_transform(x, y, size, size);
        let y = y + dy;
        let color = if alpha < 0.999 {
            color::with_alpha(color, ((color >> 24) as f32 / 255.0 * alpha).clamp(0.0, 1.0))
        } else {
            color
        };
        let (x0, y0) = (self.px_x(x), self.px_y(y));
        let px = (size * self.density).round().clamp(8.0, 128.0) as u16;
        let atlas = self.atlas;
        if let Some(rect) = with_cache(atlas, |c| c.vector_icon(atlas, icon, px)) {
            let pxf = f32::from(px);
            self.emit_clipped_uv(
                x0,
                y0,
                x0 + pxf,
                y0 + pxf,
                rect.u0,
                rect.v0,
                rect.u1,
                rect.v1,
                color,
            );
        }
    }

    /// Draw the colour Horizon sunset logo at `(x, y)` in `size x size` dp.
    pub fn horizon_logo(&mut self, x: f32, y: f32, size: f32) {
        self.draw_horizon_logo_art(x, y, size, 1.0, 1.0);
    }

    /// Draws the pixel-art Horizon sunset logo. `reveal` (0..1) uncovers it from the
    /// bottom row up; `alpha` fades the whole mark.
    pub fn draw_horizon_logo_art(&mut self, x: f32, y: f32, size: f32, alpha: f32, reveal: f32) {
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.01 || size <= 1.0 {
            return;
        }
        let n = HORIZON_LOGO_GRID;
        let cell = size / n as f32;
        let first_row = ((1.0 - reveal.clamp(0.0, 1.0)) * n as f32) as u32;
        for &(r0, rc, c0, cc, pal) in HORIZON_LOGO_RUNS.iter() {
            let row_end = r0 as u32 + rc as u32;
            if row_end <= first_row {
                continue;
            }
            let row_start = (r0 as u32).max(first_row);
            let col_end = c0 as u32 + cc as u32;
            let x0 = x + c0 as f32 * cell;
            let x1 = x + col_end as f32 * cell;
            let y0 = y + row_start as f32 * cell;
            let y1 = y + row_end as f32 * cell;
            let rgb = HORIZON_LOGO_PALETTE[pal as usize];
            self.fill(x0, y0, x1 - x0, y1 - y0, color::with_alpha(color::hex(rgb, 255), a));
        }
    }

    /// The account's real head as a flat 8x8 skin face (no 3D build), so the
    /// header paints it in the same frame: prepared skin, cached skin, Steve.
    pub fn avatar_head_2d(&mut self, x: f32, y: f32, size: f32, player_name: &str) {
        let theme = active_theme();
        let radius = size * 0.30;
        self.rounded(x, y, size, size, radius, theme.surface_dim);
        let inset = (size * 0.07).max(1.0);
        let face = (size - inset * 2.0).max(8.0);
        let pw = face / 8.0;
        if let Some(skin) = crate::skin_head::head(player_name) {
            for row in 0..8 {
                for col in 0..8 {
                    let px = x + inset + col as f32 * pw;
                    let py = y + inset + row as f32 * pw;
                    self.fill(
                        px,
                        py,
                        pw + 0.4,
                        pw + 0.4,
                        color::hex(skin.front[row][col], 255),
                    );
                    let hat = skin.hat[row][col];
                    if hat >> 24 != 0 {
                        self.fill(
                            px,
                            py,
                            pw + 0.4,
                            pw + 0.4,
                            color::hex(hat & 0xFFFFFF, (hat >> 24) as u8),
                        );
                    }
                }
            }
        } else {
            const HAIR: u32 = 0x3B2A1B;
            const SKIN: u32 = 0xC68E5E;
            const EYE: u32 = 0xF4F4F4;
            const IRIS: u32 = 0x4E6E9E;
            const NOSE: u32 = 0xA87548;
            const MOUTH: u32 = 0x6B4A32;
            const BEARD: u32 = 0x2E2117;
            const FACE: [[u32; 8]; 8] = [
                [HAIR, HAIR, HAIR, HAIR, HAIR, HAIR, HAIR, HAIR],
                [HAIR, HAIR, HAIR, HAIR, HAIR, HAIR, HAIR, HAIR],
                [HAIR, HAIR, HAIR, HAIR, HAIR, HAIR, HAIR, HAIR],
                [HAIR, SKIN, SKIN, SKIN, SKIN, SKIN, SKIN, HAIR],
                [SKIN, EYE, IRIS, SKIN, SKIN, IRIS, EYE, SKIN],
                [SKIN, SKIN, SKIN, NOSE, NOSE, SKIN, SKIN, SKIN],
                [SKIN, MOUTH, MOUTH, MOUTH, MOUTH, MOUTH, MOUTH, SKIN],
                [SKIN, BEARD, BEARD, BEARD, BEARD, BEARD, BEARD, SKIN],
            ];
            for (row, pixels) in FACE.iter().enumerate() {
                for (col, tone) in pixels.iter().enumerate() {
                    self.fill(
                        x + inset + col as f32 * pw,
                        y + inset + row as f32 * pw,
                        pw + 0.4,
                        pw + 0.4,
                        color::hex(*tone, 255),
                    );
                }
            }
        }
        self.border(x, y, size, size, radius, theme.border_strong);
    }

    /// Draw an authentic Material You 3 Switch (`52x28dp` pill track with sliding thumb and checkmark).
    pub fn m3_switch(&mut self, x: f32, y: f32, on: bool) {
        let theme = active_theme();
        let track_w = 52.0;
        let track_h = 28.0;
        let track_r = 9.0;
        if on {
            self.rounded(x, y, track_w, track_h, track_r, theme.accent);
            let thumb_d = 20.0;
            let tx = x + track_w - thumb_d - 4.0;
            let ty = y + (track_h - thumb_d) * 0.5;
            self.rounded(tx, ty, thumb_d, thumb_d, 6.5, theme.on_accent);
            self.icon(Icon::Check, tx + 3.0, ty + 3.0, 14.0, theme.accent);
        } else {
            self.rounded(x, y, track_w, track_h, track_r, theme.surface_dim);
            self.stroke_rounded(x, y, track_w, track_h, track_r, 1.5, theme.border_strong);
            let thumb_d = 16.0;
            let tx = x + 6.0;
            let ty = y + (track_h - thumb_d) * 0.5;
            self.rounded(tx, ty, thumb_d, thumb_d, 5.5, theme.text_muted);
        }
    }

    /// Draw an authentic Material You 3 Checkbox with label (`[✓] Label`) and return total width.
    pub fn m3_checkbox(&mut self, x: f32, y: f32, size: f32, checked: bool, label: &str) -> f32 {
        let theme = active_theme();
        let r = (size * 0.28).clamp(4.0, 7.0);
        if checked {
            self.rounded(x, y, size, size, r, theme.accent);
            let icon_s = (size - 4.0).max(10.0);
            self.icon(
                Icon::Check,
                x + (size - icon_s) * 0.5,
                y + (size - icon_s) * 0.5,
                icon_s,
                theme.on_accent,
            );
        } else {
            self.rounded(x, y, size, size, r, theme.surface_dim);
            self.stroke_rounded(x, y, size, size, r, 1.6, theme.border_strong);
        }
        if !label.is_empty() {
            let tw = self.text(
                Face::Medium,
                12.5,
                x + size + 8.0,
                y + (size - 18.0) * 0.5,
                if checked {
                    theme.text_primary
                } else {
                    theme.text_muted
                },
                label,
            );
            size + 8.0 + tw
        } else {
            size
        }
    }

    /// Material 2026 (M3 Expressive) Animated Switch (`m3_switch_animated`):
    /// Smoothly interpolates thumb position, thumb diameter (`16dp -> 20dp`), corner radius morphing,
    /// track color, and checkmark icon reveal using `progress` (`0.0 = off`, `1.0 = on`).
    pub fn m3_switch_animated(&mut self, x: f32, y: f32, progress: f32) {
        let theme = active_theme();
        let t = progress.clamp(0.0, 1.0);
        let spring_t = motion::spring_expressive_spatial(t);
        let eff_t = motion::spring_expressive_effects(t);
        let track_w = 52.0;
        let track_h = 28.0;
        let track_r = motion::morph_radius(9.0, 8.0, 6.0, eff_t, 0.0);
        let track_col = lerp_color(theme.surface_dim, theme.accent, eff_t);
        self.rounded(x, y, track_w, track_h, track_r, track_col);
        if eff_t < 0.96 {
            self.stroke_rounded(
                x,
                y,
                track_w,
                track_h,
                track_r,
                1.5 * (1.0 - eff_t * 0.7),
                lerp_color(theme.border_strong, theme.accent, eff_t),
            );
        }
        let thumb_d = (16.0 + 4.0 * spring_t).clamp(14.0, 22.0);
        let tx = x + 6.0 + (track_w - thumb_d - 10.0) * spring_t;
        let ty = y + (track_h - thumb_d) * 0.5;
        let thumb_r = motion::morph_radius(5.5, 6.5, 4.5, eff_t, 0.0);
        let thumb_col = lerp_color(theme.text_muted, theme.on_accent, eff_t);
        self.rounded(tx, ty, thumb_d, thumb_d, thumb_r, thumb_col);
        if eff_t > 0.35 {
            let ic_s = (14.0 * ((eff_t - 0.35) / 0.65).clamp(0.4, 1.0)).max(8.0);
            self.icon(
                Icon::Check,
                tx + (thumb_d - ic_s) * 0.5,
                ty + (thumb_d - ic_s) * 0.5,
                ic_s,
                color::with_alpha(theme.accent, ((eff_t - 0.35) / 0.65).clamp(0.0, 1.0)),
            );
        }
    }

    /// Material 2026 (M3 Expressive) Split Button:
    /// Combines a primary leading action button with a connected trailing selector/menu segment,
    /// using asymmetric architectural corner radii (`10dp` outer corners, `4dp` inner split seam).
    /// Returns `(leading_rect, trailing_rect)`.
    #[allow(clippy::too_many_arguments)]
    pub fn m3_split_button(
        &mut self,
        x: f32,
        y: f32,
        total_w: f32,
        h: f32,
        lead_icon: Option<Icon>,
        lead_label: &str,
        trail_label: &str,
        accent: bool,
    ) -> ([f32; 4], [f32; 4]) {
        let theme = active_theme();
        let gap = 3.0;
        let trail_w = (Self::measure_or_estimate(self, Face::SemiBold, 12.5, trail_label) + 34.0)
            .clamp(84.0, (total_w * 0.40).max(88.0));
        let lead_w = (total_w - trail_w - gap).max(96.0);
        let lead_r = [x, y, lead_w, h];
        let trail_r = [x + lead_w + gap, y, trail_w, h];

        let bg_lead = if accent {
            theme.accent
        } else {
            theme.surface_bright
        };
        let fg_lead = if accent {
            theme.on_accent
        } else {
            theme.text_primary
        };

        // Leading primary segment: outer-left 10dp, inner-right 4dp
        self.rounded(lead_r[0], lead_r[1], lead_r[2], lead_r[3], 9.0, bg_lead);
        self.rounded(
            lead_r[0] + lead_r[2] - 10.0,
            lead_r[1],
            10.0,
            lead_r[3],
            4.0,
            bg_lead,
        );
        if !accent {
            self.border(lead_r[0], lead_r[1], lead_r[2], lead_r[3], 9.0, theme.border);
        }

        let font = if h <= 36.0 { 13.5 } else { 14.5 };
        if let Some(ic) = lead_icon {
            let ic_s = (h * 0.40).clamp(15.0, 19.0);
            let fitted = self.fit_text(Face::SemiBold, font, lead_label, (lead_w - ic_s - 28.0).max(32.0));
            let tw = self.measure(Face::SemiBold, font, &fitted);
            let group_w = ic_s + 8.0 + tw;
            let gx = lead_r[0] + ((lead_w - group_w) * 0.5).max(10.0);
            self.icon(ic, gx, lead_r[1] + (h - ic_s) * 0.5, ic_s, fg_lead);
            self.text(
                Face::SemiBold,
                font,
                gx + ic_s + 8.0,
                lead_r[1] + ((h - font * 1.45) * 0.5).max(2.0),
                fg_lead,
                &fitted,
            );
        } else {
            self.centered_fit(
                Face::SemiBold,
                font,
                [lead_r[0] + 10.0, lead_r[1], lead_w - 20.0, h],
                fg_lead,
                lead_label,
            );
        }

        // Trailing menu/profile segment: inner-left 4dp, outer-right 9dp
        let bg_trail = if accent {
            theme.surface_selected
        } else {
            theme.surface
        };
        self.rounded(trail_r[0], trail_r[1], trail_r[2], trail_r[3], 9.0, bg_trail);
        self.rounded(trail_r[0], trail_r[1], 10.0, trail_r[3], 4.0, bg_trail);
        self.border(
            trail_r[0],
            trail_r[1],
            trail_r[2],
            trail_r[3],
            8.0,
            if accent { theme.accent } else { theme.border },
        );
        let clean_trail = trail_label.trim_end_matches(" v").trim();
        let fit_tr = self.fit_text(Face::SemiBold, 12.5, clean_trail, (trail_w - 28.0).max(24.0));
        self.text(
            Face::SemiBold,
            12.5,
            trail_r[0] + 10.0,
            trail_r[1] + ((h - 12.5 * 1.45) * 0.5).max(2.0),
            if accent { theme.accent } else { theme.text_primary },
            &fit_tr,
        );
        self.icon(
            Icon::ChevronDown,
            trail_r[0] + trail_w - 20.0,
            trail_r[1] + (h - 12.0) * 0.5,
            12.0,
            if accent { theme.accent } else { theme.text_muted },
        );

        (lead_r, trail_r)
    }

    #[inline]
    fn measure_or_estimate(ui: &Self, face: Face, size: f32, text: &str) -> f32 {
        ui.measure(face, size, text)
    }

    /// Material 2026 (M3 Expressive) Connected Button Group Segment:
    /// Renders segment `index` of `count` inside a connected button group with shape-morphing
    /// inner/outer corners and a checkmark icon on the active segment.
    #[allow(clippy::too_many_arguments)]
    pub fn m3_connected_segment(
        &mut self,
        rect: [f32; 4],
        index: usize,
        count: usize,
        label: &str,
        active: bool,
        icon: Option<Icon>,
    ) {
        let theme = active_theme();
        let [x, y, w, h] = rect;
        let outer_r = (h * 0.24).clamp(7.0, 9.0);
        let inner_r = if active { 6.0 } else { 4.0 };
        let bg = if active {
            theme.accent
        } else {
            theme.surface_bright
        };
        let fg = if active {
            theme.on_accent
        } else {
            theme.text_primary
        };
        self.rounded(x, y, w, h, inner_r, bg);
        if index == 0 {
            self.rounded(x, y, (w * 0.45).max(outer_r * 2.0), h, outer_r, bg);
        }
        if index + 1 == count {
            let cap_w = (w * 0.45).max(outer_r * 2.0);
            self.rounded(x + w - cap_w, y, cap_w, h, outer_r, bg);
        }
        self.border(
            x,
            y,
            w,
            h,
            if index == 0 || index + 1 == count {
                outer_r
            } else {
                inner_r
            },
            if active { theme.accent } else { theme.border },
        );

        let lead_ic = if active {
            Some(icon.unwrap_or(Icon::Check))
        } else {
            icon
        };
        let font = if h <= 30.0 { 11.8 } else { 12.8 };
        if let Some(ic) = lead_ic {
            let ic_s = (h * 0.40).clamp(11.0, 14.0);
            let avail = (w - ic_s - 18.0).max(16.0);
            let fitted = self.fit_text(
                if active { Face::SemiBold } else { Face::Medium },
                font,
                label,
                avail,
            );
            let tw = self.measure(
                if active { Face::SemiBold } else { Face::Medium },
                font,
                &fitted,
            );
            let tot = ic_s + 5.0 + tw;
            let sx = x + ((w - tot) * 0.5).max(6.0);
            self.icon(ic, sx, y + (h - ic_s) * 0.5, ic_s, fg);
            self.text(
                if active { Face::SemiBold } else { Face::Medium },
                font,
                sx + ic_s + 5.0,
                y + ((h - font * 1.45) * 0.5).max(1.5),
                fg,
                &fitted,
            );
        } else {
            self.centered_fit(
                if active { Face::SemiBold } else { Face::Medium },
                font,
                [x + 6.0, y, (w - 12.0).max(8.0), h],
                fg,
                label,
            );
        }
    }

    /// Material 2026 (M3 Expressive) Filter / Assist Chip:
    /// Morphs corner radius between unselected (`8dp`) and selected (`6dp`) states, with optional
    /// leading icon / selection checkmark and active tonal elevation.
    pub fn m3_filter_chip(
        &mut self,
        rect: [f32; 4],
        label: &str,
        selected: bool,
        icon: Option<Icon>,
    ) {
        let theme = active_theme();
        let [x, y, w, h] = rect;
        let rad = if selected { 6.0 } else { 8.0 };
        let bg = if selected {
            theme.surface_selected
        } else {
            theme.surface_dim
        };
        let border_col = if selected {
            theme.accent
        } else {
            theme.border
        };
        self.rounded(x, y, w, h, rad, bg);
        self.border(x, y, w, h, rad, border_col);

        let show_ic = if selected {
            Some(icon.unwrap_or(Icon::Check))
        } else {
            icon
        };
        let font = if h <= 26.0 { 10.5 } else { 11.5 };
        let fg = if selected {
            theme.accent
        } else {
            theme.text_primary
        };
        if let Some(ic) = show_ic {
            let ic_s = (h * 0.42).clamp(10.0, 13.0);
            let avail = (w - ic_s - 15.0).max(14.0);
            let fitted = self.fit_text(Face::SemiBold, font, label, avail);
            let tw = self.measure(Face::SemiBold, font, &fitted);
            let tot = ic_s + 4.5 + tw;
            let sx = x + ((w - tot) * 0.5).max(5.0);
            self.icon(ic, sx, y + (h - ic_s) * 0.5, ic_s, theme.accent);
            self.text(
                Face::SemiBold,
                font,
                sx + ic_s + 4.5,
                y + ((h - font * 1.45) * 0.5).max(1.0),
                if selected { theme.text_primary } else { fg },
                &fitted,
            );
        } else {
            self.centered_fit(
                Face::SemiBold,
                font,
                [x + 5.0, y, (w - 10.0).max(8.0), h],
                fg,
                label,
            );
        }
    }

    /// Material 2026 (M3 Expressive) Split-Track Slider with Vertical Handle Bar & End-Stop Dot:
    /// Draws a split active/inactive track separated by a `4dp` gap around a vertical bar handle (`4x14dp`).
    pub fn m3_expressive_slider(
        &mut self,
        bar_x: f32,
        bar_y: f32,
        bar_w: f32,
        bar_h: f32,
        fraction: f32,
        dragging: bool,
    ) {
        let theme = active_theme();
        let t = fraction.clamp(0.0, 1.0);
        let handle_w = if dragging { 5.0 } else { 3.5 };
        let handle_h = (bar_h + if dragging { 8.0 } else { 6.0 }).max(12.0);
        let handle_x = (bar_x + (bar_w - handle_w) * t).clamp(bar_x, bar_x + bar_w - handle_w);
        let gap = 3.5;

        // Active left track segment
        let active_w = (handle_x - bar_x - gap).max(0.0);
        if active_w > 1.5 {
            self.rounded(
                bar_x,
                bar_y,
                active_w,
                bar_h,
                bar_h * 0.5,
                theme.accent,
            );
        }

        // Inactive right track segment + M3 terminal stop dot
        let rem_x = handle_x + handle_w + gap;
        let rem_w = (bar_x + bar_w - rem_x).max(0.0);
        if rem_w > 1.5 {
            self.rounded(
                rem_x,
                bar_y,
                rem_w,
                bar_h,
                bar_h * 0.5,
                theme.surface_bright,
            );
            if rem_w > 8.0 {
                let dot_d = (bar_h * 0.55).clamp(3.0, 4.5);
                self.rounded(
                    bar_x + bar_w - dot_d - 1.5,
                    bar_y + (bar_h - dot_d) * 0.5,
                    dot_d,
                    dot_d,
                    dot_d * 0.5,
                    theme.accent,
                );
            }
        }

        // Vertical handle bar with subtle halo when actively dragged
        if dragging {
            self.rounded(
                handle_x - 3.0,
                bar_y + (bar_h - handle_h) * 0.5 - 2.0,
                handle_w + 6.0,
                handle_h + 4.0,
                3.5,
                color::with_alpha(theme.accent, 0.28),
            );
        }
        self.rounded(
            handle_x,
            bar_y + (bar_h - handle_h) * 0.5,
            handle_w,
            handle_h,
            handle_w * 0.45,
            if dragging { theme.accent } else { theme.text_primary },
        );
    }

    /// Material 2026 (M3 Expressive) Wavy Linear Progress Indicator:
    /// Renders an animated sinusoidal wave along the active progress segment, a `4dp` stop gap,
    /// the remaining track segment, and a terminal stop dot. Respects `motion::reduced_motion()`.
    #[allow(clippy::too_many_arguments)]
    pub fn m3_wavy_progress(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fraction: f32,
        phase: f32,
        paused: bool,
    ) {
        let theme = active_theme();
        let clamped = fraction.clamp(0.0, 1.0);
        let fill_w = (w * clamped).clamp(if clamped > 0.0 { h } else { 0.0 }, w);
        let gap_w = if fill_w > 0.0 && fill_w < w - 8.0 {
            4.5
        } else {
            0.0
        };
        let rem_x = x + fill_w + gap_w;
        let rem_w = (w - fill_w - gap_w).max(0.0);

        if rem_w > 2.0 {
            self.rounded(rem_x, y, rem_w, h, h * 0.45, theme.surface_dim);
            self.border(rem_x, y, rem_w, h, h * 0.45, theme.border);
            if rem_w > 12.0 {
                let dot_d = (h * 0.44).clamp(4.0, 6.0);
                self.rounded(
                    x + w - dot_d - 3.0,
                    y + (h - dot_d) * 0.5,
                    dot_d,
                    dot_d,
                    dot_d * 0.5,
                    theme.accent,
                );
            }
        }

        if fill_w > 0.0 {
            self.rounded(x, y, fill_w, h, h * 0.45, theme.accent);
            if !paused && !motion::reduced_motion() && fill_w > 20.0 {
                // Material 2026 sinusoidal wave crest overlay along active progress bar
                let wave_t = phase * std::f32::consts::TAU;
                let seg_w = 10.0f32;
                let count = ((fill_w - 8.0) / seg_w).floor().clamp(1.0, 48.0) as usize;
                let wave_amp = (h * 0.22).clamp(1.2, 3.0);
                for i in 0..count {
                    let sx = x + 4.0 + i as f32 * seg_w;
                    let theta = wave_t + i as f32 * 0.65;
                    let wy = y + (h - 3.0) * 0.5 + theta.sin() * wave_amp;
                    self.rounded(
                        sx,
                        wy,
                        (seg_w - 1.5).max(4.0),
                        3.0,
                        1.5,
                        color::with_alpha(theme.on_accent, 0.30),
                    );
                }
                // Expressive leading-edge vertical indicator bar
                self.rounded(
                    (x + fill_w - 5.0).max(x),
                    y + 2.0,
                    3.5,
                    (h - 4.0).max(2.0),
                    1.75,
                    theme.on_accent,
                );
            }
        }
    }

    /// Material 2026 (M3 Expressive) Morphing Loading Indicator (`m3_wavy_circular_spinner`):
    /// Renders an orbiting 6-node expressive harmonic ring that morphs between a rounded polygon
    /// and a star-flower constellation at `(cx, cy)` with `radius` dp.
    pub fn m3_wavy_circular_spinner(&mut self, cx: f32, cy: f32, radius: f32, phase: f32) {
        let theme = active_theme();
        let t = if motion::reduced_motion() {
            0.25
        } else {
            phase * std::f32::consts::TAU
        };
        let nodes = 6usize;
        for i in 0..nodes {
            let base_angle = t + (i as f32 / nodes as f32) * std::f32::consts::TAU;
            let wave = (t * 2.0 + i as f32 * 1.047).sin();
            let r = radius * (0.78 + 0.22 * wave);
            let dot_d = (radius * (0.28 + 0.08 * wave)).clamp(3.0, 7.5);
            let px = cx + base_angle.cos() * r - dot_d * 0.5;
            let py = cy + base_angle.sin() * r - dot_d * 0.5;
            let alpha = (0.45 + 0.55 * ((i as f32 / nodes as f32 + phase) % 1.0)).clamp(0.3, 1.0);
            self.rounded(
                px,
                py,
                dot_d,
                dot_d,
                dot_d * 0.42,
                color::with_alpha(theme.accent, alpha),
            );
        }
    }

    /// Material 2026 (M3 Expressive) Floating Snackbar / Status Banner:
    /// Renders an elevated tonal floating status bar with a leading status icon and spring entrance.
    pub fn m3_snackbar_toast(
        &mut self,
        x: f32,
        y: f32,
        max_w: f32,
        text: &str,
        is_error: bool,
        anim_t: f32,
    ) {
        if text.trim().is_empty() || max_w <= 40.0 {
            return;
        }
        let theme = active_theme();
        let t = anim_t.clamp(0.0, 1.0);
        let spring = motion::spring_expressive_default(t);
        let alpha = motion::spring_expressive_effects(t);
        let slide = if motion::reduced_motion() {
            0.0
        } else {
            (1.0 - spring) * 10.0
        };
        let h = 30.0;
        let fitted = self.fit_text(Face::Medium, 12.0, text, (max_w - 42.0).max(48.0));
        let tw = self.measure(Face::Medium, 12.0, &fitted);
        let bar_w = (tw + 40.0).clamp(110.0, max_w);
        let by = y + slide;

        let accent_c = if is_error { theme.danger } else { theme.accent };
        self.rounded(
            x,
            by,
            bar_w,
            h,
            8.0,
            color::with_alpha(theme.surface_bright, 0.96 * alpha),
        );
        self.border(
            x,
            by,
            bar_w,
            h,
            8.0,
            color::with_alpha(accent_c, 0.78 * alpha),
        );
        self.rounded(
            x + 4.0,
            by + 6.0,
            3.0,
            h - 12.0,
            1.5,
            color::with_alpha(accent_c, alpha),
        );
        self.icon(
            if is_error { Icon::Close } else { Icon::Check },
            x + 12.0,
            by + 8.0,
            14.0,
            color::with_alpha(accent_c, alpha),
        );
        self.text(
            Face::Medium,
            12.0,
            x + 30.0,
            by + 6.0,
            color::with_alpha(theme.text_primary, alpha),
            &fitted,
        );
    }

    /// Draw a cached RGBA project icon (`w x h`) into `(x, y, dw, dh)` dp.
    #[allow(clippy::too_many_arguments)]
    pub fn image(
        &mut self,
        key: &str,
        w: u32,
        h: u32,
        rgba: &[u8],
        x: f32,
        y: f32,
        dw: f32,
        dh: f32,
    ) -> bool {
        let atlas = self.atlas;
        let Some(rect) = with_cache(atlas, |c| c.image(atlas, key, w, h, rgba)) else {
            return false;
        };
        let (x0, y0, x1, y1) = (
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + dw),
            self.px_y(y + dh),
        );
        self.emit_clipped_uv(x0, y0, x1, y1, rect.u0, rect.v0, rect.u1, rect.v1, u32::MAX);
        true
    }

    /// Draw a single line of text elided to fit within `max_w` dp, inside a `(size * 1.45).round()`
    /// tall line box whose top is `y`; returns the rendered advance in dp.
    pub fn text_clipped(
        &mut self,
        face: Face,
        size: f32,
        x: f32,
        y: f32,
        max_w: f32,
        color: Color,
        text: &str,
    ) -> f32 {
        if max_w <= 2.0 || text.is_empty() {
            return 0.0;
        }
        let single_line: std::borrow::Cow<'_, str> = if text.contains(['\n', '\r']) {
            std::borrow::Cow::Owned(
                text.chars()
                    .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                    .collect(),
            )
        } else {
            std::borrow::Cow::Borrowed(text)
        };
        let fitted = self.fit_text(face, size, &single_line, max_w);
        self.text(face, size, x, y, color, &fitted)
    }

    /// Renders a single-line helper/hint string at `(x, y)` within `w` dp, auto-scaling
    /// font size from `10.5` down to `9.0` if needed so it clips cleanly on a single line.
    pub fn hint_line(&mut self, x: f32, y: f32, w: f32, text: &str, color: Color) {
        if w <= 4.0 || text.is_empty() {
            return;
        }
        let single_line: String = text
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        let mut size = 10.5f32;
        while size > 9.0 && self.measure(Face::Regular, size, &single_line) > w {
            size = (size - 0.5).max(9.0);
        }
        self.text_clipped(Face::Regular, size, x, y, w, color, &single_line);
    }

    /// Material 3 filled/outlined text field: supports compact single-line mode (`label.is_empty()` or `h < 40.0`)
    /// and top-labeled mode (`!label.is_empty() && h >= 40.0`), single-line elided hints, and automatic
    /// tail-scrolling when `active` so newly typed characters and the blinking caret stay visible.
    #[allow(clippy::too_many_arguments)]
    pub fn text_field(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        value: &str,
        hint: &str,
        active: bool,
    ) -> [f32; 4] {
        let theme = active_theme();
        let accent = theme.accent;
        let radius = (h * 0.18).clamp(6.0, 9.0);
        self.rounded(
            x,
            y,
            w,
            h,
            radius,
            if active {
                theme.surface_bright
            } else {
                theme.surface_dim
            },
        );
        self.stroke_rounded(
            x,
            y,
            w,
            h,
            radius,
            if active { 1.6 } else { 1.0 },
            if active { accent } else { theme.border },
        );
        let pad = (w * 0.035).clamp(10.0, 14.0);
        let compact = label.is_empty() || h < 40.0;
        let size = if compact {
            (h * 0.36).clamp(11.5, 14.0)
        } else {
            (h * 0.28).clamp(12.0, 14.5)
        };
        let box_h = (size * 1.45).round();
        let text_y = if compact {
            y + (h - box_h) * 0.5
        } else {
            let label_max_w = (w - pad * 2.0).max(8.0);
            self.text_clipped(
                Face::SemiBold,
                8.5,
                x + pad,
                y + 5.0,
                label_max_w,
                if active { accent } else { theme.text_muted },
                label,
            );
            y + 18.0 + ((h - 20.0) - box_h).max(0.0) * 0.5
        };
        let text_max_w = (w - pad * 2.0 - if active { 6.0 } else { 0.0 }).max(8.0);
        let clean_value: String = value
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        let tw = if clean_value.is_empty() {
            let _ = self.text_clipped(
                Face::Regular,
                size,
                x + pad,
                text_y,
                text_max_w,
                color::with_alpha(theme.text_muted, 0.72),
                hint,
            );
            0.0
        } else if active {
            let limit = (text_max_w - 4.0).max(4.0);
            let mut start_byte = 0;
            if self.measure(Face::Medium, size, &clean_value) > limit {
                for (idx, _) in clean_value.char_indices() {
                    start_byte = idx;
                    if self.measure(Face::Medium, size, &clean_value[start_byte..]) <= limit {
                        break;
                    }
                }
            }
            let suffix = &clean_value[start_byte..];
            self.text(
                Face::Medium,
                size,
                x + pad,
                text_y,
                theme.text_primary,
                suffix,
            )
        } else {
            self.text_clipped(
                Face::Medium,
                size,
                x + pad,
                text_y,
                text_max_w,
                theme.text_primary,
                &clean_value,
            )
        };
        if active {
            let blink_on = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_millis() / 500).is_multiple_of(2))
                .unwrap_or(true);
            let caret_col = if blink_on {
                accent
            } else {
                color::with_alpha(accent, 0.32)
            };
            let cx = (x + pad + tw + 1.5).min(x + w - pad);
            let ch = (size * 1.15).round();
            let cy = text_y + (box_h - ch) * 0.5;
            self.rounded(cx, cy, 1.8, ch, 0.9, caret_col);
        }
        [x, y, w, h]
    }

    // ---- text ----
    /// The advance of `text` in dp.
    pub fn measure(&self, face: Face, size: f32, text: &str) -> f32 {
        let px = (size * self.density * 4.0).round() / 4.0;
        with_cache(self.atlas, |c| {
            let font = c.fonts.face(face);
            let scaled = font.as_scaled(c.fonts.scale(face, px));
            let mut w = 0.0;
            let mut prev = None;
            for ch in text.chars() {
                let id = font.glyph_id(ch);
                if let Some(p) = prev {
                    w += scaled.kern(p, id);
                }
                w += scaled.h_advance(id);
                prev = Some(id);
            }
            w / self.density
        })
    }
    /// Draw a line of text in a `line`-tall box whose top is `y` (the
    /// launcher's line heights: about 1.45 × size for the interface);
    /// returns its width in dp.
    pub(crate) fn text_line(
        &mut self,
        face: Face,
        size: f32,
        line: f32,
        x: f32,
        y: f32,
        color: u32,
        text: &str,
    ) -> f32 {
        let (dy, alpha) = self.stagger_transform(x, y, 60.0, line);
        let y = y + dy;
        let color = if alpha < 0.999 {
            color::with_alpha(color, ((color >> 24) as f32 / 255.0 * alpha).clamp(0.0, 1.0))
        } else {
            color
        };
        let px = (size * self.density * 4.0).round() / 4.0;
        let real = color >> 24 != 0;
        let (x0, y0) = (self.px_x(x), self.px_y(y));
        let box_px = line * self.density;
        let atlas = self.atlas;
        let density = self.density;
        if text.is_empty() {
            return 0.0;
        }
        let mut quads = with_cache(atlas, |c| std::mem::take(&mut c.glyph_scratch));
        quads.clear();
        let width = with_cache(atlas, |c| {
            let scale = c.fonts.scale(face, px);
            let (ascent, descent) = {
                let f = c.fonts.face(face).as_scaled(scale);
                (f.ascent(), f.descent())
            };
            let baseline = y0 + ((box_px - (ascent - descent)) / 2.0 + ascent).round();
            let mut pen = x0;
            let mut prev = None;
            for ch in text.chars() {
                let (id, advance, kern) = {
                    let font = c.fonts.face(face);
                    let id = font.glyph_id(ch);
                    let f = font.as_scaled(scale);
                    (id, f.h_advance(id), prev.map_or(0.0, |p| f.kern(p, id)))
                };
                pen += kern;
                if real && !ch.is_whitespace() {
                    let g = c.glyph(atlas, face, id, px);
                    if let Some(rect) = g.rect {
                        quads.push(((pen + g.left).round(), baseline + g.top, rect));
                    }
                }
                pen += advance;
                prev = Some(id);
            }
            (pen - x0) / density
        });
        for &(gx, gy, rect) in &quads {
            self.emit_clipped_uv(
                gx,
                gy,
                gx + rect.w as f32,
                gy + rect.h as f32,
                rect.u0,
                rect.v0,
                rect.u1,
                rect.v1,
                color,
            );
        }
        quads.clear();
        with_cache(atlas, |c| c.glyph_scratch = quads);
        width
    }
    /// Text in a `1.45 × size` line box whose top is `y`.
    pub fn text(&mut self, face: Face, size: f32, x: f32, y: f32, color: u32, text: &str) -> f32 {
        self.text_line(face, size, (size * 1.45).round(), x, y, color, text)
    }
    pub fn text_centered(
        &mut self,
        face: Face,
        size: f32,
        cx: f32,
        y: f32,
        color: u32,
        text: &str,
    ) {
        let w = self.measure(face, size, text);
        self.text(face, size, cx - w / 2.0, y, color, text);
    }

    /// Filled axis-aligned rectangle in dp.
    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32) {
        if color >> 24 == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let (dy, alpha) = self.stagger_transform(x, y, w, h);
        let y = y + dy;
        let color = if alpha < 0.999 {
            color::with_alpha(color, ((color >> 24) as f32 / 255.0 * alpha).clamp(0.0, 1.0))
        } else {
            color
        };
        self.raw_fill(
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + w),
            self.px_y(y + h),
            color,
        );
    }

    /// Multi-pixel rounded border in dp (`stroke_w` dp thick) rendered in a single pass.
    pub fn stroke_rounded(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        stroke_w: f32,
        color: u32,
    ) {
        if color >> 24 == 0 || stroke_w <= 0.0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let (dy, alpha) = self.stagger_transform(x, y, w, h);
        let y = y + dy;
        let color = if alpha < 0.999 {
            color::with_alpha(color, ((color >> 24) as f32 / 255.0 * alpha).clamp(0.0, 1.0))
        } else {
            color
        };
        let (x0, y0, x1, y1) = (
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + w),
            self.px_y(y + h),
        );
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let t = (stroke_w * self.density).round().clamp(1.0, 24.0);
        let r = (radius * self.density)
            .round()
            .min(((x1 - x0) / 2.0).floor())
            .min(((y1 - y0) / 2.0).floor())
            .max(0.0);
        if r < 1.0 {
            self.raw_fill(x0, y0, x1, (y0 + t).min(y1), color);
            self.raw_fill(x0, (y1 - t).max(y0), x1, y1, color);
            self.raw_fill(
                x0,
                (y0 + t).min(y1),
                (x0 + t).min(x1),
                (y1 - t).max(y0),
                color,
            );
            self.raw_fill(
                (x1 - t).max(x0),
                (y0 + t).min(y1),
                x1,
                (y1 - t).max(y0),
                color,
            );
            return;
        }
        self.raw_fill(x0 + r, y0, x1 - r, (y0 + t).min(y1), color);
        self.raw_fill(x0 + r, (y1 - t).max(y0), x1 - r, y1, color);
        self.raw_fill(x0, y0 + r, (x0 + t).min(x1), y1 - r, color);
        self.raw_fill((x1 - t).max(x0), y0 + r, x1, y1 - r, color);
        if t < r {
            let atlas = self.atlas;
            if let Some(rect) = with_cache(atlas, |c| c.corner(atlas, r as u16, t as u16)) {
                self.corner_quad(rect, x0, y0, r, (false, false), color);
                self.corner_quad(rect, x1 - r, y0, r, (true, false), color);
                self.corner_quad(rect, x1 - r, y1 - r, r, (true, true), color);
                self.corner_quad(rect, x0, y1 - r, r, (false, true), color);
            }
        } else {
            self.corners(x0, y0, x1, y1, r, 0, color);
        }
    }

    /// Centered fitted text inside `[x, y, w, h]` dp.
    pub fn centered_fit(&mut self, face: Face, size: f32, rect: [f32; 4], color: u32, text: &str) {
        let [x, y, w, h] = rect;
        if w <= 2.0 || h <= 2.0 || text.is_empty() {
            return;
        }
        let fitted = self.fit_text(face, size, text, (w - 4.0).max(4.0));
        let tw = self.measure(face, size, &fitted);
        let line_h = (size * 1.45).round();
        self.text(
            face,
            size,
            x + (w - tw) * 0.5,
            y + (h - line_h) * 0.5,
            color,
            &fitted,
        );
    }

    /// Emits a convex polygon in dp coordinates with a glassmorphic top specular edge and crisp vector perimeter border.
    fn draw_convex_polygon_dp(
        &mut self,
        pts: &[[f32; 2]],
        center: [f32; 2],
        fill_color: u32,
        stroke_size: f32,
        stroke_margin: f32,
        stroke_color: u32,
    ) {
        let Some(white) = self.atlas.sprites.get("horizon/white").copied() else {
            return;
        };
        let uv = [[(white.u0 + white.u1) * 0.5, (white.v0 + white.v1) * 0.5]; 4];
        let to_gui = |p: [f32; 2]| -> [f32; 2] {
            [
                (p[0] * self.density) / self.scale,
                (p[1] * self.density) / self.scale,
            ]
        };
        let n = pts.len();
        let c_gui = to_gui(center);

        if (fill_color >> 24) > 0 {
            for i in 0..n {
                let p0 = pts[i];
                let p1 = pts[(i + 1) % n];
                self.batch
                    .quad([c_gui, to_gui(p0), to_gui(p1), c_gui], uv, fill_color);
            }
        }

        let offset_pt = |pt: [f32; 2], off: f32| -> [f32; 2] {
            let dx = pt[0] - center[0];
            let dy = pt[1] - center[1];
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            [pt[0] + dx / len * off, pt[1] + dy / len * off]
        };

        if stroke_size > 0.1 && (stroke_color >> 24) > 0 {
            let t = stroke_size.clamp(1.0, 6.0);
            for i in 0..n {
                let a0 = offset_pt(pts[i], stroke_margin);
                let a1 = offset_pt(pts[(i + 1) % n], stroke_margin);
                let b0 = offset_pt(pts[i], stroke_margin + t);
                let b1 = offset_pt(pts[(i + 1) % n], stroke_margin + t);
                self.batch.quad(
                    [to_gui(a0), to_gui(b0), to_gui(b1), to_gui(a1)],
                    uv,
                    stroke_color,
                );
            }
        }
    }

    /// Draws a GoyControl shape (0=RoundedRect, 1=Square, 2=Circle/Pill, 3=CutOctagon, 4=Diamond, 5=Hexagon, 6=Crest/Triangle)
    /// with true vector polygon geometry, glassmorphic specular highlights, and customizable stroke.
    #[allow(clippy::too_many_arguments)]
    pub fn goy_box(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        shape: u8,
        roundness: f32,
        fill_color: u32,
        stroke_size: f32,
        stroke_margin: f32,
        stroke_roundness: f32,
        stroke_color: u32,
        shadow_size: f32,
        shadow_color: u32,
    ) {
        let _ = (shadow_size, shadow_color);
        let center = [x + w * 0.5, y + h * 0.5];
        match shape {
            3 => {
                // Cut-Corner Tactical Octagon (true 8-vertex vector geometry)
                let cut = (w.min(h) * 0.25).max(3.0);
                let pts = [
                    [x + cut, y],
                    [x + w - cut, y],
                    [x + w, y + cut],
                    [x + w, y + h - cut],
                    [x + w - cut, y + h],
                    [x + cut, y + h],
                    [x, y + h - cut],
                    [x, y + cut],
                ];
                self.draw_convex_polygon_dp(
                    &pts,
                    center,
                    fill_color,
                    stroke_size,
                    stroke_margin,
                    stroke_color,
                );
                return;
            }
            4 => {
                // Chamfered-Tip Vector Diamond / Rhombus
                let cw = (w * 0.08).max(2.0);
                let ch = (h * 0.08).max(2.0);
                let pts = [
                    [center[0] - cw, y + ch],
                    [center[0] + cw, y + ch],
                    [x + w - cw, center[1] - ch],
                    [x + w - cw, center[1] + ch],
                    [center[0] + cw, y + h - ch],
                    [center[0] - cw, y + h - ch],
                    [x + cw, center[1] + ch],
                    [x + cw, center[1] - ch],
                ];
                self.draw_convex_polygon_dp(
                    &pts,
                    center,
                    fill_color,
                    stroke_size,
                    stroke_margin,
                    stroke_color,
                );
                return;
            }
            5 => {
                // True 6-Vertex Vector Hexagon
                let inset = (w * 0.23).max(3.0);
                let pts = [
                    [x + inset, y],
                    [x + w - inset, y],
                    [x + w, center[1]],
                    [x + w - inset, y + h],
                    [x + inset, y + h],
                    [x, center[1]],
                ];
                self.draw_convex_polygon_dp(
                    &pts,
                    center,
                    fill_color,
                    stroke_size,
                    stroke_margin,
                    stroke_color,
                );
                return;
            }
            6 => {
                // Sleek Tactical Apex / Crest Triangle
                let top_w = (w * 0.14).max(3.0);
                let bot_cut = (h * 0.14).max(2.5);
                let pts = [
                    [center[0] - top_w * 0.5, y],
                    [center[0] + top_w * 0.5, y],
                    [x + w, y + h - bot_cut],
                    [x + w * 0.90, y + h],
                    [x + w * 0.10, y + h],
                    [x, y + h - bot_cut],
                ];
                self.draw_convex_polygon_dp(
                    &pts,
                    center,
                    fill_color,
                    stroke_size,
                    stroke_margin,
                    stroke_color,
                );
                return;
            }
            _ => {}
        }

        let eff_radius = match shape {
            1 => 0.0,
            2 => w.min(h) * 0.5,
            _ => roundness.clamp(0.0, w.min(h) * 0.5),
        };
        let eff_stroke_r = match shape {
            1 => 0.0,
            2 => (w + stroke_margin * 2.0).min(h + stroke_margin * 2.0) * 0.5,
            _ => stroke_roundness.clamp(
                0.0,
                (w + stroke_margin * 2.0).min(h + stroke_margin * 2.0) * 0.5,
            ),
        };
        if shape == 1 || eff_radius <= 0.25 {
            self.fill(x, y, w, h, fill_color);
        } else {
            self.rounded(x, y, w, h, eff_radius, fill_color);
        }
        if stroke_size > 0.1 && (stroke_color >> 24) > 0 {
            let sx = x - stroke_margin;
            let sy = y - stroke_margin;
            let sw = (w + stroke_margin * 2.0).max(2.0);
            let sh = (h + stroke_margin * 2.0).max(2.0);
            let t = stroke_size.clamp(1.0, 6.0);
            if shape == 1 || eff_stroke_r <= 0.25 {
                self.fill(sx, sy, sw, t, stroke_color);
                self.fill(sx, sy + sh - t, sw, t, stroke_color);
                self.fill(sx, sy, t, sh, stroke_color);
                self.fill(sx + sw - t, sy, t, sh, stroke_color);
            } else if stroke_size <= 2.2 {
                self.border(sx, sy, sw, sh, eff_stroke_r, stroke_color);
            } else {
                self.stroke_rounded(sx, sy, sw, sh, eff_stroke_r, stroke_size, stroke_color);
            }
        }
    }

    /// Renders a multi-stop linear gradient inside `[x, y, w, h]` dp using the resolved theme configuration.
    pub fn fill_gradient(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        direction: GradientDirection,
        c0: u32,
        c1: u32,
        c2: Option<(f32, u32)>,
        steps: usize,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let bands = steps.clamp(8, 32);
        let sample = |t: f32| -> u32 {
            let t = t.clamp(0.0, 1.0);
            if let Some((mid_pos, cm)) = c2 {
                let mp = mid_pos.clamp(0.10, 0.90);
                if t <= mp {
                    lerp_color(c0, cm, (t / mp).clamp(0.0, 1.0))
                } else {
                    lerp_color(cm, c1, ((t - mp) / (1.0 - mp)).clamp(0.0, 1.0))
                }
            } else {
                lerp_color(c0, c1, t)
            }
        };
        match direction {
            GradientDirection::Vertical => {
                let bh = h / bands as f32;
                for i in 0..bands {
                    let t = (i as f32 + 0.5) / bands as f32;
                    self.fill(x, y + i as f32 * bh, w, bh + 0.6, sample(t));
                }
            }
            GradientDirection::Horizontal => {
                let bw = w / bands as f32;
                for i in 0..bands {
                    let t = (i as f32 + 0.5) / bands as f32;
                    self.fill(x + i as f32 * bw, y, bw + 0.6, h, sample(t));
                }
            }
            GradientDirection::DiagonalDown | GradientDirection::DiagonalUp => {
                // Render smooth 2D grid cells for diagonal gradients without heavy shader switches
                let gx = (bands / 2).clamp(6, 14);
                let gy = (bands / 2).clamp(6, 14);
                let cw = w / gx as f32;
                let ch = h / gy as f32;
                for iy in 0..gy {
                    let ty = (iy as f32 + 0.5) / gy as f32;
                    for ix in 0..gx {
                        let tx = (ix as f32 + 0.5) / gx as f32;
                        let t = if direction == GradientDirection::DiagonalDown {
                            (tx + ty) * 0.5
                        } else {
                            (tx + (1.0 - ty)) * 0.5
                        };
                        self.fill(
                            x + ix as f32 * cw,
                            y + iy as f32 * ch,
                            cw + 0.6,
                            ch + 0.6,
                            sample(t),
                        );
                    }
                }
            }
        }
    }

    /// Paints the active Jetpack Compose Material You (Material 3 Expressive 2026) theme background
    /// with a smooth 60fps animating dynamic tonal mesh & ambient glow orbs over `[x, y, w, h]` dp.
    pub fn draw_theme_background(&mut self, x: f32, y: f32, w: f32, h: f32, theme: &ResolvedTheme) {
        // The picture backgrounds replace the Material You tonal mesh entirely.
        if theme.is_horizon {
            self.draw_horizon_sunset(x, y, w, h);
            return;
        }
        if theme.is_nightly {
            self.draw_nightly_sky(x, y, w, h);
            return;
        }
        if theme.is_custom {
            self.draw_custom_background(x, y, w, h);
            return;
        }
        let t = motion::ambient_time();
        let wave_a = (t * 0.72).sin() * 0.5 + 0.5;
        let wave_b = (t * 0.48 + 1.35).cos() * 0.5 + 0.5;
        let wave_c = (t * 0.36 + 2.40).sin() * 0.5 + 0.5;
        let shift = (t * 0.55).sin() * 0.09;

        if theme.is_gradient {
            let start_c = lerp_color(theme.gradient_start, theme.accent, 0.22 * wave_a);
            let end_c = lerp_color(theme.gradient_end, theme.gradient_mid, 0.26 * wave_b);
            let mid_c = lerp_color(theme.gradient_mid, theme.accent, 0.16 * (1.0 - wave_a));
            let mid_pos = (theme.gradient_mid_pos + shift).clamp(0.20, 0.80);
            let mid = theme.use_three_stops.then_some((mid_pos, mid_c));
            self.fill_gradient(
                x,
                y,
                w,
                h,
                theme.gradient_direction,
                start_c,
                end_c,
                mid,
                22,
            );
        } else {
            let top_tint = lerp_color(
                theme.bg_solid,
                lerp_color(theme.gradient_start, theme.accent, 0.28 * wave_a),
                0.68,
            );
            let mid_tint = lerp_color(
                theme.bg_solid,
                theme.gradient_mid,
                0.42 + 0.18 * wave_c,
            );
            let bot_tint = lerp_color(
                theme.bg_solid,
                lerp_color(theme.gradient_end, theme.accent, 0.22 * wave_b),
                0.64,
            );
            let mid_pos = (0.48 + shift).clamp(0.25, 0.75);
            self.fill_gradient(
                x,
                y,
                w,
                h,
                GradientDirection::Vertical,
                top_tint,
                bot_tint,
                Some((mid_pos, mid_tint)),
                20,
            );
        }

        // Jetpack Compose Material You Expressive animated tonal aurora ribbons & ambient glow orbs
        let ribbon_alpha_1 = if theme.is_light { 0.14 } else { 0.16 };
        let ribbon_alpha_2 = if theme.is_light { 0.09 } else { 0.11 };
        let band1_h = (h * 0.34).max(56.0);
        let band1_y = y + (h - band1_h) * (0.14 + 0.18 * wave_a);
        self.fill_gradient(
            x,
            band1_y,
            w,
            band1_h,
            GradientDirection::Horizontal,
            color::with_alpha(theme.accent, ribbon_alpha_1 * (0.65 + 0.35 * wave_b)),
            color::with_alpha(theme.gradient_mid, ribbon_alpha_2),
            Some((0.46 + shift * 0.85, color::with_alpha(theme.accent, ribbon_alpha_1 * 0.85))),
            10,
        );

        let band2_h = (h * 0.28).max(46.0);
        let band2_y = y + (h - band2_h) * (0.74 - 0.16 * wave_b);
        self.fill_gradient(
            x,
            band2_y,
            w,
            band2_h,
            GradientDirection::Horizontal,
            color::with_alpha(theme.gradient_start, ribbon_alpha_2),
            color::with_alpha(theme.accent, ribbon_alpha_1 * (0.60 + 0.40 * wave_a)),
            None,
            8,
        );

        // Drifting Material You tonal ambient orbs (top-right primary glow + bottom-left secondary glow)
        let orb1_w = (w * 0.38).clamp(160.0, 340.0);
        let orb1_h = (h * 0.42).clamp(120.0, 240.0);
        let orb1_x = x + (w - orb1_w) * (0.82 - 0.14 * wave_a);
        let orb1_y = y + (h - orb1_h) * (0.08 + 0.12 * wave_b);
        self.rounded(
            orb1_x,
            orb1_y,
            orb1_w,
            orb1_h,
            orb1_h * 0.48,
            color::with_alpha(theme.accent, if theme.is_light { 0.08 } else { 0.095 }),
        );

        let orb2_w = (w * 0.34).clamp(140.0, 300.0);
        let orb2_h = (h * 0.36).clamp(100.0, 210.0);
        let orb2_x = x + (w - orb2_w) * (0.16 + 0.14 * wave_c);
        let orb2_y = y + (h - orb2_h) * (0.78 - 0.12 * wave_a);
        self.rounded(
            orb2_x,
            orb2_y,
            orb2_w,
            orb2_h,
            orb2_h * 0.48,
            color::with_alpha(theme.surface_selected, if theme.is_light { 0.22 } else { 0.28 }),
        );
    }

    /// One pixel-art cell block of the Horizon sunset, in grid units.
    fn horizon_px(&mut self, o: (f32, f32, f32), g: (i32, i32, i32, i32), color: u32) {
        let (x0, y0, cell) = o;
        let (gx, gy, gw, gh) = g;
        if gw <= 0 || gh <= 0 {
            return;
        }
        // Opaque blocks overlap by a hair so rounding never leaves seams.
        let pad = if color >> 24 == 255 { 0.6 } else { 0.0 };
        self.fill(
            x0 + gx as f32 * cell,
            y0 + gy as f32 * cell,
            gw as f32 * cell + pad,
            gh as f32 * cell + pad,
            color,
        );
    }

    /// Animated pixel-art sunset landscape ("Horizon" background) over `[x, y, w, h]` dp:
    /// banded sky, twinkling stars, a softly bobbing sun, drifting clouds and three
    /// parallax mountain ranges with pines. Frozen when reduced motion is on.
    pub fn draw_horizon_sunset(&mut self, x: f32, y: f32, w: f32, h: f32) {
        use std::sync::OnceLock;
        use std::time::Instant;
        static START: OnceLock<Instant> = OnceLock::new();
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let t = if motion::reduced_motion() || motion::launcher_animations_disabled() {
            0.0
        } else {
            START.get_or_init(Instant::now).elapsed().as_secs_f32()
        };

        const ROWS: i32 = 90;
        let cell = h / ROWS as f32;
        let cols = (w / cell).ceil() as i32 + 1;
        let horizon = (ROWS as f32 * 0.68) as i32;
        let o = (x, y, cell);

        // Sky: 14 quantized bands, deep purple -> magenta -> orange at the horizon.
        const SKY: [u32; 4] = [0x1A0B2E, 0x4A2370, 0xC2418A, 0xFF9A4D];
        const BANDS: i32 = 14;
        for band in 0..BANDS {
            let s = band as f32 / (BANDS - 1) as f32 * 3.0;
            let i = (s.floor() as usize).min(2);
            let rgb = blend_rgb(SKY[i], SKY[i + 1], s - i as f32);
            let y0 = band * horizon / BANDS;
            let y1 = (band + 1) * horizon / BANDS;
            self.horizon_px(o, (0, y0, cols, y1 - y0), color::hex(rgb, 255));
        }

        // Stars: fixed pseudo-random spots, each twinkling at its own phase.
        for i in 0..40u32 {
            let hsh = i.wrapping_mul(2_654_435_761).rotate_left(13) ^ i.wrapping_mul(40_503);
            let gx = (hsh % cols.max(1) as u32) as i32;
            let gy = ((hsh >> 11) % (ROWS as u32 * 45 / 100)) as i32;
            let phase = (hsh >> 20) as f32 * 0.0061;
            let a = 0.5 + 0.5 * (t * 1.5 + phase).sin();
            self.horizon_px(o, (gx, gy, 1, 1), color::hex(0xFFFFFF, (a * 255.0) as u8));
        }

        // Sun: glow rings, then the pixel disc; the mountains below hide its lower half.
        let r = (ROWS as f32 * 0.17) as i32;
        let cx = cols / 2;
        let cy = horizon - r / 4 + ((t * 0.4).sin() * 2.0) as i32;
        for (mult, alpha) in [(1.9_f32, 35u8), (1.5, 55)] {
            let gr = (r as f32 * mult) as i32;
            for dy in -gr..=0 {
                let hw = ((gr * gr - dy * dy) as f32).sqrt() as i32;
                self.horizon_px(o, (cx - hw, cy + dy, hw * 2, 1), color::hex(0xFFB060, alpha));
            }
        }
        for dy in -r..=0 {
            let hw = ((r * r - dy * dy) as f32).sqrt() as i32;
            let rgb = blend_rgb(0xFFF3B0, 0xFFB347, (dy + r) as f32 / r as f32);
            self.horizon_px(o, (cx - hw, cy + dy, hw * 2, 1), color::hex(rgb, 255));
        }

        // Clouds drifting left to right at different speeds.
        for i in 0..4i32 {
            let cw = 14 + i * 5;
            let span = cols + cw * 2;
            let drift = (t * (1.2 + i as f32 * 0.5)) as i32;
            let gx = (i * 53 + drift) % span - cw;
            let gy = (ROWS as f32 * (0.20 + i as f32 * 0.08)) as i32;
            let cloud = 0x8A3F9E;
            self.horizon_px(o, (gx, gy, cw, 1), color::hex(cloud, 150));
            self.horizon_px(o, (gx + 3, gy - 1, cw - 8, 1), color::hex(cloud, 150));
            self.horizon_px(o, (gx + 6, gy + 1, cw - 10, 1), color::hex(cloud, 110));
        }

        // Three parallax mountain ranges (far -> near); pines on the nearest one.
        const RANGES: [(u32, f32, f32); 3] = [
            (0x6B3A8C, 0.00, 0.8),
            (0x3B1D5C, 0.06, 1.8),
            (0x160A26, 0.12, 3.5),
        ];
        for (li, (rgb, drop, speed)) in RANGES.into_iter().enumerate() {
            let col = color::hex(rgb, 255);
            let base = horizon + (ROWS as f32 * drop) as i32 - 2;
            let scroll = (t * speed).floor();
            let amp = 7.0 + li as f32 * 2.0;
            let freq = 0.035 + li as f32 * 0.012;
            let phase = li as f32 * 1.9;
            let tops: Vec<i32> = (0..cols)
                .map(|gx| {
                    let wx = gx as f32 + scroll;
                    let ridge = amp
                        * (0.6 * (wx * freq + phase).sin()
                            + 0.4 * (wx * freq * 2.3 + phase * 1.7).sin());
                    base - (ridge as i32).max(-3) - 6
                })
                .collect();
            // Merge runs of equal height into one rect to keep the batch small.
            let mut start = 0usize;
            while start < tops.len() {
                let mut end = start + 1;
                while end < tops.len() && tops[end] == tops[start] {
                    end += 1;
                }
                let top = tops[start];
                self.horizon_px(o, (start as i32, top, (end - start) as i32, ROWS - top), col);
                start = end;
            }
            if li == 2 {
                for (gx, top) in tops.iter().enumerate() {
                    let wx = gx as i32 + scroll as i32;
                    if wx.rem_euclid(23) == 0 {
                        let gx = gx as i32;
                        self.horizon_px(o, (gx, top - 3, 1, 3), col);
                        self.horizon_px(o, (gx - 1, top - 2, 3, 1), col);
                        self.horizon_px(o, (gx - 2, top - 1, 5, 1), col);
                    }
                }
            }
        }
    }

    /// Animated pixel-art night sky ("Nightly" background) over `[x, y, w, h]` dp: a deep
    /// banded sky, stars that pulse at random rates (some with a sparkle cross), the odd
    /// shooting star, a crescent moon inside a faint glowing full moon, clouds drifting
    /// across it and three dark parallax mountain ranges. Frozen when reduced motion is on.
    pub fn draw_nightly_sky(&mut self, x: f32, y: f32, w: f32, h: f32) {
        use std::sync::OnceLock;
        use std::time::Instant;
        static START: OnceLock<Instant> = OnceLock::new();
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let t = if motion::reduced_motion() || motion::launcher_animations_disabled() {
            0.0
        } else {
            START.get_or_init(Instant::now).elapsed().as_secs_f32()
        };

        const ROWS: i32 = 90;
        let cell = h / ROWS as f32;
        let cols = (w / cell).ceil() as i32 + 1;
        let horizon = (ROWS as f32 * 0.72) as i32;
        let o = (x, y, cell);

        // Sky: 16 quantized bands, near-black navy -> indigo -> violet at the horizon.
        const SKY: [u32; 4] = [0x04060F, 0x0A1030, 0x1A2058, 0x35307A];
        const BANDS: i32 = 16;
        for band in 0..BANDS {
            let s = band as f32 / (BANDS - 1) as f32 * 3.0;
            let i = (s.floor() as usize).min(2);
            let rgb = blend_rgb(SKY[i], SKY[i + 1], s - i as f32);
            let y0 = band * horizon / BANDS;
            let y1 = (band + 1) * horizon / BANDS;
            self.horizon_px(o, (0, y0, cols, y1 - y0), color::hex(rgb, 255));
        }

        // Stars: fixed pseudo-random spots, each pulsing at its own rate and phase.
        // Every sixth one flares into a small sparkle cross at the top of its pulse.
        for i in 0..72u32 {
            let hsh = i.wrapping_mul(2_654_435_761).rotate_left(13) ^ i.wrapping_mul(40_503);
            let gx = (hsh % cols.max(1) as u32) as i32;
            let gy = ((hsh >> 9) % (horizon as u32 * 85 / 100).max(1)) as i32;
            let phase = ((hsh >> 16) & 0xFF) as f32 * 0.0246;
            let speed = 0.7 + ((hsh >> 24) & 0x7) as f32 * 0.32;
            let pulse = (0.5 + 0.5 * (t * speed + phase).sin()).powi(2);
            let a = (70.0 + 185.0 * pulse) as u8;
            let tint = match hsh % 3 {
                0 => 0xFFFFFF_u32,
                1 => 0xCFE2FF_u32,
                _ => 0xFFF1C9_u32,
            };
            self.horizon_px(o, (gx, gy, 1, 1), color::hex(tint, a));
            if i % 6 == 0 && pulse > 0.55 {
                let arm = color::hex(tint, (a as f32 * 0.55) as u8);
                self.horizon_px(o, (gx - 1, gy, 1, 1), arm);
                self.horizon_px(o, (gx + 1, gy, 1, 1), arm);
                self.horizon_px(o, (gx, gy - 1, 1, 1), arm);
                self.horizon_px(o, (gx, gy + 1, 1, 1), arm);
            }
        }

        // A shooting star streaks down-left across the upper sky every nine seconds.
        let cycle = (t / 9.0).floor();
        let local = t - cycle * 9.0;
        if t > 0.0 && local < 1.1 {
            let p = local / 1.1;
            let sh = (cycle as u32).wrapping_mul(2_246_822_519).rotate_left(11);
            let sx = (sh % (cols.max(2) as u32 / 2)) as i32 + cols / 3;
            let sy = 4 + ((sh >> 8) % 14) as i32;
            let hx = sx - (p * 28.0) as i32;
            let hy = sy + (p * 16.0) as i32;
            for k in 0..7i32 {
                let fade = (1.0 - k as f32 / 7.0) * (1.0 - p * 0.6);
                self.horizon_px(o, (hx + k * 2, hy - k, 2, 1), color::hex(0xFFFFFF, (255.0 * fade) as u8));
            }
        }

        // Moon: a breathing halo, a faint glowing full disc, then the bright crescent.
        let r = (ROWS as f32 * 0.115) as i32;
        let cx = (cols as f32 * 0.72) as i32;
        let cy = (ROWS as f32 * 0.27) as i32 + ((t * 0.35).sin() * 1.5) as i32;
        let glow = 0.75 + 0.25 * (t * 0.7).sin();
        for (mult, alpha) in [(2.6_f32, 14.0_f32), (2.0, 22.0), (1.5, 34.0)] {
            let gr = (r as f32 * mult) as i32;
            for dy in -gr..=gr {
                let hw = ((gr * gr - dy * dy) as f32).sqrt() as i32;
                self.horizon_px(
                    o,
                    (cx - hw, cy + dy, hw * 2 + 1, 1),
                    color::hex(0x9DB4FF, (alpha * glow) as u8),
                );
            }
        }
        for dy in -r..=r {
            let hw = ((r * r - dy * dy) as f32).sqrt() as i32;
            self.horizon_px(
                o,
                (cx - hw, cy + dy, hw * 2 + 1, 1),
                color::hex(0xC9D6FF, (46.0 * glow) as u8),
            );
        }
        let ox = (r as f32 * 0.42) as i32;
        let oy = -((r as f32 * 0.12) as i32);
        let r2 = (r as f32 * 0.86) as i32;
        for dy in -r..=r {
            let hw = ((r * r - dy * dy) as f32).sqrt() as i32;
            let (l, rt) = (cx - hw, cx + hw);
            let rgb = blend_rgb(0xFFFBE0, 0xE8DDAE, (dy + r) as f32 / (2 * r) as f32);
            let col = color::hex(rgb, 255);
            let dy2 = dy - oy;
            if dy2.abs() <= r2 {
                let hw2 = ((r2 * r2 - dy2 * dy2) as f32).sqrt() as i32;
                let (cl, cr) = (cx + ox - hw2, cx + ox + hw2);
                if cl > l {
                    self.horizon_px(o, (l, cy + dy, (cl - 1).min(rt) - l + 1, 1), col);
                }
                if cr < rt {
                    let s = (cr + 1).max(l);
                    self.horizon_px(o, (s, cy + dy, rt - s + 1, 1), col);
                }
            } else {
                self.horizon_px(o, (l, cy + dy, rt - l + 1, 1), col);
            }
        }

        // Clouds drifting left to right at different speeds, lit along the top by the moon.
        for i in 0..6i32 {
            let cw = 16 + (i % 3) * 7;
            let span = cols + cw * 2;
            let speed = 1.0 + i as f32 * 0.45;
            let drift = (t * speed) as i32;
            let gx = (i * 47 + drift) % span - cw;
            let gy = (ROWS as f32 * (0.14 + i as f32 * 0.07)) as i32;
            let body = 0x1B2650;
            self.horizon_px(o, (gx, gy, cw, 1), color::hex(body, 170));
            self.horizon_px(o, (gx + 3, gy - 1, cw - 8, 1), color::hex(body, 170));
            self.horizon_px(o, (gx + 6, gy + 1, cw - 10, 1), color::hex(body, 120));
            self.horizon_px(o, (gx + 3, gy - 2, cw - 8, 1), color::hex(0x6F86C8, 70));
        }

        // Three parallax mountain ranges (far -> near); pines on the nearest one.
        const RANGES: [(u32, f32, f32); 3] = [
            (0x1F2A63, 0.00, 0.8),
            (0x131B48, 0.06, 1.8),
            (0x090E28, 0.12, 3.5),
        ];
        for (li, (rgb, drop, speed)) in RANGES.into_iter().enumerate() {
            let col = color::hex(rgb, 255);
            let base = horizon + (ROWS as f32 * drop) as i32 - 2;
            let scroll = (t * speed).floor();
            let amp = 7.0 + li as f32 * 2.0;
            let freq = 0.035 + li as f32 * 0.012;
            let phase = li as f32 * 1.9;
            let tops: Vec<i32> = (0..cols)
                .map(|gx| {
                    let wx = gx as f32 + scroll;
                    let ridge = amp
                        * (0.6 * (wx * freq + phase).sin()
                            + 0.4 * (wx * freq * 2.3 + phase * 1.7).sin());
                    base - (ridge as i32).max(-3) - 6
                })
                .collect();
            let mut start = 0usize;
            while start < tops.len() {
                let mut end = start + 1;
                while end < tops.len() && tops[end] == tops[start] {
                    end += 1;
                }
                let top = tops[start];
                self.horizon_px(o, (start as i32, top, (end - start) as i32, ROWS - top), col);
                start = end;
            }
            if li == 2 {
                for (gx, top) in tops.iter().enumerate() {
                    let wx = gx as i32 + scroll as i32;
                    if wx.rem_euclid(23) == 0 {
                        let gx = gx as i32;
                        self.horizon_px(o, (gx, top - 3, 1, 3), col);
                        self.horizon_px(o, (gx - 1, top - 2, 3, 1), col);
                        self.horizon_px(o, (gx - 2, top - 1, 5, 1), col);
                    }
                }
            }
        }
    }

    /// Draws the user's GIF (see [`CustomBackground`]) cover-fitted over `[x, y, w, h]` dp,
    /// run-length merged into rectangles and darkened a little so text stays readable.
    /// Shows a plain dark fill until a GIF has been decoded. Frozen on the first frame when
    /// reduced motion is on.
    pub fn draw_custom_background(&mut self, x: f32, y: f32, w: f32, h: f32) {
        use std::sync::OnceLock;
        use std::time::Instant;
        static START: OnceLock<Instant> = OnceLock::new();
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        self.fill(x, y, w, h, color::hex(0x0B1016, 255));
        let background = CUSTOM_BACKGROUND
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let Some(background) = background else {
            return;
        };
        let total: u32 = background.delays_ms.iter().sum::<u32>().max(1);
        let animate = !(motion::reduced_motion() || motion::launcher_animations_disabled());
        let now_ms: u32 = if animate {
            (START.get_or_init(Instant::now).elapsed().as_millis() % total as u128) as u32
        } else {
            0
        };
        let mut index = 0usize;
        let mut elapsed = 0u32;
        for (i, delay) in background.delays_ms.iter().enumerate() {
            elapsed += *delay;
            if now_ms < elapsed {
                index = i;
                break;
            }
        }
        let cols = background.cols as usize;
        let rows = background.rows as usize;
        let Some(frame) = background.frames.get(index) else {
            return;
        };
        if cols == 0 || rows == 0 || frame.len() < cols * rows {
            return;
        }
        let cell = (w / cols as f32).max(h / rows as f32);
        let ox = x + (w - cell * cols as f32) * 0.5;
        let oy = y + (h - cell * rows as f32) * 0.5;
        for row in 0..rows {
            let y0 = oy + row as f32 * cell;
            let y1 = oy + (row + 1) as f32 * cell;
            if y1 < y || y0 > y + h {
                continue;
            }
            let line = &frame[row * cols..(row + 1) * cols];
            let mut start = 0usize;
            while start < cols {
                let rgb = line[start];
                let mut end = start + 1;
                while end < cols && line[end] == rgb {
                    end += 1;
                }
                let x0 = ox + start as f32 * cell;
                let x1 = ox + end as f32 * cell;
                if x1 >= x && x0 <= x + w {
                    self.fill(x0, y0, x1 - x0, y1 - y0, color::hex(rgb, 255));
                }
                start = end;
            }
        }
        self.fill(x, y, w, h, color::hex(0x000000, 96));
    }

    /// Jetpack Compose Material 3 `ElevatedCard` / `Card(surfaceContainerLow)` with tonal elevation
    /// and subtle `outlineVariant` keyline (no harsh 1px neon borders).
    pub fn m3_tonal_card(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, elevated: bool) {
        let theme = active_theme();
        let bg = if elevated {
            theme.surface
        } else {
            theme.surface_dim
        };
        self.rounded(x, y, w, h, radius, bg);
        self.border(
            x,
            y,
            w,
            h,
            radius,
            color::with_alpha(theme.border, if elevated { 0.55 } else { 0.38 }),
        );
    }

    // ---- the launcher's components & animated Horizon intro ----

    /// Draws the Horizon sunset logo centered at `(cx, cy)` in `size x size` dp: the scene
    /// builds up from the skyline to the sun, with a soft warm glow ring behind it.
    pub fn draw_animated_horizon_mark(
        &mut self,
        cx: f32,
        cy: f32,
        size: f32,
        elapsed: f32,
        alpha: f32,
    ) {
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.01 || size <= 4.0 {
            return;
        }
        let glow = 0xFFB36B_u32;
        let ring_t = motion::ease_out_cubic((elapsed / 0.55).clamp(0.0, 1.0));
        let ring_r = size * (0.56 + 0.08 * ring_t);
        self.rounded(
            cx - ring_r,
            cy - ring_r,
            ring_r * 2.0,
            ring_r * 2.0,
            ring_r * 0.36,
            color::with_alpha(color::hex(glow, 255), 0.14 * ring_t * a),
        );
        self.stroke_rounded(
            cx - ring_r,
            cy - ring_r,
            ring_r * 2.0,
            ring_r * 2.0,
            ring_r * 0.36,
            1.5,
            color::with_alpha(color::hex(glow, 255), 0.38 * ring_t * a),
        );

        let pop = motion::ease_out_quint((elapsed / 0.50).clamp(0.0, 1.0));
        let reveal = motion::ease_out_cubic((elapsed / 0.85).clamp(0.0, 1.0));
        let logo = size * (0.88 + 0.12 * pop);
        self.draw_horizon_logo_art(cx - logo * 0.5, cy - logo * 0.5, logo, a, reveal);
    }

    /// Renders a clean, modern, minimal animated Horizon intro screen centered on the
    /// procedural Horizon `H` logo with real initialization progress and smooth cross-dissolve.
    pub fn draw_horizon_intro(&mut self, elapsed: f32, loading_progress: f32, status_text: &str) {
        let real_prog = loading_progress.clamp(0.0, 1.0);
        let dissolve = if real_prog >= 0.999 && elapsed > 1.15 {
            1.0 - motion::ease_out_cubic(((elapsed - 1.15) / 0.40).clamp(0.0, 1.0))
        } else {
            1.0
        };
        if dissolve <= 0.01 {
            return;
        }
        let theme = active_theme();
        let (w, h) = (self.w, self.h);
        self.fill(
            0.0,
            0.0,
            w,
            h,
            color::with_alpha(theme.bg_solid, 0.97 * dissolve),
        );

        let cx = w * 0.5;
        let cy = (h * 0.37).clamp(110.0, h - 150.0);
        let mark_size = (h * 0.19).clamp(64.0, 88.0);
        self.draw_animated_horizon_mark(cx, cy, mark_size, elapsed, dissolve);

        let title_t =
            motion::ease_out_cubic(((elapsed - 0.18).max(0.0) / 0.42).clamp(0.0, 1.0)) * dissolve;
        if title_t > 0.02 {
            let slide = (1.0 - title_t) * 8.0;
            let title_y = cy + mark_size * 0.68 + slide;
            self.text_centered(
                Face::SemiBold,
                24.0,
                cx,
                title_y,
                color::with_alpha(theme.text_primary, title_t),
                "HORIZON",
            );
            let sub_y = title_y + 32.0;
            self.text_centered(
                Face::Medium,
                11.5,
                cx,
                sub_y,
                color::with_alpha(theme.text_muted, title_t * 0.9),
                "RUST EDITION  •  ANDROID LAUNCHER",
            );

            // Real initialization progress indicator
            let bar_w = (w * 0.34).clamp(180.0, 260.0);
            let bar_h = 6.0;
            let bar_x = cx - bar_w * 0.5;
            let bar_y = sub_y + 28.0;
            self.rounded(
                bar_x,
                bar_y,
                bar_w,
                bar_h,
                3.0,
                color::with_alpha(theme.surface_bright, title_t),
            );
            let fill_w = (bar_w * real_prog.max(0.05)).clamp(bar_h, bar_w);
            self.rounded(
                bar_x,
                bar_y,
                fill_w,
                bar_h,
                3.0,
                color::with_alpha(theme.accent, title_t),
            );

            let msg = if status_text.trim().is_empty() {
                if real_prog >= 0.999 {
                    "Ready • Tap anywhere to continue".to_owned()
                } else {
                    format!("Initializing workspace... {:.0}%", real_prog * 100.0)
                }
            } else {
                format!("{status_text} ({:.0}%)", real_prog * 100.0)
            };
            let fitted_msg = self.fit_text(Face::Regular, 11.0, &msg, (w - 48.0).max(120.0));
            self.text_centered(
                Face::Regular,
                11.0,
                cx,
                bar_y + 12.0,
                color::with_alpha(theme.text_muted, title_t * 0.88),
                &fitted_msg,
            );
        }
    }
}

// ---- the boot screen ----

/// The boot screen's palette on the pure black: a white wordmark and dimmed
/// console lines (the theme's ink is for light pages).
const BOOT_TEXT: u32 = color::hex(0xFFFFFF, 255);
const BOOT_LOG: u32 = color::hex(0x9BA1A6, 255);

/// The boot screen while the menu is not ready yet: the wordmark centred on
/// pure black and the console's last lines at the bottom right, as the
/// terminal prints them.
pub fn draw_boot(atlas: &HudAtlas, batch: &mut HudBatch, logs: &[String]) {
    // Pure black under everything: the frame's panorama (the title artwork
    // once the bundle is in) must not leak through.
    batch.fill(
        atlas,
        0.0,
        0.0,
        batch.width,
        batch.height,
        color::hex(0x000000, 255),
    );
    wordmark(atlas, batch);
    draw_boot_log(atlas, batch, logs);
}

/// The wordmark with its tagline, crisp vector H mark, and progress bar, centred.
fn wordmark(atlas: &HudAtlas, batch: &mut HudBatch) {
    let mut ui = Ui::new(atlas, batch, (0.0, 0.0));
    let size = (ui.w / 16.0).clamp(DISPLAY, 96.0);
    let small = (size * 0.25).clamp(MICRO, 18.0);
    let block = size * 1.45 + small * 0.6 + small * 1.5;
    let top = (ui.h - block) / 2.0;
    let cx = ui.w / 2.0;

    // Crisp vector H mark centered symmetrically above the wordmark
    let mark_s = (size * 0.62).clamp(28.0, 52.0);
    let mark_cy = (top - mark_s * 0.72).max(mark_s * 0.6);
    let bar_w = (mark_s * 0.14).max(3.0);
    let bar_h = (mark_s * 0.56).max(10.0);
    let off_x = mark_s * 0.18;
    let accent = active_theme().accent;
    ui.rounded(
        cx - off_x - bar_w * 0.5,
        mark_cy - bar_h * 0.5,
        bar_w,
        bar_h,
        bar_w * 0.5,
        BOOT_TEXT,
    );
    ui.rounded(
        cx + off_x - bar_w * 0.5,
        mark_cy - bar_h * 0.5,
        bar_w,
        bar_h,
        bar_w * 0.5,
        accent,
    );
    ui.rounded(
        cx - off_x,
        mark_cy - bar_w * 0.45,
        off_x * 2.0,
        bar_w * 0.9,
        bar_w * 0.45,
        accent,
    );

    ui.text_centered(Face::SemiBold, size, cx, top, BOOT_TEXT, "Horizon");
    let sub_y = top + size * 1.45 + small * 0.6;
    ui.text_centered(
        Face::Mono,
        small,
        cx,
        sub_y,
        BOOT_LOG,
        "Rust based minecraft client",
    );

    // Symmetrical centered startup progress bar below the tagline
    let prog_w = (size * 1.8).clamp(80.0, 140.0);
    let prog_y = sub_y + small * 1.7;
    ui.rounded(
        cx - prog_w * 0.5,
        prog_y,
        prog_w,
        4.0,
        2.0,
        color::with_alpha(BOOT_LOG, 0.28),
    );
    let pulse_w = prog_w * 0.56;
    ui.rounded(cx - pulse_w * 0.5, prog_y, pulse_w, 4.0, 2.0, accent);
    ui.finish();
}

/// The console's last lines at the bottom left, over any loading screen: long
/// lines wrap word-wise, the block grows upwards.
pub fn draw_boot_log(atlas: &HudAtlas, batch: &mut HudBatch, logs: &[String]) {
    if logs.is_empty() {
        return;
    }
    let mut ui = Ui::new(atlas, batch, (0.0, 0.0));
    let (size, line, pad) = (MICRO, MICRO * 1.5, 16.0);
    let max_w = ui.w / 2.0;
    let max_rows = (((ui.h - pad) / line) as usize).clamp(1, 24);
    let mut y = ui.h - pad - line;
    let mut drawn = 0;
    'lines: for text in logs.iter().rev() {
        let chunks = wrapping_lines(&ui, Face::Mono, size, text, max_w);
        for chunk in chunks.iter().rev() {
            if drawn == max_rows {
                break 'lines;
            }
            ui.text(Face::Mono, size, pad, y, BOOT_LOG, chunk);
            y -= line;
            drawn += 1;
        }
    }
    ui.finish();
}

/// The lines `text` wraps into at `max_w`, word-wise; a word wider than
/// `max_w` on its own breaks at the edge.
fn wrapping_lines(ui: &Ui, face: Face, size: f32, text: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let mut word = word;
        loop {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if ui.measure(face, size, &candidate) <= max_w {
                line = candidate;
                break;
            }
            if line.is_empty() {
                // The word alone does not fit: hard-break it at the edge.
                let mut end = 0;
                for (i, ch) in word.char_indices() {
                    if i > 0 && ui.measure(face, size, &word[..i]) > max_w {
                        break;
                    }
                    end = i + ch.len_utf8();
                }
                lines.push(word[..end].to_owned());
                word = &word[end..];
            } else {
                lines.push(std::mem::take(&mut line));
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

// ---- Centralized Launcher Theme System & Motion Utilities ----

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePreset {
    ObsidianViolet,
    CyberCyan,
    #[default]
    EmeraldForest,
    CrimsonEmber,
    SolarGold,
    SakuraRose,
    DaylightFrost,
    Custom,
}

impl ThemePreset {
    pub const ALL: [Self; 8] = [
        Self::ObsidianViolet,
        Self::CyberCyan,
        Self::EmeraldForest,
        Self::CrimsonEmber,
        Self::SolarGold,
        Self::SakuraRose,
        Self::DaylightFrost,
        Self::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ObsidianViolet => "Obsidian Violet",
            Self::CyberCyan => "Cyber Cyan",
            Self::EmeraldForest => "Emerald Forest",
            Self::CrimsonEmber => "Crimson Ember",
            Self::SolarGold => "Solar Gold",
            Self::SakuraRose => "Sakura Rose",
            Self::DaylightFrost => "Daylight Frost",
            Self::Custom => "Custom Theme",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::ObsidianViolet => "Signature Horizon dark violet",
            Self::CyberCyan => "Deep slate with electric cyan",
            Self::EmeraldForest => "Woodland moss with emerald mint",
            Self::CrimsonEmber => "Obsidian ruby with warm coral",
            Self::SolarGold => "Dark bronze with radiant amber",
            Self::SakuraRose => "Twilight plum with blossom pink",
            Self::DaylightFrost => "High-contrast Material 3 light",
            Self::Custom => "Personalized colors & gradients",
        }
    }

    /// Returns `(bg_rgb, surface_rgb, accent_rgb, grad_start_rgb, grad_mid_rgb, grad_end_rgb, is_light)`
    pub fn palette_rgb(self) -> (u32, u32, u32, u32, u32, u32, bool) {
        match self {
            Self::ObsidianViolet | Self::Custom => (
                0x13111C, 0x1D192B, 0x9A7BFF, 0x161129, 0x1E1638, 0x0E0C16, false,
            ),
            Self::CyberCyan => (
                0x0C131F, 0x152033, 0x38BDF8, 0x0D1B2E, 0x102942, 0x09101B, false,
            ),
            Self::EmeraldForest => (
                0x0D1714, 0x152620, 0x34D399, 0x0F231C, 0x163329, 0x0A120F, false,
            ),
            Self::CrimsonEmber => (
                0x191013, 0x26181D, 0xF87171, 0x261219, 0x361922, 0x120B0E, false,
            ),
            Self::SolarGold => (
                0x18140D, 0x251F15, 0xFBBF24, 0x241C0F, 0x332714, 0x120F0A, false,
            ),
            Self::SakuraRose => (
                0x18111A, 0x261A29, 0xF472B6, 0x251429, 0x341C3A, 0x110C13, false,
            ),
            Self::DaylightFrost => (
                0xF3F1FA, 0xFFFFFF, 0x6750A4, 0xEDE8FA, 0xF5F2FC, 0xE4DDF8, true,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeAppearance {
    #[default]
    Dark,
    Light,
}

impl ThemeAppearance {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark Mode",
            Self::Light => "Light Mode",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackgroundMode {
    #[default]
    Solid,
    Gradient,
    /// Animated pixel-art sunset landscape.
    Horizon,
    /// Animated pixel-art night sky with a crescent moon, stars and clouds.
    Nightly,
    /// A GIF chosen by the user, drawn as a low-resolution pixel-art animation.
    Custom,
}

impl BackgroundMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Solid => "Solid Color",
            Self::Gradient => "Multi-Stop Gradient",
            Self::Horizon => "Horizon Sunset",
            Self::Nightly => "Nightly Sky",
            Self::Custom => "Custom GIF",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GradientDirection {
    #[default]
    Vertical,
    Horizontal,
    DiagonalDown,
    DiagonalUp,
}

impl GradientDirection {
    pub const ALL: [Self; 4] = [
        Self::Vertical,
        Self::Horizontal,
        Self::DiagonalDown,
        Self::DiagonalUp,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Vertical => "Top → Bottom",
            Self::Horizontal => "Left → Right",
            Self::DiagonalDown => "Diagonal Top → Bottom",
            Self::DiagonalUp => "Diagonal Bottom → Top",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Vertical => Self::Horizontal,
            Self::Horizontal => Self::DiagonalDown,
            Self::DiagonalDown => Self::DiagonalUp,
            Self::DiagonalUp => Self::Vertical,
        }
    }
}

/// Curated palette swatches for quick one-tap selection in Launcher Settings -> Theme.
pub const THEME_ACCENT_SWATCHES: [(u32, &str, &str); 8] = [
    (0x9A7BFF, "#9A7BFF", "Violet"),
    (0x38BDF8, "#38BDF8", "Cyan"),
    (0x34D399, "#34D399", "Emerald"),
    (0xF87171, "#F87171", "Crimson"),
    (0xFBBF24, "#FBBF24", "Amber"),
    (0xF472B6, "#F472B6", "Sakura"),
    (0x60A5FA, "#60A5FA", "Cobalt"),
    (0xA78BFA, "#A78BFA", "Iris"),
];

pub const THEME_BG_SWATCHES_DARK: [(u32, &str, &str); 6] = [
    (0x13111C, "#13111C", "Obsidian"),
    (0x0C131F, "#0C131F", "Abyss Blue"),
    (0x0D1714, "#0D1714", "Deep Pine"),
    (0x191013, "#191013", "Dark Ruby"),
    (0x141418, "#141418", "Carbon"),
    (0x0A0A0E, "#0A0A0E", "OLED Pitch"),
];

pub const THEME_BG_SWATCHES_LIGHT: [(u32, &str, &str); 6] = [
    (0xF3F1FA, "#F3F1FA", "Frost Lavender"),
    (0xEEF4FA, "#EEF4FA", "Ice Sky"),
    (0xEFF7F3, "#EFF7F3", "Mint Cream"),
    (0xFAF1F2, "#FAF1F2", "Rose Quartz"),
    (0xF9F6EE, "#F9F6EE", "Warm Ivory"),
    (0xF4F5F7, "#F4F5F7", "Pure Alabaster"),
];

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LauncherThemeConfig {
    pub preset: ThemePreset,
    pub appearance: ThemeAppearance,
    pub background_mode: BackgroundMode,
    pub custom_bg_hex: String,
    pub custom_surface_hex: String,
    pub custom_accent_hex: String,
    pub gradient_direction: GradientDirection,
    pub gradient_start_hex: String,
    pub gradient_mid_hex: String,
    pub gradient_end_hex: String,
    pub gradient_mid_pos: f32,
    pub use_three_stops: bool,
}

impl Default for LauncherThemeConfig {
    fn default() -> Self {
        Self {
            preset: ThemePreset::EmeraldForest,
            appearance: ThemeAppearance::Dark,
            background_mode: BackgroundMode::Solid,
            custom_bg_hex: "#0D1714".into(),
            custom_surface_hex: "#152620".into(),
            custom_accent_hex: "#34D399".into(),
            gradient_direction: GradientDirection::DiagonalDown,
            gradient_start_hex: "#0F231C".into(),
            gradient_mid_hex: "#163329".into(),
            gradient_end_hex: "#0A120F".into(),
            gradient_mid_pos: 0.50,
            use_three_stops: true,
        }
    }
}

/// Safely parses a hex color string (`#RRGGBB`, `RRGGBB`, `#RGB`, `0xRRGGBB`) into `0xRRGGBB`.
/// Returns `None` on invalid input so callers can provide sensible fallbacks.
pub fn parse_hex_color(input: &str) -> Option<u32> {
    let clean = input
        .trim()
        .trim_start_matches('#')
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if clean.len() == 6 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
        u32::from_str_radix(clean, 16).ok()
    } else if clean.len() == 3 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut expanded = String::with_capacity(6);
        for ch in clean.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        u32::from_str_radix(&expanded, 16).ok()
    } else {
        None
    }
}

pub fn format_hex_color(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFFFFFF)
}

fn rgb_luminance(rgb: u32) -> f32 {
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn blend_rgb(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ar = ((a >> 16) & 0xFF) as f32;
    let ag = ((a >> 8) & 0xFF) as f32;
    let ab = (a & 0xFF) as f32;
    let br = ((b >> 16) & 0xFF) as f32;
    let bg = ((b >> 8) & 0xFF) as f32;
    let bb = (b & 0xFF) as f32;
    let r = (ar + (br - ar) * t).round().clamp(0.0, 255.0) as u32;
    let g = (ag + (bg - ag) * t).round().clamp(0.0, 255.0) as u32;
    let bl = (ab + (bb - ab) * t).round().clamp(0.0, 255.0) as u32;
    (r << 16) | (g << 8) | bl
}

/// Linearly interpolates two HUD ABGR colors (`color::hex` format).
pub fn lerp_color(c0: u32, c1: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let mut out = 0u32;
    for shift in [0, 8, 16, 24] {
        let v0 = ((c0 >> shift) & 0xFF) as f32;
        let v1 = ((c1 >> shift) & 0xFF) as f32;
        let v = (v0 + (v1 - v0) * t).round().clamp(0.0, 255.0) as u32;
        out |= v << shift;
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedTheme {
    pub is_light: bool,
    /// Animated "Horizon" sunset background is active.
    pub is_horizon: bool,
    /// Animated "Nightly" night-sky background is active.
    pub is_nightly: bool,
    /// The user's own "Custom" GIF background is active.
    pub is_custom: bool,
    pub is_gradient: bool,
    pub gradient_direction: GradientDirection,
    pub use_three_stops: bool,
    pub gradient_mid_pos: f32,
    /// Solid fallback background color (always valid even when gradient is enabled).
    pub bg_solid: u32,
    pub gradient_start: u32,
    pub gradient_mid: u32,
    pub gradient_end: u32,
    pub sidebar_bg: u32,
    pub header_bg: u32,
    pub surface: u32,
    pub surface_dim: u32,
    pub surface_bright: u32,
    pub surface_selected: u32,
    pub border: u32,
    pub border_strong: u32,
    pub accent: u32,
    pub accent_soft: u32,
    pub on_accent: u32,
    pub accent_text: u32,
    pub text_primary: u32,
    pub text_muted: u32,
    pub success: u32,
    pub danger: u32,
}

impl Default for ResolvedTheme {
    fn default() -> Self {
        LauncherThemeConfig::default().resolve()
    }
}

impl LauncherThemeConfig {
    pub fn sanitize(&mut self) {
        if !self.gradient_mid_pos.is_finite() {
            self.gradient_mid_pos = 0.50;
        }
        self.gradient_mid_pos = self.gradient_mid_pos.clamp(0.15, 0.85);
        let (def_bg, def_surf, def_acc, def_gs, def_gm, def_ge, _) = self.preset.palette_rgb();
        if parse_hex_color(&self.custom_bg_hex).is_none() {
            self.custom_bg_hex = format_hex_color(def_bg);
        }
        if parse_hex_color(&self.custom_surface_hex).is_none() {
            self.custom_surface_hex = format_hex_color(def_surf);
        }
        if parse_hex_color(&self.custom_accent_hex).is_none() {
            self.custom_accent_hex = format_hex_color(def_acc);
        }
        if parse_hex_color(&self.gradient_start_hex).is_none() {
            self.gradient_start_hex = format_hex_color(def_gs);
        }
        if parse_hex_color(&self.gradient_mid_hex).is_none() {
            self.gradient_mid_hex = format_hex_color(def_gm);
        }
        if parse_hex_color(&self.gradient_end_hex).is_none() {
            self.gradient_end_hex = format_hex_color(def_ge);
        }
    }

    pub fn apply_preset(&mut self, preset: ThemePreset) {
        self.preset = preset;
        if preset != ThemePreset::Custom {
            let (bg, surf, acc, gs, gm, ge, is_light) = preset.palette_rgb();
            self.appearance = if is_light {
                ThemeAppearance::Light
            } else {
                ThemeAppearance::Dark
            };
            self.custom_bg_hex = format_hex_color(bg);
            self.custom_surface_hex = format_hex_color(surf);
            self.custom_accent_hex = format_hex_color(acc);
            self.gradient_start_hex = format_hex_color(gs);
            self.gradient_mid_hex = format_hex_color(gm);
            self.gradient_end_hex = format_hex_color(ge);
        }
    }

    pub fn resolve(&self) -> ResolvedTheme {
        let (def_bg, def_surf, def_acc, def_gs, def_gm, def_ge, preset_light) =
            self.preset.palette_rgb();
        let is_light = if self.preset == ThemePreset::DaylightFrost {
            true
        } else {
            self.appearance == ThemeAppearance::Light || preset_light
        };

        let mut bg_rgb = if self.preset == ThemePreset::Custom {
            parse_hex_color(&self.custom_bg_hex).unwrap_or(def_bg)
        } else if is_light && self.preset != ThemePreset::DaylightFrost {
            blend_rgb(0xF4F2FA, def_acc, 0.06)
        } else {
            parse_hex_color(&self.custom_bg_hex).unwrap_or(def_bg)
        };

        // Ensure background matches light/dark appearance mode if user toggled appearance
        if is_light && rgb_luminance(bg_rgb) < 0.45 {
            bg_rgb = blend_rgb(0xF3F1FA, def_acc, 0.06);
        } else if !is_light && rgb_luminance(bg_rgb) > 0.65 {
            bg_rgb = def_bg;
        }

        let mut accent_rgb = parse_hex_color(&self.custom_accent_hex).unwrap_or(def_acc);
        // Protect accent legibility against background
        if is_light && rgb_luminance(accent_rgb) > 0.72 {
            accent_rgb = blend_rgb(accent_rgb, 0x1E1638, 0.45);
        } else if !is_light && rgb_luminance(accent_rgb) < 0.15 {
            accent_rgb = blend_rgb(accent_rgb, 0xFFFFFF, 0.45);
        }

        let surf_rgb = if is_light {
            blend_rgb(bg_rgb, 0xFFFFFF, 0.72)
        } else {
            let candidate = parse_hex_color(&self.custom_surface_hex).unwrap_or(def_surf);
            if rgb_luminance(candidate) > 0.55 {
                blend_rgb(bg_rgb, 0xFFFFFF, 0.07)
            } else {
                blend_rgb(bg_rgb, candidate, 0.75)
            }
        };

        let gs_rgb = parse_hex_color(&self.gradient_start_hex).unwrap_or(def_gs);
        let gm_rgb = parse_hex_color(&self.gradient_mid_hex).unwrap_or(def_gm);
        let ge_rgb = parse_hex_color(&self.gradient_end_hex).unwrap_or(def_ge);
        let (gs_adj, gm_adj, ge_adj) = if is_light {
            (
                if rgb_luminance(gs_rgb) < 0.45 {
                    blend_rgb(0xEDE8FA, accent_rgb, 0.14)
                } else {
                    gs_rgb
                },
                if rgb_luminance(gm_rgb) < 0.45 {
                    blend_rgb(0xF6F4FC, accent_rgb, 0.06)
                } else {
                    gm_rgb
                },
                if rgb_luminance(ge_rgb) < 0.45 {
                    blend_rgb(0xE3DCF7, accent_rgb, 0.18)
                } else {
                    ge_rgb
                },
            )
        } else {
            (gs_rgb, gm_rgb, ge_rgb)
        };

        let sidebar_rgb = if is_light {
            blend_rgb(bg_rgb, accent_rgb, 0.07)
        } else {
            blend_rgb(bg_rgb, 0x07060C, 0.35)
        };
        let header_rgb = if is_light {
            blend_rgb(bg_rgb, 0xFFFFFF, 0.50)
        } else {
            blend_rgb(bg_rgb, surf_rgb, 0.50)
        };
        // Jetpack Compose Material 3 tonal surface container steps:
        // surface_dim = surfaceContainerLow (elevated above background, never pitch black!)
        // surface = surfaceContainer
        // surface_bright = surfaceContainerHigh
        // surface_selected = secondaryContainer
        let surf_dim_rgb = if is_light {
            blend_rgb(bg_rgb, 0xFFFFFF, 0.52)
        } else {
            blend_rgb(bg_rgb, surf_rgb, 0.62)
        };
        let surf_bright_rgb = if is_light {
            blend_rgb(surf_rgb, accent_rgb, 0.11)
        } else {
            blend_rgb(surf_rgb, accent_rgb, 0.14)
        };
        let surf_sel_rgb = if is_light {
            blend_rgb(surf_rgb, accent_rgb, 0.22)
        } else {
            blend_rgb(surf_rgb, accent_rgb, 0.28)
        };
        let border_rgb = if is_light {
            blend_rgb(bg_rgb, accent_rgb, 0.18)
        } else {
            blend_rgb(surf_rgb, accent_rgb, 0.16)
        };
        let border_strong_rgb = if is_light {
            blend_rgb(bg_rgb, 0x1D192B, 0.34)
        } else {
            blend_rgb(surf_rgb, 0xFFFFFF, 0.28)
        };
        let on_accent_rgb = if rgb_luminance(accent_rgb) > 0.48 {
            0x140E26
        } else {
            0xFFFFFF
        };
        let text_primary_rgb = if is_light { 0x1A1626 } else { 0xF5F2FC };
        let text_muted_rgb = if is_light { 0x5E5675 } else { 0xA39BBF };

        // Solid fallback even if gradient mode is active
        let solid_fallback_rgb = if self.background_mode == BackgroundMode::Gradient {
            blend_rgb(gs_adj, ge_adj, 0.5)
        } else {
            bg_rgb
        };

        ResolvedTheme {
            is_light,
            is_horizon: self.background_mode == BackgroundMode::Horizon,
            is_nightly: self.background_mode == BackgroundMode::Nightly,
            is_custom: self.background_mode == BackgroundMode::Custom,
            is_gradient: self.background_mode == BackgroundMode::Gradient,
            gradient_direction: self.gradient_direction,
            use_three_stops: self.use_three_stops,
            gradient_mid_pos: self.gradient_mid_pos.clamp(0.15, 0.85),
            bg_solid: color::hex(solid_fallback_rgb, 255),
            gradient_start: color::hex(gs_adj, 255),
            gradient_mid: color::hex(gm_adj, 255),
            gradient_end: color::hex(ge_adj, 255),
            sidebar_bg: color::hex(sidebar_rgb, 244),
            header_bg: color::hex(header_rgb, 244),
            surface: color::hex(surf_rgb, 240),
            surface_dim: color::hex(surf_dim_rgb, 244),
            surface_bright: color::hex(surf_bright_rgb, 248),
            surface_selected: color::hex(surf_sel_rgb, 252),
            border: color::hex(border_rgb, 255),
            border_strong: color::hex(border_strong_rgb, 255),
            accent: color::hex(accent_rgb, 255),
            accent_soft: color::with_alpha(color::hex(accent_rgb, 255), 0.22),
            on_accent: color::hex(on_accent_rgb, 255),
            accent_text: color::hex(on_accent_rgb, 255),
            text_primary: color::hex(text_primary_rgb, 255),
            text_muted: color::hex(text_muted_rgb, 255),
            success: color::hex(if is_light { 0x15803D } else { 0x4ADE80 }, 255),
            danger: color::hex(if is_light { 0xDC2626 } else { 0xF87171 }, 255),
        }
    }
}

static ACTIVE_THEME: std::sync::Mutex<Option<ResolvedTheme>> = std::sync::Mutex::new(None);

/// A user-chosen GIF, pre-shrunk to a small colour grid so it can be drawn as rectangles.
/// `frames[i]` holds `cols * rows` cells as `0xRRGGBB`; `delays_ms[i]` is that frame's length.
pub struct CustomBackground {
    pub cols: u32,
    pub rows: u32,
    pub frames: Vec<Vec<u32>>,
    pub delays_ms: Vec<u32>,
}

static CUSTOM_BACKGROUND: std::sync::Mutex<Option<std::sync::Arc<CustomBackground>>> =
    std::sync::Mutex::new(None);

/// Installs (or clears) the decoded GIF used by [`BackgroundMode::Custom`].
pub fn set_custom_background(background: Option<CustomBackground>) {
    *CUSTOM_BACKGROUND.lock().unwrap_or_else(|e| e.into_inner()) =
        background.map(std::sync::Arc::new);
}

/// Whether a custom GIF has been decoded and is ready to draw.
pub fn custom_background_loaded() -> bool {
    CUSTOM_BACKGROUND
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some()
}

pub fn active_theme() -> ResolvedTheme {
    if let Ok(guard) = ACTIVE_THEME.lock()
        && let Some(theme) = *guard
    {
        return theme;
    }
    ResolvedTheme::default()
}

pub fn set_active_theme(theme: ResolvedTheme) {
    if let Ok(mut guard) = ACTIVE_THEME.lock() {
        *guard = Some(theme);
    }
}

/// Performant, hardware-conscious motion utilities following Material 3 Expressive
/// and Disney choreography principles. Automatically respects low-power / reduced-motion modes.
pub mod motion {
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Instant,
    };

    static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);
    static DISABLE_LAUNCHER_ANIMATIONS: AtomicBool = AtomicBool::new(false);
    static DISABLE_GAME_ANIMATIONS: AtomicBool = AtomicBool::new(false);

    pub fn set_reduced_motion(reduced: bool) {
        REDUCED_MOTION.store(reduced, Ordering::Relaxed);
    }

    pub fn reduced_motion() -> bool {
        REDUCED_MOTION.load(Ordering::Relaxed)
            || DISABLE_LAUNCHER_ANIMATIONS.load(Ordering::Relaxed)
    }

    pub fn set_launcher_animations_disabled(disabled: bool) {
        DISABLE_LAUNCHER_ANIMATIONS.store(disabled, Ordering::Relaxed);
    }

    pub fn launcher_animations_disabled() -> bool {
        DISABLE_LAUNCHER_ANIMATIONS.load(Ordering::Relaxed)
    }

    pub fn set_game_animations_disabled(disabled: bool) {
        DISABLE_GAME_ANIMATIONS.store(disabled, Ordering::Relaxed);
    }

    pub fn game_animations_disabled() -> bool {
        DISABLE_GAME_ANIMATIONS.load(Ordering::Relaxed)
    }

    /// Standard Material 3 decelerate cubic-bezier approximation (`1 - (1 - t)^3`).
    pub fn ease_out_cubic(t: f32) -> f32 {
        let x = t.clamp(0.0, 1.0);
        1.0 - (1.0 - x).powi(3)
    }

    /// Snappy quintic ease-out (`1 - (1 - t)^5`) for quick page/tab entrances.
    pub fn ease_out_quint(t: f32) -> f32 {
        let x = t.clamp(0.0, 1.0);
        1.0 - (1.0 - x).powi(5)
    }

    /// Critically damped Material 3 spatial spring response (smooth settle, zero distracting bounce).
    pub fn spring_spatial(t: f32) -> f32 {
        let x = t.clamp(0.0, 1.0);
        if x >= 1.0 {
            return 1.0;
        }
        1.0 - (-6.5 * x).exp() * (1.0 + 6.5 * x * 0.45)
    }

    /// Playful overshoot easing (`cubic-bezier(0.34, 1.56, 0.64, 1)`) for button & switch pops.
    pub fn ease_out_back(t: f32) -> f32 {
        let x = t.clamp(0.0, 1.0) - 1.0;
        let c1 = 1.70158;
        let c3 = c1 + 1.0;
        1.0 + c3 * x.powi(3) + c1 * x.powi(2)
    }

    /// Continuous ambient animation clock in seconds (returns `0.0` when `reduced_motion()` is active).
    pub fn ambient_time() -> f32 {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        if reduced_motion() {
            return 0.0;
        }
        START.get_or_init(Instant::now).elapsed().as_secs_f32()
    }

    /// Evaluates elapsed progress `(0.0..=1.0)` since `since` over `duration_secs`.
    /// Returns `1.0` immediately when `reduced_motion()` is enabled.
    pub fn progress(since: Option<Instant>, now: Instant, duration_secs: f32) -> f32 {
        if reduced_motion() || duration_secs <= 0.001 {
            return 1.0;
        }
        let Some(start) = since else {
            return 1.0;
        };
        let dt = now.duration_since(start).as_secs_f32();
        (dt / duration_secs).clamp(0.0, 1.0)
    }

    /// Material 2026 (M3 Expressive) Fast Spatial Spring (`stiffness: 380, damping: 22`):
    /// Underdamped second-order physical spring with ~6% expressive overshoot for switches,
    /// active nav indicators, filter chips, and button morphs.
    pub fn spring_expressive_spatial(t: f32) -> f32 {
        let x = t.clamp(0.0, 1.0);
        if x <= 0.0 || x >= 1.0 || reduced_motion() {
            return x;
        }
        let decay = (-7.2 * x).exp();
        1.0 - decay * ((8.4 * x).cos() + 0.55 * (8.4 * x).sin())
    }

    /// Material 2026 (M3 Expressive) Default Spatial Spring (`stiffness: 300, damping: 26`):
    /// Smooth spatial settle with gentle ~2.5% follow-through for cards, sheets, and dialogs.
    pub fn spring_expressive_default(t: f32) -> f32 {
        let x = t.clamp(0.0, 1.0);
        if x <= 0.0 || x >= 1.0 || reduced_motion() {
            return x;
        }
        let decay = (-7.8 * x).exp();
        1.0 - decay * ((5.6 * x).cos() + 0.85 * (5.6 * x).sin())
    }

    /// Material 2026 (M3 Expressive) Effects Spring (critically/over-damped, 0% bounce):
    /// Used exclusively for opacity, color, and elevation transitions so colors never overshoot.
    pub fn spring_expressive_effects(t: f32) -> f32 {
        let x = t.clamp(0.0, 1.0);
        if x <= 0.0 || x >= 1.0 {
            return x;
        }
        1.0 - (-8.5 * x).exp() * (1.0 + 8.5 * x * 0.35)
    }

    /// Material 2026 Dynamic Shape-Morphing Corner Radius:
    /// Morphs a component's corner radius across resting (`base_r`), selected (`active_r`),
    /// and pressed (`press_r`) states.
    pub fn morph_radius(
        base_r: f32,
        active_r: f32,
        press_r: f32,
        active_t: f32,
        press_t: f32,
    ) -> f32 {
        let a = active_t.clamp(0.0, 1.0);
        let p = press_t.clamp(0.0, 1.0);
        let r_sel = base_r + (active_r - base_r) * a;
        (r_sel + (press_r - r_sel) * p).max(2.0)
    }

    /// Micro-interaction tactile press scale (`0.965` squash on press, expressive spring rebound on release).
    pub fn press_scale(pressed: bool, release_progress: f32) -> f32 {
        if reduced_motion() {
            return 1.0;
        }
        if pressed {
            0.968
        } else if release_progress < 1.0 {
            let s = spring_expressive_spatial(release_progress.clamp(0.0, 1.0));
            0.968 + (1.0 - 0.968) * s
        } else {
            1.0
        }
    }

    /// Advances a continuous 1D second-order Hooke spring-damper state `(pos, vel)` toward `target` over `dt`.
    pub fn step_spring_damper(
        pos: &mut f32,
        vel: &mut f32,
        target: f32,
        stiffness: f32,
        damping: f32,
        dt: f32,
    ) {
        if reduced_motion() {
            *pos = target;
            *vel = 0.0;
            return;
        }
        let step_dt = dt.clamp(0.001, 0.033);
        let force = (target - *pos) * stiffness - *vel * damping;
        *vel += force * step_dt;
        *pos += *vel * step_dt;
        if (*pos - target).abs() < 0.05 && vel.abs() < 0.5 {
            *pos = target;
            *vel = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_reload_regenerates_text_and_corners_at_the_same_address() {
        let atlas = HudAtlas::build(Path::new("/nonexistent/horizon-test-assets")).unwrap();
        reset_atlas();
        let draw = |cache: &mut Cache| {
            let glyph = cache.fonts.face(Face::Regular).glyph_id('A');
            assert!(
                cache
                    .glyph(&atlas, Face::Regular, glyph, 16.0)
                    .rect
                    .is_some()
            );
            assert!(cache.corner(&atlas, 8, 0).is_some());
            std::mem::take(&mut cache.pending)
        };
        let before = with_cache(&atlas, draw);
        assert_eq!(before.len(), 2);
        assert!(with_cache(&atlas, draw).is_empty());
        reset_atlas();
        let after = with_cache(&atlas, draw);
        assert_eq!(after.len(), 2);
        for (a, b) in before.iter().zip(&after) {
            assert_eq!((a.x, a.y, a.width, a.height), (b.x, b.y, b.width, b.height));
            assert_eq!(a.rgba, b.rgba);
        }
        reset_atlas();
    }

    #[test]
    fn the_boot_atlas_holds_the_run_time_glyph_region_without_a_pack() {
        let atlas = HudAtlas::boot();
        assert_eq!((atlas.width, atlas.height), (1024, 1024));
        assert_eq!(
            atlas.sprites["horizon/ui-text"].texels((atlas.width, atlas.height)),
            (1024, 512)
        );
        assert_eq!(
            atlas.sprites["horizon/white"].texels((atlas.width, atlas.height)),
            (4, 4)
        );
    }

    #[test]
    fn the_boot_screen_centres_the_wordmark_and_left_aligns_the_console() {
        reset_atlas();
        let atlas = HudAtlas::boot();
        let mut batch = HudBatch::new([1280, 720]);
        draw_boot(&atlas, &mut batch, &[]);
        // The first quad is the pure black backdrop over the frame's panorama.
        let (w, h) = (batch.width, batch.height);
        let bg = &batch.vertices[..6];
        assert!(bg.iter().all(|v| v.color == 0xFF00_0000));
        assert!(bg.iter().all(|v| v.pos[0] == 0.0 || v.pos[0] == w));
        assert!(bg.iter().all(|v| v.pos[1] == 0.0 || v.pos[1] == h));
        assert!(
            !batch.patches.is_empty(),
            "the glyphs are queued for the atlas"
        );

        let mut batch = HudBatch::new([1280, 720]);
        wordmark(&atlas, &mut batch);
        let (min, max) = batch
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert!(
            min < batch.width / 2.0 && max > batch.width / 2.0,
            "the wordmark straddles the centre"
        );
        assert!(((min + max) / 2.0 - batch.width / 2.0).abs() < 1.0);

        let long = "[12:34:56] [Render thread/INFO]: Horizon: resources loaded in 1.0s; 1240 block states, 1500 items, 62 biomes; 8 worker threads".to_owned();
        let mut batch = HudBatch::new([1280, 720]);
        draw_boot_log(&atlas, &mut batch, &[long]);
        let (min, max) = batch
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert!(min >= 4.0, "the line stays inside the left inset");
        assert!(
            max - min <= batch.width / 2.0 + 1.0,
            "a too wide line is cut to half the screen"
        );

        let mut batch = HudBatch::new([1280, 720]);
        let ui = Ui::new(&atlas, &mut batch, (0.0, 0.0));
        let lines = wrapping_lines(
            &ui,
            Face::Mono,
            MICRO,
            "the quick brown fox jumps over the lazy dog",
            90.0,
        );
        assert!(lines.len() > 1, "a long line wraps");
        assert!(
            lines
                .iter()
                .all(|l| ui.measure(Face::Mono, MICRO, l) <= 90.0),
            "every wrapped line fits"
        );
        assert_eq!(
            lines
                .iter()
                .map(|l| l.split_whitespace().count())
                .sum::<usize>(),
            9,
            "the wrap keeps every word"
        );

        let mut batch = HudBatch::new([1280, 720]);
        draw_boot_log(&atlas, &mut batch, &[]);
        assert!(batch.vertices.is_empty() && batch.patches.is_empty());
        reset_atlas();
    }

    #[test]
    fn theme_config_parses_hex_safely_and_resolves_presets_and_gradients() {
        assert_eq!(parse_hex_color("#9A7BFF"), Some(0x9A7BFF));
        assert_eq!(parse_hex_color("38bdf8"), Some(0x38BDF8));
        assert_eq!(parse_hex_color("#F0A"), Some(0xFF00AA));
        assert_eq!(parse_hex_color("not-a-color"), None);

        let mut cfg = LauncherThemeConfig::default();
        cfg.custom_bg_hex = "invalid".into();
        cfg.gradient_mid_pos = 99.0;
        cfg.sanitize();
        assert_eq!(cfg.custom_bg_hex, "#13111C");
        assert_eq!(cfg.gradient_mid_pos, 0.85);

        cfg.apply_preset(ThemePreset::DaylightFrost);
        let light = cfg.resolve();
        assert!(light.is_light);
        assert_ne!(light.bg_solid, light.text_primary);

        cfg.background_mode = BackgroundMode::Gradient;
        cfg.gradient_direction = GradientDirection::DiagonalDown;
        let grad = cfg.resolve();
        assert!(grad.is_gradient);
        assert_ne!(grad.bg_solid >> 24, 0, "solid fallback is always opaque");
    }
}
