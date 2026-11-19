//! MCP (Model Context Protocol) server over stdio JSON-RPC 2.0.
//!
//! The goal is the opposite of "safe little buttons": give the agent a broad
//! capability surface so it can do with the game *anything*, including things
//! we never thought of. Prefer primitives (read/write/search memory and
//! files, arbitrary DLL injection, raw NRBF access) over one-off commands.
//!
//! Wire protocol: newline-delimited JSON-RPC 2.0 on stdin/stdout (MCP stdio
//! transport). Notifications (no `id`) are ignored. Every tool returns a
//! single text content block whose payload is usually pretty-printed JSON.
//!
//! Safety: tools that WRITE the save refuse to run while the game is running
//! (the game overwrites saves on exit). Everything else is the agent's
//! responsibility.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::app;
use crate::save;
use crate::tool::{cars, catalog, events, game, inject};

// ===========================================================================
// dispatch
// ===========================================================================

pub fn dispatch(request: &Value) -> Option<Value> {
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
    if method.starts_with("notifications/") {
        return None;
    }
    let id = request.get("id")?.clone();
    let params = request.get("params").cloned().unwrap_or(Value::Null);

    let ok = |result: Value| -> Value { json!({ "jsonrpc": "2.0", "id": id, "result": result }) };
    let err = |code: i64, msg: String| -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })
    };

    match method {
        "initialize" => Some(ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "swgp2-mcp", "version": env!("CARGO_PKG_VERSION") }
        }))),
        "ping" => Some(ok(json!({}))),
        "tools/list" => Some(ok(json!({
            "tools": tools()
                .iter()
                .map(|t| json!({ "name": t.name, "description": t.description, "inputSchema": t.schema }))
                .collect::<Vec<_>>()
        }))),
        "tools/call" => {
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call_tool(name, &args) {
                Ok(text) => Some(ok(json!({ "content": [{ "type": "text", "text": text }], "isError": false }))),
                Err(e) => Some(ok(json!({ "content": [{ "type": "text", "text": format!("error: {e}") }], "isError": true }))),
            }
        }
        other => Some(err(-32601, format!("method not found: {other}"))),
    }
}

// ===========================================================================
// tool registry
// ===========================================================================

struct ToolDef {
    name: &'static str,
    description: &'static str,
    schema: Value,
    handler: fn(&Value) -> Result<String, String>,
}

fn obj_schema(props: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false
    })
}

