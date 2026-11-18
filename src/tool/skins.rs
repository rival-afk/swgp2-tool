//! CAR_SKINS scanner: lists every car skin image, grouped by brand.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinKind {
    Render,   // *base* / skin image
    Mask,     // car_bounds.png
    Other,
}

#[derive(Debug, Clone)]
pub struct SkinEntry {
    pub brand: String,
    pub car: String,
    pub file: PathBuf,
    pub kind: SkinKind,
}

pub fn scan(skins_root: &Path) -> Vec<SkinEntry> {
    fn kind_of(name: &str) -> SkinKind {
        let n = name.to_lowercase();
        if n.contains("bounds") {
            SkinKind::Mask
        } else if n.contains("base") || n.contains("skin") {
            SkinKind::Render
        } else {
            SkinKind::Other
        }
    }

    let mut out = Vec::new();
    let Ok(brands) = std::fs::read_dir(skins_root) else {
        return out;
    };
    for brand in brands.flatten() {
        let brand_name = brand.file_name().to_string_lossy().into_owned();
        if !brand.path().is_dir() {
            continue;
        }
        let Ok(cars) = std::fs::read_dir(brand.path()) else {
            continue;
        };
        for car in cars.flatten() {
            if !car.path().is_dir() {
                continue;
            }
            let car_name = car.file_name().to_string_lossy().into_owned();
            let Ok(files) = std::fs::read_dir(car.path()) else {
                continue;
            };
            for f in files.flatten() {
                let p = f.path();
                let ext_ok = p
                    .extension()
                    .map(|e| {
                        e.eq_ignore_ascii_case("png")
                            || e.eq_ignore_ascii_case("tga")
                            || e.eq_ignore_ascii_case("jpg")
                    })
                    .unwrap_or(false);
                if !ext_ok {
                    continue;
                }
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                out.push(SkinEntry {
                    brand: brand_name.clone(),
                    car: car_name.clone(),
                    file: p,
                    kind: kind_of(&name),
                });
            }
        }
    }
    out.sort_by(|a, b| {
        a.brand
            .to_lowercase()
            .cmp(&b.brand.to_lowercase())
            .then(a.car.to_lowercase().cmp(&b.car.to_lowercase()))
            .then(a.file.cmp(&b.file))
    });
    out
}
