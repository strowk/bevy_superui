//! Task 11: interactive egui UI mode for `superui test --ui`.
//!
//! Launches a real windowed Bevy app (DefaultPlugins + EguiPlugin) that acts as
//! a test-runner shell:
//!
//!   * LEFT `SidePanel`   — the discovered spec list, each with a "Run" button.
//!   * CENTRAL panel      — the selected run's result, a live rendered-frame
//!                          image of the final captured frame, a time-travel
//!                          slider over the trace steps, and the selected step's
//!                          `dom_after` in a scrollable text area.
//!   * RIGHT `SidePanel`  — the selected step's status / error.
//!
//! ARCHITECTURE: the egui window app is SEPARATE from the UI-under-test app. We
//! never interleave two Bevy event loops. A "Run" click synchronously builds a
//! FRESH render app for the selected spec (`render::build_render_app_and_mount`),
//! transpiles + runs the spec to completion (blocking — it's a button press),
//! stores the resulting `Vec<TestResult>` into `UiState`, captures the final
//! rendered frame, then drops the render app. The Task 10 double-mount guards
//! make repeated runs safe.

use std::path::PathBuf;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle};

use crate::config::TestConfig;
use crate::host::HostProject;
use crate::trace::{StepStatus, TestResult};

/// All state the egui shell reads / mutates. Config + project are cloned in so
/// the resource is self-contained (the render app is rebuilt per Run).
#[derive(Resource)]
struct UiState {
    specs: Vec<PathBuf>,
    /// Config values needed by the Run handler.
    project: HostProject,
    spec_dir: PathBuf,
    width: u32,
    height: u32,
    max_diff_ratio: f64,

    /// Currently selected spec index (for highlighting).
    selected: Option<usize>,
    /// Results from the most recent run.
    last_results: Vec<TestResult>,
    /// Which `TestResult` within `last_results` is being inspected.
    selected_test: usize,
    /// Which step within the selected test is being inspected (time-travel).
    selected_step: usize,
    /// Last error (transpile failure, spec read failure, ...).
    error: Option<String>,
    /// Name of the spec whose results are currently shown.
    current_spec_name: Option<String>,

    /// egui texture for the final captured frame of the last run, if any.
    frame_texture: Option<egui::TextureId>,
    /// Kept alive so the asset isn't dropped while egui references it.
    frame_handle: Option<Handle<Image>>,
    frame_size: (u32, u32),
}

// Clone is needed to move HostProject into the resource.
impl Clone for HostProject {
    fn clone(&self) -> Self {
        HostProject {
            html: self.html.clone(),
            css: self.css.clone(),
            js_or_tsx: self.js_or_tsx.clone(),
            tsx: self.tsx,
        }
    }
}