fn tools() -> Vec<ToolDef> {
    let mut t = Vec::new();
    t.push(ToolDef { name: "save_slots", description: "List all detected Supersave<N>.sav slots", schema: obj_schema(json!({}), &[]), handler: h_save_slots });
    t.push(ToolDef { name: "save_get", description: "Read a single field from a save, e.g. path=GarageSlots.0.CarID. Returns the JSON value.", schema: obj_schema(json!({"path": {"type": "string"}, "slot": {"type": "integer", "minimum": 0}}), &["path"]), handler: h_save_get });
    t.push(ToolDef { name: "save_set", description: "Set a single field in a save (in-place byte patch, backup + verify). Refuses while the game is running.", schema: obj_schema(json!({"path": {"type": "string"}, "value": {}, "slot": {"type": "integer", "minimum": 0}}), &["path", "value"]), handler: h_save_set });
    t.push(ToolDef { name: "save_dump", description: "Dump the whole save as pretty JSON", schema: obj_schema(json!({"slot": {"type": "integer", "minimum": 0}}), &[]), handler: h_save_dump });
    t.push(ToolDef { name: "save_apply", description: "Apply a dump-shaped JSON document back into the save (scalar leaves only, byte-patched, backup + verify). Refuses while the game is running.", schema: obj_schema(json!({"json": {"type": "string"}, "slot": {"type": "integer", "minimum": 0}}), &["json"]), handler: h_save_apply });
    t.push(ToolDef { name: "save_backup", description: "Create a timestamped backup of the save", schema: obj_schema(json!({"slot": {"type": "integer", "minimum": 0}}), &[]), handler: h_save_backup });
    t.push(ToolDef { name: "save_backups", description: "List all backups of the save", schema: obj_schema(json!({"slot": {"type": "integer", "minimum": 0}}), &[]), handler: h_save_backups });
    t.push(ToolDef { name: "save_restore", description: "Restore a .savbak backup (current save is backed up first). Refuses while the game is running.", schema: obj_schema(json!({"backup": {"type": "string"}, "slot": {"type": "integer", "minimum": 0}}), &["backup"]), handler: h_save_restore });
    t.push(ToolDef { name: "save_verify", description: "Parse/emit/parse roundtrip check of the save", schema: obj_schema(json!({"slot": {"type": "integer", "minimum": 0}}), &[]), handler: h_save_verify });
    t.push(ToolDef { name: "save_cheats", description: "List the quick-cheat presets the GUI offers (for reference; use save_set to apply)", schema: obj_schema(json!({}), &[]), handler: h_save_cheats });

    t.push(ToolDef { name: "game_info", description: "Detect the portable game folder and return game info (exe, saves, boot.config, ...)", schema: obj_schema(json!({}), &[]), handler: h_game_info });
    t.push(ToolDef { name: "game_launch", description: "Launch the game (from the detected folder)", schema: obj_schema(json!({}), &[]), handler: h_game_launch });
    t.push(ToolDef { name: "events_scan", description: "Parse the game's level0 and return all events (weekends, cups, tracks, pilots, restrictions, weather) as JSON", schema: obj_schema(json!({"level": {"type": "string"}}), &[]), handler: h_events_scan });
    t.push(ToolDef { name: "wiki_best_cars", description: "Best cars for a named event (restricted by the event's own tags/dealer/model/class), ranked by CV then top speed", schema: obj_schema(json!({"event": {"type": "string"}, "count": {"type": "integer", "minimum": 1, "maximum": 25}}), &["event"]), handler: h_wiki_best_cars });
    t.push(ToolDef { name: "catalog_cars", description: "All 200 shop cars with id/dealer/model/rally/real name + official stats where known", schema: obj_schema(json!({}), &[]), handler: h_catalog_cars });
    t.push(ToolDef { name: "ranked_cars", description: "The game's official 25-car power ranking (uselesslist.txt)", schema: obj_schema(json!({}), &[]), handler: h_ranked_cars });

    t.push(ToolDef { name: "file_info", description: "File size/mtime/extension + magic-byte sniff (PE, PNG, TGA, NRBF/BinaryFormatter, zlib, zip, ...) + entropy", schema: obj_schema(json!({"path": {"type": "string"}}), &["path"]), handler: h_file_info });
    t.push(ToolDef { name: "file_hexdump", description: "Hex dump of a byte range of a file", schema: obj_schema(json!({"path": {"type": "string"}, "offset": {"type": "integer", "minimum": 0}, "length": {"type": "integer", "minimum": 1, "maximum": 65536}}), &["path"]), handler: h_file_hexdump });
    t.push(ToolDef { name: "file_strings", description: "Printable ASCII/UTF-16 strings in a file (capped)", schema: obj_schema(json!({"path": {"type": "string"}, "min_len": {"type": "integer", "minimum": 1}, "limit": {"type": "integer", "minimum": 1, "maximum": 10000}}), &["path"]), handler: h_file_strings });
    t.push(ToolDef { name: "file_search", description: "Scan a file for a byte pattern with ?? wildcards, e.g. '4D 5A ?? ?? 90'", schema: obj_schema(json!({"path": {"type": "string"}, "pattern": {"type": "string"}, "max_results": {"type": "integer", "minimum": 1, "maximum": 10000}}), &["path", "pattern"]), handler: h_file_search });
    t.push(ToolDef { name: "file_copy", description: "Copy a file (no overwrite unless overwrite=true)", schema: obj_schema(json!({"src": {"type": "string"}, "dst": {"type": "string"}, "overwrite": {"type": "boolean"}}), &["src", "dst"]), handler: h_file_copy });

    t.push(ToolDef { name: "process_find", description: "Find processes by exe-name substring (case-insensitive)", schema: obj_schema(json!({"name": {"type": "string"}}), &["name"]), handler: h_process_find });
    t.push(ToolDef { name: "process_modules", description: "List loaded modules (name, base, size, path) of a process", schema: obj_schema(json!({"pid": {"type": "integer", "minimum": 1}}), &["pid"]), handler: h_process_modules });
    t.push(ToolDef { name: "memory_read", description: "Read raw bytes from a process as hex", schema: obj_schema(json!({"pid": {"type": "integer", "minimum": 1}, "address": {"type": "integer", "minimum": 0}, "length": {"type": "integer", "minimum": 1, "maximum": 4096}}), &["pid", "address", "length"]), handler: h_memory_read });
    t.push(ToolDef { name: "memory_write", description: "Write raw bytes into a process (hex string)", schema: obj_schema(json!({"pid": {"type": "integer", "minimum": 1}, "address": {"type": "integer", "minimum": 0}, "hex": {"type": "string"}}), &["pid", "address", "hex"]), handler: h_memory_write });
    t.push(ToolDef { name: "memory_search", description: "Scan a process's committed memory for a byte pattern with ?? wildcards", schema: obj_schema(json!({"pid": {"type": "integer", "minimum": 1}, "pattern": {"type": "string"}, "max_results": {"type": "integer", "minimum": 1, "maximum": 10000}}), &["pid", "pattern"]), handler: h_memory_search });

    t.push(ToolDef { name: "inject_dll", description: "Inject an arbitrary native DLL into a process (default: the running game) via CreateRemoteThread+LoadLibraryW", schema: obj_schema(json!({"dll_path": {"type": "string"}, "pid": {"type": "integer", "minimum": 1}}), &["dll_path"]), handler: h_inject_dll });
    t.push(ToolDef { name: "mod_install", description: "Install the managed in-game mod to %APPDATA%\\swgp2-tool\\mod\\Swgp2Mod.dll", schema: obj_schema(json!({}), &[]), handler: h_mod_install });
    t.push(ToolDef { name: "mod_inject", description: "Install the mod (if missing) and inject it into the running game; returns log tail", schema: obj_schema(json!({"pid": {"type": "integer", "minimum": 1}}), &[]), handler: h_mod_inject });
    t.push(ToolDef { name: "mod_status", description: "Is the game running? Is the mod installed? Tail of mod.log", schema: obj_schema(json!({}), &[]), handler: h_mod_status });

    t
}

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    tools()
        .into_iter()
        .find(|t| t.name == name)
        .map(|t| (t.handler)(args))
        .unwrap_or_else(|| Err(format!("unknown tool: {name}")))
}

