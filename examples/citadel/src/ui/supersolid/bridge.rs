//! JSON marshalling between the Rust sim and the TSX UI.
//! The DTO is built from `UiSnapshot` here so `sim/` stays serde-free.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::sim::snapshot::UiSnapshot;

// ── Rust → JS: per-frame payload DTOs ────────────────────────────────────────

#[derive(Serialize)]
pub struct ResourceDto {
    pub id:      usize,
    pub name:    &'static str,
    pub icon:    &'static str,
    pub current: f64,
    pub rate:    f64,
    pub cap:     f64,
}

#[derive(Serialize)]
pub struct BuildingDto {
    pub id:        usize,
    pub name:      String,
    pub category:  &'static str,
    pub tier:      u8,
    pub state:     &'static str,
    pub progress:  f32,
    pub level:     u32,
    pub affordable: bool,
}

#[derive(Serialize)]
pub struct UnitDto {
    pub id:     usize,
    pub name:   String,
    pub count:  u32,
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct TechDto {
    pub id:       usize,
    pub name:     String,
    pub state:    &'static str,
    pub progress: f32,
}

#[derive(Serialize)]
pub struct EventDto {
    pub text: String,
    pub age:  f32,
}

/// Whole-frame payload. Triggered every frame; forwarded to JS `bevy.on("frame")`.
#[derive(Event, Serialize)]
pub struct FrameDto {
    pub clock:     f64,
    pub tick:      u64,
    pub resources: Vec<ResourceDto>,
    pub buildings: Vec<BuildingDto>,
    pub units:     Vec<UnitDto>,
    pub techs:     Vec<TechDto>,
    pub events:    Vec<EventDto>,
}

/// Build the JSON DTO from the current snapshot. Pure — no Bevy world access.
pub fn build_frame(snap: &UiSnapshot) -> FrameDto {
    let resources = snap.resources.iter().map(|r| ResourceDto {
        id:      r.id,
        name:    r.name,
        icon:    r.icon,
        current: r.current,
        rate:    r.rate,
        cap:     r.cap,
    }).collect();

    let buildings = snap.buildings.iter().map(|b| BuildingDto {
        id:         b.id,
        name:       b.name.clone(),
        category:   b.category,
        tier:       b.tier,
        state:      b.state,
        progress:   b.progress,
        level:      b.level,
        affordable: b.affordable,
    }).collect();

    let units = snap.units.iter().map(|u| UnitDto {
        id:     u.id,
        name:   u.name.clone(),
        count:  u.count,
        status: u.status,
    }).collect();

    let techs = snap.techs.iter().map(|t| TechDto {
        id:       t.id,
        name:     t.name.clone(),
        state:    t.state,
        progress: t.progress,
    }).collect();

    let events = snap.events.iter().map(|e| EventDto {
        text: e.text.clone(),
        age:  e.age,
    }).collect();

    FrameDto {
        clock:     snap.clock,
        tick:      snap.tick,
        resources,
        buildings,
        units,
        techs,
        events,
    }
}

// ── JS → Rust: intents ───────────────────────────────────────────────────────

/// `bevy.send("CitadelIntent", { kind })` → logged and ignored (no-op for now).
#[derive(Event, Deserialize)]
pub struct CitadelIntent {
    pub kind: String,
}

fn on_citadel_intent(ev: On<CitadelIntent>) {
    warn!("citadel: unknown CitadelIntent kind '{}'", ev.event().kind);
}

/// Register the JS-visible command/event surface. Called by `SupersolidUiPlugin`
/// and by the test harness.
pub fn register_bridge(app: &mut App) {
    use superui::prelude::SuperUiApp;
    app.add_superui_event::<FrameDto>("frame")
        .add_superui_command::<CitadelIntent>("CitadelIntent")
        .add_observer(on_citadel_intent);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::model::CitadelConfig;
    use crate::sim::economy::build_economy;
    use crate::sim::snapshot::{UiSnapshot, assemble_from};
    #[test]
    fn build_frame_maps_snapshot() {
        let cfg = CitadelConfig::default();
        let econ = build_economy(&cfg);
        let mut snap = UiSnapshot::default();
        assemble_from(&econ, &mut snap);
        let f = build_frame(&snap);
        assert_eq!(f.buildings.len(), snap.buildings.len());
        assert_eq!(f.resources.len(), 8);
    }
}
