use bevy::prelude::Resource as BevyResource;

/// Fixed simulation timestep.
pub const DT: f64 = 1.0 / 60.0;

// ---------------------------------------------------------------------------
// ResourceKind
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResourceKind {
    Minerals,
    Energy,
    Alloys,
    Food,
    Research,
    Influence,
    Unity,
    Population,
}

impl ResourceKind {
    pub fn all() -> [ResourceKind; 8] {
        [
            ResourceKind::Minerals,
            ResourceKind::Energy,
            ResourceKind::Alloys,
            ResourceKind::Food,
            ResourceKind::Research,
            ResourceKind::Influence,
            ResourceKind::Unity,
            ResourceKind::Population,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            ResourceKind::Minerals  => "Minerals",
            ResourceKind::Energy    => "Energy",
            ResourceKind::Alloys    => "Alloys",
            ResourceKind::Food      => "Food",
            ResourceKind::Research  => "Research",
            ResourceKind::Influence => "Influence",
            ResourceKind::Unity     => "Unity",
            ResourceKind::Population => "Population",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ResourceKind::Minerals   => "⛏",
            ResourceKind::Energy     => "⚡",
            ResourceKind::Alloys     => "⚙",
            ResourceKind::Food       => "🌾",
            ResourceKind::Research   => "🔬",
            ResourceKind::Influence  => "★",
            ResourceKind::Unity      => "◈",
            ResourceKind::Population => "♟",
        }
    }
}

// ---------------------------------------------------------------------------
// Category
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Economy,
    Military,
    Science,
    Civic,
}

impl Category {
    pub fn class(self) -> &'static str {
        match self {
            Category::Economy  => "economy",
            Category::Military => "military",
            Category::Science  => "science",
            Category::Civic    => "civic",
        }
    }
}

// ---------------------------------------------------------------------------
// BuildState
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildState {
    Locked,
    Available,
    Building,
    Done,
}

impl BuildState {
    pub fn class(self) -> &'static str {
        match self {
            BuildState::Locked    => "locked",
            BuildState::Available => "available",
            BuildState::Building  => "building",
            BuildState::Done      => "done",
        }
    }
}

// ---------------------------------------------------------------------------
// TechState
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TechState {
    Locked,
    Researching,
    Done,
}

impl TechState {
    pub fn class(self) -> &'static str {
        match self {
            TechState::Locked      => "locked",
            TechState::Researching => "researching",
            TechState::Done        => "done",
        }
    }
}

// ---------------------------------------------------------------------------
// UnitStatus
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnitStatus {
    Idle,
    Moving,
    Combat,
}

impl UnitStatus {
    pub fn class(self) -> &'static str {
        match self {
            UnitStatus::Idle   => "idle",
            UnitStatus::Moving => "moving",
            UnitStatus::Combat => "combat",
        }
    }
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Resource {
    pub kind:    ResourceKind,
    pub current: f64,
    pub rate:    f64,
    pub cap:     f64,
}

#[derive(Clone, Debug)]
pub struct Building {
    pub id:         usize,
    pub name:       String,
    pub category:   Category,
    pub tier:       u8,
    pub state:      BuildState,
    pub progress:   f32,
    pub level:      u32,
    pub cost:       Vec<(ResourceKind, f64)>,
    pub affordable: bool,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub id:     usize,
    pub name:   String,
    pub count:  u32,
    pub status: UnitStatus,
}

#[derive(Clone, Debug)]
pub struct Tech {
    pub id:       usize,
    pub name:     String,
    pub state:    TechState,
    pub progress: f32,
}

// ---------------------------------------------------------------------------
// CitadelConfig
// ---------------------------------------------------------------------------

#[derive(BevyResource, Clone, Debug)]
pub struct CitadelConfig {
    pub building_count: usize,
    pub unit_count:     usize,
    pub tech_count:     usize,
    pub seed:           u64,
}

impl Default for CitadelConfig {
    fn default() -> Self {
        CitadelConfig {
            building_count: 120,
            unit_count:      40,
            tech_count:      60,
            seed:             1,
        }
    }
}

