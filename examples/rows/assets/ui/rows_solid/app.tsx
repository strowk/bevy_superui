import { createSignal, For, render } from "supersolid";

// Seeded PRNG (mulberry32) — bit-identical to rows_vanilla/app.js's `rnd`. No
// Math.random anywhere: runs must be reproducible across backends.
const ADJ = ["pretty","large","big","small","tall","short","long","handsome","plain","quaint","clean","elegant","easy","angry","crazy","helpful","mushy","odd","unsightly","adorable"];
const COL = ["red","yellow","blue","green","pink","brown","purple","white","black","orange"];
const NOU = ["table","chair","house","bbq","desk","car","pony","cookie","sandwich","burger","pizza","mouse","keyboard"];

let _seed = 1;
function rnd(n: number): number {
  _seed |= 0; _seed = (_seed + 0x6D2B79F5) | 0;
  let t = Math.imul(_seed ^ (_seed >>> 15), 1 | _seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) % n;
}
function randLabel(): string { return ADJ[rnd(20)] + " " + COL[rnd(10)] + " " + NOU[rnd(13)]; }

// <For> (mapArray in render.js) keys rows by object IDENTITY: it only reuses a
// row's DOM subtree when the same object reference reappears in the new list.
// If update ops replaced a row via spread (`{ ...row, label: ... }`), every
// touched row would get a NEW identity, so <For> would tear down and rebuild
// its whole 8-element subtree instead of patching one text/class — measuring
// "rebuild a row" where rows_vanilla (and the React reference implementation,
// via key={id}) measures "patch a value". Per-row signals keep the row object
// itself stable across updates; only the bound getter re-runs, so update ops
// patch in place and list ops (append/insert/swap/remove) still move or
// rebuild whole rows correctly since THOSE are genuinely list-level changes.
type Row = {
  id: number;
  label: () => string;
  setLabel: (v: string) => void;
  cls: () => string;
  setCls: (v: string) => void;
};

let nextId = 1;
function makeRow(): Row {
  const [label, setLabel] = createSignal(randLabel());
  const [cls, setCls] = createSignal("lbl");
  return { id: nextId++, label, setLabel, cls, setCls };
}
function make(n: number): Row[] {
  const out: Row[] = [];
  for (let i = 0; i < n; i++) out.push(makeRow());
  return out;
}

function App() {
  const [rows, setRows] = createSignal<Row[]>([]);

  const swapAt = (a: number, b: number) => setRows(rs => {
    if (rs.length <= Math.max(a, b)) return rs;
    const c = rs.slice();
    const t = c[a]; c[a] = c[b]; c[b] = t;
    return c;
  });

  // Op names/order are a frozen external contract (must line up with another
  // project's published table) — do not reorder or rename. `create10k` is an
  // extra non-measured precondition helper and stays last, mirroring
  // rows_vanilla's button order.
  const ops: Record<string, () => void> = {
    create:     () => setRows(make(1000)),
    append1:    () => setRows(rs => rs.concat(make(1))),
    append1k:   () => setRows(rs => rs.concat(make(1000))),
    insert1:    () => setRows(rs => make(1).concat(rs)),
    // Every2nd contract: N/2 operations. Insert a new row before every OTHER
    // existing row (i even), not before every row — the latter would insert N
    // rows (2x, not 1.5x) and publish a number nobody can compare.
    insertEvery2nd: () => setRows(rs => {
      const out: Row[] = [];
      for (let i = 0; i < rs.length; i++) {
        if (i % 2 === 0) out.push(make(1)[0]);
        out.push(rs[i]);
      }
      return out;
    }),
    // These four write the row's OWN signal and never touch the list signal —
    // that's what keeps the row's identity (and thus its DOM subtree) stable
    // across an update. See the Row/<For> comment above.
    updateText1: () => {
      const rs = rows();
      if (rs.length) rs[0].setLabel(randLabel());
    },
    updateTextEvery2nd: () => {
      const rs = rows();
      for (let i = 0; i < rs.length; i += 2) rs[i].setLabel(randLabel());
    },
    updateColor1: () => {
      const rs = rows();
      if (rs.length) rs[0].setCls("lbl warm");
    },
    updateColorEvery2nd: () => {
      const rs = rows();
      for (let i = 0; i < rs.length; i += 2) rs[i].setCls("lbl warm");
    },
    swap1:      () => swapAt(1, 998),
    // Adjacent disjoint pairs (0,1),(2,3),... — N/2 swaps, not N/4.
    swapEvery2nd: () => setRows(rs => {
      const c = rs.slice();
      for (let i = 0; i + 1 < c.length; i += 2) { const t = c[i]; c[i] = c[i + 1]; c[i + 1] = t; }
      return c;
    }),
    remove1:    () => setRows(rs => rs.slice(1)),
    removeEvery2nd: () => setRows(rs => rs.filter((_, i) => i % 2 === 1)),
    clear:      () => setRows([]),
    create10k:  () => setRows(make(10000)),
  };

  return (
    <div id="main">
      <div class="jumbotron">
        {Object.keys(ops).map(name => (
          <button id={"op-" + name} onClick={ops[name]}>{name}</button>
        ))}
      </div>
      <div class="table" id="tbody">
        <For each={rows()}>
          {(r: Row) => (
            <div class="row" data-id={r.id}>
              <div class="col-md-1">{r.id}</div>
              <div class="col-md-4"><a class={r.cls()}>{r.label()}</a></div>
              <div class="col-md-1"><a class="remove"><span class="glyphicon"></span></a></div>
              <div class="col-md-6"></div>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
