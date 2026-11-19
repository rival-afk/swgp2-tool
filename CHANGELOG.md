# Changelog

## 3.5.0 - 2026-11-19

**New: in-game mod.** A small native helper `swgp2_mod.dll` is injected into
the running game and loads `Swgp2Mod.dll` (built from `mod/Swgp2Mod.cs`,
committed for convenience). No menu, no UI - pure keyboard cheats:

- **F7** - instant repair (Life restored to the race-start snapshot)
- **F8 / F9** - acceleration +/-0.5x (0.5x..5.0x), applied every frame to the
  car's `CarPower`; white text with the current multiplier fades away at the
  center of the screen (~3 seconds)
- **F6** - disable race filters: clears CV/year/tag/dealership limits of all
  loaded events (`LeagueData`/`WeekendData`/`RallyData`) at runtime; white
  "race filters disabled" text. One-way per game session; restart to re-enable

The mod loads via the game's own Mono runtime (embed API), zero DX hooks.
Manage it from the new **In-Game Mod** section: Install / Uninstall / Inject,
game-running status and a tail of `%APPDATA%\swgp2-tool\mod.log`.

- **New MCP server** (`swgp2-mcp`, stdio JSON-RPC 2.0) with ~30 tools:
  save get/set/dump/apply/apply-json/backup/restore/verify (mutating tools
  refuse while the game is running), game info/launch, live level0 events
  scan, wiki best-cars picks, catalog, file info/hexdump/strings/search/copy,
  process/memory tools (find, modules, read/write/search with wildcards),
  DLL injection and the mod install/inject/status helpers.
- **File analyzer** (`analyze.rs`): magic-byte type detection (PE/NRBF/PNG/
  TGA/JPG/ZIP/7z/Unity SerializedFile/...), entropy, ASCII/UTF-16 strings,
  wildcard pattern search and a full PE parser (sections, exports, imports).
- **Wiki is built live from the game's own files** (level0 parsed on the
  fly): new *Weekends*, *Endurance* and *Rallies* pages; the Tracks page is
  live too. Every event shows its restrictions (class/dealership/pilot/
  weather) and per-class best-car picks ranked by official CV, then top
  speed. Championship pages are now class-filtered (BTC/DTC/JTC).
- **Cars section is a 4-column card catalog** (12/page) with skin previews,
  filter and sort - replacing the flat table.
- **Save editor**: every field now carries a hint (allowed values or numeric
  range) in both the GUI and the TUI; the Advanced set-by-path box shows the
  same hint for known paths (case-normalised for arrays).
- **Game folder auto-detection** fixed: it honours the persisted choice from
  `%APPDATA%\swgp2-tool\settings.json` and scans Downloads/Desktop/Documents
  one level down (portable-game zip archives are found without setup).
- **Safety**: `save_set`/`save_apply`/`save_restore` via MCP and the GUI
  refuse to run while the game is running.

## 3.4.0 - 2026-11-18

- **Wiki Cars page rebuilt**: now shows ALL 200 shop cars (from the game's own
  `StreamingAssets/BrandLogos/Original_names.txt` + dealership preview images
  from `StreamingAssets/CarThumbnails/IMD*.SW`, which map 1:1 to the save's
  CarID). Filter by dealer or text, click any card for full specs, real-world
  counterpart (community IGCD list where catalogued), official ranking when the
  game ranks it, dealer, class, rally variant, skin folder.
- **New Tracks page**: every race/event/track/stage from the championships
  screen, grouped with class badges, and recommended cars for each.
- Dealer filter, game-preview images load everywhere in the wiki, dealer
  details are expanded.

## 3.3.0 - 2026-11-18

- New **Wiki** section: in-game reference built from the game's own data files -
  - *Cars*: the official 25-car power ranking (uselesslist.txt) with brand,
    year, CV, top speed, rally/circuit class, skin variant count, per-car
    power/speed ranks and skin previews. Click a row for the full profile.
  - *Championships*: all 8 in-game championships with rally/circuit/touring/
    beginner class markers and best-car picks ranked by the official CV/speed
    figures (Rookie Challenge, Junior Series, Super DTC, ONLY BTC, 300s Rally,
    400s WORLD RALLY, SUPER WORLD RALLY, JTC SUPER BATTLE).
  - *Tips*: mechanics-based tips (car wear, editing safety, backups, rankings).
