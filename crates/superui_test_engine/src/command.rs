use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    Noop,
    // extended in later tasks: Click { locator }, Fill { .. }, Expect { .. }, Screenshot { .. }
}

#[derive(Clone, Debug)]
pub struct Queued {
    pub id: u64,
    pub command: Command,
    /// The original JSON so later tasks that add richer command variants can
    /// re-parse without changing this task.
    pub raw: String,
}
