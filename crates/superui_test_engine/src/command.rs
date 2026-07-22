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
        // Fields filled in in Task 6; capture the whole payload for now.
        #[serde(flatten)]
        raw: serde_json::Value,
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
