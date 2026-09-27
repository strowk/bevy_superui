// Three ways to style a superui UI, one per chip — all in this one file.
// Edit here or in style.css and press Run; the counter survives a CSS edit.

import { createSignal, render } from "supersolid";

function Row() {
  const [n, setN] = createSignal(0);
  return (
    <div class="row">
      <div class="chips">
        {/* The style attribute — for a one-off, or a value computed at runtime
           (here the width follows the counter). */}
        <div class="chip" style={`width: ${120 + n() * 8}px`}>
          <span class="chip-tag">style attribute</span>
        </div>
        {/* An authored CSS class — edit .chip-authored in style.css and Run. */}
        <div class="chip chip-authored">
          <span class="chip-tag">authored CSS</span>
        </div>
        {/* Utility classes — compose styling from class names, Tailwind-style. */}
        <div class="chip flex items-center justify-center pl-4 pr-4 pt-2 pb-2 rounded-md bg-slate-800">
          <span class="chip-tag">utility classes</span>
        </div>
      </div>
      <div class="controls">
        <button class="step" onClick={() => setN(n() - 1)}>-</button>
        <span class="readout">n = {n()}</span>
        <button class="step" onClick={() => setN(n() + 1)}>+</button>
      </div>
    </div>
  );
}

render(() => <Row />, document.getElementById("root"));
