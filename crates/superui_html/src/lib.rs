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

    #[test]
    fn unclosed_tags_recover_without_panicking() {
        // Two <li> with no closing tags: the tree builder auto-closes the first.
        let dom = parse_document("<ul><li>a<li>b</ul>");
        assert_eq!(count_by_tag(&dom, "li"), 2);
    }

    #[test]
    fn mis_nested_tags_do_not_panic() {
        // Adoption-agency territory; we only require that it parses and a body
        // exists (exercises reparent_children / remove_from_parent paths).
        let dom = parse_document("<b><i>x</b>y</i>");
        assert!(first_by_tag(&dom, "body").is_some());
        assert!(first_by_tag(&dom, "b").is_some());
        assert!(first_by_tag(&dom, "i").is_some());
    }

    #[test]
    fn get_element_by_id_works_on_the_parsed_tree() {
        let dom = parse_document(r#"<div><input id="new-todo"></div>"#);
        let by_id = dom.get_element_by_id("new-todo").expect("element with id");
        assert_eq!(dom.tag(by_id), Some("input"));
        assert_eq!(dom.get_element_by_id("missing"), None);
    }

    #[test]
    fn parses_a_todomvc_shaped_fragment() {
        let html = r#"
            <section class="todoapp">
              <header class="header">
                <h1>todos</h1>
                <input class="new-todo" placeholder="What needs to be done?">
              </header>
              <ul class="todo-list">
                <li class="completed">
                  <div class="view">
                    <input class="toggle" type="checkbox" checked>
                    <label>Taste JavaScript</label>
                    <button class="destroy"></button>
                  </div>
                </li>
                <li>
                  <div class="view">
                    <input class="toggle" type="checkbox">
                    <label>Buy a unicorn</label>
                  </div>
                </li>
              </ul>
            </section>
        "#;
        let dom = parse_document(html);

        let section = first_by_tag(&dom, "section").expect("<section>");
        assert!(dom.class_contains(section, "todoapp"));

        // Two todo items.
        assert_eq!(count_by_tag(&dom, "li"), 2);

        // The first toggle input is checked; the second is not.
        let toggles: Vec<NodeId> = {
            let mut v = Vec::new();
            fn walk(dom: &Dom, node: NodeId, v: &mut Vec<NodeId>) {
                if dom.tag(node) == Some("input") && dom.class_contains(node, "toggle") {
                    v.push(node);
                }
                for &c in dom.children(node) {
                    walk(dom, c, v);
                }
            }
            walk(&dom, dom.document(), &mut v);
            v
        };
        assert_eq!(toggles.len(), 2);
        assert!(dom.has_attribute(toggles[0], "checked"));
        assert!(!dom.has_attribute(toggles[1], "checked"));

        // Labels carry the expected text.
        let labels: Vec<String> = {
            let mut v = Vec::new();
            fn walk(dom: &Dom, node: NodeId, v: &mut Vec<String>) {
                if dom.tag(node) == Some("label") {
                    v.push(dom.text_content(node));
                }
                for &c in dom.children(node) {
                    walk(dom, c, v);
                }
            }
            walk(&dom, dom.document(), &mut v);
            v
        };
        assert_eq!(labels, vec!["Taste JavaScript".to_string(), "Buy a unicorn".to_string()]);
    }

    #[test]
    fn comment_between_siblings_is_dropped_and_order_kept() {
        // A comment between two element siblings leaves no node behind, and the
        // surviving <li> children keep their document order.
        let dom = parse_document("<ul><li>a</li><!--x--><li>b</li></ul>");
        let ul = first_by_tag(&dom, "ul").expect("<ul>");
        let kids = dom.children(ul);
        assert_eq!(kids.len(), 2);
        assert_eq!(dom.tag(kids[0]), Some("li"));
        assert_eq!(dom.tag(kids[1]), Some("li"));
        assert_eq!(dom.text_content(kids[0]), "a");
        assert_eq!(dom.text_content(kids[1]), "b");
    }

    #[test]
    fn foster_parented_text_coalesces_across_dropped_comment() {
        // Text directly inside <table> is foster-parented before the table; the
        // comment between the two text runs is dropped and they coalesce to "ab".
        let dom = parse_document("<table>a<!--x-->b</table>");
        let body = first_by_tag(&dom, "body").expect("<body>");
        assert_eq!(dom.text_content(body), "ab");
    }
}
