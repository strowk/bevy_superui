import { createSignal, createMemo, createContext, useContext, For, Keyed, Index, Show, Switch, Match, render, Accessor } from "supersolid";

// --- Frame model: mirrors the serde DTOs in src/ui/supersolid/bridge.rs ---
// Hand-authored for now; this is the exact JSON shape bevy.on("frame") delivers.
type GameState = "MainMenu" | "Playing" | "Paused" | "GameOver";

interface Slot {
  index: number; name: string; active: boolean;
  dmg: number; rof: number; spread: number; projectiles: number; mag: number; reload: number;
}
interface Enemy { id: string; sx: number; sy: number; frac: number; }
interface Dmg { id: string; sx: number; sy: number; text: string; crit: boolean; alpha: number; }
interface Blip { id: string; mx: number; my: number; kind: string; }
interface Log { text: string; alpha: number; }

interface Frame {
  state: GameState;
  player_hp: number; player_max_hp: number;
  xp: number; level: number; wave: number; kills: number; pickups: number;
  active_weapon: string | null;
  ammo: number; ammo_size: number; reloading: boolean; cooldown_frac: number;
  dps: number; elapsed: number;
  inventory: Slot[];
  enemies: Enemy[];
  damage_numbers: Dmg[];
  blips: Blip[];
  log: Log[];
}

const EMPTY: Frame = {
  state: "MainMenu",
  player_hp: 0, player_max_hp: 1, xp: 0, level: 0, wave: 0, kills: 0, pickups: 0,
  active_weapon: null, ammo: 0, ammo_size: 0, reloading: false, cooldown_frac: 0,
  dps: 0, elapsed: 0,
  inventory: [], enemies: [], damage_numbers: [], blips: [], log: [],
};

// The per-frame snapshot flows to every HUD widget through context instead of
// being drilled as an `f` prop. App provides the live `frame` accessor; each
// widget reads it with `useContext(FrameCtx)`. Default is an accessor to EMPTY so
// a widget rendered outside the provider still type-checks and reads sane values.
const FrameCtx = createContext<Accessor<Frame>>(() => EMPTY);

function intent(kind: string, index?: number) {
  bevy.send("HordeIntent", { kind, index: index || 0 });
}

