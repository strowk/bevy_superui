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
        <Match when={state() === "Playing"}><div id="playing" /></Match>
        <Match when={state() === "Paused"}><div id="paused" /></Match>
        <Match when={state() === "GameOver"}><div id="game-over" /></Match>
      </Switch>}
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
