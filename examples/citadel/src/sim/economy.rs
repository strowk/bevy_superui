use std::collections::VecDeque;
use bevy::prelude::{App, Plugin, Res, ResMut, Startup, Update};
use bevy::prelude::Resource as BevyResource;
use bevy::ecs::schedule::IntoScheduleConfigs;

use crate::sim::snapshot::{UiSnapshot, assemble_snapshot};

use crate::sim::model::{
    Building, BuildState, Category, CitadelConfig, DT, ResourceKind, Rng,
    Tech, TechState, Unit, UnitStatus, Resource,
    building_name, tech_name, unit_name,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single event/notification line shown in the event log.
#[derive(Clone, Debug)]
pub struct EventLine {
    pub text: String,
    pub age:  f32,
}

/// The live economy state — the single source of truth for the simulation.
#[derive(BevyResource, Clone, Debug)]
pub struct Economy {
    pub resources: Vec<Resource>,
    pub buildings: Vec<Building>,
    pub units:     Vec<Unit>,
    pub techs:     Vec<Tech>,
    /// Indices into `buildings` that are currently under construction.
    pub queue:     Vec<usize>,
    pub clock:     f64,
    pub tick:      u64,
    pub events:    VecDeque<EventLine>,
    pub rng:       Rng,
}

// ---------------------------------------------------------------------------
// build_economy — deterministic initial state
// ---------------------------------------------------------------------------

pub fn build_economy(cfg: &CitadelConfig) -> Economy {
    let mut rng = Rng::new(cfg.seed);

    // --- Resources -----------------------------------------------------------
    let resources: Vec<Resource> = ResourceKind::all()
        .iter()
        .map(|&kind| {
            let cap     = 1000.0 + rng.next_f32() as f64 * 4000.0;
            let rate    = 2.0   + rng.next_f32() as f64 * 18.0;
            let current = cap   * (0.3 + rng.next_f32() as f64 * 0.5);
            Resource { kind, current, rate, cap }
        })
        .collect();

    // --- Buildings -----------------------------------------------------------
    // Distribution: ~30% Done, ~15% Building, ~40% Available, ~15% Locked
    let categories = [Category::Economy, Category::Military, Category::Science, Category::Civic];
    let mut buildings: Vec<Building> = (0..cfg.building_count)
        .map(|i| {
            let category = categories[i % categories.len()];
            let tier     = ((i / categories.len()) % 5 + 1) as u8;
            let r        = rng.next_f32();
            let state    = if r < 0.30 {
                BuildState::Done
            } else if r < 0.45 {
                BuildState::Building
            } else if r < 0.85 {
                BuildState::Available
            } else {
                BuildState::Locked
            };
            let progress = if state == BuildState::Building {
                rng.next_f32() * 0.9
            } else {
                0.0
            };
            let level    = if state == BuildState::Done { rng.range(1, 6) as u32 } else { 0 };
            // Each building has 1..3 resource costs
            let n_costs  = rng.range(1, 4);
            let cost = (0..n_costs)
                .map(|_| {
                    let kind = ResourceKind::all()[rng.range(0, 8)];
                    let amt  = 50.0 + rng.next_f32() as f64 * 950.0;
                    (kind, amt)
                })
                .collect();
            Building {
                id:         i,
                name:       building_name(i),
                category,
                tier,
                state,
                progress,
                level,
                cost,
                affordable: false,  // recomputed below
            }
        })
        .collect();

    // Populate the construction queue (capped at 8).
    let mut queue: Vec<usize> = Vec::new();
    for (i, b) in buildings.iter().enumerate() {
        if b.state == BuildState::Building && queue.len() < 8 {
            queue.push(i);
        }
    }
    // If we got more Building buildings than 8, revert the overflow back to
    // Available so invariants are clean.
    for (i, b) in buildings.iter_mut().enumerate() {
        if b.state == BuildState::Building && !queue.contains(&i) {
            b.state    = BuildState::Available;
            b.progress = 0.0;
        }
    }

    // --- Affordable recompute ------------------------------------------------
    recompute_affordable(&mut buildings, &resources);

    // --- Units ---------------------------------------------------------------
    let units: Vec<Unit> = (0..cfg.unit_count)
        .map(|i| {
            let statuses = [UnitStatus::Idle, UnitStatus::Moving, UnitStatus::Combat];
            let status   = statuses[rng.range(0, statuses.len())];
            Unit {
                id:     i,
                name:   unit_name(i),
                count:  rng.range(10, 500) as u32,
                status,
            }
        })
        .collect();

    // --- Techs ---------------------------------------------------------------
    let techs: Vec<Tech> = (0..cfg.tech_count)
        .map(|i| {
            let r = rng.next_f32();
            let state = if r < 0.35 {
                TechState::Done
            } else if r < 0.55 {
                TechState::Researching
            } else {
                TechState::Locked
            };
            let progress = if state == TechState::Researching { rng.next_f32() * 0.85 } else { 0.0 };
            Tech { id: i, name: tech_name(i), state, progress }
        })
        .collect();

    Economy {
        resources,
        buildings,
        units,
        techs,
        queue,
        clock: 0.0,
        tick:  0,
        events: VecDeque::new(),
        rng,
    }
}

// ---------------------------------------------------------------------------
// tick_economy — one deterministic step
// ---------------------------------------------------------------------------

/// Rate at which a queued building advances per tick (reaches 1.0 in ~10 s).
const BUILD_RATE: f32 = DT as f32 / 10.0;

/// Number of ticks between starting a new building.
const BUILD_START_EVERY: u64 = 90;   // ~1.5 s

/// Number of ticks between advancing research.
const RESEARCH_EVERY: u64 = 60;     // ~1 s

/// Maximum events kept.
const MAX_EVENTS: usize = 12;

pub fn tick_economy(econ: &mut Economy, cfg: &CitadelConfig) {
    econ.clock += DT;
    econ.tick  += 1;

    // --- Age events (expire old ones) ----------------------------------------
    for ev in &mut econ.events {
        ev.age += DT as f32;
    }
    while econ.events.len() > MAX_EVENTS
        || econ.events.front().map(|e| e.age > 30.0).unwrap_or(false)
    {
        econ.events.pop_front();
    }

    // --- Resources: accumulate -----------------------------------------------
    for r in &mut econ.resources {
        r.current = (r.current + r.rate * DT).min(r.cap);
    }

    // --- Process construction queue ------------------------------------------
    let mut finished: Vec<usize> = Vec::new();
    for &bi in &econ.queue {
        econ.buildings[bi].progress += BUILD_RATE;
        if econ.buildings[bi].progress >= 1.0 {
            econ.buildings[bi].state    = BuildState::Done;
            econ.buildings[bi].progress = 1.0;
            econ.buildings[bi].level   += 1;
            finished.push(bi);
        }
    }

    for bi in finished {
        econ.queue.retain(|&x| x != bi);
        let name = econ.buildings[bi].name.clone();
        push_event(
            &mut econ.events,
            format!("✓ {} construction complete (L{})", name, econ.buildings[bi].level),
        );

        // Unlock ~1 locked building when one finishes
        let locked_ids: Vec<usize> = econ.buildings.iter()
            .enumerate()
            .filter(|(_, b)| b.state == BuildState::Locked)
            .map(|(i, _)| i)
            .collect();
        if !locked_ids.is_empty() {
            let pick = econ.rng.range(0, locked_ids.len());
            econ.buildings[locked_ids[pick]].state = BuildState::Available;
        }

        // Bump a random unit's count
        if !econ.units.is_empty() {
            let pick = econ.rng.range(0, econ.units.len());
            econ.units[pick].count += econ.rng.range(1, 20) as u32;
        }
    }

    // --- Start a new building periodically -----------------------------------
    if econ.tick % BUILD_START_EVERY == 0 && econ.queue.len() < 8 {
        // Find affordable Available buildings
        let candidates: Vec<usize> = econ.buildings.iter()
            .enumerate()
            .filter(|(_, b)| b.state == BuildState::Available && b.affordable)
            .map(|(i, _)| i)
            .collect();
        if !candidates.is_empty() {
            let pick  = econ.rng.range(0, candidates.len());
            let bi    = candidates[pick];
            econ.buildings[bi].state    = BuildState::Building;
            econ.buildings[bi].progress = 0.0;
            econ.queue.push(bi);
            let name = econ.buildings[bi].name.clone();
            push_event(&mut econ.events, format!("▶ {} construction started", name));
        } else {
            // No affordable one — try any Available
            let all_avail: Vec<usize> = econ.buildings.iter()
                .enumerate()
                .filter(|(_, b)| b.state == BuildState::Available)
                .map(|(i, _)| i)
                .collect();
            if !all_avail.is_empty() {
                let pick  = econ.rng.range(0, all_avail.len());
                let bi    = all_avail[pick];
                econ.buildings[bi].state    = BuildState::Building;
                econ.buildings[bi].progress = 0.0;
                econ.queue.push(bi);
                let name = econ.buildings[bi].name.clone();
                push_event(&mut econ.events, format!("▶ {} construction started", name));
            }
        }
    }

    // --- Recompute affordable (with hysteresis: only every 30 ticks) ---------
    if econ.tick % 30 == 0 {
        recompute_affordable(&mut econ.buildings, &econ.resources);
    }

    // --- Advance research periodically ---------------------------------------
    if econ.tick % RESEARCH_EVERY == 0 {
        let researching_ids: Vec<usize> = econ.techs.iter()
            .enumerate()
            .filter(|(_, t)| t.state == TechState::Researching)
            .map(|(i, _)| i)
            .collect();
        if !researching_ids.is_empty() {
            let pick = econ.rng.range(0, researching_ids.len());
            let ti   = researching_ids[pick];
            econ.techs[ti].progress += 0.05 + econ.rng.next_f32() * 0.1;
            if econ.techs[ti].progress >= 1.0 {
                econ.techs[ti].state    = TechState::Done;
                econ.techs[ti].progress = 1.0;
                let name = econ.techs[ti].name.clone();
                push_event(&mut econ.events, format!("🔬 {} research complete", name));

                // Start researching a locked tech
                let locked_techs: Vec<usize> = econ.techs.iter()
                    .enumerate()
                    .filter(|(_, t)| t.state == TechState::Locked)
                    .map(|(i, _)| i)
                    .collect();
                if !locked_techs.is_empty() {
                    let lp = econ.rng.range(0, locked_techs.len());
                    econ.techs[locked_techs[lp]].state = TechState::Researching;
                }
            }
        } else {
            // Kick off first available locked tech
            let locked_techs: Vec<usize> = econ.techs.iter()
                .enumerate()
                .filter(|(_, t)| t.state == TechState::Locked)
                .map(|(i, _)| i)
                .collect();
            if !locked_techs.is_empty() {
                let lp = econ.rng.range(0, locked_techs.len());
                econ.techs[locked_techs[lp]].state = TechState::Researching;
            }
        }
    }

    // --- Occasionally shuffle unit status ------------------------------------
    if econ.tick % 120 == 0 && !econ.units.is_empty() {
        let pick = econ.rng.range(0, econ.units.len());
        let statuses = [UnitStatus::Idle, UnitStatus::Moving, UnitStatus::Combat];
        let new_status = statuses[econ.rng.range(0, statuses.len())];
        econ.units[pick].status = new_status;
    }

    let _ = cfg; // cfg fields (building_count etc.) used at build time; reserved for future limits
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn push_event(events: &mut VecDeque<EventLine>, text: String) {
    while events.len() >= MAX_EVENTS {
        events.pop_front();
    }
    events.push_back(EventLine { text, age: 0.0 });
}

fn recompute_affordable(buildings: &mut Vec<Building>, resources: &[Resource]) {
    for b in buildings.iter_mut() {
        let affordable = b.cost.iter().all(|(kind, amt)| {
            resources.iter()
                .find(|r| r.kind == *kind)
                .map(|r| r.current >= *amt)
                .unwrap_or(false)
        });
        b.affordable = affordable;
    }
}

// ---------------------------------------------------------------------------
// Bevy plugin
// ---------------------------------------------------------------------------

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        // Insert the config if not already present (allows callers to supply
        // their own config before adding this plugin).
        if !app.world().contains_resource::<CitadelConfig>() {
            app.insert_resource(CitadelConfig::default());
        }

        // Build economy at startup; init snapshot resource.
        app.init_resource::<UiSnapshot>();
        app.add_systems(Startup, startup_economy);
        app.add_systems(Update, advance_sim);
        app.add_systems(Update, assemble_snapshot.after(advance_sim));
    }
}

