//! Full reset of the game's saves.
//!
//! "Reset" = move every `*.sav` in the save folder into a timestamped archive
//! (`Backups\FullReset-<stamp>\`) instead of deleting outright - the reset is
//! reversible from inside the tool. A plain "delete" is available as an extra.

use crate::save;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ResetArchive {
    pub dir: PathBuf,
    pub stamp: String,
    pub files: Vec<PathBuf>,
    pub total_size: u64,
}

/// Save folder derived from the default save path (slot 0).
pub fn save_dir() -> Option<PathBuf> {
    save::default_save_path(0).and_then(|p| p.parent().map(PathBuf::from))
}

/// All .sav files directly inside `dir`.
pub fn saves_in(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().map(|e| e.eq_ignore_ascii_case("sav")).unwrap_or(false))
        .collect();
    out.sort();
    out
}

/// Seconds-resolution stamp, same as the regular backups.
fn stamp_now() -> String {
    save::chrono_stamp()
}

/// Archive every save into `Backups\FullReset-<stamp>\`.
/// Returns (archive, files that were moved).
pub fn stage_reset(dir: &Path) -> Result<ResetArchive, String> {
    let saves = saves_in(dir);
    if saves.is_empty() {
        return Err("no save files to reset".into());
    }
    let stamp = stamp_now();
    let archive_dir = dir.join("Backups").join(format!("FullReset-{stamp}"));
    fs::create_dir_all(&archive_dir).map_err(|e| e.to_string())?;

    let mut files = Vec::new();
    let mut total_size = 0u64;
    for src in &saves {
        let dst = archive_dir.join(src.file_name().unwrap());
        fs::copy(src, &dst).map_err(|e| format!("cannot archive {}: {e}", src.display()))?;
        total_size += dst.metadata().map(|m| m.len()).unwrap_or(0);
        files.push(dst);
    }
    Ok(ResetArchive {
        dir: archive_dir,
        stamp,
        files,
        total_size,
    })
}

/// Actually delete the live .sav files (call after stage_reset succeeded).
pub fn apply_reset(dir: &Path) -> Result<usize, String> {
    let saves = saves_in(dir);
    let mut n = 0;
    for p in saves {
        fs::remove_file(&p).map_err(|e| format!("cannot remove {}: {e}", p.display()))?;
        n += 1;
    }
    Ok(n)
}

/// Full reset: archive then delete. Backup-then-delete is atomic-ish: if the
/// archive step fails, nothing is deleted.
pub fn reset_all(dir: &Path) -> Result<(ResetArchive, usize), String> {
    let archive = stage_reset(dir)?;
    let n = apply_reset(dir)?;
    Ok((archive, n))
}

/// All previously made reset archives, newest first.
pub fn list_archives(dir: &Path) -> Vec<ResetArchive> {
    let backups = dir.join("Backups");
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(&backups) {
        for e in rd.flatten() {
            let p = e.path();
            let Some(name) = p.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let Some(stamp) = name.strip_prefix("FullReset-") else {
                continue;
            };
            if !p.is_dir() {
                continue;
            }
            let files: Vec<PathBuf> = fs::read_dir(&p)
                .into_iter()
                .flatten()
                .flatten()
                .map(|f| f.path())
                .filter(|f| f.extension().map(|x| x.eq_ignore_ascii_case("sav")).unwrap_or(false))
                .collect();
            let total_size = files.iter().filter_map(|f| f.metadata().ok()).map(|m| m.len()).sum();
            out.push(ResetArchive {
                dir: p,
                stamp: stamp.to_string(),
                files,
                total_size,
            });
        }
    }
    out.sort_by(|a, b| b.stamp.cmp(&a.stamp));
    out
}

/// Restore a reset archive back into the save folder.
/// Current saves are moved aside into a fresh `FullReset-pre-restore-<stamp>` archive first.
pub fn restore_archive(dir: &Path, archive: &ResetArchive) -> Result<usize, String> {
    if archive.files.is_empty() {
        return Err("archive is empty".into());
    }
    // Move current saves aside (don't delete silently).
    let current = saves_in(dir);
    if !current.is_empty() {
        let stamp = stamp_now();
        let aside = dir.join("Backups").join(format!("FullReset-pre-restore-{stamp}"));
        fs::create_dir_all(&aside).map_err(|e| e.to_string())?;
        for src in &current {
            let dst = aside.join(src.file_name().unwrap());
            fs::copy(src, &dst).map_err(|e| e.to_string())?;
            fs::remove_file(src).map_err(|e| e.to_string())?;
        }
    }
    let mut n = 0;
    for f in &archive.files {
        let dst = dir.join(f.file_name().unwrap());
        fs::copy(f, &dst).map_err(|e| format!("cannot restore {}: {e}", f.display()))?;
        n += 1;
    }
    Ok(n)
}

/// Delete a reset archive forever.
pub fn delete_archive(archive: &ResetArchive) -> Result<(), String> {
    fs::remove_dir_all(&archive.dir).map_err(|e| e.to_string())
}
