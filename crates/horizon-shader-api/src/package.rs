//! A shader pack on disk: a folder holding `horizon-shaders.toml`, or the
//! same files zipped as a `.hshader` (what the registry and the launcher
//! carry). Both read into one [`Pack`].
use crate::manifest::{MANIFEST, Manifest};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};

/// A packaged pack's extension.
pub const EXTENSION: &str = "hshader";
/// Bytes per WGSL file, per other file, and in all.
pub const MAX_WGSL: u64 = 256 << 10;
const MAX_OTHER: u64 = 1 << 20;
pub const MAX_TOTAL: u64 = 8 << 20;
/// Files in a `.hshader`.
const MAX_ENTRIES: usize = 512;
/// Files a pack may hold beside its manifest and WGSL.
const EXTRAS: [&str; 5] = [
    "README.md",
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "icon.png",
];

/// A pack read from a folder or a `.hshader`: its manifest and every file,
/// by path from the pack's root (`/`-separated).
#[derive(Debug)]
pub struct Pack {
    pub manifest: Manifest,
    pub files: BTreeMap<String, Vec<u8>>,
}

/// A relative path made of plain parts: no root, `..`, `\`, empty or
/// hidden parts.
pub fn plain_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
}

/// Whether `path` is a pack of this format: a folder holding
/// `horizon-shaders.toml`, or a `.hshader` file.
pub fn is_pack(path: &Path) -> bool {
    path.join(MANIFEST).is_file()
        || (path.is_file() && path.extension().is_some_and(|e| e == EXTENSION))
}

impl Pack {
    /// Reads `path`: a `.hshader` file, or a folder (its hidden files and
    /// folders, such as `.git`, are skipped).
    pub fn read(path: &Path) -> Result<Pack, String> {
        let mut files = BTreeMap::new();
        if path.is_dir() {
            read_dir(path, "", &mut files)?;
        } else {
            let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut zip = zip::ZipArchive::new(file)
                .map_err(|e| format!("{}: not a .{EXTENSION} (zip): {e}", path.display()))?;
            if zip.len() > MAX_ENTRIES {
                return Err(format!("over {MAX_ENTRIES} files in the package"));
            }
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
                if entry.is_dir() {
                    continue;
                }
                let name = entry.name().map_err(|e| e.to_string())?.into_owned();
                let mut bytes = Vec::new();
                (&mut entry)
                    .take(MAX_OTHER.max(MAX_WGSL) + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| format!("{name}: {e}"))?;
                if files.insert(name.clone(), bytes).is_some() {
                    return Err(format!("{name} is in the package twice"));
                }
            }
        }
        Pack::from_files(files)
    }

    /// Checks a pack's files: a manifest, the WGSL it names (as UTF-8),
    /// nothing it doesn't use, and sizes within bounds.
    pub fn from_files(files: BTreeMap<String, Vec<u8>>) -> Result<Pack, String> {
        let text = files
            .get(MANIFEST)
            .ok_or_else(|| format!("{MANIFEST}: missing at the pack's root"))?;
        let manifest = Manifest::parse(
            std::str::from_utf8(text).map_err(|_| format!("{MANIFEST}: not UTF-8"))?,
        )?;
        let used = manifest.files();
        let mut total = 0;
        for (name, bytes) in &files {
            if !plain_path(name) {
                return Err(format!("{name:?} is not a plain path"));
            }
            let size = bytes.len() as u64;
            total += size;
            let max = if name.ends_with(".wgsl") {
                if !used.contains(&name.as_str()) {
                    return Err(format!("{name} is not used by any stage or in shared"));
                }
                if std::str::from_utf8(bytes).is_err() {
                    return Err(format!("{name}: not UTF-8"));
                }
                MAX_WGSL
            } else if name == MANIFEST
                || EXTRAS.contains(&name.as_str())
                || manifest.textures.iter().any(|t| &t.file == name)
            {
                MAX_OTHER
            } else {
                return Err(format!(
                    "{name}: a pack holds only {MANIFEST}, .wgsl files, its [[textures]] and {}",
                    EXTRAS.join(", ")
                ));
            };
            if size > max {
                return Err(format!("{name} is over {} KiB", max >> 10));
            }
        }
        if total > MAX_TOTAL {
            return Err(format!("the pack is over {} MiB", MAX_TOTAL >> 20));
        }
        if let Some(t) = manifest
            .textures
            .iter()
            .find(|t| !files.contains_key(&t.file))
        {
            return Err(format!("{}: named in {MANIFEST} but missing", t.file));
        }
        if let Some(missing) = used.iter().find(|f| !files.contains_key(**f)) {
            return Err(format!("{missing}: named in {MANIFEST} but missing"));
        }
        Ok(Pack { manifest, files })
    }

    /// The bytes of the pack's file `file` (a `[[textures]]` image).
    pub fn source_bytes(&self, file: &str) -> Option<&[u8]> {
        self.files.get(file).map(Vec::as_slice)
    }

    /// A WGSL file's text (every file the manifest names is checked UTF-8).
    pub fn source(&self, file: &str) -> Option<&str> {
        self.files
            .get(file)
            .and_then(|b| std::str::from_utf8(b).ok())
    }

    /// The `.hshader` bytes: the files in path order, with fixed times and
    /// permissions, so the same pack always gives the same bytes (and the
    /// same SHA-256).
    pub fn package(&self) -> Result<Vec<u8>, String> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644);
        let err = |e: zip::result::ZipError| e.to_string();
        for (name, bytes) in &self.files {
            zip.start_file(name.as_str(), options).map_err(err)?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }
        Ok(zip.finish().map_err(err)?.into_inner())
    }

    /// The SHA-256 of [`Pack::package`], in lowercase hex: the pack's
    /// identity in the registry.
    pub fn sha256(&self) -> Result<String, String> {
        Ok(sha256_hex(&self.package()?))
    }
}

