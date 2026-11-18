//! Shared front-end logic used by both the GUI and the TUI.
//!
//! Everything here is flag-free: the binaries take no arguments.

use crate::nrbf::{self, Graph};
use crate::save::{self, SaveError};
use serde_json::Value as Json;
use std::fs;
use std::path::{Path, PathBuf};

pub fn render_json(j: &Json) -> String {
    serde_json::to_string_pretty(j).unwrap_or_else(|_| j.to_string())
}

/// Editor-friendly view of a save: (path, label). Used by both front-ends.
pub const COMMON_FIELDS: &[(&str, &str)] = &[
    ("Economy", "Credits (Cr)"),
    ("Coins", "Coins"),
    ("Language", "Language"),
    ("Difficulty", "Difficulty"),
    ("StatsData_.RacesCompleted", "Races completed"),
    ("StatsData_.Victories", "Victories"),
    ("StatsData_.MoneyWon", "Money won"),
    ("StatsData_.MoneySpent", "Money spent"),
    ("StatsData_.TotalKms", "Total distance (km)"),
];

/// Load a save file into a graph.
pub fn load(path: &Path) -> Result<Graph, String> {
    save::load(path).map(|(g, _)| g).map_err(|e| e.to_string())
}

/// Current value at `path` as an editor string.
pub fn field_text(g: &Graph, path: &str) -> String {
    match save::get_path(g, path) {
        Ok(Json::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// Parse editor text into a JSON value: numbers/bools/null stay typed,
/// anything else becomes a string.
pub fn parse_value(s: &str) -> Json {
    let t = s.trim();
    match serde_json::from_str::<Json>(t) {
        Ok(Json::Object(_)) | Ok(Json::Array(_)) => Json::String(t.to_string()), // don't set containers as typed values here
        Ok(v) => v,
        Err(_) => Json::String(t.to_string()),
    }
}

/// Apply (path -> value) edits to the graph, back up the original file,
/// write the save, then verify by re-parsing; restores the backup on failure.
/// Returns the backup path on success.
pub fn save_with_backup(save_path: &Path, edits: &[(String, Json)]) -> Result<PathBuf, String> {
    let (mut g, _raw) = save::load(save_path).map_err(|e| e.to_string())?;
    for (path, value) in edits {
        save::set_path(&mut g, path, value).map_err(|e| e.to_string())?;
    }
    let expected = render_json(&save::dump(&g).map_err(|e| e.to_string())?);

    let backup = save::create_backup(save_path).map_err(|e| e.to_string())?;
    save::store(save_path, &g).map_err(|e| e.to_string())?;

    // Re-read what we just wrote and confirm it round-trips to the same data.
    let after_bytes = fs::read(save_path).map_err(|e| e.to_string())?;
    let plain = crate::crypto::decrypt_save(&after_bytes).map_err(|e| format!("verify failed: {e}"))?;
    let g2 = nrbf::parse(&plain).map_err(|e| format!("verify failed to re-parse: {e}"))?;
    let after = render_json(&save::dump(&g2).map_err(|e| e.to_string())?);

    if expected == after {
        return Ok(backup);
    }
    let _ = fs::copy(&backup, save_path);
    Err(format!(
        "written save failed verification and was restored from backup {}",
        backup.display()
    ))
}

/// Apply an arbitrary path/value edit (the "advanced" box).
pub fn set_single(save_path: &Path, path: &str, value: Json) -> Result<PathBuf, String> {
    save_with_backup(save_path, &[(path.to_string(), value)])
}

/// Merge a JSON document (as produced by `dump`) into the save.
pub fn apply_json_file(save_path: &Path, json_file: &Path) -> Result<PathBuf, String> {
    let json: Json = serde_json::from_str(
        &fs::read_to_string(json_file).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("invalid JSON in {}: {e}", json_file.display()))?;

    let (mut g, _raw) = save::load(save_path).map_err(|e| e.to_string())?;
    save::merge_root(&mut g, &json).map_err(|e: SaveError| e.to_string())?;
    let expected = render_json(&save::dump(&g).map_err(|e| e.to_string())?);

    let backup = save::create_backup(save_path).map_err(|e| e.to_string())?;
    save::store(save_path, &g).map_err(|e| e.to_string())?;

    let after_bytes = fs::read(save_path).map_err(|e| e.to_string())?;
    let plain = crate::crypto::decrypt_save(&after_bytes).map_err(|e| format!("verify failed: {e}"))?;
    let g2 = nrbf::parse(&plain).map_err(|e| format!("verify failed to re-parse: {e}"))?;
    let after = render_json(&save::dump(&g2).map_err(|e| e.to_string())?);

    if expected == after {
        return Ok(backup);
    }
    let _ = fs::copy(&backup, save_path);
    Err(format!(
        "written save failed verification and was restored from backup {}",
        backup.display()
    ))
}

/// Decrypt the save to a raw BinaryFormatter dump JSON string (pretty).
pub fn dump_json(save_path: &Path) -> Result<String, String> {
    let (g, _) = save::load(save_path).map_err(|e| e.to_string())?;
    Ok(render_json(&save::dump(&g).map_err(|e| e.to_string())?))
}

/// Restore a backup file over the current save; backs up the current save first.
pub fn restore_backup(save_path: &Path, backup: &Path) -> Result<PathBuf, String> {
    if !backup.exists() {
        return Err(format!("backup not found: {}", backup.display()));
    }
    let current = save::create_backup(save_path).map_err(|e| e.to_string())?;
    fs::copy(backup, save_path).map_err(|e| e.to_string())?;
    Ok(current)
}

/// Existing save slots to show in the picker; falls back to the default path.
pub fn slot_choices() -> Vec<PathBuf> {
    let slots = save::list_slots();
    if !slots.is_empty() {
        return slots;
    }
    match save::default_save_path(0) {
        Some(p) => vec![p],
        None => Vec::new(),
    }
}
