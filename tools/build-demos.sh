#!/usr/bin/env bash
#
# Build the gallery demos into website/src/examples/<slug>/ so that
# `mdbook serve website` (or `mdbook build`) will serve the running WebAssembly
# apps locally at /examples/<slug>/, exactly like the published site does.
#
# Run this whenever you want to (re)generate the in-browser demos, THEN start
# `mdbook serve website`. The book's own pages (landing, docs, gallery) still
# hot-reload under `mdbook serve`; the demo wasm is a prebuilt blob and does not.
# To iterate on a demo's own code, run it natively instead:
#     cargo run -p <slug> --features hmr
#
# Usage:
#     bash tools/build-demos.sh                 # build all demos
#     bash tools/build-demos.sh todomvc horde   # build just these
#
# The built output (large wasm) is git-ignored (see .gitignore:
# /website/src/examples/*/ and /website/src/examples/vendor/).
set -euo pipefail

cd "$(dirname "$0")/.."   # repo root

TARGET=wasm32-unknown-unknown
CARGO_TARGET_ROOT="${CARGO_TARGET_DIR:-target}"
OUT_ROOT=website/src/examples

# slug -> extra `cargo build` args. Keep in sync with examples/gallery.json
# ("build_args"). The stress tests drop default features (bevy_dev_tools' FPS
# overlay needs GPU vertex-storage that WebGL2 lacks).
declare -A BUILD_ARGS=(
  [counter]=""
  [todomvc]=""
  [todomvc_supersolid]=""
  [game_menu]=""
  [citadel]="--no-default-features"
  [horde]="--no-default-features"
)

# Which slugs to build: the args, or all of them.
if [ "$#" -gt 0 ]; then
  slugs=("$@")
else
  slugs=(counter todomvc todomvc_supersolid game_menu citadel horde)
fi

echo "==> ensuring the wasm target is installed"
rustup target add "$TARGET" >/dev/null 2>&1 || true

# wasm-bindgen's CLI must EXACTLY match the wasm-bindgen crate version, or the
# generated glue fails at runtime. Derive the expected version and check the CLI.
expected=""
if command -v python >/dev/null 2>&1; then
  expected=$(cargo metadata --format-version=1 --quiet | python -c \
    "import json,sys; vs=sorted({p['version'] for p in json.load(sys.stdin)['packages'] if p['name']=='wasm-bindgen'}); print(vs[0] if len(vs)==1 else '')" \
    2>/dev/null || true)
fi
if [ -n "$expected" ]; then
  have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
  if [ "$have" != "$expected" ]; then
    echo "!! wasm-bindgen CLI is '${have:-not installed}' but this project needs $expected."
    echo "   Install the exact match, then re-run:"
    echo "       cargo install wasm-bindgen-cli --version $expected"
    exit 1
  fi
  echo "==> wasm-bindgen $expected (CLI matches) OK"
else
  echo "==> (skipping wasm-bindgen version check — python not found)"
fi

echo "==> copying shared code-viewer vendor"
mkdir -p "$OUT_ROOT"
rm -rf "$OUT_ROOT/vendor"
cp -r tools/gallery/vendor "$OUT_ROOT/vendor"

for slug in "${slugs[@]}"; do
  args="${BUILD_ARGS[$slug]-}"
  echo "==> building $slug ${args:+($args) }(release wasm — this can take a while)"
  # shellcheck disable=SC2086  # $args is intentionally word-split (empty or a flag)
  cargo build -p "$slug" --release --target "$TARGET" $args

  out="$OUT_ROOT/$slug"
  mkdir -p "$out"
  wasm-bindgen --no-typescript --target web \
    --out-dir "$out" --out-name "$slug" \
    "$CARGO_TARGET_ROOT/$TARGET/release/$slug.wasm"
  cargo run -q -p xtask -- host-page --slug "$slug" --out "$out"
  rm -rf "$out/assets"
  cp -r "examples/$slug/assets" "$out/assets"
  # The landing page embeds counter via a canvas-only host page (no code viewer).
  if [ "$slug" = "counter" ]; then
    cp "examples/counter/web-embed.html" "$out/embed.html"
  fi
  echo "    -> $out"
done

echo
echo "Done. Now run:  mdbook serve website"
echo "Then open:      http://localhost:3000/examples/<slug>/"
