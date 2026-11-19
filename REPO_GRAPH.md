# REPO_GRAPH — swgp2-tool (read this instead of the source)

Crate layout: cargo workspace `members = [".", "mod/loader"]` - root lib (`swgp2_tool`) shared by two flag-free binaries + the in-game mod loader.
Everything save-related works on a parsed **Graph**; JSON is only a view/interchange format.

```
src/bin/swgp2-tool.rs  GUI (eframe/egui). run: `swgp2-tool`  (no args)
                        menu bar + toolbar(Load/Reload/Save) + left rail sections:
                        Save Editor | Cars | Car Skins | Wiki | Game | Reset | About
                        top toolbar Save buttons act on current save; quick cheats;
                        cars: 4-column paged card catalog (12/page) + skin preview;
                        Wiki: cars gallery (200 shop cars), championships (class-
                        filtered), live Tracks/Weekends/Endurance/Rallies (parsed
                        from level0 on the fly), tips
src/bin/swgp2-tui.rs   TUI (ratatui/crossterm). run: `swgp2-tui`        (no args)
        │ both call ▼
src/app.rs            shared front-end logic (flag-free)
  load(path)                      decrypt + parse → Graph
  field_text(g, path)             value → display string
  parse_value(s)                  editor text → Json (number/bool/string)
  save_with_backup is GONE (was unsafe) - see `patch_save` / `set_single`
        (in-place byte patches by byte offset; no re-serialization)
  set_single / apply_json_file    path-edit / whole-document merge, same safety
  dump_json(path)                 pretty JSON string
  restore_backup(save, bak)       copies .savbak over save (backs current first)
  slot_choices()                  detected save slots (falls back to slot 0)
  COMMON_FIELDS                   the 10 editor fields (Economy, Coins, StatsData_.*…)
        │ uses ▼
src/tool/            game-data side of the tool (no GUI deps)
  game.rs    detect_game_dir() - shallow scan of drive roots for "Super Woden GP 2.exe"
             game_info(dir)      - product/company/boot.config/exe mtime/skin-save counts
             streaming_data_dir / skins_root / custom_music_dir paths
  reset.rs   save_dir(), saves_in(dir), reset_all()
             - stage_reset: copy all .sav → Backups\FullReset-<stamp>\, then delete
             list_archives / restore_archive (current saves moved aside to a
             FullReset-pre-restore-<stamp> archive first) / delete_archive
             covered by tests/reset.rs
  cars.rs    CarEntry::parse_file(uselesslist.txt) - "[-R]NAME cv CV kmh Kmh || year"
             (leading "-R" = rally → folder key "R<NAME>"); attach_skin_dirs matches folders
             under CAR_SKINS/<BRAND>/<CAR> (case/alnum-insensitive); 25/25 match on install
  skins.rs   SkinEntry scan(skins_root) - every .png/.tga/.jpg under CAR_SKINS, kind=Render|Mask
  settings.rs settings.json in %APPDATA%/swgp2-tool - persists chosen game_dir
  wiki.rs      Wiki content from game files: CHAMPIONSHIPS + TRACKS tables (8 leagues
              kind Rally/Circuit/Touring/Beginner + class filter BTC/DTC/JTC,
              ranked by CV desc then speed desc), 200-car catalog with
              dealer-compatible unicode dealer names (Original_names.txt), TIPS list
              (tracks available as fallback if the live parser fails)
  catalog.rs   EMBEDDED 200-car catalog (id=DEALER#MODEL tables) + IMD<id>.SW
              thumbnail loader (PNG inside .SW container) + uselesslist stats merge
  catalog.txt  200-line embedded catalog data (id|DEALER|MODEL|RALLY|REAL-WORLD-NAME)
  events.rs    * live parser for the game's event records in level* (anchor
              `01 00 00 00 01 00 00 00` + u32 K + lp-string name):
              K 602=weekend, 271=track, 262=pilot, 86=cup, 53=restriction tag,
              529=weather. Groups restrictions per event; used by the
              Tracks/Weekends/Endurance/Rallies wiki pages
  inject.rs    * Win32 DLL injector (CreateRemoteThread + kernel32!LoadLibraryW);
              process finder (ToolHelp), module list (ToolHelp), raw memory
              read/write/search (VirtualQueryEx + wildcard pattern)
  loader/      (mod/loader/) swgp2-mod native cdylib - injected into the game;
              loads managed `mod/Swgp2Mod.dll` into Mono via the embed API
              (domain_assembly_open -> Swgp2Mod.Entry.Main); zero dependencies
        │ uses ▼
src/save.rs           high-level save handling (all errors = SaveError(String))
  load(path) → (Graph, raw)       read → crypto::decrypt → nrbf::parse
  store(path, g)                  nrbf::Writer::write → crypto::encrypt → fs::write
  dump(g) → Json | get_path / set_path(g, "a.b.0.c", Json)  (List<T> traversed transparently)
  merge_root(g, json)             deep-merge dump-shaped JSON back in
  default_save_path / list_slots                       %USERPROFILE%\AppData\LocalLow\ViJuDa\Super Woden GP 2\Save\Supersave<N>.sav
  create_backup / list_backups / backup_dir_for        sibling `Backups\<stem>.<yyyymmdd-hhmmss>.savbak`
  chrono_stamp()                  timestamp w/o chrono crate
        │ uses ▼
src/nrbf.rs           MS-NRBF (.NET BinaryFormatter) parser+writer (~37 KB, THE CORE)
  parse(&[u8]) → Graph            Node arena (Vec<Node>), root: usize, ids map
  Writer::new().write(&g)         byte-identical re-emit (roundtrip-lossless)
  Graph { arena, root, … }  Node::{ Class{…} }  Value/Prim enums
        │ uses ▼
src/crypto.rs         AES-128-CBC, zero IV, PKCS7; KEY = 16 ASCII bytes "W9JU3JASLM7765D2"
  encrypt_save / decrypt_save(&[u8]) -> Result<Vec<u8>, String>
```

Data flow: `.sav → decrypt → NRBF bytes → Graph ⇄ dump/get/set ⇄ Json`.
WRITES ARE IN-PLACE BYTE PATCHES (since 3.2.0): resolve path → primitive
byte offset (via `Graph::record_pos` / `Graph::child_pos`) → overwrite just
those bytes → re-encrypt. No re-serialization, ever. A no-op patch must be
byte-identical (tested). Whole-graph dump → backup → AES-write → re-parse →
JSON-compare verify remains as a final guard; restore-on-failure is automatic.

Safety model: game must be CLOSED (it overwrites saves on exit). Backups live in
`<save dir>\Backups\*.savbak`. `patch_save` is the only write path that any
front-end uses for user data — do not bypass it.

Game assets consumed (portable Unity install, e.g. `F:\games\SWGP2`):
`Super Woden GP 2_Data\StreamingAssets\Data\uselesslist.txt` (car power ranking),
`...\CAR_SKINS\<BRAND>\<CAR>\*_base.png|tga` (skins), `...\Credits.txt`,
`...\CustomMusic\` (user mp3s), `app.info`, `boot.config`.

Tests: `cargo test --release` → tests/roundtrip.rs (save roundtrip on a temp copy;
skips w/o local save) + tests/cars.rs (car DB parse / 25-25 skin match; skips w/o game).

Known quirks: char/DateTime/Decimal stored as strings; member order preserved;
paths are case-sensitive dot-separated member names, array indices numeric;
`uselesslist.txt` is not valid UTF-8 (mojibake) - parser filters to ASCII
structure around numbers. Builds are LOCAL ONLY (Windows); no CI workflows.