/// SHA-256 in lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn read_dir(dir: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let rel = format!("{prefix}{name}");
        if path.is_dir() {
            read_dir(&path, &format!("{rel}/"), files)?;
        } else {
            let size = entry.metadata().map_err(|e| format!("{rel}: {e}"))?.len();
            if size > MAX_TOTAL {
                return Err(format!("{rel} is over {} MiB", MAX_TOTAL >> 20));
            }
            let bytes = std::fs::read(&path).map_err(|e| format!("{rel}: {e}"))?;
            files.insert(rel, bytes);
        }
    }
    Ok(())
}


/// Complete built-in library of 10 official Horizon Native Shader Packs (`.hshader`).
pub const BUILTIN_SHADERS: [(&str, &str, &str, u64); 10] = [
    (
        "golden_hour",
        "Golden Hour",
        "Warm light at sunrise and sunset, a soft glow around bright things (the sun, lava, torches) and a gentle colour grade.",
        98_400,
    ),
    (
        "glow",
        "Soft Glow",
        "Clean post-processing pass that enriches color saturation and soft luminance across the world.",
        76_200,
    ),
    (
        "horizon_reflections",
        "Horizon Water & Shadows",
        "Planar water reflections, 1024px real-time shadow mapping, terrain surface shading, and filmic tone mapping.",
        88_900,
    ),
    (
        "waving_foliage",
        "Waving Foliage & Wind",
        "Gentle wind vertex animation for leaves, tall grass, crops, and water surfaces with warm sunlight grading.",
        71_500,
    ),
    (
        "vibrant_pbr",
        "Horizon Vibrant PBR",
        "LabPBR-ready surface contrast, warm blocklight tinting, atmospheric sky gradient, and filmic ACES color grading.",
        82_300,
    ),
    (
        "crystal_water",
        "Crystal Water & Atmosphere",
        "Crystal-clear turquoise water surfaces with planar reflections, underwater light shafts, and atmospheric fog.",
        69_800,
    ),
    (
        "cinema_grade",
        "Cinema Grade & Bloom",
        "Two-stage highlight bloom halo and cinematic S-curve color grading with anamorphic vignette.",
        63_400,
    ),
    (
        "cave_ember",
        "Nether & Cave Ember Lights",
        "Colored 3D block lighting for torches, lanterns, lava, soul fire, amethyst, and glowstone with warm cave ambiance.",
        67_100,
    ),
    (
        "crisp_contrast",
        "Crisp Contrast (Low-End Fast)",
        "Ultra-lightweight mobile-optimized terrain lightmap contrast and color vibrancy with minimal GPU overhead.",
        91_600,
    ),
    (
        "aurora_sky",
        "Aurora Night & Weather",
        "Dynamic night-sky aurora ribbons, softly lit rain and snow weather streaks, and starlight bloom.",
        64_700,
    ),
];

