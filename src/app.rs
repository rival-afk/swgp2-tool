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

/// Editor-friendly view of a save: (path, label, hint). Used by both
/// front-ends. The hint tells the user what the value means and either lists
/// all possible values or the numeric range.
pub struct FieldMeta {
    pub path: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

pub const COMMON_FIELDS: &[FieldMeta] = &[
    FieldMeta {
        path: "Economy",
        label: "Credits (Cr)",
        hint: "Range: 0 … 999,999,999 Cr. The 999,999,999 cheat button sets this value.",
    },
    FieldMeta {
        path: "Coins",
        label: "Coins",
        hint: "Range: 0 … 999,999 coins.",
    },
    FieldMeta {
        path: "Language",
        label: "Language",
        hint: "All possible values: RUS, ENG (the game ships both; anything else may fail to load). Only equal-length strings can be patched.",
    },
    FieldMeta {
        path: "Difficulty",
        label: "Difficulty",
        hint: "Range: 0 … 2 (AI difficulty, 0 = easiest, 2 = hardest). 1 in the default save.",
    },
    FieldMeta {
        path: "StatsData_.RacesCompleted",
        label: "Races completed",
        hint: "Range: 0 … 65,535 (uint16) — lifetime count of races.",
    },
    FieldMeta {
        path: "StatsData_.Victories",
        label: "Victories",
        hint: "Range: 0 … 65,535 (uint16) — lifetime count of wins.",
    },
    FieldMeta {
        path: "StatsData_.MoneyWon",
        label: "Money won",
        hint: "Range: 0 … 999,999,999 Cr — lifetime money won.",
    },
    FieldMeta {
        path: "StatsData_.MoneySpent",
        label: "Money spent",
        hint: "Range: 0 … 999,999,999 Cr — lifetime money spent.",
    },
    FieldMeta {
        path: "StatsData_.TotalKms",
        label: "Total distance (km)",
        hint: "Range: 0 … 999,999.9 km (float) — lifetime distance.",
    },
];

/// Hints for every other well-known path in the save. Used by the "Advanced -
/// set any path" box (exact match, array indices normalised to `N`).
pub const FIELD_HINTS: &[(&str, &str)] = &[
    // --- root
    ("Analog", "All possible values: false = digital controls, true = analog controls."),
    ("CornerWarning", "All possible values: false / true — show the corner-cut (track limits) warning."),
    ("ShowRecords", "All possible values: false / true — show lap records in the HUD."),
    ("Miles", "All possible values: false = kilometres, true = miles."),
    ("Vibration", "All possible values: false / true — controller vibration."),
    ("Damage", "All possible values: false / true — car damage enabled."),
    ("Gamemode", "All possible values seen in-game: RALLY, TIMETRIAL. Default: RALLY (fresh saves), do not set unless you know the mode exists."),
    ("Gametime", "Range: 0 … 4,294,967,295 s (float) — total time played."),
    ("Myslot", "Range: 0 … 999 — index of the active garage slot."),
    ("SortMode", "Range: 0 … 3 — garage car list sort order. 3 in the default save."),
    ("VideoMode", "Range: 0 … 2 — video mode (0/1/2, likely windowed/borderless/fullscreen). 2 in the default save."),
    ("QualityLevel", "Range: 0 … 3 — graphics quality preset index (0 = lowest, higher = better). 2 in the default save."),
    ("HistoricSlot", "Range: 0 … 999 — historic-car slot index."),
    ("LeagueStars", "Null in the default save. Championship star record; cannot be patched (null ↔ value unsupported)."),
    // --- garage slots
    ("GarageSlots.N.CarID", "Range: 0 … 199 — car id (IMD<id> / Original_names.txt id; id 0–199 map 1:1 to the shop catalog)."),
    ("GarageSlots.N.SlotID", "Range: 0 … 999 — garage slot index (order of the car in the garage)."),
    ("GarageSlots.N.SkinID", "Range: 0 … 999 — skin id within the car's skin folder."),
    ("GarageSlots.N.Kms", "Range: 0 … 999,999.9 km (float) — mileage/wear of this car."),
    ("GarageSlots.N.DirtyNess", "Range: 0 … 999 — dirt level of this car."),
    ("GarageSlots.N.Oil", "Range: 0 … 999 — oil level of this car."),
    ("GarageSlots.N.RimsID", "Range: 0 … 999 — rims style id."),
    ("GarageSlots.N.Gift", "All possible values: false / true — the car was a gift."),
    ("GarageSlots.N.Engine", "Range: 0 … 999 — engine upgrade level."),
    ("GarageSlots.N.Brakes", "Range: 0 … 999 — brakes upgrade level."),
    ("GarageSlots.N.Chassis", "Range: 0 … 999 — chassis upgrade level."),
    ("GarageSlots.N.Gearbox", "Range: 0 … 999 — gearbox upgrade level."),
    ("GarageSlots.N.Suspension", "Range: 0 … 999 — suspension upgrade level."),
    ("GarageSlots.N.Tyres", "Range: 0 … 999 — tyres upgrade level."),
    // --- league (circuit championships)
    ("LeagueStatus_.ChampIndex", "Range: 0 … 255 — index of the current championship."),
    ("LeagueStatus_.GarSlot", "Range: 0 … 999 — garage slot used for the league."),
    ("LeagueStatus_.Round", "Range: 0 … 999 — current round index (0 = not started)."),
    ("LeagueStatus_.ChampPrices.N", "Range: 0 … 999,999,999 Cr — entry price of a championship; last two entries are 0 (free)."),
    ("LeagueStatus_.Points.N", "Range: 0 … 65,535 — championship points scored (one entry per championship)."),
    ("LeagueStatus_.Stars.N", "Range: 0 … 255 — stars earned."),
    // --- weekend events
    ("WeekendStatus_.ChampIndex", "Range: 0 … 255 — index of the current weekend event."),
    ("WeekendStatus_.Round", "Range: 0 … 255 — current round of the weekend event."),
    ("WeekendStatus_.ChampPrices.N", "Range: 0 … 999,999,999 Cr — entry price of a weekend event; trailing entries are 0 (free)."),
    ("WeekendStarGroups.N", "Range: 0 … 255 — star rating of a weekend event."),
    // --- rally championship
    ("RallyStatus_.RallyIndex", "Range: 0 … 255 — index of the current rally."),
    ("RallyStatus_.Round", "Range: 0 … 255 — current rally round."),
    ("RallyStatus_.GarSlot", "Range: 0 … 999 — garage slot used for the rally."),
    ("RallyStatus_.ChampPrices.N", "Range: 0 … 999,999,999 Cr — rally entry price; trailing entries are 0 (free)."),
    ("RallyStatus_.PlayerRoundTimes.N", "Range: 0 … 999,999.9 s (float) — your stage times (25 slots)."),
    ("RallyStatus_.OpponentTimes.N", "Range: 0 … 999,999.9 s (float) — AI rivals' stage times."),
    ("RallyStatus_.OpponentTotalTimes.N", "Range: 0 … 999,999.9 s (float) — AI rivals' total rally times."),
    ("RallyStatus_.Stars.N", "Range: 0 … 255 — stars earned in the rally."),
    // --- arcade
    ("ArcadeStatus_.Round", "Range: 0 … 255 — arcade round."),
    ("ArcadeStatus_.RouteIndex", "Range: 0 … 255 — arcade route index."),
    ("ArcadeStatus_.Opened", "All possible values: false / true — arcade mode unlocked."),
    ("ArcadeStatus_.HasContinued", "All possible values: false / true — used a continue in arcade."),
    // --- stats
    ("StatsData_.RacesCompleted", "Range: 0 … 65,535 (uint16) — lifetime races completed."),
    ("StatsData_.Victories", "Range: 0 … 65,535 — lifetime victories."),
    ("StatsData_.ArcadeRaces", "Range: 0 … 65,535 — lifetime arcade races."),
    ("StatsData_.MoneyWon", "Range: 0 … 999,999,999 Cr — lifetime money won."),
    ("StatsData_.MoneySpent", "Range: 0 … 999,999,999 Cr — lifetime money spent."),
    ("StatsData_.TotalKms", "Range: 0 … 999,999.9 km (float) — lifetime distance."),
    ("StatsData_.ArcadeKms", "Range: 0 … 999,999.9 km (float) — lifetime arcade distance."),
    ("StatsData_.InvitationKms", "Range: 0 … 999,999.9 km (float) — lifetime invitational distance."),
    ("StatsData_.Invitationkey", "Range: 0 … 65,535 — invitational key code."),
    ("StatsData_.Invitationtries.N", "Range: 0 … 65,535 — invitational attempts (3 tries)."),
    ("StatsData_.AirTime", "Range: 0 … 999,999.9 s (float) — lifetime airtime."),
    ("StatsData_.Gametime", "Range: 0 … 4,294,967,295 s (float) — play time (duplicates the root Gametime)."),
    ("StatsData_.Breakdowns", "Range: 0 … 65,535 — breakdowns suffered."),
    ("StatsData_.OilChanges", "Range: 0 … 65,535 — oil changes done."),
    ("StatsData_.CarsCleaned", "Range: 0 … 65,535 — cars cleaned."),
    ("StatsData_.Streak", "Range: 0 … 65,535 — current win streak."),
    ("StatsData_.ArcadeRound", "Range: 0 … 65,535 — arcade round reached."),
    ("StatsData_.GhostViewMode", "Range: 0 … 2 — ghost view mode."),
    ("StatsData_.Vsync", "All possible values: 0 = off, 1 = on."),
    ("StatsData_.MasterVolume", "Range: -96.0 … 0.0 dB (float, negative = quieter)."),
    ("StatsData_.RaceMusic", "Range: -96.0 … 0.0 dB (float)."),
    ("StatsData_.MenuMusic", "Range: -96.0 … 0.0 dB (float)."),
    ("StatsData_.RaceSFX", "Range: -96.0 … 0.0 dB (float)."),
    ("StatsData_.MenuSFX", "Range: -96.0 … 0.0 dB (float)."),
    ("StatsData_.FixedMinimap", "All possible values: false / true — minimap fixed on the track."),
    ("StatsData_.ResizeHud", "All possible values: false / true — HUD resized."),
    ("StatsData_.Flare", "All possible values: false / true — lens flare effect."),
    ("StatsData_.FXAA", "All possible values: false / true — FXAA anti-aliasing."),
    ("StatsData_.SSAO", "All possible values: false / true — screen-space ambient occlusion."),
    ("StatsData_.MultiPlayerCollisions", "All possible values: false / true."),
    ("StatsData_.MenuCycleAnimation", "All possible values: false / true — animated menu backgrounds."),
    ("StatsData_.Tutorial_01", "All possible values: false / true — tutorial completed."),
    ("StatsData_.CustomMusic", "All possible values: false / true — custom music enabled."),
    ("StatsData_.Country", "Range: 0 … 255 — country code index; 21 = Russia in the default save."),
    ("StatsData_.Creditsrolled", "All possible values: false / true — end-credits rolled."),
    ("StatsData_.Unlocked.N", "All possible values: false / true — unlocked content flag (999 entries)."),
    ("StatsData_.AchievementsClear.N", "All possible values: false / true — achievement cleared (50 entries)."),
];

/// Hint for an arbitrary path (array indices are normalised to `N`).
pub fn field_hint(path: &str) -> Option<&'static str> {
    let norm = path
        .split('.')
        .map(|p| {
            if p.chars().all(|c| c.is_ascii_digit()) {
                "N"
            } else {
                p
            }
        })
        .collect::<Vec<_>>()
        .join(".");
    FIELD_HINTS
        .iter()
        .find(|(p, _)| *p == norm)
        .map(|(_, h)| *h)
        .or_else(|| COMMON_FIELDS.iter().find(|f| f.path == path).map(|f| f.hint))
}

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
///
/// Refuses to run while the game is running - the game overwrites the save
/// on exit, so writing then is a silent no-op.
pub fn patch_save(save_path: &Path, edits: &[(String, Json)]) -> Result<PathBuf, String> {
    if crate::tool::inject::find_game_pid().is_some() {
        return Err("the game is running - close it first, otherwise it will overwrite the save on exit".into());
    }
    patch_save_impl(save_path, edits)
}

/// Same as `patch_save` but without the "is the game running?" guard.
/// For tests only - callers must guarantee it is safe (the game is closed,
/// or the target file is a throwaway copy).
#[doc(hidden)]
pub fn patch_save_unchecked(save_path: &Path, edits: &[(String, Json)]) -> Result<PathBuf, String> {
    patch_save_impl(save_path, edits)
}

fn patch_save_impl(save_path: &Path, edits: &[(String, Json)]) -> Result<PathBuf, String> {
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
