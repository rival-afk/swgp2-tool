//! Full car catalog embedded from the game's own data files.
//!
//! Sources (see tool/wiki.rs and tool/cars.rs for the parsing of those):
//! - `StreamingAssets/BrandLogos/Original_names.txt` - all 200 shop cars
//!   (`id=DEALER#MODEL`, a leading R marks the rally variant)
//! - `StreamingAssets/CarThumbnails/IMD###.SW` - dealership preview for each car
//!   (the file index equals the car ID)
//! - `StreamingAssets/CAR_SKINS/<BRAND>/<FOLDER>` - every skin ever sold
//! - `StreamingAssets/Data/uselesslist.txt` - official power ranking (25 cars)
//! - the community IGCD database for the real-world cars the fictional ones are based on
//!
//! Nothing here is invented: missing stats simply show as "?" in the UI.

use std::path::PathBuf;

use crate::tool::game;

pub struct CatalogCar {
    pub id: u32,
    pub dealer: &'static str,
    pub model: &'static str,
    pub rally: bool,
    /// Real-world car this one is based on (IGCD, if the community knows it).
    pub real: &'static str,
    /// Filled in at load time from uselesslist.txt: (cv, top speed, year).
    pub cv: Option<u32>,
    pub top_speed: Option<u32>,
    pub year: Option<u32>,
}

/// Raw catalog: id|dealer|model|rally|real-world-name (empty real name = unknown)
const CATALOG_RAW: &str = include_str!("catalog.txt");

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

pub fn all_cars() -> Vec<CatalogCar> {
    let mut out = Vec::with_capacity(200);
    for (i, line) in CATALOG_RAW.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // format: id|DEALER|MODEL|RALLY(0/1)|real world name
        let mut it = line.split('|');
        let id: u32 = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(i as u32);
        let dealer = it.next().unwrap_or("?").trim();
        let model = it.next().unwrap_or("?").trim();
        let rally = it.next().map(|s| s.trim() == "1" || s.trim().eq_ignore_ascii_case("true")).unwrap_or(false);
        let real = it.next().unwrap_or("").trim();
        out.push(CatalogCar {
            id,
            dealer,
            model,
            rally,
            real,
            cv: None,
            top_speed: None,
            year: None,
        });
    }
    out
}

/// Load the full catalog merged with the official ranking from the local game copy.
pub fn load_catalog(game_dir: &std::path::Path) -> Vec<CatalogCar> {
    let mut cars = all_cars();
    attach_stats(&mut cars, game_dir);
    cars
}

/// Merge the official power ranking (uselesslist.txt) into the catalog, in place.
pub fn attach_stats(cars: &mut [CatalogCar], game_dir: &std::path::Path) {
    use crate::tool::cars::CarEntry;
    let txt = game::streaming_data_dir(game_dir).join("uselesslist.txt");
    let mut ranked = CarEntry::parse_file(&txt);
    // match by normalized full (dealer-less) name; rally variants need the R-prefix too
    for car in cars.iter_mut() {
        let key = norm(car.model);
        for r in ranked.iter_mut() {
            let rkey = norm(&r.name);
            if rkey == key {
                car.cv = Some(r.cv);
                car.top_speed = Some(r.top_speed);
                car.year = Some(r.year);
                break;
            }
        }
    }
}

/// Path to the dealership preview image of a car.
pub fn thumbnail_path(game_dir: &std::path::Path, id: u32) -> PathBuf {
    game::streaming_data_dir(game_dir)
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .join("CarThumbnails")
        .join(format!("IMD{id:03}.SW"))
}

/// Full display name: dealership + model, e.g. "SOOP GLORIA" or "RAVEN R892 (RALLY)".
pub fn display_name(car: &CatalogCar) -> String {
    if car.rally {
        format!("{} {} RALLY", car.dealer, car.model)
    } else {
        format!("{} {}", car.dealer, car.model)
    }
}
