//! Parser for StreamingAssets/Data/uselesslist.txt - the game's car power ranking.
//!
//! Line format (after mojibake cleanup):  `[-R]NAME… <cv> CV  <kmh> Kmh  || <year>`
//! Example: `MOBY DICK PATROL 240 CV  240 Kmh  || 1974`
//!          `-RSHUKUFUKU 250 CV  214 Kmh  || 1987`   (leading "-R" = rally car)

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CarEntry {
    pub name: String,
    pub rally: bool,
    pub cv: u32,
    pub top_speed: u32,
    pub year: u32,
    /// All CAR_SKINS folders matching this car (base + rally variants).
    pub skin_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarSort {
    Rank, // file order
    Cv,
    TopSpeed,
    Year,
    Name,
}

/// Normalize a car name / folder name for comparison (A–Z0–9 only, uppercase).
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

impl CarEntry {
    /// Key used to match skin folders: rally cars live in "R<NAME>" folders.
    pub fn skin_key(&self) -> String {
        if self.rally {
            format!("R{}", norm(&self.name))
        } else {
            norm(&self.name)
        }
    }

    pub fn parse_file(path: &Path) -> Vec<CarEntry> {
        let Ok(bytes) = std::fs::read(path) else {
            return Vec::new();
        };
        // The file is not reliably UTF-8; lossy conversion is fine, we only
        // need ASCII structure around the numbers.
        let text = String::from_utf8_lossy(&bytes);
        let mut out = Vec::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || !line.contains("CV") {
                continue;
            }
            let tokens: Vec<&str> = line.split_whitespace().collect();
            // find the "CV" token
            let Some(cv_i) = tokens.iter().position(|t| t.eq_ignore_ascii_case("CV")) else {
                continue;
            };
            if cv_i == 0 {
                continue;
            }
            let cv: u32 = match tokens[cv_i - 1].parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let top_speed: u32 = tokens.get(cv_i + 1).and_then(|t| t.parse().ok()).unwrap_or(0);
            let year: u32 = tokens.iter().rev().find_map(|t| t.parse::<u32>().ok()).unwrap_or(0);

            // name = tokens before cv, with leading rally marker stripped
            let mut name_tokens: Vec<&str> = tokens[..cv_i - 1].to_vec();
            let mut rally = false;
            // detect leading "-R" / "–R" attached to first token
            if let Some(first) = name_tokens.first().copied() {
                let stripped: String = first
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_uppercase();
                if stripped.starts_with('R') && first.chars().any(|c| !c.is_ascii_alphanumeric()) {
                    rally = true;
                    name_tokens[0] = &first[first.find('R').unwrap_or(0) + 1..];
                    if name_tokens[0].is_empty() {
                        name_tokens.remove(0);
                    }
                }
            }
            let name = name_tokens
                .join(" ")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '.')
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if name.is_empty() {
                continue;
            }
            out.push(CarEntry {
                name,
                rally,
                cv,
                top_speed,
                year,
                skin_dirs: Vec::new(),
            });
        }
        out
    }
}

/// Match each car to its skin folder(s) under `skins_root` (CAR_SKINS/<brand>/<car>).
pub fn attach_skin_dirs(cars: &mut [CarEntry], skins_root: &Path) {
    // index folders once: normalized folder name -> path
    let mut folders: Vec<(String, PathBuf)> = Vec::new();
    if let Ok(brands) = std::fs::read_dir(skins_root) {
        for brand in brands.flatten() {
            let bpath = brand.path();
            if !bpath.is_dir() {
                continue;
            }
            if let Ok(car_dirs) = std::fs::read_dir(&bpath) {
                for car in car_dirs.flatten() {
                    let cpath = car.path();
                    if cpath.is_dir() {
                        if let Some(name) = cpath.file_name() {
                            folders.push((norm(&name.to_string_lossy()), cpath));
                        }
                    }
                }
            }
        }
    }

    for car in cars.iter_mut() {
        let key = car.skin_key();
        car.skin_dirs = folders
            .iter()
            .filter(|(f, _)| *f == key || (f.starts_with(&key) && f.len() <= key.len() + 4))
            .map(|(_, p)| p.clone())
            .collect();
        // Also keep the plain (non-rally) folder for rally cars as fallback.
        if car.rally && car.skin_dirs.is_empty() {
            let base = norm(&car.name);
            car.skin_dirs = folders.iter().filter(|(f, _)| *f == base).map(|(_, p)| p.clone()).collect();
        }
    }
}

/// Pick a preview image for a skin folder: prefer a "*base*" image, else the first image.
pub fn pick_preview(skin_dirs: &[PathBuf]) -> Option<PathBuf> {
    let is_img = |p: &Path| {
        p.extension()
            .map(|e| {
                e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("tga") || e.eq_ignore_ascii_case("jpg")
            })
            .unwrap_or(false)
    };
    let mut first_img = None;
    for dir in skin_dirs {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if !is_img(&p) {
                    continue;
                }
                let fname = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                if fname.contains("base") {
                    return Some(p);
                }
                if first_img.is_none() {
                    first_img = Some(p);
                }
            }
        }
    }
    first_img
}

/// All render images in a car's skin folders.
pub fn list_skin_images(skin_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in skin_dirs {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                let ext_ok = p
                    .extension()
                    .map(|x| {
                        x.eq_ignore_ascii_case("png") || x.eq_ignore_ascii_case("tga") || x.eq_ignore_ascii_case("jpg")
                    })
                    .unwrap_or(false);
                if ext_ok {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}