// ===========================================================================
// shared helpers
// ===========================================================================

fn require_game_closed() -> Result<(), String> {
    if inject::find_game_pid().is_some() {
        Err("the game is running - close it first, otherwise it will overwrite the save on exit".to_string())
    } else {
        Ok(())
    }
}

fn slot_path(slot: Option<u32>) -> Result<PathBuf, String> {
    match slot {
        Some(n) => save::default_save_path(n).ok_or_else(|| format!("slot {n} is not available")),
        None => app::slot_choices()
            .into_iter()
            .next()
            .ok_or_else(|| "no save found - has the game been run?".to_string()),
    }
}

fn path_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing required argument '{key}'"))
}

fn opt_u64(args: &Value, key: &str, default: u64) -> u64 {
    args.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
}

fn opt_slot(args: &Value) -> Option<u32> {
    args.get("slot").and_then(|v| v.as_u64()).map(|n| n as u32)
}

// ===========================================================================
// save tools
// ===========================================================================

fn h_save_slots(_: &Value) -> Result<String, String> {
    let slots = app::slot_choices();
    let list: Vec<_> = slots.iter().map(|p| json!({ "path": p.display().to_string(), "size": std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) })).collect();
    Ok(serde_json::to_string_pretty(&json!({ "slots": list })).unwrap())
}

fn h_save_get(args: &Value) -> Result<String, String> {
    let path = path_arg(args, "path")?;
    let sp = slot_path(opt_slot(args))?;
    let (g, _) = save::load(&sp).map_err(|e| e.to_string())?;
    let v = save::get_path(&g, &path).map_err(|e| e.to_string())?;
    Ok(serde_json::to_string_pretty(&json!({ "path": path, "value": v })).unwrap())
}

fn h_save_set(args: &Value) -> Result<String, String> {
    require_game_closed()?;
    let path = path_arg(args, "path")?;
    let value = args.get("value").cloned().ok_or("missing required argument 'value'")?;
    let sp = slot_path(opt_slot(args))?;
    let backup = app::set_single(&sp, &path, value)?;
    Ok(format!("set {path} (backup: {})", backup.display()))
}

