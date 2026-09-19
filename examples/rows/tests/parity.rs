//! Spec §3.2: the two backends must differ only in what drives the DOM.

use rows::bench::{build_bench_app, dom_node_count, Backend};

const VANILLA_CSS: &str = include_str!("../assets/ui/rows_vanilla/rows.css");
const SOLID_CSS: &str = include_str!("../assets/ui/rows_solid/rows.css");

#[test]
fn stylesheets_are_byte_identical() {
    assert_eq!(
        VANILLA_CSS, SOLID_CSS,
        "the two backends must share one stylesheet verbatim, or the published \
         Flair cascade column differs for reasons unrelated to the framework"
    );
}

#[test]
fn both_backends_mount_and_render_chrome() {
    for backend in [Backend::Vanilla, Backend::Supersolid] {
        let mut app = build_bench_app(backend);
        for _ in 0..30 {
            app.update();
        }
        let n = dom_node_count(&app);
        assert!(n > 10, "{} mounted only {n} DOM elements", backend.label());
    }
}
