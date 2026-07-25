//! Interactive egui UI mode for `superui test --ui`.
//!
//! A SINGLE windowed Bevy app hosts BOTH the egui runner shell AND the
//! under-test UI (mounted into this same world, rendered to an offscreen
//! image). There is exactly one `RenderPlugin` in the process, so the
//! `init_empty_bind_group_layout` global is initialized once — the previous
//! design built a second render app per Run and panicked.
//!
//!   * LEFT `SidePanel`   — discovered spec list, each with a "Run" button.
//!   * CENTRAL panel      — run progress, the live offscreen frame, a
//!                          time-travel slider over trace steps, and the
//!                          selected step's `dom_after`.
//!   * RIGHT `SidePanel`  — selected step status / error.
//!
//! A Run request is recorded in `UiState.pending_run`; the exclusive
//! `run_stepper` system (in `Last`, after superui reconcile) tears down the
//! prior UI, mounts a fresh DOM, and steps the spec one frame at a time via
//! `ui_driver`. The window stays responsive throughout.

use std::path::PathBuf;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle, EguiUserTextures};

use crate::config::TestConfig;
use crate::driver::RunOptions;
use crate::host::{self, HostProject};
use crate::render::{make_target_image, CaptureSink, RenderTargetHandle};
use crate::trace::{StepStatus, TestResult};
use crate::ui_driver::{self, ActiveRun};

#[derive(Resource)]
struct UiState {
    specs: Vec<PathBuf>,
    spec_dir: PathBuf,
    // Retained for future use (e.g. dynamic resize); currently sized at setup only.
    #[allow(dead_code)]
    width: u32,
    #[allow(dead_code)]
    height: u32,
    max_diff_ratio: f64,

    selected: Option<usize>,
    /// A Run was requested this frame (spec index); consumed by run_stepper.
    pending_run: Option<usize>,

    last_results: Vec<TestResult>,
    selected_test: usize,
    selected_step: usize,
    error: Option<String>,
    current_spec_name: Option<String>,
    status_line: String,

    /// Per-test rendered frame, registered as an egui texture (index parallels
    /// `last_results`). `None` = no frame captured for that test. The central
    /// panel shows the entry for `selected_test`.
    frame_textures: Vec<Option<(egui::TextureId, (u32, u32))>>,
    /// Kept alive so egui's referenced image assets aren't dropped.
    frame_handles: Vec<Handle<Image>>,
}

/// Entity of the offscreen camera the under-test UI renders into.
#[derive(Resource)]
struct UnderTestCamera(Entity);

pub fn run(cfg: &TestConfig, project: &HostProject, specs: &[PathBuf]) {
    let mut app = App::new();
    // MUST be before DefaultPlugins (AssetPlugin inside it requires custom
    // asset sources to be registered first).
    host::register_project_assets(&mut app, project);
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "superui test — UI mode".to_string(),
                    resolution: (1400u32, 900u32).into(),
                    ..default()
                }),
                ..default()
            })
            // The offscreen under-test camera is captured via `Screenshot::image`.
            // Pipelined rendering runs the render sub-app a frame behind on a
            // worker thread; the render-to-image + screenshot readback path used
            // by our headless render host is only exercised without it (see
            // `render::build_render_app`). Disable it here too for parity.
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>(),
    );
    app.add_plugins(EguiPlugin::default());
    app.add_plugins(superui::prelude::SuperUiPlugin);
    // Opt-in (`--features mcp_debug`): expose the runner over BRP so bevy_brp_mcp
    // can inspect the world (cameras / UI entities / layout) and screenshot it.
    #[cfg(feature = "mcp_debug")]
    app.add_plugins(bevy_brp_extras::BrpExtrasPlugin);

    // Offscreen render target for the under-test UI + capture sink.
    let image = make_target_image(cfg.width, cfg.height);
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(RenderTargetHandle(handle.clone()));
    app.insert_resource(CaptureSink::default());
    // ActiveRun holds Boa !Send values — must be a non-send resource.
    app.init_non_send_resource::<ActiveRun>();

    app.insert_resource(UiState {
        specs: specs.to_vec(),
        spec_dir: cfg.spec_dir.clone(),
        width: cfg.width,
        height: cfg.height,
        max_diff_ratio: cfg.max_diff_ratio,
        selected: None,
        pending_run: None,
        last_results: Vec::new(),
        selected_test: 0,
        selected_step: 0,
        error: None,
        current_spec_name: None,
        status_line: String::new(),
        frame_textures: Vec::new(),
        frame_handles: Vec::new(),
    });

    app.add_systems(Startup, setup_cameras);
    app.add_systems(EguiPrimaryContextPass, ui_system);
    app.add_systems(Last, run_stepper);
    // Env-gated headless self-run: `SUPERUI_UI_AUTORUN=1` auto-triggers a Run of
    // the first spec shortly after launch (useful for smoke tests / driving the
    // window from a screenshot tool without a manual click).
    app.init_resource::<AutoRun>();
    app.add_systems(Update, auto_run_first_spec);
    app.run();
}