fn h_save_dump(args: &Value) -> Result<String, String> {
    let sp = slot_path(opt_slot(args))?;
    app::dump_json(&sp)
}

fn h_save_apply(args: &Value) -> Result<String, String> {
    require_game_closed()?;
    let json = path_arg(args, "json")?;
    let sp = slot_path(opt_slot(args))?;
    let tmp = std::env::temp_dir().join("swgp2-mcp-apply.json");
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    let backup = app::apply_json_file(&sp, &tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(format!("applied (backup: {})", backup.display()))
}

fn h_save_backup(args: &Value) -> Result<String, String> {
    let sp = slot_path(opt_slot(args))?;
    let b = save::create_backup(&sp).map_err(|e| e.to_string())?;
    Ok(format!("backup: {}", b.display()))
}

fn h_save_backups(args: &Value) -> Result<String, String> {
    let sp = slot_path(opt_slot(args))?;
    let list = save::list_backups(&sp).map_err(|e| e.to_string())?;
    let items: Vec<_> = list.iter().map(|p| p.display().to_string()).collect();
    Ok(serde_json::to_string_pretty(&json!({ "backups": items })).unwrap())
}

fn h_save_restore(args: &Value) -> Result<String, String> {
    require_game_closed()?;
    let backup = path_arg(args, "backup")?;
    let sp = slot_path(opt_slot(args))?;
    let cur = app::restore_backup(&sp, Path::new(&backup))?;
    Ok(format!("restored {backup} (previous save: {})", cur.display()))
}

fn h_save_verify(args: &Value) -> Result<String, String> {
    let sp = slot_path(opt_slot(args))?;
    let raw = std::fs::read(&sp).map_err(|e| e.to_string())?;
    let plain = crate::crypto::decrypt_save(&raw).map_err(|e| e.to_string())?;
    let g1 = crate::nrbf::parse(&plain).map_err(|e| e.to_string())?;
    let re = crate::nrbf::Writer::new().write(&g1).map_err(|e| e.to_string())?;
    let g2 = crate::nrbf::parse(&re).map_err(|e| e.to_string())?;
    let d1 = serde_json::to_string(&save::dump(&g1).map_err(|e| e.to_string())?).unwrap_or_default();
    let d2 = serde_json::to_string(&save::dump(&g2).map_err(|e| e.to_string())?).unwrap_or_default();
    if d1 == d2 { Ok("save verified - parse/emit roundtrip OK".to_string()) } else { Err("roundtrip mismatch".to_string()) }
}

fn h_save_cheats(_: &Value) -> Result<String, String> {
    Ok(json!({
        "cheats": [
            { "name": "+1,000,000 Cr", "patch": { "Economy": "+1000000" } },
            { "name": "Cr = 999,999,999", "patch": { "Economy": 999_999_999u64 } },
            { "name": "Coins = 999,999", "patch": { "Coins": 999_999u64 } }
        ],
        "note": "use save_set with path=Economy / Coins to apply"
    }).to_string())
}

// ===========================================================================
// game / wiki tools
// ===========================================================================

fn h_game_info(_: &Value) -> Result<String, String> {
    let Some(dir) = game::detect_game_dir() else {
        return Err("game folder not detected - set it in the GUI or pick it manually".to_string());
    };
    match game::game_info(&dir) {
        Ok(info) => Ok(serde_json::to_string_pretty(&json!({
            "dir": info.dir, "exe": info.exe, "save_dir": info.save_dir,
            "save_slots": info.save_slots, "backups": info.backups,
            "skin_cars": info.skin_cars, "car_skins": info.car_skins,
            "boot_config": info.boot_config,
        }))
        .unwrap()),
        Err(e) => Err(format!("game folder invalid: {e}")),
    }
}

fn h_game_launch(_: &Value) -> Result<String, String> {
    let Some(dir) = game::detect_game_dir() else {
        return Err("game folder not detected".to_string());
    };
    let exe = dir.join(game::EXE_NAME);
    std::process::Command::new(&exe)
        .spawn()
        .map(|_| format!("launched {}", exe.display()))
        .map_err(|e| format!("launch failed: {e}"))
}

fn h_events_scan(args: &Value) -> Result<String, String> {
    let level = args
        .get("level")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| events::default_level_path(&game::detect_game_dir().unwrap_or_else(|| PathBuf::from("."))));
    let evs = events::scan_events(&level).map_err(|e| e.to_string())?;
    let out: Vec<Value> = evs
        .iter()
        .map(|e| {
            json!({
                "name": e.name,
                "kind": format!("{:?}", e.kind),
                "tags": e.tags,
                "dealer": e.dealer,
                "model": e.model,
                "weather": e.weather,
                "pilot": e.pilot,
                "class": e.class,
                "endurance": e.is_endurance(),
                "rally_stage": e.is_rally_stage(),
                "restriction": e.restriction_text(),
                "offset": e.offset,
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "count": out.len(), "events": out })).unwrap())
}

