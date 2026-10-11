// VOIDFARER — a sci-fi "Flight OS" game menu, authored in Solid-style .tsx and
// run by superui's supersolid runtime on top of bevy_ui.
//
// A sci-fi game-menu design. Four screens (main / pause / systems config /
// game over) plus a bottom-right "preview" tab bar that switches between them —
// that tab bar and the config toggles are what exercise supersolid's
// fine-grained reactivity.
//
// NOTE — the app is split across sibling modules: one per screen (`main.tsx`,
// `pause.tsx`, `settings.tsx`, `gameover.tsx`), shared leaf widgets and static
// data (`widgets.tsx`, `data.tsx`), and the tab-bar chrome (`chrome.tsx`). This
// file imports them all via relative `./` paths and wires up `App` + `render`.
//
// AUTHORING GOTCHA (shared with the other supersolid examples): control-flow
// components (<Show>) must be wrapped in `{...}` inside plain elements so the
// transpiler routes them through $ss.insert (which resolves their accessor). A
// bare <Show> child lowers to $ss.child and renders nothing.

import { createSignal, Show, render } from "supersolid";
import { MainMenu } from "./main";
import { PauseMenu } from "./pause";
import { Settings } from "./settings";
import { GameOver } from "./gameover";
import { Chrome } from "./chrome";
import { STARS } from "./data";

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
