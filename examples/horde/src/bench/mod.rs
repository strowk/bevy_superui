//! Headless macro-benchmark harness for the horde game.
//! See docs/superpowers/specs/2026-07-21-horde-benchmark-harness-design.md.

/// Which UI backend the bench app assembles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// sim + snapshot + synthetic projection only — the shared floor.
    Null,
    /// native `bevy_ui` UI.
    Native,
    /// supersolid TSX UI.
    Supersolid,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Null => "null",
            Backend::Native => "native",
            Backend::Supersolid => "supersolid",
        }
    }
}
