# swgp2-save-editor

Save-file editor for **Super Woden GP 2** (ViJuDa, Unity + Mono).

Single native binary. No runtime, no Python, no .NET required. Two ways to run it:

- **`swgp2`** — simple GUI. Double-click it (or run it with no arguments) and edit.
- **`swgp2-tui`** — same thing in a terminal. Just run it.

Neither takes any flags — launch and go.

> Status: **2.0**. The old flag-based CLI was removed; everything it could do is
> now in the GUI and TUI (edit fields, set any path, dump/apply JSON, backups).

## What it does

The game stores saves at

```
%USERPROFILE%\AppData\LocalLow\ViJuDa\Super Woden GP 2\Save\Supersave<N>.sav
```

A `.sav` is a **.NET BinaryFormatter** stream encrypted with **AES-128-CBC**
(zero IV, PKCS7 padding, UTF-8 key, key is the 16-byte ASCII string
`W9JU3JASLM7765D2`). This tool:

- shows and edits the common fields (credits, coins, difficulty, races,
  victories, money won/spent, distance, …)
- can set **any** field by path (`GarageSlots.0.CarID`,
  `LeagueStatus_.ChampPrices.2`, `StatsData_.RacesCompleted`, …)
- dumps the entire save to JSON and merges a JSON document back in
- creates timestamped backups on **every** write and verifies the written file
  by re-parsing it — if verification fails the backup is restored automatically
- restores backups from within the app

**The game must be closed while editing** — otherwise it will overwrite the
file on exit.

## GUI (`swgp2`)

- Slot picker + "Open file…" for a non-default `.sav`
- Edit Credits / Coins / Language / Difficulty / stats inline, press **Save**
  (auto-creates a backup)
- *Advanced – set any path*: `GarageSlots.0.CarID = 5` → **Set**
- *Raw JSON*: dump to file, apply from file, or preview
- *Backups*: back up now / restore a previous backup
- Status bar shows what happened; errors are red

## TUI (`swgp2-tui`)

```
↑/↓ select field   Enter edit   s save (auto-backup)   Tab switch slot
b backup           r restore latest backup (asks y/n)   d dump JSON → ./swgp2-dump.json
q quit
```

## Building from source

Rust toolchain only:

```
cargo build --release
```

produces `target/release/swgp2[.exe]` (GUI) and `target/release/swgp2-tui[.exe]`
(TUI) — single static binaries, nothing else needed.

Tests: `cargo test --release` (round-trip check on a temp copy of your save;
skips cleanly if there is no save on this machine).

## How it works / file format

```
.sav  =  AES-128-CBC( KEY = "W9JU3JASLM7765D2", IV = 0x00*16, PKCS7 )
       -> BinaryFormatter stream (MS-NRBF subset: SerializedStreamHeader,
          BinaryLibrary, ClassWithMembersAndTypes, SystemClassWithMembersAndTypes,
          ClassWithId, BinaryObjectString, BinaryArray, ArraySinglePrimitive,
          ArraySingleString, ArraySingleObject, MemberPrimitiveTyped,
          MemberReference, ObjectNull, MessageEnd)
```

Parsing is lossless: parse → emit → parse produces identical data (verified by
the test suite). Addresses of fields are byte-identical between parse and emit,
so nothing is normalized or lost. Forks/modiffs: see **REPO_GRAPH.md** for a
compact map of the codebase (written to save AI-assistant tokens).

## Releases

GitHub Releases carry prebuilt binaries. Tagging `v*` builds Windows, Linux and
macOS binaries via `.github/workflows/release.yml` and attaches them to the release.

## Disclaimer

Unofficial fan tool. Super Woden GP 2 is the property of ViJuDa / the game
developer. Provided as-is for interoperability with your own local saves; use
at your own risk. Not affiliated with, endorsed by, or associated with the
game's author or publisher.
