use crate::gallery::MARKER;
use serde_json::Value;

/// Replace the gallery marker with `fragment` in every chapter's `content`.
pub fn expand(book: &mut Value, fragment: &str) {
    walk(book, fragment);
}

fn walk(v: &mut Value, fragment: &str) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(content)) = map.get_mut("content") {
                if content.contains(MARKER) {
                    *content = content.replace(MARKER, fragment);
                }
            }
            for (_k, child) in map.iter_mut() {
                walk(child, fragment);
            }
        }
        Value::Array(arr) => {
            for child in arr.iter_mut() {
                walk(child, fragment);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_marker_in_nested_chapters() {
        let mut book = serde_json::json!({
            "items": [
                { "Chapter": {
                    "content": format!("intro {} tail", MARKER),
                    "sub_items": [
                        { "Chapter": { "content": format!("child {}", MARKER), "sub_items": [] } }
                    ]
                }}
            ]
        });
        expand(&mut book, "<GRID>");
        let s = serde_json::to_string(&book).unwrap();
        assert!(!s.contains(MARKER), "marker should be gone");
        assert_eq!(s.matches("<GRID>").count(), 2, "both chapters expanded");
    }
}
