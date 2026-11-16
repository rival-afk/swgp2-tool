//! swgp2-save-editor - save-file editor for Super Woden GP 2.
//!
//! Usage: swgp2-save-editor [--slot N | --save PATH] <command> [args...]

mod crypto;
mod nrbf;
mod save;

use serde_json::Value as Json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
swgp2-save-editor - Super Woden GP 2 save editor (v{version})

USAGE:
    swgp2-save-editor [OPTIONS] <COMMAND> [ARGS]

OPTIONS:
    --slot <N>     Save slot to use (default: 0 -> Supersave0.sav)
    --save <PATH>  Use an explicit save file instead of the default path
    -h, --help     Show this help
    -V, --version  Show version

COMMANDS:
    info                      Show save overview (credits, coins, cars, stats)
    dump [FILE]               Decrypt & print the whole save as JSON (or write to FILE)
    apply <FILE.json>         Merge a JSON document (as produced by `dump`) into the save
    get <PATH>                Print a single value, e.g. `get Economy`
    set <PATH> <JSON>         Set a value (auto-backup + verify), e.g. `set Economy 999999`
    decrypt <OUT>             Decrypt the save to a raw BinaryFormatter stream
    encrypt <IN> [OUT]        Encrypt a decrypted stream back into a .sav
    roundtrip                 Verify the parser/writer round-trip on this save
    backup                    Create a timestamped backup of the save
    list-backups              List backups created by this tool
    restore <FILE>            Restore a backup (auto-backup of current save first)
    list-slots                Show which save slots exist

PATH SYNTAX:
    SaveObject members are addressed by name, array elements by numeric index.
    Examples:
        Economy                     GarageSlots.0.CarID
        StatsData_.RacesCompleted   LeagueStatus_.ChampPrices.2
        WeekendStarGroups.3.Stars.0

SAFETY:
    Every write command first creates a timestamped backup in a `Backups`
    folder next to the save file, then re-verifies the written file; if the
    round-trip check fails the editor aborts and restores the backup.

The game must be CLOSED while editing, otherwise it will overwrite the file.
";

struct Options {
    slot: u32,
    save: Option<PathBuf>,
}

fn parse_options(args: &mut Vec<String>) -> Result<Options, String> {
    let mut slot = 0u32;
    let mut save = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--slot" => {
                let v = args.get(i + 1).ok_or("--slot expects a number")?;
                slot = v.parse().map_err(|_| "--slot expects a number")?;
                args.drain(i..i + 2);
            }
            "--save" => {
                let v = args.get(i + 1).ok_or("--save expects a path")?;
                save = Some(PathBuf::from(v));
                args.drain(i..i + 2);
            }
            "-h" | "--help" => {
                println!("{}", HELP.replace("{version}", VERSION));
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("swgp2-save-editor {VERSION}");
                std::process::exit(0);
            }
            _ => i += 1,
        }
    }
    Ok(Options { slot, save })
}

fn resolve_save(opts: &Options) -> Result<PathBuf, String> {
    if let Some(p) = &opts.save {
        if !p.exists() {
            return Err(format!("save file not found: {}", p.display()));
        }
        return Ok(p.clone());
    }
    let p = save::default_save_path(opts.slot)
        .ok_or_else(|| "could not determine the save directory (USERPROFILE/HOME not set)".to_string())?;
    if !p.exists() {
        return Err(format!("save file not found: {} (use --slot or --save)", p.display()));
    }
    Ok(p)
}

fn render_json(j: &Json) -> String {
    serde_json::to_string_pretty(j).unwrap_or_else(|_| j.to_string())
}

/// Load, apply `edit` to the graph, back up the original file, write, and
/// verify the written file by re-parsing it and comparing JSON dumps.
fn write_with_backup<F>(save_path: &PathBuf, edit: F) -> Result<(), String>
where
    F: FnOnce(&mut nrbf::Graph) -> Result<(), save::SaveError>,
{
    let (mut g, _raw) = save::load(save_path).map_err(|e| e.to_string())?;

    edit(&mut g).map_err(|e| e.to_string())?;
    let expected = render_json(&save::dump(&g).map_err(|e| e.to_string())?);

    let backup = save::create_backup(save_path).map_err(|e| e.to_string())?;
    save::store(save_path, &g).map_err(|e| e.to_string())?;

    // Re-read what we just wrote and confirm it round-trips to the same data.
    let after_bytes = fs::read(save_path).map_err(|e| e.to_string())?;
    let plain = crypto::decrypt_save(&after_bytes).map_err(|e| format!("verify failed: {e}"))?;
    let g2 = nrbf::parse(&plain).map_err(|e| format!("verify failed to re-parse: {e}"))?;
    let after = render_json(&save::dump(&g2).map_err(|e| e.to_string())?);

    if expected == after {
        return Ok(());
    }
    // Verification failed: restore the backup so the game still has a working save.
    let _ = fs::copy(&backup, save_path);
    Err(format!(
        "written save failed verification and was restored from backup {backup}",
        backup = backup.display()
    ))
}