#[derive(Resource, Default)]
struct AutoRun {
    frames: u32,
    triggered: bool,
}

/// When `SUPERUI_UI_AUTORUN=1`, click "Run" on the first spec after ~90 frames
/// (enough for the window + assets to settle). No-op otherwise.
fn auto_run_first_spec(mut auto: ResMut<AutoRun>, mut state: ResMut<UiState>) {
    if auto.triggered || std::env::var("SUPERUI_UI_AUTORUN").is_err() {
        return;
    }
    auto.frames += 1;
    if auto.frames >= 90 && !state.specs.is_empty() {
        state.pending_run = Some(0);
        auto.triggered = true;
    }
}

/// Spawn the egui window camera and the offscreen under-test camera.
fn setup_cameras(mut commands: Commands, target: Res<RenderTargetHandle>) {
    // egui shell renders to the window.
    commands.spawn(Camera2d);
    // Under-test UI renders to the offscreen image.
    let cam = commands
        .spawn((
            Camera2d,
            Camera { order: -1, ..default() },
            bevy::camera::RenderTarget::from(target.0.clone()),
        ))
        .id();
    commands.insert_resource(UnderTestCamera(cam));
}

/// Exclusive system: pick up a pending Run, then advance any active run one
/// frame. Runs in `Last`, after superui reconcile.
fn run_stepper(world: &mut World) {
    // Start a new run if one was requested this frame.
    let pending = world.resource_mut::<UiState>().pending_run.take();
    if let Some(i) = pending {
        let (spec, spec_dir, max_diff) = {
            let s = world.resource::<UiState>();
            (
                s.specs[i].clone(),
                s.spec_dir.clone(),
                s.max_diff_ratio,
            )
        };
        let file = spec
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "spec".to_string());

        // Reset display state for the new run.
        {
            let mut s = world.resource_mut::<UiState>();
            s.error = None;
            s.selected_test = 0;
            s.selected_step = 0;
            s.last_results.clear();
            s.current_spec_name = Some(file.clone());
            s.status_line = "starting\u{2026}".to_string();
        }

        match start_run_from_spec(world, &spec, &file, &spec_dir, max_diff) {
            Ok(run) => {
                world.non_send_resource_mut::<ActiveRun>().0 = Some(run);
            }
            Err(e) => {
                world.resource_mut::<UiState>().error = Some(e);
            }
        }
    }

    // Advance the active run.
    let mut active = world.non_send_resource_mut::<ActiveRun>().0.take();
    if let Some(mut run) = active.take() {
        ui_driver::step(world, &mut run);

        // Publish progress + results to UiState.
        let done = run.is_done();
        {
            let mut s = world.resource_mut::<UiState>();
            s.status_line = run.progress_label();
            if done {
                s.last_results = run.results.clone();
                // Point the time-travel slider at the LAST step of the first
                // test (the finished state), not step 0.
                s.selected_test = 0;
                s.selected_step = s
                    .last_results
                    .first()
                    .map(|t| t.steps.len().saturating_sub(1))
                    .unwrap_or(0);
            }
        }

        if done {
            // Register each test's captured frame as an egui texture.
            register_test_frames(world, &run.test_frames);
        } else {
            world.non_send_resource_mut::<ActiveRun>().0 = Some(run);
        }
    }
}