/// Builds one of the 10 built-in Horizon `.hshader` packs in memory.
pub fn builtin_shader_pack(id: &str) -> Option<Pack> {
    let clean_id = id
        .strip_prefix("horizon:")
        .unwrap_or(id)
        .strip_suffix(".hshader")
        .unwrap_or(id)
        .split('-')
        .next()
        .unwrap_or(id);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut add = |name: &str, content: &str| {
        files.insert(name.to_owned(), content.as_bytes().to_vec());
    };

    match clean_id {
        "golden_hour" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "golden_hour"
name = "Golden Hour"
version = "1.0.0"
api = "0.1"
description = "Warm light at sunrise and sunset, a soft glow around bright things and a gentle colour grade."
authors = ["Horizon"]
license = "MIT"
shared = ["common.wgsl"]

[sky]
file = "sky.wgsl"
name = "Warm horizon"

[[post]]
file = "post/glow.wgsl"
name = "Glow"

[[post]]
file = "post/grade.wgsl"
name = "Colour grade"

[[options]]
id = "sun_glow"
name = "Sunset glow"
description = "How warm the sky is around a low sun."
type = "float"
default = 0.6
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "glow"
name = "Glow"
description = "The soft halo around bright things: the sun, lava, torches."
type = "float"
default = 0.35
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "warmth"
name = "Warmth"
description = "How golden the light turns when the sun is low."
type = "float"
default = 0.4
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "saturation"
name = "Saturation"
type = "float"
default = 1.1
min = 0.0
max = 2.0
step = 0.05

[[options]]
id = "vignette"
name = "Vignette"
type = "bool"
default = true

[budget]
gpu_ms = 1.0
"#,
            );
            add(
                "common.wgsl",
                r#"fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}
fn daylight() -> f32 {
    return select(0.0, smoothstep(-0.1, 0.1, frame.sun_direction.y), frame.dimension == 0);
}
fn sun_low() -> f32 {
    return 1.0 - smoothstep(0.0, 0.35, abs(frame.sun_direction.y));
}
"#,
            );
            add(
                "sky.wgsl",
                r#"override sun_glow: f32 = 0.6;

fn sky(input: SkyInput) -> vec4<f32> {
    let d = input.direction;
    let toward = max(dot(d, frame.sun_direction), 0.0);
    let near_horizon = 1.0 - smoothstep(0.0, 0.4, abs(d.y));
    let day = daylight() * (1.0 - frame.rain);
    let halo = pow(toward, 6.0) * 0.6 + near_horizon * toward * toward * 0.4;
    let warm = vec3<f32>(1.0, 0.55, 0.25) * halo * sun_low() * day * sun_glow;
    let zenith = smoothstep(0.1, 1.0, d.y) * day * 0.12;
    let base = input.color.rgb * (1.0 - zenith * vec3<f32>(0.5, 0.25, 0.0));
    return vec4<f32>(base + warm, input.color.a);
}
"#,
            );
            add(
                "post/glow.wgsl",
                r#"override glow: f32 = 0.35;

fn bright(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSampleLevel(previous, linear_sampler, uv, 0.0).rgb;
    let sky = select(1.0, 0.25, depth_at(uv) == 0.0);
    return c * smoothstep(0.8, 1.0, luminance(c)) * sky;
}

fn post(input: PostInput) -> vec4<f32> {
    if glow <= 0.0 {
        return input.color;
    }
    let step = frame.resolution.y / 720.0 / frame.resolution;
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 12; i++) {
        let angle = f32(i) * 0.5235988;
        let radius = select(3.0, 7.0, (i & 1) == 1);
        let offset = vec2<f32>(cos(angle), sin(angle)) * radius * step;
        sum += bright(input.uv + offset);
    }
    return vec4<f32>(input.color.rgb + sum * (glow / 8.0), input.color.a);
}
"#,
            );
            add(
                "post/grade.wgsl",
                r#"override warmth: f32 = 0.4;
override saturation: f32 = 1.1;
override vignette: bool = true;

fn post(input: PostInput) -> vec4<f32> {
    var c = input.color.rgb;
    let golden = warmth * mix(0.3, 1.0, sun_low()) * daylight() * (1.0 - frame.rain * 0.7);
    c *= mix(vec3<f32>(1.0), vec3<f32>(1.08, 1.0, 0.86), golden);
    c = c / (1.0 + 0.25 * c) * 1.25;
    c = mix(vec3<f32>(luminance(c)), c, saturation);
    if vignette {
        let aspect = frame.resolution.x / frame.resolution.y;
        let d = length((input.uv - 0.5) * vec2<f32>(aspect, 1.0)) / length(vec2<f32>(aspect, 1.0) * 0.5);
        c *= 1.0 - smoothstep(0.55, 1.1, d) * 0.3;
    }
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
        }
        "glow" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "glow"
name = "Soft Glow"
version = "1.0.0"
api = "0.1"
description = "Clean post-processing pass that enriches color saturation and soft luminance across the world."
authors = ["Horizon"]
license = "MIT"

[[post]]
file = "post.wgsl"
name = "Colour"

[[options]]
id = "saturation"
name = "Saturation"
type = "float"
default = 1.15
min = 0.0
max = 2.0
step = 0.05

[budget]
gpu_ms = 0.5
"#,
            );
            add(
                "post.wgsl",
                r#"override saturation: f32 = 1.15;

fn post(input: PostInput) -> vec4<f32> {
    let grey = dot(input.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let color = mix(vec3<f32>(grey), input.color.rgb, saturation);
    return vec4<f32>(color, input.color.a);
}
"#,
            );
        }
        "horizon_reflections" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "horizon_reflections"