fn h_wiki_best_cars(args: &Value) -> Result<String, String> {
    let name = path_arg(args, "event")?;
    let count = opt_u64(args, "count", 5).clamp(1, 25) as usize;
    let Some(gd) = game::detect_game_dir() else {
        return Err("game folder not detected".to_string());
    };
    let events_ = events::load_for_game(&gd).map_err(|e| e.to_string())?;
    let Some(ev) = events_.iter().find(|e| e.name.eq_ignore_ascii_case(&name)) else {
        return Err(format!("event '{name}' not found (use events_scan to list)"));
    };
    let cat = catalog::load_catalog(&gd);
    let rally_only = ev.is_rally_stage();
    let picks = events::best_cars_for_event(ev, &cat, count, rally_only);
    let list: Vec<Value> = picks
        .iter()
        .map(|c| {
            json!({
                "name": catalog::display_name(c),
                "id": c.id,
                "dealer": c.dealer,
                "model": c.model,
                "rally": c.rally,
                "cv": c.cv,
                "top_speed": c.top_speed,
                "year": c.year,
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "event": ev.name, "restriction": ev.restriction_text(), "picks": list })).unwrap())
}

fn h_catalog_cars(_: &Value) -> Result<String, String> {
    let Some(gd) = game::detect_game_dir() else {
        return Err("game folder not detected".to_string());
    };
    let cat = catalog::load_catalog(&gd);
    let list: Vec<Value> = cat
        .iter()
        .map(|c| {
            json!({
                "id": c.id, "dealer": c.dealer, "model": c.model,
                "rally": c.rally, "real": c.real,
                "cv": c.cv, "top_speed": c.top_speed, "year": c.year,
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "count": list.len(), "cars": list })).unwrap())
}

fn h_ranked_cars(_: &Value) -> Result<String, String> {
    let Some(gd) = game::detect_game_dir() else {
        return Err("game folder not detected".to_string());
    };
    let txt = game::streaming_data_dir(&gd).join("uselesslist.txt");
    let list = cars::CarEntry::parse_file(&txt);
    let out: Vec<Value> = list
        .iter()
        .map(|c| json!({ "name": c.name, "cv": c.cv, "top_speed": c.top_speed, "year": c.year, "rally": c.rally }))
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "count": out.len(), "cars": out })).unwrap())
}

// ===========================================================================
// file tools
// ===========================================================================

fn h_file_info(args: &Value) -> Result<String, String> {
    let path = path_arg(args, "path")?;
    let p = Path::new(&path);
    let meta = std::fs::metadata(p).map_err(|e| e.to_string())?;
    let raw = std::fs::read(p).map_err(|e| e.to_string())?;
    let head = &raw[..raw.len().min(16)];
    let magic = sniff_magic(head);
    let mut freq = [0u64; 256];
    for &b in &raw {
        freq[b as usize] += 1;
    }
    let len = raw.len() as f64;
    let entropy: f64 = freq.iter().filter(|&&c| c > 0).map(|&c| {
        let p = c as f64 / len;
        -p * p.log2()
    }).sum();
    Ok(serde_json::to_string_pretty(&json!({
        "path": path, "size": meta.len(), "readonly": meta.permissions().readonly(),
        "magic": magic, "entropy_bits_per_byte": (entropy * 1e4).round() / 1e4,
    }))
    .unwrap())
}

