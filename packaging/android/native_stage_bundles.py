#!/usr/bin/env python3
"""Stages native mod bundles (.hnmod, combined .hmod, or directories), orders [dependencies],
wires [api] trait mods into horizon-android, and writes build.json."""
import hashlib
import json
import pathlib
import re
import shutil
import sys
import tomllib
import zipfile


def stage_bundles(work: pathlib.Path, commit: str, inputs: list[pathlib.Path]):
    bundles = {}
    for idx, inp in enumerate(inputs):
        bdir = work / "raw_bundles" / f"b_{idx}"
        bdir.mkdir(parents=True, exist_ok=True)
        if inp.is_file():
            with zipfile.ZipFile(inp) as zf:
                zf.extractall(bdir)
        elif inp.is_dir():
            shutil.copytree(inp, bdir, dirs_exist_ok=True)
        else:
            raise SystemExit(f"native_build: {inp} does not exist")

        mixin_toml = bdir / "horizon-mixin.toml"
        mixin_root = bdir
        if not mixin_toml.is_file():
            found = list(bdir.glob("mixins/*/horizon-mixin.toml"))
            if not found:
                raise SystemExit(f"native_build: {inp} has no horizon-mixin.toml")
            mixin_toml = found[0]
            mixin_root = mixin_toml.parent

        doc = tomllib.loads(mixin_toml.read_text(encoding="utf-8"))
        mod_id = doc["mod"]["id"]
        if mod_id in bundles:
            raise SystemExit(f"native_build: duplicate mod id {mod_id}")
        deps = list((doc.get("dependencies") or {}).keys())
        bundles[mod_id] = {
            "id": mod_id,
            "version": doc["mod"]["version"],
            "bdir": bdir,
            "mixin_root": mixin_root,
            "mixin_toml": mixin_toml,
            "doc": doc,
            "deps": deps,
        }

    ordered = []
    visiting = set()
    visited = set()

    def visit(mid):
        if mid in visited:
            return
        if mid in visiting:
            raise SystemExit(f"native_build: dependency cycle involving {mid}")
        visiting.add(mid)
        for dep in bundles[mid]["deps"]:
            if dep not in bundles:
                raise SystemExit(f"native_build: {mid} requires missing dependency {dep}")
            visit(dep)
        visiting.remove(mid)
        visited.add(mid)
        ordered.append(mid)

    for mid in sorted(bundles.keys()):
        visit(mid)

    (work / "stage_mods.json").write_text(json.dumps({"mods": ordered}), encoding="utf-8")

    for order_idx, mid in enumerate(ordered):
        info = bundles[mid]
        bdir = info["bdir"]
        mixin_root = info["mixin_root"]
        doc = info["doc"]

        for data_candidate in [mixin_root / "data", bdir / "data"]:
            if data_candidate.is_dir():
                shutil.copytree(data_candidate, work / "native_data" / mid, dirs_exist_ok=True)

        if (bdir / "horizon-mod.toml").is_file():
            c_dst = work / "native_content" / mid
            c_dst.mkdir(parents=True, exist_ok=True)
            shutil.copy2(bdir / "horizon-mod.toml", c_dst / "horizon-mod.toml")
            for sub in ("assets", "data"):
                if (bdir / sub).is_dir():
                    shutil.copytree(bdir / sub, c_dst / sub, dirs_exist_ok=True)

        if "api" in doc:
            api_cfg = doc["api"]
            api_path_rel = api_cfg.get("path", ".")
            api_src = (
                (bdir / api_path_rel / "src")
                if (bdir / api_path_rel / "src").is_dir()
                else (mixin_root / api_path_rel / "src")
            )
            if api_src.is_dir():
                api_dst = work / "api_mods" / mid
                shutil.copytree(api_src, api_dst / "src", dirs_exist_ok=True)
                crate_name = f"horizon_native_mod_{order_idx}_{mid.replace('-', '_')}"
                cargo_toml = (
                    f'[package]\nname = "{crate_name}"\nversion = "{info["version"]}"\n'
                    f'edition = "2024"\npublish = false\n\n[dependencies]\n'
                    f'horizon-client = {{ path = "../../crates/horizon-client", default-features = false }}\n'
                    f'serde = {{ version = "1", features = ["derive"] }}\n'
                    f'serde_json = "1"\nglam = "0.30"\nlog = "0.4"\ntoml = "1.1"\n'
                )
                (api_dst / "Cargo.toml").write_text(cargo_toml, encoding="utf-8")
                (api_dst / "entry.json").write_text(
                    json.dumps({"id": mid, "crate": crate_name, "entry": api_cfg.get("entry", "hooks")}),
                    encoding="utf-8",
                )

        injects = [inj for inj in doc.get("inject", []) if "trait_method" not in inj]
        copy_dir = work / "mods" / mid
        shutil.copytree(mixin_root, copy_dir, dirs_exist_ok=True)
        raw_toml = (copy_dir / "horizon-mixin.toml").read_text(encoding="utf-8")
        raw_toml = re.sub(
            r'^horizon_revision\s*=\s*"[0-9a-f]*"',
            f'horizon_revision = "{commit}"',
            raw_toml,
            flags=re.M,
        )
        raw_toml = re.sub(r"\n\[(?:dependencies|api)\][^\[]*", "\n", raw_toml, flags=re.S)
        (copy_dir / "horizon-mixin.toml").write_text(raw_toml, encoding="utf-8")
        if not injects:
            (copy_dir / ".api_only").write_text("1", encoding="utf-8")


