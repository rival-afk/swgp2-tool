# swgp2-save-editor

Save-file editor / decoder for **Super Woden GP 2** (ViJuDa, Unity + Mono).

Single native binary. No runtime, no Python, no .NET required — just download
and run `swgp2-save-editor` (CLI).

> Status: **early / intermediate.** Core works and is verified round-trip,
> but the command surface and README may change before a 1.0.

## What it does

The game stores saves at

```
%USERPROFILE%\AppData\LocalLow\ViJuDa\Super Woden GP 2\Save\Supersave<N>.sav
```

A `.sav` is a **.NET BinaryFormatter** stream encrypted with **AES-128-CBC**
(zero IV, PKCS7 padding, UTF-8 key, key is the 16-byte ASCII string
`W9JU3JASLM7765D2`). This tool:

- decrypts and re-encrypts `.sav` files (`decrypt` / `encrypt`)
- parses and re-serializes the BinaryFormatter stream **losslessly**
  (parse → emit → parse produces identical data; `roundtrip` verifies it)
- prints the whole save as JSON (`dump`) and merges a JSON document back in
  (`apply`) — this covers *every* field in the save, including cars, stars,
  championship progress, stat data, etc.
- gets/sets individual fields by path (`get` / `set`), e.g. `Economy` (credits)
- creates timestamped backups automatically on every write (`backup`,
  `list-backups`, `restore`) and verifies the written file by re-parsing it —
  if verification fails it restores the backup automatically

## Usage

```
swgp2-save-editor [--slot N | --save PATH] <command> [args...]

info                      Save overview (credits, coins, cars, stats)
dump [FILE]               Decrypt & print the whole save as JSON
apply <FILE.json>         Merge a JSON document (as by `dump`) into the save
get  <PATH>               Print a single value, e.g. `get Economy`
set  <PATH> <JSON>        Set a value, e.g. `set Economy 999999`
decrypt <OUT>             Decrypt to a raw BinaryFormatter stream
encrypt <IN> [OUT]        Encrypt a stream back into a .sav
roundtrip                 Verify lossless round-trip on this save
backup                    Create a timestamped backup
list-backups              List backups made by this tool
restore <FILE>            Restore a backup (backs up current save first)
```

Path syntax: `Member` for top-level members, `.` for nesting, numeric index
for arrays: `GarageSlots.0.CarID`, `StatsData_.RacesCompleted`,
`LeagueStatus_.ChampPrices.2`, `WeekendStarGroups.3.Stars.0`.

Paths traverse `List<T>` transparently (`GarageSlots.0.CarID` works even
though the save serializes it as `System.Collections.Generic.List<GarageSlot>`).

**The game must be closed while editing** — otherwise it will overwrite the
file on exit.

## Safety

Every write command first creates a timestamped backup in a `Backups` folder
next to the save (`Supersave0.20261116-094619.savbak`). After writing, the tool
re-decrypts and re-parses the file and compares it with the intended data; if
they differ, the backup is restored automatically.

## Building from source

Rust toolchain only:

```
cargo build --release
```

produces `target/release/swgp2-save-editor[.exe]` — a single static binary.

## How it works / file format

```
.sav  =  AES-128-CBC( KEY = "W9JU3JASLM7765D2", IV = 0x00*16, PKCS7 )
       -> BinaryFormatter stream (MS-NRBF subset:
          SerializedStreamHeader, BinaryLibrary, ClassWithMembersAndTypes,
          SystemClassWithMembersAndTypes, ClassWithId, BinaryObjectString,
          BinaryArray, ArraySinglePrimitive, ArraySingleString,
          ArraySingleObject, MemberPrimitiveTyped, MemberReference,
          ObjectNull, MessageEnd)
```

The save contains a `SaveObject` with `Economy` (in-game credits, shown as
"Cr"), `Coins`, `GarageSlots`, `LeagueStatus_`, `RallyStatus_`, `StatsData_`
and so on. Addresses of fields are byte-identical between parse and emit, so
nothing is normalized or lost.

## Disclaimer

Unofficial fan tool. Super Woden GP 2 is the property of ViJuDa / the Game
Developer. Provided as-is for interoperability with your own local saves; use
at your own risk. Not affiliated with, endorsed by, or associated with the
game's author or publisher.
