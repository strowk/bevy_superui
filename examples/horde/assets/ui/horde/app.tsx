import { createSignal, createMemo, For, Show, Switch, Match, render } from "supersolid";

const EMPTY = {
  state: "MainMenu",
  player_hp: 0, player_max_hp: 1, xp: 0, level: 0, wave: 0, kills: 0, pickups: 0,
  active_weapon: null, ammo: 0, ammo_size: 0, reloading: false, cooldown_frac: 0,
  dps: 0, elapsed: 0,
  inventory: [], enemies: [], damage_numbers: [], blips: [], log: [],
};

function intent(kind, index) {
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

function PlayerStatus(props) {
  const f = props.f;
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

function Meters(props) {
  const f = props.f;
  return (
    <div class="panel" id="meters">
      <span>{`Wave ${f().wave}   Kills ${f().kills}   DPS ${Math.round(f().dps)}   ${mmss(f().elapsed)}`}</span>
    </div>
  );
}

function CombatLog(props) {
  return (
    <div class="panel" id="combat-log">
      {<For each={props.f().log}>
        {(line) => <span class="log-line" style={`opacity: ${line.alpha}`}>{line.text}</span>}
      </For>}
    </div>
  );
}

function Hud(props) {
  return (
    <div id="playing">
      <PlayerStatus f={props.f} />
      <Meters f={props.f} />
      <CombatLog f={props.f} />
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

// Placeholder; real body added in Task B6.
function Settings(props) {
  return (
    <div class="modal" id="settings">
      <button id="settings-close" onClick={() => props.onClose()}>Close</button>
    </div>
  );
}

function App() {
  const [frame, setFrame] = createSignal(EMPTY);
  bevy.on("frame", (f) => setFrame(f));
  const state = createMemo(() => frame().state);

  return (
    <div id="hud">
      {<Switch>
        <Match when={state() === "MainMenu"}><MainMenu /></Match>
        <Match when={state() === "Playing"}><Hud f={frame} /></Match>
        <Match when={state() === "Paused"}><div id="paused" /></Match>
        <Match when={state() === "GameOver"}><div id="game-over" /></Match>
      </Switch>}
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
