import { TABS } from "./data";

// ── Chrome (top / bottom bars) ──────────────────────────────────────────────

// The bars are returned as a FRAGMENT, not wrapped in a full-screen div, so they
// become direct children of `.stage` — the same stacking context as the `.screen`
// panels. That matters because flair maps `z-index` to bevy_ui's *local* ZIndex:
// inside a wrapper, the bars' `z-index:100` would only order them against each
// other, leaving the whole wrapper (z-index 0) *below* the screens (z-index 20).
// A full-screen, transparent-but-pickable `.screen` would then sit on top of the
// bars and swallow every click aimed at a tab. As siblings of the screens, the
// bars' `z-index:100` wins globally, so tabs stay clickable — and since the bars
// only cover thin top/bottom strips, they don't block the centered screen content.
export function Chrome(props) {
  return (
    <>
      <div class="topbar">
        <div class="mono">
          <span>VOIDFARER FLIGHT OS </span>
          <span class="accent">v0.5.0</span>
        </div>
        <span class="mono">BUILD 2026-07-17 // BEVY RUNTIME // SECTOR 7G</span>
      </div>
      <div class="botbar">
        <span class="mono small">[UP/DN] NAVIGATE     [ENTER] SELECT     [ESC] BACK</span>
        <div class="tabs">
          <span class="tabs-label">PREVIEW</span>
          {TABS.map((t) => (
            <div
              class={"tab" + (props.screen === t.value ? " active" : "")}
              onClick={() => props.nav(t.value)}
            >
              {t.label}
            </div>
          ))}
        </div>
      </div>
    </>
  );
}
