# ARCHITECTURE — read this before touching anything

This file explains how the repo is put together and, more importantly, **what
will silently kill the save editor if you don't know it**. Written 2026-11-19.

## What this repo is

Cargo **workspace** (root package `swgp2-tool` + member `mod/loader`):

| Binary/crate | Stack                | Purpose                              |
|---------------|----------------------|--------------------------------------|
| `swgp2-tool`  | eframe/egui (GUI)    | main GUI, no flags, single window    |
| `swgp2-tui`   | ratatui (terminal)   | same save editor in a terminal       |
| `swgp2-mcp`   | stdio JSON-RPC (MCP) | agent-facing control of the game/save|
| `swgp2-mod`   | native cdylib        | injected loader: loads managed `Swgp2Mod.dll` into the game's Mono and calls `Entry.Main()` (installs the in-game menu) |

The crate is a plain library crate; front-ends are thin. **All save writes go
through `src/app.rs::patch_save`** — never re-serialize a save any other way.
`mod/loader` is intentionally dependency-free (hand-written FFI).

## Build — the two traps that will eat your time

1. **The repo lives in OneDrive** (`C:\Users\stude\OneDrive\Рабочий стол\ivanovich`).
   Building directly in it repeatedly produced "cannot find .rlib" / linker
   errors (OneDrive on-demand file semantics + a non-ASCII path).
   **Always build with an external target dir**, e.g.:

   ```powershell
   $env:CARGO_TARGET_DIR="C:\Users\stude\swgp2-target"
   cargo build --release        # root package only
   cargo build --release -p swgp2-mod   # the in-game loader DLL
   ```

   `target/` is gitignored anyway. Do not try to "fix" the OneDrive path;
   just redirect CARGO_TARGET_DIR.

2. **No CI.** Builds are local-only Windows. Tests: `cargo test --release`.
   The roundtrip test and the car-DB test skip cleanly when the game/save
   isn't present. Do not assume CI will catch things.

## Safety model (non-negotiable)

- **The game must be closed while editing.** It overwrites `Supersave<N>.sav`
  on exit.
- Writes are **in-place byte patches**, never re-serialization (since 3.2.0).
  Only the bytes of the edited value change. A no-op patch must be
  byte-identical — covered by `patch_noop_is_byte_identical` test.
- Every write: backup first → AES-encrypt → re-decrypt+re-parse → compare
  dump against expected graph → on mismatch the backup is restored
  automatically. **Do not weaken this.**
- `app::patch_save` / `save::patch_target` are the only sanctioned write
  paths. Front-ends and MCP tools must call them. Do not "just" write bytes
  into the save elsewhere.
- String patches must be **equal length** (only same-length strings can be
  byte-patched). Longer/shorter strings are rejected — this is intentional.
- Backups live next to the save in `Backups\` and use `.savbak` with a
  timestamp; same-second collisions get a `.N` suffix.

## Fast map of the code

```
src/lib.rs            crate root: pub mod app, crypto, nrbf, save, tool
src/crypto.rs         AES-128-CBC, zero IV, PKCS7; KEY = "W9JU3JASLM7765D2" (16 ASCII bytes)
src/nrbf.rs           MS-NRBF parser+writer. Node arena, byte-identical re-emit. THE CORE — be extremely careful
src/save.rs           load/store, dump/get/set_path, patch_target/patch_save primitives, backups
src/app.rs            shared front-end logic (COMMON_FIELDS + per-field hints, patch_save orchestration)
src/tool/             game-data side (no GUI deps):
  game.rs             detect_game_dir, game_info, streaming_data_dir, EXE_NAME
  cars.rs             uselesslist.txt ranking parser (25 ranked cars), attach_skin_dirs
  catalog.rs          embedded 200-car catalog + IMD<id>.SW thumbnail loader
  catalog.txt         200-line embedded id|DEALER|MODEL|RALLY|REAL data
  skins.rs            skin file scan (CAR_SKINS)
  wiki.rs             CHAMPIONSHIPS / TRACKS / TIPS (curated) + best_picks(kind, n) (class-filtered)
  events.rs           ★ live level-file parser: weekends / cups / tracks / pilots /
                        weather / restrictions → wiki pages built from game data
  inject.rs           ★ Win32 DLL injector (CreateRemoteThread+LoadLibraryW); process
                        finder, module list, memory read/write/search (wildcard pattern)
  settings.rs         settings.json in %APPDATA%/swgp2-tool
  reset.rs            full-save wipe with recovery archives
  unity.rs            (pending) minimal Unity SerializedFile parser + Mesh reader (map → .glb)
  glb.rs              (pending) glTF 2.0 binary (.glb) writer
  analyze.rs          (pending) file_info/hexdump/strings/search/PE/NRBF probe/SW-thumbnail decode
mod/
  Swgp2Mod.cs         managed C# mod source: Unity IMGUI menu (car power, freeze Life, etc.)
  Swgp2Mod.dll        compiled mod, committed intentionally (build without Roslyn)
  loader/             swgp2-mod native cdylib (zero deps): waits for mono-2.0-bdwgc.dll,
                       mono_domain_assembly_open(Swgp2Mod.dll) → Entry.Main()
src/bin/swgp2-tool.rs GUI
src/bin/swgp2-tui.rs  TUI
src/bin/swgp2-mcp.rs  (pending) MCP server (stdio JSON-RPC 2.0) — the "do anything" agent surface
src/mcp.rs            (pending) MCP framework: registry, schemas, dispatch
```

## Data flow

`.sav → AES-decrypt → NRBF bytes → Graph ⇄ dump/get/set ⇄ Json`

Saves are BinaryFormatter streams; `src/nrbf.rs` parses them losslessly.
JSON is only a view/interchange format, never the storage format.

## Conventions

- All save-related errors are `SaveError(String)` (`save.rs`) / plain `String`
  at the front-end layer. Keep it that way; the GUI shows the message.
- Hint/description strings live in `app::COMMON_FIELDS` (path, label, hint) —
  the TUI renders the same tuple, so GUI and TUI stay in sync for free.
- Wiki data sources must come from the game's own files (see GAME_FILES.md).
  Never invent stats; unknown → say so.
- The game's version matters for file formats. This was written against
  **Super Woden GP 2 v1.31b (build 18323290), Unity 2021.3.29f1**. If the
  game updates, level-file records may change — the events parser degrades
  to "no data found" and the wiki falls back to curated lists (see wiki.rs).

## Versioning

- Version in `Cargo.toml` only; binaries report it via `env!("CARGO_PKG_VERSION")`.
- `Cargo.lock` should be committed bumped together (the 3.4.0 commit forgot it).
- CHANGELOG entries are per-release; README mirrors user-visible features.
- Release tags `v*` are created manually from the Windows build machine.