fn sniff_magic(head: &[u8]) -> &'static str {
    if head.starts_with(b"MZ") { return "PE (Windows executable/DLL)"; }
    if head.starts_with(b"\x89PNG\r\n\x1a\n") { return "PNG image"; }
    if head.starts_with(b"II*\x00") || head.starts_with(b"MM\x00*") { return "TIFF image"; }
    if head.len() >= 4 && head[..4] == [0x00, 0x01, 0x00, 0x00] { return "NRBF (.NET BinaryFormatter)"; }
    if head.starts_with(b"\x1f\x8b") { return "gzip"; }
    if head.starts_with(b"PK\x03\x04") { return "ZIP"; }
    if head.starts_with(b"7z\xBC\xAF\x27\x1C") { return "7-Zip"; }
    if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WAVE") { return "WAV"; }
    if head.starts_with(b"ID3") { return "MP3 (ID3)"; }
    if head.starts_with(b"\x00\x00\x01\xBA") || head.starts_with(b"\x00\x00\x01\xB3") { return "MPEG"; }
    if head.starts_with(b"GIF8") { return "GIF"; }
    if head.starts_with(b"OggS") { return "Ogg"; }
    if head.starts_with(b"fLaC") { return "FLAC"; }
    if head.starts_with(b"{\"") || head.starts_with(b"[{") || head.starts_with(b"\xef\xbb\xbf{") { return "JSON (text)"; }
    "unknown"
}

fn h_file_hexdump(args: &Value) -> Result<String, String> {
    let path = path_arg(args, "path")?;
    let offset = opt_u64(args, "offset", 0);
    let length = opt_u64(args, "length", 256).clamp(1, 65536) as usize;
    let raw = std::fs::read(&path).map_err(|e| e.to_string())?;
    let start = (offset as usize).min(raw.len());
    let end = (start + length).min(raw.len());
    let mut out = String::new();
    let mut i = start;
    while i < end {
        let chunk_end = (i + 16).min(end);
        let chunk = &raw[i..chunk_end];
        let hex: String = chunk.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
        let ascii: String = chunk.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
        out.push_str(&format!("{i:08x}  {hex:<48}  {ascii}\n"));
        i = chunk_end;
    }
    Ok(out)
}

fn h_file_strings(args: &Value) -> Result<String, String> {
    let path = path_arg(args, "path")?;
    let min_len = opt_u64(args, "min_len", 4).clamp(1, 1024) as usize;
    let limit = opt_u64(args, "limit", 1000).clamp(1, 10000) as usize;
    let raw = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < raw.len() && out.len() < limit {
        if raw[i] >= 0x20 && raw[i] < 0x7f {
            let mut j = i;
            while j < raw.len() && raw[j] >= 0x20 && raw[j] < 0x7f {
                j += 1;
            }
            if j - i >= min_len {
                out.push(String::from_utf8_lossy(&raw[i..j]).into_owned());
            }
            i = j;
        } else if i + 1 < raw.len() && raw[i] >= 0x20 && raw[i] < 0x7f && raw[i + 1] == 0 {
            // utf-16le-ish
            let mut j = i;
            while j + 1 < raw.len() && raw[j] >= 0x20 && raw[j] < 0x7f && raw[j + 1] == 0 {
                j += 2;
            }
            if (j - i) / 2 >= min_len {
                let s: String = raw[i..j].chunks(2).filter(|c| c.len() == 2).map(|c| c[0] as char).collect();
                out.push(s);
            }
            i = j.max(i + 2);
        } else {
            i += 1;
        }
    }
    Ok(serde_json::to_string_pretty(&json!({ "path": path, "count": out.len(), "strings": out })).unwrap())
}

fn h_file_search(args: &Value) -> Result<String, String> {
    let path = path_arg(args, "path")?;
    let pattern = path_arg(args, "pattern")?;
    let max_results = opt_u64(args, "max_results", 100).clamp(1, 10000) as usize;
    let (pat, mask) = inject::parse_pattern(&pattern)?;
    let raw = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mut hits = Vec::new();
    for i in 0..raw.len().saturating_sub(pat.len() - 1) {
        let mut m = true;
        for j in 0..pat.len() {
            if mask[j] && raw[i + j] != pat[j] {
                m = false;
                break;
            }
        }
        if m {
            hits.push(i as u64);
            if hits.len() >= max_results {
                break;
            }
        }
    }
    Ok(serde_json::to_string_pretty(&json!({ "path": path, "count": hits.len(), "offsets": hits })).unwrap())
}

