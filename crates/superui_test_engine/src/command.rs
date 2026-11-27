use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    Noop,
    Click {
        locator: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    Fill {
        locator: crate::locator::LocatorSpec,
        text: String,
    },
    Press {
        locator: crate::locator::LocatorSpec,
        key: String,
    },
    Hover {
        locator: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    MouseDown {
        locator: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    MouseUp {
        locator: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    MoveTo {
        locator: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// Composite drag gesture: `mousedown` on `from`, then the `moveTo`
    /// hover+move bookkeeping and `mouseup` on `to`.
    DragTo {
        from: crate::locator::LocatorSpec,
        to: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    Expect {
        /// "visible" | "text" | "count" | "class" | "attribute" | "screenshot"
        matcher: String,
        #[serde(default)]
        locator: Option<crate::locator::LocatorSpec>,
        #[serde(default)]
        expected: serde_json::Value,
        #[serde(default)]
        opts: serde_json::Value,
    },
    /// Deliver a game→UI bridge event to the UI's `bevy.on(name, …)` subscribers
    /// (the counterpart of the game calling `commands.trigger`), carrying `value`
    /// as the JSON payload. Lets a spec supply data a UI pulls over the bridge,
    /// which the headless host has no game side to send.
    Emit {
        name: String,
        #[serde(default)]
        value: serde_json::Value,
    },
    // `rename_all` on the enum only renames variant tags (the `"type"` value);
    // it does not cascade to a variant's own fields (the other variants never
    // needed it — `modifiers`/`locator`/`text`/`key` are camelCase-neutral).
    // `pointer_id`/`pointer_type` are the first fields that need it, so each
    // of these three variants repeats `rename_all` for its own fields.
    #[serde(rename_all = "camelCase")]
    PointerDown {
        locator: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default = "default_pointer_id")]
        pointer_id: u32,
        #[serde(default = "default_pointer_type")]
        pointer_type: String,
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
    },
    #[serde(rename_all = "camelCase")]
    PointerMove {
        locator: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default = "default_pointer_id")]
        pointer_id: u32,
        #[serde(default = "default_pointer_type")]
        pointer_type: String,
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
    },
    #[serde(rename_all = "camelCase")]
    PointerUp {
        locator: crate::locator::LocatorSpec,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default = "default_pointer_id")]
        pointer_id: u32,
        #[serde(default = "default_pointer_type")]
        pointer_type: String,
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
    },
}

/// Default `pointer_id` for a hand-written `PointerDown`/`PointerMove`/`PointerUp`
/// command JSON that omits it. The prelude always sends one explicitly (see
/// `pointerCmd` in `prelude.js`); this exists to keep `serde` happy and to
/// document the W3C default of pointer 1.
fn default_pointer_id() -> u32 {
    1
}

/// Default `pointer_type` for a hand-written pointer command JSON that omits
/// it, mirroring `default_pointer_id`.
fn default_pointer_type() -> String {
    "mouse".into()
}

#[derive(Clone, Debug)]
pub struct Queued {
    pub id: u64,
    pub command: Command,
    /// The original JSON so later tasks that add richer command variants can
    /// re-parse without changing this task.
    pub raw: String,
}
