<div class="superui-landing"></div>

<section class="hero">
  <h1>superui</h1>
  <p class="tagline">Browser-like HTML/CSS/JS &amp; Solid-style TSX game UI for
  Bevy — with hot reload.</p>
  <div class="cta">
    <a class="btn btn-primary" href="docs/">Read the docs</a>
    <a class="btn" href="examples/">See examples</a>
    <a class="btn" href="https://github.com/strowk/bevy_superui">GitHub</a>
  </div>
</section>

Write reactive UI the way you already know:

```jsx
function Counter() {
  const [count, setCount] = createSignal(0);
  return (
    <button onClick={() => setCount(count() + 1)}>
      clicked {count()} times
    </button>
  );
}
```

<section class="features">
  <div class="feature"><h3>Web stack</h3><p>Author UI in plain HTML, CSS and
  JavaScript, running on <code>bevy_ui</code>.</p></div>
  <div class="feature"><h3>Solid-style TSX</h3><p>Fine-grained reactive
  components via the supersolid framework.</p></div>
  <div class="feature"><h3>Hot reload</h3><p>Edit <code>.tsx</code> and see
  changes live, with state preserved.</p></div>
  <div class="feature"><h3>Familiar APIs</h3><p>A browser-like DOM/CSS surface —
  reuse your web knowledge.</p></div>
</section>

<section class="status-note">
  <strong>Early stage:</strong> superui is in very early development and largely
  AI-generated; APIs are in flux. Explore the working
  <a href="examples/">examples</a>.
</section>
