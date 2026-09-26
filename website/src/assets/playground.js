// Reusable playground editor. The generated page sets window.__PLAYGROUND__ =
// { slug, sources:[{name,path,lang}], apply_source, apply_utilities, poll_diagnostics }.
// Source contents are NOT inlined; fetch them from `path` (resolves against the
// page's <base href="./">), same as the PARTS viewer.
const PG = window.__PLAYGROUND__;
const EDITABLE = ["app.tsx", "style.css"];
const MODE = { "app.tsx": "jsx", "style.css": "css" };

function srcFor(p) { return PG.sources.find((s) => s.path.endsWith(p)); }

const consoleEl = document.getElementById("pg-console");
function log(msg) { consoleEl.textContent += msg + "\n"; consoleEl.scrollTop = consoleEl.scrollHeight; }

const editors = {};
const contents = {}; // path-suffix -> text (kept for non-editable files, e.g. index.html)

async function fetchText(path) {
  try {
    const res = await fetch(path);
    if (!res.ok) { log("failed to load " + path + ": HTTP " + res.status); return ""; }
    return await res.text();
  } catch (e) { log("failed to load " + path + ": " + e); return ""; }
}

async function boot() {
  // Fetch every source once (editable + index.html for utilities scanning).
  for (const s of PG.sources) contents[s.name] = await fetchText(s.path);

  const host = document.getElementById("pg-editors");
  for (const path of EDITABLE) {
    if (!srcFor(path)) continue;
    const wrap = document.createElement("div");
    wrap.className = "pg-editor is-off";
    wrap.dataset.path = path;
    host.appendChild(wrap);
    editors[path] = window.CodeMirror(wrap, {
      value: contents[path] ?? "", mode: MODE[path], theme: "material-darker",
      lineNumbers: true, lineWrapping: true,
    });
  }

  document.querySelectorAll(".pg-tab").forEach((t) =>
    t.addEventListener("click", () => show(t.dataset.path)));
  document.getElementById("pg-run").addEventListener("click", runAll);

  show(EDITABLE[0]);
  runAll(); // first paint: seed CSS + utilities + tsx (UI is already mounted)
  setInterval(() => {
    const p = PG.poll_diagnostics();
    if (p && p !== "[]") log(p);
  }, 500);
}

function show(path) {
  for (const p of EDITABLE) {
    document.querySelector(`.pg-editor[data-path="${p}"]`)?.classList.toggle("is-off", p !== path);
    document.querySelector(`.pg-tab[data-path="${p}"]`)?.classList.toggle("is-active", p === path);
  }
  editors[path]?.refresh();
}

function tsxVal() { return editors["app.tsx"]?.getValue() ?? contents["app.tsx"] ?? ""; }
function cssVal() { return editors["style.css"]?.getValue() ?? contents["style.css"] ?? ""; }
function htmlVal() { return contents["index.html"] ?? ""; }

function runAll() {
  const out = [
    PG.apply_source("style.css", cssVal()),                    // 1. seed authored CSS
    PG.apply_utilities(JSON.stringify([tsxVal(), htmlVal()])), // 2. regen utilities
    PG.apply_source("app.tsx", tsxVal()),                      // 3. re-transpile + hot-swap
  ];
  // Log every diagnostic, not just ok:false: oxc recovers from most syntax
  // errors and reports them as Warning with ok:true.
  for (const r of out) {
    try {
      const j = JSON.parse(r);
      if (j && Array.isArray(j.diagnostics)) {
        for (const d of j.diagnostics) log((d.severity ? d.severity + ": " : "") + d.message);
      }
    } catch (_) {}
  }
}

boot();
