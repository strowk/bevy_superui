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

  function initSuperui() {
    injectBackground();
    // Task 3 adds: enhanceHeader();
    // Task 7 adds: initLandingCounter();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initSuperui);
  } else {
    initSuperui();
  }
  window.__superuiInit = initSuperui;
})();
