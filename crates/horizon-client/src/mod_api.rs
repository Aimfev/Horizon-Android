//! Trait mods implement `Hooks` and link into the game executable.
//! The executable passes the ordered `Mods` to `entry::main_with`.
//! Trait mods are process-wide, like mixin module statics.
//! Hooks use `&self + Send + Sync` for shared and reentrant calls; see `docs/native-mods.md`.
//! Generate the trait and dispatch with `cargo run -p xtask -- mod-api`.
use std::sync::{Arc, OnceLock};
use crate as horizon_client;

pub use crate::camera::{CameraEntity, FovPlayer};
pub use crate::command::Suggestion;
pub use crate::containers::{ModBlockEntity, ModContainer, ModFluidTanks};
pub use crate::crafting::{CustomRecipe, Ingredient};
pub use crate::extras::Corner;
pub use crate::extras_hud::Corners;
pub use crate::first_person::Hand;
pub use crate::fog::{FogData, FogType};
pub use crate::hud::HudBatch;
pub use crate::hud_layout::{GuiDraw, HealthInputs};
pub use crate::items::ItemStack;
pub use crate::merchant::MerchantOffer;
pub use crate::music::Music;
pub use crate::particles::{ParticleOptions, ParticleType};
pub use crate::text::Text;
pub use horizon_assets::item_model::DisplayContext;
pub use horizon_core::{
    geometry::Direction,
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
pub use horizon_world::consumable::UseAnimation;
pub use horizon_world::{
    fluid::FluidState,
    level::{BlockId, Dimension as LevelDimension, PlacementContext},
    loot::LootStack,
    nbt::{Compound, Tag},
    registry::BlockStateId,
};
pub use serde_json;
use serde_json::{Map, Value};
use std::time::Duration;
pub use wgpu;
/// The window events `input_key` hands over (`winit::event::KeyEvent`) and their key types.
pub use winit;

/// One mod block entity's frame, as `submit` gets it.
#[derive(Clone, Debug, Default)]
pub struct BlockEntityDraw {
    pub partial: f32,
    pub game_time: i64,
    pub light: u32,
}

impl BlockEntityDraw {
    pub fn push(&mut self) {}
    pub fn pop(&mut self) {}
    pub fn translate(&mut self, _by: [f32; 3]) {}
    pub fn rotate_about(&mut self, _axis: [f32; 3], _degrees: f32, _pivot: [f32; 3]) {}
    pub fn scale(&mut self, _by: [f32; 3]) {}
    pub fn keep(&mut self) {}
}

/// One face of a `native_draw_cube` box.
#[derive(Clone, Copy, Debug)]
pub struct CubeFace<'a> {
    pub sprite: &'a str,
    pub uv: Option<[f32; 4]>,
}

/// Biome tint colors for grass, foliage, dry foliage, water, and water fog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BiomeColors {
    pub grass: u32,
    pub foliage: u32,
    pub dry_foliage: u32,
    pub water: u32,
    pub water_fog: u32,
}

/// Block entity energy storage buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModEnergyBuffer {
    pub stored: i64,
    pub capacity: i64,
    pub max_receive: i64,
    pub max_extract: i64,
}

/// Item stack energy storage buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemEnergyBuffer {
    pub stored: i64,
    pub capacity: i64,
    pub max_receive: i64,
    pub max_extract: i64,
}

/// Background music layer entry.
#[derive(Clone, Debug, PartialEq)]
pub struct MusicLayer {
    pub sound: String,
    pub volume: f32,
    pub pitch: f32,
}

/// Recipe book category descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeBookCategory {
    pub id: String,
    pub icons: Vec<String>,
}

/// Context passed to custom loot conditions and functions.
#[derive(Clone, Copy, Debug)]
pub struct NativeLootContext<'a> {
    pub dimension: &'a str,
    pub origin: [f64; 3],
    pub tool: Option<&'a ItemStack>,
    pub explosion_radius: Option<f32>,
    pub luck: f32,
}

/// World-space render hooks for custom GPU passes.
pub trait RenderHooks: Send + Sync {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderNeeds {
    pub depth: bool,
}

#[derive(Debug)]
pub struct RenderNeedsEvent<'a> {
    pub needs: &'a mut RenderNeeds,
}

#[derive(Clone, Debug)]
pub struct WorldDrawn {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct WorldDrawnEvent<'a> {
    pub drawn: &'a mut WorldDrawn,
}

/// Server-authoritative attachment storage types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Holder {
    Level,
    Chunk([i32; 2]),
    BlockEntity([i32; 3]),
    Entity(u32),
    Player(u128),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentError {
    MissingHolder,
    InvalidData(String),
}

