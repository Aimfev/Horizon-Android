//! Render hook points for native mods (docs/native-mods.md): empty functions the renderer calls at
//! fixed places of a frame, which a mod's mixins hook like those of
//! `horizon-client/src/game/native_hooks.rs`, and listed with them in `native-hooks.json`. API
//! 58.

use super::Vertex;
use super::pack_draw::EntitySpan;
use glam::{DVec3, Mat4};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

mod generated;
pub use generated::{
    HandDrawnEvent, RenderHooks, RenderNeedsEvent, WorldDrawnEvent, WorldWarpEvent,
};
pub use wgpu;

static RENDERERS: OnceLock<Box<[&'static dyn RenderHooks]>> = OnceLock::new();

/// Install trait mod renderers in dispatch order, once per process.
pub fn install(
    renderers: Box<[&'static dyn RenderHooks]>,
) -> Result<(), Box<[&'static dyn RenderHooks]>> {
    RENDERERS.set(renderers)
}

static MODS_BUILT_IN: AtomicBool = AtomicBool::new(false);

pub fn set_mods_built_in(on: bool) {
    MODS_BUILT_IN.store(on, Ordering::Relaxed);
}

pub fn world_drawn_hooked() -> bool {
    MODS_BUILT_IN.load(Ordering::Relaxed)
        || RENDERERS
            .get()
            .is_some_and(|renderers| !renderers.is_empty())
}

pub struct WorldDrawn<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub color: &'a wgpu::Texture,
    pub color_view: &'a wgpu::TextureView,
    pub color_format: wgpu::TextureFormat,
    pub size: [u32; 2],
    pub depth: &'a wgpu::TextureView,
    pub depth_size: [u32; 2],
    pub view_proj: Mat4,
    pub jittered: Mat4,
    pub camera: DVec3,
    pub entities: &'a [Vertex],
    pub spans: &'a [EntitySpan],
    pub atlas: Option<&'a wgpu::Texture>,
    pub quad_indices: &'a wgpu::Buffer,
}

#[non_exhaustive]
pub struct BlockSpriteArrays<'a> {
    pub views: &'a [wgpu::TextureView],
    pub split: u32,
    pub size: u32,
    pub nearest: &'a wgpu::Sampler,
    pub filtered: &'a wgpu::Sampler,
}

#[non_exhaustive]
pub struct HandDrawn<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub color: &'a wgpu::Texture,
    pub color_view: &'a wgpu::TextureView,
    pub color_format: wgpu::TextureFormat,
    pub size: [u32; 2],
    pub hand: &'a [Vertex],
    pub view_proj: Mat4,
    pub camera: DVec3,
    pub atlas: Option<&'a wgpu::Texture>,
    pub quad_indices: &'a wgpu::Buffer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderNeeds {
    pub entity_spans: bool,
    pub world_depth: bool,
}

#[inline(never)]
pub fn native_world_drawn(world: &mut WorldDrawn<'_>) {
    generated::dispatch_world_drawn(RENDERERS.get().map_or(&[], |renderers| renderers), world);
}

#[inline(never)]
pub fn native_hand_drawn(hand: &mut HandDrawn<'_>) {
    generated::dispatch_hand_drawn(RENDERERS.get().map_or(&[], |renderers| renderers), hand);
}

#[inline(never)]
pub fn native_render_needs(needs: &mut RenderNeeds) {
    generated::dispatch_render_needs(RENDERERS.get().map_or(&[], |renderers| renderers), needs);
}

pub fn render_needs() -> RenderNeeds {
    let mut needs = RenderNeeds::default();
    native_render_needs(&mut needs);
    needs
}

#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct WorldWarp {
    pub source: Option<Arc<str>>,
    pub on: bool,
    pub params: [f32; 16],
}

#[inline(never)]
pub fn native_world_warp(warp: &mut WorldWarp) {
    generated::dispatch_world_warp(RENDERERS.get().map_or(&[], |renderers| renderers), warp);
}

pub fn world_warp() -> WorldWarp {
    let mut warp = WorldWarp::default();
    native_world_warp(&mut warp);
    warp
}
