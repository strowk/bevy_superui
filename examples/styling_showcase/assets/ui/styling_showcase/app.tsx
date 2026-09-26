// One module on purpose: superui's transpiler strips cross-module imports, so
// every component is a plain function here. This demo shows the three ways to
// style a superui UI, one per chip, sharing a single reactive signal so a live
// CSS edit visibly preserves state.

import { createSignal, render } from "supersolid";

function Row() {
  const [n, setN] = createSignal(0);
  return (
    <div class="row">
      <div class="controls">
        <button class="step" onClick={() => setN(n() - 1)}>-</button>
        <span class="readout">n = {n()}</span>
        <button class="step" onClick={() => setN(n() + 1)}>+</button>
      </div>
      <div class="chips">
        {/* A: inline style, width driven by the signal (runtime-computed) */}
        <div class="chip" style={`width: ${120 + n() * 8}px`}>
          <span class="chip-tag">inline style</span>
        </div>
        {/* B: authored CSS — edit .chip-authored in style.css and Run */}
        <div class="chip chip-authored">
          <span class="chip-tag">authored CSS</span>
        </div>
        {/* C: utility classes — regenerated in-browser by apply_utilities */}
        <div class="chip flex items-center justify-center pl-4 pr-4 pt-2 pb-2 rounded-md bg-slate-800">
          <span class="chip-tag">utility classes</span>
        </div>
      </div>
    </div>
  );
}

render(() => <Row />, document.getElementById("root"));
