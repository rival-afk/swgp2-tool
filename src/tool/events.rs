//! Live parser for the game's event database (level files).
//!
//! Super Woden GP 2 stores championships, weekend festival events, rally
//! stages, tracks, pilots and weather as opaque binary records inside the
//! serialized scenes (`level*`). Reverse-engineered layout (see
//! docs/GAME_FILES.md):
//!
//! ```text
//! ... record header bytes ...
//! 01 00 00 00 01 00 00 00 <K:u32>   <- anchor; K identifies the record type
//! <some ints/floats/ptrs>           <- 12-60 bytes
//! <len:u32><name bytes><NUL>        <- the displayed name
//! ... trailing record data ...
//! ```
//!
//! Observed K values:
//!   0x025a (602)  Weekend festival event (one-off races incl. endurance)
//!   0x10f  (271)  Track / venue (circuits, ovals, rally stages)
//!   0x106  (262)  Pilot (championship/round surnames used in UI)
//!   0x056  ( 86)  Cup/trophy championship event
//!   0x035  ( 53)  Class/dealer restriction tag ("RACE,GR.5", "NANWOLF", ...)
//!   0x211  (529)  Weather ("CLEAR", "RAIN", "SNOW", "STORM", "FOG")
//!   0x120  (288)  Key binding name ("LeftArrow", ...)
//!   0x03a  ( 58)  URL
//!
//! Restriction records (K=53) sit adjacent to the event record they belong
//! to: tags/dealer/model/weather/pilot for an event are found by scanning a
//! window *before* it. Every menu appears twice in level0 (two copies of the
//! serialized menu), so results are deduped by name.
//!
//! Everything here is best-effort heuristics over undocumented binary data.
//! If the game updates its format, scans yield nothing and the wiki falls
//! back to the curated lists.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const ANCHOR: &[u8] = b"\x01\x00\x00\x00\x01\x00\x00\x00";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordClass {
    Weekend,
    Cup,
    Track,
    Pilot,
    Tag,
    Weather,
    KeyName,
    Url,
    Header,
    Unknown,
}

