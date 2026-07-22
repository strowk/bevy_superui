use bevy::prelude::{Res, ResMut};
use bevy::prelude::Resource as BevyResource;

use crate::sim::economy::Economy;

// ---------------------------------------------------------------------------
// Snap structs — UI-ready, Clone, no Bevy-resource trait needed individually
// ---------------------------------------------------------------------------

/// UI-ready snapshot of one resource.
#[derive(Clone, Debug)]
pub struct ResSnap {
    pub id:      usize,
    pub name:    &'static str,
    pub icon:    &'static str,
    pub current: f64,
    pub rate:    f64,
    pub cap:     f64,
}

/// UI-ready snapshot of one building.
#[derive(Clone, Debug)]
pub struct BldSnap {
    pub id:        usize,
    pub name:      String,
    pub category:  &'static str,
    pub tier:      u8,
    /// CSS class string: "locked" | "available" | "building" | "done"
    pub state:     &'static str,
    pub progress:  f32,
    pub level:     u32,
    pub affordable: bool,
}

/// UI-ready snapshot of one unit stack.
#[derive(Clone, Debug)]
pub struct UnitSnap {
    pub id:     usize,
    pub name:   String,
    pub count:  u32,
    /// CSS class string: "idle" | "moving" | "combat"
    pub status: &'static str,
}

/// UI-ready snapshot of one tech.
#[derive(Clone, Debug)]
pub struct TechSnap {
    pub id:       usize,
    pub name:     String,
    /// CSS class string: "locked" | "researching" | "done"
    pub state:    &'static str,
    pub progress: f32,
}

/// UI-ready snapshot of one event log entry.
#[derive(Clone, Debug)]
pub struct EvtSnap {
    pub text: String,
    pub age:  f32,
}

// ---------------------------------------------------------------------------
// UiSnapshot — the top-level Bevy resource holding all snap data
// ---------------------------------------------------------------------------

#[derive(BevyResource, Default, Clone, Debug)]
pub struct UiSnapshot {
    pub clock:     f64,
    pub tick:      u64,
    pub resources: Vec<ResSnap>,
    pub buildings: Vec<BldSnap>,
    pub units:     Vec<UnitSnap>,
    pub techs:     Vec<TechSnap>,
    pub events:    Vec<EvtSnap>,
}

// ---------------------------------------------------------------------------
// Pure assembly function (called by the Bevy system and by tests directly)
// ---------------------------------------------------------------------------

/// Rebuild `snap` entirely from `econ`. Pure — no Bevy world access.
pub fn assemble_from(econ: &Economy, snap: &mut UiSnapshot) {
    snap.clock = econ.clock;
    snap.tick  = econ.tick;

    // Resources
    snap.resources = econ.resources.iter().enumerate().map(|(i, r)| ResSnap {
        id:      i,
        name:    r.kind.name(),
        icon:    r.kind.icon(),
        current: r.current,
        rate:    r.rate,
        cap:     r.cap,
    }).collect();

    // Buildings
    snap.buildings = econ.buildings.iter().map(|b| BldSnap {
        id:         b.id,
        name:       b.name.clone(),
        category:   b.category.class(),
        tier:       b.tier,
        state:      b.state.class(),
        progress:   b.progress,
        level:      b.level,
        affordable: b.affordable,
    }).collect();

    // Units
    snap.units = econ.units.iter().map(|u| UnitSnap {
        id:     u.id,
        name:   u.name.clone(),
        count:  u.count,
        status: u.status.class(),
    }).collect();

    // Techs
    snap.techs = econ.techs.iter().map(|t| TechSnap {
        id:       t.id,
        name:     t.name.clone(),
        state:    t.state.class(),
        progress: t.progress,
    }).collect();

    // Events (VecDeque → Vec, front = oldest)
    snap.events = econ.events.iter().map(|e| EvtSnap {
        text: e.text.clone(),
        age:  e.age,
    }).collect();
}

// ---------------------------------------------------------------------------
// Bevy system
// ---------------------------------------------------------------------------

/// System: runs every frame in `Update`, after `advance_sim`.
pub fn assemble_snapshot(econ: Res<Economy>, mut snap: ResMut<UiSnapshot>) {
    assemble_from(&econ, &mut snap);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::model::*;
    use crate::sim::economy::*;
    #[test]
    fn snapshot_mirrors_economy() {
        let cfg = CitadelConfig::default();
        let econ = build_economy(&cfg);
        let mut snap = UiSnapshot::default();
        assemble_from(&econ, &mut snap); // pure helper the system calls
        assert_eq!(snap.buildings.len(), econ.buildings.len());
        assert_eq!(snap.resources.len(), 8);
        assert!(snap.buildings.iter().any(|b| b.state == "building"));
    }
}