function hpColor(f) {
  f = Math.max(0, Math.min(1, f));
  const r = Math.round((0.95 * (1 - f * f) + 0.10) * 255);
  const g = Math.round((0.30 + 0.62 * f) * 255);
  const b = Math.round(0.30 * 255);
  return `rgb(${r}, ${g}, ${b})`;
}
function mmss(sec) {
  const m = Math.floor(sec / 60), s = Math.floor(sec % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function PlayerStatus() {
  const f = useContext(FrameCtx);
  const hpFrac = () => f().player_hp / f().player_max_hp;
  const xpFrac = () => (f().xp % 100) / 100;
  return (
    <div class="panel" id="player-status">
      <span class="label">HP</span>
      <div class="bar-track">
        <div class="bar-fill" id="hp-fill"
             style={`width: ${Math.round(100 * hpFrac())}%; background-color: ${hpColor(hpFrac())}`}></div>
      </div>
      <span class="label">XP</span>
      <div class="bar-track">
        <div class="bar-fill xp" id="xp-fill" style={`width: ${Math.round(100 * xpFrac())}%`}></div>
      </div>
      <span class="badge" id="weapon-badge">{f().active_weapon || "—"}</span>
      <span class="ammo" id="ammo">{f().reloading ? "reloading…" : `${f().ammo} / ${f().ammo_size}`}</span>
    </div>
  );
}

function Meters() {
  const f = useContext(FrameCtx);
  return (
    <div class="panel" id="meters">
      <span>{`Wave ${f().wave}   Kills ${f().kills}   DPS ${Math.round(f().dps)}   ${mmss(f().elapsed)}`}</span>
    </div>
  );
}

function CombatLog() {
  const f = useContext(FrameCtx);
  return (
    <div class="panel" id="combat-log">
      {<For each={f().log}>
        {(line) => <span class="log-line" style={`color: rgba(237, 245, 255, ${line.alpha})`}>{line.text}</span>}
      </For>}
    </div>
  );
}

function WeaponBar() {
  const f = useContext(FrameCtx);
  return (
    <div id="weapon-bar">
      {<For each={f().inventory}>
        {(slot) => (
          <button class={slot.active ? "slot active" : "slot"} data-index={slot.index}
                  onClick={() => intent("SwitchWeapon", slot.index)}>
            {`${slot.index + 1}. ${slot.name}`}
          </button>
        )}
      </For>}
    </div>
  );
}

// The three high-frequency, per-entity overlays render through <Keyed> — an
// entity-keyed reactive store fused with rendering. Each entity id owns a stable
// row with per-FIELD signals; the per-frame snapshot is diffed into those cells,
// so a moving enemy re-runs only its own position binding (its unchanged health/
// id bindings stay put) and there is no per-frame whole-list reconcile. `e` is the
// row proxy: `e.sx` is a fine-grained reactive read.
function Minimap() {
  const f = useContext(FrameCtx);
  return (
    <div class="panel" id="minimap">
      {<Keyed each={f().blips} by="id">
        {(b) => (
          <div class={"blip " + b.kind}
               style={`left: ${Math.round(b.mx * 100)}%; top: ${Math.round(b.my * 100)}%`}></div>
        )}
      </Keyed>}
    </div>
  );
}

function Nameplates() {
  const f = useContext(FrameCtx);
  return (
    <div class="overlay" id="nameplates">
      {<Keyed each={f().enemies} by="id">
        {(e) => (
          <div class="nameplate" data-id={e.id}
               style={`left: ${Math.round(e.sx - 22)}px; top: ${Math.round(e.sy - 30)}px`}>
            <div class="np-fill"
                 style={`width: ${Math.round(e.frac * 100)}%; background-color: ${hpColor(e.frac)}`}></div>
          </div>
        )}
      </Keyed>}
    </div>
  );
}

function DamageNumbers() {
  const f = useContext(FrameCtx);
  return (
    <div class="overlay" id="damage-numbers">
      {<Keyed each={f().damage_numbers} by="id">
        {(d) => (
          <span class={d.crit ? "dmg crit" : "dmg"} data-id={d.id}
                style={`left: ${Math.round(d.sx)}px; top: ${Math.round(d.sy)}px; color: rgba(${d.crit ? "255, 199, 71" : "237, 245, 255"}, ${d.alpha})`}>
            {d.text}
          </span>
        )}
      </Keyed>}
    </div>
  );
}

function Hud() {
  return (
    <div id="playing">
      <PlayerStatus />
      <Meters />
      <CombatLog />
      <WeaponBar />
      <Minimap />
      <Nameplates />
      <DamageNumbers />
    </div>
  );
}

function Inventory(props: { onClose: () => void }) {
  const f = useContext(FrameCtx);
  return (
    <div class="modal dim" id="inventory">
      <h2 class="screen-title">Inventory (I to close)</h2>
      <div class="inv-grid">
        {<For each={f().inventory}>
          {(w) => (
            <div class={w.active ? "inv-card active" : "inv-card"}>
              <span class="inv-name">{w.name}</span>
              <span class="inv-stat">{`DMG ${Math.round(w.dmg)}   RoF ${w.rof.toFixed(2)}s`}</span>
              <span class="inv-stat">{`Spread ${w.spread.toFixed(2)}   x${w.projectiles}`}</span>
              <span class="inv-stat">{`Mag ${w.mag}   Reload ${w.reload.toFixed(1)}s`}</span>
            </div>
          )}
        </For>}
      </div>
      <button class="menu-btn" id="inv-close" onClick={() => props.onClose()}>Close</button>
    </div>
  );
}

function MainMenu() {
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  return (
    <div class="screen" id="main-menu">
      <h1 class="title" id="title">HORDE</h1>
      <span class="subtitle">survive the swarm</span>
      <button class="menu-btn" id="start" onClick={() => intent("StartGame")}>Start  (Enter)</button>
      <button class="menu-btn" id="open-settings" onClick={() => setSettingsOpen(true)}>Settings</button>
      <button class="menu-btn" id="quit" onClick={() => intent("Quit")}>Quit</button>
      {<Show when={settingsOpen()}>
        <Settings onClose={() => setSettingsOpen(false)} />
      </Show>}
    </div>
  );
}

function Settings(props: { onClose: () => void }) {
  const [cap, setCap] = createSignal(0);
  return (
    <div class="modal dim" id="settings">
      <h2 class="screen-title">Settings</h2>
      <div class="settings-row">
        <button id="cap-dec" onClick={() => bevy.send("AdjustEnemyCap", { delta: -20 })}>−</button>
        <span id="cap-label">Enemy cap ±20</span>
        <button id="cap-inc" onClick={() => bevy.send("AdjustEnemyCap", { delta: 20 })}>+</button>
      </div>
      <span class="inv-stat">UI backend: supersolid (TSX)</span>
      <button class="menu-btn" id="settings-close" onClick={() => props.onClose()}>Close</button>
    </div>
  );
}

function Pause() {
  return (
    <div class="screen dim" id="paused">
      <h2 class="screen-title">Paused</h2>
      <button class="menu-btn" id="resume" onClick={() => intent("Resume")}>Resume  (Esc)</button>
      <button class="menu-btn" id="restart" onClick={() => intent("Restart")}>Restart</button>
      <button class="menu-btn" id="pause-quit" onClick={() => intent("Quit")}>Quit</button>
    </div>
  );
}

function GameOver() {
  const f = useContext(FrameCtx);
  return (
    <div class="screen dim" id="game-over">
      <h2 class="screen-title danger">You Died</h2>
      <div class="panel stats">
        <span>{`Kills: ${f().kills}`}</span>
        <span>{`Wave reached: ${f().wave}`}</span>
        <span>{`Pickups: ${f().pickups}`}</span>
        <span>{`Time survived: ${mmss(f().elapsed)}`}</span>
      </div>
      <button class="menu-btn" id="go-restart" onClick={() => intent("Restart")}>Restart  (Enter)</button>
      <button class="menu-btn" id="go-quit" onClick={() => intent("Quit")}>Quit</button>
    </div>
  );
}

function App() {
  const [frame, setFrame] = createSignal(EMPTY);
  bevy.on("frame", (f: Frame) => setFrame(f));
  const state = createMemo(() => frame().state);
  const [invOpen, setInvOpen] = createSignal(false);
  bevy.on("toggleInventory", () => setInvOpen((o) => !o));

  return (
    <div id="hud">
      <FrameCtx.Provider value={frame}>
        {<Switch>
          <Match when={state() === "MainMenu"}><MainMenu /></Match>
          <Match when={state() === "Playing"}>
            <div id="playing-root">
              <Hud />
              {<Show when={invOpen()}>
                <Inventory onClose={() => setInvOpen(false)} />
              </Show>}
            </div>
          </Match>
          <Match when={state() === "Paused"}><Pause /></Match>
          <Match when={state() === "GameOver"}><GameOver /></Match>
        </Switch>}
      </FrameCtx.Provider>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