/// Entry point invoked by the CLI for `superui test --ui`.
pub fn run(cfg: &TestConfig, project: &HostProject, specs: &[PathBuf]) {
    let state = UiState {
        specs: specs.to_vec(),
        project: project.clone(),
        spec_dir: cfg.spec_dir.clone(),
        width: cfg.width,
        height: cfg.height,
        max_diff_ratio: cfg.max_diff_ratio,
        selected: None,
        last_results: Vec::new(),
        selected_test: 0,
        selected_step: 0,
        error: None,
        current_spec_name: None,
        frame_texture: None,
        frame_handle: None,
        frame_size: (0, 0),
    };

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "superui test — UI mode".to_string(),
            resolution: (1400u32, 900u32).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(EguiPlugin::default());
    app.insert_resource(state);
    app.add_systems(Startup, setup_camera);
    app.add_systems(EguiPrimaryContextPass, ui_system);
    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Build the whole egui surface each frame from `UiState`.
fn ui_system(
    mut contexts: EguiContexts,
    mut state: ResMut<UiState>,
    mut images: ResMut<Assets<Image>>,
) -> Result {
    // A run was requested this frame? (deferred so we don't borrow-conflict).
    let mut run_index: Option<usize> = None;

    // Clone the egui Context (cheap Arc clone) so the `contexts` borrow is
    // released — we need `&mut contexts` again in the deferred Run handler to
    // register the captured-frame texture.
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
            for (i, spec) in state.specs.iter().enumerate() {
                let name = spec
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| spec.to_string_lossy().to_string());
                ui.horizontal(|ui| {
                    let selected = state.selected == Some(i);
                    if ui.selectable_label(selected, &name).clicked() {
                        // Selection only; Run is the explicit action.
                        run_index = None;
                    }
                    if ui.button("Run").clicked() {
                        run_index = Some(i);
                    }
                });
            }
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
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .id_salt("test_err")
                        .show(ui, |ui| {
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
                            egui::ScrollArea::vertical()
                                .max_height(160.0)
                                .id_salt("step_err")
                                .show(ui, |ui| {
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
    // Copy the small scalar bits out first so we don't hold an immutable borrow
    // of `state` across the mutable slider binding below.
    let frame_texture = state.frame_texture;
    let frame_size = state.frame_size;
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

        // Test selector (a spec may register multiple tests).
        if n_tests > 1 {
            ui.horizontal(|ui| {
                ui.label("Test:");
                for i in 0..n_tests {
                    let name = state.last_results[i].name.clone();
                    if ui
                        .selectable_label(state.selected_test == i, name)
                        .clicked()
                    {
                        state.selected_test = i;
                        state.selected_step = 0;
                    }
                }
            });
            ui.separator();
        }

        // Live rendered-frame pane (final captured frame of the run).
        if let Some(tex) = frame_texture {
            let (w, h) = frame_size;
            // Fit into a sensible on-screen box while keeping aspect ratio.
            let max_w = ui.available_width().min(640.0).max(64.0);
            let scale = if w > 0 { max_w / w as f32 } else { 1.0 };
            let size = egui::vec2(w as f32 * scale, h as f32 * scale);
            ui.label("Final rendered frame:");
            ui.image(egui::load::SizedTexture::new(tex, size));
            ui.separator();
        }

        // Time-travel over the selected test's steps.
        let step_count = state
            .last_results
            .get(state.selected_test)
            .map(|t| t.steps.len())
            .unwrap_or(0);

        if step_count > 0 {
            // Clamp selected_step in case the test changed.
            if state.selected_step >= step_count {
                state.selected_step = step_count - 1;
            }
            ui.horizontal(|ui| {
                ui.label("Step (time-travel):");
                let mut sel = state.selected_step;
                let resp = ui.add(
                    egui::Slider::new(&mut sel, 0..=step_count.saturating_sub(1))
                        .integer(),
                );
                if resp.changed() {
                    state.selected_step = sel;
                }
                ui.label(format!("{} / {}", state.selected_step + 1, step_count));
            });

            // Action label for the current step.
            if let Some(step) = state
                .last_results
                .get(state.selected_test)
                .and_then(|t| t.steps.get(state.selected_step))
            {
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
            egui::ScrollArea::vertical()
                .id_salt("dom_view")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut dom.as_str())
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });
        } else if current_spec_name.is_some() {
            ui.label("(no trace steps recorded for this test)");
        }
    });

    // ---- Deferred Run (after all panels drawn / borrows released) --------
    if let Some(i) = run_index {
        state.selected = Some(i);
        let spec = state.specs[i].clone();
        run_selected(&mut state, &spec, &mut images, &mut contexts);
    }

    Ok(())
}

/// Blocking Run handler: transpile + run the selected spec against a fresh
/// render app, store the trace, and capture the final frame into an egui image.
fn run_selected(
    state: &mut UiState,
    spec: &std::path::Path,
    images: &mut Assets<Image>,
    contexts: &mut EguiContexts,
) {
    state.error = None;
    state.selected_test = 0;
    state.selected_step = 0;
    state.current_spec_name = spec
        .file_name()
        .map(|n| n.to_string_lossy().to_string());

    // Release the previous frame texture (if any).
    if let Some(old) = state.frame_handle.take() {
        contexts.remove_image(&old);
    }
    state.frame_texture = None;

    let file = spec
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "spec".to_string());

    let src = match std::fs::read_to_string(spec) {
        Ok(s) => s,
        Err(e) => {
            state.error = Some(format!("read {file}: {e}"));
            return;
        }
    };

    let js = match crate::transpile::transpile_spec(&src, &file) {
        Ok(j) => j,
        Err(e) => {
            state.error = Some(format!("transpile {file}: {e}"));
            return;
        }
    };

    // Build a fresh render app and run the spec to completion.
    let mut app =
        crate::render::build_render_app_and_mount(&state.project, state.width, state.height);

    let opts = crate::driver::RunOptions {
        snapshot: Some(crate::snapshot::SnapshotConfig {
            dir: state.spec_dir.clone(),
            update: false,
            max_diff_ratio: state.max_diff_ratio,
            platform: std::env::consts::OS.to_string(),
        }),
        spec_file: file.clone(),
        render: true,
    };

    state.last_results = crate::driver::run_spec_with(&mut app, &js, &opts);

    // Capture the final frame and register it with egui.
    if let Some(captured) = crate::render::capture(&mut app) {
        if !captured.rgba.is_empty() {
            let size = Extent3d {
                width: captured.width,
                height: captured.height,
                depth_or_array_layers: 1,
            };
            let image = Image::new(
                size,
                TextureDimension::D2,
                captured.rgba,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            let handle = images.add(image);
            let tex = contexts.add_image(EguiTextureHandle::Strong(handle.clone()));
            state.frame_texture = Some(tex);
            state.frame_handle = Some(handle);
            state.frame_size = (captured.width, captured.height);
        }
    }
}
