//! Trace types for recording per-step test execution results.

use superui_dom::{Dom, NodeId, NodeKind};

#[derive(Clone, Debug, serde::Serialize)]
pub enum StepStatus {
    Ok,
    Failed(String),
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Step {
    pub index: usize,
    pub action: String,
    pub status: StepStatus,
    pub dom_after: String,
    pub screenshot: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
    pub steps: Vec<Step>,
}

/// Serialize the `<body>` subtree (or document root if no body) as an
/// indented HTML-ish string for diagnostic / trace purposes.
pub fn serialize_body(dom: &Dom) -> String {
    let doc = dom.document();
    let body = dom.query_selector(doc, "body").unwrap_or(doc);
    let mut out = String::new();
    write_node(dom, body, &mut out, 0);
    out
}

fn write_node(dom: &Dom, node: NodeId, out: &mut String, depth: usize) {
    let pad = "  ".repeat(depth);
    match dom.get(node).map(|n| &n.kind) {
        Some(NodeKind::Document) => {
            // Document root: just recurse into children without wrapping tag.
            for &c in dom.children(node) {
                write_node(dom, c, out, depth);
            }
        }
        Some(NodeKind::Element(_)) => {
            let tag = dom.tag(node).unwrap_or("unknown");
            let mut attrs = String::new();
            for (k, v) in dom.attributes(node) {
                attrs.push_str(&format!(" {k}=\"{v}\""));
            }
            out.push_str(&format!("{pad}<{tag}{attrs}>\n"));
            for &c in dom.children(node) {
                write_node(dom, c, out, depth + 1);
            }
            out.push_str(&format!("{pad}</{tag}>\n"));
        }
        Some(NodeKind::Text(t)) => {
            let trimmed = t.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("{pad}{trimmed}\n"));
            }
        }
        None => {}
    }
}