// ---------------------------------------------------------------------------
// Rng — deterministic xorshift64
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Ensure state is never zero (xorshift64 is stuck at 0).
        let state = if seed == 0 { 0xDEAD_BEEF_CAFE_1234 } else { seed };
        Rng(state)
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }

    pub fn next_f32(&mut self) -> f32 {
        // Map [0, 2^32) into [0.0, 1.0)
        self.next_u32() as f32 / u32::MAX as f32
    }

    pub fn range(&mut self, lo: usize, hi: usize) -> usize {
        // hi is exclusive
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u64;
        lo + (self.next_u32() as u64 % span) as usize
    }
}

// ---------------------------------------------------------------------------
// Deterministic name generators (no randomness — pure index)
// ---------------------------------------------------------------------------

const BUILDING_PREFIXES: &[&str] = &[
    "Grand", "Central", "Iron", "Crystal", "Solar", "Orbital",
    "Deep", "Ancient", "Stellar", "Quantum", "Arcane", "Frontier",
];

const BUILDING_SUFFIXES: &[&str] = &[
    "Forge",    "Nexus",    "Spire",   "Vault",  "Hub",
    "Array",    "Citadel",  "Bastion", "Court",  "Annex",
    "Platform", "Foundry",  "Archive", "Beacon",
];

const UNIT_TYPES: &[&str] = &[
    "Legion",   "Brigade",  "Squadron", "Fleet",   "Division",
    "Corps",    "Vanguard", "Guard",    "Rangers",
];

const UNIT_DESIGNATORS: &[&str] = &[
    "Alpha", "Beta", "Gamma", "Delta", "Epsilon",
    "Zeta",  "Eta",  "Theta", "Iota",  "Kappa",
    "Lambda","Mu",
];

const TECH_AREAS: &[&str] = &[
    "Propulsion",   "Metallurgy",  "Computing",   "Genetics",
    "Optics",       "Plasma",      "Nano",        "Quantum",
    "Biosystems",   "Terraforming","Shielding",   "Logistics",
];

const TECH_TIERS: &[&str] = &["I", "II", "III", "IV", "V"];

/// Deterministic building name from index (combines prefix, suffix, and tier marker).
pub fn building_name(i: usize) -> String {
    let prefix = BUILDING_PREFIXES[i % BUILDING_PREFIXES.len()];
    let suffix  = BUILDING_SUFFIXES[(i / BUILDING_PREFIXES.len()) % BUILDING_SUFFIXES.len()];
    // Include i itself so that every index produces a distinct name even when
    // prefix+suffix would collide in theory.  The tier suffix guarantees
    // consecutive indices (that land on the same prefix/suffix slot) still differ.
    let tier = i / (BUILDING_PREFIXES.len() * BUILDING_SUFFIXES.len()) + 1;
    if tier == 1 {
        format!("{} {}", prefix, suffix)
    } else {
        format!("{} {} Mk{}", prefix, suffix, tier)
    }
}

/// Deterministic unit name from index.
pub fn unit_name(i: usize) -> String {
    let designator = UNIT_DESIGNATORS[i % UNIT_DESIGNATORS.len()];
    let kind       = UNIT_TYPES[(i / UNIT_DESIGNATORS.len()) % UNIT_TYPES.len()];
    format!("{} {}", designator, kind)
}

/// Deterministic tech name from index.
pub fn tech_name(i: usize) -> String {
    let area = TECH_AREAS[i % TECH_AREAS.len()];
    let tier = TECH_TIERS[(i / TECH_AREAS.len()) % TECH_TIERS.len()];
    format!("{} {}", area, tier)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rng_is_deterministic_and_names_are_stable() {
        let mut a = Rng::new(42); let mut b = Rng::new(42);
        for _ in 0..1000 { assert_eq!(a.next_u32(), b.next_u32()); }
        assert_eq!(building_name(7), building_name(7));
        assert_ne!(building_name(7), building_name(8));
        assert_eq!(ResourceKind::all().len(), 8);
        assert_eq!(Category::Military.class(), "military");
    }
}
