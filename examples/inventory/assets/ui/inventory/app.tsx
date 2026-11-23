// Chest <-> player-inventory drag-and-drop demo — the worked example for
// superui's W3C pointer events + `setPointerCapture`, the one path that
// drives both mouse and touch drags identically.
//
// NOTE — everything lives in this one file on purpose: superui's transpiler
// strips cross-module imports, so components are plain functions in one module.
//
// Drag protocol (mirrors native HTML5 drag-and-drop minus the DataTransfer
// ceremony):
//   - `startDrag` (a tile's `onPointerDown`) records `{id, from}` and calls
//     `e.target.setPointerCapture(e.pointerId)`, then installs `document`-level
//     `pointermove`/`pointerup` listeners. Capture pins `e.target` to the tile
//     for the rest of the gesture (mouse or touch — the same pointer-event
//     family carries both), no matter what's visually underneath the pointer;
//     the document-level listeners track the drag past the tile's own bounds.
//   - `onMove` repositions a floating tile from `event.clientX/clientY`.
//   - `onUp` resolves the drop *slot* from a screen position, not `event.target`
//     (capture made that the dragged tile itself): `document.elementFromPoint`
//     finds whatever's actually under the pointer, then `findSlot` walks its
//     `parentNode` chain upward for the nearest `data-slot` ancestor —
//     `closest()` is not implemented yet, so this walk is written by hand. Each
//     panel's own container carries `data-slot="chest"` / `data-slot="inv"`, so
//     dropping anywhere inside a panel (on its background or on a tile) finds
//     the panel's slot.

import { createSignal, For, Show, render } from "supersolid";

interface Item {
  id: string;
  // Short plain-text tag, not an emoji: Bevy's bundled default font has no
  // emoji glyphs, so an emoji here would reconcile to a blank/tofu tile.
  icon: string;
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
function Tile(props: { item: Item; onPointerDown: (e: any) => void }) {
  return (
    <div
      class="tile"
      id={`tile-${props.item.id}`}
      data-item={props.item.id}
      onPointerDown={props.onPointerDown}
      title={props.item.name}
    >
      {props.item.icon}
    </div>
  );
}

function App() {
  const [chest, setChest] = createSignal<Item[]>([
    { id: "sword", icon: "SWD", name: "Sword" },
    { id: "shield", icon: "SHD", name: "Shield" },
    { id: "potion", icon: "POT", name: "Potion" },
  ]);
  const [inv, setInv] = createSignal<Item[]>([
    { id: "coin", icon: "COI", name: "Coin" },
  ]);

  // The in-flight drag, or null when nothing is being dragged.
  const [dragging, setDragging] = createSignal<{ id: string; from: SlotName } | null>(null);
  const [floatPos, setFloatPos] = createSignal({ x: 0, y: 0 });

  const storeFor = (name: SlotName) => (name === "chest" ? chest : inv);
  const setterFor = (name: SlotName) => (name === "chest" ? setChest : setInv);

  // Shift-click the opposite of a drag: move the item straight to the other
  // panel without a drag, reading the modifier off the pointerdown event.
  const quickMove = (id: string, from: SlotName) => {
    const to: SlotName = from === "chest" ? "inv" : "chest";
    const fromItems = storeFor(from)();
    const item = fromItems.find((it) => it.id === id);
    if (!item) return;
    setterFor(from)(fromItems.filter((it) => it.id !== id));
    setterFor(to)([...storeFor(to)(), item]);
  };

  // Stable function identities: declared once per app lifetime (the component
  // body runs once) so `removeEventListener` below matches the exact listener
  // `addEventListener` installed.
  const onMove = (e: any) => {
    setFloatPos({ x: e.clientX, y: e.clientY });
  };

  const onUp = (e: any) => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
    const d = dragging();
    setDragging(null);
    if (!d) return;
    // `e.target` is pinned to the dragged tile by `setPointerCapture` — the
    // drop slot has to come from where the pointer actually is, not who's
    // receiving the event.
    const to = findSlot(document.elementFromPoint(e.clientX, e.clientY));
    if (!to || to === d.from) return;
    const fromItems = storeFor(d.from)();
    const item = fromItems.find((it) => it.id === d.id);
    if (!item) return;
    setterFor(d.from)(fromItems.filter((it) => it.id !== d.id));
    setterFor(to)([...storeFor(to)(), item]);
  };

  const startDrag = (e: any, id: string, from: SlotName) => {
    if (e.shiftKey) {
      quickMove(id, from);
      return;
    }
    e.target.setPointerCapture(e.pointerId);
    setDragging({ id, from });
    setFloatPos({ x: e.clientX, y: e.clientY });
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
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
            {(item) => <Tile item={item} onPointerDown={(e) => startDrag(e, item.id, "chest")} />}
          </For>}
        </div>
      </div>
      <div class="panel" id="inv" data-slot="inv">
        <div class="panel-title">Inventory</div>
        <div class="tiles">
          {<For each={inv()}>
            {(item) => <Tile item={item} onPointerDown={(e) => startDrag(e, item.id, "inv")} />}
          </For>}
        </div>
      </div>
      {<Show when={draggingItem()}>
        <div
          id="floating-tile"
          class="tile floating"
          // Positive offset, not centered-on-cursor: with the cursor at
          // (x, y) and this box's top-left at (x+12, y+12), the box sits
          // down-right of the pointer and (x, y) itself lands OUTSIDE it.
          // That's cosmetic now, not load-bearing: `onUp` resolves the drop
          // from `document.elementFromPoint(e.clientX, e.clientY)`, a real
          // screen-position hit-test — NOT `event.target` (capture pins that
          // to the dragged tile itself for the whole gesture, floating
          // preview or not). So even a cursor-centered preview sitting
          // topmost under the pointer couldn't steal the drop the way it
          // would have under the old `event.target`-based resolution; the
          // offset just keeps the preview from visually covering what the
          // player is aiming at.
          style={`left: ${floatPos().x + 12}px; top: ${floatPos().y + 12}px`}
        >
          {draggingItem() ? draggingItem().icon : ""}
        </div>
      </Show>}
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
