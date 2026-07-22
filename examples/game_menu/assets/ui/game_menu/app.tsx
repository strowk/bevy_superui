// VOIDFARER — a sci-fi "Flight OS" game menu, authored in Solid-style .tsx and
// run by superui's supersolid runtime on top of bevy_ui.
//
// A sci-fi game-menu design. Four screens (main / pause / systems config /
// game over) plus a bottom-right "preview" tab bar that switches between them —
// that tab bar and the config toggles are what exercise supersolid's
// fine-grained reactivity.
//
// NOTE — everything lives in this one file on purpose: superui's transpiler
// strips cross-module imports (see crates/supersolid/src/imports.rs), so
// `import { X } from "./x"` would silently drop `X`. Components are therefore
// plain functions in one module, which is the supported way to "extract" them.
//
// AUTHORING GOTCHA (shared with the other supersolid examples): control-flow
// components (<Show>) must be wrapped in `{...}` inside plain elements so the
// transpiler routes them through $ss.insert (which resolves their accessor). A
// bare <Show> child lowers to $ss.child and renders nothing.

import { createSignal, Show, render } from "supersolid";

// ── Static data ────────────────────────────────────────────────────────────

// A deterministic scatter of stars (no repeating-gradient tiling in bevy_ui, so
// the fine starfield is a handful of positioned dots). Golden-ratio spacing
// keeps them from clumping; alpha/size vary by index.
const STARS = Array.from({ length: 64 }, (_, i) => ({
  x: (i * 61.803) % 100,
  y: (i * 24.79 + ((i * i) % 40)) % 100,
  a: (0.28 + ((i * 17) % 60) / 100).toFixed(2),
  s: 1 + (i % 3),
}));

const RES_OPTS = [
  { value: "1280", label: "1280" },
  { value: "1920", label: "1920" },
  { value: "native", label: "NATIVE" },
];
const DIFF_OPTS = [
  { value: "standard", label: "STANDARD" },
  { value: "void", label: "VOID" },
];
const TABS = [
  { value: "main", label: "MAIN" },
  { value: "pause", label: "PAUSE" },
  { value: "settings", label: "SETTINGS" },
  { value: "over", label: "GAME OVER" },
];

// ── Leaf widgets ────────────────────────────────────────────────────────────

function MenuItem(props) {
  return (
    <div
      class={
        "menu-item" +
        (props.primary ? " primary" : "") +
        (props.danger ? " danger" : "")
      }
      onClick={props.onClick}
    >
      <div class="mi-left">
        <span class={"mi-num" + (props.primary ? " on" : "")}>{props.num}</span>
        <span class={"mi-title" + (props.primary ? " on" : "")}>{props.title}</span>
      </div>
      <span class="mi-hint">{props.hint}</span>
    </div>
  );
}

function Toggle(props) {
  // Drive the knob with a single `left` value (never swap left<->right): setting
  // one side then removing it leaves the unset side with no baseline, which makes
  // flair warn "Cannot set property … to None". 44px track - 16px knob - 2px = 26.
  return (
    <div class={"toggle" + (props.on ? " on" : "")} onClick={props.onClick}>
      <div class="knob" style={"left: " + (props.on ? "26px" : "2px")}></div>
    </div>
  );
}

function Slider(props) {
  return (
    <div class="slider">
      <div class="slider-fill" style={"width: " + props.value + "%"}></div>
      <div class="slider-handle" style={"left: " + props.value + "%"}></div>
    </div>
  );
}

function Segmented(props) {
  return (
    <div class="seg">
      {props.options.map((opt) => (
        <div
          class={"seg-opt" + (props.value === opt.value ? " active" : "")}
          onClick={() => props.onSelect(opt.value)}
        >
          {opt.label}
        </div>
      ))}
    </div>
  );
}

function ConfigRow(props) {
  return (
    <div class="cfg-row">
      <span class="cfg-label">{props.label}</span>
      {props.children}
    </div>
  );
}

