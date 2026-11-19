//! Wiki content for the tool, built from the game's own data files.
//!
//! Sources (all inside the game folder):
//! - `StreamingAssets/BrandLogos/Original_names.txt` - the IDs of all 200 cars
//!   (`id=DEALER#MODEL`, a leading ® marks the rally variant)
//! - `StreamingAssets/CarThumbnails/IMD###.SW` - dealership preview per car ID
//! - `StreamingAssets/CAR_SKINS/<BRAND>/<CAR>/` - every car + its skins
//! - `StreamingAssets/Data/uselesslist.txt` - official power ranking (25 cars)
//! - Super Woden GP 2_Data/level0 - championships/tracks screen
//!
//! Recommendations are ranked strictly by official CV/speed where known.
//! Class-restricted championships/tracks (BTC, DTC, JTC) have no official
//! stats in the game's data files and are shown as unknown.

use crate::tool::catalog::CatalogCar;

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
            ChampKind::Rally => "RALLY",
            ChampKind::Circuit => "CIRCUIT",
            ChampKind::Touring => "TOURING",
            ChampKind::Beginner => "BEGINNER",
        }
    }

    pub fn is_touring(self) -> bool {
        self == ChampKind::Touring
    }
}

pub struct Championship {
    pub name: &'static str,
    pub kind: ChampKind,
    pub note: &'static str,
    /// Optional class restriction ("DTC", "BTC", "JTC") baked into the league rules.
    pub class_token: Option<&'static str>,
}

pub const CHAMPIONSHIPS: &[Championship] = &[
    Championship {
        name: "Rookie Challenge",
        kind: ChampKind::Beginner,
        class_token: None,
        note: "Entry-level circuit championship. Any car works; pick the fastest one you can afford - it will carry you further.",
    },
    Championship {
        name: "Junior Series Championship",
        kind: ChampKind::Beginner,
        class_token: None,
        note: "The second circuit step. The cars below are ranked by official CV/speed.",
    },
    Championship {
        name: "Junior Series - Kenya",
        kind: ChampKind::Beginner,
        class_token: None,
        note: "Junior Series in Kenya - circuit racing on African tracks.",
    },
    Championship {
        name: "Junior Series - Spain",
        kind: ChampKind::Beginner,
        class_token: None,
        note: "Junior Series in Spain.",
    },
    Championship {
        name: "Junior Series - Sweden",
        kind: ChampKind::Beginner,
        class_token: None,
        note: "Junior Series in Sweden.",
    },
    Championship {
        name: "300s Series - Argentina",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Rally series in Argentina.",
    },
    Championship {
        name: "300s Series - Finland",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Rally series in Finland.",
    },
    Championship {
        name: "300s Series - Kenya",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Rally series in Kenya.",
    },
    Championship {
        name: "300s Series - Spain",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Rally series in Spain.",
    },
    Championship {
        name: "300s Series - Sweden",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Rally series in Sweden.",
    },
    Championship {
        name: "400s Series - Argentina",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Top rally series in Argentina.",
    },
    Championship {
        name: "400s Series - Finland",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Top rally series in Finland.",
    },
    Championship {
        name: "400s Series - Kenya",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Top rally series in Kenya.",
    },
    Championship {
        name: "400s Series - Spain",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Top rally series in Spain.",
    },
    Championship {
        name: "400s Series - Sweden",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Top rally series in Sweden.",
    },
    Championship {
        name: "SUPER WORLD RALLY CHAMPIONSHIP",
        kind: ChampKind::Rally,
        class_token: None,
        note: "The top rally championship. Rally cars only - ranked by official CV.",
    },
    Championship {
        name: "300s Rally Championship",
        kind: ChampKind::Rally,
        class_token: None,
        note: "Rally championship with ~300 hp cars. Rally cars only.",
    },
    Championship {
        name: "400s WORLD RALLY CHAMPIONSHIP",
        kind: ChampKind::Rally,
        class_token: None,
        note: "The top world rally division. Rally cars only - ranked by official CV.",
    },
    Championship {
        name: "Super DTC CHAMPIONSHIP",
        kind: ChampKind::Touring,
        class_token: Some("DTC"),
        note: "DTC touring cars (models ending in DTC). Ranked by CV/speed within the class.",
    },
    Championship {
        name: "ONLY BTC CHAMPIONSHIP",
        kind: ChampKind::Touring,
        class_token: Some("BTC"),
        note: "BTC touring cars (models ending in BTC). Ranked by CV/speed within the class.",
    },
    Championship {
        name: "JTC SUPER BATTLE",
        kind: ChampKind::Touring,
        class_token: Some("JTC"),
        note: "JTC touring class (models ending in JTC). Ranked by CV/speed within the class.",
    },
];

