//! Empty stub. The DOM/Web API surface lives in `superui_js`: the JS shadow DOM
//! (`js/dom.js`) defines `document`, nodes, events, and layout reads, and each
//! engine backend registers the host imports (`console`, the `setTimeout`
//! family, `__ss_measure`, `__superui_bevy_send`). Nothing remains here; the
//! crate is kept only so dependents' manifests keep resolving.
