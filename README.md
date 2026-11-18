# swgp2-tool

All-in-one tool for **Super Woden GP 2** (ViJuDa, Unity + Mono).

Single native binary. No runtime, no Python, no .NET required. Just run it:

- **`swgp2-tool.exe`** — 7z-style GUI: white, simple, everything two clicks away
- **`swgp2-tui.exe`** — the same save editor in a terminal

No flags. Double-click and go.

## What's inside

### Save Editor

The game stores saves at

```
%USERPROFILE%\AppData\LocalLow\ViJuDa\Super Woden GP 2\Save\Supersave<N>.sav
```

A `.sav` is a **.NET BinaryFormatter** stream encrypted with **AES-128-CBC**
(zero IV, PKCS7 padding, key = the 16-byte ASCII string `W9JU3JASLM7765D2`).

- Edit Credits / Coins / Language / Difficulty / races / victories / money won
  & spent / distance inline, then **Save** (auto-backup + auto-verify)
- Top-toolbar **Load / Reload / Save** for the save file itself
- Set **any** field by path (`GarageSlots.0.CarID`,
  `LeagueStatus_.ChampPrices.2`, …) in the *Advanced* section
- *Raw JSON*: dump the whole save to a file / apply a JSON document back / preview
- *Backups*: timestamped backup on **every** write; list / restore / delete
  from inside the app; restore auto-backs-up the current save first
- *Decrypt / Encrypt*: raw `.sav` ⇄ BinaryFormatter stream
- *Verify*: parse → emit → parse round-trip check; a written save that fails
  verification is restored from backup automatically
- Writes are **in-place byte patches** (since 3.2.0) - never re-serialized;
  only the bytes of the value you edit change, so the game always loads it
  exactly like the original file
- Quick cheats: `+1,000,000 Cr`, `Cr = 999,999,999`, `Coins = 999,999`

### Reset Saves

Full save wipe with a safety net: **Reset ALL saves** archives every `.sav`
into `Backups\FullReset-<stamp>\` *before* deleting (two-step confirm).
Recovery archives can be **restored** (current saves are moved aside first) or
**deleted forever** from the same screen.

### Cars

Power ranking parsed from the game's `uselesslist.txt` (25 cars with CV, top
speed and year), with **PNG/TGA preview of the actual car skin** from
`StreamingAssets/CAR_SKINS`. Sortable columns, live filter, rally-car handling —
100% of the ranked cars resolve to their skin folders.

### Car Skins

Browser over all ~640 skin files in `CAR_SKINS` (renders and `Car_bounds`
masks): brand → car → file, filterable, with full-size preview and in-place
navigation across a car's skins.

### Game

- Auto-detects the portable game folder (e.g. `F:\games\SWGP2`) or lets you pick it
  (remembered in `%APPDATA%\swgp2-tool\settings.json`)
- Game info: product/company, exe size & mtime, skin/save/backup counts,
  `boot.config` viewer
- One-click: **Launch game**, open game folder, save folder, Backups,
  `CustomMusic`, `uselesslist.txt`, `Credits.txt`
- **Back up ALL save slots** in one click

### Wiki

Built-in reference compiled from the game's own data files: the official
25-car power ranking (uselesslist.txt) with CV/top speed/year and rally class,
brand catalog from CAR_SKINS, all 8 championships with best-car picks (by
official CV/speed), plus tips. No invented meta - everything is the game's
data, class-restricted championships without official stats say so.

### About

Version, repo link, safety model, and the game's own `Credits.txt` rendered in-app.

**The game must be closed while editing** — otherwise it will overwrite the
file on exit.

## Building from source

Rust toolchain only:

```
cargo build --release
```

produces `target/release/swgp2-tool[.exe]` (GUI) and `target/release/swgp2-tui[.exe]`
— single static binaries, nothing else needed.

Tests: `cargo test --release` (round-trip check on a temp copy of your save, a
car-DB parse/skin-match check; both skip cleanly when the game isn't installed here).

## Releases

Windows binaries are built locally (no CI) and attached to GitHub Releases.
This is a Windows game; Linux/macOS builds are not produced.

## File format / internals

```
.sav  =  AES-128-CBC( KEY = "W9JU3JASLM7765D2", IV = 0x00*16, PKCS7 )
       -> BinaryFormatter stream (MS-NRBF subset: SerializedStreamHeader,
          BinaryLibrary, ClassWithMembersAndTypes, SystemClassWithMembersAndTypes,
          ClassWithId, BinaryObjectString, BinaryArray, ArraySinglePrimitive,
          ArraySingleString, ArraySingleObject, MemberPrimitiveTyped,
          MemberReference, ObjectNull, MessageEnd)
```

Parsing is lossless: parse → emit → parse produces identical data (verified by
the test suite). See **REPO_GRAPH.md** for a compact map of the codebase.

## Disclaimer

Unofficial fan tool. Super Woden GP 2 is the property of ViJuDa / the game
developer. Provided as-is for interoperability with your own local saves; use
at your own risk. Not affiliated with, endorsed by, or associated with the
game's author or publisher.