fn startup_economy(cfg: Res<CitadelConfig>, mut commands: bevy::prelude::Commands) {
    let econ = build_economy(&cfg);
    commands.insert_resource(econ);
}

fn advance_sim(cfg: Res<CitadelConfig>, mut econ: ResMut<Economy>) {
    tick_economy(&mut econ, &cfg);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::model::*;
    #[test]
    fn economy_is_steady_and_deterministic() {
        let cfg = CitadelConfig::default();
        let mut a = build_economy(&cfg);
        let mut b = build_economy(&cfg);
        assert_eq!(a.buildings.len(), cfg.building_count);
        assert!(a.buildings.iter().any(|x| x.state == BuildState::Building));
        assert!(a.buildings.iter().any(|x| x.state == BuildState::Done));
        for _ in 0..600 { tick_economy(&mut a, &cfg); tick_economy(&mut b, &cfg); }
        // Determinism: identical after 600 ticks.
        assert_eq!(a.clock.to_bits(), b.clock.to_bits());
        assert_eq!(a.buildings.iter().filter(|x| x.state==BuildState::Done).count(),
                   b.buildings.iter().filter(|x| x.state==BuildState::Done).count());
        // Steady-state: still fully populated, still has activity.
        assert_eq!(a.buildings.len(), cfg.building_count);
        assert!(a.clock > 0.0);
    }
}
