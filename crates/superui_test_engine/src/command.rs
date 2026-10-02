use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    Noop,
    Click {
        locator: crate::locator::LocatorSpec,
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
}

#[derive(Clone, Debug)]
pub struct Queued {
    pub id: u64,
    pub command: Command,
    /// The original JSON so later tasks that add richer command variants can
    /// re-parse without changing this task.
    pub raw: String,
}
