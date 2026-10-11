// ── Leaf widgets ────────────────────────────────────────────────────────────

export function MenuItem(props) {
  return (
    <div
      class={
        "menu-item" +
        (props.primary ? " primary" : "") +
        (props.danger ? " danger" : "")
      }
      onClick={props.onClick}
    >
      <div class="mi-left">
        <span class={"mi-num" + (props.primary ? " on" : "")}>{props.num}</span>
        <span class={"mi-title" + (props.primary ? " on" : "")}>{props.title}</span>
      </div>
      <span class="mi-hint">{props.hint}</span>
    </div>
  );
}

export function Toggle(props) {
  // Drive the knob with a single `left` value (never swap left<->right): setting
  // one side then removing it leaves the unset side with no baseline, which makes
  // flair warn "Cannot set property … to None". 44px track - 16px knob - 2px = 26.
  return (
    <div class={"toggle" + (props.on ? " on" : "")} onClick={props.onClick}>
      <div class="knob" style={"left: " + (props.on ? "26px" : "2px")}></div>
    </div>
  );
}

export function Slider(props) {
  return (
    <div class="slider">
      <div class="slider-fill" style={"width: " + props.value + "%"}></div>
      <div class="slider-handle" style={"left: " + props.value + "%"}></div>
    </div>
  );
}

export function Segmented(props) {
  return (
    <div class="seg">
      {props.options.map((opt) => (
        <div
          class={"seg-opt" + (props.value === opt.value ? " active" : "")}
          onClick={() => props.onSelect(opt.value)}
        >
          {opt.label}
        </div>
      ))}
    </div>
  );
}

export function ConfigRow(props) {
  return (
    <div class="cfg-row">
      <span class="cfg-label">{props.label}</span>
      {props.children}
    </div>
  );
}

export function Stat(props) {
  return (
    <div class="stat">
      <span class="stat-label">{props.label}</span>
      <span class={"stat-val" + (props.accent ? " accent" : "")}>{props.value}</span>
    </div>
  );
}
