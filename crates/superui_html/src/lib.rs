//! HTML-document parsing for bevy_superui.
//!
//! Parses an HTML string into a [`superui_dom::Dom`] via `html5ever`. Knows
//! nothing about Bevy or JavaScript. Headless-testable.

mod sink;

pub use sink::parse_document;

#[cfg(test)]
mod parse_tests {
    use super::parse_document;
    use superui_dom::{Dom, NodeId};

    /// First element (document order) with the given tag name.
    fn first_by_tag(dom: &Dom, tag: &str) -> Option<NodeId> {
        fn walk(dom: &Dom, node: NodeId, tag: &str) -> Option<NodeId> {
            if dom.tag(node) == Some(tag) {
                return Some(node);
            }
            for &c in dom.children(node) {
                if let Some(found) = walk(dom, c, tag) {
                    return Some(found);
                }
            }
            None
        }
        walk(dom, dom.document(), tag)
    }

    #[test]
    fn parses_implied_html_head_body_structure() {
        let dom = parse_document("<div></div>");
        let html = first_by_tag(&dom, "html").expect("implied <html>");
        let head = first_by_tag(&dom, "head").expect("implied <head>");
        let body = first_by_tag(&dom, "body").expect("implied <body>");
        assert_eq!(dom.parent(html), Some(dom.document()));
        assert_eq!(dom.parent(head), Some(html));
        assert_eq!(dom.parent(body), Some(html));
        let div = first_by_tag(&dom, "div").expect("<div>");
        assert_eq!(dom.parent(div), Some(body));
    }

    #[test]
    fn parses_nested_elements() {
        let dom = parse_document("<ul><li></li></ul>");
        let ul = first_by_tag(&dom, "ul").expect("<ul>");
        let li = first_by_tag(&dom, "li").expect("<li>");
        assert_eq!(dom.parent(li), Some(ul));
    }

    #[test]
    fn text_becomes_a_text_node() {
        let dom = parse_document("<p>hello</p>");
        let p = first_by_tag(&dom, "p").expect("<p>");
        assert_eq!(dom.text_content(p), "hello");
    }

    #[test]
    fn attributes_are_parsed() {
        let dom = parse_document(r#"<input type="checkbox" id="done">"#);
        let input = first_by_tag(&dom, "input").expect("<input>");
        assert_eq!(dom.get_attribute(input, "type"), Some("checkbox"));
        assert_eq!(dom.get_attribute(input, "id"), Some("done"));
    }

    #[test]
    fn tag_names_are_lowercased() {
        let dom = parse_document("<DIV></DIV>");
        assert!(first_by_tag(&dom, "div").is_some());
        assert!(first_by_tag(&dom, "DIV").is_none());
    }
}