impl RecordClass {
    fn from_k(k: u32) -> RecordClass {
        match k {
            602 => RecordClass::Weekend,
            271 => RecordClass::Track,
            262 => RecordClass::Pilot,
            86 => RecordClass::Cup,
            53 => RecordClass::Tag,
            529 => RecordClass::Weather,
            288 => RecordClass::KeyName,
            58 => RecordClass::Url,
            212 => RecordClass::Header,
            _ => RecordClass::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    /// Byte offset of the record anchor.
    pub offset: u64,
    /// Raw K value from the anchor.
    pub kind: u32,
    pub class: RecordClass,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameEvent {
    /// Display name, e.g. "Muscle Car Weekend".
    pub name: String,
    pub kind: EventKind,
    /// Restriction tag, e.g. "RACE,GR.5" (may be empty = no restriction).
    pub tags: Vec<String>,
    /// Restriction by dealership, e.g. "NANWOLF".
    pub dealer: Option<String>,
    /// Restriction by exact model, e.g. "ROUGHTON GR.5".
    pub model: Option<String>,
    /// Weather entry ("CLEAR", "RAIN", "SNOW", "STORM", "FOG"), unverifiable but parsed.
    pub weather: Option<String>,
    /// Pilot assigned to the round (from the adjacent pilot record).
    pub pilot: Option<String>,
    /// Class category shown after the name ("MUSCLECAR", "NASCAR", "SMALL", …).
    pub class: Option<String>,
    /// Offset of the record in level0.
    pub offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Weekend,
    Cup,
    Track,
}

impl EventKind {
    pub fn label(self) -> &'static str {
        match self {
            EventKind::Weekend => "Weekend",
            EventKind::Cup => "Cup",
            EventKind::Track => "Track",
        }
    }
}

/// One length-prefixed UTF-8 string: `u32 len` + bytes. No trailing NUL is
/// required (records repeat the length instead of nulling the string).
fn lp_string_at(data: &[u8], pos: usize) -> Option<(String, usize)> {
    if pos + 4 > data.len() {
        return None;
    }
    let len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
    if len == 0 || len > 80 {
        return None;
    }
    if pos + 4 + len > data.len() {
        return None;
    }
    let end = pos + 4 + len;
    let raw = &data[pos + 4..end];
    if !raw.iter().all(|&b| b >= 0x20 || b == 0x09) {
        return None;
    }
    let s = String::from_utf8(raw.to_vec()).ok()?.trim_end().to_string();
    // plausibility: starts with alphanumeric, holds at least 2 letters
    if !s.chars().next()?.is_ascii_alphanumeric() {
        return None;
    }
    if s.chars().filter(|c| c.is_ascii_alphabetic()).count() < 2 {
        return None;
    }
    Some((s, end))
}

/// Find the first plausible record name within `delta` bytes after the anchor.
fn record_name(data: &[u8], anchor: usize, delta_max: usize) -> Option<(String, usize)> {
    for delta in [12usize, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 72, 80] {
        if delta > delta_max {
            break;
        }
        if let Some((s, _)) = lp_string_at(data, anchor + delta) {
            return Some((s, anchor + delta));
        }
    }
    None
}

/// Scan a level file for name records. Both menu copies are present in the
/// file; results are deduped by name (known-kind records are preferred over
/// zero-kind duplicates of the same name).
pub fn scan_records(path: &Path) -> std::io::Result<Vec<GameRecord>> {
    let data = std::fs::read(path)?;
    let mut by_name: HashMap<String, GameRecord> = HashMap::new();
    let mut i = 0usize;
    while i + ANCHOR.len() + 8 <= data.len() {
        if data[i..i + ANCHOR.len()] == *ANCHOR {
            let k = u32::from_le_bytes([data[i + 8], data[i + 9], data[i + 10], data[i + 11]]);
            if let Some((name, _)) = record_name(&data, i, 80) {
                let class = RecordClass::from_k(k);
                match by_name.entry(name.clone()) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        if e.get().class == RecordClass::Unknown && class != RecordClass::Unknown {
                            e.insert(GameRecord {
                                offset: i as u64,
                                kind: k,
                                class,
                                name,
                            });
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(GameRecord {
                            offset: i as u64,
                            kind: k,
                            class,
                            name,
                        });
                    }
                }
            }
            i += ANCHOR.len();
        } else {
            i += 1;
        }
    }
    let mut out: Vec<GameRecord> = by_name.into_values().collect();
    out.sort_by_key(|r| r.offset);
    Ok(out)
}

/// Raw length-prefixed string scan (restrictions/weather/pilots have no record
/// anchor of their own). Returns (offset, name) for every plausible string.
pub fn scan_lp_strings(data: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 < data.len() {
        if let Some((s, end)) = lp_string_at(data, i) {
            out.push((i, s));
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

fn norm_key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

const DEALERS: [&str; 10] = [
    "AALIA", "CINDER", "DECASTRO", "ENHORNING", "MYLENE", "NANWOLF", "RAVEN", "SOOP", "VIJUDA", "ZAPHIR",
];
const WEATHERS: [&str; 5] = ["CLEAR", "RAIN", "SNOW", "STORM", "FOG"];
/// UI-noise strings that are class/tag-shaped but mean nothing.
const NOISE: [&str; 8] = [
    "MENU", "LOADING", "DEBUG", "STARTSFX", "MOVESFX", "COINSFX", "RACE", "CLASSTXT",
];
/// All 25 pilots from StreamingAssets/_pilot_names.txt (names only).
const PILOT_NAMES: [&str; 25] = [
    "CAITLYN GLOVER", "THOMAS FLORES", "BEN CHALCRAFT", "SUSAN MCREADY", "LUCIANO DI MOZE",
    "HATTIE ANDERSON", "OLIVER MARS", "NIAS BERNHARD", "LEANDRO VERGARA", "EDUARDO CAPITÁN",
    "YUN HUANG", "HARRISON HIGGINS", "JUSTIN DAVIES", "ERIN GUINESS", "DIANA PROAÑO",
    "PHILIPP MANGENG", "JOOP VLAANDEREN", "MASATOSHI WATANABE", "ANDRÉS PAMPILLON", "VICTOR JUSTO",
    "JAVI BONUS", "BÖRJE FORNBERG", "JEAN-BAPTISTE DUJARDIN", "ALEKSIS PIRHONEN", "KANEKO SHINJU",
];

/// The 200 shop cars, keyed normalized model -> catalog model name.
fn catalog_model_keys() -> Vec<(String, &'static str)> {
    crate::tool::catalog::all_cars()
        .into_iter()
        .map(|c| (norm_key(c.model), c.model))
        .collect()
}

fn is_tag(name: &str) -> bool {
    name.contains(',')
        && !name.contains(' ')
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == ',' || c == '.' || c == '&')
}

/// Class/category token that may follow an event record ("MUSCLECAR", "SMALL", "NASCAR", "OVAL"...).
fn is_class_like(name: &str) -> bool {
    !name.contains(' ')
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '&')
        && name.len() >= 3
}

/// Build structured events from a level file. Meant for level0 (the menu).
pub fn scan_events(path: &Path) -> std::io::Result<Vec<GameEvent>> {
    let data = std::fs::read(path)?;
    let records = scan_records(path)?;
    let lp = scan_lp_strings(&data);

    // events: K=602 (weekend), 86 (cup), 271 (track); records with an unknown
    // kind slot still carry valid event names (never class/pilot/model shaped)
    let models = catalog_model_keys();
    let model_keys: std::collections::HashSet<String> = models.iter().map(|(k, _)| k.clone()).collect();
    let pilot_keys: std::collections::HashSet<String> = PILOT_NAMES.iter().map(|p| norm_key(p)).collect();

    fn looks_like_event_name(name: &str) -> bool {
        const SUFFIXES: [&str; 12] = [
            "CUP", "TROPHY", "SERIES", "CHALLENGE", "MEETING", "FESTIVAL", "FEST", "SHOWDOWN", "RALLY", "RACE",
            "DANCE", "CLASSIC",
        ];
        if name.contains('.') {
            return false; // file-like name, e.g. "level0.resS"
        }
        let up = name.to_ascii_uppercase();
        if up.split_whitespace().count() < 2 {
            return false;
        }
        if SUFFIXES.iter().any(|s| up.ends_with(s)) {
            return true;
        }
        // endurance-style ending, e.g. "… 12H"
        let bytes = up.as_bytes();
        bytes.len() >= 2
            && bytes[bytes.len() - 1] == b'H'
            && bytes[..bytes.len() - 1].iter().next_back().is_some_and(|b| b.is_ascii_digit())
    }

    let events: Vec<&GameRecord> = records
        .iter()
        .filter(|r| {
            if matches!(r.class, RecordClass::Weekend | RecordClass::Cup | RecordClass::Track) {
                return !is_tag(&r.name);
            }
            // rescue: unknown kind slot, but the name is clearly an event
            if r.class == RecordClass::Unknown
                && looks_like_event_name(&r.name)
                && !is_tag(&r.name)
                && !NOISE.contains(&r.name.as_str())
                && !DEALERS.contains(&r.name.as_str())
                && !model_keys.contains(&norm_key(&r.name))
                && !pilot_keys.contains(&norm_key(&r.name))
            {
                return true;
            }
            false
        })
        .collect();

    let mut out: Vec<GameEvent> = Vec::with_capacity(events.len());

    for ev in events {
        // restriction context: scan length-prefixed strings in a window BEFORE the event
        let mut tags = Vec::new();
        let mut dealer = None;
        let mut model = None;
        let mut weather = None;
        let mut pilot = None;
        let mut class = None;

        let ev_off = ev.offset as usize;
        let win_lo = ev_off.saturating_sub(600);
        let win_hi = ev_off + 300;

        let start = lp.partition_point(|(off, _)| *off < win_lo);
        let end = lp.partition_point(|(off, _)| *off <= win_hi);

        // nearest block wins: iterate the window backwards from the event
        for &(off, ref name) in lp[start..end].iter().rev() {
            if off >= ev_off {
                continue;
            }
            let nk = norm_key(name);
            if is_tag(name) && !NOISE.contains(&name.as_str()) {
                if tags.is_empty() {
                    tags.push(name.to_string());
                }
            } else if DEALERS.contains(&name.as_str()) {
                if dealer.is_none() {
                    dealer = Some(name.to_string());
                }
            } else if pilot_keys.contains(&nk) {
                if pilot.is_none() {
                    pilot = Some(name.to_string());
                }
            } else if WEATHERS.contains(&name.as_str()) {
                if weather.is_none() {
                    weather = Some(name.to_string());
                }
            } else if models.iter().any(|(k, _)| *k == nk) {
                // exact shop-car restriction, e.g. "ROUGHTON GR.5"
                if model.is_none() {
                    model = Some(name.to_string());
                }
            } else if is_class_like(name) && tag_looks_like_class(name) && !NOISE.contains(&name.as_str()) {
                // single uppercase class word, e.g. CONCEPT
                if !tags.iter().any(|t| t == name) {
                    tags.push(name.to_string());
                }
            }
        }

        // class/category: the first class-like word AFTER the event name
        for &(off, ref name) in &lp[start..end] {
            if off <= ev_off {
                continue;
            }
            let nk = norm_key(name);
            if is_class_like(name)
                && !NOISE.contains(&name.as_str())
                && !DEALERS.contains(&name.as_str())
                && !models.iter().any(|(k, _)| *k == nk)
                && !pilot_keys.contains(&nk)
                && !WEATHERS.contains(&name.as_str())
                && !is_tag(name)
            {
                class = Some(name.clone());
                break;
            }
        }

        out.push(GameEvent {
            name: ev.name.clone(),
            kind: match ev.class {
                RecordClass::Weekend => EventKind::Weekend,
                RecordClass::Cup => EventKind::Cup,
                RecordClass::Track => EventKind::Track,
                _ => {
                    if is_endurance(&ev.name) {
                        EventKind::Weekend
                    } else {
                        EventKind::Cup
                    }
                }
            },
            tags,
            dealer,
            model,
            weather,
            pilot,
            class,
            offset: ev.offset,
        });
    }

    out.sort_by_key(|e| (kind_rank(e.kind), e.offset));
    Ok(out)
}

fn kind_rank(k: EventKind) -> u8 {
    match k {
        EventKind::Weekend => 0,
        EventKind::Cup => 1,
        EventKind::Track => 2,
    }
}

fn tag_looks_like_class(name: &str) -> bool {
    // single uppercase word like CONCEPT, TRUCK, RALLY, VAN
    !name.contains(' ') && name.chars().all(|c| c.is_ascii_uppercase() || c == '&') && name.len() >= 2
}

/// Path of the level file that holds the menu (level0 in the game's Data folder).
pub fn default_level_path(game_dir: &Path) -> PathBuf {
    game_dir.join("Super Woden GP 2_Data").join("level0")
}

/// Endurance race? (24H / 12H / 8H / 6H style stage or race names)
pub fn is_endurance(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper
        .split(|c: char| c.is_whitespace() || c == '(' || c == '-' || c == '/')
        .any(|tok| {
            let digits: String = tok.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.len() >= 1 && tok.ends_with('H') && tok[digits.len()..] == *"H"
        })
}

/// Rally stage? (name ends with "S<n>" e.g. "Sweden S2", "Kenya S1"; also Hillclimb)
pub fn is_rally_stage(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if upper.contains("HILLCLIMB") {
        return true;
    }
    let mut tokens = upper.split_whitespace();
    match tokens.next_back() {
        Some(last) => {
            last.len() >= 2
                && last.starts_with('S')
                && last[1..].chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// Sort helpers used by the UI
impl GameEvent {
    pub fn is_endurance(&self) -> bool {
        is_endurance(&self.name)
    }
    pub fn is_rally_stage(&self) -> bool {
        is_rally_stage(&self.name)
    }
    pub fn restriction_text(&self) -> String {
        let mut parts = Vec::new();
        if !self.tags.is_empty() {
            parts.push(self.tags.join(" + "));
        }
        if let Some(d) = &self.dealer {
            parts.push(format!("dealer {d}"));
        }
        if let Some(m) = &self.model {
            parts.push(format!("model {m}"));
        }
        if let Some(p) = &self.pilot {
            parts.push(format!("pilot {p}"));
        }
        if let Some(w) = &self.weather {
            parts.push(format!("weather {w}"));
        }
        if parts.is_empty() {
            "no restriction".to_string()
        } else {
            parts.join(" - ")
        }
    }
}

/// Split a tag like "RACE,GR.5" into class tokens.
pub fn tag_tokens(tag: &str) -> Vec<&str> {
    tag.split(',').map(str::trim).filter(|t| !t.is_empty()).collect()
}

/// Does a car model match this restriction/class token?
pub fn model_matches_token(model: &str, token: &str) -> bool {
    let m = norm_key(model);
    let t = norm_key(token);
    if t.is_empty() {
        return false;
    }
    // GR.5 / GR.A etc. are class suffixes, compare case-insensitively
    m.contains(&t)
}

/// Class-like tokens we understand for the "best cars" filter.
const KNOWN_CLASS_TOKENS: [&str; 8] = ["GR.5", "GR.A", "DTC", "BTC", "JTC", "F1", "F3", "TRC"];

/// Order events by kind then endurance, then name.
pub fn sort_events(events: &mut [GameEvent]) {
    events.sort_by(|a, b| {
        kind_rank(a.kind)
            .cmp(&kind_rank(b.kind))
            .then(b.is_endurance().cmp(&a.is_endurance()))
            .then(a.name.cmp(&b.name))
    });
}

/// Ordering for the events list UI (stable grouping used there).
pub fn compare_events(a: &GameEvent, b: &GameEvent) -> Ordering {
    kind_rank(a.kind)
        .cmp(&kind_rank(b.kind))
        .then(a.offset.cmp(&b.offset))
}

/// Convenience for the UI: events grouped by (Weekend / Cup / Track).
pub fn grouped(events: &[GameEvent]) -> (Vec<&GameEvent>, Vec<&GameEvent>, Vec<&GameEvent>) {
    let wk = events.iter().filter(|e| e.kind == EventKind::Weekend).collect();
    let cup = events.iter().filter(|e| e.kind == EventKind::Cup).collect();
    let trk = events.iter().filter(|e| e.kind == EventKind::Track).collect();
    (wk, cup, trk)
}

/// All class tokens present in an event (tags + trailing class word) — used
/// for the best-cars filter.
pub fn class_tokens(event: &GameEvent) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |t: &str| {
        let up = t.to_ascii_uppercase();
        if KNOWN_CLASS_TOKENS.iter().any(|k| k.eq_ignore_ascii_case(&up)) && !out.iter().any(|x| x == &up) {
            out.push(up);
        }
    };
    for tag in &event.tags {
        for token in tag_tokens(tag) {
            push(token);
        }
    }
    if let Some(c) = &event.class {
        push(c);
    }
    out
}

/// Map class token -> catalog car matches (used by the wiki best-cars algorithm).
pub fn cars_matching_class<'a>(
    cars: &'a [crate::tool::catalog::CatalogCar],
    token: &str,
) -> Vec<&'a crate::tool::catalog::CatalogCar> {
    cars.iter()
        .filter(|c| model_matches_token(c.model, token))
        .collect()
}

/// Best cars for an event: apply restriction filters, then rank by CV desc,
/// then top-speed desc. Unknown stats sort last (treated as 0).
/// `rally_only` restricts the pool to rally cars (for rally stages).
pub fn best_cars_for_event<'a>(
    event: &GameEvent,
    cars: &'a [crate::tool::catalog::CatalogCar],
    n: usize,
    rally_only: bool,
) -> Vec<&'a crate::tool::catalog::CatalogCar> {
    let mut v: Vec<&crate::tool::catalog::CatalogCar> = cars
        .iter()
        .filter(|c| {
            if rally_only && !c.rally {
                return false;
            }
            if let Some(d) = &event.dealer {
                if !c.dealer.eq_ignore_ascii_case(d) {
                    return false;
                }
            }
            if let Some(m) = &event.model {
                if norm_key(c.model) != norm_key(m) {
                    return false;
                }
            }
            let classes = class_tokens(event);
            if !classes.is_empty() && !classes.iter().any(|t| model_matches_token(c.model, t)) {
                return false;
            }
            true
        })
        .collect();
    v.sort_by(|a, b| {
        b.cv
            .unwrap_or(0)
            .cmp(&a.cv.unwrap_or(0))
            .then(b.top_speed.unwrap_or(0).cmp(&a.top_speed.unwrap_or(0)))
    });
    v.truncate(n);
    v
}

/// Cache-friendly: collect events once per call; callers should cache if reused.
pub fn load_for_game(game_dir: &Path) -> std::io::Result<Vec<GameEvent>> {
    let level = default_level_path(game_dir);
    let mut events = scan_events(&level)?;
    sort_events(&mut events);
    Ok(events)
}

/// Map a game directory to wall-clock file mtime (secs) for caching purposes.
pub fn level_stamp(game_dir: &Path) -> Option<u64> {
    let level = default_level_path(game_dir);
    std::fs::metadata(level)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Count how many raw records were found (for diagnostics/MCP).
pub fn record_stats(path: &Path) -> std::io::Result<(usize, usize)> {
    let records = scan_records(path)?;
    let total = records.len();
    let named = records.iter().filter(|r| !r.name.is_empty()).count();
    Ok((total, named))
}
