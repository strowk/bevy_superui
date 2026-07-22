//! Task 8: offscreen render host + screenshot capture.
//!
//! Builds a render-capable superui App whose UI is drawn into an offscreen
//! `Image` render target, then reads that frame's RGBA pixels back via Bevy's
//! screenshot observer API (`Screenshot::image` + `ScreenshotCaptured`).
//!
//! This is an integration path: it needs a real (or software) render adapter,
//! so the accompanying test is `#[ignore]` by default.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use crate::host::{self, HostProject};

/// A captured offscreen frame: tightly-packed RGBA8 pixels, row-major.
#[derive(Debug, Clone)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Handle to the offscreen image the camera renders into.
#[derive(Resource, Clone)]
pub struct RenderTargetHandle(pub Handle<Image>);

/// Shared slot the capture observer writes the decoded frame into.
#[derive(Resource, Clone, Default)]
pub(crate) struct CaptureSink(pub Arc<Mutex<Option<CapturedImage>>>);

/// Build the canonical render-to-texture `Image` for `width`x`height`.
pub fn make_target_image(width: u32, height: u32) -> Image {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    // The offscreen target must be a render attachment and copyable so the
    // screenshot pipeline can read it back.
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::RENDER_ATTACHMENT
        | TextureUsages::COPY_SRC;
    image
}

/// Build a render-capable superui App: the full default plugin set (render
/// pipeline + UI) minus winit, with a UI camera targeting an offscreen image.
///
/// Mirrors [`host::build_headless_app`] but keeps the real GPU render pipeline
/// instead of the hand-picked headless plugin subset.
pub fn build_render_app(project: &HostProject, width: u32, height: u32) -> App {
    let mut app = App::new();
    let ui_js_path = host::register_project_assets(&mut app, project);

    // Full default plugins (render + core pipeline + UI) but no window/event
    // loop — we render offscreen into an Image target instead.
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<bevy::winit::WinitPlugin>()
            // Pipelined rendering moves the render sub-app onto a worker thread
            // and hands it off through `RenderAppChannels`. That handshake
            // assumes the app is driven by the winit event loop; when we drive
            // it with manual `app.update()` calls it panics ("resource does not
            // exist: RenderAppChannels"). Disable it so rendering runs inline.
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>(),
    );
    app.add_plugins(superui::prelude::SuperUiPlugin);

    // Offscreen render target image.
    let image = make_target_image(width, height);
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(RenderTargetHandle(handle.clone()));
    app.insert_resource(CaptureSink::default());

    // A single 2D (UI) camera drawing into the offscreen image. superui UI
    // nodes attach to the single default camera, so they render here.
    app.world_mut().spawn((
        Camera2d,
        Camera {
            target: bevy::camera::RenderTarget::from(handle),
            ..default()
        },
    ));

    app.finish();
    app.insert_resource(host::HostAssetPaths { js: ui_js_path });
    app
}

/// Convenience: build the render app, mount the UI, install the test ABI, and
/// tick enough frames for the DOM to render into the target.
pub fn build_render_app_and_mount(project: &HostProject, width: u32, height: u32) -> App {
    let mut app = build_render_app(project, width, height);
    host::mount(&mut app);
    host::install_abi(&mut app);

    // CRITICAL for non-blank screenshots: bevy_ui does NOT auto-associate a root
    // UI node's percentage/viewport sizing with a camera that renders to an
    // *Image* target (only the implicit Window camera gets that association).
    // Without this, the `SuperUiRoot` node's `width/height: 100%` resolves
    // against an *unknown* viewport and collapses to 0x0, which in turn makes
    // every `inset:0`/`position:absolute`/`100%` descendant collapse — the whole
    // UI lays out at negative/zero coordinates off-screen and the frame is blank
    // (only the flat backdrop fill is captured). Tagging the root with the
    // offscreen camera makes `100%` resolve to the target size (e.g. 1280x720).
    let cam = {
        let world = app.world_mut();
        let mut cq = world.query_filtered::<Entity, With<Camera>>();
        cq.iter(world).next()
    };
    if let Some(cam) = cam {
        let world = app.world_mut();
        let mut rq = world.query_filtered::<Entity, With<superui::prelude::SuperUiRoot>>();
        let roots: Vec<Entity> = rq.iter(world).collect();
        for root in roots {
            world.entity_mut(root).insert(bevy::ui::UiTargetCamera(cam));
        }
    }

    // Let layout + render settle so the target actually has pixels.
    host::tick(&mut app, 8);
    app
}

/// Spawn a screenshot request against the offscreen target, tick until the
/// async capture fires, and return the decoded RGBA frame.
pub fn capture(app: &mut App) -> Option<CapturedImage> {
    let handle = app.world().resource::<RenderTargetHandle>().0.clone();
    let sink = app.world().resource::<CaptureSink>().0.clone();

    // Clear any previous capture.
    *sink.lock().unwrap() = None;

    let observer_sink = sink.clone();
    app.world_mut()
        .spawn(Screenshot::image(handle))
        .observe(
            move |trigger: On<ScreenshotCaptured>, mut commands: Commands| {
                let img: &Image = &trigger.event().image;
                let captured = CapturedImage {
                    width: img.width(),
                    height: img.height(),
                    rgba: img.data.clone().unwrap_or_default(),
                };
                *observer_sink.lock().unwrap() = Some(captured);
                // The screenshot entity is one-shot; clean it up.
                commands.entity(trigger.event().entity).despawn();
            },
        );

    // Capture is async (spans render sub-app frames); poll the sink.
    for _ in 0..64 {
        app.update();
        if sink.lock().unwrap().is_some() {
            break;
        }
    }
    let captured = sink.lock().unwrap().take();
    captured
}
