#[test]
fn order_is_topological() {
    let order = xtask::publish_order();
    let pos = |name: &str| order.iter().position(|c| *c == name)
        .unwrap_or_else(|| panic!("{name} missing from publish_order"));
    // forks before their dependents
    assert!(pos("superui_flair_core_macros") < pos("superui_flair_core"));
    assert!(pos("superui_flair_core") < pos("superui_flair_style"));
    assert!(pos("superui_flair_style") < pos("superui_flair_css_parser"));
    assert!(pos("superui_flair_css_parser") < pos("superui_css"));
    // leaf libs before aggregators
    assert!(pos("superui_css") < pos("superui_bridge"));
    assert!(pos("superui_bridge") < pos("superui"));
    assert!(pos("superui") < pos("superui_test_engine"));
    // superui_paths (leaf) must precede its dependents
    assert!(pos("superui_paths") < pos("supersolid"));
    assert!(pos("superui_paths") < pos("superui"));
    // superui_css_utilities depends on superui_css and is an optional dep of superui
    assert!(pos("superui_css") < pos("superui_css_utilities"));
    assert!(pos("superui_css_utilities") < pos("superui"));
    // all 17 publishable crates present, each once
    assert_eq!(order.len(), 17);
    let mut sorted = order.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 17, "publish_order has duplicates");
}