#[derive(Clone, Debug, Default)]
pub struct Attachments {
    pub entries: std::collections::BTreeMap<String, Tag>,
}

#[derive(Clone, Debug)]
pub struct AttachmentType<T> {
    pub id: &'static str,
    pub _marker: std::marker::PhantomData<T>,
}

pub trait AttachmentCodec: Sized {
    fn encode(&self) -> Tag;
    fn decode(tag: &Tag) -> Result<Self, AttachmentError>;
}

pub trait AttachmentHost {
    fn attachments(&mut self, holder: Holder) -> Result<&mut Attachments, AttachmentError>;
}

/// Bulk block edit operation.
#[derive(Clone, Debug, Default)]
pub struct NativeBlockRun {
    pub start: [i32; 3],
    pub len: u32,
    pub state: BlockStateId,
}

#[derive(Clone, Debug, Default)]
pub struct NativeBlockEdit {
    pub runs: Vec<NativeBlockRun>,
}

/// World clock snapshot (`game_time` and `day_time`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeClock {
    pub game_time: i64,
    pub day_time: i64,
}

/// Column block state snapshot.
#[derive(Clone, Debug, Default)]
pub struct NativeColumnBlocks {
    pub min_y: i32,
    pub states: Vec<BlockStateId>,
}

/// Declarative NeoForge-compatible data map type registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDataMapType {
    pub registry: String,
    pub id: String,
    pub synced: bool,
}

/// Experience orb entity snapshot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeExperienceOrb {
    pub id: u32,
    pub position: [f64; 3],
    pub value: u32,
    pub count: u32,
}

/// Falling block entity snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeFalling {
    pub drop_item: bool,
    pub hurts: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHeightmap {
    WorldSurface,
    MotionBlocking,
    MotionBlockingNoLeaves,
    OceanFloor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeldItem {
    pub item: String,
    pub count: u32,
    pub main_hand: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeHit {
    Block {
        pos: [i32; 3],
        face: Direction,
        location: [f64; 3],
        inside: bool,
    },
    Entity {
        id: u32,
        kind: String,
        location: [f64; 3],
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeMapDecoration {
    pub kind: String,
    pub x: i8,
    pub y: i8,
    pub rot: i8,
    pub label: Option<Text>,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeMapInfo<'a> {
    pub id: i32,
    pub dimension: &'a str,
    pub center: [i32; 2],
    pub scale: u8,
    pub locked: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeMobRender<'a> {
    pub id: u32,
    pub kind: &'a str,
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub body_yaw: f32,
    pub partial_tick: f32,
    pub light: u32,
    pub baby: bool,
    pub invisible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeModEvent {
    Custom { name: String, payload: Value },
}

#[derive(Clone, Copy, Debug)]
pub struct NativePlayerRender<'a> {
    pub uuid: u128,
    pub name: &'a str,
    pub slim: bool,
    pub crouching: bool,
    pub swimming: bool,
    pub fall_flying: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NativeShake {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
    pub offset: [f32; 3],
}

/// Camera values from the most recently extracted world frame.
#[derive(Clone, Debug)]
pub struct NativeCamera {
    pub position: [f64; 3],
    pub eye_height: f32,
    pub partial_tick: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
    /// Camera-relative view matrix in column-major order.
    pub view: [f32; 16],
    /// Reversed-Z projection matrix in column-major order.
    pub projection: [f32; 16],
    pub dimension: String,
}

/// A dispenser hook's answer: the slot's remainder (None empties it) and the
/// level event to play, `CLICK` (1000), `FAIL` (1001) or 0 for none.
pub type NativeDispensed = (Option<ItemStack>, i32);

/// What dropped the items heard by the `loot_drops` hook.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct NativeLootDrops<'a> {
    pub table: &'a str,
    pub dimension: &'a str,
    pub origin: [f64; 3],
    pub block: Option<([i32; 3], BlockStateId)>,
    pub entity: Option<(u32, &'a str)>,
    pub player: Option<u128>,
    pub tool: Option<&'a ItemStack>,
    pub explosion: Option<f32>,
    pub used: bool,
}

/// The open screen, for the screen hooks.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct NativeScreenView<'a> {
    pub id: &'static str,
    pub menu: Option<&'a str>,
    pub title: Option<&'a Text>,
    pub rect: [f32; 4],
    pub slots: usize,
    pub mouse: [f32; 2],
    pub partial_tick: f32,
}

/// A depth-tested outline of the client's block selection shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBlockRim {
    pub pos: [i32; 3],
    pub argb: u32,
    pub width: f32,
}

/// A camera-local visual light, with no gameplay light or shadow writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeColoredLight {
    pub at: [f64; 3],
    pub rgb: u32,
    pub radius: f32,
    pub intensity: f32,
}

/// The minimap a native mod shows (`native_minimap`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeMinimap {
    pub corner: Corner,
    pub size: f32,
    pub blocks_per_pixel: f32,
    pub rotate: bool,
    pub round: bool,
}

/// A mod block's screen over its menu (`native_screen`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeScreen {
    pub width: f32,
    pub height: f32,
    pub slots: Vec<Option<[f32; 2]>>,
    pub inventory_y: f32,
    pub inventory_x: f32,
    pub own_labels: bool,
    pub own_background: bool,
}

