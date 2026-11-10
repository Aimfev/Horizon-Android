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
pub use crate::mobs::Mob;
pub use crate::particles::{ParticleOptions, ParticleType};
pub use crate::sound_engine::SoundPlayback;
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

pub use crate::gfx::native_render::{
    BlockSpriteArrays, HandDrawn, HandDrawnEvent, RenderHooks, RenderNeeds, RenderNeedsEvent,
    WorldDrawn, WorldDrawnEvent, WorldWarp, WorldWarpEvent,
};
pub use crate::mobs::model_layers::{
    CubeDefinition, CubeDeformation, CubeListBuilder, LayerDefinition, MeshDefinition, ModelPart,
    ModelPolygon, ModelVertex, PartId, PartPose,
};
pub use crate::spans::Span as TickSpan;
pub use crate::world_primitives::{PrimitiveShape, WorldPrimitive, WorldPrimitives};
pub use horizon_assets::item_model::StackView;
pub use horizon_protocol::entity_data::Value as EntityDataValue;
pub use horizon_protocol::packets::play::Clientbound as ClientboundPlayPacket;
pub use horizon_protocol::packets::play::Serverbound as ServerboundPlayPacket;
pub use horizon_world::block_state_properties::BlockProperties;
pub use horizon_world::level::Ticks;
pub use horizon_world::player_inventory::EquipmentSlot;
pub use std::ops::Range;
pub use std::path::{Path, PathBuf};
pub use winit::keyboard::KeyCode;

