//! Wiki content for the tool, built from data shipped inside the game.
//!
//! Sources (all inside the game folder):
//! - `StreamingAssets/Data/uselesslist.txt` - the game's own car power ranking
//!   (25 cars: name, CV, top speed, year; leading "-R" marks rally cars)
//! - `StreamingAssets/CAR_SKINS/<BRAND>/<CAR>/` - every car + its skins
//! - `Super Woden GP 2_Data/level0` - championship names screen
//!
//! Championship recommendations are ranked strictly by the official CV/speed
//! figures - they are not "playtested meta" and say so in the UI.

use crate::tool::cars::CarEntry;

/// Championship as shown on the in-game championships screen (level0 of
/// Super Woden GP 2_Data). Names of championships restricted to a car class
/// are marked; class data lives only in skin folder suffixes, so class
/// membership is approximate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChampKind {
    Rally,
    Circuit,
    Touring,
    Beginner,
}

impl ChampKind {
    pub fn label(self) -> &'static str {
        match self {
            ChampKind::Rally => "Rally",
            ChampKind::Circuit => "Circuit",
            ChampKind::Touring => "Touring",
            ChampKind::Beginner => "Beginner",
        }
    }
}

pub struct Championship {
    pub name: &'static str,
    pub kind: ChampKind,
    pub note: &'static str,
}

pub const CHAMPIONSHIPS: &[Championship] = &[
    Championship { name: "Rookie Challenge", kind: ChampKind::Beginner, note: "Entry-level circuit championship. Any car works in practice - pick the car with the highest CV you can afford, it will carry you further." },
    Championship { name: "Junior Series Championship", kind: ChampKind::Circuit, note: "The second circuit step. Cars sorted by CV are the safe picks." },
    Championship { name: "Super DTC CHAMPIONSHIP", kind: ChampKind::Touring, note: "DTC touring cars (skin folders named *DTC). Track-day feel, big speed." },
    Championship { name: "ONLY BTC CHAMPIONSHIP", kind: ChampKind::Touring, note: "BTC touring class (skin folders named *BTC). No official stats in game files for the BTC cars; pick by price/availability." },
    Championship { name: "300s Rally Championship", kind: ChampKind::Rally, note: "Rally with ~300 hp cars. Use rally cars (skin folders prefixed R*)." },
    Championship { name: "400s WORLD RALLY CHAMPIONSHIP", kind: ChampKind::Rally, note: "The top rally division. Rally cars only - best CV first." },
    Championship { name: "SUPER WORLD RALLY CHAMPIONSHIP", kind: ChampKind::Rally, note: "World Rally division. Rally cars only - best CV first." },
    Championship { name: "JTC SUPER BATTLE", kind: ChampKind::Touring, note: "JTC touring class (skin folders named *JTC). Strong aero, big speed." },
];

/// Cars with "rally" in their folder name or the official "-R" prefix, sorted by CV desc.
pub fn rally_cars<'a>(cars: &'a [CarEntry]) -> Vec<&'a CarEntry> {
    let mut v: Vec<&CarEntry> = cars.iter().filter(|c| c.rally).collect();
    v.sort_by(|a, b| b.cv.cmp(&a.cv).then(b.top_speed.cmp(&a.top_speed)));
    v
}

/// Non-rally cars sorted by CV desc.
pub fn circuit_cars<'a>(cars: &'a [CarEntry]) -> Vec<&'a CarEntry> {
    let mut v: Vec<&CarEntry> = cars.iter().filter(|c| !c.rally).collect();
    v.sort_by(|a, b| b.cv.cmp(&a.cv).then(b.top_speed.cmp(&a.top_speed)));
    v
}

/// Ranked: pick the N best cars for a championship kind.
pub fn pick_for(cars: &[CarEntry], kind: ChampKind, n: usize) -> Vec<&CarEntry> {
    let mut v: Vec<&CarEntry> = cars
        .iter()
        .filter(|c| match kind {
            ChampKind::Rally => c.rally,
            _ => !c.rally,
        })
        .collect();
    v.sort_by(|a, b| b.cv.cmp(&a.cv).then(b.top_speed.cmp(&a.top_speed)));
    v.truncate(n);
    v
}

/// Brand name for a car from its skin folders (first folder's brand).
pub fn brand_of(car: &CarEntry) -> String {
    car.skin_dirs
        .first()
        .and_then(|d| {
            d.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "?".to_string())
}

/// Practical tips referencing only mechanics visible in the game/save.
pub const TIPS: &[&str] = &[
    "The game must be closed while editing - it overwrites the save on exit.",
    "Cars degrade with use (see Kms, Oil, DirtyNess in the save): keep the tank full and repair before big races.",
    "Rally cars are stored in CAR_SKINS folders prefixed with R (e.g. RSHUKUFUKU) - they exist for most ranked cars.",
    "GarageSlots in the save holds every car instance you own; the tool's Save Editor can set any field in it.",
    "The game keeps a Backups folder next to the save; the tool adds its own timestamped backups on every write.",
    "uselesslist.txt is the official in-game ranking of the fastest 25 cars - it is the baseline for all picks in this wiki.",
    "If a championship limits you to a class (BTC, DTC, JTC), no official stats exist for those cars - pick by price and availability.",
    "Weekend races exist as a separate mode (WeekendStarGroups in the save) - same mechanics, different rewards.",
];
