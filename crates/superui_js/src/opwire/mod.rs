//! Value types and flat-buffer codec for the JS shadow DOM's per-frame batch
//! of recorded mutations.

pub mod applier;
pub mod bootstrap;
mod codec;
pub mod idmap;

pub use applier::OpApplier;
pub use bootstrap::bootstrap_js;
pub use codec::{CodecError, JsNodeId, Op, OpBatch, StrId};
pub use idmap::IdMap;
