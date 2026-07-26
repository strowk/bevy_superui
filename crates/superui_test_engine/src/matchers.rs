//! Expect-matcher evaluation against the live DOM.
//!
//! Each matcher inspects the current DOM state and returns `Ok(())` when the
//! assertion holds right now, or `Err(diagnostic)` otherwise. The driver polls
//! `evaluate` every ticked frame until it passes or a timeout budget elapses,
//! giving Playwright-style auto-waiting.
//!
//! ## Intentional Phase-1 simplifications
//! - **`toBeVisible`**: "visible" means *attached to the DOM and not carrying an
//!   inline `display:none`*. A layout-size check via `ComputedNode` can be added
//!   once a render host is wired in.
//! - **`toHaveClass`**: the expected value is the *source* of the JS regex (e.g.
//!   `/item/` -> `"item"`). We do a substring match of that source against the
//!   element's class list rather than running a real regex engine. Anchors like
//!   `\b` are trimmed. Full JS-regex fidelity is out of scope for this task.

use bevy::prelude::*;
use superui_bridge::UiRuntime;

use crate::locator::{resolve_locator, LocatorSpec};

pub fn evaluate(
    world: &World,
    matcher: &str,
    locator: &Option<LocatorSpec>,
    expected: &serde_json::Value,
) -> Result<(), String> {
    let rt = world.non_send::<UiRuntime>();
    let dom = rt.dom.borrow();
    let nodes = match locator {
        Some(spec) => resolve_locator(&dom, spec),
        None => vec![],
    };
    match matcher {
        "count" => {
            let want = expected.as_u64().unwrap_or(0) as usize;
            if nodes.len() == want {
                Ok(())
            } else {
                Err(format!("expected count {want}, got {}", nodes.len()))
            }
        }
        "visible" => {
            if nodes.is_empty() {
                return Err("not visible: no match".into());
            }
            // Visibility: attached + not inline display:none. (Layout-size check
            // via ComputedNode can be added once render host is used.)
            let node = nodes[0];
            let hidden = dom
                .get_attribute(node, "style")
                .map(|s| s.replace(' ', "").contains("display:none"))
                .unwrap_or(false);
            if hidden {
                Err("element has display:none".into())
            } else {
                Ok(())
            }
        }
        "text" => {
            let want = expected.as_str().unwrap_or("");
            let node = nodes.first().ok_or_else(|| "no match for text".to_string())?;
            let got = dom.text_content(*node);
            if got == want {
                Ok(())
            } else {
                Err(format!("expected text {want:?}, got {got:?}"))
            }
        }
        "class" => {
            let pat = expected.as_str().unwrap_or("");
            let node = nodes.first().ok_or_else(|| "no match for class".to_string())?;
            let classes = dom.classes(*node).join(" ");
            // Phase-1: substring match of the regex source (no full regex engine).
            if classes.contains(pat.trim_matches(|c| c == '\\' || c == 'b')) {
                Ok(())
            } else {
                Err(format!("expected class matching {pat:?} in {classes:?}"))
            }
        }
        "attribute" => {
            let name = expected.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let node = nodes
                .first()
                .ok_or_else(|| "no match for attribute".to_string())?;
            let got = dom.get_attribute(*node, name).map(|s| s.to_string());
            match expected.get("value").and_then(|v| v.as_str()) {
                Some(want) if got.as_deref() == Some(want) => Ok(()),
                Some(want) => Err(format!("attribute {name}: expected {want:?}, got {got:?}")),
                None if got.is_some() => Ok(()),
                None => Err(format!("attribute {name} not present")),
            }
        }
        other => Err(format!("unknown matcher: {other}")),
    }
}