/// A native mod as Options > Mods lists it (`native_mods`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeMod {
    pub id: String,
    pub title: String,
    pub description: String,
    pub keys: Vec<[String; 3]>,
}

/// An item the panel shows (`native_panel_items`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeStack {
    pub item: String,
    pub count: u32,
    pub components: String,
}

/// What the player did on a native mod's panel (`native_panel_event`).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum NativePanelEvent {
    Clicked { index: u32, right: bool },
    Search(String),
    Key { key: String, stack: NativeStack },
    SlotClicked {
        recipe: u32,
        slot: u32,
        stack: NativeStack,
        right: bool,
    },
    Action(u32),
    Page(u32),
    Back,
    Tab(u32),
}

/// The zoom a native mod asks for (`native_zoom`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeZoom {
    pub factor: f32,
    pub smooth: bool,
    pub turn: f32,
}

/// A player on the server, as a server hook sees them.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeServerPlayer {
    pub uuid: u128,
    pub name: String,
    pub permission: u8,
    pub dimension: String,
    pub position: [f64; 3],
    pub game_mode: i32,
    pub health: f32,
    pub yaw: f32,
    pub pitch: f32,
}

/// A command a native mod adds.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NativeCommand {
    pub usage: String,
    pub level: u8,
}

impl NativeCommand {
    pub fn new(usage: &str) -> Self {
        Self {
            usage: usage.to_owned(),
            level: 0,
        }
    }
    pub fn op(usage: &str, level: u8) -> Self {
        Self {
            usage: usage.to_owned(),
            level: level.min(4),
        }
    }
}

/// Which `Game` observes the phase; a mirror `Game` never fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NativeAuthority {
    LocalAuthoritative,
    IntegratedServer,
    DedicatedServer,
    RemoteClient,
}

/// One open attempt of one world; it rejects a previous open's commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeWorldEpoch(u64);

impl NativeWorldEpoch {
    #[doc(hidden)]
    pub fn new(n: u64) -> Self {
        Self(n)
    }
    pub fn get(self) -> u64 {
        self.0
    }
}

/// An entity of the open world, for the `Host` methods that read or change one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityRef {
    epoch: NativeWorldEpoch,
    id: u32,
}

impl EntityRef {
    #[doc(hidden)]
    pub fn new(epoch: NativeWorldEpoch, id: u32) -> Self {
        Self { epoch, id }
    }
    pub fn id(self) -> u32 {
        self.id
    }
    pub fn epoch(self) -> NativeWorldEpoch {
        self.epoch
    }
}

/// A level of the open world, by its dimension (`minecraft:the_nether`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LevelRef {
    epoch: NativeWorldEpoch,
    dimension: &'static str,
}

impl LevelRef {
    #[doc(hidden)]
    pub fn new(epoch: NativeWorldEpoch, dimension: &'static str) -> Self {
        Self { epoch, dimension }
    }
    pub fn dimension(self) -> &'static str {
        self.dimension
    }
    pub fn epoch(self) -> NativeWorldEpoch {
        self.epoch
    }
}

/// Non-paused authoritative server ticks since open (not game or frame time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTickId(u64);

impl NativeTickId {
    #[doc(hidden)]
    pub fn new(n: u64) -> Self {
        Self(n)
    }
    pub fn get(self) -> u64 {
        self.0
    }
}

/// The world phases (`native_world_lifecycle`'s `phase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
#[non_exhaustive]
pub enum WorldPhase {
    WorldOpenReady,
    WorldOpenFailed,
    WorldCloseFinal,
    WorldCloseFailed,
}

/// The tick phases carry whether the world runs, including frozen steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServerTickPhase {
    ServerTickStart { world_running: bool },
    ServerTickEnd { world_running: bool },
}

