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

    /// Count elements (document order) with the given tag name.
    fn count_by_tag(dom: &Dom, tag: &str) -> usize {
        fn walk(dom: &Dom, node: NodeId, tag: &str, acc: &mut usize) {
            if dom.tag(node) == Some(tag) {
                *acc += 1;
            }
            for &c in dom.children(node) {
                walk(dom, c, tag, acc);
            }
        }
        let mut n = 0;
        walk(dom, dom.document(), tag, &mut n);
        n
    }

    #[test]
    fn comments_are_dropped() {
        let dom = parse_document("<div><!-- a comment --></div>");
        let div = first_by_tag(&dom, "div").expect("<div>");
        assert_eq!(dom.children(div).len(), 0);
        assert_eq!(dom.text_content(div), "");
    }

    #[test]
    fn doctype_is_dropped() {
        let dom = parse_document("<!DOCTYPE html><html><head></head><body></body></html>");
        // The document's only child is <html>; no doctype node was added.
        assert_eq!(dom.children(dom.document()).len(), 1);
        let html = first_by_tag(&dom, "html").expect("<html>");
        assert_eq!(dom.children(dom.document()), &[html]);
    }

    #[test]
    fn plain_text_is_a_single_text_node() {
        let dom = parse_document("<p>Hello, world</p>");
        let p = first_by_tag(&dom, "p").expect("<p>");
        assert_eq!(dom.children(p).len(), 1);
        assert_eq!(dom.text_content(p), "Hello, world");
    }

    #[test]
    fn text_split_by_a_dropped_comment_is_coalesced() {
        // html5ever emits text "a", a comment, then text "b". The comment is
        // dropped, and "b" must merge into the "a" text node left behind.
        let dom = parse_document("<p>a<!--x-->b</p>");
        let p = first_by_tag(&dom, "p").expect("<p>");
        assert_eq!(dom.children(p).len(), 1);
        assert_eq!(dom.text_content(p), "ab");
    }

    #[test]
    fn void_element_has_no_children_and_next_is_a_sibling() {
        let dom = parse_document("<input><span></span>");
        let input = first_by_tag(&dom, "input").expect("<input>");
        let span = first_by_tag(&dom, "span").expect("<span>");
        assert_eq!(dom.children(input).len(), 0);
        // input and span are siblings (span is NOT a child of the void input).
        assert_eq!(dom.parent(input), dom.parent(span));
    }

    #[test]
    fn unknown_tag_becomes_a_plain_element() {
        let dom = parse_document("<my-widget></my-widget>");
        assert_eq!(count_by_tag(&dom, "my-widget"), 1);
    }

    #[test]
    fn boolean_attribute_is_present_with_empty_value() {
        let dom = parse_document(r#"<input type="checkbox" checked>"#);
        let input = first_by_tag(&dom, "input").expect("<input>");
        assert!(dom.has_attribute(input, "checked"));
        assert_eq!(dom.get_attribute(input, "checked"), Some(""));
    }
}
