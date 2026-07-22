import { createSignal, createMemo, Keyed, render } from "supersolid";

// Empty frame shape (mirrors FrameDto). Keeps the first render before any
// `frame` event well-formed so every list renders an (empty) container.
const EMPTY = {
  clock: 0, tick: 0,
  resources: [], buildings: [], units: [], techs: [], events: [],
};

function mmss(sec) {
  const m = Math.floor(sec / 60), s = Math.floor(sec % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

// Round a resource value to a compact string (e.g. 12.3k).
function amt(v) {
  if (v >= 10000) return `${(v / 1000).toFixed(1)}k`;
  return `${Math.round(v)}`;
}
function rate(v) {
  return v >= 0 ? `+${v.toFixed(1)}` : v.toFixed(1);
}

// Synthesize a plausible per-card cost line from tier + category. Purely static
// (depends only on stable fields) so it never re-runs per frame.
function costLine(b) {
  const base = b.tier * 40;
  const a = base;
  const c = base + (b.category === "military" ? 25 : b.category === "science" ? 15 : 10);
  return `${a} min   ${c} all`;
}

// ── Top ledger: ~8 resource chips + mission clock ───────────────────────────
// These read frame() directly (few nodes) — the live top-level region.
function Ledger(props) {
  const f = props.f;
  return (
    <div id="ledger">
      <div class="ledger-brand">
        <span class="brand-mark">◆</span>
        <span class="brand-name">CITADEL</span>
      </div>
      <div class="chips">
        {<Keyed each={f().resources} by="id">
          {(r) => (
            <div class="chip" data-id={r.id}>
              <span class="chip-icon">{r.icon}</span>
              <div class="chip-body">
                <span class="chip-name">{r.name}</span>
                <div class="chip-nums">
                  <span class="chip-val">{amt(r.current)}</span>
                  <span class={r.rate >= 0 ? "chip-rate up" : "chip-rate down"}>{rate(r.rate)}</span>
                </div>
              </div>
            </div>
          )}
        </Keyed>}
      </div>
      <div class="clock-box">
        <span class="clock-label">MISSION</span>
        <span class="clock" id="mission-clock">{mmss(f().clock)}</span>
      </div>
    </div>
  );
}

// ── Left: tech rail ─────────────────────────────────────────────────────────
function TechRail(props) {
  const f = props.f;
  return (
    <div class="rail" id="tech-rail">
      <div class="rail-head">
        <span class="rail-title">RESEARCH</span>
        <span class="rail-sub">tech tree</span>
      </div>
      <div class="rail-list">
        {<Keyed each={f().techs} by="id">
          {(t) => (
            <div class={"tech " + t.state} data-id={t.id}>
              <span class="tech-dot"></span>
              <div class="tech-body">
                <span class="tech-name">{t.name}</span>
                <div class="tech-track">
                  <div class="tech-fill" style={`width: ${Math.round(t.progress * 100)}%`}></div>
                </div>
              </div>
              <span class="tech-state">{t.state}</span>
            </div>
          )}
        </Keyed>}
      </div>
    </div>
  );
}

// ── Center: production grid of building cards ───────────────────────────────
function ProductionGrid(props) {
  const f = props.f;
  return (
    <div id="production">
      <div class="prod-head">
        <span class="prod-title">PRODUCTION</span>
        <span class="prod-sub">imperial construction registry</span>
      </div>
      <div class="grid" id="build-grid">
        {<Keyed each={f().buildings} by="id">
          {(b) => (
            <div class={"card " + b.category + " tier-" + b.tier + " " + b.state} data-id={b.id}>
              <div class="card-top">
                <span class="card-name">{b.name}</span>
                <span class={"tier-dot t" + b.tier}></span>
              </div>
              <div class="card-tags">
                <span class={"tag cat " + b.category}>{b.category}</span>
                <span class="tag lvl">{`Lvl ${b.level}`}</span>
                <span class={b.affordable ? "tag afford ok" : "tag afford no"}>{b.affordable ? "ready" : "short"}</span>
              </div>
              <div class="card-cost">
                <span class="cost-label">COST</span>
                <span class="cost-vals">{costLine(b)}</span>
              </div>
              <div class="card-track">
                <div class="card-fill" style={`width: ${Math.round(b.progress * 100)}%`}></div>
              </div>
              <div class="card-foot">
                <span class={"badge st " + b.state}>{b.state}</span>
                <span class="card-tierlab">{`T${b.tier}`}</span>
              </div>
            </div>
          )}
        </Keyed>}
      </div>
    </div>
  );
}

// ── Right: unit roster ──────────────────────────────────────────────────────
function UnitRoster(props) {
  const f = props.f;
  return (
    <div class="side-panel" id="roster">
      <div class="side-head">
        <span class="side-title">FLEET ROSTER</span>
      </div>
      <div class="roster-list">
        {<Keyed each={f().units} by="id">
          {(u) => (
            <div class={"unit " + u.status} data-id={u.id}>
              <span class="unit-glyph">▣</span>
              <span class="unit-name">{u.name}</span>
              <span class="unit-count">{`x${u.count}`}</span>
              <span class={"unit-status " + u.status}>{u.status}</span>
            </div>
          )}
        </Keyed>}
      </div>
    </div>
  );
}

// ── Right: build queue (buildings currently `building`) ─────────────────────
function BuildQueue(props) {
  const f = props.f;
  const queued = createMemo(() => f().buildings.filter((b) => b.state === "building"));
  return (
    <div class="side-panel" id="queue">
      <div class="side-head">
        <span class="side-title">BUILD QUEUE</span>
      </div>
      <div class="queue-list">
        {<Keyed each={queued()} by="id">
          {(b) => (
            <div class={"qrow tier-" + b.tier} data-id={b.id}>
              <span class="q-name">{b.name}</span>
              <div class="q-track">
                <div class="q-fill" style={`width: ${Math.round(b.progress * 100)}%`}></div>
              </div>
              <span class="q-pct">{`${Math.round(b.progress * 100)}%`}</span>
            </div>
          )}
        </Keyed>}
      </div>
    </div>
  );
}

// ── Right: event log (fading lines) ─────────────────────────────────────────
function EventLog(props) {
  const f = props.f;
  return (
    <div class="side-panel" id="events">
      <div class="side-head">
        <span class="side-title">DISPATCHES</span>
      </div>
      <div class="event-list">
        {<Keyed each={f().events} by="id">
          {(e) => (
            <span class="event-line" data-id={e.id}
                  style={`color: rgba(206, 221, 245, ${Math.max(0.28, 1 - e.age * 0.06).toFixed(3)})`}>
              {"> " + e.text}
            </span>
          )}
        </Keyed>}
      </div>
    </div>
  );
}

function App() {
  const [frame, setFrame] = createSignal(EMPTY);
  bevy.on("frame", (f) => {
    // Events have no id in the DTO; synthesize a stable key from index so
    // <Keyed> has a `by` field.
    if (f.events) f.events.forEach((e, i) => { e.id = i; });
    setFrame(f);
  });

  return (
    <div id="hud">
      <Ledger f={frame} />
      <div id="body">
        <TechRail f={frame} />
        <ProductionGrid f={frame} />
        <div id="right-column">
          <UnitRoster f={frame} />
          <BuildQueue f={frame} />
          <EventLog f={frame} />
        </div>
      </div>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
