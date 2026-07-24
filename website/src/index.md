<div class="superui-landing"></div>
<header class="su-lhead">
  <a class="su-brand" href="index.html">
    <span class="su-badge">SU</span>
    <span class="su-word"><b>SUPERUI</b><small>BEVY GAME UI</small></span>
  </a>
  <nav class="su-lnav">
    <a class="su-nav-active" href="index.html">HOME</a>
    <a href="docs/">DOCS</a>
    <a href="examples/">EXAMPLES</a>
  </nav>
  <div class="su-lhead-right">
    <span class="su-chip"><span class="su-dot"></span>v0.1 · EARLY BUILD</span>
    <a class="su-gh" href="https://github.com/strowk/bevy_superui" target="_blank" rel="noopener">GITHUB ↗</a>
  </div>
</header>
<main class="su-landing-main">
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
</main>
<footer class="su-footer su-lfoot">
  <span class="su-f-brand">SUPERUI</span><span>// build a12f · 2026</span>
  <span class="su-f-right">MIT / APACHE-2.0 · MADE FOR BEVY</span>
</footer>