def wire_api(work: pathlib.Path, build: pathlib.Path):
    api_dir = work / "api_mods"
    if not api_dir.is_dir() or not any(api_dir.iterdir()):
        return
    dst_root = build / "native-api"
    shutil.rmtree(dst_root, ignore_errors=True)
    shutil.copytree(api_dir, dst_root)

    entries = []
    for sub in sorted(dst_root.iterdir()):
        if (sub / "entry.json").is_file():
            entries.append((sub.name, json.loads((sub / "entry.json").read_text())))
    if not entries:
        return

    android_cargo = build / "crates/horizon-android/Cargo.toml"
    cargo_txt = android_cargo.read_text(encoding="utf-8")
    dep_lines = "\n".join(
        f'{meta["crate"]} = {{ path = "../../native-api/{folder}" }}'
        for folder, meta in entries
    )
    cargo_txt = cargo_txt.replace("[dependencies]\n", f"[dependencies]\n{dep_lines}\n", 1)
    android_cargo.write_text(cargo_txt, encoding="utf-8")

    am_path = build / "crates/horizon-client/src/android_main.rs"
    am_txt = am_path.read_text(encoding="utf-8")
    ids_lit = ", ".join(f'"{meta["id"]}"' for _, meta in entries)
    hooks_lit = ", ".join(
        f'Box::new({meta["crate"]}::{meta["entry"]}()) as Box<dyn horizon_client::mod_api::Hooks>'
        for _, meta in entries
    )
    install_call = (
        f"horizon_client::mod_api::name_mods(&[{ids_lit}]);\n    "
        f"horizon_client::mod_api::install_runtime_hooks(std::sync::Arc::from(vec![{hooks_lit}]));"
    )
    am_txt = am_txt.replace(
        "horizon_client::mod_api::install_runtime_hooks(Default::default());",
        install_call,
        1,
    )
    am_path.write_text(am_txt, encoding="utf-8")


def write_build_json(build, mods, out, key, commit, profile, mixins):
    sys.path.insert(0, str(pathlib.Path(build) / "packaging/android"))
    from native_mod_permissions import needs

    version = re.search(
        r'^VERSION_NAME = "([^"]+)"',
        (pathlib.Path(build) / "packaging/android/build_apk.py").read_text(),
        re.M,
    ).group(1)
    library = pathlib.Path(out) / "libhorizon_android.so"
    listed = []
    for folder in sorted(pathlib.Path(mods).iterdir()):
        mod = tomllib.loads((folder / "horizon-mixin.toml").read_text())["mod"]
        entry = {"id": mod["id"], "version": mod["version"]}
        wanted = needs(folder)
        if wanted:
            entry["permissions"] = wanted
            print(f"native_build: {mod['id']} needs {', '.join(wanted)}", file=sys.stderr)
        listed.append(entry)
    json.dump(
        {
            "key": key,
            "version": version,
            "commit": commit,
            "profile": profile,
            "mixins": mixins,
            "mods": listed,
            "permissions": sorted({p for m in listed for p in m.get("permissions", [])}),
            "sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
            "size": library.stat().st_size,
        },
        open(pathlib.Path(out) / "build.json", "w"),
        indent=2,
    )


if __name__ == "__main__":
    if sys.argv[1] == "--wire-api":
        wire_api(pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3]))
    elif sys.argv[1] == "--write-build-json":
        write_build_json(*sys.argv[2:])
    else:
        stage_bundles(
            pathlib.Path(sys.argv[1]),
            sys.argv[2],
            [pathlib.Path(p) for p in sys.argv[3:]],
        )
