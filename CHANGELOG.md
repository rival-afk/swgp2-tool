# Changelog

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
