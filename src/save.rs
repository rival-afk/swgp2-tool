//! Save-file handling: decrypt/encrypt, backups, JSON dump/apply, field edits.

use crate::crypto;
use crate::nrbf::{self, Additional, Graph, Node, Prim, Value};
use serde_json::{Map, Number, Value as Json};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const GAME_DIR_REL: &str = r"AppData\LocalLow\ViJuDa\Super Woden GP 2";

#[derive(Debug)]
pub struct SaveError(pub String);

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for SaveError {}
impl From<io::Error> for SaveError {
    fn from(e: io::Error) -> Self {
        SaveError(e.to_string())
    }
}
impl From<nrbf::NrbfError> for SaveError {
    fn from(e: nrbf::NrbfError) -> Self {
        SaveError(e.to_string())
    }
}
type Result<T> = std::result::Result<T, SaveError>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(SaveError(msg.into()))
}

pub fn default_save_path(slot: u32) -> Option<PathBuf> {
    let base = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))?;
    Some(
        PathBuf::from(base)
            .join(GAME_DIR_REL)
            .join("Save")
            .join(format!("Supersave{slot}.sav")),
    )
}

pub fn list_slots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(base) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else {
        return out;
    };
    let dir = PathBuf::from(base).join(GAME_DIR_REL).join("Save");
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|e| e == "sav").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------------------
// Backups
// ---------------------------------------------------------------------------

pub fn backup_dir_for(save_path: &Path) -> PathBuf {
    save_path.parent().unwrap_or(Path::new(".")).join("Backups")
}

pub fn backup_path(save_path: &Path) -> Result<PathBuf> {
    let dir = backup_dir_for(save_path);
    fs::create_dir_all(&dir)?;
    let stem = save_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "save".into());
    let stamp = chrono_stamp();
    let mut candidate = dir.join(format!("{stem}.{stamp}.savbak"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem}.{stamp}.{n}.savbak"));
        n += 1;
    }
    Ok(candidate)
}

pub fn chrono_stamp() -> String {
    // Avoid an external time crate; compute from system time.
        let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let (h, mi, s) = {
        let rem = secs % 86400;
        (rem / 3600, (rem % 3600) / 60, rem % 60)
    };
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y + if m <= 2 { 1 } else { 0 }, m, d, h, mi, s)
}

pub fn create_backup(save_path: &Path) -> Result<PathBuf> {
    let dst = backup_path(save_path)?;
    fs::copy(save_path, &dst)?;
    Ok(dst)
}