- All stats are read from the game's data files - nothing is invented; class-
  restricted (BTC/DTC/JTC) championships have no official stats and say so.

## 3.2.0 - 2026-11-18

**Critical fix:** the tool no longer re-serializes the save at all. The old
write path (parse → mutate graph → re-emit NRBF → encrypt) produced a stream
that the game's BinaryFormatter partially rejected, so the game silently
"forgot" parts of the profile (garage, stats, …) while our own reader saw a
consistent file. Writes are now **in-place byte patches**: every byte the user
didn't edit stays identical to the game-written original. Safety model is
provably intact — `patch_noop_is_byte_identical` test.

- `save::patch_target` / `save::patch_save` resolve any path to the exact
  byte offset of a fixed-size primitive or an equal-length string payload
  (`Economy`, `Coins`, `StatsData_.*`, `GarageSlots.N.*`, array elements, …).
- `apply` JSON: only scalar leaves are patched; structural changes are rejected.
- Language strings are patchable when their byte length is unchanged (e.g. RUS→ENG).
- Backups keep a .N suffix on same-second collisions.
- Verified by .NET Framework BinaryFormatter probe (examples) and new
  `patch_noop_is_byte_identical` / `patch_and_reload` tests.
- The local save was restored from backup (pre-corruption) with the fix applied.

## 3.1.0 - 2026-11-18

- New **Reset Saves** section (menu: Tools → Reset ALL saves…): full wipe of
  all saves with a recovery archive — every `.sav` is copied into
  `Backups\FullReset-<stamp>\` before deletion (two-step confirm). Archives
  can be restored later (current saves are moved aside into a pre-restore
  archive first) or deleted forever. Covered by tests/reset.rs.

## 3.0.0 - 2026-11-18

**Repo renamed** `rival-afk/swgp2-save-editor` → `rival-afk/swgp2-tool`.
**Breaking:** `swgp2.exe` (simple GUI) is replaced by the full `swgp2-tool.exe`.

- `swgp2-tool` — 7z-style GUI (white, menu bar + top toolbar with
  **Load / Reload / Save**, left section rail, status bar):
  - *Save Editor*: inline fields, quick cheats (+Cr, Cr/Coins max), advanced
    set-by-path, raw JSON dump/apply/preview, decrypt/encrypt, Verify,
    backups (create/restore/delete + Back up ALL slots)
  - *Cars*: sortable power ranking from the game's `uselesslist.txt` (25 cars,
    CV/speed/year) with **PNG/TGA skin preview** and rally-car handling
  - *Car Skins*: browser over all ~640 skin files (brand → car → file) with preview
  - *Game*: auto-detect/choose the portable game folder (persisted in
    `%APPDATA%/swgp2-tool/settings.json`), game info (exe, skins, saves,
    boot.config), one-click Launch game / open folders / Back up ALL saves
  - *About*: repo, safety model, in-game Credits.txt
- Tests: roundtrip test + cars DB parse/skin-match test (both skip gracefully
  when the game/save isn't there).
- Builds are local-only; no CI. Windows binaries in the Release.

## 2.0.0 - 2026-11-18

**Breaking:** the flag-based CLI (`--slot`, `--save`, `dump`, `set`, …) was
removed. Everything it did is now available with zero arguments:

- `swgp2` — new simple GUI: slot picker, inline editing of common fields,
  advanced set-by-path, raw-JSON dump/apply/preview, backup & restore.
- `swgp2-tui` — new terminal UI with the same functionality.

Internally the crate is split into a library (`crypto`, `nrbf`, `save`, `app`)
shared by both front-ends. Write path unchanged and always safe:
edit → backup → write → re-parse & verify (auto-restore on mismatch).
`cargo test --release` runs a round-trip test on a temp copy of your save.