/// Natural mob spawn entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnEntry {
    pub kind: String,
    pub weight: u32,
    pub min_count: u32,
    pub max_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeConfigKind {
    Client,
    Common,
    Server,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NativeConfig {
    pub mod_id: String,
    pub kind: NativeConfigKind,
    pub file: String,
    pub default: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NativeConfigFile {
    pub mod_id: String,
    pub kind: NativeConfigKind,
    pub file: String,
    pub path: PathBuf,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NativeReloadListener {
    pub id: String,
    pub folder: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeTextField {
    pub value: String,
    pub cursor: usize,
    pub highlight: usize,
}

#[derive(Clone, Debug, Default)]
pub struct NativeWorldDraw {
    pub camera: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInventoryKind {
    Player,
    Ender,
    Cursor,
    Crafting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeInventoryError {
    InsidePlayerTurn,
    NoServer,
    LoggingIn,
    Dead,
    Online,
    NoWorld,
    NoSavedPlayer,
    HasFile,
    NotOnline,
    BothAway,
    SameInventory,
    StackChanged,
    NoRoom,
    WrongSize { want: usize, got: usize },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HudMode {
    #[default]
    AllVisible,
    NoHud,
    AllHidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NativeFluidAction {
    Simulate,
    Execute,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum NativeFluidDrain {
    Any(i32),
    Exact(NativeFluidStack),
}

#[derive(Clone, Copy, Debug)]
pub struct NativeNoteBlockChange<'a> {
    pub dimension: LevelDimension,
    pub pos: BlockPos,
    pub instrument: &'a str,
    pub old_note: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeLootOdds {
    pub item: String,
    pub components: String,
    pub chance: f64,
    pub min: u32,
    pub max: u32,
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NativeRecipeLookup {
    Recipes,
    Uses,
    Key(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeRecipeCategory {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: NativeStack,
    pub catalysts: Vec<NativeStack>,
    pub recipes: Vec<NativeCategoryRecipe>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeCategoryRecipe {
    pub inputs: Vec<Vec<NativeStack>>,
    pub outputs: Vec<NativeChanceOutput>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeChanceOutput {
    pub stack: NativeStack,
    pub chance: f32,
    pub min: u32,
    pub max: u32,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum NativePanelStyle {
    #[default]
    RecipeBook,
    Viewer,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeBossBar {
    pub name: Text,
    pub progress: f32,
    pub color: i32,
    pub overlay: i32,
    pub darken: bool,
    pub music: bool,
    pub fog: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct NativeLevelStats {
    pub chunks: usize,
    pub forced_chunks: usize,
    pub block_entities: usize,
    pub ticking_block_entities: usize,
    pub entities: Vec<(String, usize)>,
    pub block_entity_types: Vec<(String, usize)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct NativeMobCost {
    pub dimension: String,
    pub kind: String,
    pub nanos: u64,
    pub mob_ticks: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeFishingMedium {
    pub surface: f64,
    pub loot_table: Option<String>,
    pub lure_ticks: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LivingRef(EntityRef);

impl LivingRef {
    #[doc(hidden)]
    pub fn new(entity: EntityRef) -> Self {
        Self(entity)
    }
    pub fn entity(self) -> EntityRef {
        self.0
    }
    pub fn id(self) -> u32 {
        self.0.id()
    }
}

impl From<LivingRef> for EntityRef {
    fn from(living: LivingRef) -> Self {
        living.0
    }
}

pub type TaskCallback = Box<dyn FnOnce(&mut dyn Host) + Send + 'static>;

pub trait HostTaskExt: Host {
    fn spawn_task_with<T: Send + 'static>(
        &self,
        task: impl FnOnce() -> T + Send + 'static,
        callback: impl FnOnce(&mut dyn Host, T) + Send + 'static,
    ) {
        let _ = (task, callback);
    }
}

impl<H: Host + ?Sized> HostTaskExt for H {}

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
pub enum NativeAuthority {
    LocalAuthoritative,
    IntegratedServer,
    DedicatedServer,
    RemoteClient,
}

/// One open attempt of one world; it rejects a previous open's commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeWorldEpoch(pub u64);

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
pub struct NativeTickId(pub u64);

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
pub enum WorldPhase {
    WorldOpenReady,
    WorldOpenFailed,
    WorldCloseFinal,
    WorldCloseFailed,
}

/// The tick phases carry whether the world runs, including frozen steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerTickPhase {
    ServerTickStart { world_running: bool },
    ServerTickEnd { world_running: bool },
}

/// Context for every world lifecycle phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

static OFF: OnceLock<Box<[std::sync::atomic::AtomicBool]>> = OnceLock::new();

/// Names the entries of [`mods`] by their bundles' `[mod] id`s, so a panic in a hook names its mod.
pub fn name_mods(ids: &'static [&'static str]) {
    if MOD_IDS.set(ids).is_err() {
        log::warn!("Trait mods already named; ignoring the second list");
        return;
    }
    let _ = OFF.set(
        ids.iter()
            .map(|_| std::sync::atomic::AtomicBool::new(false))
            .collect(),
    );
}

/// Turns the trait mod named `id` ([`name_mods`]) on or off for the following dispatches.
pub fn set_mod_on(id: &str, on: bool) -> bool {
    let (Some(ids), Some(flags)) = (MOD_IDS.get(), OFF.get()) else {
        return false;
    };
    let mut found = false;
    for (flag, _) in flags.iter().zip(ids.iter()).filter(|(_, m)| **m == id) {
        flag.store(!on, std::sync::atomic::Ordering::Relaxed);
        found = true;
    }
    found
}

/// The ids [`name_mods`] gave the entries of [`mods`].
pub fn mod_ids() -> &'static [&'static str] {
    MOD_IDS.get().copied().unwrap_or(&[])
}

fn is_off(index: usize) -> bool {
    OFF.get()
        .and_then(|flags| flags.get(index))
        .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
}

fn place_in<T>(all: &[T], part: &[T]) -> Option<usize> {
    let range = all.as_ptr_range();
    range
        .contains(&part.as_ptr())
        .then(|| (part.as_ptr().addr() - range.start.addr()) / size_of::<T>().max(1))
}

fn entered<'a>(mods: &'a [Box<dyn Hooks>], hook: &'static str) -> Entered<'a> {
    Entered {
        first: place_in(self::mods(), mods),
        mods: mods.iter().enumerate(),
        hook,
        guard: None,
    }
}

struct Entered<'a> {
    first: Option<usize>,
    mods: std::iter::Enumerate<std::slice::Iter<'a, Box<dyn Hooks>>>,
    hook: &'static str,
    guard: Option<crate::crash::NativeModGuard>,
}

impl<'a> Iterator for Entered<'a> {
    type Item = &'a Box<dyn Hooks>;

    fn next(&mut self) -> Option<Self::Item> {
        drop(self.guard.take());
        let (index, hooks) = loop {
            let (index, hooks) = self.mods.next()?;
            match self.first {
                Some(first) if is_off(first + index) => continue,
                Some(first) => break (first + index, hooks),
                None => break (index, hooks),
            }
        };
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

/// Installs trait mods and registers all client-library, render, and data-load hook callbacks.
pub fn install_runtime_hooks(mods: Mods) {
    if !mods.is_empty() || MODS.get().is_none() {
        let _ = MODS.set(mods);
    }
    let renderers = self::mods()
        .iter()
        .filter_map(|m| m.render())
        .collect();
    let _ = crate::gfx::native_render::install(renderers);
    let _ = horizon_world::native_load_hooks::install(
        horizon_world::native_load_hooks::NativeLoadHooks {
            loot_table_load: |id, table| {
                dispatch_loot_table_load(self::mods(), id, table);
            },
            modify_default_components: |item, components| {
                dispatch_modify_default_components(self::mods(), item, components);
            },
        },
    );
    let _ = crate::native_client_hooks::install(crate::native_client_hooks::NativeClientHooks {
        menu_slot_rules: |menu, slot, stack, may_place, max_stack| {
            dispatch_menu_slot_rules(self::mods(), menu, slot, stack, may_place, max_stack);
        },
        player_chat: |player, name, message, replies, cancel| {
            dispatch_player_chat(self::mods(), player, name, message, replies, cancel);
        },
        player_chat_v2: |player, name, level, message, replies, cancel| {
            dispatch_player_chat_v2(self::mods(), player, name, level, message, replies, cancel);
        },
        chat_decorate: |player, name, message, decorated| {
            dispatch_chat_decorate(self::mods(), player, name, message, decorated);
        },
        sound_play: |sound, volume, pitch, cancel| {
            dispatch_sound_play(self::mods(), sound, volume, pitch, cancel);
        },
        living_heal: |entity, amount, cancel| {
            dispatch_living_heal(self::mods(), entity, amount, cancel);
        },
        living_breathe: |entity, kind, can_breathe, consume, refill| {
            dispatch_living_breathe(self::mods(), entity, kind, can_breathe, consume, refill);
        },
        living_drown: |entity, kind, is_drowning, damage_amount, bubble_count, cancel| {
            dispatch_living_drown(
                self::mods(),
                entity,
                kind,
                is_drowning,
                damage_amount,
                bubble_count,
                cancel,
            );
        },
        living_fall: |entity, kind, distance, multiplier, cancel| {
            dispatch_living_fall(self::mods(), entity, kind, distance, multiplier, cancel);
        },
        living_jump: |entity, kind| {
            dispatch_living_jump(self::mods(), entity, kind);
        },
        register_goals: |mob| {
            dispatch_register_goals(self::mods(), mob);
        },
        player_pre_login: |player, name, address, defer| {
            dispatch_player_pre_login(self::mods(), player, name, address, defer);
        },
        player_profile: |player, name, properties| {
            dispatch_player_profile(self::mods(), player, name, properties);
        },
        ingredient: |kind, data, items| {
            dispatch_ingredient(self::mods(), kind, data, items);
        },
        ingredient_test: |kind, data, stack, accept| {
            dispatch_ingredient_test(self::mods(), kind, data, stack, accept);
        },
        log_line: |level, thread, message, line| {
            dispatch_log_line(self::mods(), level, thread, message, line);
        },
        handshake: |address, host, intent, cancel, reason| {
            dispatch_handshake(self::mods(), address, host, intent, reason, cancel);
        },
    });
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

/// How far a chunk a mod asked for with `native_chunk_load_request` is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeChunkLoad {
    Pending,
    Loaded,
    Failed,
}

/// An owned fluid payload (amount in millibuckets).
#[derive(Clone, Debug, PartialEq)]
pub struct NativeFluidStack {
    pub fluid: String,
    pub amount: i32,
    pub components: Compound,
}

/// A tank exposed by a block's fluid capability, in provider order.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeFluidTankView {
    pub tank: usize,
    pub stack: Option<NativeFluidStack>,
    pub capacity: i32,
}

/// What a mod asks of the terrain's picture each frame (`native_world_hold`). API 414.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorldHold {
    #[default]
    None,
    Hold,
    Release,
}

// World-space vertex and fragment warp shader hook (`native_world_warp`). API 415.
// Re-exported definition: pub struct WorldWarp in crate::gfx::native_render.
