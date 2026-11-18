# REPO_GRAPH — swgp2-save-editor (read this instead of the source)

Crate layout: `lib (swgp2_save_editor)` shared by two flag-free binaries.
Everything works on a parsed **Graph**; JSON is only a view/interchange format.

```
src/bin/swgp2.rs      GUI (eframe/egui). run: `swgp2`            (no args)
src/bin/swgp2-tui.rs  TUI (ratatui/crossterm). run: `swgp2-tui`  (no args)
        │ both call ▼
src/app.rs            shared front-end logic (flag-free)
  load(path)                      decrypt + parse → Graph
  field_text(g, path)             value → display string
  parse_value(s)                  editor text → Json (number/bool/string)
  save_with_backup(path, edits)   edit→dump→backup→store→RE-VERIFY→(restore on fail)
  set_single / apply_json_file    path-edit / whole-document merge, same safety
  dump_json(path)                 pretty JSON string
  restore_backup(save, bak)       copies .savbak over save (backs up current first)
  slot_choices()                  detected save slots (falls back to slot 0)
  COMMON_FIELDS                   the 9 editor fields (Economy, Coins, StatsData_.*…)
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
src/nrbf.rs           MS-NRBF (.NET BinaryFormatter) parser+writer, 37252 B, THE CORE
  parse(&[u8]) → Graph            Node arena (Vec<Node>), root: usize, ids map
  Writer::new().write(&g)         byte-identical re-emit (roundtrip-lossless)
  Graph { arena, root, … }  Node::{ Class{member_names,…}, Object… }  Value/Prim enums
  dump: node_to_json (in save.rs) walks Graph → Json
        │ uses ▼
src/crypto.rs         AES-128-CBC, zero IV, PKCS7; KEY = 16 ASCII bytes "W9JU3JASLM7765D2"
  encrypt_save / decrypt_save(&[u8]) -> Result<Vec<u8>, String>
```

Data flow: `.sav → decrypt → NRBF bytes → Graph ⇄ dump/get/set ⇄ Json`.
Writes always go: edit Graph → JSON-dump compare expectation → backup → store →
re-decrypt+re-parse → JSON-compare; mismatch ⇒ original restored from backup.

Safety model: game must be CLOSED (it overwrites saves on exit). Backups live in
`<save dir>\Backups\*.savbak`. `save_with_backup` is the only write path that both
binaries use for user data — do not bypass it.

Tests: `cargo test --release` → tests/roundtrip.rs verifies load→save→dump is a
fixed point on a temp copy of the real save (skips if no save present).

Known quirks: char/DateTime/Decimal stored as strings; member order preserved;
paths are case-sensitive dot-separated member names, array indices numeric.
