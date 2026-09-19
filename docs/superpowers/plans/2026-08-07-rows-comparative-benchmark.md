# Rows Comparative Benchmark Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `examples/rows` — the js-framework-benchmark rows workload on superui, in two backends — and publish per-op latency numbers at `docs/BENCHMARKS.md` that can be set directly beside bevy-react's, extracting the shared bench harness into `crates/superui_bench_support` on the way so rows is not a third copy.

**Architecture:** A headless, deterministic Bevy app drives the standard rows workload through two backends over identical markup and one shared stylesheet: `vanilla` (plain DOM via `superui_api`) and `supersolid` (the same app in TSX, rows rendered by `<For>`). Each op is triggered by a real synthetic click through `superui_bridge::events::click_effect`, then `app.update()` is stepped until a frame applies zero DOM mutations; the summed frame time is the op's `Total`. Reporting is two passes that are never blended: an untraced pass producing the citable `Total`, and a `bevy/trace` pass producing the per-stage breakdown via the tracing busy-time attributor lifted out of citadel.

**Tech Stack:** Rust, Bevy 0.19, `superui`/`superui_api`/`superui_bridge`/`superui_css`, `supersolid` (TSX → JS at build time), Boa, `tracing`/`tracing-subscriber`, `dhat` (optional).

**Spec:** [2026-08-07-rows-comparative-benchmark-design.md](../specs/2026-08-07-rows-comparative-benchmark-design.md)

## Global Constraints

- **Bevy 0.19** (`bevy = { workspace = true }`); all new crates use `edition.workspace = true`, `version.workspace = true`.
- **`crates/superui_bench_support` MUST set `publish = false`.** It is a `crates/*` workspace member and must never reach crates.io.
- **DO NOT use git worktrees.** The `target/` directory is huge; a worktree would create a second one. (Project CLAUDE.md.)
- **Determinism is non-negotiable:** fixed seed, seeded PRNG in JS for row labels, no `Math.random()`, no wall-clock in app code.
- **The comparability contract (spec §2) is frozen.** Op names, op order, the 1k/10k scales, and keyed-only rendering are not implementation choices. Changing any of them invalidates comparison against published tables.
- **Ops, exact names and order:** `create`, `append1`, `append1k`, `insert1`, `insertEvery2nd`, `updateText1`, `updateTextEvery2nd`, `updateColor1`, `updateColorEvery2nd`, `swap1`, `swapEvery2nd`, `remove1`, `removeEvery2nd`, `clear`.
- **The `Every2nd` rule: every `*Every2nd` op performs exactly N/2 operations** at N
  rows — 500 at 1k, 5000 at 10k. `updateTextEvery2nd` updates N/2 rows,
  `updateColorEvery2nd` recolors N/2, `removeEvery2nd` removes N/2 (leaving N/2),
  `insertEvery2nd` inserts N/2 (leaving 3N/2, **not** 2N), and `swapEvery2nd`
  performs N/2 swaps of adjacent disjoint pairs (0,1),(2,3),…  An op that does N/4
  or N operations instead publishes a number a reader cannot compare, so this rule
  is part of the contract and must be restated in `docs/BENCHMARKS.md`.
- **`<For>` only.** `<Keyed>` is forbidden here — its DOM order is append/remove (`crates/supersolid_runtime/src/render.js:308`), which renders `swap1`/`insertEvery2nd` in the wrong order. `<Index>` is out of scope.
- **Both backends share one stylesheet verbatim** and produce the same element tree. Otherwise the `Flair cascade` column differs for reasons unrelated to the framework.
- **All benchmark runs use `--release`.** Never compare a debug number against a release one.
- **Any `--profile` run needs BOTH `bevy/trace` AND `bevy/debug`.** `trace` creates the
  per-system spans; `debug` makes their names resolvable. With `trace` alone every name
  reads `<Enable the debug feature to see the name>` (`bevy_utils-0.19.0/src/debug_info.rs:11`)
  and the entire frame collapses into the `other` bucket — a report that looks populated
  but attributes nothing. This is not optional and must be stated in `docs/BENCHMARKS.md`,
  since a reader reproducing the numbers with `bevy/trace` alone would get a useless table.
- **Commit after every task.** Commit summary says what was done; the body says *why*, not a restatement of the diff (project CLAUDE.md).

---

## File Structure

