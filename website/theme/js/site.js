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
    const bar = document.getElementById('menu-bar');
    if (!bar || bar.querySelector('.su-brand')) return; // idempotent

    // mdBook sets a global `var path_to_root` (e.g. "../") on every page.
    const root = (typeof path_to_root === 'string') ? path_to_root : '';

    // Brand (badge + wordmark) linking to the site root, inserted after left-buttons.
    const brand = document.createElement('a');
    brand.className = 'su-brand';
    brand.href = root + 'index.html';
    brand.innerHTML =
      '<span class="su-badge">SU</span>' +
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
    gh.textContent = 'GITHUB ↗';
    right.appendChild(gh);
  }

  function initSuperui() {
    injectBackground();
    enhanceHeader();
    // Task 7 adds: initLandingCounter();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initSuperui);
  } else {
    initSuperui();
  }
  window.__superuiInit = initSuperui;
})();
