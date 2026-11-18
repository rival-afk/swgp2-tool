//! Shared front-end logic used by both the GUI and the TUI.
//!
//! Everything here is flag-free: the binaries take no arguments.

use crate::nrbf::{self, Graph};
use crate::save::{self};
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

/// Apply (path -> value) edits to the save **in place** (no re-serialization):
/// every byte the user didn't change stays byte-identical. Backs up first,
/// then verifies by decrypting and re-parsing the written file and comparing
/// its dump with the expected in-memory result.
pub fn patch_save(save_path: &Path, edits: &[(String, Json)]) -> Result<PathBuf, String> {
    if edits.is_empty() {
        return Err("no edits to apply".into());
    }
    let raw = fs::read(save_path).map_err(|e| e.to_string())?;
    let plain = crate::crypto::decrypt_save(&raw)?;
    let g = nrbf::parse(&plain).map_err(|e| e.to_string())?;

    // Resolve every target up-front (fail before touching the disk).
    let mut targets = Vec::with_capacity(edits.len());
    for (path, json) in edits {
        let target = save::patch_target(&g, path).map_err(|e| format!("{path}: {e}"))?;
        match &target {
            save::PatchTarget::Primitive { ptype, .. } => {
                let prim = save::prim_from_json(json, *ptype).map_err(|e| format!("{path}: {e}"))?;
                save::encode_prim(&prim).map_err(|e| format!("{path}: {e}"))?;
            }
            save::PatchTarget::Str { len, .. } => {
                let s = json.as_str().ok_or_else(|| format!("{path}: expected a string"))?;
                if s.len() != *len {
                    return Err(format!(
                        "{path}: string is {} bytes, the save holds {} bytes (only same-length strings can be patched)",
                        s.len(),
                        len
                    ));
                }
            }
        }
        targets.push(target);
    }

    // In-memory expected result (used for post-write verification).
    let mut expected_graph = nrbf::parse(&plain).map_err(|e| e.to_string())?;
    for (path, json) in edits {
        save::set_path(&mut expected_graph, path, json).map_err(|e| format!("{path}: {e}"))?;
    }
    let expected = render_json(&save::dump(&expected_graph).map_err(|e| e.to_string())?);

    let backup = save::create_backup(save_path).map_err(|e| e.to_string())?;

    let mut buf = plain.clone();
    for ((path, json), target) in edits.iter().zip(targets) {
        match target {
            save::PatchTarget::Primitive { offset, ptype } => {
                let prim = save::prim_from_json(json, ptype).map_err(|e| format!("{path}: {e}"))?;
                let enc = save::encode_prim(&prim).map_err(|e| format!("{path}: {e}"))?;
                buf[offset..offset + enc.len()].copy_from_slice(&enc);
            }
            save::PatchTarget::Str { payload_offset, len } => {
                let s = json.as_str().expect("validated above");
                buf[payload_offset..payload_offset + len].copy_from_slice(s.as_bytes());
            }
        }
    }

    let enc = crate::crypto::encrypt_save(&buf);
    fs::write(save_path, enc).map_err(|e| e.to_string())?;

    // Full verify: decrypt -> parse -> dump compare against the expected graph.
    let after = fs::read(save_path).map_err(|e| e.to_string())?;
    let plain2 = crate::crypto::decrypt_save(&after).map_err(|e| format!("verify failed: {e}"))?;
    let g2 = nrbf::parse(&plain2).map_err(|e| format!("verify failed to re-parse: {e}"))?;
    let actual = render_json(&save::dump(&g2).map_err(|e| e.to_string())?);

    if actual == expected {
        return Ok(backup);
    }
    // Verification failed: restore the backup so the game still has a working save.
    let _ = fs::copy(&backup, save_path);
    Err("written save failed verification and was restored from backup".into())
}

/// Apply an arbitrary path/value edit (the "advanced" box).
pub fn set_single(save_path: &Path, path: &str, value: Json) -> Result<PathBuf, String> {
    patch_save(save_path, &[(path.to_string(), value)])
}

/// Merge a JSON document (as produced by `dump`) into the save, but only
/// byte-patchable scalar leaves (numbers/bools/equal-length strings).
/// Structural changes (added/removed members, resized arrays) are rejected.
pub fn apply_json_file(save_path: &Path, json_file: &Path) -> Result<PathBuf, String> {
    let new: Json = serde_json::from_str(
        &fs::read_to_string(json_file).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("invalid JSON in {}: {e}", json_file.display()))?;

    let (g, _) = save::load(save_path).map_err(|e| e.to_string())?;
    let base = save::dump(&g).map_err(|e| e.to_string())?;

    let mut edits = Vec::new();
    let mut path = Vec::new();
    collect_scalar_edits(&base, &new, &mut path, &mut edits)?;
    if edits.is_empty() {
        return Err("nothing to change".into());
    }
    patch_save(save_path, &edits)
}

fn collect_scalar_edits(
    base: &Json,
    patch: &Json,
    path: &mut Vec<String>,
    out: &mut Vec<(String, Json)>,
) -> Result<(), String> {
    match (base, patch) {
        (Json::Object(b), Json::Object(p)) => {
            for (k, v) in p {
                let Some(bv) = b.get(k) else {
                    return Err(format!("no such member `{}` in the save", join_path(path, k)));
                };
                path.push(k.clone());
                collect_scalar_edits(bv, v, path, out)?;
                path.pop();
            }
            Ok(())
        }
        (Json::Array(b), Json::Array(p)) => {
            if b.len() != p.len() {
                return Err(format!(
                    "array size mismatch at `{}`: save has {} elements, JSON has {}",
                    path.join("."),
                    b.len(),
                    p.len()
                ));
            }
            for (i, v) in p.iter().enumerate() {
                path.push(i.to_string());
                collect_scalar_edits(&b[i], v, path, out)?;
                path.pop();
            }
            Ok(())
        }
        (Json::String(_), Json::String(_))
        | (Json::Number(_), Json::Number(_))
        | (Json::Bool(_), Json::Bool(_))
        | (Json::Null, Json::Null) => {
            out.push((path.join("."), patch.clone()));
            Ok(())
        }
        (_, Json::Null) => Ok(()), // cannot express a null via byte patching; skip
        _ => Err(format!("type mismatch at `{}`", path.join("."))),
    }
}

fn join_path(path: &[String], k: &str) -> String {
    let mut p = path.to_vec();
    p.push(k.to_string());
    p.join(".")
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