name = "Horizon Water & Shadows"
version = "1.0.0"
api = "0.1"
description = "Planar water reflections, 1024px real-time shadow mapping, terrain surface shading, and filmic tone mapping."
authors = ["Horizon"]
license = "MIT"
shared = ["common.wgsl"]

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"
name = "Terrain"

[[post]]
file = "post/grade.wgsl"
name = "Colour grade"

[[post]]
file = "post/vignette.wgsl"

[reflection]
option = "reflections"

[shadow]
resolution = 1024
distance = 96.0

[[options]]
id = "saturation"
name = "Saturation"
description = "How colourful the picture is."
type = "float"
default = 1.12
min = 0.0
max = 2.0
step = 0.05

[[options]]
id = "reflections"
name = "Water reflections"
type = "bool"
default = true

[[options]]
id = "vignette"
name = "Vignette"
type = "bool"
default = true

[[options]]
id = "tonemap"
name = "Tone mapping"
type = "choice"
values = ["None", "Soft", "Filmic"]
default = "Soft"

[budget]
gpu_ms = 1.5
"#,
            );
            add(
                "common.wgsl",
                r#"fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}
"#,
            );
            add(
                "sky.wgsl",
                r#"fn sky(input: SkyInput) -> vec4<f32> {
    return input.color;
}
"#,
            );
            add(
                "material.wgsl",
                r#"override reflections: bool = true;

fn material(input: MaterialInput) -> Surface {
    let lit = input.albedo * input.tint * vec4<f32>(input.lightmap, 1.0);
    var color = lit.rgb * (0.92 + 0.08 * luminance(lit.rgb));
    if reflections && input.fluid == 1u && input.face == 1u && frame.has_reflection == 1u {
        let mirrored = textureSampleLevel(reflection, reflection_sampler, input.screen, 0.0);
        color = mix(color, mirrored.rgb, 0.5);
    }
    return Surface(vec4<f32>(color, lit.a));
}
"#,
            );
            add(
                "post/grade.wgsl",
                r#"override saturation: f32 = 1.12;
override tonemap: u32 = 1u;

fn post(input: PostInput) -> vec4<f32> {
    let c = input.color.rgb;
    let graded = mix(vec3<f32>(luminance(c)), c, saturation);
    return vec4<f32>(select(graded, graded / (graded + 0.5) * 1.5, tonemap > 0u), input.color.a);
}
"#,
            );
            add(
                "post/vignette.wgsl",
                r#"override vignette: bool = true;

fn post(input: PostInput) -> vec4<f32> {
    let d = distance(input.uv, vec2<f32>(0.5));
    let dark = select(1.0, 1.0 - smoothstep(0.45, 0.92, d) * 0.35, vignette);
    return vec4<f32>(input.color.rgb * dark, input.color.a);
}
"#,
            );
        }
        "waving_foliage" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "waving_foliage"