pub fn list_backups(save_path: &Path) -> Result<Vec<PathBuf>> {
    let dir = backup_dir_for(save_path);
    let mut out = Vec::new();
    if dir.exists() {
        for e in fs::read_dir(&dir)? {
            let p = e?.path();
            if p.extension().map(|e| e == "savbak").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

// ---------------------------------------------------------------------------
// Load / store
// ---------------------------------------------------------------------------

pub fn load(save_path: &Path) -> Result<(Graph, Vec<u8>)> {
    let raw = fs::read(save_path)?;
    let plain = crypto::decrypt_save(&raw).map_err(SaveError)?;
    let g = nrbf::parse(&plain)?;
    Ok((g, raw))
}

/// Parse -> re-serialize -> encrypt. Used for all writes.
pub fn store(save_path: &Path, g: &Graph) -> Result<()> {
    let bytes = nrbf::Writer::new().write(g)?;
    let enc = crypto::encrypt_save(&bytes);
    fs::write(save_path, enc)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// JSON dump
// ---------------------------------------------------------------------------

fn prim_to_json(p: &Prim) -> Json {
    match p {
        Prim::Boolean(v) => Json::Bool(*v),
        Prim::Byte(v) => Json::Number(Number::from(*v)),
        Prim::SByte(v) => Json::Number(Number::from(*v as i64)),
        Prim::Int16(v) => Json::Number(Number::from(*v)),
        Prim::UInt16(v) => Json::Number(Number::from(*v)),
        Prim::Int32(v) => Json::Number(Number::from(*v)),
        Prim::UInt32(v) => Json::Number(Number::from(*v)),
        Prim::Int64(v) => Json::Number(Number::from(*v)),
        Prim::UInt64(v) => Json::Number(Number::from(*v)),
        Prim::TimeSpan(v) => Json::Number(Number::from(*v)),
        Prim::DateTime(v) => Json::Number(Number::from(*v)),
        Prim::Single(v) => Json::Number(Number::from_f64(*v as f64).unwrap_or(Number::from(0))),
        Prim::Double(v) => Json::Number(Number::from_f64(*v).unwrap_or(Number::from(0))),
        Prim::Char(v) | Prim::Decimal(v) => Json::String(v.clone()),
        Prim::Null => Json::Null,
    }
}

fn value_to_json(g: &Graph, v: &Value, depth: usize) -> Result<Json> {
    if depth > 256 {
        return err("dump exceeded maximum nesting depth");
    }
    match v {
        Value::Null => Ok(Json::Null),
        Value::Prim(p) => Ok(prim_to_json(p)),
        Value::Ref(idx) => node_to_json(g, *idx, depth + 1),
    }
}

fn is_list_class(node: &Node) -> bool {
    matches!(node, Node::Class { name, .. } if name.starts_with("System.Collections.Generic.List`1"))
}

fn list_member_index(g: &Graph, idx: usize, key: &str) -> Option<usize> {
    match &g.arena[idx] {
        Node::Class { member_names, .. } => member_names.iter().position(|n| n == key),
        _ => None,
    }
}

/// For List<T> nodes, transparently advance to the underlying _items array so
/// paths like `GarageSlots.0.CarID` work as expected.
fn effective_node(g: &Graph, idx: usize) -> usize {
    if is_list_class(&g.arena[idx]) {
        if let Some(mi) = list_member_index(g, idx, "_items") {
            if let Node::Class { members, .. } = &g.arena[idx] {
                if let Value::Ref(j) = &members[mi] {
                    return *j;
                }
            }
        }
    }
    idx
}

fn node_to_json(g: &Graph, idx: usize, depth: usize) -> Result<Json> {
    match &g.arena[idx] {
        Node::Str(s) => Ok(Json::String(s.clone())),
        Node::Class { member_names, members, .. } => {
            if is_list_class(&g.arena[idx]) {
                // Unwrap List<T> into a plain JSON array, honoring _size
                // (BinaryFormatter keeps the raw backing array, which may be
                // longer than the logical list).
                if let Some(si) = list_member_index(g, idx, "_size") {
                    let sz = match &members[si] {
                        Value::Prim(Prim::Int32(sz)) => (*sz).max(0) as usize,
                        _ => usize::MAX,
                    };
                    for (name, v) in member_names.iter().zip(members) {
                        if name == "_items" {
                            if let Value::Ref(j) = v {
                                let j = *j;
                                return list_items_to_json(g, j, sz, depth);
                            }
                            return Ok(Json::Null);
                        }
                    }
                }
                for (name, v) in member_names.iter().zip(members) {
                    if name == "_items" {
                        return value_to_json(g, v, depth);
                    }
                }
                return Ok(Json::Null);
            }
            let mut m = Map::with_capacity(member_names.len());
            for (name, v) in member_names.iter().zip(members) {
                m.insert(name.clone(), value_to_json(g, v, depth)?);
            }
            Ok(Json::Object(m))
        }
        Node::ArraySinglePrimitive { items, .. } => {
            Ok(Json::Array(items.iter().map(prim_to_json).collect()))
        }
        Node::ArraySingleString { items } | Node::ArraySingleObject { items } => {
            Ok(Json::Array(
                items.iter().map(|v| value_to_json(g, v, depth + 1)).collect::<Result<_>>()?,
            ))
        }
        Node::BinaryArray { items, .. } => {
            Ok(Json::Array(
                items.iter().map(|v| value_to_json(g, v, depth + 1)).collect::<Result<_>>()?,
            ))
        }
    }
}

pub fn dump(g: &Graph) -> Result<Json> {
    node_to_json(g, g.root, 0)
}

// ---------------------------------------------------------------------------
// JSON apply (merge a JSON document into the parsed graph in place)
// ---------------------------------------------------------------------------

fn json_to_prim(j: &Json, ptype: u8) -> Result<Prim> {
    let as_i64 = |j: &Json| j.as_i64();
    let range = |v: i64, lo: i64, hi: i64, what: &str| -> Result<i64> {
        if (lo..=hi).contains(&v) {
            Ok(v)
        } else {
            err(format!("value {v} out of range for {what}"))
        }
    };
    Ok(match ptype {
        1 => Prim::Boolean(j.as_bool().ok_or_else(|| SaveError("expected true/false".into()))?),
        2 => Prim::Byte(range(as_i64(j).ok_or_else(|| SaveError("expected integer".into()))?, 0, u8::MAX as i64, "Byte")? as u8),
        3 => {
            let s = j.as_str().ok_or_else(|| SaveError("expected 1-character string".into()))?;
            if s.chars().count() != 1 {
                return err("Char must be a single character");
            }
            Prim::Char(s.to_string())
        }
        5 => {
            let s = match j {
                Json::String(s) => s.clone(),
                Json::Number(n) => n.to_string(),
                _ => return err("Decimal expects a string or number"),
            };
            Prim::Decimal(s)
        }
        6 => Prim::Double(j.as_f64().ok_or_else(|| SaveError("expected number".into()))?),
        7 => Prim::Int16(range(as_i64(j).ok_or_else(|| SaveError("expected integer".into()))?, i16::MIN as i64, i16::MAX as i64, "Int16")? as i16),
        8 => Prim::Int32(range(as_i64(j).ok_or_else(|| SaveError("expected integer".into()))?, i32::MIN as i64, i32::MAX as i64, "Int32")? as i32),
        9 | 12 => Prim::Int64(as_i64(j).ok_or_else(|| SaveError("expected integer".into()))?),
        10 => Prim::SByte(range(as_i64(j).ok_or_else(|| SaveError("expected integer".into()))?, i8::MIN as i64, i8::MAX as i64, "SByte")? as i8),
        11 => Prim::Single(j.as_f64().ok_or_else(|| SaveError("expected number".into()))? as f32),
        13 | 16 => Prim::UInt64({
            let v = j.as_u64().or_else(|| j.as_i64().map(|v| v as u64)).ok_or_else(|| SaveError("expected unsigned integer".into()))?;
            if ptype == 13 {
                return Ok(Prim::DateTime(v));
            }
            v
        }),
        14 => Prim::UInt16({
            let v = as_i64(j).ok_or_else(|| SaveError("expected integer".into()))?;
            range(v, 0, u16::MAX as i64, "UInt16")? as u16
        }),
        15 => Prim::UInt32({
            let v = as_i64(j).ok_or_else(|| SaveError("expected integer".into()))?;
            range(v, 0, u32::MAX as i64, "UInt32")? as u32
        }),
        17 => Prim::Null,
        other => return err(format!("unsupported primitive type {other}")),
    })
}

#[allow(dead_code)] // kept for future informational output
pub fn member_type_name(bt: u8) -> &'static str {
    match bt {
        0 => "primitive",
        1 => "string",
        2 => "object",
        3 => "system-class",
        4 => "class",
        5 => "object-array",
        6 => "string-array",
        7 => "primitive-array",
        _ => "unknown",
    }
}

fn find_member(g: &Graph, idx: usize, key: &str) -> Result<usize> {
    match &g.arena[idx] {
        Node::Class { member_names, .. } => {
            if let Some(i) = member_names.iter().position(|n| n == key) {
                return Ok(i);
            }
            let lower = key.to_lowercase();
            if let Some(i) = member_names.iter().position(|n| n.to_lowercase() == lower) {
                return Ok(i);
            }
            err(format!("member '{key}' not found (available: {})", member_names.join(", ")))
        }
        _ => err("path segment does not select a class member"),
    }
}

fn merge_member_value(g: &mut Graph, binary_type: u8, additional: &Additional, member: &mut Value, json: &Json) -> Result<()> {
    if binary_type == 0 {
        if json.is_null() {
            return err("cannot set a primitive member to null");
        }
        let Additional::Prim(ptype) = additional else {
            return err("primitive member has no primitive type metadata");
        };
        *member = Value::Prim(json_to_prim(json, *ptype)?);
        return Ok(());
    }
    match json {
        Json::Null => {
            *member = Value::Null;
            Ok(())
        }
        Json::String(_) => match member {
            Value::Ref(j) => {
                let Node::Str(_) = &g.arena[*j] else {
                    return err("string JSON does not match the member's object reference type");
                };
                merge_node(g, *j, json)
            }
            Value::Null => {
                // Materialize a new string object for this member.
                let idx = g.arena.len();
                g.arena.push(Node::Str(String::new()));
                merge_node(g, idx, json)?;
                *member = Value::Ref(idx);
                Ok(())
            }
            Value::Prim(_) => err("member holds a primitive, JSON string does not fit"),
        },
        Json::Object(_) | Json::Array(_) => match member {
            Value::Ref(j) => merge_node(g, *j, json),
            Value::Null => err("member is null in the save file; cannot materialize a new object of unknown class"),
            Value::Prim(_) => err("member holds a primitive"),
        },
        Json::Bool(_) | Json::Number(_) => err("JSON scalar does not fit this member (expected string/object/array or null)"),
    }
}

fn list_items_to_json(g: &Graph, items_idx: usize, size: usize, depth: usize) -> Result<Json> {
    match &g.arena[items_idx] {
        Node::ArraySinglePrimitive { items, .. } => {
            Ok(Json::Array(
                items.iter().take(size).map(prim_to_json).collect(),
            ))
        }
        Node::ArraySingleString { items } | Node::ArraySingleObject { items } => {
            Ok(Json::Array(
                items.iter()
                    .take(size)
                    .map(|v| value_to_json(g, v, depth + 1))
                    .collect::<Result<_>>()?,
            ))
        }
        Node::BinaryArray { items, .. } => {
            Ok(Json::Array(
                items.iter()
                    .take(size)
                    .map(|v| value_to_json(g, v, depth + 1))
                    .collect::<Result<_>>()?,
            ))
        }
        _ => err("List<T>._items is not an array"),
    }
}

fn merge_node(g: &mut Graph, idx: usize, json: &Json) -> Result<()> {
    // List<T> unwrap/wrap: plain JSON arrays map to the _items member.
    if json.is_array() && is_list_class(&g.arena[idx]) {
        let js_len = json.as_array().unwrap().len();
        let items_mi = list_member_index(g, idx, "_items").ok_or_else(|| SaveError("List<T> without _items member".into()))?;
        let items_ref = match &g.arena[idx] {
            Node::Class { members, .. } => match &members[items_mi] {
                Value::Ref(j) => *j,
                _ => return err("List<T>._items is not an object reference"),
            },
            _ => return err("List<T> node is not a class"),
        };
        // Resize the backing array to exactly the JSON array length before
        // merging element-wise (BinaryFormatter tolerates _items.len() >= _size).
        match &mut g.arena[items_ref] {
            Node::ArraySinglePrimitive { items, .. } => items.resize(js_len, Prim::Int32(0)),
            Node::ArraySingleString { items } | Node::ArraySingleObject { items } => {
                items.resize(js_len, Value::Null)
            }
            Node::BinaryArray { items, bt, .. } => {
                if *bt == 0 {
                    items.resize(js_len, Value::Prim(Prim::Int32(0)))
                } else {
                    items.resize(js_len, Value::Null)
                }
            }
            _ => return err("List<T>._items is not an array"),
        }
        merge_node(g, items_ref, json)?;
        // Keep _size in sync with the actual array length.
        if let Some(size_mi) = list_member_index(g, idx, "_size") {
            if let Node::Class { members, .. } = &mut g.arena[idx] {
                if let Value::Prim(p) = &mut members[size_mi] {
                    *p = Prim::Int32(js_len as i32);
                }
            }
        }
        return Ok(());
    }
    match json {
        Json::Null => return err("use null only for member values, not for nested nodes"),
        Json::String(_) => match &mut g.arena[idx] {
            Node::Str(s) => {
                if let Json::String(v) = json {
                    *s = v.clone();
                }
                Ok(())
            }
            _ => err("JSON string does not match the node type in the save file"),
        },
        Json::Object(map) => match &g.arena[idx] {
            Node::Class { .. } => {
                let mut pairs = Vec::with_capacity(map.len());
                for (k, v) in map {
                    let mi = find_member(g, idx, k)?;
                    pairs.push((mi, v.clone()));
                }
                for (mi, v) in pairs {
                    let (binary_type, additional) = match &g.arena[idx] {
                        Node::Class { binary_types, additional, .. } => (binary_types[mi], additional[mi].clone()),
                        _ => unreachable!(),
                    };
                    let member_opt = match &mut g.arena[idx] {
                        Node::Class { members, .. } => members.get_mut(mi),
                        _ => unreachable!(),
                    };
                    let member = member_opt.unwrap();
                    // Detach member value so we can re-assign after borrow ends
                    let mut member_value = std::mem::replace(member, Value::Null);
                    merge_member_value(g, binary_type, &additional, &mut member_value, &v)?;
                    let member = match &mut g.arena[idx] {
                        Node::Class { members, .. } => members.get_mut(mi).unwrap(),
                        _ => unreachable!(),
                    };
                    *member = member_value;
                }
                Ok(())
            }
            _ => err("JSON object does not match the node type in the save file"),
        },
        Json::Array(js_items) => match &mut g.arena[idx] {
            Node::ArraySinglePrimitive { ptype, items: dst } => {
                if dst.len() != js_items.len() {
                    return err(format!("array length mismatch: save has {}, JSON has {}", dst.len(), js_items.len()));
                }
                let mut new_items = Vec::with_capacity(dst.len());
                for el in js_items {
                    if el.is_null() {
                        return err("primitive array elements cannot be null");
                    }
                    new_items.push(json_to_prim(el, *ptype)?);
                }
                *dst = new_items;
                Ok(())
            }
            Node::ArraySingleString { items: dst } | Node::ArraySingleObject { items: dst } => {
                if dst.len() != js_items.len() {
                    return err(format!("array length mismatch: save has {}, JSON has {}", dst.len(), js_items.len()));
                }
                let mut owned = std::mem::take(dst);
                merge_array_elements(g, &mut owned, js_items, None)?;
                match &mut g.arena[idx] {
                    Node::ArraySingleString { items: dst } | Node::ArraySingleObject { items: dst } => {
                        *dst = owned;
                    }
                    _ => unreachable!(),
                }
                Ok(())
            }
            Node::BinaryArray { bt, additional, items: dst, .. } => {
                if dst.len() != js_items.len() {
                    return err(format!("array length mismatch: save has {}, JSON has {}", dst.len(), js_items.len()));
                }
                if *bt == 0 {
                    let Additional::Prim(ptype) = additional else {
                        return err("primitive binary array without primitive type metadata");
                    };
                    let mut new_items = Vec::with_capacity(dst.len());
                    for el in js_items {
                        if el.is_null() {
                            return err("primitive array elements cannot be null");
                        }
                        new_items.push(Value::Prim(json_to_prim(el, *ptype)?));
                    }
                    *dst = new_items;
                    Ok(())
                } else {
                    let mut owned = std::mem::take(dst);
                    let info = (*bt, additional.clone());
                    merge_array_elements(g, &mut owned, js_items, Some(info))?;
                    match &mut g.arena[idx] {
                        Node::BinaryArray { items: dst, .. } => *dst = owned,
                        _ => unreachable!(),
                    }
                    Ok(())
                }
            }
            _ => err("JSON array does not match the node type in the save file"),
        },
        _ => err("JSON value does not match the node type in the save file"),
    }
}

fn merge_array_elements(g: &mut Graph, dst: &mut Vec<Value>, items: &[Json], _info: Option<(u8, Additional)>) -> Result<()> {
    for i in 0..dst.len() {
        let el = &items[i];
        if el.is_null() {
            dst[i] = Value::Null;
            continue;
        }
        let slot = std::mem::replace(&mut dst[i], Value::Null);
        let mut slot = slot;
        merge_member_value(g, 2, &Additional::None, &mut slot, el)?;
        dst[i] = slot;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Whole-document apply
// ---------------------------------------------------------------------------

/// Merge a full JSON document (as produced by `dump`) into the parsed graph.
pub fn merge_root(g: &mut Graph, json: &Json) -> Result<()> {
    merge_node(g, g.root, json)
}

// ---------------------------------------------------------------------------
// Path operations (get / set)
// ---------------------------------------------------------------------------

fn parse_index(seg: &str) -> Result<usize> {
    seg.parse::<usize>().map_err(|_| SaveError(format!("'{seg}' is not an array index")))
}

pub fn get_path(g: &Graph, path: &str) -> Result<Json> {
    let segs = split_path(path);
    if segs.is_empty() {
        return dump(g);
    }
    let last = segs.len() - 1;
    let mut idx = effective_node(g, g.root);
    for (k, seg) in segs.iter().enumerate() {
        let is_last = k == last;
        idx = effective_node(g, idx);
        match &g.arena[idx] {
            Node::Class { .. } => {
                let mi = find_member(g, idx, seg)?;
                match &g.arena[idx] {
                    Node::Class { members, .. } => match &members[mi] {
                        Value::Ref(j) => {
                            idx = *j;
                        }
                        Value::Prim(p) => {
                            if is_last {
                                return Ok(prim_to_json(p));
                            }
                            return err(format!("'{seg}' is a primitive value; cannot descend into it"));
                        }
                        Value::Null => {
                            if is_last {
                                return Ok(Json::Null);
                            }
                            return err(format!("'{seg}' is null; cannot descend into it"));
                        }
                    },
                    _ => unreachable!(),
                }
            }
            Node::ArraySinglePrimitive { items, .. } => {
                let i = parse_index(seg)?;
                let p = items.get(i).ok_or_else(|| SaveError("array index out of bounds".into()))?;
                if is_last {
                    return Ok(prim_to_json(p));
                }
                return err("path descends into a primitive array element");
            }
            Node::ArraySingleString { items } | Node::ArraySingleObject { items } | Node::BinaryArray { items, .. } => {
                let i = parse_index(seg)?;
                let el = items.get(i).ok_or_else(|| SaveError("array index out of bounds".into()))?;
                match el {
                    Value::Ref(j) => {
                        idx = *j;
                    }
                    Value::Null => {
                        if is_last {
                            return Ok(Json::Null);
                        }
                        return err(format!("element {i} is null; cannot descend into it"));
                    }
                    Value::Prim(_) => {
                        if is_last {
                            return Ok(Json::Null);
                        }
                        return err("path descends into a primitive element");
                    }
                }
            }
            Node::Str(_) => return err("path continues into a string value"),
        }
    }
    value_to_json(g, &Value::Ref(idx), 0)
}

fn split_path(path: &str) -> Vec<String> {
    path.split('.')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

pub fn set_path(g: &mut Graph, path: &str, json: &Json) -> Result<()> {
    let segs = split_path(path);
    if segs.is_empty() {
        return err("empty path");
    }
    let (parent_path, last) = segs.split_at(segs.len() - 1);
    let last = &last[0];

    // navigate to parent node
    let mut idx = effective_node(g, g.root);
    for seg in parent_path {
        idx = effective_node(g, idx);
        idx = match &g.arena[idx] {
            Node::Class { .. } => {
                let mi = find_member(g, idx, seg)?;
                match &g.arena[idx] {
                    Node::Class { members, .. } => match &members[mi] {
                        Value::Ref(j) => *j,
                        _ => return err(format!("'{seg}' is a primitive; cannot descend into it")),
                    },
                    _ => unreachable!(),
                }
            }
            Node::ArraySingleString { items } | Node::ArraySingleObject { items } | Node::BinaryArray { items, .. } => {
                let i = parse_index(seg)?;
                match items.get(i) {
                    Some(Value::Ref(j)) => *j,
                    Some(_) => return err(format!("'{seg}' element is null or primitive")),
                    None => return err("array index out of bounds"),
                }
            }
            Node::ArraySinglePrimitive { .. } => return err("path into primitive array needs an index"),
            Node::Str(_) => return err("path continues into a string"),
        };
    }

    let idx = effective_node(g, idx);
    match &g.arena[idx] {
        Node::Class { .. } => {
            let mi = find_member(g, idx, last)?;
            let (bt, ai) = match &g.arena[idx] {
                Node::Class { binary_types, additional, .. } => (binary_types[mi], additional[mi].clone()),
                _ => unreachable!(),
            };
            let mut slot = match &mut g.arena[idx] {
                Node::Class { members, .. } => std::mem::replace(&mut members[mi], Value::Null),
                _ => unreachable!(),
            };
            merge_member_value(g, bt, &ai, &mut slot, json)?;
            match &mut g.arena[idx] {
                Node::Class { members, .. } => {
                    members[mi] = slot;
                }
                _ => unreachable!(),
            }
            Ok(())
        }
        Node::ArraySinglePrimitive { ptype, .. } => {
            let ptype = *ptype;
            let i = parse_index(last)?;
            let len = match &g.arena[idx] {
                Node::ArraySinglePrimitive { items, .. } => items.len(),
                _ => unreachable!(),
            };
            if i >= len {
                return err("array index out of bounds");
            }
            if json.is_null() {
                return err("primitive array elements cannot be null");
            }
            let v = json_to_prim(json, ptype)?;
            match &mut g.arena[idx] {
                Node::ArraySinglePrimitive { items, .. } => items[i] = v,
                _ => unreachable!(),
            }
            Ok(())
        }
        Node::ArraySingleString { .. } | Node::ArraySingleObject { .. } => {
            let i = parse_index(last)?;
            let len = match &g.arena[idx] {
                Node::ArraySingleString { items } | Node::ArraySingleObject { items } => items.len(),
                _ => unreachable!(),
            };
            if i >= len {
                return err("array index out of bounds");
            }
            let mut slot = match &mut g.arena[idx] {
                Node::ArraySingleString { items } | Node::ArraySingleObject { items } => {
                    std::mem::replace(&mut items[i], Value::Null)
                }
                _ => unreachable!(),
            };
            merge_member_value(g, 2, &Additional::None, &mut slot, json)?;
            match &mut g.arena[idx] {
                Node::ArraySingleString { items } | Node::ArraySingleObject { items } => {
                    items[i] = slot;
                }
                _ => unreachable!(),
            }
            Ok(())
        }
        Node::BinaryArray { bt, additional, .. } => {
            let i = parse_index(last)?;
            let len = match &g.arena[idx] {
                Node::BinaryArray { items, .. } => items.len(),
                _ => unreachable!(),
            };
            if i >= len {
                return err("array index out of bounds");
            }
            if *bt == 0 {
                let ptype = match additional {
                    Additional::Prim(p) => *p,
                    _ => return err("primitive binary array without primitive type metadata"),
                };
                if json.is_null() {
                    return err("primitive array elements cannot be null");
                }
                let v = json_to_prim(json, ptype)?;
                match &mut g.arena[idx] {
                    Node::BinaryArray { items, .. } => items[i] = Value::Prim(v),
                    _ => unreachable!(),
                }
                Ok(())
            } else {
                let mut slot = match &mut g.arena[idx] {
                    Node::BinaryArray { items, .. } => std::mem::replace(&mut items[i], Value::Null),
                    _ => unreachable!(),
                };
                merge_member_value(g, 2, &Additional::None, &mut slot, json)?;
                match &mut g.arena[idx] {
                    Node::BinaryArray { items, .. } => items[i] = slot,
                    _ => unreachable!(),
                }
                Ok(())
            }
        }
        Node::Str(_) => err("cannot set a path into a string"),
    }
}

// ---------------------------------------------------------------------------
// Byte-level patching: edit primitives in place without re-serializing
// ---------------------------------------------------------------------------

/// Where a patchable value lives inside the decrypted save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchTarget {
    /// fixed-size primitive; `offset` is absolute in the plain stream
    Primitive { offset: usize, ptype: u8 },
    /// string payload (length prefix stays unchanged, so patch length must match)
    Str { payload_offset: usize, len: usize },
}

pub fn patch_target(g: &Graph, path: &str) -> Result<PatchTarget> {
    let segs = split_path(path);
    if segs.is_empty() {
        return err("empty path");
    }
    let last = segs.len() - 1;
    let mut idx = effective_node(g, g.root);

    for (i, seg) in segs.iter().enumerate() {
        let is_last = i == last;
        idx = effective_node(g, idx);
        match &g.arena[idx] {
            Node::Class { .. } => {
                let mi = find_member(g, idx, seg)?;
                if is_last {
                    return class_member_target(g, idx, mi);
                }
                match &g.arena[idx] {
                    Node::Class { members, .. } => {
                        idx = match &members[mi] {
                            Value::Ref(j) => *j,
                            _ => return err(format!("'{seg}' is a primitive; cannot descend into it")),
                        };
                    }
                    _ => unreachable!(),
                }
            }
            Node::ArraySinglePrimitive { ptype, items } => {
                let el = parse_index(seg)?;
                if el >= items.len() {
                    return err("array index out of bounds");
                }
                if is_last {
                    return Ok(PatchTarget::Primitive { offset: g.child_pos[idx][el], ptype: *ptype });
                }
                return err("cannot descend into a primitive array element");
            }
            Node::ArraySingleString { items } | Node::ArraySingleObject { items } | Node::BinaryArray { items, .. } => {
                let el = parse_index(seg)?;
                if el >= items.len() {
                    return err("array index out of bounds");
                }
                if is_last {
                    return element_target(g, idx, el);
                }
                idx = match &items[el] {
                    Value::Ref(j) => *j,
                    _ => return err(format!("'{seg}' element is null or primitive; cannot descend into it")),
                };
            }
            Node::Str(_) => return err("path continues into a string"),
        }
    }
    err("path could not be resolved")
}

fn class_member_target(g: &Graph, idx: usize, mi: usize) -> Result<PatchTarget> {
    let (bt, additional) = match &g.arena[idx] {
        Node::Class { binary_types, additional, .. } => (binary_types[mi], additional[mi].clone()),
        _ => unreachable!(),
    };
    match bt {
        0 => match additional {
            Additional::Prim(ptype) => Ok(PatchTarget::Primitive { offset: g.child_pos[idx][mi], ptype }),
            _ => err("primitive member without primitive type metadata"),
        },
        1 => {
            let j = match &g.arena[idx] {
                Node::Class { members, .. } => match &members[mi] {
                    Value::Ref(j) => *j,
                    _ => return err("string member is not an object reference"),
                },
                _ => unreachable!(),
            };
            str_target(g, j)
        }
        other => err(format!("member type {other} is not primitive or string (not patchable)")),
    }
}

fn element_target(g: &Graph, idx: usize, el: usize) -> Result<PatchTarget> {
    match &g.arena[idx] {
        Node::ArraySingleString { items } | Node::ArraySingleObject { items } => {
            match &items[el] {
                Value::Ref(j) => str_target(g, *j),
                Value::Prim(p) => Ok(PatchTarget::Primitive { offset: g.child_pos[idx][el], ptype: prim_guess_ptype(p) }),
                Value::Null => err("element is null (not patchable)"),
            }
        }
        Node::BinaryArray { bt, additional, items, .. } => {
            if *bt == 0 {
                match additional {
                    Additional::Prim(ptype) => Ok(PatchTarget::Primitive { offset: g.child_pos[idx][el], ptype: *ptype }),
                    _ => err("primitive binary array without primitive type metadata"),
                }
            } else {
                match &items[el] {
                    Value::Ref(j) => str_target(g, *j),
                    Value::Prim(p) => Ok(PatchTarget::Primitive { offset: g.child_pos[idx][el], ptype: prim_guess_ptype(p) }),
                    Value::Null => err("element is null (not patchable)"),
                }
            }
        }
        _ => unreachable!(),
    }
}

fn str_target(g: &Graph, j: usize) -> Result<PatchTarget> {
    match &g.arena[j] {
        Node::Str(_) => {
            let pos = &g.child_pos[j];
            if pos.len() != 2 {
                return err("internal error: string node without payload position");
            }
            Ok(PatchTarget::Str { payload_offset: pos[0], len: pos[1] })
        }
        _ => err("string member does not reference a string object"),
    }
}

fn prim_guess_ptype(p: &Prim) -> u8 {
    match p {
        Prim::Boolean(_) => 1,
        Prim::Byte(_) => 2,
        Prim::Char(_) => 3,
        Prim::Decimal(_) => 5,
        Prim::Double(_) => 6,
        Prim::Int16(_) => 7,
        Prim::Int32(_) => 8,
        Prim::Int64(_) => 9,
        Prim::SByte(_) => 10,
        Prim::Single(_) => 11,
        Prim::TimeSpan(_) => 12,
        Prim::DateTime(_) => 13,
        Prim::UInt16(_) => 14,
        Prim::UInt32(_) => 15,
        Prim::UInt64(_) => 16,
        Prim::Null => 17,
    }
}

/// Size in bytes of a primitive when encoded in a stream (None = variable-length).
pub fn prim_byte_len(ptype: u8) -> Option<usize> {
    match ptype {
        1 => Some(1),
        2 => Some(1),
        6 => Some(8),
        7 => Some(2),
        8 => Some(4),
        9 => Some(8),
        10 => Some(1),
        11 => Some(4),
        12 => Some(8),
        13 => Some(8),
        14 => Some(2),
        15 => Some(4),
        16 => Some(8),
        _ => None,
    }
}

/// Encode a JSON value the way BinaryFormatter encodes a primitive member.
pub fn prim_from_json(j: &Json, ptype: u8) -> Result<Prim> {
    json_to_prim(j, ptype)
}

/// Encode a primitive to its little-endian byte representation (variable-length excluded).
pub fn encode_prim(p: &Prim) -> Result<Vec<u8>> {
    Ok(match p {
        Prim::Boolean(v) => vec![*v as u8],
        Prim::Byte(v) => vec![*v],
        Prim::SByte(v) => vec![*v as u8],
        Prim::Double(v) => v.to_le_bytes().to_vec(),
        Prim::Int16(v) => v.to_le_bytes().to_vec(),
        Prim::Int32(v) => v.to_le_bytes().to_vec(),
        Prim::Int64(v) => v.to_le_bytes().to_vec(),
        Prim::Single(v) => v.to_le_bytes().to_vec(),
        Prim::TimeSpan(v) => v.to_le_bytes().to_vec(),
        Prim::DateTime(v) => v.to_le_bytes().to_vec(),
        Prim::UInt16(v) => v.to_le_bytes().to_vec(),
        Prim::UInt32(v) => v.to_le_bytes().to_vec(),
        Prim::UInt64(v) => v.to_le_bytes().to_vec(),
        Prim::Char(_) => return err("Char fields are variable-length strings in NRBF (cannot patch)"),
        Prim::Decimal(_) => return err("Decimal fields are variable-length strings in NRBF (cannot patch)"),
        Prim::Null => return err("cannot patch a null primitive"),
    })
}


