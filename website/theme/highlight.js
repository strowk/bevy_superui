// superui: syntax highlighting is done at BUILD TIME by the Shiki mdBook
// preprocessor (tools/mdbook-shiki), which emits pre-colored <pre class="shiki">
// markup. This file replaces mdBook's bundled highlight.js so the client-side
// highlighter never runs and clobbers that markup. Every method mdBook's book.js
// may call is a safe no-op.
window.hljs = {
  configure() {},
  highlight() { return { value: "" }; },
  highlightAll() {},
  highlightBlock() {},
  highlightElement() {},
  highlightAuto() { return { value: "" }; },
  registerLanguage() {},
  registerAliases() {},
  listLanguages() { return []; },
  getLanguage() { return undefined; },
  initHighlighting() {},
  initHighlightingOnLoad() {},
};
