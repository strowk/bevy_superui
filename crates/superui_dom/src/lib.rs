//! Headless, arena-backed DOM tree for bevy_superui.
//!
//! Knows nothing about Bevy or JavaScript. The structural source of truth that
//! the reconciler diffs against and that the JS layer mutates.

mod node;
mod tree;
mod attr;

pub use node::{ElementData, Listener, ListenerId, NodeData, NodeId, NodeKind};
pub use tree::{Dom, DomError};
