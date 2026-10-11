// ── Static data ────────────────────────────────────────────────────────────

// A deterministic scatter of stars (no repeating-gradient tiling in bevy_ui, so
// the fine starfield is a handful of positioned dots). Golden-ratio spacing
// keeps them from clumping; alpha/size vary by index.
export const STARS = Array.from({ length: 64 }, (_, i) => ({
  x: (i * 61.803) % 100,
  y: (i * 24.79 + ((i * i) % 40)) % 100,
  a: (0.28 + ((i * 17) % 60) / 100).toFixed(2),
  s: 1 + (i % 3),
}));

export const RES_OPTS = [
  { value: "1280", label: "1280" },
  { value: "1920", label: "1920" },
  { value: "native", label: "NATIVE" },
];
export const DIFF_OPTS = [
  { value: "standard", label: "STANDARD" },
  { value: "void", label: "VOID" },
];
export const TABS = [
  { value: "main", label: "MAIN" },
  { value: "pause", label: "PAUSE" },
  { value: "settings", label: "SETTINGS" },
  { value: "over", label: "GAME OVER" },
];
