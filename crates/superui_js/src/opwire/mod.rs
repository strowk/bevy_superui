//! Value types and flat-buffer codec for the JS shadow DOM's per-frame batch
//! of recorded mutations.

pub mod applier;
mod codec;
pub mod idmap;

pub use applier::OpApplier;
pub use codec::{CodecError, JsNodeId, Op, OpBatch, StrId};
pub use idmap::IdMap;
