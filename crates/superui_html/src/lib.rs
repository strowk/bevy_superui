//! HTML-document parsing for bevy_superui.
//!
//! Parses an HTML string into a [`superui_dom::Dom`] via `html5ever`. Knows
//! nothing about Bevy or JavaScript. Headless-testable.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