fn h_file_copy(args: &Value) -> Result<String, String> {
    let src = path_arg(args, "src")?;
    let dst = path_arg(args, "dst")?;
    let overwrite = args.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(false);
    if Path::new(&dst).exists() && !overwrite {
        return Err(format!("{dst} already exists (pass overwrite=true)"));
    }
    std::fs::copy(&src, &dst).map_err(|e| e.to_string())?;
    Ok(format!("copied {src} -> {dst}"))
}

// ===========================================================================
// process tools
// ===========================================================================

fn h_process_find(args: &Value) -> Result<String, String> {
    let name = path_arg(args, "name")?;
    let pids = inject::find_processes(&name)?;
    Ok(serde_json::to_string_pretty(&json!({ "name": name, "pids": pids })).unwrap())
}

fn h_process_modules(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_u64()).ok_or("missing required argument 'pid'")? as u32;
    let modules = inject::list_modules(pid)?;
    let list: Vec<Value> = modules
        .iter()
        .map(|m| json!({ "name": m.name, "base": format!("0x{:x}", m.base), "size": m.size, "path": m.path }))
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "pid": pid, "count": list.len(), "modules": list })).unwrap())
}

fn h_memory_read(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_u64()).ok_or("missing 'pid'")? as u32;
    let address = args.get("address").and_then(|v| v.as_u64()).ok_or("missing 'address'")?;
    let length = opt_u64(args, "length", 64).clamp(1, 4096) as usize;
    let bytes = inject::memory_read(pid, address, length)?;
    let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
    Ok(format!("0x{address:x} ({length} bytes):\n{hex}"))
}

fn h_memory_write(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_u64()).ok_or("missing 'pid'")? as u32;
    let address = args.get("address").and_then(|v| v.as_u64()).ok_or("missing 'address'")?;
    let hex = path_arg(args, "hex")?;
    let bytes: Result<Vec<u8>, _> = hex.split_whitespace().map(|t| u8::from_str_radix(t, 16)).collect();
    let bytes = bytes.map_err(|_| "bad hex string".to_string())?;
    let n = inject::memory_write(pid, address, &bytes)?;
    Ok(format!("wrote {n} bytes at 0x{address:x}"))
}

fn h_memory_search(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_u64()).ok_or("missing 'pid'")? as u32;
    let pattern = path_arg(args, "pattern")?;
    let max_results = opt_u64(args, "max_results", 100).clamp(1, 10000) as usize;
    let hits = inject::memory_search(pid, &pattern, max_results)?;
    let list: Vec<String> = hits.iter().map(|a| format!("0x{a:x}")).collect();
    Ok(serde_json::to_string_pretty(&json!({ "pid": pid, "count": list.len(), "addresses": list })).unwrap())
}

// ===========================================================================
// mod / injection tools
// ===========================================================================

fn h_inject_dll(args: &Value) -> Result<String, String> {
    let dll_path = path_arg(args, "dll_path")?;
    let pid = args.get("pid").and_then(|v| v.as_u64()).map(|n| n as u32).or_else(inject::find_game_pid);
    let Some(pid) = pid else {
        return Err("the game is not running (pass an explicit pid)".to_string());
    };
    inject::inject_dll(pid, Path::new(&dll_path))?;
    Ok(format!("injected {dll_path} into pid {pid}"))
}

fn h_mod_install(_: &Value) -> Result<String, String> {
    let p = inject::install_mod()?;
    Ok(format!("mod installed to {}", p.display()))
}

fn h_mod_inject(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_u64()).map(|n| n as u32).or_else(inject::find_game_pid);
    let Some(pid) = pid else {
        return Err("the game is not running (pass an explicit pid)".to_string());
    };
    let _ = inject::install_mod()?;
    // inject the loader; if only our managed dll is next to us, fall back to it
    let loader = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("swgp2-mod.dll")))
        .filter(|p| p.is_file())
        .unwrap_or_else(inject::mod_install_path);
    inject::inject_dll(pid, &loader)?;
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let log = inject::read_mod_log(15);
    Ok(format!("injected {loader:?} into pid {pid}\n{log}"))
}

fn h_mod_status(_: &Value) -> Result<String, String> {
    let running = inject::find_game_pid().is_some();
    let installed = inject::mod_install_path().is_file();
    let log = inject::read_mod_log(10);
    Ok(format!(
        "game running: {running}\nmod installed: {installed}\nmod.log tail:\n{log}"
    ))
}
