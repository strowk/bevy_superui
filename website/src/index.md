<div class="superui-landing"></div>
<header class="su-lhead">
  <a class="su-brand" href="index.html">
    <img class="su-logo" src="logo.svg" alt="superui">
    <span class="su-word"><b>SUPERUI</b><small>BEVY GAME UI</small></span>
  </a>
  <nav class="su-lnav">
    <a class="su-nav-active" href="index.html">HOME</a>
    <a href="docs/">DOCS</a>
    <a href="examples/">EXAMPLES</a>
  </nav>
  <div class="su-lhead-right">
    <span class="su-chip"><span class="su-dot"></span>v0.1 · EARLY BUILD</span>
    <a class="su-gh" href="https://github.com/strowk/bevy_superui" target="_blank" rel="noopener"><svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"/></svg><span>GITHUB ↗</span></a>
  </div>
</header>
<div class="su-landing-main">
  <div class="su-hero">
    <div class="su-hero-text">
      <div class="su-eyebrow"><span class="su-eyebrow-dot"></span>SYSTEM ONLINE // UI RUNTIME</div>
      <h1 class="su-h1">Game UI<br>that speaks<br><span class="su-accent">your stack.</span></h1>
      <p class="su-tagline">Browser-like <strong>HTML / CSS / JS</strong> and Solid-style
        <strong>TSX</strong> for <strong>Bevy</strong> — fine-grained reactivity with live hot reload.</p>
      <div class="su-cta">
        <a class="su-btn su-btn-primary" href="docs/">READ THE DOCS →</a>
        <a class="su-btn su-btn-ghost" href="examples/">SEE EXAMPLES</a>
      </div>
    </div>
    <div class="su-card su-code-card">
      <div class="su-card-bar">
        <span class="su-tl su-tl-r"></span><span class="su-tl su-tl-a"></span><span class="su-tl su-tl-t"></span>
        <span class="su-card-name">counter.tsx</span>
        <span class="su-card-tag">CODE SAMPLE</span>
      </div>
      <pre class="su-code"><span class="k">function</span> <span class="fn">Counter</span>() {
  <span class="k">const</span> [count, setCount] = <span class="fn">createSignal</span>(<span class="n">0</span>);
  <span class="k">return</span> (
    &lt;<span class="tag">button</span> <span class="attr">onClick</span>={() =&gt; <span class="fn">setCount</span>(count() + <span class="n">1</span>)}&gt;
      clicked {count()} times
    &lt;/<span class="tag">button</span>&gt;
  );
}</pre>
      <div class="su-live">
        <div class="su-live-label" id="su-live-label">// LIVE PREVIEW · booting runtime…</div>
        <div class="su-live-stage">
          <iframe id="su-counter-frame" class="su-counter-frame" src="examples/counter/embed.html"
                  title="Live counter example" loading="lazy"></iframe>
          <div class="su-live-overlay" id="su-live-overlay"></div>
        </div>
        <button class="su-btn su-btn-reset" id="su-reset" type="button">reset</button>
      </div>
    </div>
  </div>
  <div class="su-features">
    <div class="su-feature"><div class="su-f-tag">01 // DOM</div><h3>Web stack</h3>
      <p>Author UI in plain HTML, CSS and JavaScript, running natively on bevy_ui.</p></div>
    <div class="su-feature"><div class="su-f-tag">02 // TSX</div><h3>Solid-style TSX</h3>
      <p>Fine-grained reactive components via the supersolid framework.</p></div>
    <div class="su-feature"><div class="su-f-tag">03 // HMR</div><h3>Hot reload</h3>
      <p>Edit .tsx and see changes live — with signal state preserved.</p></div>
    <div class="su-feature"><div class="su-f-tag">04 // API</div><h3>Familiar APIs</h3>
      <p>A browser-like DOM/CSS surface. Reuse the web knowledge you already have.</p></div>
  </div>
  <div class="su-banner">
    <span class="su-banner-chip">⚠ EARLY BUILD</span>
    <p>superui is in very early development and largely AI-generated; APIs are in flux.
       Explore the working <a href="examples/">examples</a>.</p>
  </div>
</div>
<footer class="su-footer su-lfoot">
  <span class="su-f-brand">SUPERUI</span><span>// build a12f · 2026</span>
  <span class="su-f-right">MIT / APACHE-2.0 · MADE FOR BEVY</span>
</footer>
