// Minimal static stub (Task 1): mounts an empty root so the crate builds and
// the UI asset dir is non-empty. The bridge subscription and card rendering
// (`bevy.on("bestiary", ...)`, `<For>` over creatures) land in Task 3.

import { render } from "supersolid";

render(() => <div class="bestiary" />, document.getElementById("root"));
