# Changelog

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