/// Read + transpile a spec file and begin a run against a fresh under-test DOM.
fn start_run_from_spec(
    world: &mut World,
    spec: &std::path::Path,
    file: &str,
    spec_dir: &std::path::Path,
    max_diff_ratio: f64,
) -> Result<crate::ui_driver::RunState, String> {
    let src = std::fs::read_to_string(spec).map_err(|e| format!("read {file}: {e}"))?;
    let js = crate::transpile::transpile_spec(&src, file).map_err(|e| format!("transpile {file}: {e}"))?;

    let opts = RunOptions {
        snapshot: Some(crate::snapshot::SnapshotConfig {
            dir: spec_dir.to_path_buf(),
            update: false,
            max_diff_ratio,
            platform: std::env::consts::OS.to_string(),
        }),
        spec_file: file.to_string(),
        render: true,
    };

    let cam = world.resource::<UnderTestCamera>().0;
    Ok(ui_driver::start_run(world, Some(cam), js, file.to_string(), opts))
}

/// Register each test's captured RGBA frame as an egui texture, releasing any
/// previously registered ones. `frames` is indexed per test (parallel to
/// `last_results`); a `None` entry becomes a `None` texture slot.
///
/// Uses `EguiUserTextures` (a normal `Resource`) directly rather than driving
/// `SystemState<EguiContexts>` from an exclusive system, which avoids lifetime
/// issues with the world borrow.  `EguiContexts::add_image` is just a thin
/// proxy to `EguiUserTextures::add_image`, so the result is identical.
fn register_test_frames(world: &mut World, frames: &[Option<(u32, u32, Vec<u8>)>]) {
    // Release the previous egui texture registrations.
    let old_handles = std::mem::take(&mut world.resource_mut::<UiState>().frame_handles);
    for old in &old_handles {
        world.resource_mut::<EguiUserTextures>().remove_image(old);
    }

    let mut textures: Vec<Option<(egui::TextureId, (u32, u32))>> = Vec::with_capacity(frames.len());
    let mut handles: Vec<Handle<Image>> = Vec::new();
    for frame in frames {
        match frame {
            Some((w, h, rgba)) if !rgba.is_empty() => {
                let size = Extent3d { width: *w, height: *h, depth_or_array_layers: 1 };
                let image = Image::new(
                    size,
                    TextureDimension::D2,
                    rgba.clone(),
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                let handle = world.resource_mut::<Assets<Image>>().add(image);
                let tex = world
                    .resource_mut::<EguiUserTextures>()
                    .add_image(EguiTextureHandle::Strong(handle.clone()));
                handles.push(handle);
                textures.push(Some((tex, (*w, *h))));
            }
            _ => textures.push(None),
        }
    }

    let mut s = world.resource_mut::<UiState>();
    s.frame_textures = textures;
    s.frame_handles = handles;
}

fn ui_system(mut contexts: EguiContexts, mut state: ResMut<UiState>) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let ctx = &ctx;

    // ---- LEFT: spec list -------------------------------------------------
    egui::SidePanel::left("spec_list")
        .resizable(true)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("Specs");
            ui.separator();
            if state.specs.is_empty() {
                ui.label("(no specs discovered)");
            }
            let specs: Vec<(usize, String)> = state
                .specs
                .iter()
                .enumerate()
                .map(|(i, spec)| {
                    let name = spec
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| spec.to_string_lossy().to_string());
                    (i, name)
                })
                .collect();
            for (i, name) in specs {
                ui.horizontal(|ui| {
                    let selected = state.selected == Some(i);
                    if ui.selectable_label(selected, &name).clicked() {
                        state.selected = Some(i);
                    }
                    if ui.button("Run").clicked() {
                        state.selected = Some(i);
                        state.pending_run = Some(i);
                    }
                });
            }
            ui.separator();
            ui.label(state.status_line.clone());
        });

    // ---- RIGHT: status / error of selected step --------------------------
    egui::SidePanel::right("status_panel")
        .resizable(true)
        .default_width(320.0)
        .show(ctx, |ui| {
            ui.heading("Status");
            ui.separator();
            if let Some(err) = &state.error {
                ui.colored_label(egui::Color32::from_rgb(230, 80, 80), "Error:");
                ui.label(err);
                ui.separator();
            }
            if let Some(test) = state.last_results.get(state.selected_test) {
                let (color, label) = if test.passed {
                    (egui::Color32::from_rgb(80, 200, 120), "PASSED")
                } else {
                    (egui::Color32::from_rgb(230, 80, 80), "FAILED")
                };
                ui.horizontal(|ui| {
                    ui.label("Test:");
                    ui.strong(&test.name);
                });
                ui.colored_label(color, label);
                if let Some(e) = &test.error {
                    ui.separator();
                    ui.label("Test error:");
                    egui::ScrollArea::vertical().max_height(120.0).id_salt("test_err").show(ui, |ui| {
                        ui.monospace(e);
                    });
                }
                ui.separator();
                if let Some(step) = test.steps.get(state.selected_step) {
                    ui.label(format!("Step {}:", step.index));
                    ui.monospace(&step.action);
                    match &step.status {
                        StepStatus::Ok => {
                            ui.colored_label(egui::Color32::from_rgb(80, 200, 120), "step ok");
                        }
                        StepStatus::Failed(msg) => {
                            ui.colored_label(egui::Color32::from_rgb(230, 80, 80), "step failed");
                            egui::ScrollArea::vertical().max_height(160.0).id_salt("step_err").show(ui, |ui| {
                                ui.monospace(msg);
                            });
                        }
                    }
                }
            } else {
                ui.label("Run a spec to see results.");
            }
        });

    // ---- CENTRAL: frame image + time-travel + DOM ------------------------
    // The rendered frame for the SELECTED test (captured before the closure
    // borrows `state` mutably; a tab switch this frame applies next frame).
    let sel_frame = state
        .frame_textures
        .get(state.selected_test)
        .copied()
        .flatten();
    let current_spec_name = state.current_spec_name.clone();
    let n_tests = state.last_results.len();

    egui::CentralPanel::default().show(ctx, |ui| {
        if let Some(name) = &current_spec_name {
            ui.heading(format!("Run: {name}"));
        } else {
            ui.heading("superui test runner");
            ui.label("Select a spec on the left and press Run.");
        }
        ui.separator();

        if n_tests > 1 {
            ui.horizontal(|ui| {
                ui.label("Test:");
                for i in 0..n_tests {
                    let name = state.last_results[i].name.clone();
                    if ui.selectable_label(state.selected_test == i, name).clicked() {
                        state.selected_test = i;
                        state.selected_step = 0;
                    }
                }
            });
            ui.separator();
        }

        if let Some((tex, (w, h))) = sel_frame {
            let max_w = ui.available_width().min(640.0).max(64.0);
            let scale = if w > 0 { max_w / w as f32 } else { 1.0 };
            let size = egui::vec2(w as f32 * scale, h as f32 * scale);
            ui.label("Rendered frame (selected test):");
            ui.image(egui::load::SizedTexture::new(tex, size));
            ui.separator();
        } else if !state.last_results.is_empty() {
            ui.label("(no rendered frame for this test)");
            ui.separator();
        }

        let step_count = state.last_results.get(state.selected_test).map(|t| t.steps.len()).unwrap_or(0);
        if step_count > 0 {
            if state.selected_step >= step_count {
                state.selected_step = step_count - 1;
            }
            ui.horizontal(|ui| {
                ui.label("Step (time-travel):");
                let mut sel = state.selected_step;
                let resp = ui.add(egui::Slider::new(&mut sel, 0..=step_count.saturating_sub(1)).integer());
                if resp.changed() {
                    state.selected_step = sel;
                }
                ui.label(format!("{} / {}", state.selected_step + 1, step_count));
            });
            if let Some(step) = state.last_results.get(state.selected_test).and_then(|t| t.steps.get(state.selected_step)) {
                ui.horizontal(|ui| {
                    ui.label("Action:");
                    ui.monospace(&step.action);
                });
            }
            ui.separator();
            ui.label("DOM after this step:");
            let dom = state
                .last_results
                .get(state.selected_test)
                .and_then(|t| t.steps.get(state.selected_step))
                .map(|s| s.dom_after.clone())
                .unwrap_or_default();
            egui::ScrollArea::vertical().id_salt("dom_view").auto_shrink([false, false]).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut dom.as_str()).code_editor().desired_width(f32::INFINITY));
            });
        } else if current_spec_name.is_some() {
            ui.label("(running\u{2026} or no trace steps yet)");
        }
    });

    Ok(())
}
