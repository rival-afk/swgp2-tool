//! Game folder detection & info for Super Woden GP 2 (portable Unity game).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const EXE_NAME: &str = "Super Woden GP 2.exe";

fn is_game_dir(dir: &Path) -> bool {
    dir.join(EXE_NAME).is_file() && dir.join("Super Woden GP 2_Data").is_dir()
}

/// Shallow, bounded search for the game folder. Checks (in order):
/// 1. the persisted choice from settings.json,
/// 2. drive roots (top 2 levels + `<root>\games`),
/// 3. the user's Downloads/Desktop/Documents folders (one level down) - this is
///    where portable-game zip archives usually get extracted,
/// 4. `<USERPROFILE>\Super Woden GP 2`.
/// Never scans whole disks, always bounded.
pub fn detect_game_dir() -> Option<PathBuf> {
    // 1. persisted choice (GUI "Locate game folder…")
    if let Some(dir) = crate::tool::settings::load()
        .game_dir
        .filter(|d| is_game_dir(d))
    {
        return Some(dir);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    let roots = ["C:\\", "D:\\", "E:\\", "F:\\"]
        .iter()
        .map(PathBuf::from)
        .filter(|r| r.is_dir())
        .collect::<Vec<_>>();

    // 2. drive roots
    for root in &roots {
        candidates.push(root.clone());
        if let Ok(rd) = fs::read_dir(root) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    candidates.push(p);
                }
            }
        }
        if let Ok(rd) = fs::read_dir(root.join("games")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    candidates.push(p);
                }
            }
        }
    }

    // 3. the user's usual "where games live" folders, one level down
    if let Some(home) = std::env::var_os("USERPROFILE") {
        for sub in ["Downloads", "Desktop", "Documents"] {
            let base = PathBuf::from(&home).join(sub);
            if let Ok(rd) = fs::read_dir(&base) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        candidates.push(p.clone());
                        // also the exe directly inside the game folder (scan one level deeper)
                        if let Ok(rd2) = fs::read_dir(&p) {
                            for e2 in rd2.flatten() {
                                let p2 = e2.path();
                                if p2.is_dir() && is_game_dir(&p2) {
                                    candidates.push(p2);
                                }
                            }
                        }
                    }
                }
            }
        }
        candidates.push(PathBuf::from(&home).join("Super Woden GP 2"));
    }

    candidates.into_iter().find(|d| {
        let name = d.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        (name.contains("swgp2") || name.contains("woden") || is_game_dir(d)) && is_game_dir(d)
    })
}

#[derive(Debug, Clone)]
pub struct GameInfo {
    pub dir: PathBuf,
    pub exe: PathBuf,
    pub data_dir: PathBuf,
    pub save_dir: PathBuf,
    pub app_company: String,
    pub app_product: String,
    pub exe_size: u64,
    pub exe_modified: Option<SystemTime>,
    pub boot_config: Vec<(String, String)>,
    pub skin_cars: usize,
    pub car_skins: usize,
    pub save_slots: usize,
    pub backups: usize,
}

pub fn game_info(dir: &Path) -> Result<GameInfo, String> {
    let exe = dir.join(EXE_NAME);
    if !exe.is_file() {
        return Err(format!("{EXE_NAME} not found in {}", dir.display()));
    }
    let data_dir = dir.join("Super Woden GP 2_Data");
    if !data_dir.is_dir() {
        return Err(format!("Super Woden GP 2_Data not found in {}", dir.display()));
    }

    // app.info: "<company>\n<product>"
    let (app_company, app_product) = fs::read_to_string(data_dir.join("app.info"))
        .map(|s| {
            let mut lines = s.lines();
            (
                lines.next().unwrap_or("?").trim().to_string(),
                lines.next().unwrap_or("?").trim().to_string(),
            )
        })
        .unwrap_or_else(|_| ("?".into(), "?".into()));

    let boot_config = fs::read_to_string(data_dir.join("boot.config"))
        .map(|s| {
            s.lines()
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                .collect()
        })
        .unwrap_or_default();

    let meta = fs::metadata(&exe).map_err(|e| e.to_string())?;

    let save_dir = crate::save::default_save_path(0)
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_default();
    let save_slots = crate::save::list_slots().len();
    let backups = save_dir
        .join("Backups")
        .read_dir()
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().map(|x| x == "savbak").unwrap_or(false))
                .count()
        })
        .unwrap_or(0);

    // Skin counts (best effort).
    let skins_root = data_dir.join("StreamingAssets").join("CAR_SKINS");
    let (mut skin_cars, mut car_skins) = (0usize, 0usize);
    if let Ok(brands) = fs::read_dir(&skins_root) {
        for brand in brands.flatten() {
            if let Ok(cars) = fs::read_dir(brand.path()) {
                for car in cars.flatten() {
                    if car.path().is_dir() {
                        skin_cars += 1;
                        car_skins += fs::read_dir(car.path())
                            .map(|rd| {
                                rd.flatten()
                                    .filter(|f| {
                                        let p = f.path();
                                        p.extension()
                                            .map(|x| x.eq_ignore_ascii_case("png") || x.eq_ignore_ascii_case("tga") || x.eq_ignore_ascii_case("jpg"))
                                            .unwrap_or(false)
                                    })
                                    .count()
                            })
                            .unwrap_or(0);
                    }
                }
            }
        }
    }

    Ok(GameInfo {
        dir: dir.to_path_buf(),
        exe,
        data_dir,
        save_dir,
        app_company,
        app_product,
        exe_size: meta.len(),
        exe_modified: meta.modified().ok(),
        boot_config,
        skin_cars,
        car_skins,
        save_slots,
        backups,
    })
}

/// Path to StreamingAssets/Data (credits, car DB).
pub fn streaming_data_dir(game_dir: &Path) -> PathBuf {
    game_dir.join("Super Woden GP 2_Data").join("StreamingAssets").join("Data")
}

/// Path to StreamingAssets/CAR_SKINS.
pub fn skins_root(game_dir: &Path) -> PathBuf {
    game_dir.join("Super Woden GP 2_Data").join("StreamingAssets").join("CAR_SKINS")
}

/// Path to the CustomMusic folder.
pub fn custom_music_dir(game_dir: &Path) -> PathBuf {
    game_dir.join("Super Woden GP 2_Data").join("StreamingAssets").join("CustomMusic")
}
