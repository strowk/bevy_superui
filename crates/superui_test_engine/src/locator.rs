use superui_dom::{Dom, NodeId};

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct LocatorStep {
    pub sel: String,
    #[serde(rename = "hasText", default)]
    pub has_text: Option<String>,
}

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct LocatorSpec {
    pub steps: Vec<LocatorStep>,
    #[serde(default)]
    pub nth: Option<usize>,
}

pub fn resolve_locator(dom: &Dom, spec: &LocatorSpec) -> Vec<NodeId> {
    // Start from a single virtual scope: the document.
    let mut scopes = vec![dom.document()];
    for step in &spec.steps {
        let mut next = Vec::new();
        for &scope in &scopes {
            for cand in dom.query_selector_all(scope, &step.sel) {
                if let Some(t) = &step.has_text {
                    if !dom.text_content(cand).contains(t.as_str()) {
                        continue;
                    }
                }
                if !next.contains(&cand) {
                    next.push(cand);
                }
            }
        }
        scopes = next;
    }
    match spec.nth {
        Some(i) => scopes.into_iter().nth(i).into_iter().collect(),
        None => scopes,
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_locator, LocatorSpec, LocatorStep};
    use superui_dom::Dom;

    fn dom() -> (Dom, ) {
        let mut d = Dom::new();
        let doc = d.document();
        let body = d.create_element("body");
        d.append_child(doc, body).unwrap();
        for (cls, txt) in [("tab", "MAIN"), ("tab", "SETTINGS")] {
            let el = d.create_element("div");
            d.set_attribute(el, "class", cls).unwrap();
            let t = d.create_text(txt);
            d.append_child(el, t).unwrap();
            d.append_child(body, el).unwrap();
        }
        (d,)
    }

    #[test]
    fn resolves_selector_with_has_text() {
        let (d,) = dom();
        let spec = LocatorSpec {
            steps: vec![LocatorStep { sel: ".tab".into(), has_text: Some("SETTINGS".into()) }],
            nth: None,
        };
        let got = resolve_locator(&d, &spec);
        assert_eq!(got.len(), 1);
        assert_eq!(d.text_content(got[0]), "SETTINGS");
    }

    #[test]
    fn resolves_nth() {
        let (d,) = dom();
        let spec = LocatorSpec {
            steps: vec![LocatorStep { sel: ".tab".into(), has_text: None }],
            nth: Some(0),
        };
        assert_eq!(resolve_locator(&d, &spec).len(), 1);
    }
}
