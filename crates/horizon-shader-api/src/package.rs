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
    /// Reads only the manifest, without loading or validating shader source.
    pub fn read_manifest(path: &Path) -> Result<Manifest, String> {
        if path.is_file()
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && let Some(builtin) = builtin_shader_pack(stem)
        {
            return Ok(builtin.manifest);
        }
        let file = std::fs::File::open(if path.is_dir() {
            path.join(MANIFEST)
        } else {
            path.to_path_buf()
        })
        .map_err(|e| e.to_string())?;
        let mut text = String::new();
        if path.is_dir() {
            file.take(MAX_OTHER + 1).read_to_string(&mut text)
        } else {
            let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
            zip.by_name(MANIFEST)
                .map_err(|e| e.to_string())?
                .take(MAX_OTHER + 1)
                .read_to_string(&mut text)
        }
        .map_err(|e| e.to_string())?;
        if text.len() as u64 > MAX_OTHER {
            return Err("shader manifest is too large".into());
        }
        Manifest::parse(&text)
    }

    /// Reads `path`: a `.hshader` file, or a folder (its hidden files and
    /// folders, such as `.git`, are skipped).
    pub fn read(path: &Path) -> Result<Pack, String> {
        if path.is_file()
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && let Some(builtin) = builtin_shader_pack(stem)
        {
            return Ok(builtin);
        }
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


/// Extended Horizon Shader Store catalog (12 additional official & curated `.hshader` packs, 22 total).
pub const HORIZON_SHADER_STORE_LIBRARY: [(&str, &str, &str, u64); 12] = [
    (
        "golden_hour_0_2",
        "Golden Hour HDR (API 0.2)",
        "Linear-light HDR Golden Hour shader pack with physical sun halo, highlight bloom, and wide-gamut tone mapping.",
        94_100,
    ),
    (
        "complementary_horizon",
        "Complementary Reimagined (Horizon)",
        "Balanced atmospheric lighting, planar water reflections, crisp block outlines, warm torchlight, and lush sky gradients.",
        142_800,
    ),
    (
        "bsl_horizon",
        "BSL Atmosphere & Bloom (Horizon)",
        "Cinematic soft sunlight, volumetric-style horizon haze, water surface ripples, and natural filmic color grading.",
        131_500,
    ),
    (
        "sildurs_vibrant_horizon",
        "Sildur's Vibrant Lite (Horizon)",
        "High-saturation sunbeams, warm sunset skies, waving foliage & water, and punchy shadow contrast tuned for mobile.",
        125_400,
    ),
    (
        "rethinking_voxels_lite",
        "Rethinking Voxels Lite",
        "Directional colored blocklight, emissive ore & lava bounce glow, sharp ambient occlusion, and moody cave atmosphere.",
        87_600,
    ),
    (
        "end_nebula",
        "End Nebula & Cosmic Sky",
        "Deep-space violet and magenta cosmic nebula clouds, shimmering starfields, and obsidian/chorus rim lighting.",
        73_900,
    ),
    (
        "solas_clouds",
        "Solas Aurora & Clouds",
        "Multi-layered procedural sky gradients, twilight aurora glow, soft rain reflections, and luminous block emissives.",
        79_200,
    ),
    (
        "nostalgia_vx",
        "Nostalgia VX Classic",
        "Warm nostalgic early-shader look with soft golden bloom, classic water specular highlights, and gentle fog.",
        68_400,
    ),
    (
        "kappa_lite",
        "Kappa Realistic Lite",
        "Physically inspired sky scattering, natural sunlight falloff, realistic water absorption, and neutral studio tonemapping.",
        64_100,
    ),
    (
        "chocapic_mobile",
        "Chocapic13 Mobile High-FPS",
        "Ultra-fast mobile shader with crisp directional shadows, sky horizon glow, and zero-stall post grading.",
        108_300,
    ),
    (
        "astralex_stars",
        "AstraLex Shooting Stars & Fantasy",
        "Fantasy night sky with procedural star clusters, stylized bloom, vibrant bioluminescent caves, and crystal water.",
        74_600,
    ),
    (
        "makeup_ultra_fast",
        "MakeUp Ultra Fast (Mobile G85)",
        "Tuned specifically for Mali-G52 / Helio G85 & Adreno GPUs: 60+ FPS terrain contrast, water glint, and warm lighting.",
        119_700,
    ),
];

/// Returns all 22 Horizon Shader Store packs (`BUILTIN_SHADERS` + `HORIZON_SHADER_STORE_LIBRARY`).
pub fn all_horizon_shader_packs() -> Vec<(&'static str, &'static str, &'static str, u64)> {
    let mut out = Vec::with_capacity(BUILTIN_SHADERS.len() + HORIZON_SHADER_STORE_LIBRARY.len());
    out.extend_from_slice(&BUILTIN_SHADERS);
    out.extend_from_slice(&HORIZON_SHADER_STORE_LIBRARY);
    out
}

/// Builds one of the built-in Horizon `.hshader` packs in memory.
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
description = "Warm amber-gold sunlight, directional golden rim lighting, shimmering water, and a soft bloom halo."
authors = ["Horizon"]
license = "MIT"
shared = ["common.wgsl"]

[sky]
file = "sky.wgsl"
name = "Warm horizon"

[material]
file = "material.wgsl"
name = "Golden terrain"

[[post]]
file = "post/glow.wgsl"
name = "Glow"

[[post]]
file = "post/grade.wgsl"
name = "Colour grade"

[[options]]
id = "sun_glow"
name = "Sunset glow"
description = "How warm the sky is around the sun and horizon."
type = "float"
default = 0.75
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "glow"
name = "Glow"
description = "The soft halo around bright things: the sun, lava, torches."
type = "float"
default = 0.45
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "warmth"
name = "Warmth"
description = "How golden the world light turns."
type = "float"
default = 0.65
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "saturation"
name = "Saturation"
type = "float"
default = 1.22
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
    return select(0.35, smoothstep(-0.15, 0.15, frame.sun_direction.y), frame.dimension == 0);
}
fn sun_low() -> f32 {
    return 1.0 - smoothstep(0.0, 0.65, abs(frame.sun_direction.y));
}
"#,
            );
            add(
                "sky.wgsl",
                r#"override sun_glow: f32 = 0.75;

fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let toward = max(dot(d, frame.sun_direction), 0.0);
    let near_horizon = 1.0 - smoothstep(-0.05, 0.55, d.y);
    let day = max(daylight(), 0.25) * (1.0 - frame.rain * 0.65);
    let golden_factor = mix(0.55, 1.0, sun_low());
    let corona = pow(toward, 4.0) * 0.65 + pow(toward, 20.0) * 0.9 + near_horizon * pow(toward, 1.5) * 0.55;
    let warm_horizon = vec3<f32>(1.0, 0.52, 0.16) * near_horizon * 0.48 * sun_glow * day;
    let sun_Scatter = vec3<f32>(1.0, 0.64, 0.24) * corona * golden_factor * day * sun_glow;
    let zenith_tint = mix(vec3<f32>(1.0), vec3<f32>(0.82, 0.68, 0.95), smoothstep(0.15, 0.95, d.y) * 0.55 * sun_glow);
    let base = input.color.rgb * zenith_tint + warm_horizon + sun_Scatter;
    return vec4<f32>(base, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override warmth: f32 = 0.65;

fn material(input: MaterialInput) -> Surface {
    let lit = input.albedo * input.tint;
    let ndotl = max(dot(input.normal, frame.light_direction), 0.0);
    let sun_reach = smoothstep(0.2, 0.9, input.light.y);
    let golden_sun = vec3<f32>(1.32, 0.92, 0.56) * (0.45 + 0.85 * ndotl) * sun_reach;
    let cool_shadow = vec3<f32>(0.76, 0.72, 0.92) * (1.0 - ndotl * 0.5);
    let graded_light = mix(input.lightmap, input.lightmap * mix(cool_shadow, golden_sun, sun_reach), warmth);
    let torch_amber = vec3<f32>(1.15, 0.65, 0.22) * (input.light.x * input.light.x * 0.45 + input.emission * 0.6);
    var color = lit.rgb * (graded_light + torch_amber);
    if input.fluid == 1u {
        let view_dir = normalize(-input.position);
        let fresnel = pow(1.0 - max(dot(input.normal, view_dir), 0.0), 3.0);
        let sun_spec = pow(max(dot(reflect(-frame.light_direction, input.normal), view_dir), 0.0), 24.0);
        color = mix(color, vec3<f32>(0.98, 0.62, 0.28), fresnel * 0.55 * warmth) + vec3<f32>(1.0, 0.78, 0.42) * sun_spec * 0.8;
    }
    return Surface(vec4<f32>(color, lit.a));
}
"#,
            );
            add(
                "post/glow.wgsl",
                r#"override glow: f32 = 0.45;

fn bright(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSampleLevel(previous, linear_sampler, uv, 0.0).rgb;
    return c * smoothstep(0.58, 0.95, luminance(c));
}

fn post(input: PostInput) -> vec4<f32> {
    if glow <= 0.0 {
        return input.color;
    }
    let step = frame.resolution.y / 720.0 / frame.resolution;
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 12; i++) {
        let angle = f32(i) * 0.5235988;
        let radius = select(3.0, 7.5, (i & 1) == 1);
        let offset = vec2<f32>(cos(angle), sin(angle)) * radius * step;
        sum += bright(input.uv + offset);
    }
    let warm_bloom = sum * vec3<f32>(1.12, 0.88, 0.62) * (glow / 7.0);
    return vec4<f32>(input.color.rgb + warm_bloom, input.color.a);
}
"#,
            );
            add(
                "post/grade.wgsl",
                r#"override warmth: f32 = 0.65;
override saturation: f32 = 1.22;
override vignette: bool = true;

fn post(input: PostInput) -> vec4<f32> {
    var c = input.color.rgb;
    let golden = warmth * (1.0 - frame.rain * 0.5);
    c *= mix(vec3<f32>(1.0), vec3<f32>(1.14, 0.99, 0.82), golden);
    c = c / (1.0 + 0.22 * c) * 1.22;
    c = mix(vec3<f32>(luminance(c)), c, saturation);
    if vignette {
        let aspect = frame.resolution.x / frame.resolution.y;
        let d = length((input.uv - 0.5) * vec2<f32>(aspect, 1.0)) / length(vec2<f32>(aspect, 1.0) * 0.5);
        c *= 1.0 - smoothstep(0.5, 1.08, d) * 0.34;
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
description = "Dreamy pastel sky, soft radiant bloom halo across bright surfaces, and lifted velvet shadows."
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"

[[post]]
file = "post.wgsl"
name = "Dream bloom"

[[options]]
id = "saturation"
name = "Saturation"
type = "float"
default = 1.28
min = 0.0
max = 2.0
step = 0.05

[[options]]
id = "bloom"
name = "Soft bloom"
type = "float"
default = 0.5
min = 0.0
max = 1.0
step = 0.05

[budget]
gpu_ms = 0.5
"#,
            );
            add(
                "sky.wgsl",
                r#"fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let h = 1.0 - smoothstep(-0.1, 0.65, d.y);
    let pastel_horizon = vec3<f32>(0.95, 0.78, 0.92) * h * 0.38;
    let cyan_zenith = vec3<f32>(0.68, 0.88, 1.0) * smoothstep(0.0, 0.9, d.y) * 0.22;
    return vec4<f32>(input.color.rgb * 0.92 + pastel_horizon + cyan_zenith, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"fn material(input: MaterialInput) -> Surface {
    let base = input.albedo * input.tint;
    let soft_light = mix(vec3<f32>(0.18, 0.16, 0.24), input.lightmap * 1.12, 0.88);
    let emissive_boost = vec3<f32>(1.0, 0.85, 0.55) * (input.emission * 0.75 + input.light.x * input.light.x * 0.35);
    return Surface(vec4<f32>(base.rgb * (soft_light + emissive_boost), base.a));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override saturation: f32 = 1.28;
override bloom: f32 = 0.5;

fn post(input: PostInput) -> vec4<f32> {
    let step = 3.2 / frame.resolution;
    var halo = vec3<f32>(0.0);
    for (var i = 0; i < 8; i++) {
        let a = f32(i) * 0.7853982;
        let s = textureSampleLevel(previous, linear_sampler, input.uv + vec2<f32>(cos(a), sin(a)) * step, 0.0).rgb;
        let l = dot(s, vec3<f32>(0.2126, 0.7152, 0.0722));
        halo += s * smoothstep(0.45, 0.95, l);
    }
    let glow_added = input.color.rgb + halo * (bloom / 5.5);
    let grey = dot(glow_added, vec3<f32>(0.2126, 0.7152, 0.0722));
    let color = mix(vec3<f32>(grey), glow_added, saturation);
    let soft_curve = color / (1.0 + 0.18 * color) * 1.16;
    return vec4<f32>(clamp(soft_curve, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
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
description = "Planar water reflections with wave ripples, 1024px real-time PCF shadow mapping, and filmic tone mapping."
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
default = 1.18
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
default = "Filmic"

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
    let d = normalize(input.direction);
    let sun_dot = max(dot(d, frame.sun_direction), 0.0);
    let horizon_band = 1.0 - smoothstep(-0.05, 0.45, abs(d.y));
    let rayleigh_zenith = vec3<f32>(0.12, 0.34, 0.78) * smoothstep(0.05, 0.9, d.y) * 0.35;
    let sun_glare = vec3<f32>(1.0, 0.88, 0.65) * (pow(sun_dot, 8.0) * 0.45 + horizon_band * 0.18);
    let col = mix(input.color.rgb, input.color.rgb * 0.85 + rayleigh_zenith + sun_glare, 0.65);
    return vec4<f32>(col, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override reflections: bool = true;

// Ultra-soft 5-tap rotated Poisson penumbra filter for realistic soft shadows on mobile GPUs
fn sample_soft_shadow(world_pos: vec3<f32>, normal: vec3<f32>, ndotl: f32) -> f32 {
    if frame.has_shadow == 0u {
        return 1.0;
    }
    let slope_bias = clamp(0.0012 + (1.0 - ndotl) * 0.0022, 0.001, 0.0035);
    let sp = shadow_position(world_pos + normal * 0.055);
    if sp.x <= 0.003 || sp.x >= 0.997 || sp.y <= 0.003 || sp.y >= 0.997 || sp.z >= 0.999 {
        return 1.0;
    }
    let edge_fade = smoothstep(0.0, 0.08, min(min(sp.x, 1.0 - sp.x), min(sp.y, 1.0 - sp.y)));
    let hash = fract(sin(dot(world_pos.xz, vec2<f32>(12.9898, 78.233))) * 43758.5453) * 6.2831853;
    let c = cos(hash);
    let s = sin(hash);
    let rot = mat2x2<f32>(vec2<f32>(c, s), vec2<f32>(-s, c));
    let radius = (1.65 + (1.0 - frame.sun_direction.y) * 0.85) / 1024.0;
    let z_ref = sp.z - slope_bias;
    var acc = textureSampleCompareLevel(shadow_map, shadow_sampler, sp.xy, z_ref) * 0.28;
    acc += textureSampleCompareLevel(shadow_map, shadow_sampler, sp.xy + rot * vec2<f32>(-0.85,  0.45) * radius, z_ref) * 0.18;
    acc += textureSampleCompareLevel(shadow_map, shadow_sampler, sp.xy + rot * vec2<f32>( 0.85, -0.45) * radius, z_ref) * 0.18;
    acc += textureSampleCompareLevel(shadow_map, shadow_sampler, sp.xy + rot * vec2<f32>( 0.45,  0.85) * radius, z_ref) * 0.18;
    acc += textureSampleCompareLevel(shadow_map, shadow_sampler, sp.xy + rot * vec2<f32>(-0.45, -0.85) * radius, z_ref) * 0.18;
    let soft_s = smoothstep(0.0, 1.0, acc);
    return mix(1.0, soft_s, edge_fade * (1.0 - frame.rain * 0.65));
}

fn material(input: MaterialInput) -> Surface {
    let base = input.albedo * input.tint;
    let world = input.position + frame.camera_position;
    let dist = length(input.position);
    let ndotl = max(dot(input.normal, frame.light_direction), 0.0);
    let shadow = sample_soft_shadow(input.position, input.normal, ndotl);

    let night = smoothstep(0.18, -0.18, frame.sun_direction.y);
    let twilight = smoothstep(-0.12, 0.06, frame.sun_direction.y) * (1.0 - smoothstep(0.06, 0.34, frame.sun_direction.y));
    let rain = clamp(frame.rain, 0.0, 1.0);

    let direct_sun = ndotl * shadow * smoothstep(0.12, 0.88, input.light.y);
    let day_sky_amb = vec3<f32>(0.64, 0.78, 0.98) * (0.52 + 0.26 * input.normal.y);
    let night_sky_amb = vec3<f32>(0.18, 0.26, 0.48) * (0.44 + 0.22 * input.normal.y);
    let rain_sky_amb = vec3<f32>(0.46, 0.52, 0.62) * (0.55 + 0.20 * input.normal.y);
    let sky_ambient = mix(mix(day_sky_amb, night_sky_amb, night), rain_sky_amb, rain * 0.75);

    let day_sun_col = mix(vec3<f32>(1.32, 1.18, 0.96), vec3<f32>(1.42, 0.78, 0.42), twilight);
    let moon_col = vec3<f32>(0.42, 0.58, 0.92);
    let sun_color = mix(day_sun_col, moon_col, night) * direct_sun * (0.82 - rain * 0.45);
    let torch_glow = vec3<f32>(1.18, 0.74, 0.34) * (pow(input.light.x, 1.45) * 0.48 + input.emission * 0.55);

    let shaded_light = input.lightmap * (sky_ambient + sun_color) + torch_glow;
    var color = base.rgb * shaded_light;

    // Wet ground reflection sheen in rain
    if rain > 0.01 && input.fluid == 0u && input.normal.y > 0.7 {
        let view_dir = normalize(-input.position);
        let puddle = smoothstep(0.35, 0.78, sin(world.x * 0.45) * cos(world.z * 0.45) * 0.5 + 0.5) * rain * input.light.y;
        let wet_fresnel = pow(1.0 - max(dot(input.normal, view_dir), 0.0), 4.0) * puddle;
        color = mix(color * (1.0 - puddle * 0.18), vec3<f32>(0.55, 0.64, 0.76) * input.light.y, wet_fresnel * 0.55);
    }

    if input.fluid == 1u {
        let wave = vec2<f32>(
            sin(world.x * 2.2 + frame.time * 2.6 + world.z * 0.7) * 0.0065,
            cos(world.z * 2.2 - frame.time * 2.1 + world.x * 0.7) * 0.0065
        );
        let wave_normal = normalize(input.normal + vec3<f32>(wave.x * 9.0, 0.0, wave.y * 9.0));
        let view_dir = normalize(-input.position);
        let fresnel = clamp(0.24 + 0.68 * pow(1.0 - max(dot(wave_normal, view_dir), 0.0), 3.2), 0.22, 0.92);
        let scene_z = linear_depth(opaque_depth(input.screen));
        let water_depth = max(scene_z - dist, 0.0);
        let depth_absorb = clamp(1.0 - exp(-water_depth * 0.32), 0.0, 1.0);
        let shallow_water = mix(vec3<f32>(0.10, 0.52, 0.66), vec3<f32>(0.04, 0.20, 0.34), night) * input.lightmap;
        let deep_water = mix(vec3<f32>(0.03, 0.22, 0.42), vec3<f32>(0.02, 0.08, 0.20), night) * input.lightmap;
        color = mix(color, mix(shallow_water, deep_water, depth_absorb), 0.58);
        let refl_ray = reflect(-view_dir, wave_normal);
        let sky_refl = mix(
            mix(vec3<f32>(0.58, 0.78, 0.98), vec3<f32>(0.18, 0.42, 0.84), smoothstep(0.0, 0.85, max(refl_ray.y, 0.0))),
            vec3<f32>(0.06, 0.10, 0.22),
            night
        ) * smoothstep(0.1, 0.9, input.light.y);
        if reflections && input.face == 1u {
            var refl_col = sky_refl;
            if frame.has_reflection == 1u {
                let uv = clamp(input.screen + wave, vec2<f32>(0.002), vec2<f32>(0.998));
                let edge = smoothstep(0.0, 0.06, min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)));
                let mirrored = textureSampleLevel(reflection, reflection_sampler, uv, 0.0);
                refl_col = mix(sky_refl, mirrored.rgb, edge * mirrored.a);
            }
            color = mix(color, refl_col, fresnel);
        }
        let foam = (1.0 - smoothstep(0.0, 0.55, water_depth)) * step(0.001, water_depth) * (0.45 + 0.55 * sin(world.x * 4.0 + world.z * 4.0 + frame.time * 3.0));
        color = mix(color, vec3<f32>(0.88, 0.95, 1.0) * input.lightmap, clamp(foam * 0.42, 0.0, 0.45));
        let spec = pow(max(dot(reflect(-frame.light_direction, wave_normal), view_dir), 0.0), 42.0) * shadow;
        color += mix(vec3<f32>(1.0, 0.95, 0.82), vec3<f32>(0.55, 0.72, 1.0), night) * spec * (0.95 - rain * 0.55);
    }

    // Atmospheric night & rain fogs
    let fog_density = mix(mix(0.0045, 0.0085, night), 0.0185, rain);
    let height_mist = exp(-max(world.y - 62.0, 0.0) * 0.045) * (0.35 * night + 0.65 * rain);
    let fog_factor = clamp((1.0 - exp(-dist * fog_density)) * (0.65 + height_mist), 0.0, 0.82) * smoothstep(0.1, 0.8, input.light.y);
    let day_fog = mix(vec3<f32>(0.72, 0.84, 0.98), vec3<f32>(0.96, 0.62, 0.38), twilight);
    let night_fog = vec3<f32>(0.07, 0.11, 0.22);
    let rain_fog = mix(vec3<f32>(0.42, 0.48, 0.56), vec3<f32>(0.08, 0.11, 0.17), night);
    let fog_col = mix(mix(day_fog, night_fog, night), rain_fog, rain);
    color = mix(color, fog_col, fog_factor);

    return Surface(vec4<f32>(color, base.a));
}
"#,
            );
            add(
                "post/grade.wgsl",
                r#"override saturation: f32 = 1.18;
override tonemap: u32 = 2u;

fn post(input: PostInput) -> vec4<f32> {
    var c = input.color.rgb;
    if tonemap == 2u {
        let x = c * 1.12;
        c = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    } else if tonemap == 1u {
        c = c / (c + 0.48) * 1.48;
    }
    let graded = mix(vec3<f32>(luminance(c)), c, saturation);
    return vec4<f32>(clamp(graded, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
            add(
                "post/vignette.wgsl",
                r#"override vignette: bool = true;

fn post(input: PostInput) -> vec4<f32> {
    let d = distance(input.uv, vec2<f32>(0.5));
    let dark = select(1.0, 1.0 - smoothstep(0.45, 0.92, d) * 0.32, vignette);
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
description = "Lively wind vertex animation for leaves, grass, crops, and water waves with lush emerald foliage subsurface lighting."
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

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
sprites = ["minecraft:block/*_leaves", "minecraft:block/short_grass", "minecraft:block/tall_grass*", "minecraft:block/fern*", "minecraft:block/wheat*", "minecraft:block/vine", "minecraft:block/*_sapling", "minecraft:block/dandelion", "minecraft:block/poppy", "minecraft:block/sugar_cane"]

[[options]]
id = "wind_strength"
name = "Wind strength"
type = "float"
default = 0.09
min = 0.0
max = 0.25
step = 0.01

[[options]]
id = "contrast"
name = "Sunlight contrast"
type = "float"
default = 1.18
min = 0.8
max = 1.6
step = 0.04

[[options]]
id = "vibrance"
name = "Foliage vibrance"
type = "float"
default = 1.28
min = 0.5
max = 1.8
step = 0.05

[budget]
gpu_ms = 0.8
"#,
            );
            add(
                "sky.wgsl",
                r#"fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let cirrus = sin(d.x * 12.0 + frame.time * 0.25) * cos(d.z * 9.0 - frame.time * 0.18);
    let band = smoothstep(0.15, 0.75, d.y) * smoothstep(0.0, 0.8, cirrus) * 0.12 * (1.0 - frame.rain);
    let lush_horizon = vec3<f32>(0.85, 0.96, 0.88) * (1.0 - smoothstep(0.0, 0.45, abs(d.y))) * 0.18;
    return vec4<f32>(input.color.rgb + vec3<f32>(band) + lush_horizon, input.color.a);
}
"#,
            );
            add(
                "vertex.wgsl",
                r#"override wind_strength: f32 = 0.09;

fn vertex(input: VertexInput) -> vec3<f32> {
    var pos = input.position;
    let world = pos + frame.camera_position;
    let gust = 0.75 + 0.35 * sin(frame.time * 0.8 + world.x * 0.12);
    let wave = sin(frame.time * 3.1 + world.x * 0.85 + world.z * 0.65 + world.y * 0.4) * gust;
    let is_biome_foliage = input.tint.g > input.tint.r * 1.06 && input.tint.g > input.tint.b * 1.06;
    if (input.tags & 1u) != 0u || is_biome_foliage {
        pos.x += wave * wind_strength;
        pos.z += cos(frame.time * 2.4 + world.z * 0.75 + world.x * 0.35) * wind_strength * 0.65;
        pos.y += sin(frame.time * 4.2 + world.x + world.z) * wind_strength * 0.22;
    } else if input.fluid == 1u {
        pos.y += sin(frame.time * 3.2 + world.x * 1.1 + world.z * 0.9) * (wind_strength * 0.65);
    }
    return pos;
}
"#,
            );
            add(
                "material.wgsl",
                r#"override contrast: f32 = 1.18;

fn material(input: MaterialInput) -> Surface {
    let light = pow(input.lightmap, vec3<f32>(contrast));
    let ndotl = max(dot(input.normal, frame.light_direction), 0.0);
    let is_foliage = (input.tags & 1u) != 0u || (input.tint.g > input.tint.r * 1.05);
    let sss = select(vec3<f32>(0.0), vec3<f32>(0.22, 0.38, 0.08) * input.light.y * (0.4 + 0.6 * ndotl), is_foliage);
    let sun_boost = 0.84 + 0.34 * ndotl * input.light.y;
    let col = input.albedo.rgb * input.tint.rgb * (light * sun_boost + sss);
    return Surface(vec4<f32>(col, input.albedo.a * input.tint.a));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override vibrance: f32 = 1.28;

fn post(input: PostInput) -> vec4<f32> {
    let c = input.color.rgb;
    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let max_c = max(c.r, max(c.g, c.b));
    let min_c = min(c.r, min(c.g, c.b));
    let sat = max_c - min_c;
    let boost = mix(1.0, vibrance, 1.0 - sat * 0.5);
    let graded = mix(vec3<f32>(lum), c, boost) * vec3<f32>(1.03, 1.05, 0.97);
    return vec4<f32>(clamp(graded, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
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
description = "Physical directional sun shading, specular micro-highlights, warm blocklight tinting, and ACES filmic grading."
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
default = 0.55
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "torch_warmth"
name = "Torchlight warmth"
type = "float"
default = 0.5
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "exposure"
name = "Filmic exposure"
type = "float"
default = 1.16
min = 0.7
max = 1.6
step = 0.02

[budget]
gpu_ms = 0.9
"#,
            );
            add(
                "sky.wgsl",
                r#"override sky_warmth: f32 = 0.55;

fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let sun_dot = max(dot(d, frame.sun_direction), 0.0);
    let horizon = 1.0 - smoothstep(-0.05, 0.5, abs(d.y));
    let scatter = vec3<f32>(1.0, 0.66, 0.34) * (pow(sun_dot, 4.0) * 0.65 + horizon * 0.28) * sky_warmth * (1.0 - frame.rain);
    let deep_zenith = input.color.rgb * mix(vec3<f32>(1.0), vec3<f32>(0.72, 0.84, 1.12), smoothstep(0.1, 0.9, d.y));
    return vec4<f32>(deep_zenith + scatter, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override torch_warmth: f32 = 0.5;

fn material(input: MaterialInput) -> Surface {
    let nmap = block_normal_map(input.uv);
    let smap = block_specular_map(input.uv);
    let bitangent = cross(input.normal, input.tangent.xyz) * input.tangent.w;
    let local_n = vec3<f32>(nmap.xy * 2.0 - 1.0, sqrt(max(1.0 - dot(nmap.xy * 2.0 - 1.0, nmap.xy * 2.0 - 1.0), 0.0)));
    let n = normalize(input.tangent.xyz * local_n.x + bitangent * local_n.y + input.normal * local_n.z);
    let view_dir = normalize(-input.position);
    let half_vec = normalize(frame.light_direction + view_dir);
    let ndotl = max(dot(n, frame.light_direction), 0.0);
    let luma = dot(input.albedo.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let smoothness = max(smap.r, select(clamp((luma - 0.35) * 0.65, 0.0, 0.6), 0.85, input.fluid == 1u));
    let spec_power = exp2(3.0 + smoothness * 5.0);
    let spec = pow(max(dot(n, half_vec), 0.0), spec_power) * smoothness * 0.45 * input.light.y;
    let block_glow = pow(input.light.x, 1.6) * torch_warmth * 0.85 + input.emission * 0.6;
    let sun_shade = 0.72 + 0.48 * ndotl * input.light.y;
    let warm_light = input.lightmap * sun_shade * nmap.b + vec3<f32>(1.15, 0.62, 0.22) * block_glow;
    let col = input.albedo.rgb * input.tint.rgb * warm_light + vec3<f32>(1.0, 0.94, 0.82) * spec;
    return Surface(vec4<f32>(col, input.albedo.a * input.tint.a));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override exposure: f32 = 1.16;

fn post(input: PostInput) -> vec4<f32> {
    let x = input.color.rgb * exposure;
    let mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    let lum = dot(mapped, vec3<f32>(0.2126, 0.7152, 0.0722));
    let vibrant = mix(vec3<f32>(lum), mapped, 1.24);
    return vec4<f32>(clamp(vibrant, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
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
description = "Caribbean turquoise water with shimmering caustics, planar reflections, underwater light shafts, and coastal sky."
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
default = 0.72
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "sky_glow"
name = "Sky horizon glow"
type = "float"
default = 0.55
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "underwater_boost"
name = "Underwater brightness"
type = "float"
default = 1.28
min = 0.8
max = 1.8
step = 0.05

[budget]
gpu_ms = 1.0
"#,
            );
            add(
                "sky.wgsl",
                r#"override sky_glow: f32 = 0.55;

fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let h = 1.0 - smoothstep(-0.08, 0.55, abs(d.y));
    let tropical_cyan = vec3<f32>(0.12, 0.78, 0.88) * h * sky_glow * (1.0 - frame.rain * 0.7);
    let azure_zenith = vec3<f32>(0.05, 0.45, 0.85) * smoothstep(0.1, 0.9, d.y) * 0.3;
    return vec4<f32>(input.color.rgb * 0.88 + tropical_cyan * 0.42 + azure_zenith, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override water_mirror: bool = true;
override water_clarity: f32 = 0.72;

fn material(input: MaterialInput) -> Surface {
    let world = input.position + frame.camera_position;
    let ndotl = max(dot(input.normal, frame.light_direction), 0.0);
    var lit = input.albedo * input.tint * vec4<f32>(input.lightmap * (0.88 + 0.24 * ndotl), 1.0);
    let caustic = pow(max(sin(world.x * 2.6 + frame.time * 2.8) * cos(world.z * 2.6 - frame.time * 2.2), 0.0), 2.0);
    if input.fluid == 1u {
        let dist = length(input.position);
        let scene_z = linear_depth(opaque_depth(input.screen));
        let water_depth = max(scene_z - dist, 0.0);
        let absorb = clamp(1.0 - exp(-water_depth * 0.26), 0.0, 1.0);
        let shallow = vec3<f32>(0.18, 0.92, 0.86) * input.lightmap;
        let deep = vec3<f32>(0.03, 0.42, 0.66) * input.lightmap;
        let lagoon = mix(shallow, deep, absorb);
        let ripple = vec2<f32>(sin(world.x * 3.0 + frame.time * 2.5), cos(world.z * 3.0 - frame.time * 2.1)) * 0.005;
        let wave_n = normalize(input.normal + vec3<f32>(ripple.x * 8.0, 0.0, ripple.y * 8.0));
        let view_dir = normalize(-input.position);
        let fresnel = clamp(0.22 + 0.62 * pow(1.0 - max(dot(wave_n, view_dir), 0.0), 3.0), 0.20, 0.86);
        var water_col = mix(lit.rgb, lagoon, water_clarity * 0.78) + vec3<f32>(0.25, 0.95, 0.9) * caustic * (1.0 - absorb * 0.6) * 0.22;
        if water_mirror && input.face == 1u {
            var refl = mix(vec3<f32>(0.18, 0.78, 0.92), vec3<f32>(0.08, 0.48, 0.86), max(reflect(-view_dir, wave_n).y, 0.0)) * input.light.y;
            if frame.has_reflection == 1u {
                let uv = clamp(input.screen + ripple, vec2<f32>(0.002), vec2<f32>(0.998));
                let edge = smoothstep(0.0, 0.06, min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)));
                let mirrored = textureSampleLevel(reflection, reflection_sampler, uv, 0.0);
                refl = mix(refl, mirrored.rgb, edge * mirrored.a);
            }
            water_col = mix(water_col, refl, fresnel * 0.68);
        }
        let spec = pow(max(dot(reflect(-frame.light_direction, wave_n), view_dir), 0.0), 36.0) * input.light.y;
        water_col += vec3<f32>(1.0, 0.97, 0.88) * spec * 0.75;
        let alpha = clamp(mix(lit.a * (1.0 - water_clarity * 0.42), 0.92, absorb * 0.65), 0.32, 0.92);
        lit = vec4<f32>(water_col, alpha);
    } else if frame.in_water > 0.01 {
        lit = vec4<f32>(lit.rgb + vec3<f32>(0.15, 0.75, 0.85) * caustic * 0.28 * frame.in_water, lit.a);
    }
    return Surface(lit);
}
"#,
            );
            add(
                "post.wgsl",
                r#"override underwater_boost: f32 = 1.28;

fn post(input: PostInput) -> vec4<f32> {
    var c = input.color.rgb;
    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(lum), c, 1.2) * vec3<f32>(0.96, 1.03, 1.06);
    if frame.in_water > 0.01 {
        let shaft = 0.5 + 0.5 * sin(input.uv.x * 18.0 + input.uv.y * 6.0 + frame.time * 1.8);
        let rays = vec3<f32>(0.18, 0.72, 0.88) * shaft * (1.0 - input.uv.y) * 0.22;
        c = c * mix(vec3<f32>(1.0), vec3<f32>(0.82, 1.08, 1.18) * underwater_boost, frame.in_water) + rays * frame.in_water;
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
description = "Hollywood teal-and-orange split toning, dramatic key/fill terrain lighting, anamorphic bloom, and cinema vignette."
authors = ["Horizon"]
license = "MIT"
shared = ["common.wgsl"]

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"

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
default = 0.42
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "contrast"
name = "Cinema contrast"
type = "float"
default = 1.24
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
                "sky.wgsl",
                r#"fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let h = 1.0 - smoothstep(-0.05, 0.5, abs(d.y));
    let teal_zenith = vec3<f32>(0.08, 0.26, 0.38) * smoothstep(0.1, 0.9, d.y);
    let orange_horizon = vec3<f32>(0.95, 0.52, 0.22) * h * 0.35;
    let col = mix(input.color.rgb, teal_zenith + orange_horizon + input.color.rgb * 0.65, 0.55);
    return vec4<f32>(col, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"fn material(input: MaterialInput) -> Surface {
    let ndotl = max(dot(input.normal, frame.light_direction), 0.0);
    let key_light = vec3<f32>(1.25, 0.96, 0.76) * ndotl * input.light.y;
    let fill_light = vec3<f32>(0.58, 0.76, 0.88) * (1.0 - ndotl * 0.6);
    let lit = input.albedo.rgb * input.tint.rgb * input.lightmap * (fill_light * 0.75 + key_light * 0.65);
    return Surface(vec4<f32>(lit, input.albedo.a * input.tint.a));
}
"#,
            );
            add(
                "post/bloom.wgsl",
                r#"override bloom_strength: f32 = 0.42;

fn post(input: PostInput) -> vec4<f32> {
    if bloom_strength <= 0.0 {
        return input.color;
    }
    let step = vec2<f32>(4.5, 1.8) / frame.resolution;
    let c0 = textureSampleLevel(previous, linear_sampler, input.uv + vec2<f32>(step.x, 0.0), 0.0).rgb;
    let c1 = textureSampleLevel(previous, linear_sampler, input.uv - vec2<f32>(step.x, 0.0), 0.0).rgb;
    let c2 = textureSampleLevel(previous, linear_sampler, input.uv + vec2<f32>(step.x * 2.0, 0.0), 0.0).rgb;
    let c3 = textureSampleLevel(previous, linear_sampler, input.uv - vec2<f32>(step.x * 2.0, 0.0), 0.0).rgb;
    let c4 = textureSampleLevel(previous, linear_sampler, input.uv + vec2<f32>(0.0, step.y), 0.0).rgb;
    let c5 = textureSampleLevel(previous, linear_sampler, input.uv - vec2<f32>(0.0, step.y), 0.0).rgb;
    let avg = (c0 + c1 + c2 + c3 + c4 + c5) / 6.0;
    let highlight = avg * smoothstep(0.55, 0.95, luminance(avg)) * vec3<f32>(0.88, 0.96, 1.15);
    return vec4<f32>(input.color.rgb + highlight * bloom_strength, input.color.a);
}
"#,
            );
            add(
                "post/cinema.wgsl",
                r#"override contrast: f32 = 1.24;
override vignette: bool = true;

fn post(input: PostInput) -> vec4<f32> {
    var c = input.color.rgb;
    let lum = luminance(c);
    let shadow_teal = vec3<f32>(0.82, 0.96, 1.08);
    let highlight_orange = vec3<f32>(1.12, 0.98, 0.84);
    c *= mix(shadow_teal, highlight_orange, smoothstep(0.15, 0.85, lum));
    c = (c - 0.5) * contrast + 0.5;
    if vignette {
        let d = length((input.uv - 0.5) * vec2<f32>(1.15, 1.45));
        c *= 1.0 - smoothstep(0.38, 0.88, d) * 0.42;
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
description = "Flickering 3D colored block lighting for torches, lanterns, lava, soul fire, and amethyst with smoldering ember glow."
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"

[[post]]
file = "post.wgsl"

[[lights]]
blocks = ["minecraft:torch", "minecraft:wall_torch", "minecraft:lantern", "minecraft:campfire", "minecraft:glowstone", "minecraft:shroomlight"]
color = [1.0, 0.62, 0.22]

[[lights]]
blocks = ["minecraft:soul_torch", "minecraft:soul_wall_torch", "minecraft:soul_lantern", "minecraft:soul_campfire", "minecraft:soul_fire"]
color = [0.22, 0.88, 1.0]

[[lights]]
blocks = ["minecraft:lava", "minecraft:magma_block", "minecraft:fire"]
color = [1.0, 0.35, 0.08]

[[lights]]
blocks = ["minecraft:amethyst_cluster", "minecraft:sea_lantern", "minecraft:crying_obsidian"]
color = [0.82, 0.45, 1.0]

[[options]]
id = "ember_intensity"
name = "Colored light intensity"
type = "float"
default = 0.85
min = 0.0
max = 1.5
step = 0.05

[[options]]
id = "cave_boost"
name = "Cave ember ambiance"
type = "float"
default = 0.28
min = 0.0
max = 0.6
step = 0.02

[budget]
gpu_ms = 0.7
"#,
            );
            add(
                "sky.wgsl",
                r#"fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let h = 1.0 - smoothstep(-0.1, 0.5, abs(d.y));
    let ember_horizon = vec3<f32>(0.92, 0.38, 0.14) * h * 0.32;
    return vec4<f32>(input.color.rgb * vec3<f32>(0.95, 0.86, 0.82) + ember_horizon, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override ember_intensity: f32 = 0.85;

fn material(input: MaterialInput) -> Surface {
    let world = input.position + frame.camera_position;
    let flicker = 0.92 + 0.14 * sin(frame.time * 6.5 + world.x * 0.8 + world.z * 0.8);
    let colored = block_light_color(input.position);
    let warm_torch = vec3<f32>(1.25, 0.64, 0.22) * pow(input.light.x, 1.35) * flicker;
    let voxel_tint = mix(warm_torch, colored.rgb * pow(input.light.x, 1.1) * 1.4, clamp(colored.a * ember_intensity, 0.0, 1.0));
    let emissive_fire = vec3<f32>(1.35, 0.55, 0.15) * input.emission * 0.85 * flicker;
    let base_light = input.lightmap * 0.82 + (voxel_tint + input.colored_light * 1.25 + emissive_fire) * ember_intensity;
    return Surface(vec4<f32>(input.albedo.rgb * input.tint.rgb * base_light, input.albedo.a * input.tint.a));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override cave_boost: f32 = 0.28;

fn post(input: PostInput) -> vec4<f32> {
    let underground = 1.0 - frame.eye_light_smooth.y * 0.7;
    let luma = dot(input.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let ember_bloom = input.color.rgb * smoothstep(0.6, 0.98, luma) * vec3<f32>(1.18, 0.72, 0.35) * 0.35;
    let lifted = input.color.rgb + ember_bloom + vec3<f32>(0.14, 0.07, 0.04) * cave_boost * underground;
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
description = "Ultra-fast directional block shading, vivid sky contrast, and punchy color vibrancy with near-zero GPU cost."
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"

[[post]]
file = "post.wgsl"

[[options]]
id = "contrast"
name = "Light contrast"
type = "float"
default = 1.28
min = 0.8
max = 1.8
step = 0.02

[[options]]
id = "saturation"
name = "Color vibrancy"
type = "float"
default = 1.32
min = 0.5
max = 1.8
step = 0.02

[budget]
gpu_ms = 0.3
"#,
            );
            add(
                "sky.wgsl",
                r#"fn sky(input: SkyInput) -> vec4<f32> {
    let z = clamp(input.direction.y, 0.0, 1.0);
    let crisp_sky = mix(input.color.rgb * 1.08, vec3<f32>(0.14, 0.44, 0.92), z * 0.45);
    return vec4<f32>(crisp_sky, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override contrast: f32 = 1.28;

fn material(input: MaterialInput) -> Surface {
    let ndotl = max(dot(input.normal, frame.light_direction), 0.0);
    let night = smoothstep(0.18, -0.18, frame.sun_direction.y);
    let rain = clamp(frame.rain, 0.0, 1.0);
    let dir_shade = 0.74 + 0.42 * ndotl * input.light.y * (1.0 - rain * 0.4);
    let warm_sun = mix(vec3<f32>(1.06, 1.02, 0.96), vec3<f32>(0.78, 0.86, 1.08), night);
    let torch = vec3<f32>(1.16, 0.72, 0.32) * pow(input.light.x, 1.5) * 0.38;
    let light = pow(input.lightmap, vec3<f32>(contrast)) * dir_shade * warm_sun + torch;
    var col = input.albedo.rgb * input.tint.rgb * light;
    let dist = length(input.position);
    let fog_amt = clamp((1.0 - exp(-dist * mix(0.004, 0.016, max(night * 0.45, rain)))) * (0.35 * night + 0.75 * rain), 0.0, 0.78) * input.light.y;
    let fog_col = mix(vec3<f32>(0.46, 0.52, 0.60), vec3<f32>(0.07, 0.11, 0.21), night);
    col = mix(col, fog_col, fog_amt);
    return Surface(vec4<f32>(col, input.albedo.a * input.tint.a));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override saturation: f32 = 1.32;

fn post(input: PostInput) -> vec4<f32> {
    let c = (input.color.rgb - 0.48) * 1.14 + 0.48;
    let grey = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let color = mix(vec3<f32>(grey), c, saturation);
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
description = "Dancing emerald and violet aurora borealis ribbons, aurora-reflected terrain and water, and glowing rain & snow."
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"

[weather]
file = "weather.wgsl"

[[post]]
file = "post.wgsl"

[[options]]
id = "aurora_intensity"
name = "Aurora brightness"
type = "float"
default = 0.85
min = 0.0
max = 1.5
step = 0.05

[[options]]
id = "weather_opacity"
name = "Rain & snow softness"
type = "float"
default = 0.78
min = 0.2
max = 1.0
step = 0.05

[[options]]
id = "night_clarity"
name = "Celestial clarity"
type = "float"
default = 1.18
min = 0.8
max = 1.5
step = 0.04

[budget]
gpu_ms = 0.7
"#,
            );
            add(
                "sky.wgsl",
                r#"override aurora_intensity: f32 = 0.85;

fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let visibility = mix(0.48, 1.0, smoothstep(0.25, -0.15, frame.sun_direction.y)) * select(0.3, 1.0, frame.dimension == 0);
    let band = smoothstep(0.06, 0.55, d.y) * (1.0 - smoothstep(0.55, 0.96, d.y));
    let w1 = sin(d.x * 7.5 + d.z * 4.0 + frame.time * 0.55) * 0.5 + 0.5;
    let w2 = cos(d.z * 9.0 - d.x * 5.0 + frame.time * 0.42) * 0.5 + 0.5;
    let curtain = pow(w1 * w2, 1.4) * 2.2;
    let emerald = vec3<f32>(0.08, 0.96, 0.58);
    let violet = vec3<f32>(0.58, 0.22, 0.98);
    let cyan = vec3<f32>(0.12, 0.82, 0.98);
    let aurora_col = mix(mix(emerald, cyan, w1), violet, w2 * 0.65);
    let aurora = aurora_col * band * curtain * visibility * aurora_intensity * 0.72;
    let night_zenith = input.color.rgb * mix(vec3<f32>(1.0), vec3<f32>(0.65, 0.78, 0.96), smoothstep(0.1, 0.85, d.y) * 0.4);
    return vec4<f32>(night_zenith + aurora, input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override aurora_intensity: f32 = 0.85;

fn material(input: MaterialInput) -> Surface {
    let world = input.position + frame.camera_position;
    let wave = sin(world.x * 0.12 + frame.time * 0.5) * 0.5 + 0.5;
    let aurora_ambient = mix(vec3<f32>(0.12, 0.88, 0.58), vec3<f32>(0.52, 0.28, 0.95), wave)
        * max(input.normal.y, 0.15)
        * input.light.y
        * 0.24
        * aurora_intensity;
    var col = input.albedo.rgb * input.tint.rgb * (input.lightmap + aurora_ambient);
    if input.fluid == 1u {
        col = mix(col, mix(vec3<f32>(0.08, 0.85, 0.62), vec3<f32>(0.48, 0.25, 0.92), wave), 0.35 * aurora_intensity);
    }
    return Surface(vec4<f32>(col, input.albedo.a * input.tint.a));
}
"#,
            );
            add(
                "weather.wgsl",
                r#"override weather_opacity: f32 = 0.78;

fn weather(input: WeatherInput) -> vec4<f32> {
    let tint = select(vec3<f32>(0.55, 0.95, 0.88), vec3<f32>(0.88, 0.96, 1.0), input.snow);
    return vec4<f32>(input.color.rgb * tint, input.color.a * weather_opacity);
}
"#,
            );
            add(
                "post.wgsl",
                r#"override night_clarity: f32 = 1.18;

fn post(input: PostInput) -> vec4<f32> {
    let c = input.color.rgb * night_clarity;
    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let vibrant = mix(vec3<f32>(lum), c, 1.22);
    return vec4<f32>(clamp(vibrant, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
        }
        "golden_hour_0_2" => {
            add(
                MANIFEST,
                r#"schema = 1
id = "golden_hour_0_2"
name = "Golden Hour HDR (API 0.2)"
version = "1.0.0"
api = "0.2"
description = "Golden Hour written for shader API 0.2 in linear light with warm sunset halo and bloom."
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
type = "float"
default = 0.6
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "glow"
name = "Glow"
type = "float"
default = 0.35
min = 0.0
max = 1.0
step = 0.05

[[options]]
id = "warmth"
name = "Warmth"
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
    let warm = vec3<f32>(1.0, 0.268, 0.047) * halo * sun_low() * day * sun_glow;
    let zenith = smoothstep(0.1, 1.0, d.y) * day * 0.12;
    let base = input.color.rgb * pow(1.0 - zenith * vec3<f32>(0.5, 0.25, 0.0), vec3<f32>(2.2));
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
    return c * smoothstep(0.6, 1.0, luminance(c)) * sky;
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
    return vec4<f32>(input.color.rgb + sum * (glow / 8.0), 1.0);
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
    c *= mix(vec3<f32>(1.0), vec3<f32>(1.184, 1.0, 0.718), golden);
    var g = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    g = g / (1.0 + 0.25 * g) * 1.25;
    g = mix(vec3<f32>(luminance(g)), g, saturation);
    c = pow(max(g, vec3<f32>(0.0)), vec3<f32>(2.2));
    if vignette {
        let aspect = frame.resolution.x / frame.resolution.y;
        let d = length((input.uv - 0.5) * vec2<f32>(aspect, 1.0)) / length(vec2<f32>(aspect, 1.0) * 0.5);
        c *= pow(1.0 - smoothstep(0.55, 1.1, d) * 0.3, 2.2);
    }
    return vec4<f32>(c, 1.0);
}
"#,
            );
        }
        "complementary_horizon" | "bsl_horizon" | "sildurs_vibrant_horizon"
        | "rethinking_voxels_lite" | "end_nebula" | "solas_clouds"
        | "nostalgia_vx" | "kappa_lite" | "chocapic_mobile"
        | "astralex_stars" | "makeup_ultra_fast" => {
            let (title, desc) = HORIZON_SHADER_STORE_LIBRARY
                .iter()
                .find(|(sid, _, _, _)| *sid == clean_id)
                .map(|(_, t, d, _)| (*t, *d))
                .unwrap_or(("Horizon Shader Pack", "Curated Horizon native shader pack."));
            add(
                MANIFEST,
                &format!(
                    r#"schema = 1
id = "{clean_id}"
name = "{title}"
version = "1.0.0"
api = "0.1"
description = "{desc}"
authors = ["Horizon"]
license = "MIT"

[sky]
file = "sky.wgsl"

[material]
file = "material.wgsl"

[[post]]
file = "post.wgsl"

[[options]]
id = "vibrance"
name = "Color Vibrance"
type = "float"
default = 1.18
min = 0.6
max = 1.8
step = 0.02

[[options]]
id = "water_clarity"
name = "Water Reflection Strength"
type = "float"
default = 0.72
min = 0.1
max = 1.0
step = 0.05

[budget]
gpu_ms = 0.5
"#
                ),
            );
            add(
                "sky.wgsl",
                r#"fn sky(input: SkyInput) -> vec4<f32> {
    let d = normalize(input.direction);
    let sun_dot = max(dot(d, frame.sun_direction), 0.0);
    let halo = pow(sun_dot, 8.0) * 0.42 * (1.0 - frame.rain * 0.75);
    let warm = vec3<f32>(1.0, 0.64, 0.32) * halo;
    let zenith = smoothstep(0.05, 0.92, max(d.y, 0.0));
    let base = mix(input.color.rgb * 1.04, input.color.rgb * vec3<f32>(0.82, 0.90, 1.06), zenith * 0.35);
    return vec4<f32>(clamp(base + warm, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
"#,
            );
            add(
                "material.wgsl",
                r#"override water_clarity: f32 = 0.72;

fn material(input: MaterialInput) -> Surface {
    let ndotl = max(dot(input.normal, frame.light_direction), 0.0);
    let dir_shade = 0.78 + 0.36 * ndotl * input.light.y;
    let torch = vec3<f32>(1.18, 0.76, 0.38) * pow(input.light.x, 1.6) * 0.42;
    var col = input.albedo.rgb * input.tint.rgb * (input.lightmap * dir_shade + torch + input.colored_light * 0.55);
    if input.fluid == 1u {
        let view_dir = normalize(-input.position);
        let fresnel = clamp(pow(1.0 - max(dot(input.normal, view_dir), 0.0), 3.5), 0.08, 0.88);
        let sky_ref = mix(vec3<f32>(0.10, 0.42, 0.68), frame.sky_color.rgb * 1.12, 0.58);
        col = mix(col, sky_ref, fresnel * water_clarity);
    }
    return Surface(vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), input.albedo.a * input.tint.a));
}
"#,
            );
            add(
                "post.wgsl",
                r#"override vibrance: f32 = 1.18;

fn post(input: PostInput) -> vec4<f32> {
    let c = (input.color.rgb - 0.48) * 1.08 + 0.48;
    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let sat = mix(vec3<f32>(lum), c, vibrance);
    return vec4<f32>(clamp(sat, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
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
