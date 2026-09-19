// Seeded PRNG (mulberry32). No Math.random anywhere — runs must be reproducible.
var _seed = 1;
function srand(s) { _seed = s >>> 0; }
function rnd(n) {
  _seed |= 0; _seed = (_seed + 0x6D2B79F5) | 0;
  var t = Math.imul(_seed ^ (_seed >>> 15), 1 | _seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) % n;
}

var ADJ = ["pretty","large","big","small","tall","short","long","handsome","plain","quaint","clean","elegant","easy","angry","crazy","helpful","mushy","odd","unsightly","adorable"];
var COL = ["red","yellow","blue","green","pink","brown","purple","white","black","orange"];
var NOU = ["table","chair","house","bbq","desk","car","pony","cookie","sandwich","burger","pizza","mouse","keyboard"];

var nextId = 1;
var rows = [];              // [{id, label, node}]
var tbody = null;

function label() { return ADJ[rnd(20)] + " " + COL[rnd(10)] + " " + NOU[rnd(13)]; }

function buildRow(item) {
  var row = document.createElement("div");
  row.setAttribute("class", "row");
  row.setAttribute("data-id", String(item.id));

  var c1 = document.createElement("div");
  c1.setAttribute("class", "col-md-1");
  c1.textContent = String(item.id);

  var c2 = document.createElement("div");
  c2.setAttribute("class", "col-md-4");
  var a1 = document.createElement("a");
  a1.setAttribute("class", "lbl");
  a1.textContent = item.label;
  c2.appendChild(a1);

  var c3 = document.createElement("div");
  c3.setAttribute("class", "col-md-1");
  var a2 = document.createElement("a");
  a2.setAttribute("class", "remove");
  // The reference row nests a glyph span inside the remove anchor. It carries no
  // text, but it is the 8th element and dropping it would make our Nodes column
  // incomparable with every other published number on this workload.
  var s1 = document.createElement("span");
  s1.setAttribute("class", "glyphicon");
  a2.appendChild(s1);
  c3.appendChild(a2);

  var c4 = document.createElement("div");
  c4.setAttribute("class", "col-md-6");

  row.appendChild(c1); row.appendChild(c2); row.appendChild(c3); row.appendChild(c4);
  item.node = row;
  item.lbl = a1;
  return row;
}

function make(n) {
  var out = [];
  for (var i = 0; i < n; i++) out.push({ id: nextId++, label: label() });
  return out;
}

function appendAll(items) {
  for (var i = 0; i < items.length; i++) tbody.appendChild(buildRow(items[i]));
  for (var j = 0; j < items.length; j++) rows.push(items[j]);
}

var OPS = {
  create: function () { clear(); appendAll(make(1000)); },
  create10k: function () { clear(); appendAll(make(10000)); },
  append1: function () { appendAll(make(1)); },
  append1k: function () { appendAll(make(1000)); },
  insert1: function () {
    var it = make(1)[0];
    var node = buildRow(it);
    tbody.insertBefore(node, rows.length ? rows[0].node : null);
    rows.splice(0, 0, it);
  },
  // Rebuild the row array in ONE pass. Do NOT splice inside the loop: each splice is
  // O(n), so N/2 of them is O(n^2) — at 10k rows that measured 7.4 s against the
  // supersolid backend's 3.1 s, a gap that is array bookkeeping rather than DOM cost.
  // js-framework-benchmark's own vanilla entry avoids loop-splicing for the same
  // reason. The DOM work is identical either way: one insertBefore per new row.
  insertEvery2nd: function () {
    var out = [];
    for (var i = 0; i < rows.length; i++) {
      if (i % 2 === 0) {
        var it = make(1)[0];
        tbody.insertBefore(buildRow(it), rows[i].node);
        out.push(it);
      }
      out.push(rows[i]);
    }
    rows = out;
  },
  updateText1: function () {
    if (!rows.length) return;
    rows[0].label = label();
    rows[0].lbl.textContent = rows[0].label;
  },
  updateTextEvery2nd: function () {
    for (var i = 0; i < rows.length; i += 2) {
      rows[i].label = label();
      rows[i].lbl.textContent = rows[i].label;
    }
  },
  updateColor1: function () {
    if (!rows.length) return;
    rows[0].lbl.setAttribute("class", "lbl warm");
  },
  updateColorEvery2nd: function () {
    for (var i = 0; i < rows.length; i += 2) rows[i].lbl.setAttribute("class", "lbl warm");
  },
  swap1: function () {
    if (rows.length < 999) return;
    swap(1, 998);
  },
  swapEvery2nd: function () {
    // Stride 2, not 4: every *Every2nd op performs N/2 operations (the contract).
    // Adjacent disjoint pairs (0,1),(2,3),... = 500 swaps at 1k.
    for (var i = 0; i + 1 < rows.length; i += 2) swap(i, i + 1);
  },
  remove1: function () {
    if (!rows.length) return;
    // TEMP WORKAROUND. The natural spelling is `rows[0].node.remove()`, but
    // Element.prototype.remove() is not bound in superui_api — only appendChild,
    // removeChild, insertBefore and replaceChild are. Calling it throws a TypeError,
    // and the event-dispatch path currently discards listener exceptions, so this op
    // would silently do nothing at all. `removeChild` is bound and performs the same
    // single detachment (every row is a direct child of `tbody`).
    // TODO: restore the line below once Element.remove() is bound.
    //   rows[0].node.remove();
    tbody.removeChild(rows[0].node);
    rows.splice(0, 1);
  },
  // One pass, no loop-splicing — see the note on insertEvery2nd. Removes the
  // even-indexed rows, keeping 1, 3, 5, … exactly as before.
  removeEvery2nd: function () {
    var out = [];
    for (var i = 0; i < rows.length; i++) {
      if (i % 2 === 0) {
        // TEMP WORKAROUND (see remove1): Element.prototype.remove() is unbound.
        // TODO: restore `rows[i].node.remove();` once Element.remove() is bound.
        tbody.removeChild(rows[i].node);
      } else {
        out.push(rows[i]);
      }
    }
    rows = out;
  },
  clear: clear,
};

function swap(a, b) {
  var ra = rows[a], rb = rows[b];
  var afterB = rb.node.nextSibling;
  tbody.insertBefore(rb.node, ra.node);
  tbody.insertBefore(ra.node, afterB);
  rows[a] = rb; rows[b] = ra;
}

function clear() {
  // TEMP WORKAROUND (see remove1): Element.prototype.remove() is unbound.
  // TODO: restore `rows[i].node.remove();` once Element.remove() is bound.
  for (var i = 0; i < rows.length; i++) tbody.removeChild(rows[i].node);
  rows = [];
}

function boot() {
  tbody = document.getElementById("tbody");
  srand(1);
  var names = Object.keys(OPS);
  for (var i = 0; i < names.length; i++) {
    (function (name) {
      var btn = document.getElementById("op-" + name);
      if (btn) btn.addEventListener("click", function () { OPS[name](); });
    })(names[i]);
  }
}

boot();
