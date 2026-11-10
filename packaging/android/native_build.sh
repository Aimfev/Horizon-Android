#!/usr/bin/env bash
# Horizon with native mods (docs/horizon-native-mods.md): the mods' horizon-mixins bundles and
# [api] trait mods applied to Horizon's source, built as a release is (build_apk.py, the fast
# profile). Runs on the build VM, locally, and on GitHub's runners (.github/workflows/native-mods.yml).
#
#   native_build.sh SOURCE OUT BUNDLE...
set -euo pipefail

MIXINS_GIT=https://github.com/horizon-rust/horizon-mixins
MIXINS_REV=1e758beab4f00b7ab82183f5b4a7ec4f42599a98

fail() { echo "native_build: $*" >&2; exit 1; }
[ $# -ge 3 ] || fail "usage: native_build.sh SOURCE OUT BUNDLE..."
source=$(cd "$1" && pwd)
mkdir -p "$2"
out=$(cd "$2" && pwd)
shift 2
cache=${NATIVE_CACHE:-$HOME/.cache/horizon-native}
profile=${PROFILE:-fast}
mkdir -p "$cache"

tools=$cache/mixins-${MIXINS_REV:0:12}
if [ ! -x "$tools/bin/horizon-mixins" ]; then
  cargo install --locked --git "$MIXINS_GIT" --rev "$MIXINS_REV" --root "$tools" horizon-mixins-cli
fi

commit=$(git -C "$source" rev-parse HEAD)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

mkdir "$work/source"
git -C "$source" archive --format=tar HEAD | tar -x -C "$work/source"
find "$work/source" -type l -delete
echo "$commit" > "$work/source/.horizon-revision"

manifest=crates/horizon-android/Cargo.toml
lib_line='path = "../horizon-client/src/android_lib.rs"'
grep -qxF "$lib_line" "$work/source/$manifest" || fail "$manifest no longer has: $lib_line"
sed -i "s|^path = \"../horizon-client/src/android_lib.rs\"\$|path = \"plan-stub.rs\"|" "$work/source/$manifest"
: > "$work/source/crates/horizon-android/plan-stub.rs"

lib_attr='#![cfg_attr(not(target_os = "android"), forbid(unsafe_code))]'
lib_attr_alt='#![forbid(unsafe_code)]'
(grep -qxF "$lib_attr" "$work/source/crates/horizon-client/src/lib.rs" || grep -qxF "$lib_attr_alt" "$work/source/crates/horizon-client/src/lib.rs") || fail "crates/horizon-client/src/lib.rs no longer has: $lib_attr"
grep -vxF "$lib_attr" "$work/source/crates/horizon-client/src/lib.rs" | grep -vxF "$lib_attr_alt" > "$work/lib.rs"
cp "$work/lib.rs" "$work/source/crates/horizon-client/src/lib.rs"

mkdir -p "$work/raw_bundles" "$work/mods" "$work/api_mods" "$work/native_data" "$work/native_content"
python3 "$source/packaging/android/native_stage_bundles.py" "$work" "$commit" "$@"
python3 "$source/packaging/android/native_mod_adapt.py" "$work/source" "$work/mods"

key=$(
  {
    echo "$commit $profile $MIXINS_REV"
    (cd "$work/mods" && find . -type f -print0 | sort -z | xargs -0 sha256sum)
  } | sha256sum | cut -c1-16
)
echo "native_build: build $key of $commit with $(ls "$work/mods" | tr '\n' ' ')"

mkdir -p "$work/api_only_mods"
for d in "$work/mods"/*; do
  [ -d "$d" ] || continue
  if [ -f "$d/.api_only" ]; then
    mv "$d" "$work/api_only_mods/"
  fi
done

cargo fetch --locked --manifest-path "$work/source/Cargo.toml" > /dev/null
if [ -n "$(ls -A "$work/mods" 2>/dev/null)" ]; then
  "$tools/bin/horizon-mixins" apply --workspace "$work/source" --mods "$work/mods" --out "$work/stage"
else
  cp -a "$work/source" "$work/stage"
fi

for d in "$work/api_only_mods"/*; do
  [ -d "$d" ] && mv "$d" "$work/mods/"
done

cp "$work/stage_mods.json" "$work/stage/.horizon-mixins-stage.json"

build=$cache/build
mkdir -p "$build"
rsync -r --checksum --delete --exclude /target/ --exclude "/$manifest" --exclude /crates/horizon-android/plan-stub.rs "$work/stage/" "$build/"
git -C "$source" show "HEAD:$manifest" > "$work/manifest"
cmp -s "$work/manifest" "$build/$manifest" || cp "$work/manifest" "$build/$manifest"

python3 "$source/packaging/android/native_stage_bundles.py" --wire-api "$work" "$build"

rm -f "$build"/target/android/horizon-*.apk
(cd "$build" && python3 packaging/android/build_apk.py --profile "$profile" ${NATIVE_LIBRARY_ONLY:+--library-only})

library=$build/target/android/libhorizon_android.so
cp "$library" "$out/libhorizon_android.so"
if [ -d "$work/native_data" ] && [ -n "$(ls -A "$work/native_data" 2>/dev/null)" ]; then
  rm -rf "$out/native-data"
  cp -r "$work/native_data" "$out/native-data"
fi
if [ -d "$work/native_content" ] && [ -n "$(ls -A "$work/native_content" 2>/dev/null)" ]; then
  rm -rf "$out/native-content"
  cp -r "$work/native_content" "$out/native-content"
fi
[ -n "${NATIVE_LIBRARY_ONLY:-}" ] || cp "$build"/target/android/horizon-*-arm64-v8a.apk "$out/"
python3 "$source/packaging/android/native_stage_bundles.py" --write-build-json "$build" "$work/mods" "$out" "$key" "$commit" "$profile" "$MIXINS_REV"
cat "$out/build.json"
