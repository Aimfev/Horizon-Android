fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let stage = root.join(".horizon-mixins-stage.json");
    if stage.is_file() {
        println!("cargo:rerun-if-changed={}", stage.display());
        if let Ok(text) = std::fs::read_to_string(&stage)
            && let Some((_, after)) = text.split_once("\"mods\"")
            && let Some((_, list)) = after.split_once('[')
            && let Some((list, _)) = list.split_once(']')
        {
            let mods: Vec<&str> = list
                .split(',')
                .filter_map(|item| item.trim().strip_prefix('"')?.strip_suffix('"'))
                .filter(|id| !id.is_empty())
                .collect();
            if !mods.is_empty() {
                println!("cargo:rustc-env=HORIZON_MIXIN_MODS={}", mods.join(","));
            }
        }
    }
    // Android tombstones can match a native frame to the exact private symbols.
    // This affects the final ELF only, not code generation or release optimization.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-arg=-Wl,--build-id=sha1");
    }
}
