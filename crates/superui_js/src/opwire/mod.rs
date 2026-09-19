//! Value types and flat-buffer codec for the JS shadow DOM's per-frame batch
//! of recorded mutations.

mod codec;

pub use codec::{CodecError, JsNodeId, Op, OpBatch, StrId};
