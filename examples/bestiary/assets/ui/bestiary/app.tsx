import { createSignal, For, onMount, render } from "supersolid";

function App() {
  const [creatures, setCreatures] = createSignal([]);

  onMount(() => {
    bevy.on("bestiary", (data) => setCreatures(data.creatures));
    bevy.send("uiReady", null);
  });

  return (
    <div class="bestiary">
      <For each={creatures()}>{(c, i) => (
        <div class={`card ${c.element}`} id={`creature-${i()}`}>
          <span class="name">{c.name}</span>
          <span class="element">{c.element}</span>
          <span class="stat">HP {c.hp}</span>
          <span class="stat">ATK {c.attack}</span>
        </div>
      )}</For>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