/// Named races/tracks and their class. All names below are taken from the
/// game's own level files (level0 = the championships screen).
pub const TRACKS: &[Championship] = &[
    // --- circuits
    Championship { name: "Natsuko Circuit", kind: ChampKind::Circuit, class_token: None, note: "The flagship circuit. Standard circuit racing." },
    Championship { name: "Natsuko Circuit 24H", kind: ChampKind::Circuit, class_token: None, note: "24-hour endurance race at Natsuko Circuit." },
    Championship { name: "Denise Beach", kind: ChampKind::Circuit, class_token: None, note: "Beach circuit. Long, sweepers-heavy layout." },
    Championship { name: "Denise Beach Trophy", kind: ChampKind::Circuit, class_token: None, note: "Denise Beach trophy race." },
    Championship { name: "Plattsburg Sunset", kind: ChampKind::Circuit, class_token: None, note: "Street circuit at sunset." },
    Championship { name: "Plattsburg Race", kind: ChampKind::Circuit, class_token: None, note: "Plattsburg street race." },
    Championship { name: "Plattsburg 24H", kind: ChampKind::Circuit, class_token: None, note: "24-hour endurance race at Plattsburg." },
    Championship { name: "Limestone Hatch", kind: ChampKind::Circuit, class_token: None, note: "Limestone quarry track." },
    Championship { name: "West Millswood", kind: ChampKind::Circuit, class_token: None, note: "Countryside circuit, rolling hills." },
    Championship { name: "West Millswood 8H", kind: ChampKind::Circuit, class_token: None, note: "8-hour endurance race at West Millswood." },
    Championship { name: "Ichikawa", kind: ChampKind::Circuit, class_token: None, note: "Japanese street circuit." },
    Championship { name: "Ichikawa 12H", kind: ChampKind::Circuit, class_token: None, note: "12-hour endurance race at Ichikawa." },
    // --- trophies / cups
    Championship { name: "Astral Speedway Trophy", kind: ChampKind::Circuit, class_token: None, note: "Oval speedway oval, high speed." },
    Championship { name: "Bolboreta Trophy", kind: ChampKind::Circuit, class_token: None, note: "Bolboreta circuit trophy." },
    Championship { name: "Eisenach Trophy", kind: ChampKind::Circuit, class_token: None, note: "Eisenach trophy." },
    Championship { name: "Greenlands 24H", kind: ChampKind::Circuit, class_token: None, note: "Greenlands endurance race." },
    Championship { name: "Knights Cup", kind: ChampKind::Circuit, class_token: None, note: "Knights of the ... cup." },
    Championship { name: "HERO CUP", kind: ChampKind::Circuit, class_token: None, note: "HERO CUP." },
    Championship { name: "MOON CUP", kind: ChampKind::Circuit, class_token: None, note: "Moon Cup." },
    Championship { name: "NE RACE", kind: ChampKind::Circuit, class_token: None, note: "Northern European race." },
    Championship { name: "SW HERO CUP", kind: ChampKind::Circuit, class_token: None, note: "Southwest hero cup." },
    Championship { name: "SW WORLD CUP", kind: ChampKind::Circuit, class_token: None, note: "Southwest world cup." },
    Championship { name: "AUBURN RACE", kind: ChampKind::Circuit, class_token: None, note: "Auburn race." },
    Championship { name: "DALTON RACE", kind: ChampKind::Circuit, class_token: None, note: "Dalton race." },
    Championship { name: "SUPERHAI RACE", kind: ChampKind::Circuit, class_token: None, note: "Superhai race." },
    Championship { name: "SuperHai Vs Dalton Race", kind: ChampKind::Circuit, class_token: None, note: "Superhai vs Dalton head-to-head." },
    Championship { name: "Sivi IV Cup", kind: ChampKind::Circuit, class_token: None, note: "Sivi IV cup." },
    Championship { name: "Junior Cup", kind: ChampKind::Beginner, class_token: None, note: "Junior class cup." },
    // --- rally stages (from level0: country names + S<stage number>)
    Championship { name: "Argentina S1", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Argentina." },
    Championship { name: "Argentina S2", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Argentina." },
    Championship { name: "Finland S1", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Finland." },
    Championship { name: "Finland S2", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Finland." },
    Championship { name: "Finland S3", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Finland." },
    Championship { name: "Kenya S1", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Kenya." },
    Championship { name: "Kenya S2", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Kenya." },
    Championship { name: "Kenya S3", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Kenya." },
    Championship { name: "Kenya S4", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Kenya." },
    Championship { name: "Spain S1", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Spain." },
    Championship { name: "Spain S2", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Spain." },
    Championship { name: "Spain S3", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Spain." },
    Championship { name: "Spain S4", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Spain." },
    Championship { name: "Sweden S1", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Sweden." },
    Championship { name: "Sweden S2", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Sweden." },
    Championship { name: "Sweden S3", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Sweden." },
    Championship { name: "Sweden S4", kind: ChampKind::Rally, class_token: None, note: "Rally stage in Sweden." },
    Championship { name: "Usa Hillclimb S1", kind: ChampKind::Rally, class_token: None, note: "Hillclimb stage in the USA." },
];

/// Ranked: the N best known-stat cars for a championship kind, then CV desc,
/// then top speed desc. `class_token` (DTC/BTC/JTC/GR.5/…) restricts the pool
/// to cars whose model name contains the token.
pub fn best_picks<'a>(cars: &'a [CatalogCar], kind: ChampKind, class_token: Option<&str>, n: usize) -> Vec<&'a CatalogCar> {
    let mut v: Vec<&CatalogCar> = cars
        .iter()
        .filter(|c| match kind {
            ChampKind::Rally | ChampKind::Touring => c.rally == (kind == ChampKind::Rally),
            ChampKind::Circuit | ChampKind::Beginner => !c.rally,
        })
        .filter(|c| !kind.is_touring() || is_tourer(c))
        .filter(|c| {
            class_token
                .map(|t| c.model.to_ascii_uppercase().contains(&t.to_ascii_uppercase()))
                .unwrap_or(true)
        })
        .collect();
    v.sort_by(|a, b| {
        let av = a.cv.unwrap_or(0);
        let bv = b.cv.unwrap_or(0);
        bv.cmp(&av).then(b.top_speed.unwrap_or(0).cmp(&a.top_speed.unwrap_or(0)))
    });
    v.truncate(n);
    v
}

/// Rough heuristic: is this touring-class by folder name?
fn is_tourer(c: &CatalogCar) -> bool {
    let n = c.model.to_ascii_uppercase();
    n.contains("BTC") || n.contains("DTC") || n.contains("JTC")
}

/// Practical tips referencing only mechanics visible in the game/save.
pub const TIPS: &[&str] = &[
    "The game must be closed while editing - it overwrites the save on exit.",
    "Cars degrade with use (see Kms, Oil, DirtyNess in the save): repair before big races and keep supplies stocked.",
    "Rally cars are stored in CAR_SKINS folders prefixed with R (e.g. RSHUKUFUKU) - they exist for most ranked cars.",
    "GarageSlots in the save holds every car instance you own; the tool's Save Editor can set any field in it.",
    "The game keeps a Backups folder next to the save; the tool adds its own timestamped backups on every write.",
    "uselesslist.txt is the official in-game ranking of the fastest 25 cars - it is the baseline for all picks in this wiki.",
    "If a championship or track limits you to a class (BTC, DTC, JTC), no official stats exist for those cars - pick by price and availability.",
    "Weekend races exist as a separate mode (WeekendStarGroups in the save) - same mechanics, different rewards.",
];
