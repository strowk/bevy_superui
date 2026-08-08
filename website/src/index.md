<div class="superui-landing"></div>
<div class="su-landing-main">
  <div class="su-hero">
    <div class="su-hero-text">
      <div class="su-tickline"><span></span>RENDERS THROUGH BEVY_UI</div>
      <h1 class="su-h1">Game UI you<br>already know<br><span class="su-accent">how to write.</span></h1>
      <p class="su-tagline">Browser-grade <strong>HTML, CSS and JS</strong> — plus Solid-style
        <strong>TSX</strong> — driving real <strong>Bevy</strong> interfaces. Fine-grained
        reactivity, hot reload, no new mental model.</p>
      <div class="su-cta">
        <a class="su-btn su-btn-primary" href="docs/">READ THE DOCS</a>
        <a class="su-btn su-btn-ghost" href="examples/">SEE EXAMPLES</a>
      </div>
      <div class="su-stack"><span>RUST</span><i>+</i><span>BEVY_UI</span><i>+</i><span>WASM</span><i>+</i><span>NATIVE</span></div>
    </div>
    <div class="su-hero-panel">
      <div class="su-panel-label"><span class="su-accent">DETAIL A</span><span>counter.tsx</span><span class="su-panel-label-end">SCALE 1:1</span></div>
      <div class="su-frame">
        <pre class="su-code"><span class="k">function</span> <span class="fn">Counter</span>() {
  <span class="k">const</span> [count, setCount] = <span class="fn">createSignal</span>(<span class="n">0</span>);
  <span class="k">return</span> (
    &lt;<span class="tag">button</span> <span class="attr">onClick</span>={() =&gt; <span class="fn">setCount</span>(count() + <span class="n">1</span>)}&gt;
      clicked {count()} times
    &lt;/<span class="tag">button</span>&gt;
  );
}</pre>
        <div class="su-live">
          <div class="su-tickline" id="su-live-label"><span></span>LIVE — BOOTING RUNTIME…</div>
          <div class="su-live-stage">
            <iframe id="su-counter-frame" class="su-counter-frame" src="examples/counter/embed.html"
                    title="Live counter example" loading="lazy"></iframe>
            <div class="su-live-overlay" id="su-live-overlay"></div>
          </div>
          <button class="su-btn su-btn-reset" id="su-reset" type="button">RESET</button>
        </div>
      </div>
    </div>
  </div>

  <div class="su-section-head">
    <h2>Principle of operation</h2><div class="su-dash"></div><span>4 CALLOUTS</span>
  </div>
  <div class="su-features">
    <div class="su-feature"><div class="su-num">1</div><h3>Web stack</h3>
      <p>Author interfaces in plain HTML, CSS and JavaScript, rendered natively through bevy_ui.</p>
      <div class="su-feature-tag">DOM SURFACE</div></div>
    <div class="su-feature"><div class="su-num">2</div><h3>Solid-style TSX</h3>
      <p>Fine-grained reactive components driven by signals — no virtual DOM diffing.</p>
      <div class="su-feature-tag">SUPERSOLID</div></div>
    <div class="su-feature"><div class="su-num">3</div><h3>Hot reload</h3>
      <p>Edit a .tsx file and watch the running game update, signal state intact.</p>
      <div class="su-feature-tag">NATIVE BUILDS</div></div>
    <div class="su-feature"><div class="su-num">4</div><h3>Familiar APIs</h3>
      <p>A browser-like DOM and CSS surface, so web knowledge transfers directly.</p>
      <div class="su-feature-tag">ZERO RELEARN</div></div>
  </div>

  <div class="su-note">
    <div class="su-note-rail">!</div>
    <div class="su-note-body">
      <div class="su-note-title">Early build — not final</div>
      <p>superui is in very early development and largely AI-generated. APIs are subject to
         change without notice. Working demos are in the <a href="examples/">examples</a>.</p>
    </div>
  </div>
</div>