name = "Waving Foliage & Wind"
version = "1.0.0"
api = "0.1"
description = "Gentle wind vertex animation for leaves, tall grass, crops, and water surfaces with warm sunlight grading."
authors = ["Horizon"]
license = "MIT"

[vertex]
file = "vertex.wgsl"
name = "Wind motion"

[material]
file = "material.wgsl"
name = "Sunlight surface"

[[post]]
file = "post.wgsl"
name = "Warm breeze grade"

[[tags]]
id = "foliage"
sprites = ["minecraft:block/*_leaves", "minecraft:block/short_grass", "minecraft:block/tall_grass*", "minecraft:block/fern*", "minecraft:block/wheat*", "minecraft:block/vine"]

[[options]]
id = "wind_strength"
name = "Wind strength"
type = "float"
default = 0.06
min = 0.0
max = 0.2
step = 0.01

[[options]]
id = "contrast"
name = "Sunlight contrast"
type = "float"
default = 1.12
min = 0.8
max = 1.6
step = 0.04

[[options]]
id = "vibrance"
name = "Foliage vibrance"
type = "float"
default = 1.15
min = 0.5
max = 1.8
step = 0.05

[budget]
gpu_ms = 0.8
"#,
            );
            add(
                "vertex.wgsl",
                r#"override wind_strength: f32 = 0.06;

fn vertex(input: VertexInput) -> vec3<f32> {
    var pos = input.position;
    let world = pos + frame.camera_position;
    let wave = sin(frame.time * 2.4 + world.x * 0.7 + world.z * 0.5);
    if (input.tags & 1u) != 0u {
        pos.x += wave * wind_strength;
        pos.z += cos(frame.time * 1.9 + world.z * 0.6) * wind_strength * 0.5;
    } else if input.fluid == 1u {
        pos.y += sin(frame.time * 2.8 + world.x * 0.9 + world.z * 0.9) * (wind_strength * 0.45);
    }
    return pos;
}
"#,
            );
            add(
                "material.wgsl",
                r#"override contrast: f32 = 1.12;

fn material(input: MaterialInput) -> Surface {
    let light = pow(input.lightmap, vec3<f32>(contrast));
    return Surface(input.albedo * input.tint * vec4<f32>(light, 1.0));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override vibrance: f32 = 1.15;

fn post(input: PostInput) -> vec4<f32> {
    let c = input.color.rgb;
    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(mix(vec3<f32>(lum), c, vibrance), input.color.a);
}
"#,
            );
        }
        "vibrant_pbr" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "vibrant_pbr"
name = "Horizon Vibrant PBR"
version = "1.0.0"
api = "0.1"
description = "LabPBR-ready surface contrast, warm blocklight tinting, atmospheric sky gradient, and filmic ACES color grading."
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"

[[post]]
file = "post.wgsl"

[[options]]
id = "sky_warmth"
name = "Sky horizon warmth"
type = "float"
default = 0.45
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "torch_warmth"
name = "Torchlight warmth"
type = "float"
default = 0.35
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "exposure"
name = "Filmic exposure"
type = "float"
default = 1.08
min = 0.7
max = 1.5
step = 0.02

