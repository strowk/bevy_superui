import { createSignal, render } from "supersolid";

// Default/empty frame until the first bevy.on("frame") arrives.
const EMPTY = {
  state: "MainMenu",
  player_hp: 0, player_max_hp: 1, xp: 0, level: 0, wave: 0, kills: 0, pickups: 0,
  active_weapon: null, ammo: 0, ammo_size: 0, reloading: false, cooldown_frac: 0,
  dps: 0, elapsed: 0,
  inventory: [], enemies: [], damage_numbers: [], blips: [], log: [],
};

function App() {
  const [frame, setFrame] = createSignal(EMPTY);
  // Rust pushes the whole UiSnapshot+state here every frame (design §2).
  bevy.on("frame", (f) => setFrame(f));

  return (
    <div id="hud">
      <h1 id="title">HORDE</h1>
      <span id="state">{frame().state}</span>
      <div id="spike-track">
        <div id="spike-fill"
             style={`width: ${Math.round(100 * frame().player_hp / frame().player_max_hp)}%`}></div>
      </div>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
