//! Value types and flat-buffer codec for the JS shadow DOM's per-frame batch
//! of recorded mutations.

mod codec;
pub mod idmap;

pub use codec::{CodecError, JsNodeId, Op, OpBatch, StrId};
pub use idmap::IdMap;