**New — `crates/superui_bench_support/`** (the generic half; never sees an example's config type)

| file | responsibility |
|---|---|
| `Cargo.toml` | `publish = false`; deps `bevy`, optional `dhat`; feature `dhat-prof` |
| `src/lib.rs` | module decls + re-exports |
| `src/stats.rs` | `Stats`, `stats_from` — moved byte-identical |
| `src/profile.rs` | the whole per-system tracing busy-time attributor: layer, buckets, stage table. Takes `impl FnOnce() -> App`. |
| `src/alloc.rs` | `AllocReport`, `alloc_table`, `run_alloc_with` |
| `src/cli.rs` | `BenchArgs`, `parse_args` — `backend` stays a `String` for the example to map |

**New — `examples/rows/`**

| file | responsibility |
|---|---|
| `Cargo.toml` | bins `rows` + `rows-bench`; features `bench`, `dhat-prof` |
| `build.rs` | `supersolid::build::transpile_dir("assets/ui/rows_solid")` |
| `assets/ui/rows_vanilla/{index.html,app.js,rows.css}` | vanilla backend |
| `assets/ui/rows_solid/{index.html,app.tsx,rows.css}` | supersolid backend; `rows.css` byte-identical to vanilla's |
| `src/lib.rs` | crate root, `pub mod bench` |
| `src/main.rs` | windowed `rows` bin (dev convenience) |
| `src/bench/mod.rs` | `Backend`, `build_bench_app`, `Op`, `OpResult`, the op driver + quiescence loop |
| `src/bench/report.rs` | untraced pass / traced pass table + JSON formatting (rows-specific, stays local per spec §6) |
| `src/bin/bench.rs` | `rows-bench` CLI entry |
| `tests/ops.rs` | per-op DOM-state assertions, run against both backends |
| `benchmark.md` | how to run |

**Modified**

| file | change |
|---|---|
| `examples/citadel/src/bench/mod.rs` | delete `Stats`/`stats_from`; re-export from support crate |
| `examples/citadel/src/bench/profile.rs` | delete; `run_profile` becomes a thin wrapper |
| `examples/horde/src/bench/mod.rs` | same |
| `examples/horde/src/bench/profile.rs` | delete; `run_profile` keeps the god-mode system, delegates the rest |
| `docs/BENCHMARKS.md` | **new** — published results |

---

## Task 1: Rows crate skeleton, vanilla app, and the 10k feasibility spike

The 10k question (spec §9.3) is front-loaded deliberately: if a 10k-row tree cannot be built headlessly, the reporting shape in Tasks 11–13 changes, and we want that answer before extracting anything.

**Files:**
- Create: `examples/rows/Cargo.toml`
- Create: `examples/rows/src/lib.rs`
- Create: `examples/rows/src/bench/mod.rs`
- Create: `examples/rows/assets/ui/rows_vanilla/index.html`
- Create: `examples/rows/assets/ui/rows_vanilla/app.js`
- Create: `examples/rows/assets/ui/rows_vanilla/rows.css`
- Test: `examples/rows/tests/spike_10k.rs`

**Interfaces:**
- Consumes: `superui::prelude::{SuperUiPlugin, SuperUiRoot}`; `superui_bridge::UiRuntime`.
- Produces: `rows::bench::{Backend, build_bench_app}`.
  - `pub enum Backend { Vanilla, Supersolid }`, `Backend::label(self) -> &'static str`, `Backend::asset_dir(self) -> &'static str`
  - `pub fn build_bench_app(backend: Backend) -> App`

- [ ] **Step 1: Create the crate manifest**

`examples/rows/Cargo.toml`:

```toml
[package]
name = "rows"
edition.workspace = true
version.workspace = true
license.workspace = true
publish = false
default-run = "rows"

[lib]
name = "rows"
path = "src/lib.rs"

[[bin]]
name = "rows"
path = "src/main.rs"

[[bin]]
name = "rows-bench"
path = "src/bin/bench.rs"
required-features = ["bench"]

[features]
default = []
bench = []
dhat-prof = ["dep:dhat", "superui_bench_support/dhat-prof"]

[dependencies]
bevy = { workspace = true, default-features = true }
superui = { path = "../../crates/superui" }
superui_css = { path = "../../crates/superui_css" }
superui_bridge = { path = "../../crates/superui_bridge" }
superui_dom = { path = "../../crates/superui_dom" }

[dependencies.dhat]
optional = true
version = "0.3"

[build-dependencies]
supersolid = { path = "../../crates/supersolid" }
```

Note: `src/main.rs`, `src/bin/bench.rs`, `build.rs` and the `superui_bench_support` dependency arrive in later tasks. To keep this task compiling on its own, temporarily comment out the two `[[bin]]` blocks and the `[build-dependencies]` section; Task 8 restores them. Add a `# TASK-1 TEMP` marker comment on each so they are easy to find.

- [ ] **Step 2: Write the vanilla app markup**

`examples/rows/assets/ui/rows_vanilla/index.html`. One button per op, `id="op-<name>"` — the bench locates buttons by these ids, so they must match the frozen op names exactly.

```html
<!doctype html>
<html>
  <head>
    <link rel="stylesheet" href="rows.css" />
    <script src="app.js"></script>
  </head>
  <body>
    <div id="main">
      <div class="jumbotron">
        <button id="op-create">create</button>
        <button id="op-append1">append1</button>
        <button id="op-append1k">append1k</button>
        <button id="op-insert1">insert1</button>
        <button id="op-insertEvery2nd">insertEvery2nd</button>
        <button id="op-updateText1">updateText1</button>
        <button id="op-updateTextEvery2nd">updateTextEvery2nd</button>
        <button id="op-updateColor1">updateColor1</button>
        <button id="op-updateColorEvery2nd">updateColorEvery2nd</button>
        <button id="op-swap1">swap1</button>
        <button id="op-swapEvery2nd">swapEvery2nd</button>
        <button id="op-remove1">remove1</button>
        <button id="op-removeEvery2nd">removeEvery2nd</button>
        <button id="op-clear">clear</button>
      </div>
      <div class="table" id="tbody"></div>
    </div>
  </body>
</html>
```

- [ ] **Step 3: Write the shared stylesheet**

`examples/rows/assets/ui/rows_vanilla/rows.css`. Kept small and selector-shaped so the flair cascade has real work matching classes — this file is copied verbatim into `rows_solid/` in Task 8, and a test asserts the two stay identical.

```css
#main { display: flex; flex-direction: column; }
.jumbotron { display: flex; flex-direction: row; }
.table { display: flex; flex-direction: column; }
.row { display: flex; flex-direction: row; height: 20px; }
.col-md-1 { width: 40px; }
.col-md-4 { width: 300px; }
.col-md-6 { width: 200px; }
.lbl { color: #dddddd; }
.lbl.warm { color: #ff8844; }
.lbl.cool { color: #4488ff; }
.remove { color: #aa3333; }
.row.danger { background-color: #331111; }
```

- [ ] **Step 4: Write the vanilla rows app**

`examples/rows/assets/ui/rows_vanilla/app.js`. The row shape is js-framework-benchmark's, with `<div>` where `<table>/<tr>/<td>` would be (spec §3.1). **8 elements + 2 text nodes per row** — this number is published, so changing the markup changes the published `Nodes` column.

```js
// Seeded PRNG (mulberry32). No Math.random anywhere — runs must be reproducible.
var _seed = 1;
function srand(s) { _seed = s >>> 0; }
function rnd(n) {
  _seed |= 0; _seed = (_seed + 0x6D2B79F5) | 0;
  var t = Math.imul(_seed ^ (_seed >>> 15), 1 | _seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) % n;
}

var ADJ = ["pretty","large","big","small","tall","short","long","handsome","plain","quaint","clean","elegant","easy","angry","crazy","helpful","mushy","odd","unsightly","adorable"];
var COL = ["red","yellow","blue","green","pink","brown","purple","white","black","orange"];
var NOU = ["table","chair","house","bbq","desk","car","pony","cookie","sandwich","burger","pizza","mouse","keyboard"];

var nextId = 1;
var rows = [];              // [{id, label, node}]
var tbody = null;

function label() { return ADJ[rnd(20)] + " " + COL[rnd(10)] + " " + NOU[rnd(13)]; }

function buildRow(item) {
  var row = document.createElement("div");
  row.setAttribute("class", "row");
  row.setAttribute("data-id", String(item.id));

  var c1 = document.createElement("div");
  c1.setAttribute("class", "col-md-1");
  c1.textContent = String(item.id);

  var c2 = document.createElement("div");
  c2.setAttribute("class", "col-md-4");
  var a1 = document.createElement("a");
  a1.setAttribute("class", "lbl");
  a1.textContent = item.label;
  c2.appendChild(a1);

  var c3 = document.createElement("div");
  c3.setAttribute("class", "col-md-1");
  var a2 = document.createElement("a");
  a2.setAttribute("class", "remove");
  // The reference row nests a glyph span inside the remove anchor. It carries no
  // text, but it is the 8th element and dropping it would make our Nodes column
  // incomparable with every other published number on this workload.
  var s1 = document.createElement("span");
  s1.setAttribute("class", "glyphicon");
  a2.appendChild(s1);
  c3.appendChild(a2);

  var c4 = document.createElement("div");
  c4.setAttribute("class", "col-md-6");

  row.appendChild(c1); row.appendChild(c2); row.appendChild(c3); row.appendChild(c4);
  item.node = row;
  item.lbl = a1;
  return row;
}

function make(n) {
  var out = [];
  for (var i = 0; i < n; i++) out.push({ id: nextId++, label: label() });
  return out;
}

function appendAll(items) {
  for (var i = 0; i < items.length; i++) tbody.appendChild(buildRow(items[i]));
  for (var j = 0; j < items.length; j++) rows.push(items[j]);
}

var OPS = {
  create: function () { clear(); appendAll(make(1000)); },
  create10k: function () { clear(); appendAll(make(10000)); },
  append1: function () { appendAll(make(1)); },
  append1k: function () { appendAll(make(1000)); },
  insert1: function () {
    var it = make(1)[0];
    var node = buildRow(it);
    tbody.insertBefore(node, rows.length ? rows[0].node : null);
    rows.splice(0, 0, it);
  },
  // Rebuild the row array in ONE pass. Do NOT splice inside the loop: each splice is
  // O(n), so N/2 of them is O(n²) — at 10k that measured 7.4 s against supersolid's
  // 3.1 s, a gap that is array bookkeeping rather than DOM cost. The reference
  // js-framework-benchmark vanilla entry deliberately avoids loop-splicing for the
  // same reason. The DOM work here is identical either way.
  insertEvery2nd: function () {
    var out = [];
    for (var i = 0; i < rows.length; i++) {
      if (i % 2 === 0) {
        var it = make(1)[0];
        tbody.insertBefore(buildRow(it), rows[i].node);
        out.push(it);
      }
      out.push(rows[i]);
    }
    rows = out;
  },
  updateText1: function () {
    if (!rows.length) return;
    rows[0].label = label();
    rows[0].lbl.textContent = rows[0].label;
  },
  updateTextEvery2nd: function () {
    for (var i = 0; i < rows.length; i += 2) {
      rows[i].label = label();
      rows[i].lbl.textContent = rows[i].label;
    }
  },
  updateColor1: function () {
    if (!rows.length) return;
    rows[0].lbl.setAttribute("class", "lbl warm");
  },
  updateColorEvery2nd: function () {
    for (var i = 0; i < rows.length; i += 2) rows[i].lbl.setAttribute("class", "lbl warm");
  },
  swap1: function () {
    if (rows.length < 999) return;
    swap(1, 998);
  },
  swapEvery2nd: function () {
    // Stride 2, not 4: every *Every2nd* op performs N/2 operations (see Global
    // Constraints). Adjacent disjoint pairs (0,1),(2,3),... = 500 swaps at 1k.
    for (var i = 0; i + 1 < rows.length; i += 2) swap(i, i + 1);
  },
  remove1: function () {
    if (!rows.length) return;
    rows[0].node.remove();
    rows.splice(0, 1);
  },
  // One pass, no loop-splicing — see the note on insertEvery2nd. Removes the
  // even-indexed rows, keeping 1, 3, 5, … exactly as before.
  removeEvery2nd: function () {
    var out = [];
    for (var i = 0; i < rows.length; i++) {
      if (i % 2 === 0) {
        // TEMP WORKAROUND (see remove1): Element.prototype.remove() is unbound.
        // TODO: restore `rows[i].node.remove();` once Element.remove() is bound.
        tbody.removeChild(rows[i].node);
      } else {
        out.push(rows[i]);
      }
    }
    rows = out;
  },
  clear: clear,
};

function swap(a, b) {
  var ra = rows[a], rb = rows[b];
  var afterB = rb.node.nextSibling;
  tbody.insertBefore(rb.node, ra.node);
  tbody.insertBefore(ra.node, afterB);
  rows[a] = rb; rows[b] = ra;
}

function clear() {
  for (var i = 0; i < rows.length; i++) rows[i].node.remove();
  rows = [];
}

function boot() {
  tbody = document.getElementById("tbody");
  srand(1);
  var names = Object.keys(OPS);
  for (var i = 0; i < names.length; i++) {
    (function (name) {
      var btn = document.getElementById("op-" + name);
      if (btn) btn.addEventListener("click", function () { OPS[name](); });
    })(names[i]);
  }
}

boot();
```

Note `create10k` has no button in `index.html` and is not a measured op — it is the precondition builder the driver uses to reach 10k rows. Task 9 adds its button.

- [ ] **Step 5: Write the crate root and minimal bench module**

`examples/rows/src/lib.rs`:

```rust
//! The js-framework-benchmark rows workload on superui, in two backends.
//! See `benchmark.md` and `docs/BENCHMARKS.md`.

pub mod bench;
```

`examples/rows/src/bench/mod.rs`:

```rust
//! Headless, deterministic harness for the rows workload.

use std::time::Duration;

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::image::TextureAtlasPlugin;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::text::TextPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::UiPlugin;

use superui::prelude::{SuperUiPlugin, SuperUiRoot};

const V_HTML: &str = include_str!("../../assets/ui/rows_vanilla/index.html");
const V_CSS: &str = include_str!("../../assets/ui/rows_vanilla/rows.css");
const V_JS: &str = include_str!("../../assets/ui/rows_vanilla/app.js");

/// Fixed per-update time step: exactly one FixedUpdate tick per app.update().
pub const DT: f64 = 1.0 / 60.0;

/// Which UI backend drives the identical rows markup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Vanilla,
    Supersolid,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Vanilla => "vanilla",
            Backend::Supersolid => "supersolid",
        }
    }
    pub fn asset_dir(self) -> &'static str {
        match self {
            Backend::Vanilla => "ui/rows_vanilla",
            Backend::Supersolid => "ui/rows_solid",
        }
    }
}

fn memory_asset_dir(backend: Backend) -> Dir {
    let dir = Dir::new("assets".into());
    match backend {
        Backend::Vanilla => {
            dir.insert_asset("ui/rows_vanilla/index.html".as_ref(), V_HTML.as_bytes());
            dir.insert_asset("ui/rows_vanilla/rows.css".as_ref(), V_CSS.as_bytes());
            dir.insert_asset("ui/rows_vanilla/app.js".as_ref(), V_JS.as_bytes());
        }
        Backend::Supersolid => unimplemented!("supersolid assets land in Task 8"),
    }
    dir
}

/// Build a finished, headless, deterministic bench app for `backend`.
pub fn build_bench_app(backend: Backend) -> App {
    let mut app = App::new();

    let dir = memory_asset_dir(backend);
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );

    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        StatesPlugin,
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();

    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)));
    app.insert_resource(Time::<Fixed>::from_seconds(DT));

    app.add_plugins(SuperUiPlugin);

    let dir_name = backend.asset_dir();
    app.add_systems(Startup, move |mut commands: Commands, assets: Res<AssetServer>| {
        commands.spawn(SuperUiRoot::from_asset_dir(dir_name, &assets));
    });

    app.finish();
    app
}

/// Boot chrome: DOM elements present with ZERO rows. Backend-dependent — the
/// supersolid app mounts inside a `<div id="root">` (the convention every other
/// supersolid example uses) that the vanilla app has no equivalent of, so chrome is
/// 24 vs 23. Constant and op-independent, so it does not affect timings, but the
/// published `Nodes` column exists for cross-implementation normalisation and must
/// therefore be chrome-free: report `dom_node_count - chrome_node_count`, never raw.
pub fn chrome_node_count(backend: Backend) -> usize {
    let mut app = build_bench_app(backend);
    for _ in 0..30 {
        app.update();
    }
    dom_node_count(&app)
}

/// Number of DOM elements the app contains, via the live `UiRuntime`.
pub fn dom_node_count(app: &App) -> usize {
    app.world()
        .get_non_send::<superui_bridge::UiRuntime>()
        .map(|rt| {
            let d = rt.dom.borrow();
            d.query_selector_all(d.document(), "*").len()
        })
        .unwrap_or(0)
}
```

- [ ] **Step 6: Write the failing 10k spike test**

`examples/rows/tests/spike_10k.rs`. This is the spec §9.3 risk probe. It drives the app to 10 000 rows through the real JS path and asserts the tree materialized.

```rust
//! Spec §9.3 risk probe: does a 10k-row tree survive headless bevy_ui?
//! Front-loaded because a negative answer reshapes the published tables.

use rows::bench::{build_bench_app, dom_node_count, Backend};
use superui_bridge::{PendingDomEvents, UiRuntime};

/// Click the button with `id`, by pushing a DOM click the same way the picking
/// observer would. Returns false when the button is not in the DOM yet.
fn click(app: &mut bevy::prelude::App, id: &str) -> bool {
    let node = {
        let Some(rt) = app.world().get_non_send::<UiRuntime>() else { return false };
        let d = rt.dom.borrow();
        let Some(n) = d.query_selector(d.document(), &format!("#{id}")) else { return false };
        n
    };
    let rt = app.world().get_non_send::<UiRuntime>().unwrap();
    let mut pending = app.world_mut().resource_mut::<PendingDomEvents>();
    superui_bridge::events::click_effect(rt, node, &mut pending);
    true
}

#[test]
fn ten_thousand_rows_materialize() {
    let mut app = build_bench_app(Backend::Vanilla);
    // Mount: give the asset load + first render a few frames.
    for _ in 0..20 {
        app.update();
    }
    assert!(click(&mut app, "op-create10k"), "op-create10k button not found");
    for _ in 0..10 {
        app.update();
    }
    let n = dom_node_count(&app);
    // 10 000 rows x 8 elements = 80 000, plus chrome.
    assert!(n > 80_000, "expected >80000 DOM elements at 10k rows, got {n}");
}
```

- [ ] **Step 7: Run the spike and record the answer**

Run: `cargo test --release -p rows --test spike_10k -- --nocapture`

Expected: FAIL on the missing `op-create10k` button (it has no `<button>` yet).

Add `<button id="op-create10k">create10k</button>` to `index.html`'s jumbotron, then re-run.

**This step's output is a decision, not just a pass/fail.** Record the result in the task's commit message:
- **Passes** → proceed unchanged; note the observed node count and wall time.
- **Fails / hangs / OOMs** → do **not** work around it silently. Record what happened, then continue the plan with 1k only, and Task 13 publishes the 10k table as "did not complete", with the failure documented (spec §9.3). Raise it before starting Task 2.

- [ ] **Step 8: Commit**

```bash
git add examples/rows
git commit -m "feat(rows): add the js-framework-benchmark rows workload, vanilla backend

The existing horde and citadel benchmarks measure steady-state per-frame
cost on bespoke workloads, so their numbers are only meaningful against
themselves. This adds the standard rows workload, whose whole purpose is
to produce a figure comparable with other frameworks' published numbers.

The 10k spike test leads because a 10k-row tree failing headlessly would
change the shape of the published tables, and that is worth knowing
before any harness extraction work."
```

---

## Task 2: Capture the pre-extraction baselines

The extraction in Tasks 3–7 is a refactor of two examples whose published numbers back existing writeups. "It compiles" is not evidence. This task captures the "before" side of the gate.

**Files:**
- Create: `.superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/` (scratch — **not** committed)

**Interfaces:**
- Consumes: nothing.
- Produces: baseline JSON files consumed by Tasks 6 and 7.

- [ ] **Step 1: Capture citadel's baseline**

```bash
mkdir -p .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines
cd /home/tim/bevy_superui
for i in 1 2 3; do
  cargo run --release -p citadel --features bench --bin citadel-bench -- \
    --backend supersolid --frames 120 --warmup 200 --format json \
    > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-$i.json
done
cat .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-*.json
```

- [ ] **Step 2: Capture horde's baseline**

```bash
for i in 1 2 3; do
  cargo run --release -p horde --features bench --bin horde-bench -- \
    --backend supersolid --frames 300 --warmup 300 --format json \
    > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-$i.json
done
cat .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-*.json
```

- [ ] **Step 3: Capture both table-format baselines**

The JSON gate covers numbers; this covers formatting, which must not drift.

```bash
cargo run --release -p citadel --features bench --bin citadel-bench -- \
  --backend supersolid --frames 120 --warmup 200 \
  > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel.txt
cargo run --release -p horde --features bench --bin horde-bench -- \
  --backend supersolid --frames 300 --warmup 300 \
  > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde.txt
```

- [ ] **Step 4: Record the gate criteria**

Write `.superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/GATE.md` stating the pass conditions used in Tasks 6 and 7:

```markdown
# Extraction gate

Pass conditions, checked after each migration:

1. **JSON key set and order byte-identical.** `jq -S 'keys' before == after`, and the
   raw key order in the emitted string is unchanged. A new/renamed/reordered key
   fails the gate outright — it means the shared formatter changed the contract.
2. **Table output structurally identical.** `diff <(sed 's/[0-9.]\+/N/g' before.txt)
   <(sed 's/[0-9.]\+/N/g' after.txt)` is empty — same lines, same labels, same
   layout, numbers masked.
3. **Numbers within noise.** `total_mean_ms` and `ui_ms` medians of 3 runs within
   +/-10% of the baseline medians. Timing is timing; 10% is the noise band, not a
   licence to drift.

Any failure means the extraction is wrong. Fix it before proceeding — do not
re-baseline to make the gate pass.

**Keep these files until Task 12 is done.** Task 12 refactors `run_profile_with`
into a wrapper over `run_profile_driven`, changing the code Tasks 6-7 certified,
and re-runs this gate against *these* pre-extraction baselines. Do not regenerate
them in between — a baseline taken after the extraction would only prove the
extraction agrees with itself.
```

- [ ] **Step 5: Commit**

Nothing to commit (scratch files only). Confirm the working tree is clean:

```bash
git status --porcelain
```

Expected: empty output.

---

## Task 3: Create `superui_bench_support` with the stats module

**Files:**
- Create: `crates/superui_bench_support/Cargo.toml`
- Create: `crates/superui_bench_support/src/lib.rs`
- Create: `crates/superui_bench_support/src/stats.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct Stats { pub mean_ms: f64, pub p50_ms: f64, pub p95_ms: f64, pub p99_ms: f64, pub fps: f64 }`
  - `pub fn stats_from(samples: Vec<f64>) -> Stats`

- [ ] **Step 1: Create the manifest**

`crates/superui_bench_support/Cargo.toml`. **`publish = false` is mandatory** (global constraints).

```toml
[package]
name = "superui_bench_support"
edition.workspace = true
version.workspace = true
license.workspace = true
publish = false
description = "Shared harness for the superui macro-benchmarks (dev-only, never published)."

[features]
default = []
dhat-prof = ["dep:dhat"]

[dependencies]
bevy = { workspace = true, default-features = true }

[dependencies.dhat]
optional = true
version = "0.3"
```

- [ ] **Step 2: Write the failing test**

`crates/superui_bench_support/src/stats.rs` — test first, moved verbatim from `examples/citadel/src/bench/mod.rs:543-551` so a behaviour change would be caught:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_on_known_data() {
        let samples: Vec<f64> = (1..=100).map(|n| n as f64).collect();
        let s = stats_from(samples);
        assert!((s.mean_ms - 50.5).abs() < 1e-9);
        assert_eq!(s.p50_ms, 51.0);
        assert_eq!(s.p95_ms, 95.0);
        assert_eq!(s.p99_ms, 99.0);
        assert!((s.fps - 1000.0 / 50.5).abs() < 1e-9);
    }

    #[test]
    #[should_panic(expected = "empty samples")]
    fn empty_samples_panic() {
        stats_from(Vec::new());
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p superui_bench_support`
Expected: FAIL — `cannot find function stats_from`.

- [ ] **Step 4: Write the implementation**

Prepend to `crates/superui_bench_support/src/stats.rs` — byte-identical to citadel's `mod.rs:119-146`:

```rust
//! Per-frame timing statistics. Moved verbatim from the horde/citadel harnesses;
//! identical in both, so this is a pure de-duplication with no behaviour change.

/// Per-frame timing statistics from a benchmark run.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub fps: f64,
}

/// Nearest-rank percentile over sorted samples.
pub fn stats_from(mut samples: Vec<f64>) -> Stats {
    assert!(!samples.is_empty(), "stats_from: empty samples");
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = samples.len();
    let pct = |p: f64| {
        let idx = ((n - 1) as f64 * p).round() as usize;
        samples[idx]
    };
    let mean = samples.iter().sum::<f64>() / n as f64;
    Stats {
        mean_ms: mean,
        p50_ms: pct(0.50),
        p95_ms: pct(0.95),
        p99_ms: pct(0.99),
        fps: if mean > 0.0 { 1000.0 / mean } else { f64::INFINITY },
    }
}
```

`crates/superui_bench_support/src/lib.rs`:

```rust
//! Shared harness for the superui macro-benchmarks (horde, citadel, rows).
//!
//! Everything here is generic over the example: the app-dependent half is passed
//! in as an `impl FnOnce() -> App`, so this crate never sees an example's config
//! type. Report formatting deliberately stays in each example — the three
//! benchmarks print genuinely different things and unifying them would need a
//! config knob per divergence.

pub mod stats;

pub use stats::{stats_from, Stats};
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p superui_bench_support`
Expected: PASS, 2 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bench_support
git commit -m "feat(bench-support): add the shared macro-benchmark crate

horde and citadel each carry a full copy of the same benchmark harness,
and rows would have made three. The copies are not textually identical --
they are one design re-typed against a different config type -- so the
crate is built around passing the app-building closure in, which is what
lets the generic half exist without knowing any example's config.

Starts with stats, the one part that is already byte-identical in both,
so the first migration step carries no behaviour risk."
```

---

## Task 4: Move the tracing profiler into the support crate

This is the bulk of the duplication (~350 lines × 2, ~95% identical). The generic version takes an app-building closure and a rebuild hint string, which are the only two things that differed.

**Files:**
- Create: `crates/superui_bench_support/src/profile.rs`
- Modify: `crates/superui_bench_support/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub fn run_profile_with(build: impl FnOnce() -> bevy::prelude::App, frames: usize, warmup: usize, rebuild_hint: &str)`
  - `pub fn bucket_for(name: &str) -> Bucket` and `pub enum Bucket { Marshal, BoaRender, Reconcile, Flair, Taffy, UiOther, Other }` (made `pub` so rows' traced pass can reuse the mapping)

- [ ] **Step 1: Write the failing test**

Append to `crates/superui_bench_support/src/profile.rs`. The bucket mapping is the part with real logic, and it is what rows' traced pass depends on:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_map_known_system_paths() {
        assert_eq!(bucket_for("supersolid::emit_bevy_inbox_system"), Bucket::BoaRender);
        assert_eq!(bucket_for("superui_bridge::reconcile_system"), Bucket::Reconcile);
        assert_eq!(bucket_for("bevy_flair_style::systems::recalculate"), Bucket::Flair);
        assert_eq!(bucket_for("bevy_ui::layout::ui_layout_system"), Bucket::Taffy);
        assert_eq!(bucket_for("superui_bridge::push_ui_frame"), Bucket::Marshal);
        assert_eq!(bucket_for("bevy_text::text_system"), Bucket::UiOther);
        assert_eq!(bucket_for("my_game::sim::advance"), Bucket::Other);
    }

    #[test]
    fn schedule_runner_wrappers_are_excluded() {
        // These WRAP every other system, so counting them double-counts the frame.
        assert!(is_wrapper("bevy_app::main_schedule::Main::run_main"));
        assert!(is_wrapper("bevy_app::main_schedule::run_fixed_main_schedule"));
        assert!(!is_wrapper("superui_bridge::reconcile_system"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p superui_bench_support profile`
Expected: FAIL — `bucket_for`, `Bucket`, `is_wrapper` not found.

- [ ] **Step 3: Move the implementation**

Copy `examples/citadel/src/bench/profile.rs` lines 32–231 into `crates/superui_bench_support/src/profile.rs` **above** the test module, with exactly these changes:

1. Delete `use crate::bench::{build_bench_app, Backend};` and `use crate::sim::CitadelConfig;`.
2. Make `Bucket`, `bucket_for`, and `is_wrapper` `pub` (rows traced pass consumes them).
3. Keep `Agg`, `SysKey`, `Enter`, `NameVisitor`, `SystemTimingLayer`, `is_system_span`, `install` private.

Then add the generic entry point, replacing citadel's `run_profile` (`profile.rs:235-261`) and `print_report` (`263-353`):

```rust
/// Run a profiled session and print the per-stage attribution.
///
/// Generic over the example: `build` supplies the finished `App`, so this crate
/// never needs the caller's config type. `rebuild_hint` is the exact command to
/// print when no system spans were recorded (i.e. the caller forgot `bevy/trace`).
/// Note `bevy/debug` is separately required for the span NAMES to resolve; without it
/// spans exist but all attribute to `other`.
pub fn run_profile_with(
    build: impl FnOnce() -> App,
    frames: usize,
    warmup: usize,
    rebuild_hint: &str,
) {
    let agg = install();
    let mut app = build();

    for _ in 0..warmup {
        app.update();
    }

    {
        let mut a = agg.lock().unwrap();
        a.per_system.clear();
        a.recording = true;
    }
    let wall = Instant::now();
    for _ in 0..frames {
        app.update();
    }
    let wall_total_ms = wall.elapsed().as_secs_f64() * 1000.0;
    {
        agg.lock().unwrap().recording = false;
    }

    print_report(&agg, frames, wall_total_ms, rebuild_hint);
}
```

In `print_report`, add the `rebuild_hint: &str` parameter and replace the hardcoded citadel command (`profile.rs:271`) with:

```rust
        println!(
            "profile: no system spans were recorded.\n\
             This mode needs Bevy's per-system instrumentation. Rebuild with:\n\
             \n    {rebuild_hint}\n"
        );
```

Everything else in `print_report` is unchanged.

Add to `lib.rs`:

```rust
pub mod profile;

pub use profile::{run_profile_with, Bucket};
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p superui_bench_support`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_bench_support
git commit -m "feat(bench-support): move the per-system tracing profiler in

This is the bulk of the horde/citadel duplication: two ~350-line files
that differ only in a doc-comment command string, the config type, and
horde's god-mode system. Taking the app builder as a closure and the
rebuild hint as a string removes all three differences, so one copy now
serves both, and rows gets its stage breakdown for free.

The bucket mapping is public because rows' traced pass needs the same
system-name to stage mapping to keep its published breakdown consistent
with the other two benchmarks."
```

---

## Task 5: Move the dhat and CLI modules

**Files:**
- Create: `crates/superui_bench_support/src/alloc.rs`
- Create: `crates/superui_bench_support/src/cli.rs`
- Modify: `crates/superui_bench_support/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct AllocReport { pub backend: String, pub frames: usize, pub bytes_per_frame: f64, pub blocks_per_frame: f64 }`
  - `pub fn alloc_table(r: &AllocReport) -> String`
  - `pub fn run_alloc_with(backend: String, build: impl FnOnce() -> App, frames: usize, warmup: usize) -> AllocReport` (behind `dhat-prof`)
  - `pub struct BenchArgs { pub backend: Option<String>, pub caps: Vec<usize>, pub frames: usize, pub warmup: usize, pub seed: u64, pub json: bool, pub dhat: bool, pub profile: bool }`
  - `pub fn parse_args(argv: &[String], defaults: ArgDefaults) -> Result<BenchArgs, String>`
  - `pub struct ArgDefaults { pub frames: usize, pub warmup: usize, pub cap_flags: &'static [&'static str] }`

- [ ] **Step 1: Write the failing tests**

`crates/superui_bench_support/src/alloc.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_table_reports_per_frame_figures() {
        let r = AllocReport {
            backend: "supersolid".to_string(),
            frames: 100,
            bytes_per_frame: 2048.0,
            blocks_per_frame: 12.0,
        };
        let t = alloc_table(&r);
        assert!(t.contains("backend=supersolid"), "{t}");
        assert!(t.contains("2048.0 bytes/frame"), "{t}");
        assert!(t.contains("12.0 allocs/frame"), "{t}");
    }
}
```

`crates/superui_bench_support/src/cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn defaults() -> ArgDefaults {
        ArgDefaults { frames: 1000, warmup: 100, cap_flags: &["--building-count", "--enemy-cap"] }
    }

    #[test]
    fn parses_flags_and_leaves_backend_a_string() {
        let a = parse_args(
            &args(&[
                "--backend", "supersolid", "--sweep", "60,120", "--frames", "500",
                "--warmup", "50", "--seed", "7", "--format", "json",
            ]),
            defaults(),
        )
        .unwrap();
        // Backend stays a String: the variant sets differ per example, so mapping
        // it is the caller's job.
        assert_eq!(a.backend.as_deref(), Some("supersolid"));
        assert_eq!(a.caps, vec![60, 120]);
        assert_eq!(a.frames, 500);
        assert_eq!(a.warmup, 50);
        assert_eq!(a.seed, 7);
        assert!(a.json);
    }

    #[test]
    fn cap_flag_aliases_are_accepted() {
        let a = parse_args(&args(&["--backend", "null", "--enemy-cap", "400"]), defaults()).unwrap();
        assert_eq!(a.caps, vec![400]);
        let b = parse_args(&args(&["--backend", "null", "--building-count", "120"]), defaults()).unwrap();
        assert_eq!(b.caps, vec![120]);
    }

    #[test]
    fn preset_is_tolerated_and_unknown_args_are_not() {
        assert!(parse_args(&args(&["--backend", "null", "--preset", "stress"]), defaults()).is_ok());
        assert!(parse_args(&args(&["--nope"]), defaults()).is_err());
    }

    #[test]
    fn defaults_apply_when_unset() {
        let a = parse_args(&args(&["--backend", "null"]), defaults()).unwrap();
        assert_eq!(a.frames, 1000);
        assert_eq!(a.warmup, 100);
        assert!(a.caps.is_empty(), "caps stay empty so the caller can apply its own default");
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p superui_bench_support`
Expected: FAIL — `AllocReport`, `alloc_table`, `parse_args`, `ArgDefaults` not found.

- [ ] **Step 3: Write the implementations**

Prepend to `crates/superui_bench_support/src/alloc.rs`:

```rust
//! Allocation-churn measurement (dhat). `backend` is a plain String because the
//! three examples have different backend enums.

use bevy::prelude::App;

#[derive(Clone, Debug)]
pub struct AllocReport {
    pub backend: String,
    pub frames: usize,
    pub bytes_per_frame: f64,
    pub blocks_per_frame: f64,
}

#[cfg(feature = "dhat-prof")]
pub fn run_alloc_with(
    backend: String,
    build: impl FnOnce() -> App,
    frames: usize,
    warmup: usize,
) -> AllocReport {
    let mut app = build();
    for _ in 0..warmup {
        app.update();
    }
    let before = dhat::HeapStats::get();
    for _ in 0..frames {
        app.update();
    }
    let after = dhat::HeapStats::get();
    let dbytes = after.total_bytes.saturating_sub(before.total_bytes) as f64;
    let dblocks = after.total_blocks.saturating_sub(before.total_blocks) as f64;
    AllocReport {
        backend,
        frames,
        bytes_per_frame: dbytes / frames as f64,
        blocks_per_frame: dblocks / frames as f64,
    }
}

pub fn alloc_table(r: &AllocReport) -> String {
    format!(
        "alloc churn: backend={} frames={} | {:.1} bytes/frame | {:.1} allocs/frame\n",
        r.backend, r.frames, r.bytes_per_frame, r.blocks_per_frame,
    )
}
```

Prepend to `crates/superui_bench_support/src/cli.rs`:

```rust
//! Shared `--key value` / `--flag` parsing.
//!
//! `backend` is deliberately left as a `String`: horde has a Native backend that
//! citadel and rows do not, so mapping the string to an enum belongs to the caller.

/// Per-example parser defaults. `cap_flags` are the flag spellings that set the
/// size knob (horde: `--enemy-cap`, citadel: `--building-count`, rows: `--rows`).
#[derive(Clone, Copy, Debug)]
pub struct ArgDefaults {
    pub frames: usize,
    pub warmup: usize,
    pub cap_flags: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub struct BenchArgs {
    pub backend: Option<String>,
    pub caps: Vec<usize>,
    pub frames: usize,
    pub warmup: usize,
    pub seed: u64,
    pub json: bool,
    pub dhat: bool,
    pub profile: bool,
}

/// Minimal `--key value` / `--flag` parser.
pub fn parse_args(argv: &[String], defaults: ArgDefaults) -> Result<BenchArgs, String> {
    let mut backend: Option<String> = None;
    let mut caps: Vec<usize> = Vec::new();
    let mut frames = defaults.frames;
    let mut warmup = defaults.warmup;
    let mut seed = 0u64;
    let mut json = false;
    let mut dhat = false;
    let mut profile = false;

    let mut i = 0;
    while i < argv.len() {
        let key = argv[i].as_str();
        let advance = |i: &mut usize| -> Result<&str, String> {
            *i += 1;
            argv.get(*i).map(|s| s.as_str()).ok_or_else(|| format!("missing value for {key}"))
        };
        if defaults.cap_flags.contains(&key) {
            let v = advance(&mut i)?
                .parse()
                .map_err(|_| format!("bad {key}"))?;
            caps = vec![v];
            i += 1;
            continue;
        }
        match key {
            "--backend" => backend = Some(advance(&mut i)?.to_string()),
            // Accepted and ignored, for script compatibility across the examples.
            "--preset" => {
                advance(&mut i)?;
            }
            "--sweep" => {
                caps = advance(&mut i)?
                    .split(',')
                    .map(|s| s.trim().parse::<usize>().map_err(|_| "bad --sweep list".to_string()))
                    .collect::<Result<_, _>>()?;
            }
            "--frames" => frames = advance(&mut i)?.parse().map_err(|_| "bad --frames".to_string())?,
            "--warmup" => warmup = advance(&mut i)?.parse().map_err(|_| "bad --warmup".to_string())?,
            "--seed" => seed = advance(&mut i)?.parse().map_err(|_| "bad --seed".to_string())?,
            "--format" => json = advance(&mut i)? == "json",
            "--dhat" => dhat = true,
            "--profile" => profile = true,
            other => return Err(format!("unknown arg '{other}'")),
        }
        i += 1;
    }

    Ok(BenchArgs { backend, caps, frames, warmup, seed, json, dhat, profile })
}
```

Add to `lib.rs`:

```rust
pub mod alloc;
pub mod cli;

pub use alloc::{alloc_table, AllocReport};
pub use cli::{parse_args, ArgDefaults, BenchArgs};
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p superui_bench_support`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_bench_support
git commit -m "feat(bench-support): move dhat and CLI parsing in

Both were copied between horde and citadel with only the config type and
the size-flag spelling differing. Parameterising the size flag and
leaving --backend as an unmapped String covers every difference without
forcing the three examples to share a backend enum they genuinely do not
share -- horde has a Native backend the others have no meaning for.

parse_args intentionally returns caps empty rather than substituting a
default, because each example's default size differs and silently
inserting the wrong one would change its published numbers."
```

---

## Task 6: Migrate citadel and verify the gate

Citadel first: it is the strict subset, so it is the lower-risk of the two migrations.

**Files:**
- Modify: `examples/citadel/Cargo.toml`
- Modify: `examples/citadel/src/bench/mod.rs`
- Delete: `examples/citadel/src/bench/profile.rs`

**Interfaces:**
- Consumes: `superui_bench_support::{Stats, stats_from, run_profile_with, alloc_table, AllocReport, parse_args, ArgDefaults, BenchArgs}`.
- Produces: unchanged public surface — `citadel::bench::{Backend, build_bench_app, run_report, report_table, report_json, sweep_table, sim_for}` all keep their signatures.

- [ ] **Step 1: Add the dependency**

In `examples/citadel/Cargo.toml`, under `[dependencies]`:

```toml
superui_bench_support = { path = "../../crates/superui_bench_support" }
```

and change the `dhat-prof` feature to forward:

```toml
dhat-prof = ["dep:dhat", "superui_bench_support/dhat-prof"]
```

- [ ] **Step 2: Replace stats with a re-export**

In `examples/citadel/src/bench/mod.rs`, delete the `Stats` struct and `stats_from` (lines 119–146) and the `stats_tests::percentiles_on_known_data` test (now owned by the support crate), then add near the top:

```rust
pub use superui_bench_support::{stats_from, Stats};
```

Keep `stats_tests::timing_run_produces_frames` — it tests citadel's `time_backend`, not the moved stats.

- [ ] **Step 3: Replace the profiler with a wrapper**

Delete `examples/citadel/src/bench/profile.rs` entirely and replace it with:

```rust
//! Per-stage profiling of the supersolid frame (the `--profile` bench mode).
//!
//! The attribution machinery lives in `superui_bench_support::profile`; citadel
//! only supplies the app and the rebuild hint. Citadel is steady-state (no player,
//! no GameState, no death), so unlike horde it needs no god-mode system.

use crate::bench::{build_bench_app, Backend};
use crate::sim::CitadelConfig;

const REBUILD_HINT: &str = "cargo run --release -p citadel --features bench,bevy/trace,bevy/debug --bin citadel-bench -- --profile ...";

pub fn run_profile(cfg: CitadelConfig, frames: usize, warmup: usize) {
    superui_bench_support::run_profile_with(
        || build_bench_app(Backend::Supersolid, cfg),
        frames,
        warmup,
        REBUILD_HINT,
    );
}
```

- [ ] **Step 4: Replace dhat and CLI**

In `mod.rs`, delete `AllocReport`, `run_alloc`, `alloc_table` (lines 442–479) and `BenchArgs`/`parse_args` (lines 367–438) plus the `cli_tests` module (563–596, now owned by the support crate). Replace with:

```rust
pub use superui_bench_support::{alloc_table, AllocReport};

const ARG_DEFAULTS: superui_bench_support::ArgDefaults = superui_bench_support::ArgDefaults {
    frames: 1000,
    warmup: 100,
    cap_flags: &["--building-count", "--enemy-cap"],
};

/// Citadel's arg parsing: shared parser, then map the backend string and apply
/// citadel's own default building_count.
pub fn parse_args(argv: &[String]) -> Result<BenchArgs, String> {
    let mut a = superui_bench_support::parse_args(argv, ARG_DEFAULTS)?;
    if a.caps.is_empty() {
        a.caps = vec![CitadelConfig::default().building_count];
    }
    Ok(a)
}

/// Map the parsed backend string to citadel's enum. `--profile` implies supersolid.
pub fn backend_of(a: &BenchArgs) -> Result<Backend, String> {
    match a.backend.as_deref() {
        Some("null") => Ok(Backend::Null),
        Some("supersolid") => Ok(Backend::Supersolid),
        Some(other) => Err(format!("unknown backend '{other}'")),
        None if a.profile => Ok(Backend::Supersolid),
        None => Err("--backend is required (null|supersolid)".to_string()),
    }
}

pub use superui_bench_support::BenchArgs;
```

Add `#[cfg(feature = "dhat-prof")]` `run_alloc` as a wrapper:

```rust
#[cfg(feature = "dhat-prof")]
pub fn run_alloc(backend: Backend, cfg: CitadelConfig, frames: usize, warmup: usize) -> AllocReport {
    superui_bench_support::alloc::run_alloc_with(
        backend.label().to_string(),
        || build_bench_app(backend, cfg),
        frames,
        warmup,
    )
}
```

- [ ] **Step 5: Update the bin for the new backend mapping**

In `examples/citadel/src/bin/bench.rs`, `parse_args` now returns a `BenchArgs` whose `backend` is a `String`. Replace uses of `args.backend` with `bench::backend_of(&args)?`. Read the file and adjust the call sites — the rest of the bin is unchanged.

- [ ] **Step 6: Verify it builds and unit tests pass**

```bash
cargo test --release -p citadel --features bench
```

Expected: PASS.

- [ ] **Step 7: Run the gate**

```bash
cd /home/tim/bevy_superui
for i in 1 2 3; do
  cargo run --release -p citadel --features bench --bin citadel-bench -- \
    --backend supersolid --frames 120 --warmup 200 --format json \
    > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-after-$i.json
done
cargo run --release -p citadel --features bench --bin citadel-bench -- \
  --backend supersolid --frames 120 --warmup 200 \
  > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-after.txt

# Gate 1: JSON keys identical (order included)
diff <(grep -o '"[a-z_]*":' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-1.json) \
     <(grep -o '"[a-z_]*":' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-after-1.json) \
  && echo "GATE 1 PASS" || echo "GATE 1 FAIL"

# Gate 2: table structurally identical (numbers masked)
diff <(sed 's/[0-9.]\+/N/g' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel.txt) \
     <(sed 's/[0-9.]\+/N/g' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-after.txt) \
  && echo "GATE 2 PASS" || echo "GATE 2 FAIL"
```

Gate 3 (numbers within ±10%): compare `total_mean_ms` and `ui_ms` medians across the three before/after JSON files by inspection.

**All three gates must pass.** If any fails, fix the extraction — do not re-baseline.

- [ ] **Step 8: Commit**

```bash
git add examples/citadel
git commit -m "refactor(citadel): move the bench harness onto superui_bench_support

Citadel's bench module had no unique items -- every struct and function
in it also existed in horde's, having been copy-pasted and re-typed
against CitadelConfig. Delegating to the shared crate removes the copy
without changing what citadel measures or prints.

The report formatters stay local on purpose: citadel emits
building_count where horde emits enemy_cap and a native-floor block, so
unifying them would either change this example's published output or
need a config knob per divergence. Output was verified byte-identical in
shape and within noise numerically against the pre-migration baseline."
```

---

## Task 7: Migrate horde and verify the gate

Same shape as Task 6, with two extra wrinkles: horde has a `Native` backend, and its profiler keeps a god-mode system.

**Files:**
- Modify: `examples/horde/Cargo.toml`
- Modify: `examples/horde/src/bench/mod.rs`
- Modify: `examples/horde/src/bench/profile.rs`
- Modify: `examples/horde/src/bin/bench.rs`

**Interfaces:**
- Consumes: `superui_bench_support::*` as in Task 6.
- Produces: unchanged public surface for `horde::bench::*`.

- [ ] **Step 1: Add the dependency**

In `examples/horde/Cargo.toml`, under `[dependencies]`:

```toml
superui_bench_support = { path = "../../crates/superui_bench_support" }
```

and forward the feature:

```toml
dhat-prof = ["dep:dhat", "superui_bench_support/dhat-prof"]
```

- [ ] **Step 2: Replace stats, dhat and CLI**

Apply the same edits as Task 6 Steps 2 and 4 to `examples/horde/src/bench/mod.rs`, with horde's values:

```rust
pub use superui_bench_support::{alloc_table, stats_from, AllocReport, BenchArgs, Stats};

const ARG_DEFAULTS: superui_bench_support::ArgDefaults = superui_bench_support::ArgDefaults {
    frames: 1000,
    warmup: 100,
    cap_flags: &["--enemy-cap", "--building-count"],
};

pub fn parse_args(argv: &[String]) -> Result<BenchArgs, String> {
    let mut a = superui_bench_support::parse_args(argv, ARG_DEFAULTS)?;
    if a.caps.is_empty() {
        a.caps = vec![SimConfig::default().enemy_cap];
    }
    Ok(a)
}

/// Horde's backend set includes Native, which the other examples have no meaning for.
pub fn backend_of(a: &BenchArgs) -> Result<Backend, String> {
    match a.backend.as_deref() {
        Some("null") => Ok(Backend::Null),
        Some("native") => Ok(Backend::Native),
        Some("supersolid") => Ok(Backend::Supersolid),
        Some(other) => Err(format!("unknown backend '{other}'")),
        None if a.profile => Ok(Backend::Supersolid),
        None => Err("--backend is required (null|native|supersolid)".to_string()),
    }
}
```

**Before writing this, read horde's existing `parse_args` and default-cap handling.** If horde's `--preset` is *used* rather than ignored (citadel tolerates and discards it), that logic must be preserved here — the shared parser discards it, so horde must read the preset itself. Check `examples/horde/src/bench/mod.rs` for `preset` handling and keep whatever it does.

- [ ] **Step 3: Reduce the profiler to a wrapper, keeping god-mode**

Replace `examples/horde/src/bench/profile.rs` with:

```rust
//! Per-stage profiling of the supersolid frame (the `--profile` bench mode).
//!
//! The attribution machinery lives in `superui_bench_support::profile`. Horde adds
//! one thing citadel does not need: god-moding the player so a long warmup under a
//! stress swarm stays in `Playing` with a full swarm rendered, instead of dying and
//! dropping to the cheap `GameOver` screen where the profile would measure an
//! almost-empty UI.

use bevy::prelude::*;

use crate::bench::{build_bench_app, Backend};
use crate::sim::{Health, Player, SimConfig};

const REBUILD_HINT: &str = "cargo run --release -p horde --features bench,bevy/trace,bevy/debug --bin horde-bench -- --profile ...";

/// Keep the auto-player alive so the game stays in `Playing` with a full swarm
/// rendered. Runs in `First`, before any sim damage.
fn keep_player_alive(mut q: Query<&mut Health, With<Player>>) {
    if let Ok(mut hp) = q.single_mut() {
        hp.current = hp.max.max(1.0e9);
    }
}

pub fn run_profile(sim: SimConfig, frames: usize, warmup: usize) {
    superui_bench_support::run_profile_with(
        || {
            let mut app = build_bench_app(Backend::Supersolid, sim);
            app.add_systems(First, keep_player_alive);
            app
        },
        frames,
        warmup,
        REBUILD_HINT,
    );
}
```

- [ ] **Step 4: Update the bin**

As Task 6 Step 5, but for `examples/horde/src/bin/bench.rs`: replace `args.backend` uses with `bench::backend_of(&args)?`.

- [ ] **Step 5: Verify it builds and unit tests pass**

```bash
cargo test --release -p horde --features bench
```

Expected: PASS.

- [ ] **Step 6: Run the gate**

```bash
for i in 1 2 3; do
  cargo run --release -p horde --features bench --bin horde-bench -- \
    --backend supersolid --frames 300 --warmup 300 --format json \
    > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-after-$i.json
done
cargo run --release -p horde --features bench --bin horde-bench -- \
  --backend supersolid --frames 300 --warmup 300 \
  > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-after.txt

diff <(grep -o '"[a-z_]*":' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-1.json) \
     <(grep -o '"[a-z_]*":' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-after-1.json) \
  && echo "GATE 1 PASS" || echo "GATE 1 FAIL"

diff <(sed 's/[0-9.]\+/N/g' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde.txt) \
     <(sed 's/[0-9.]\+/N/g' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-after.txt) \
  && echo "GATE 2 PASS" || echo "GATE 2 FAIL"
```

Also verify the profile path still produces a populated stage table:

```bash
cargo run --release -p horde --features bench,bevy/trace,bevy/debug --bin horde-bench -- \
  --profile --preset stress --enemy-cap 400 --frames 60 --warmup 300
```

Expected: a per-stage table with non-zero Boa render / reconcile / flair / taffy rows — **not** the "no system spans were recorded" message.

- [ ] **Step 7: Commit**

```bash
git add examples/horde
git commit -m "refactor(horde): move the bench harness onto superui_bench_support

Completes the de-duplication: horde and citadel now share one harness
instead of carrying two copies of the same design re-typed against
different config types, which is what would have forced rows to add a
third.

Horde keeps the two things that are genuinely its own -- the Native
backend, which the other examples have no meaning for, and the god-mode
system that keeps a stress-swarm profile in Playing rather than
measuring the near-empty GameOver screen. Output verified unchanged in
shape and within noise against the pre-migration baseline, including the
--profile stage table."
```

---

## Task 8: The supersolid rows backend

**Files:**
- Create: `examples/rows/assets/ui/rows_solid/index.html`
- Create: `examples/rows/assets/ui/rows_solid/app.tsx`
- Create: `examples/rows/assets/ui/rows_solid/rows.css`
- Create: `examples/rows/build.rs`
- Modify: `examples/rows/Cargo.toml` (restore the `TASK-1 TEMP` sections)
- Modify: `examples/rows/src/bench/mod.rs`
- Test: `examples/rows/tests/parity.rs`

**Interfaces:**
- Consumes: `rows::bench::{Backend, build_bench_app, dom_node_count}`.
- Produces: a working `Backend::Supersolid` app.

- [ ] **Step 1: Copy the stylesheet verbatim**

```bash
cp examples/rows/assets/ui/rows_vanilla/rows.css examples/rows/assets/ui/rows_solid/rows.css
```

Spec §3.2: the two backends must differ *only* in what drives the DOM. A different stylesheet would move the `Flair cascade` column for reasons unrelated to the framework.

- [ ] **Step 2: Write the failing parity test**

`examples/rows/tests/parity.rs`:

```rust
//! Spec §3.2: the two backends must differ only in what drives the DOM.

const VANILLA_CSS: &str = include_str!("../assets/ui/rows_vanilla/rows.css");
const SOLID_CSS: &str = include_str!("../assets/ui/rows_solid/rows.css");

#[test]
fn stylesheets_are_byte_identical() {
    assert_eq!(
        VANILLA_CSS, SOLID_CSS,
        "the two backends must share one stylesheet verbatim, or the published \
         Flair cascade column differs for reasons unrelated to the framework"
    );
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p rows --test parity`
Expected: FAIL — `rows_solid/rows.css` does not exist (if Step 1 was skipped) or the test file does not compile yet.

After Step 1 it should PASS. Run it to confirm.

- [ ] **Step 4: Write the supersolid markup and app**

`examples/rows/assets/ui/rows_solid/index.html`:

```html
<!doctype html>
<html>
  <head>
    <link rel="stylesheet" href="rows.css" />
    <script type="module" src="app.tsx"></script>
  </head>
  <body></body>
</html>
```

`examples/rows/assets/ui/rows_solid/app.tsx`. Same row shape, same 8 elements per row, same seeded PRNG. `<For>` is mandatory (global constraints).

```tsx
const ADJ = ["pretty","large","big","small","tall","short","long","handsome","plain","quaint","clean","elegant","easy","angry","crazy","helpful","mushy","odd","unsightly","adorable"];
const COL = ["red","yellow","blue","green","pink","brown","purple","white","black","orange"];
const NOU = ["table","chair","house","bbq","desk","car","pony","cookie","sandwich","burger","pizza","mouse","keyboard"];

let _seed = 1;
function rnd(n: number): number {
  _seed |= 0; _seed = (_seed + 0x6D2B79F5) | 0;
  let t = Math.imul(_seed ^ (_seed >>> 15), 1 | _seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) % n;
}
function randLabel(): string { return ADJ[rnd(20)] + " " + COL[rnd(10)] + " " + NOU[rnd(13)]; }

// PER-ROW SIGNALS, not plain fields. `<For>` keys rows by OBJECT IDENTITY
// (`render.js:434` — `prevIndex.set(items[i], i)` then `prevIndex.has(it)`), so
// replacing a row object with a spread copy makes `<For>` miss the reuse path, call
// `makeRow`, and rebuild that row's entire 8-element subtree. An update op would then
// be measuring a row rebuild against the vanilla backend's single text write — two
// different operations reported as one comparison. It would also break the head-to-head:
// React's js-framework-benchmark entry uses `key={id}` and PATCHES the text, so the
// number we would be sitting next to reflects patching, not rebuilding.
// Keeping the row object stable and putting the mutable fields behind signals is what
// SolidJS's own entry does.
type Row = {
  id: number;
  label: () => string;
  setLabel: (v: string) => void;
  cls: () => string;
  setCls: (v: string) => void;
};

let nextId = 1;
function makeRow(): Row {
  const [label, setLabel] = createSignal(randLabel());
  const [cls, setCls] = createSignal("lbl");
  return { id: nextId++, label, setLabel, cls, setCls };
}
function make(n: number): Row[] {
  const out: Row[] = [];
  for (let i = 0; i < n; i++) out.push(makeRow());
  return out;
}

function App() {
  const [rows, setRows] = createSignal<Row[]>([]);

  const swapAt = (a: number, b: number) => setRows(rs => {
    if (rs.length <= Math.max(a, b)) return rs;
    const c = rs.slice();
    const t = c[a]; c[a] = c[b]; c[b] = t;
    return c;
  });

  const ops: Record<string, () => void> = {
    create:     () => setRows(make(1000)),
    create10k:  () => setRows(make(10000)),
    append1:    () => setRows(rs => rs.concat(make(1))),
    append1k:   () => setRows(rs => rs.concat(make(1000))),
    insert1:    () => setRows(rs => make(1).concat(rs)),
    insertEvery2nd: () => setRows(rs => {
      const out: Row[] = [];
      for (let i = 0; i < rs.length; i++) { out.push(make(1)[0]); out.push(rs[i]); }
      return out;
    }),
    // Update ops write the row's OWN signal and never touch the list signal, so the
    // row object stays identical, `<For>` reuses its DOM, and only the bound text or
    // class re-runs — the same work the vanilla backend does with a direct write.
    updateText1: () => {
      const rs = rows();
      if (rs.length) rs[0].setLabel(randLabel());
    },
    updateTextEvery2nd: () => {
      const rs = rows();
      for (let i = 0; i < rs.length; i += 2) rs[i].setLabel(randLabel());
    },
    updateColor1: () => {
      const rs = rows();
      if (rs.length) rs[0].setCls("lbl warm");
    },
    updateColorEvery2nd: () => {
      const rs = rows();
      for (let i = 0; i < rs.length; i += 2) rs[i].setCls("lbl warm");
    },
    swap1:      () => swapAt(1, 998),
    swapEvery2nd: () => setRows(rs => {
      const c = rs.slice();
      for (let i = 0; i + 1 < c.length; i += 2) { const t = c[i]; c[i] = c[i + 1]; c[i + 1] = t; }
      return c;
    }),
    remove1:    () => setRows(rs => rs.slice(1)),
    removeEvery2nd: () => setRows(rs => rs.filter((_, i) => i % 2 === 1)),
    clear:      () => setRows([]),
  };

  return (
    <div id="main">
      <div class="jumbotron">
        {Object.keys(ops).map(name => (
          <button id={"op-" + name} onClick={ops[name]}>{name}</button>
        ))}
      </div>
      <div class="table" id="tbody">
        <For each={rows()}>
          {(r: Row) => (
            <div class="row" data-id={r.id}>
              <div class="col-md-1">{r.id}</div>
              <div class="col-md-4"><a class={r.cls()}>{r.label()}</a></div>
              <div class="col-md-1"><a class="remove"><span class="glyphicon"></span></a></div>
              <div class="col-md-6"></div>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

render(() => <App />, document.body);
```

**Before writing this file, read `examples/citadel/assets/ui/citadel/app.tsx`** and match its import/render conventions exactly — whether `createSignal`/`For`/`render` are globals or imported, and how the root is mounted. The above assumes globals (as `render.js:615-617` registers `For`/`Keyed`/`Index` on `globalThis`); adjust to whatever citadel actually does.

- [ ] **Step 5: Add the build script and restore the manifest**

`examples/rows/build.rs`:

```rust
//! Pre-transpile this example's `.tsx` to `.superui/build/*.js` for the bench
//! (which loads assets from memory) and for wasm / no-HMR native builds.
fn main() {
    supersolid::build::transpile_dir("assets/ui/rows_solid");
}
```

In `examples/rows/Cargo.toml`, uncomment both `# TASK-1 TEMP` sections (the two `[[bin]]` blocks and `[build-dependencies]`), and add:

```toml
superui_bench_support = { path = "../../crates/superui_bench_support" }
```

- [ ] **Step 6: Wire the supersolid assets into the memory source**

In `examples/rows/src/bench/mod.rs`, add the constants and replace the `unimplemented!`:

```rust
const S_HTML: &str = include_str!("../../assets/ui/rows_solid/index.html");
const S_CSS: &str = include_str!("../../assets/ui/rows_solid/rows.css");
const S_TSX: &str = include_str!("../../assets/ui/rows_solid/app.tsx");
const S_JS: &str = include_str!("../../assets/ui/rows_solid/.superui/build/app.js");
```

```rust
        Backend::Supersolid => {
            dir.insert_asset("ui/rows_solid/index.html".as_ref(), S_HTML.as_bytes());
            dir.insert_asset("ui/rows_solid/rows.css".as_ref(), S_CSS.as_bytes());
            dir.insert_asset("ui/rows_solid/app.tsx".as_ref(), S_TSX.as_bytes());
            dir.insert_asset("ui/rows_solid/.superui/build/app.js".as_ref(), S_JS.as_bytes());
        }
```

- [ ] **Step 7: Write the minimal windowed bin**

`examples/rows/src/main.rs` — dev convenience so the app can be eyeballed:

```rust
//! Windowed rows app. `cargo run -p rows` (vanilla) — set ROWS_BACKEND=supersolid
//! for the TSX build.
use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

fn main() {
    let dir = match std::env::var("ROWS_BACKEND").as_deref() {
        Ok("supersolid") => "ui/rows_solid",
        _ => "ui/rows_vanilla",
    };
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, move |mut c: Commands, a: Res<AssetServer>| {
            c.spawn(Camera2d);
            c.spawn(SuperUiRoot::from_asset_dir(dir, &a));
        })
        .run();
}
```

- [ ] **Step 8: Verify both backends mount**

Extend `examples/rows/tests/parity.rs`:

```rust
use rows::bench::{build_bench_app, dom_node_count, Backend};

#[test]
fn both_backends_mount_and_render_chrome() {
    for backend in [Backend::Vanilla, Backend::Supersolid] {
        let mut app = build_bench_app(backend);
        for _ in 0..30 {
            app.update();
        }
        let n = dom_node_count(&app);
        assert!(n > 10, "{} mounted only {n} DOM elements", backend.label());
    }
}
```

Run: `cargo test --release -p rows --test parity`
Expected: PASS, 2 tests.

- [ ] **Step 9: Commit**

```bash
git add examples/rows
git commit -m "feat(rows): add the supersolid backend

Gives the workload a second driver over identical markup, so the
difference between the two columns is supersolid's overhead over raw DOM
rather than a difference in what is being rendered.

<For> is required rather than preferred: <Keyed>'s DOM order is
append/remove, which would render swap1 and insertEvery2nd in the wrong
order, and keyed rendering is also what puts these numbers in the same
js-framework-benchmark category as the React implementation they are
meant to be compared against. The stylesheet is shared verbatim and a
test enforces it, because a diverging selector set would move the flair
cascade column for reasons unrelated to the framework."
```

---

## Task 9: The op driver

The measurement core: preconditions, synthetic click, and the quiescence loop.

**Files:**
- Modify: `examples/rows/src/bench/mod.rs`
- Test: `examples/rows/tests/driver.rs`

**Interfaces:**
- Consumes: `rows::bench::{Backend, build_bench_app, dom_node_count}`; `superui_bridge::{UiRuntime, PendingDomEvents, events::click_effect}`.
- Produces:
  - `pub const OPS: [&str; 14]` — the frozen op list, in order
  - `pub struct OpSample { pub total_ms: f64, pub frames: usize, pub rows_before: usize, pub nodes_after: usize }`
  - `pub fn click_op(app: &mut App, op: &str) -> bool`
  - `pub fn step_to_quiescence(app: &mut App) -> (f64, usize)`
  - `pub fn measure_op(app: &mut App, op: &str, rows_before: usize) -> OpSample`
  - `pub fn precondition(app: &mut App, rows: usize)`

- [ ] **Step 1: Write the failing test**

`examples/rows/tests/driver.rs`:

```rust
use rows::bench::{build_bench_app, measure_op, precondition, row_count, Backend, OPS};

#[test]
fn op_list_is_the_frozen_contract() {
    // Spec §2: these names and this order are not implementation choices.
    assert_eq!(
        OPS,
        [
            "create", "append1", "append1k", "insert1", "insertEvery2nd",
            "updateText1", "updateTextEvery2nd", "updateColor1", "updateColorEvery2nd",
            "swap1", "swapEvery2nd", "remove1", "removeEvery2nd", "clear",
        ]
    );
}

#[test]
fn an_op_settles_and_reports_its_frame_count() {
    let mut app = build_bench_app(Backend::Vanilla);
    precondition(&mut app, 1000);
    assert_eq!(row_count(&app), 1000, "precondition did not reach 1000 rows");

    let s = measure_op(&mut app, "append1", 1000);
    assert!(s.total_ms > 0.0, "op reported zero time");
    assert!(s.frames >= 1, "op reported zero frames");
    assert_eq!(row_count(&app), 1001, "append1 must add exactly one row");
}

/// The regression guard for the quiescence predicate.
///
/// `append1` changes the node count, so it is measured correctly even by a broken
/// predicate — which is exactly why it cannot guard this. `updateText1` mutates one
/// text node and changes NOTHING about the node count, so a predicate that cannot
/// see in-place work reports `total_ms = 0.0, frames = 0` here while the row's label
/// visibly changes. Six of the fourteen ops have this shape.
#[test]
fn an_in_place_op_is_measured_not_skipped() {
    let mut app = build_bench_app(Backend::Vanilla);
    precondition(&mut app, 1000);

    let nodes_before = dom_node_count(&app);
    let label_before = first_row_label(&app);

    let s = measure_op(&mut app, "updateText1", 1000);

    assert_eq!(
        dom_node_count(&app),
        nodes_before,
        "updateText1 must not change the node count — if it does, the op is wrong"
    );
    assert_ne!(
        first_row_label(&app),
        label_before,
        "updateText1 did not actually change the label; the test proves nothing"
    );
    assert!(
        s.total_ms > 0.0 && s.frames >= 1,
        "in-place op reported total_ms={} frames={} — the quiescence predicate cannot \
         see work that leaves the node count unchanged",
        s.total_ms,
        s.frames
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --release -p rows --test driver`
Expected: FAIL — `measure_op`, `precondition`, `row_count`, `OPS` not found.

- [ ] **Step 3: Add the `create10k` button to both apps**

Vanilla `index.html` already got it in Task 1 Step 7. The supersolid app generates buttons from `Object.keys(ops)`, which already includes `create10k`. Confirm both have it.

- [ ] **Step 4: Write the driver**

Append to `examples/rows/src/bench/mod.rs`:

```rust
use std::time::Instant;
use superui_bridge::{PendingDomEvents, UiRuntime};

/// The frozen op set (spec §2). Names and order are the comparability contract.
pub const OPS: [&str; 14] = [
    "create", "append1", "append1k", "insert1", "insertEvery2nd",
    "updateText1", "updateTextEvery2nd", "updateColor1", "updateColorEvery2nd",
    "swap1", "swapEvery2nd", "remove1", "removeEvery2nd", "clear",
];

/// One measured operation.
#[derive(Clone, Copy, Debug)]
pub struct OpSample {
    pub total_ms: f64,
    /// Frames the op took to settle. Expected 1; published so a spill is visible.
    pub frames: usize,
    pub rows_before: usize,
    pub nodes_after: usize,
}

/// Live `.row` count, read from the DOM.
pub fn row_count(app: &App) -> usize {
    app.world()
        .get_non_send::<UiRuntime>()
        .map(|rt| {
            let d = rt.dom.borrow();
            d.query_selector_all(d.document(), ".row").len()
        })
        .unwrap_or(0)
}

/// Dispatch a real DOM click at `#op-<op>`, exactly as the picking observer would.
/// Returns false when the button is not mounted.
///
/// Two API notes, both confirmed the hard way in Task 1:
/// - `click_effect` is re-exported at the crate root; `superui_bridge::events` is
///   private, so `superui_bridge::events::click_effect` does not compile.
/// - `UiRuntime` is NonSend and `PendingDomEvents` is a resource, and `World` will
///   not hand out both borrows at once. `resource_scope` temporarily removes the
///   resource so the closure can hold it mutably beside the runtime borrow.
pub fn click_op(app: &mut App, op: &str) -> bool {
    let world = app.world_mut();
    let Some(rt) = world.get_non_send::<UiRuntime>() else { return false };
    let node = {
        let d = rt.dom.borrow();
        d.query_selector(d.document(), &format!("#op-{op}"))
    };
    let Some(node) = node else { return false };

    world.resource_scope::<PendingDomEvents, _>(|world, mut pending| {
        let rt = world.non_send::<UiRuntime>();
        superui_bridge::click_effect(rt, node, &mut pending);
    });
    true
}

/// Completed reconcile passes so far. See the `reconciles` counter in
/// `superui_bridge` — this is the only signal that survives a frame boundary.
fn reconcile_count(app: &App) -> u64 {
    app.world()
        .get_non_send::<UiRuntime>()
        .map(|rt| rt.reconciles)
        .unwrap_or(0)
}

/// Step `app.update()` until a frame does no reconcile work.
///
/// Returns (summed ms of the frames that did work, that frame count). A fixed
/// frame count would silently absorb an op that spills; this makes it visible.
///
/// **Do not use `rt.dirty` here, and do not compare DOM node counts.** `dirty` is
/// set by `drain_dom_events_system` and cleared by `reconcile_system` *within the
/// same `app.update()`* (they are `.chain()`ed in `Update`), so a read after
/// `app.update()` returns always observes `false` — the check is dead code. Falling
/// back to node counts is worse than useless: `updateText1`, `updateTextEvery2nd`,
/// `updateColor1`, `updateColorEvery2nd`, `swap1` and `swapEvery2nd` change no node
/// count at all, so 6 of the 14 ops would report `total_ms = 0.0, frames = 0` while
/// visibly doing work. The monotonic reconcile counter is the signal that survives
/// the frame.
pub fn step_to_quiescence(app: &mut App) -> (f64, usize) {
    const MAX_FRAMES: usize = 64;
    let mut total_ms = 0.0;
    let mut worked = 0usize;

    for _ in 0..MAX_FRAMES {
        let before = reconcile_count(app);
        let t = Instant::now();
        app.update();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let after = reconcile_count(app);

        if after == before {
            // Settled: this frame reconciled nothing, so it is not counted.
            return (total_ms, worked);
        }
        total_ms += ms;
        worked += 1;
    }
    panic!("op did not settle within {MAX_FRAMES} frames");
}

/// Drive the app to `rows` rows without timing it.
pub fn precondition(app: &mut App, rows: usize) {
    // Mount first.
    for _ in 0..30 {
        app.update();
    }
    let op = match rows {
        0 => "clear",
        1_000 => "create",
        10_000 => "create10k",
        other => panic!("unsupported precondition: {other} rows"),
    };
    assert!(click_op(app, op), "precondition button #op-{op} not found");
    let _ = step_to_quiescence(app);
    assert_eq!(row_count(app), rows, "precondition did not reach {rows} rows");
}

/// Trigger `op` and measure it. The app must already be at its precondition.
pub fn measure_op(app: &mut App, op: &str, rows_before: usize) -> OpSample {
    assert!(click_op(app, op), "button #op-{op} not found");
    let (total_ms, frames) = step_to_quiescence(app);
    OpSample { total_ms, frames, rows_before, nodes_after: dom_node_count(app) }
}
```

**Required change to `crates/superui_bridge/` — a reconcile counter.**

Neither `rt.dirty` nor a DOM node count can serve as the quiescence signal:

- `drain_dom_events_system` sets `dirty` and `reconcile_system` clears it, and the two
  are `.chain()`ed within the same `Update` run — so by the time `app.update()`
  returns, `dirty` is always back to `false`. Reading it there is dead code.
- Node counts miss every op that mutates in place. `updateText1`,
  `updateTextEvery2nd`, `updateColor1`, `updateColorEvery2nd`, `swap1` and
  `swapEvery2nd` all leave the count identical — 6 of the 14 measured ops.

Add a monotonic counter that survives the frame. In
`crates/superui_bridge/src/runtime.rs`, beside the existing `pub dirty: bool`:

```rust
    /// Completed reconcile passes, monotonic. `dirty` is set and cleared within a
    /// single schedule run, so it cannot be observed from outside; this counter is
    /// what lets an external driver ask "did a reconcile happen during that
    /// `app.update()`?" after the fact. Used by the rows benchmark's quiescence loop.
    pub reconciles: u64,
```

and in `crates/superui_bridge/src/reconcile.rs`, inside the existing `if rt.dirty`
branch of `reconcile_system`:

```rust
    if rt.dirty {
        rt.reconcile(world);
        rt.dirty = false;
        rt.reconciles += 1;
    }
```

Initialise it to `0` wherever `UiRuntime` is constructed. This is additive and
non-breaking; keep it to exactly these two edits.

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test --release -p rows --test driver`
Expected: PASS, 2 tests.

- [ ] **Step 6: Commit**

```bash
git add examples/rows
git commit -m "feat(rows): add the op driver

Times an operation the way bevy-react defines it -- event trigger to
change detected -- by dispatching a real click through click_effect
rather than calling a JS entry point, so the measured span covers the
same work as the numbers being compared against.

Quiescence is a predicate rather than a fixed frame count, because a
fixed count would silently absorb an op that spills across frames and
inflate its total. The frame count is carried on the sample so it can be
published: it should read 1 everywhere, and anything else is visible in
the table instead of hidden in a number."
```

---

## Task 10: Per-op DOM-state assertions on both backends

Guards the harness: an op that measures the wrong mutation quickly is worse than no measurement. Also enforces spec §3.2 — that both backends really do the same work.

**Files:**
- Create: `examples/rows/tests/ops.rs`

**Interfaces:**
- Consumes: `rows::bench::{Backend, build_bench_app, measure_op, precondition, row_count, OPS}`.
- Produces: nothing.

- [ ] **Step 1: Write the failing tests**

`examples/rows/tests/ops.rs`:

```rust
//! Spec §8: each op must leave the DOM in the expected state, on BOTH backends.
//! These guard the harness, not the framework — a benchmark that measures the
//! wrong mutation fast is worse than no benchmark.

use rows::bench::{
    build_bench_app, first_row_ids, measure_op, precondition, row_count, Backend, OPS,
};

fn backends() -> [Backend; 2] {
    [Backend::Vanilla, Backend::Supersolid]
}

#[test]
fn every_op_settles_in_one_frame() {
    for b in backends() {
        for op in OPS {
            let mut app = build_bench_app(b);
            precondition(&mut app, if op == "create" { 0 } else { 1000 });
            let s = measure_op(&mut app, op, 1000);
            assert_eq!(
                s.frames,
                1,
                "{}/{op} settled in {} frames, not 1 — the published Frames column \
                 must read 1 or the total is being spread across frames",
                b.label(),
                s.frames
            );
        }
    }
}

#[test]
fn row_counts_after_each_op() {
    // (op, rows before, expected rows after)
    let cases: [(&str, usize, usize); 8] = [
        ("create", 0, 1000),
        ("append1", 1000, 1001),
        ("append1k", 1000, 2000),
        ("insert1", 1000, 1001),
        // N/2 = 500 inserts, not 1000 — see the Every2nd rule in Global Constraints.
        ("insertEvery2nd", 1000, 1500),
        ("remove1", 1000, 999),
        ("removeEvery2nd", 1000, 500),
        ("clear", 1000, 0),
    ];
    for b in backends() {
        for (op, before, after) in cases {
            let mut app = build_bench_app(b);
            precondition(&mut app, before);
            measure_op(&mut app, op, before);
            assert_eq!(row_count(&app), after, "{}/{op}", b.label());
        }
    }
}

#[test]
fn swap1_swaps_exactly_two_rows_and_preserves_order() {
    for b in backends() {
        let mut app = build_bench_app(b);
        precondition(&mut app, 1000);
        let before = first_row_ids(&app, 1000);
        measure_op(&mut app, "swap1", 1000);
        let after = first_row_ids(&app, 1000);

        assert_eq!(after[1], before[998], "{}: row 1 must hold old row 998", b.label());
        assert_eq!(after[998], before[1], "{}: row 998 must hold old row 1", b.label());
        for i in 0..1000 {
            if i != 1 && i != 998 {
                assert_eq!(after[i], before[i], "{}: row {i} moved but should not have", b.label());
            }
        }
    }
}

#[test]
fn remove_every_2nd_leaves_the_right_ids_in_order() {
    for b in backends() {
        let mut app = build_bench_app(b);
        precondition(&mut app, 1000);
        let before = first_row_ids(&app, 1000);
        measure_op(&mut app, "removeEvery2nd", 1000);
        let after = first_row_ids(&app, 500);

        let expected: Vec<i64> = before.iter().skip(1).step_by(2).copied().collect();
        assert_eq!(after, expected, "{}", b.label());
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --release -p rows --test ops`
Expected: FAIL — `first_row_ids` not found.

- [ ] **Step 3: Add the DOM inspection helper**

Append to `examples/rows/src/bench/mod.rs`:

```rust
/// Text of the first row's `.lbl` anchor. Used to prove an in-place update really
/// changed something, so the quiescence regression test cannot pass vacuously.
pub fn first_row_label(app: &App) -> String {
    let Some(rt) = app.world().get_non_send::<UiRuntime>() else { return String::new() };
    let d = rt.dom.borrow();
    d.query_selector(d.document(), ".row .lbl")
        .map(|n| d.text_content(n))
        .unwrap_or_default()
}

/// The `data-id` of the first `n` rows, in DOM order. Used by the op tests to
/// assert ordering, which is the property <Keyed> would have broken.
pub fn first_row_ids(app: &App, n: usize) -> Vec<i64> {
    let Some(rt) = app.world().get_non_send::<UiRuntime>() else { return Vec::new() };
    let d = rt.dom.borrow();
    d.query_selector_all(d.document(), ".row")
        .into_iter()
        .take(n)
        .map(|node| {
            d.get_attribute(node, "data-id")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(-1)
        })
        .collect()
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --release -p rows --test ops`
Expected: PASS, 4 tests.

**If `every_op_settles_in_one_frame` fails**, do not relax the assertion. An op spilling frames is a real finding (spec §9.1): record which op and how many frames, then decide whether the driver's predicate is wrong or the pipeline genuinely takes two passes. Raise it before continuing.

**If a supersolid case fails but vanilla passes**, the `<For>` implementation diverges from the vanilla one — fix `app.tsx`, not the test.

- [ ] **Step 5: Commit**

```bash
git add examples/rows
git commit -m "test(rows): assert each op's DOM outcome on both backends

A benchmark harness has no red-green cycle, so these stand in for it: an
op that measures the wrong mutation quickly would publish a fast number
for work nobody asked for.

Running the same assertions against both backends is also what makes the
vanilla-vs-supersolid column comparison mean anything, since it proves
the two are performing identical work rather than merely producing
similar-looking trees. The swap and removeEvery2nd order assertions are
the ones that would have caught <Keyed>'s append/remove ordering."
```

---

## Task 11: `rows-bench` and untraced pass reporting

**Files:**
- Create: `examples/rows/src/bench/report.rs`
- Create: `examples/rows/src/bin/bench.rs`
- Modify: `examples/rows/src/bench/mod.rs`

**Interfaces:**
- Consumes: `superui_bench_support::{parse_args, ArgDefaults, BenchArgs, stats_from, Stats}`; `rows::bench::*`.
- Produces:
  - `pub struct OpReport { pub op: String, pub rows_before: usize, pub nodes: usize, pub frames: usize, pub total: Stats }`
  - `pub fn run_ops(backend: Backend, rows: usize, reps: usize, warmup: usize) -> Vec<OpReport>`
  - `pub fn untraced_table(backend: Backend, rows: usize, reports: &[OpReport]) -> String`
  - `pub fn untraced_json(backend: Backend, rows: usize, reports: &[OpReport]) -> String`

- [ ] **Step 1: Write the failing test**

Append to `examples/rows/tests/driver.rs`:

```rust
use rows::bench::{untraced_json, untraced_table, run_ops};

#[test]
fn untraced_reports_every_op_in_contract_order() {
    let reports = run_ops(Backend::Vanilla, 1000, 2, 1);
    assert_eq!(reports.len(), OPS.len(), "every op must be reported");
    for (r, expected) in reports.iter().zip(OPS) {
        assert_eq!(r.op, expected, "op order is the comparability contract");
    }

    let t = untraced_table(Backend::Vanilla, 1000, &reports);
    assert!(t.contains("swap1"), "{t}");
    assert!(t.contains("Frames"), "the Frames column must be published: {t}");
    assert!(!t.to_lowercase().contains("traced"), "untraced pass must not claim traced data: {t}");

    let j = untraced_json(Backend::Vanilla, 1000, &reports);
    assert!(j.contains("\"traced\":false"), "untraced pass JSON must mark itself untraced: {j}");
    assert!(j.contains("\"p50_ms\""), "{j}");
    assert!(j.contains("\"p95_ms\""), "{j}");
    assert!(j.contains("\"p99_ms\""), "{j}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --release -p rows --test driver`
Expected: FAIL — `run_ops`, `untraced_table`, `untraced_json` not found.

- [ ] **Step 3: Write the runner and reporting**

First, add the chrome helper to `examples/rows/src/bench/mod.rs` (it does not exist
yet — it belongs beside `dom_node_count`, which is already there):

```rust
/// Boot chrome: DOM elements present with ZERO rows. Backend-dependent — the
/// supersolid app mounts inside a `<div id="root">` (the convention every other
/// supersolid example uses) that the vanilla app has no equivalent of, so chrome is
/// 24 vs 23. Constant and op-independent, so it does not affect timings, but the
/// published `Nodes` column exists for cross-implementation normalisation and must
/// therefore be chrome-free.
pub fn chrome_node_count(backend: Backend) -> usize {
    let mut app = build_bench_app(backend);
    for _ in 0..30 {
        app.update();
    }
    dom_node_count(&app)
}
```

Then `examples/rows/src/bench/report.rs`:

```rust
//! untraced pass reporting: the citable, untraced numbers.
//!
//! Spec §5 — untraced pass and traced pass are never blended. This module emits only untraced pass
//! and marks its JSON `"traced": false` so a merged file cannot be misread.

use bevy::prelude::App;
use superui_bench_support::{stats_from, Stats};

use crate::bench::{
    build_bench_app, chrome_node_count, measure_op, precondition, Backend, OPS,
};

#[derive(Clone, Debug)]
pub struct OpReport {
    pub op: String,
    pub rows_before: usize,
    pub nodes: usize,
    pub frames: usize,
    pub total: Stats,
}

/// Pre-op row count for an op, matching bevy-react's `Rows` column.
fn rows_before_for(op: &str, rows: usize) -> usize {
    if op == "create" { 0 } else { rows }
}

/// The button that performs `op` at this scale.
///
/// `create` is the ONLY op whose size follows the table. bevy-react's 10k `create`
/// emits 40001 ops (10 000 rows × ~4), against 4001 at 1k — so at the 10k scale ours
/// must build 10 000 rows or the row is not comparable. The fixture exposes that as a
/// separate `create10k` button (it doubles as the 10k precondition helper).
///
/// Every other op needs no mapping: `append1k`/`append1`/`insert1` are fixed-size by
/// definition (bevy-react's `append1k` is 4001 ops at BOTH scales), and the `*Every2nd`
/// family derives its size from the current table.
fn button_for(op: &str, rows: usize) -> &str {
    if op == "create" && rows == 10_000 { "create10k" } else { op }
}

/// Measure every op at `rows` scale, `reps` timed reps after `warmup` discarded.
///
/// Each rep rebuilds the app so the precondition is reached from a clean state —
/// ops mutate the table, so reusing one app would drift the precondition.
pub fn run_ops(backend: Backend, rows: usize, reps: usize, warmup: usize) -> Vec<OpReport> {
    let mut out = Vec::with_capacity(OPS.len());
    // Subtract boot chrome so `Nodes` counts only row-attributable elements. The two
    // backends have different chrome (24 vs 23 — supersolid mounts inside `#root`),
    // and a column that silently differs by one defeats the normalisation it exists for.
    let chrome = chrome_node_count(backend);

    for op in OPS {
        let before = rows_before_for(op, rows);
        let mut samples: Vec<f64> = Vec::with_capacity(reps);
        let mut frames = 0usize;
        let mut nodes = 0usize;

        for rep in 0..(warmup + reps) {
            let mut app: App = build_bench_app(backend);
            precondition(&mut app, before);
            // Logical op name is what gets published; the button may differ (see
            // `button_for` — only `create` at the 10k scale).
            let s = measure_op(&mut app, button_for(op, rows), before);
            if rep >= warmup {
                samples.push(s.total_ms);
                frames = frames.max(s.frames);
                nodes = s.nodes_after;
            }
        }

        out.push(OpReport {
            op: op.to_string(),
            rows_before: before,
            nodes: nodes.saturating_sub(chrome),
            frames,
            total: stats_from(samples),
        });
    }
    out
}

pub fn untraced_table(backend: Backend, rows: usize, reports: &[OpReport]) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(
        s,
        // Phrased without the word "traced" on purpose — the test below asserts the
        // untraced pass table never contains it, and "untraced" would trip that substring check.
        "rows={rows} backend={} — untraced pass (no tracing in the build; these are the citable numbers)",
        backend.label()
    );
    let _ = writeln!(
        s,
        "| {:<20} | {:>6} | {:>7} | {:>6} | {:>9} | {:>9} | {:>9} |",
        "Op", "Rows", "Nodes", "Frames", "p50 (ms)", "p95 (ms)", "p99 (ms)"
    );
    for r in reports {
        let _ = writeln!(
            s,
            "| {:<20} | {:>6} | {:>7} | {:>6} | {:>9.3} | {:>9.3} | {:>9.3} |",
            r.op, r.rows_before, r.nodes, r.frames, r.total.p50_ms, r.total.p95_ms, r.total.p99_ms
        );
    }
    s
}

pub fn untraced_json(backend: Backend, rows: usize, reports: &[OpReport]) -> String {
    let ops: Vec<String> = reports
        .iter()
        .map(|r| {
            format!(
                "{{\"op\":\"{}\",\"rows\":{},\"nodes\":{},\"frames\":{},\
                 \"p50_ms\":{:.6},\"p95_ms\":{:.6},\"p99_ms\":{:.6},\"mean_ms\":{:.6}}}",
                r.op, r.rows_before, r.nodes, r.frames,
                r.total.p50_ms, r.total.p95_ms, r.total.p99_ms, r.total.mean_ms
            )
        })
        .collect();
    format!(
        "{{\"backend\":\"{}\",\"rows\":{},\"traced\":false,\"ops\":[{}]}}",
        backend.label(),
        rows,
        ops.join(",")
    )
}
```

Add `pub mod report;` and `pub use report::{untraced_json, untraced_table, run_ops, OpReport};` to `examples/rows/src/bench/mod.rs`.

- [ ] **Step 4: Write the bin**

`examples/rows/src/bin/bench.rs`:

```rust
//! `rows-bench` — the js-framework-benchmark rows workload on superui.
//! See `examples/rows/benchmark.md`.

use rows::bench::{untraced_json, untraced_table, run_ops, Backend};
use superui_bench_support::{parse_args, ArgDefaults};

const DEFAULTS: ArgDefaults = ArgDefaults {
    frames: 20, // reps, not frames — rows measures ops, not steady-state frames
    warmup: 3,
    cap_flags: &["--rows"],
};

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv, DEFAULTS) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: rows-bench --backend vanilla|supersolid [--rows 1000|10000] \
                 [--reps N] [--warmup N] [--seed N] [--format table|json] [--profile]"
            );
            std::process::exit(2);
        }
    };

    let backend = match args.backend.as_deref() {
        Some("vanilla") => Backend::Vanilla,
        Some("supersolid") => Backend::Supersolid,
        Some(other) => {
            eprintln!("unknown backend '{other}' (vanilla|supersolid)");
            std::process::exit(2);
        }
        None => {
            eprintln!("--backend is required (vanilla|supersolid)");
            std::process::exit(2);
        }
    };

    let scales = if args.caps.is_empty() { vec![1000] } else { args.caps.clone() };

    for rows in scales {
        if args.profile {
            rows::bench::profile::run_profile(backend, rows, args.frames, args.warmup);
            continue;
        }
        let reports = run_ops(backend, rows, args.frames, args.warmup);
        if args.json {
            println!("{}", untraced_json(backend, rows, &reports));
        } else {
            print!("{}", untraced_table(backend, rows, &reports));
        }
    }
}
```

Note `--reps` maps onto the shared parser's `--frames` slot. Add `"--reps"` as an accepted alias by extending the shared parser's `--frames` arm in `crates/superui_bench_support/src/cli.rs`:

```rust
            "--frames" | "--reps" => frames = advance(&mut i)?.parse().map_err(|_| "bad --frames/--reps".to_string())?,
```

and add a test for it in `cli.rs`'s test module:

```rust
    #[test]
    fn reps_is_an_alias_for_frames() {
        let a = parse_args(&args(&["--backend", "x", "--reps", "25"]), defaults()).unwrap();
        assert_eq!(a.frames, 25);
    }
```

`rows::bench::profile::run_profile` arrives in Task 12; stub it for now:

```rust
// in examples/rows/src/bench/mod.rs
pub mod profile;
```

```rust
// examples/rows/src/bench/profile.rs — filled in by Task 12
use crate::bench::Backend;

pub fn run_profile(_backend: Backend, _rows: usize, _reps: usize, _warmup: usize) {
    unimplemented!("traced pass lands in Task 12");
}
```

- [ ] **Step 5: Run to verify it passes**

```bash
cargo test --release -p rows --features bench --test driver
cargo run --release -p rows --features bench --bin rows-bench -- --backend vanilla --rows 1000 --reps 3 --warmup 1
```

Expected: tests PASS; the bin prints a 14-row table with a `Frames` column reading 1 throughout.

- [ ] **Step 6: Commit**

```bash
git add examples/rows crates/superui_bench_support
git commit -m "feat(rows): add rows-bench and the untraced untraced pass report

untraced pass is the number that gets cited, so it is produced by a build with
no tracing in it at all -- the breakdown is worth having but not at the
cost of contaminating the headline figure with instrumentation overhead.

The JSON marks itself traced:false rather than relying on the filename,
because the two passes produce similarly-shaped output and a merged or
renamed file would otherwise be impossible to tell apart. Each rep
rebuilds the app so every measurement starts from a clean precondition
instead of drifting as earlier ops mutate the table."
```

---

## Task 12: traced pass — the traced stage breakdown

**Files:**
- Modify: `examples/rows/src/bench/profile.rs`

**Interfaces:**
- Consumes: `superui_bench_support::{run_profile_with, Bucket}`; `rows::bench::*`.
- Produces: `pub fn run_profile(backend: Backend, rows: usize, reps: usize, warmup: usize)`

- [ ] **Step 1: Write the failing test**

Append to `examples/rows/tests/driver.rs`:

```rust
#[test]
fn traced_pass_is_labelled_and_never_merged_with_untraced() {
    // traced pass's own JSON must be distinguishable from untraced pass's (spec §5).
    let j = rows::bench::profile::traced_json_header(Backend::Vanilla, 1000);
    assert!(j.contains("\"traced\":true"), "{j}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --release -p rows --features bench --test driver`
Expected: FAIL — `traced_json_header` not found.

- [ ] **Step 3: Write traced pass**

Replace `examples/rows/src/bench/profile.rs`:

```rust
//! traced pass: the traced per-stage breakdown (spec §5).
//!
//! Requires **both** `bevy/trace` and `bevy/debug`. `trace` creates the per-system
//! spans; `debug` is what makes their names resolvable — without it every name reads
//! `<Enable the debug feature to see the name>` and the whole frame collapses into the
//! `other` bucket, which looks like a working report but attributes nothing.
//! The stage columns sum to the TRACED total, not to untraced pass's `Total` — the two
//! passes are separate runs and this module never emits untraced pass's numbers.

use crate::bench::{build_bench_app, measure_op, precondition, Backend, OPS};

const REBUILD_HINT: &str = "cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- --profile --backend vanilla --rows 1000";

/// JSON header identifying this as traced output, so a traced pass file can never be
/// mistaken for a untraced pass file.
pub fn traced_json_header(backend: Backend, rows: usize) -> String {
    format!(
        "{{\"backend\":\"{}\",\"rows\":{},\"traced\":true,\"note\":\"stage columns sum to the traced total, not to untraced pass Total\"",
        backend.label(),
        rows
    )
}

/// Run every op under the tracing attributor and print the per-stage table.
pub fn run_profile(backend: Backend, rows: usize, reps: usize, warmup: usize) {
    println!(
        "\n=== rows traced pass: traced stage breakdown (rows={rows}, backend={}) ===",
        backend.label()
    );
    println!(
        "NOTE: these totals include tracing overhead. The citable Total is untraced pass \
         (run without --profile and without bevy/trace)."
    );

    for op in OPS {
        let before = if op == "create" { 0 } else { rows };
        println!("\n-- op: {op} --");
        superui_bench_support::run_profile_with(
            || {
                let mut app = build_bench_app(backend);
                precondition(&mut app, before);
                app
            },
            reps,
            warmup,
            REBUILD_HINT,
        );
        // `run_profile_with` drives plain `app.update()` frames; for rows the work
        // under measurement is the op itself, so trigger it on each measured frame
        // via the driver instead. See Step 4.
        let _ = (op, reps, warmup);
    }
}
```

- [ ] **Step 4: Adapt the attributor to per-op driving**

`run_profile_with` drives bare `app.update()` frames, which suits horde/citadel's steady-state model but not rows' per-op model. Add a driving hook to the support crate — in `crates/superui_bench_support/src/profile.rs`:

```rust
/// Like [`run_profile_with`], but the caller supplies BOTH halves of an iteration:
/// `prepare` (untraced) and `measure` (traced).
///
/// The split exists because a per-op benchmark has to restore state between
/// iterations, and that restore must not land in the attribution. Rows resets the
/// table to its precondition — clearing 1000 rows and rebuilding 1000 rows — before
/// an op that may touch a single row. With one blanket recording window, every op's
/// breakdown is ~the reset: `swap1` (moves 2 rows) measured 481 ms against `create`
/// (builds 1000) at 434 ms, which is the tell that the op's own signal had been
/// buried. Recording is therefore toggled on around `measure` only.
///
/// Steady-state benchmarks (horde, citadel) have nothing to restore — every frame IS
/// the measured thing — so they pass an empty `prepare` via [`run_profile_with`].
pub fn run_profile_driven(
    build: impl FnOnce() -> App,
    mut prepare: impl FnMut(&mut App),
    mut measure: impl FnMut(&mut App),
    iters: usize,
    warmup: usize,
    rebuild_hint: &str,
) {
    let agg = install();
    let mut app = build();

    for _ in 0..warmup {
        prepare(&mut app);
        measure(&mut app);
    }
    {
        let mut a = agg.lock().unwrap();
        a.per_system.clear();
        a.recording = false;
    }

    // Wall time accumulates ONLY the measured halves, so the printed frame cost and
    // the stage percentages describe the same window.
    let mut wall_total_ms = 0.0;
    for _ in 0..iters {
        prepare(&mut app);
        agg.lock().unwrap().recording = true;
        let t = Instant::now();
        measure(&mut app);
        wall_total_ms += t.elapsed().as_secs_f64() * 1000.0;
        agg.lock().unwrap().recording = false;
    }

    print_report(&agg, iters, wall_total_ms, rebuild_hint);
}
```

and redefine `run_profile_with` in terms of it, so horde/citadel keep working unchanged:

```rust
pub fn run_profile_with(
    build: impl FnOnce() -> App,
    frames: usize,
    warmup: usize,
    rebuild_hint: &str,
) {
    // Steady-state: nothing to restore between frames, so `prepare` is a no-op and
    // every frame is measured. Behaviour is identical to before the split.
    run_profile_driven(
        build,
        |_app| {},
        |app| { app.update(); },
        frames,
        warmup,
        rebuild_hint,
    );
}
```

Export it: `pub use profile::{run_profile_driven, run_profile_with, Bucket};`

Then in rows' `run_profile`, replace the `run_profile_with` call with:

```rust
        superui_bench_support::run_profile_driven(
            || {
                let mut app = build_bench_app(backend);
                precondition(&mut app, before);
                app
            },
            // prepare — UNTRACED. Restoring the table to `before` rows costs ~2000 row
            // operations; if this were traced it would bury the op's own signal.
            |app| {
                crate::bench::precondition_reset(app, before);
            },
            // measure — TRACED. Only the op itself.
            |app| {
                crate::bench::measure_op(app, op, before);
            },
            reps,
            warmup,
            REBUILD_HINT,
        );
```

Add `precondition_reset` to `examples/rows/src/bench/mod.rs` — like `precondition` but without the initial mount frames, for an already-mounted app:

```rust
/// Reset an already-mounted app back to `rows` rows, untimed.
pub fn precondition_reset(app: &mut App, rows: usize) {
    assert!(click_op(app, "clear"), "#op-clear not found");
    let _ = step_to_quiescence(app);
    if rows > 0 {
        let op = match rows {
            1_000 => "create",
            10_000 => "create10k",
            other => panic!("unsupported precondition: {other} rows"),
        };
        assert!(click_op(app, op), "#op-{op} not found");
        let _ = step_to_quiescence(app);
    }
    assert_eq!(row_count(app), rows, "reset did not reach {rows} rows");
}
```

**The reset must be in `prepare`, never in `measure`.** That is the whole point of the
split: `prepare` restores the table to its precondition untraced, `measure` triggers
the op and steps to quiescence with recording on. If the reset leaks into `measure`,
every op's breakdown becomes ~the reset and the table stops answering the question it
exists to answer.

- [ ] **Step 5: Run to verify it passes**

```bash
cargo test --release -p rows --features bench --test driver
cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- \
  --backend vanilla --rows 1000 --reps 5 --warmup 2 --profile
```

Expected: tests PASS; the profile run prints a populated per-stage table (non-zero reconcile / flair / taffy), **not** "no system spans were recorded".

**Then re-run the Task 2 gate on BOTH examples.** This step refactors
`run_profile_with` into a wrapper over `run_profile_driven` — i.e. it changes the
very code Tasks 6 and 7 certified as behaviour-preserving. The gate's guarantee
must hold at the end of the branch, not only mid-way, so it is re-run here rather
than assumed to survive:

```bash
cd /home/tim/bevy_superui
for i in 1 2 3; do
  cargo run --release -p citadel --features bench --bin citadel-bench -- \
    --backend supersolid --frames 120 --warmup 200 --format json \
    > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-t12-$i.json
  cargo run --release -p horde --features bench --bin horde-bench -- \
    --backend supersolid --frames 300 --warmup 300 --format json \
    > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-t12-$i.json
done
cargo run --release -p citadel --features bench --bin citadel-bench -- \
  --backend supersolid --frames 120 --warmup 200 > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/citadel-t12.txt
cargo run --release -p horde --features bench --bin horde-bench -- \
  --backend supersolid --frames 300 --warmup 300 > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/horde-t12.txt

# Gate 1 + 2 against the ORIGINAL pre-extraction baselines from Task 2.
for ex in citadel horde; do
  diff <(grep -o '"[a-z_]*":' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/$ex-1.json) \
       <(grep -o '"[a-z_]*":' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/$ex-t12-1.json) \
    && echo "$ex GATE 1 PASS" || echo "$ex GATE 1 FAIL"
  diff <(sed 's/[0-9.]\+/N/g' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/$ex.txt) \
       <(sed 's/[0-9.]\+/N/g' .superpowers/sdd/2026-08-07-rows-comparative-benchmark/baselines/$ex-t12.txt) \
    && echo "$ex GATE 2 PASS" || echo "$ex GATE 2 FAIL"
done
```

Gate 3 (medians within ±10% of the Task 2 baselines) by inspection, as in Tasks 6–7.

Finally, confirm both `--profile` paths still produce populated stage tables:

```bash
cargo run --release -p citadel --features bench,bevy/trace,bevy/debug --bin citadel-bench -- \
  --profile --frames 60 --warmup 100
cargo run --release -p horde --features bench,bevy/trace,bevy/debug --bin horde-bench -- \
  --profile --preset stress --enemy-cap 400 --frames 60 --warmup 300
```

Expected: both print a populated per-stage table — **not** "no system spans were
recorded". Compare against the Task 2 baselines: same stage labels, same row order.

- [ ] **Step 6: Commit**

```bash
git add examples/rows crates/superui_bench_support
git commit -m "feat(rows): add the traced traced pass stage breakdown

Reuses the same system-name to stage mapping as horde and citadel so all
three benchmarks describe a frame the same way, which is the point of
having extracted it.

The attributor previously assumed a steady-state model where an
iteration is one app.update(). Rows measures discrete operations, so the
per-iteration driver is now a parameter; the steady-state entry point is
defined in terms of it and is unchanged for the other two examples. The
output states that its totals carry tracing overhead, so a traced pass number
is never quoted as if it were the citable one."
```

---

## Task 13: `benchmark.md`, `docs/BENCHMARKS.md`, and the published run

**Files:**
- Create: `examples/rows/benchmark.md`
- Create: `docs/BENCHMARKS.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the published tables.

- [ ] **Step 1: Write `examples/rows/benchmark.md`**

Follow the shape of `examples/citadel/benchmark.md` — purpose, backends, run commands, how to read the report, the two-pass explanation. Must state:

- both profiles matter, but never compare debug against release;
- the row markup is 8 elements + 2 text nodes per row, and the `Nodes` column counts elements;
- `<Keyed>` is deliberately absent, and why (`render.js:308`);
- untraced pass vs traced pass and which is citable.

- [ ] **Step 2: Produce the published numbers**

```bash
cd /home/tim/bevy_superui
git rev-parse HEAD                 # record this in the doc
mkdir -p .superpowers/sdd/2026-08-07-rows-comparative-benchmark/rows-results

for b in vanilla supersolid; do
  for r in 1000 10000; do
    cargo run --release -p rows --features bench --bin rows-bench -- \
      --backend $b --rows $r --reps 20 --warmup 3 --format json \
      > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/rows-results/passA-$b-$r.json
  done
done

for b in vanilla supersolid; do
  for r in 1000 10000; do
    cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- \
      --backend $b --rows $r --reps 10 --warmup 2 --profile \
      > .superpowers/sdd/2026-08-07-rows-comparative-benchmark/rows-results/passB-$b-$r.txt
  done
done
```

At 10k, drop `--reps` to 10 (spec §4.3). If the 10k spike in Task 1 failed, skip the `10000` runs and document it per Step 3.

- [ ] **Step 3: Write `docs/BENCHMARKS.md`**

Structure, mirroring bevy-react's doc so the two can be read side by side:

1. **Header** — the commit hash from Step 2 and the machine spec (CPU, RAM, GPU), as bevy-react's doc does. Without these the numbers are not citable.
2. **How to reproduce** — the exact commands from Step 2.
3. **untraced pass tables** (the citable numbers, untraced): one per scale, columns `Op | Rows | Nodes | Frames | vanilla p50 | supersolid p50`.
4. **traced pass tables** (traced): one per scale, columns `Op | Total (traced) | JS (Boa) | Reconcile | Command | Flair cascade | Taffy | Other`.
5. **A header note stating, in plain words:** untraced pass carries no tracing overhead and is what to cite; **stage columns sum to `Total (traced)`, not to `Total`**; and the per-op overhead delta between the passes.
6. **The bevy-react crosswalk table** — copy spec §5.2 verbatim.
7. **Caveats** — `<Keyed>` absence and why; the `div`-for-`table` substitution; nodes-per-row;
   the vanilla fixture using `tbody.removeChild(node)` rather than the natural
   `node.remove()`, because `Element.prototype.remove()` is unbound in `superui_api`
   (only appendChild/removeChild/insertBefore/replaceChild are bound). Same single
   detachment and no measurable
   difference, but it is a divergence from how the app would actually be written, and
   a benchmark that claims to be representative should say so; that traced pass's stage split includes the precondition reset (per Task 12 Step 4) if that is what was chosen.
8. **Findings** — write what the numbers actually show. If `swap1` beats bevy-react's 997/9997-op behaviour, say so with the measured figures; if it does not, say that instead. Do not editorialize beyond what was measured.

- [ ] **Step 4: Verify the doc against the raw output**

Re-read `docs/BENCHMARKS.md` beside the JSON in `.superpowers/sdd/2026-08-07-rows-comparative-benchmark/rows-results/`. Every number in the doc must appear in the raw output. Confirm:
- no traced pass number is presented as a untraced pass number;
- the `Frames` column is present and its values are stated;
- the commit hash matches `git rev-parse HEAD` at the time of the runs.

- [ ] **Step 5: Commit**

```bash
git add docs/BENCHMARKS.md examples/rows/benchmark.md
git commit -m "docs: publish the rows benchmark results

Gives the project a number that can be set beside another framework's
rather than only against its own history, which is what horde and
citadel -- being bespoke workloads tuned to specific reconciler changes
-- cannot provide.

The tables are split by measurement regime rather than merged for
convenience: the untraced pass is what gets cited, and the stage
breakdown sums to its own traced total, so quoting a row from the wrong
table would overstate the cost by the tracing overhead. Commit hash and
machine spec are recorded because per-op latencies are meaningless
without them."
```

---

## Self-Review

**Spec coverage:**

| spec section | task |
|---|---|
| §2 comparability contract (ops, scales, keying, determinism) | Global Constraints; Task 1 Step 4; Task 9 Step 1 |
| §3.1 markup, div substitution, nodes/row, updateColor as class | Task 1 Steps 2–4 |
| §3.2 two backends, shared stylesheet, `<For>` only | Task 1, Task 8 (+ `parity.rs` enforcement) |
| §4.1 precondition / click_effect trigger / timed frames | Task 9 |
| §4.2 quiescence predicate + `Frames` column | Task 9 Step 4; Task 10 Step 1 |
| §4.3 p50 headline, p95/p99 in JSON, rep defaults | Task 11 Step 3 |
| §5 untraced/traced split, four tables, sums-to-traced note | Tasks 11, 12, 13 |
| §5.1 stage columns, `Other` never dropped | Task 4 (bucket mapping, `Bucket::Other`) |
| §5.2 bevy-react crosswalk | Task 13 Step 3 item 6 |
| §6 extraction + §6.1 behaviour-preserving gate | Tasks 2–7 |
| §7 deliverables | File Structure |
| §8 testing | Tasks 10, 8 (`parity.rs`) |
| §9.1 frames-per-op | Task 10 Step 4 |
| §9.2 vanilla Boa bucket separability | Task 12 Step 5 — if `JS (Boa)` is empty for vanilla, document the merge per spec rather than inventing a split |
| §9.3 10k feasibility | Task 1 (front-loaded) |
| §9.4 crate naming | Task 3 Step 1 — reuses `superui_bench_support` per spec §6.1 |
| §10 verification | Tasks 6, 7 (gate), 10 (tests), 13 (doc check) |

**Known gaps, stated rather than hidden:**

- **`dhat` for rows** is wired in the manifest (`dhat-prof`) but no task exercises it. Deliberate: allocation churn is a steady-state question and rows measures discrete ops. Left available, unused.
- **`sweep_table`** has no rows equivalent — rows has exactly two scales, both published, so a sweep view adds nothing.
- **Task 12 Step 4's precondition-reset attribution** is a genuine open item, flagged in-place with a decision to make once numbers exist rather than guessed at now.

**Type consistency:** `Backend::{label,asset_dir}`, `OpSample{total_ms,frames,rows_before,nodes_after}`, `OpReport{op,rows_before,nodes,frames,total}`, `Stats{mean_ms,p50_ms,p95_ms,p99_ms,fps}`, `BenchArgs{backend:Option<String>,caps,frames,warmup,seed,json,dhat,profile}`, `ArgDefaults{frames,warmup,cap_flags}` — checked consistent across Tasks 1, 5, 9, 11, 12.
