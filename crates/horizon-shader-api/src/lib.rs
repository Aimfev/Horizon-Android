//! Horizon's own shader packs (docs/api/shaders.md): WGSL written against
//! the renderer's stages, described by `horizon-shaders.toml` and shipped as
//! a folder or a `.hshader`. This crate reads and checks packs; the game, the
//! launcher and the registry all use it, so a pack is judged the same way
//! everywhere.
pub mod check;
pub mod inputs;
pub mod manifest;
pub mod package;
pub mod reference;
pub mod stages;

pub use check::{Problem, Report, check};
pub use inputs::Frame;
pub use manifest::{
    API, Api, Blend, LightColor, MANIFEST, Manifest, Material, OptionKind, OptionPage, PackOption,
    PackTexture, PageItem, SCHEMA, SpriteTag, Stage,
};
pub use package::{
    BUILTIN_SHADERS, EXTENSION, HORIZON_SHADER_STORE_LIBRARY, Pack, all_horizon_shader_packs,
    build_builtin_hshader_bytes, builtin_shader_pack,
};
pub use stages::{Composed, StageKind, compose};
