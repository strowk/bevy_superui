// Shared behaviour for the blueprint theme.
//
// Loaded by both the mdBook pages (theme/head.hbs, deferred) and the generated demo pages
// (tools/gallery/host.html.tmpl). Every step no-ops when its target is absent, so the same
// file is safe on every page kind.
//
// The header, footer and background layers are static markup in theme/index.hbs and
// tools/gallery/host.html.tmpl. Only things that cannot be known at template time belong
// in here.
//
// mdBook exposes `path_to_root` as a global; the demo template defines it by hand.
(function () {
  const ROOT = (typeof path_to_root === "string") ? path_to_root : "";

  function initLandingCounter() {
    const frame = document.getElementById("su-counter-frame");
    if (!frame) return;
    const label = document.getElementById("su-live-label");
    const overlay = document.getElementById("su-live-overlay");
    const reset = document.getElementById("su-reset");

    const arming = () => {
      if (label) label.textContent = "LIVE — BOOTING RUNTIME…";
      if (overlay) overlay.classList.remove("su-hidden");
    };
    const ready = () => {
      if (label) label.textContent = "LIVE — RUNNING IN YOUR BROWSER";
      if (overlay) overlay.classList.add("su-hidden");
    };

    window.addEventListener("message", (e) => {
      if (e.source === frame.contentWindow && e.data === "superui:ready") ready();
    });
    // Fallback if the message is missed: reveal after load + a grace delay.
    frame.addEventListener("load", () => setTimeout(ready, 4000));
    if (reset) reset.addEventListener("click", () => {
      arming();
      frame.contentWindow.location.reload();
    });
    arming();
  }

  // Home is a prefix chapter, so mdBook wires the first docs chapter's "previous"
  // arrow back to it. It is a marketing page, not something to page back into.
  function suppressLandingPrev() {
    let landing;
    try { landing = new URL(ROOT + "index.html", location.href).href; }
    catch (_) { return; }
    document.querySelectorAll(".nav-chapters.previous, .mobile-nav-chapters.previous")
      .forEach((a) => {
        const href = a.getAttribute("href");
        if (!href) return;
        try { if (new URL(href, location.href).href === landing) a.remove(); }
        catch (_) {}
      });
  }

  // Docs pages get a mono eyebrow above the H1 reading "SECTION · <part name>",
  // derived from the active sidebar entry's preceding .part-title. Renders nothing
  // when that structure is absent rather than guessing.
  function docsEyebrow() {
    const main = document.querySelector(".content main");
    if (!main || main.querySelector(".su-eyebrow")) return;
    if (document.querySelector(".superui-landing")) return;
    const h1 = main.querySelector("h1");
    if (!h1) return;

    const active = document.querySelector(".chapter li a.active");
    if (!active) return;
    const li = active.closest("li");
    if (!li) return;

    let part = null;
    for (let n = li.previousElementSibling; n; n = n.previousElementSibling) {
      if (n.classList.contains("part-title")) { part = n.textContent.trim(); break; }
    }
    if (!part) return;

    const eyebrow = document.createElement("div");
    eyebrow.className = "su-eyebrow";
    eyebrow.textContent = "SECTION · " + part.toUpperCase();
    h1.before(eyebrow);
  }

  function init() {
    docsEyebrow();
    suppressLandingPrev();
    initLandingCounter();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
  window.__superuiInit = init;
})();