function Stat(props) {
  return (
    <div class="stat">
      <span class="stat-label">{props.label}</span>
      <span class={"stat-val" + (props.accent ? " accent" : "")}>{props.value}</span>
    </div>
  );
}

// ── Screens ─────────────────────────────────────────────────────────────────

function MainMenu(props) {
  return (
    <div class="screen main">
      <div class="brand">
        <div class="emblem">
          <div class="emblem-inner">
            <div class="emblem-core"></div>
          </div>
        </div>
        <div class="brand-text">
          <div class="title">
            <span>VOID</span>
            <span class="accent">FARER</span>
          </div>
          <div class="tagline">
            <span>DEEP-VOID STATION ASSEMBLY</span>
            <span class="cursor">_</span>
          </div>
        </div>
      </div>
      <div class="divider"></div>
      <div class="menu-list">
        <MenuItem num="01" title="NEW EXPEDITION" hint="LAUNCH >" primary={true}
                  onClick={() => props.nav("over")} />
        <MenuItem num="02" title="LOAD ARCHIVE" hint="3 SAVES >"
                  onClick={() => props.nav("over")} />
        <MenuItem num="03" title="SYSTEMS CONFIG" hint=">"
                  onClick={() => props.nav("settings")} />
        <MenuItem num="04" title="POWER DOWN" hint="QUIT >" danger={true}
                  onClick={() => props.nav("over")} />
      </div>
    </div>
  );
}

function PauseMenu(props) {
  return (
    <div class="screen overlay">
      <div class="pause-card">
        <div class="card-head">
          <div class="blip"></div>
          <span class="card-head-text">// OPERATIONS SUSPENDED</span>
        </div>
        <div class="pause-title">PAUSED</div>
        <div class="pause-sub">SIMULATION HALTED // LIFE SUPPORT NOMINAL</div>
        <div class="btn-col">
          <div class="btn primary" onClick={() => props.nav("main")}>{"> RESUME OPERATIONS"}</div>
          <div class="btn">SAVE STATE</div>
          <div class="btn" onClick={() => props.nav("settings")}>SYSTEMS CONFIG</div>
          <div class="btn quiet" onClick={() => props.nav("main")}>ABANDON TO MENU</div>
        </div>
      </div>
    </div>
  );
}

function Settings(props) {
  // Local, interactive config state — flipping these updates only the affected
  // widget (fine-grained reactivity), no full re-render.
  const [resolution, setResolution] = createSignal("1920");
  const [difficulty, setDifficulty] = createSignal("standard");
  const [vsync, setVsync] = createSignal(true);
  const [fps, setFps] = createSignal(true);
  const [camera, setCamera] = createSignal(false);

  return (
    <div class="screen overlay">
      <div class="settings-card">
        <div class="settings-head">
          <span class="settings-title">SYSTEMS CONFIG</span>
          <div class="close-btn" onClick={() => props.nav("main")}>{"< RETURN [ESC]"}</div>
        </div>
        <div class="settings-body">
          <div class="settings-col">
            <div class="section">DISPLAY</div>
            <ConfigRow label="Resolution">
              <Segmented options={RES_OPTS} value={resolution()} onSelect={setResolution} />
            </ConfigRow>
            <ConfigRow label="V-Sync">
              <Toggle on={vsync()} onClick={() => setVsync(!vsync())} />
            </ConfigRow>
            <ConfigRow label="FPS counter">
              <Toggle on={fps()} onClick={() => setFps(!fps())} />
            </ConfigRow>
            <ConfigRow label="Screen shake">
              <Slider value={60} />
            </ConfigRow>
          </div>
          <div class="settings-col">
            <div class="section">AUDIO</div>
            <ConfigRow label="Master">
              <Slider value={80} />
            </ConfigRow>
            <ConfigRow label="Effects">
              <Slider value={70} />
            </ConfigRow>
            <ConfigRow label="Music">
              <Slider value={45} />
            </ConfigRow>
            <div class="section gap">GAMEPLAY</div>
            <ConfigRow label="Difficulty">
              <Segmented options={DIFF_OPTS} value={difficulty()} onSelect={setDifficulty} />
            </ConfigRow>
            <ConfigRow label="Camera follow">
              <Toggle on={camera()} onClick={() => setCamera(!camera())} />
            </ConfigRow>
          </div>
        </div>
      </div>
    </div>
  );
}

