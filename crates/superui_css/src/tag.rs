//! HTML tag-name interning: turn a runtime tag `&str` into the `&'static str`
//! flair's `TypeName` component requires, so element selectors work for any tag.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use superui_flair_style::components::TypeName;

fn interner() -> &'static Mutex<HashSet<&'static str>> {
    static INTERNER: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    INTERNER.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Intern an HTML tag name to a process-lifetime `&'static str`, ASCII-lowercased
/// (HTML tag names are case-insensitive). Each distinct tag is leaked exactly
/// once; the tag vocabulary is finite, so this is a bounded one-time cost.
/// Repeated calls for the same tag (in any casing) return the same pointer.
pub fn intern_tag(tag: &str) -> &'static str {
    let lower = tag.to_ascii_lowercase();
    let mut set = interner().lock().expect("tag interner poisoned");
    if let Some(existing) = set.get(lower.as_str()) {
        return existing;
    }
    let leaked: &'static str = Box::leak(lower.into_boxed_str());
    set.insert(leaked);
    leaked
}

/// The flair `TypeName` component for an HTML tag (lowercased, interned). Insert
/// this on a UI entity to give it its element-selector identity (`div`, `li`, …).
pub fn html_type_name(tag: &str) -> TypeName {
    TypeName(intern_tag(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interns_stably_and_case_insensitively() {
        let a = intern_tag("div");
        let b = intern_tag("DIV");
        let c = intern_tag("div");
        // Same tag (any casing) → identical pointer, not just equal strings.
        assert!(std::ptr::eq(a, c));
        assert!(std::ptr::eq(a, b));
        assert_eq!(a, "div");
    }

    #[test]
    fn distinct_tags_get_distinct_static_strs() {
        let li = intern_tag("li");
        let ul = intern_tag("ul");
        assert_eq!(li, "li");
        assert_eq!(ul, "ul");
        assert!(!std::ptr::eq(li, ul));
    }

    #[test]
    fn html_type_name_carries_the_interned_tag() {
        let tn = html_type_name("Input"); // mixed case in → lowercased
        assert_eq!(tn.0, "input");
        // Confirm the interner and TypeName wiring only (no ECS world needed).
        let _ = superui_flair_style::components::StyleData::default();
        assert!(html_type_name("input").0 == "input");
    }
}
