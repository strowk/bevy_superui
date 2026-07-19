//! Text-node value accessors (`data` / `nodeValue` / `textContent`) on the shared
//! text prototype. All three map to the node's text data, which `superui_dom`
//! stores directly in `NodeKind::Text` — so they reuse the element crate's
//! `text_content_get`/`text_content_set` (correct for text nodes as-is).

use boa_engine::Context;
use superui_js::with_host_state;

use crate::element::{text_content_get, text_content_set};
use crate::node::set_accessor;

/// Install text-node value accessors onto the text proto.
pub fn install_text(context: &mut Context) {
    let text = with_host_state(context, |s| s.protos.text.clone()).expect("text proto");
    set_accessor(&text, "textContent", text_content_get, text_content_set, context);
    set_accessor(&text, "nodeValue", text_content_get, text_content_set, context);
    set_accessor(&text, "data", text_content_get, text_content_set, context);
}