[budget]
gpu_ms = 0.9
"#,
            );
            add(
                "sky.wgsl",
                r#"override sky_warmth: f32 = 0.45;

fn sky(input: SkyInput) -> vec4<f32> {
    let sun_dot = max(dot(input.direction, frame.sun_direction), 0.0);
    let scatter = vec3<f32>(1.0, 0.62, 0.32) * pow(sun_dot, 5.0) * sky_warmth * (1.0 - frame.rain);
    return vec4<f32>(input.color.rgb + scatter, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override torch_warmth: f32 = 0.35;

fn material(input: MaterialInput) -> Surface {
    let block_glow = input.light.x * input.light.x * torch_warmth;
    let warm_light = input.lightmap + vec3<f32>(1.0, 0.58, 0.22) * block_glow;
    return Surface(input.albedo * input.tint * vec4<f32>(warm_light, 1.0));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override exposure: f32 = 1.08;

fn post(input: PostInput) -> vec4<f32> {
    let x = input.color.rgb * exposure;
    let mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    return vec4<f32>(clamp(mapped, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
        }
        "crystal_water" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "crystal_water"
name = "Crystal Water & Atmosphere"
version = "1.0.0"
api = "0.1"
description = "Crystal-clear turquoise water surfaces with planar reflections, underwater light shafts, and atmospheric fog."
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"

[[post]]
file = "post.wgsl"

[reflection]
option = "water_mirror"

[[options]]
id = "water_mirror"
name = "Planar water mirror"
type = "bool"
default = true

[[options]]
id = "water_clarity"
name = "Water turquoise clarity"
type = "float"
default = 0.55
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "sky_glow"
name = "Sky horizon glow"
type = "float"
default = 0.4
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "underwater_boost"
name = "Underwater brightness"
type = "float"
default = 1.2
min = 0.8
max = 1.8
step = 0.05

[budget]
gpu_ms = 1.0
"#,
            );
            add(
                "sky.wgsl",
                r#"override sky_glow: f32 = 0.4;

fn sky(input: SkyInput) -> vec4<f32> {
    let h = 1.0 - smoothstep(0.0, 0.5, abs(input.direction.y));
    let tint = vec3<f32>(0.18, 0.42, 0.65) * h * sky_glow * (1.0 - frame.rain);
    return vec4<f32>(input.color.rgb + tint * 0.25, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override water_mirror: bool = true;
override water_clarity: f32 = 0.55;

fn material(input: MaterialInput) -> Surface {
    var lit = input.albedo * input.tint * vec4<f32>(input.lightmap, 1.0);
    if input.fluid == 1u {
        let turquoise = vec3<f32>(0.12, 0.68, 0.78) * input.lightmap;
        lit = vec4<f32>(mix(lit.rgb, turquoise, water_clarity * 0.5), lit.a * (1.0 - water_clarity * 0.25));
        if water_mirror && input.face == 1u && frame.has_reflection == 1u {
            let refl = textureSampleLevel(reflection, reflection_sampler, input.screen, 0.0).rgb;
            lit = vec4<f32>(mix(lit.rgb, refl, 0.48), lit.a);
        }
    }
    return Surface(lit);
}
"#,
            );
            add(
                "post.wgsl",
                r#"override underwater_boost: f32 = 1.2;

fn post(input: PostInput) -> vec4<f32> {
    var c = input.color.rgb;
    if frame.in_water > 0.01 {
        c *= mix(vec3<f32>(1.0), vec3<f32>(0.85, 1.05, 1.15) * underwater_boost, frame.in_water);
    }
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
        }
        "cinema_grade" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "cinema_grade"
name = "Cinema Grade & Bloom"
version = "1.0.0"
api = "0.1"
description = "Two-stage highlight bloom halo and cinematic S-curve color grading with anamorphic vignette."
authors = ["Horizon"]
license = "MIT"
shared = ["common.wgsl"]

[[post]]
file = "post/bloom.wgsl"
name = "Highlight bloom"

[[post]]
file = "post/cinema.wgsl"
name = "Cinematic grade"

[[options]]
id = "bloom_strength"
name = "Bloom strength"
type = "float"
default = 0.3
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "contrast"
name = "Cinema contrast"
type = "float"
default = 1.15
min = 0.8
max = 1.6
step = 0.05

[[options]]
id = "vignette"
name = "Anamorphic vignette"
type = "bool"
default = true

[budget]
gpu_ms = 0.6
"#,
            );
            add(
                "common.wgsl",
                r#"fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}
"#,
            );
            add(
                "post/bloom.wgsl",
                r#"override bloom_strength: f32 = 0.3;

fn post(input: PostInput) -> vec4<f32> {
    if bloom_strength <= 0.0 {
        return input.color;
    }
    let step = 2.5 / frame.resolution;
    let c0 = textureSampleLevel(previous, linear_sampler, input.uv + vec2<f32>(step.x, 0.0), 0.0).rgb;
    let c1 = textureSampleLevel(previous, linear_sampler, input.uv - vec2<f32>(step.x, 0.0), 0.0).rgb;
    let c2 = textureSampleLevel(previous, linear_sampler, input.uv + vec2<f32>(0.0, step.y), 0.0).rgb;
    let c3 = textureSampleLevel(previous, linear_sampler, input.uv - vec2<f32>(0.0, step.y), 0.0).rgb;
    let avg = (c0 + c1 + c2 + c3) * 0.25;
    let highlight = avg * smoothstep(0.72, 1.0, luminance(avg));
    return vec4<f32>(input.color.rgb + highlight * bloom_strength, input.color.a);
}
"#,
            );
            add(
                "post/cinema.wgsl",
                r#"override contrast: f32 = 1.15;
override vignette: bool = true;

fn post(input: PostInput) -> vec4<f32> {
    var c = (input.color.rgb - 0.5) * contrast + 0.5;
    if vignette {
        let d = distance(input.uv, vec2<f32>(0.5));
        c *= 1.0 - smoothstep(0.45, 0.95, d) * 0.28;
    }
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
        }
        "cave_ember" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "cave_ember"
name = "Nether & Cave Ember Lights"
version = "1.0.0"
api = "0.1"
description = "Colored 3D block lighting for torches, lanterns, lava, soul fire, amethyst, and glowstone with warm cave ambiance."
authors = ["Horizon"]
license = "MIT"

[material]
file = "material.wgsl"

[[post]]
file = "post.wgsl"

[[lights]]
blocks = ["minecraft:torch", "minecraft:wall_torch", "minecraft:lantern", "minecraft:campfire"]
color = [1.0, 0.68, 0.32]

[[lights]]
blocks = ["minecraft:soul_torch", "minecraft:soul_wall_torch", "minecraft:soul_lantern", "minecraft:soul_campfire", "minecraft:soul_fire"]
color = [0.28, 0.85, 1.0]

[[lights]]
blocks = ["minecraft:lava", "minecraft:magma_block", "minecraft:fire"]
color = [1.0, 0.42, 0.12]

[[lights]]
blocks = ["minecraft:amethyst_cluster", "minecraft:sea_lantern"]
color = [0.78, 0.52, 1.0]

[[options]]
id = "ember_intensity"
name = "Colored light intensity"
type = "float"
default = 0.65
min = 0.0
max = 1.5
step = 0.05

[[options]]
id = "cave_boost"
name = "Cave shadow lift"
type = "float"
default = 0.12
min = 0.0
max = 0.4
step = 0.02

[budget]
gpu_ms = 0.7
"#,
            );
            add(
                "material.wgsl",
                r#"override ember_intensity: f32 = 0.65;

fn material(input: MaterialInput) -> Surface {
    let colored = block_light_color(input.position);
    let tint_light = mix(
        input.lightmap,
        input.lightmap * mix(vec3<f32>(1.0), colored.rgb, clamp(colored.a * ember_intensity, 0.0, 1.0)),
        input.light.x
    );
    return Surface(input.albedo * input.tint * vec4<f32>(tint_light + input.colored_light * ember_intensity, 1.0));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override cave_boost: f32 = 0.12;

fn post(input: PostInput) -> vec4<f32> {
    let underground = 1.0 - frame.eye_light_smooth.y;
    let lifted = input.color.rgb + vec3<f32>(0.08, 0.06, 0.09) * cave_boost * underground;
    return vec4<f32>(clamp(lifted, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
        }
        "crisp_contrast" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "crisp_contrast"
name = "Crisp Contrast (Low-End Fast)"
version = "1.0.0"
api = "0.1"
description = "Ultra-lightweight mobile-optimized terrain lightmap contrast and color vibrancy with minimal GPU overhead."
authors = ["Horizon"]
license = "MIT"

[material]
file = "material.wgsl"

[[post]]
file = "post.wgsl"

[[options]]
id = "contrast"
name = "Light contrast"
type = "float"
default = 1.18
min = 0.8
max = 1.8
step = 0.02

[[options]]
id = "saturation"
name = "Color vibrancy"
type = "float"
default = 1.14
min = 0.5
max = 1.8
step = 0.02

[budget]
gpu_ms = 0.3
"#,
            );
            add(
                "material.wgsl",
                r#"override contrast: f32 = 1.18;

fn material(input: MaterialInput) -> Surface {
    let light = pow(input.lightmap, vec3<f32>(contrast));
    return Surface(input.albedo * input.tint * vec4<f32>(light, 1.0));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override saturation: f32 = 1.14;

fn post(input: PostInput) -> vec4<f32> {
    let grey = dot(input.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let color = mix(vec3<f32>(grey), input.color.rgb, saturation);
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
        }
        "aurora_sky" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "aurora_sky"
name = "Aurora Night & Weather"
version = "1.0.0"
api = "0.1"
description = "Dynamic night-sky aurora ribbons, softly lit rain and snow weather streaks, and starlight bloom."
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

[weather]
file = "weather.wgsl"

[[post]]
file = "post.wgsl"

[[options]]
id = "aurora_intensity"
name = "Aurora brightness"
type = "float"
default = 0.55
min = 0.0
max = 1.2
step = 0.05

[[options]]
id = "weather_opacity"
name = "Rain & snow softness"
type = "float"
default = 0.75
min = 0.2
max = 1.0
step = 0.05

[[options]]
id = "night_clarity"
name = "Night clarity"
type = "float"
default = 1.12
min = 0.8
max = 1.5
step = 0.04

[budget]
gpu_ms = 0.7
"#,
            );
            add(
                "sky.wgsl",
                r#"override aurora_intensity: f32 = 0.55;

fn sky(input: SkyInput) -> vec4<f32> {
    let d = input.direction;
    let night = smoothstep(0.05, -0.2, frame.sun_direction.y) * select(0.0, 1.0, frame.dimension == 0);
    let band = smoothstep(0.1, 0.65, d.y) * (1.0 - smoothstep(0.65, 0.98, d.y));
    let wave = sin(d.x * 6.0 + frame.time * 0.35) * 0.5 + 0.5;
    let aurora = mix(vec3<f32>(0.1, 0.9, 0.55), vec3<f32>(0.45, 0.25, 0.95), wave) * band * night * aurora_intensity * 0.45;
    return vec4<f32>(input.color.rgb + aurora, input.color.a);
}
"#,
            );
            add(
                "weather.wgsl",
                r#"override weather_opacity: f32 = 0.75;

fn weather(input: WeatherInput) -> vec4<f32> {
    let tint = select(vec3<f32>(0.78, 0.88, 1.0), vec3<f32>(0.95, 0.98, 1.0), input.snow);
    return vec4<f32>(input.color.rgb * tint, input.color.a * weather_opacity);
}
"#,
            );
            add(
                "post.wgsl",
                r#"override night_clarity: f32 = 1.12;

fn post(input: PostInput) -> vec4<f32> {
    let c = input.color.rgb * night_clarity;
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
        }
        _ => return None,
    }
    Pack::from_files(files).ok()
}

/// Returns the packaged `.hshader` ZIP archive bytes for one of the built-in Horizon shader packs.
pub fn build_builtin_hshader_bytes(id: &str) -> Option<Vec<u8>> {
    builtin_shader_pack(id)?.package().ok()
}