function GameOver(props) {
  return (
    <div class="screen over">
      <div class="over-tag">
        <div class="blip danger"></div>
        <span class="over-tag-text">// TELEMETRY DOWNLINK TERMINATED</span>
      </div>
      <div class="over-title">SIGNAL LOST</div>
      <div class="over-sub">HULL INTEGRITY 0% // STATION DESTROYED IN SECTOR 7G</div>
      <div class="statrow">
        <Stat label="TIME SURVIVED" value="00:42:17" />
        <Stat label="MODULES DEPLOYED" value="23" />
        <Stat label="THREATS NEUTRALIZED" value="148" accent={true} />
      </div>
      <div class="action-row">
        <div class="action solid" onClick={() => props.nav("main")}>{"> RELAUNCH EXPEDITION"}</div>
        <div class="action" onClick={() => props.nav("main")}>LOAD ARCHIVE</div>
        <div class="action" onClick={() => props.nav("main")}>MAIN MENU</div>
      </div>
    </div>
  );
}

// ── Chrome (top / bottom bars) ──────────────────────────────────────────────

// The bars are returned as a FRAGMENT, not wrapped in a full-screen div, so they
// become direct children of `.stage` — the same stacking context as the `.screen`
// panels. That matters because flair maps `z-index` to bevy_ui's *local* ZIndex:
// inside a wrapper, the bars' `z-index:100` would only order them against each
// other, leaving the whole wrapper (z-index 0) *below* the screens (z-index 20).
// A full-screen, transparent-but-pickable `.screen` would then sit on top of the
// bars and swallow every click aimed at a tab. As siblings of the screens, the
// bars' `z-index:100` wins globally, so tabs stay clickable — and since the bars
// only cover thin top/bottom strips, they don't block the centered screen content.
function Chrome(props) {
  return (
    <>
      <div class="topbar">
        <div class="mono">
          <span>VOIDFARER FLIGHT OS </span>
          <span class="accent">v0.5.0</span>
        </div>
        <span class="mono">BUILD 2026-07-17 // BEVY RUNTIME // SECTOR 7G</span>
      </div>
      <div class="botbar">
        <span class="mono small">[UP/DN] NAVIGATE     [ENTER] SELECT     [ESC] BACK</span>
        <div class="tabs">
          <span class="tabs-label">PREVIEW</span>
          {TABS.map((t) => (
            <div
              class={"tab" + (props.screen === t.value ? " active" : "")}
              onClick={() => props.nav(t.value)}
            >
              {t.label}
            </div>
          ))}
        </div>
      </div>
    </>
  );
}

// ── Root ────────────────────────────────────────────────────────────────────

function App() {
  const [screen, setScreen] = createSignal("main");
  const nav = (name) => setScreen(name);

  return (
    <div class="stage">
      <div class="backdrop nebula"></div>
      <div class="backdrop vignette"></div>
      <div class="starfield">
        {STARS.map((st) => (
          <div
            class="star"
            style={
              "left: " + st.x + "%; top: " + st.y + "%; width: " + st.s +
              "px; height: " + st.s + "px; background-color: rgba(200,220,240," + st.a + ")"
            }
          ></div>
        ))}
      </div>

      {<Show when={screen() === "main"}>
        <MainMenu nav={nav} />
      </Show>}
      {<Show when={screen() === "pause"}>
        <PauseMenu nav={nav} />
      </Show>}
      {<Show when={screen() === "settings"}>
        <Settings nav={nav} />
      </Show>}
      {<Show when={screen() === "over"}>
        <GameOver nav={nav} />
      </Show>}

      <Chrome screen={screen()} nav={nav} />
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
