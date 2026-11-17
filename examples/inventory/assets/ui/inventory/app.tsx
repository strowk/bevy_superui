// Chest <-> player-inventory drag-and-drop demo — the worked example for
// superui's browser-faithful mouse events + document-level drag capture.
//
// NOTE — everything lives in this one file on purpose: superui's transpiler
// strips cross-module imports, so components are plain functions in one module.
//
// Drag protocol (mirrors native HTML5 drag-and-drop minus the DataTransfer
// ceremony):
//   - `startDrag` (a tile's `onMouseDown`) records `{id, from}` and installs
//     `document`-level `mousemove`/`mouseup` listeners — so the drag tracks the
//     cursor and resolves its drop no matter what element is under it when the
//     button comes up, exactly like a real browser's capture-on-mousedown.
//   - `onMove` repositions a floating tile from `event.clientX/clientY`.
//   - `onUp` resolves the drop *slot* (not a screen position) by walking
//     `event.target.parentNode` upward for the nearest `data-slot` ancestor —
//     `closest()` is not implemented yet, so this walk is written by hand. Each
//     panel's own container carries `data-slot="chest"` / `data-slot="inv"`, so
//     dropping anywhere inside a panel (on its background or on a tile) finds
//     the panel's slot.

import { createSignal, For, Show, render } from "supersolid";

interface Item {
  id: string;
  emoji: string;
  name: string;
}

type SlotName = "chest" | "inv";

// Walk `node` and its ancestors (via `parentNode`) for the nearest element
// carrying `data-slot`, returning its value ("chest" | "inv") or null if none
// is found before the chain runs out. `document`/`window` sit above the DOM
// tree in the `parentNode` chain but don't implement `getAttribute`, so guard
// every step instead of assuming every ancestor is an Element.
function findSlot(node: any): SlotName | null {
  let n = node;
  while (n) {
    if (typeof n.getAttribute === "function") {
      const v = n.getAttribute("data-slot");
      if (v === "chest" || v === "inv") return v;
    }
    n = n.parentNode;
  }
  return null;
}

// Leaf component for one tile — no control-flow of its own, so it is safe to
// pass reactive props straight through (unlike a container with its own
// `<For>`, this one only needs the single item it was handed).
function Tile(props: { item: Item; onMouseDown: (e: any) => void }) {
  return (
    <div
      class="tile"
      id={`tile-${props.item.id}`}
      data-item={props.item.id}
      onMouseDown={props.onMouseDown}
      title={props.item.name}
    >
      {props.item.emoji}
    </div>
  );
}

function App() {
  const [chest, setChest] = createSignal<Item[]>([
    { id: "sword", emoji: "⚔️", name: "Sword" },
    { id: "shield", emoji: "🛡️", name: "Shield" },
    { id: "potion", emoji: "🧪", name: "Potion" },
  ]);
  const [inv, setInv] = createSignal<Item[]>([
    { id: "coin", emoji: "🪙", name: "Coin" },
  ]);

  // The in-flight drag, or null when nothing is being dragged.
  const [dragging, setDragging] = createSignal<{ id: string; from: SlotName } | null>(null);
  const [floatPos, setFloatPos] = createSignal({ x: 0, y: 0 });

  const storeFor = (name: SlotName) => (name === "chest" ? chest : inv);
  const setterFor = (name: SlotName) => (name === "chest" ? setChest : setInv);

  // Stable function identities: declared once per app lifetime (the component
  // body runs once) so `removeEventListener` below matches the exact listener
  // `addEventListener` installed.
  const onMove = (e: any) => {
    setFloatPos({ x: e.clientX, y: e.clientY });
  };

  const onUp = (e: any) => {
    document.removeEventListener("mousemove", onMove);
    document.removeEventListener("mouseup", onUp);
    const d = dragging();
    setDragging(null);
    if (!d) return;
    const to = findSlot(e.target);
    if (!to || to === d.from) return;
    const fromItems = storeFor(d.from)();
    const item = fromItems.find((it) => it.id === d.id);
    if (!item) return;
    setterFor(d.from)(fromItems.filter((it) => it.id !== d.id));
    setterFor(to)([...storeFor(to)(), item]);
  };

  const startDrag = (e: any, id: string, from: SlotName) => {
    setDragging({ id, from });
    setFloatPos({ x: e.clientX, y: e.clientY });
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  };

  // The dragged item's own data, looked up from whichever store it still
  // lives in (the move only happens on drop) — drives the floating preview.
  const draggingItem = () => {
    const d = dragging();
    if (!d) return null;
    return storeFor(d.from)().find((it) => it.id === d.id) || null;
  };

  return (
    <div id="app">
      <div class="panel" id="chest" data-slot="chest">
        <div class="panel-title">Chest</div>
        <div class="tiles">
          {<For each={chest()}>
            {(item) => <Tile item={item} onMouseDown={(e) => startDrag(e, item.id, "chest")} />}
          </For>}
        </div>
      </div>
      <div class="panel" id="inv" data-slot="inv">
        <div class="panel-title">Inventory</div>
        <div class="tiles">
          {<For each={inv()}>
            {(item) => <Tile item={item} onMouseDown={(e) => startDrag(e, item.id, "inv")} />}
          </For>}
        </div>
      </div>
      {<Show when={draggingItem()}>
        <div
          id="floating-tile"
          class="tile floating"
          style={`left: ${floatPos().x - 20}px; top: ${floatPos().y - 20}px`}
        >
          {draggingItem() ? draggingItem().emoji : ""}
        </div>
      </Show>}
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
