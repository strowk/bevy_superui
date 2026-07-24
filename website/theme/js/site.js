// superui redesign — injected on every mdBook page (additional-js).
(function () {
  function injectBackground() {
    if (document.querySelector('.su-bg')) return; // idempotent
    for (const kind of ['aurora', 'grid', 'scan']) {
      const el = document.createElement('div');
      el.className = 'su-bg su-bg--' + kind;
      document.body.insertBefore(el, document.body.firstChild);
    }
  }

  function enhanceHeader() {
    const bar = document.getElementById('mdbook-menu-bar') || document.getElementById('menu-bar');
    if (!bar || bar.querySelector('.su-brand')) return; // idempotent

    // mdBook sets a global `var path_to_root` (e.g. "../") on every page.
    const root = (typeof path_to_root === 'string') ? path_to_root : '';

    // Brand (badge + wordmark) linking to the site root, inserted after left-buttons.
    const brand = document.createElement('a');
    brand.className = 'su-brand';
    brand.href = root + 'index.html';
    brand.innerHTML =
      '<img class="su-logo" src="' + root + 'logo.svg" alt="superui">' +
      '<span class="su-word"><b>SUPERUI</b><small>BEVY GAME UI</small></span>';
    const left = bar.querySelector('.left-buttons') || bar.firstElementChild;
    left.parentNode.insertBefore(brand, left.nextSibling);

    // Right cluster: version chip + GitHub text link (default git icon hidden via CSS).
    const right = bar.querySelector('.right-buttons') || bar;
    const chip = document.createElement('span');
    chip.className = 'su-chip';
    chip.innerHTML = '<span class="su-dot"></span>v0.1 · EARLY BUILD';
    right.insertBefore(chip, right.firstChild);

    const gh = document.createElement('a');
    gh.className = 'su-gh';
    gh.href = 'https://github.com/strowk/bevy_superui';
    gh.target = '_blank';
    gh.rel = 'noopener';
    gh.innerHTML =
      '<svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true">' +
      '<path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 ' +
      '0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53' +
      '.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95' +
      ' 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27' +
      ' 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95' +
      '.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"/>' +
      '</svg><span>GITHUB ↗</span>';
    right.appendChild(gh);
  }

  function initLandingCounter() {
    const frame = document.getElementById('su-counter-frame');
    if (!frame) return; // not the landing page
    const label = document.getElementById('su-live-label');
    const overlay = document.getElementById('su-live-overlay');
    const reset = document.getElementById('su-reset');

    function arming() {
      if (label) label.textContent = '// LIVE PREVIEW · booting runtime…';
      if (overlay) overlay.classList.remove('su-hidden');
    }
    function ready() {
      if (label) label.textContent = '// LIVE PREVIEW';
      if (overlay) overlay.classList.add('su-hidden');
    }

    window.addEventListener('message', (e) => {
      if (e.source === frame.contentWindow && e.data === 'superui:ready') ready();
    });
    // Fallback: if the message is missed, reveal after the frame's load event + a grace delay.
    frame.addEventListener('load', () => setTimeout(ready, 4000));

    if (reset) reset.addEventListener('click', () => {
      arming();
      frame.contentWindow.location.reload();
    });

    arming();
  }

  // The landing (site root index.html) is a prefix chapter in SUMMARY.md, so on the
  // first docs chapter mdBook wires the "previous" arrow back to it. The landing is
  // a marketing page, not something to page back into — drop those prev arrows.
  function suppressLandingPrev() {
    const root = (typeof path_to_root === 'string') ? path_to_root : '';
    let landing;
    try { landing = new URL(root + 'index.html', location.href).href; } catch (_) { return; }
    document.querySelectorAll('.nav-chapters.previous, .mobile-nav-chapters.previous').forEach((a) => {
      const href = a.getAttribute('href');
      if (!href) return;
      try { if (new URL(href, location.href).href === landing) a.remove(); } catch (_) {}
    });
  }

  function initSuperui() {
    injectBackground();
    enhanceHeader();
    suppressLandingPrev();
    initLandingCounter();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initSuperui);
  } else {
    initSuperui();
  }
  window.__superuiInit = initSuperui;
})();
