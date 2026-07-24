//! Task 8 integration spike: offscreen render + screenshot capture.
//! Ignored by default — needs a GPU/software render adapter. This dev box
//! has a GPU, so run with:
//!   cargo test -p superui_test_engine --test render_capture -- --ignored --nocapture

use superui_test_engine::host::HostProject;
use superui_test_engine::render;

fn fixture() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: "#box{width:200px;height:150px;background-color:#ff0000;}".into(),
        js_or_tsx: r#"import { render } from "supersolid";
render(() => <div id="box"></div>, document.getElementById("root"));"#
            .into(),
        tsx: true,
    }
}

#[test]
#[ignore = "requires a GPU/software render adapter"]
fn captures_nonempty_frame() {
    let mut app = render::build_render_app_and_mount(&fixture(), 320, 240);
    let img = render::capture(&mut app).expect("frame captured");
    assert_eq!(img.width, 320, "captured width matches request");
    assert_eq!(img.height, 240, "captured height matches request");
    assert!(
        !img.rgba.is_empty(),
        "captured buffer is empty (no pixel data)"
    );
    assert!(
        img.rgba.iter().any(|&b| b != 0),
        "frame is all zeros — nothing rendered into the target"
    );
    eprintln!(
        "captured {}x{}, {} bytes, first non-zero at {:?}",
        img.width,
        img.height,
        img.rgba.len(),
        img.rgba.iter().position(|&b| b != 0)
    );
}