fn cmd_info(save_path: &PathBuf) -> Result<(), String> {
    let (g, _) = save::load(save_path).map_err(|e| e.to_string())?;
    let get = |p: &str| save::get_path(&g, p).unwrap_or(Json::Null);
    let cars = match save::get_path(&g, "GarageSlots") {
        Ok(Json::Array(a)) => a.len(),
        _ => 0,
    };
    let bytes = fs::metadata(save_path).map(|m| m.len()).unwrap_or(0);
    println!("Save file:      {}", save_path.display());
    println!("Size:           {bytes} bytes");
    println!("Economy (Cr):   {}", get("Economy"));
    println!("Coins:          {}", get("Coins"));
    println!("Cars in garage: {cars}");
    println!("Language:       {}", get("Language"));
    println!("Difficulty:     {}", get("Difficulty"));
    println!("Races completed: {}", get("StatsData_.RacesCompleted"));
    println!("Victories:      {}", get("StatsData_.Victories"));
    println!("Money won:      {}", get("StatsData_.MoneyWon"));
    println!("Money spent:    {}", get("StatsData_.MoneySpent"));
    println!("Total distance: {} km", get("StatsData_.TotalKms"));
    Ok(())
}

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let opts = match parse_options(&mut args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    let Some(cmd) = args.first().cloned() else {
        println!("{}", HELP.replace("{version}", VERSION));
        return ExitCode::SUCCESS;
    };
    let rest: Vec<String> = args[1..].to_vec();

    let result: Result<(), String> = (|| {
        match cmd.as_str() {
            "info" => {
                let save_path = resolve_save(&opts)?;
                cmd_info(&save_path)
            }
            "dump" => {
                let save_path = resolve_save(&opts)?;
                let (g, _) = save::load(&save_path).map_err(|e| e.to_string())?;
                let out = render_json(&save::dump(&g).map_err(|e| e.to_string())?);
                match rest.first() {
                    Some(file) => fs::write(file, out).map_err(|e| e.to_string())?,
                    None => println!("{out}"),
                }
                Ok(())
            }
            "apply" => {
                let file = rest.first().ok_or("apply expects a JSON file argument")?;
                let json: Json = serde_json::from_str(
                    &fs::read_to_string(file).map_err(|e| e.to_string())?,
                )
                .map_err(|e| format!("invalid JSON: {e}"))?;
                let save_path = resolve_save(&opts)?;
                write_with_backup(&save_path, |g| save::merge_root(g, &json))
            }
            "get" => {
                let path = rest.first().ok_or("get expects a path argument")?;
                let save_path = resolve_save(&opts)?;
                let (g, _) = save::load(&save_path).map_err(|e| e.to_string())?;
                println!("{}", render_json(&save::get_path(&g, path).map_err(|e| e.to_string())?));
                Ok(())
            }
            "set" => {
                let path = rest.first().ok_or("set expects a path argument")?;
                let raw = rest.get(1).ok_or("set expects a value (JSON) argument")?;
                let json: Json = serde_json::from_str(raw)
                    .map_err(|_| "value is not valid JSON (use e.g. 999999, true, \"RUS\", 0.5)".to_string())?;
                let save_path = resolve_save(&opts)?;
                write_with_backup(&save_path, |g| save::set_path(g, path, &json))
            }
            "decrypt" => {
                let out = rest.first().ok_or("decrypt expects an output file argument")?;
                let save_path = resolve_save(&opts)?;
                let raw = fs::read(&save_path).map_err(|e| e.to_string())?;
                let plain = crypto::decrypt_save(&raw).map_err(|e| e)?;
                fs::write(out, plain).map_err(|e| e.to_string())?;
                Ok(())
            }
            "encrypt" => {
                let inp = rest.first().ok_or("encrypt expects an input file argument")?;
                let plain = fs::read(inp).map_err(|e| e.to_string())?;
                let out = match rest.get(1) {
                    Some(o) => PathBuf::from(o),
                    None => {
                        let save_path = resolve_save(&opts)?;
                        save::create_backup(&save_path).map_err(|e| e.to_string())?;
                        save_path
                    }
                };
                let enc = crypto::encrypt_save(&plain);
                fs::write(&out, enc).map_err(|e| e.to_string())?;
                println!("wrote {}", out.display());
                Ok(())
            }
            "roundtrip" => {
                let save_path = resolve_save(&opts)?;
                let raw = fs::read(&save_path).map_err(|e| e.to_string())?;
                let plain_a = crypto::decrypt_save(&raw).map_err(|e| e)?;
                let g1 = nrbf::parse(&plain_a).map_err(|e| e.to_string())?;
                let reserialized = nrbf::Writer::new().write(&g1).map_err(|e| e.to_string())?;
                let g2 = nrbf::parse(&reserialized).map_err(|e| format!("re-parse failed: {e}"))?;
                let d1 = render_json(&save::dump(&g1).map_err(|e| e.to_string())?);
                let d2 = render_json(&save::dump(&g2).map_err(|e| e.to_string())?);
                if d1 == d2 {
                    println!("roundtrip OK (parse -> emit -> parse produced identical data)");
                    Ok(())
                } else {
                    Err("roundtrip MISMATCH - please report this as a bug".into())
                }
            }
            "backup" => {
                let save_path = resolve_save(&opts)?;
                let dst = save::create_backup(&save_path).map_err(|e| e.to_string())?;
                println!("backup created: {}", dst.display());
                Ok(())
            }
            "list-backups" => {
                let save_path = resolve_save(&opts)?;
                for p in save::list_backups(&save_path).map_err(|e| e.to_string())? {
                    println!("{}", p.display());
                }
                Ok(())
            }
            "restore" => {
                let name = rest.first().ok_or("restore expects the backup file path from list-backups")?;
                let name_path = PathBuf::from(name);
                if !name_path.exists() {
                    return Err(format!("backup not found: {name}"));
                }
                let save_path = resolve_save(&opts)?;
                let current = save::create_backup(&save_path).map_err(|e| e.to_string())?;
                fs::copy(&name_path, &save_path).map_err(|e| e.to_string())?;
                println!("restored from {name}; previous save backed up to {}", current.display());
                Ok(())
            }
            "list-slots" => {
                for p in save::list_slots() {
                    println!("{}", p.display());
                }
                Ok(())
            }
            other => Err(format!("unknown command: {other} (try --help)")),
        }
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
