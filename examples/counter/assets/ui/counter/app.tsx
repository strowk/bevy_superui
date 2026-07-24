// The smallest Supersolid app — a button that counts its own clicks.
//
// NOTE — everything lives in this one file on purpose: superui's transpiler
// strips cross-module imports, so components are plain functions in one module.

import { createSignal, render } from "supersolid";

function Counter() {
  const [count, setCount] = createSignal(0);
  return (
    <button class="counter" onClick={() => setCount(count() + 1)}>
      clicked {count()} times
    </button>
  );
}

render(() => <Counter />, document.getElementById("root"));