/// Context for every world lifecycle phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NativeLifecycleContext {
    pub authority: NativeAuthority,
    pub epoch: NativeWorldEpoch,
    pub tick: NativeTickId,
}

/// The ordered list of mods shared by the client and server.
pub type Mods = Arc<[Box<dyn Hooks>]>;

pub static MODS: OnceLock<Mods> = OnceLock::new();

/// The process-wide list in dispatch order, or an empty list before installation.
pub fn mods() -> &'static [Box<dyn Hooks>] {
    MODS.get().map_or(&[], |mods| mods.as_ref())
}

/// The bundles' `[mod] id`s, one a [`mods`] entry in the same order.
static MOD_IDS: OnceLock<&'static [&'static str]> = OnceLock::new();

/// Names the entries of [`mods`] by their bundles' `[mod] id`s, so a panic in a hook names its mod.
pub fn name_mods(ids: &'static [&'static str]) {
    if MOD_IDS.set(ids).is_err() {
        log::warn!("Trait mods already named; ignoring the second list");
    }
}

fn entered<'a>(mods: &'a [Box<dyn Hooks>], hook: &'static str) -> Entered<'a> {
    Entered {
        mods: mods.iter().enumerate(),
        hook,
        guard: None,
    }
}

struct Entered<'a> {
    mods: std::iter::Enumerate<std::slice::Iter<'a, Box<dyn Hooks>>>,
    hook: &'static str,
    guard: Option<crate::crash::NativeModGuard>,
}

impl<'a> Iterator for Entered<'a> {
    type Item = &'a Box<dyn Hooks>;

    fn next(&mut self) -> Option<Self::Item> {
        drop(self.guard.take());
        let (index, hooks) = self.mods.next()?;
        self.guard = Some(crate::crash::enter_native_mod(
            crate::crash::NativeModCall {
                id: MOD_IDS.get().and_then(|ids| ids.get(index)).copied(),
                index,
                hook: self.hook,
            },
        ));
        Some(hooks)
    }
}

/// Whether a mod claims a cancellable hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Cancel,
}

mod generated_host;
pub use generated_host::Host;

/// Result of accessing or decoding an attachment.
pub type AttachmentResult<T> = std::result::Result<T, AttachmentError>;

impl AttachmentHost for dyn Host + '_ {
    fn attachments(&mut self, holder: Holder) -> Result<&mut Attachments, AttachmentError> {
        Host::attachments(self, holder)
    }
}

/// Read-only level services, like vanilla's `LevelReader`.
pub trait LevelHost {
    fn block_state(&self, pos: [i32; 3]) -> BlockStateId;
    fn is_client_side(&self) -> bool;
    fn state_with_property(
        &self,
        state: BlockStateId,
        key: &str,
        value: &str,
    ) -> Option<BlockStateId>;
}

/// Access vanilla's `Level.random`, so draws come in vanilla's order.
pub trait LevelRandomHost: LevelHost {
    fn attachments(&mut self, holder: Holder) -> Result<&mut Attachments, AttachmentError>;
    fn random(&mut self) -> Option<&mut LegacyRandom>;
}

/// A `LevelReader` with vanilla's `ScheduledTickAccess`. Shape updates cannot write blocks.
pub trait LevelTicksHost: LevelHost {
    fn schedule_block_tick(&mut self, pos: [i32; 3], block: &str, delay: i32) -> bool;
    fn schedule_fluid_tick(&mut self, pos: [i32; 3], fluid: &str, delay: i32) -> bool;
}

/// Mutable level services, like vanilla's `ServerLevel`.
pub trait LevelWriteHost: LevelTicksHost + LevelRandomHost {
    fn set_block_property(&mut self, pos: [i32; 3], key: &str, value: &str, flags: i32) -> bool;
    fn play_sound(
        &mut self,
        sound: &str,
        at: [f64; 3],
        source: usize,
        volume: f32,
        pitch: f32,
    ) -> bool;
    fn send_particles(
        &mut self,
        options: &ParticleOptions,
        state: Option<BlockStateId>,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: i32,
    ) -> bool;
}

macro_rules! level_attachment_host {
    ($($host:ident),+) => { $(
        impl AttachmentHost for dyn $host + '_ {
            fn attachments(&mut self, holder: Holder) -> Result<&mut Attachments, AttachmentError> {
                LevelRandomHost::attachments(self, holder)
            }
        }
    )+ };
}
level_attachment_host!(LevelRandomHost, LevelWriteHost);

mod generated;
pub use generated::Hooks;
#[doc(hidden)]
pub use generated::*;
