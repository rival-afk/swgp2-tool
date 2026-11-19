# GAME FILES — Super Woden GP 2 v1.31b (Unity 2021.3.29f1)

Portable install layout: `<game>/Super Woden GP 2.exe`, `<game>/Super Woden GP 2_Data/`,
`<game>/MonoBleedingEdge/`. Verified against
`C:\Users\stude\Downloads\Super.Woden.GP.2 v1.31b (Build 18323290)`.

Save location (fixed by the game, not configurable):

```
%USERPROFILE%\AppData\LocalLow\ViJuDa\Super Woden GP 2\Save\Supersave<N>.sav
                                                    Backups\*.savbak   (ours + the game's own backup dir)
```

## StreamingAssets/Data

| File | Content |
|---|---|
| `uselesslist.txt` | official ranked-car power list, 25 entries: `NAME <n> CV  <n> Kmh  \|\|  YEAR`. Leading `®` (0xAE) = rally-class entry. Not valid UTF-8 — parser filters ASCII structure around numbers. |
| `Credits.txt` | in-game credits |
| `elf.dat` | **the real Assembly-CSharp** — x86 mixed-mode PE renamed `.dat`. 595 456 B, 622 typedefs (vs 653 in `Managed/Assembly-CSharp.dll`). No exports; imports mscoree.dll only. Contains the game logic; the Managed copy is likely the integrity-check decoy (or vice-versa). Don't ship/modify it. |
| `doplhin.dat` | default **SaveObject** serialized as raw NRBF (not encrypted). Fresh-save template. Parses with our `nrbf::parse`. |

## StreamingAssets (other)

- `BrandLogos/Original_names.txt` — `id=DEALER#MODEL`, leading `®` = rally
  variant. 200 shop cars. The id equals the save's `CarID`/`IMD###`.
- `CarThumbnails/IMD<id>.SW` — dealership preview per car. It is a **PNG with
  a custom extension** (verified; loader falls back to `image::open`).
- `CAR_SKINS/<BRAND>/<CAR>/*_base.png|tga` — skins (~640 files); `Car_bounds`
  masks are excluded from previews by default. Rally variants live in folders
  prefixed `R` (e.g. `RSHUKUFUKU`). `CAR_SKINS` brand folders often diverge
  from dealer name case.
- `CustomMusic/` — user mp3s.
- `_DIPSWITCHES.txt`, `DipSwitches instructions.txt` — game-supported dip
  switches: rename `_DIPSWITCHES.txt` → `DIPSWITCHES.txt`, keep line order.
  Known switches: `ARCADE_MODE_FORCE_CRT=TRUE`, `MENUS_POSTPROCESSING_FX=TRUE`.
  The game recreates `_DIPSWITCHES.txt` (as reference) on update.
- `_pilot_names.txt` — 25 pilots, `NN=NAME#cc` (country code). These pilot
  names also appear inside level files as championship/round entries.

## Level files (`level0` … `level49`) — the big one

Serialized scenes (Unity **SerializedFile, format 22, Unity 2021.3.29f1**,
little-endian, ~20 MB each, 50 of them). `level0` contains the main menu
scene (every level re-includes it, so strings appear in all of them).

**String quirk:** the file is UTF-8 (names contain `é`/`ñ`); most strings
are length-prefixed (`u32 len` + bytes + `\0`), some raw. Non-ASCII splits
naive ASCII tokenizers — use UTF-8-aware extraction.

### Serialized "record" structure (reverse-engineered 2026-11-19)

Menu entries (championship races, weekends, rallies, TimeTrial) are NOT plain
Unity objects; they are opaque binary records with this shape:

```
... header-ish bytes ...
01 00 00 00 01 00 00 00 <K:u32>   ← the anchor; K identifies the record type
<some ints/floats/ptrs>           ← usually 12–60 bytes
<len:u32><name utf-8><\0>         ← the displayed name
... trailing data ...
```

Observed `K` values and meaning (level0, both menu copies):

| K | Kind | Entries |
|---|---|---|
| 0x025a (602) | **Weekend events** | Miniring 6H, Greenlands 24H, Eisenach Trophy, Plattsburg Sunset, Stock 300s Cars Meeting, 400's / 500's RACE CARS CHALLENGE, Italian Road Car Fest, Speed festival Junior/Senior, Sivi IV Cup, Classic Coupé Showdown, West Millswood 8H, Japanese Stock Cars Meeting, Plattsburg 24H, Denise Beach Trophy, Natsuko Circuit 24H, Small-Van-o-rama, 50's to 70's SMALL CARS, Muscle Car Weekend, Astral Speedway Trophy, Le Fantomas Danse, OVAL SERIES … |
| 0x10f (271) | **Venues / tracks** (circuits, rally stages, ovals) | Astral Speedway, Natsuko Circuit, Plattsburg Raceway/Sunset, Muffring Short, Sakae Ring, Firewheel Ring, Saturn Ring, Miniring, Greenlands, West Millswood, Denise Avenue, Lima…, Snowy Chicken, Limestone Hatch, Sweden S1–S4, Finland S1–S3, Argentina S1/S2, Kenya S1–S4, Spain S1–S4, Usa Hillclimb S1 — i.e. everything the TRACKS list contains |
| 0x106 (262) | **Pilots** (championship/round names) | Caitlyn Glover, Philipp Mangeng, THOMAS FLORES, YUN HUANG, Nias Bernhard, JUSTIN DAVIES, Javi Bonus, OLIVER MARS, Luciano Di Moze, Kaneko Shinju, Börje Fornberg, Andrés Pampillon, Jean-Baptiste Dujardin, LEANDRO VERGARA, Ben Chalcraft, DIANA PROAÑO, HATTIE ANDERSON, … |
| 0x56 (86) | **Cup/Trophy championships** | Knights Cup, Moon R-CUP, ONLY BTC CHAMPIONSHIP, GROUP 5 LEGENDS, Super DTC CHAMPIONSHIP, Junior Cup, … |
| 0x35 (53) | **Class / dealer restriction tags** | `RALLY,RACE`, `RACE,GR.5`, `RACE,PROTO`, `MUSCLECAR`, `SMALL`, `VAN,SMALL`, `RACE,F1`, `F1,NASCAR`, `RACE,GR.A`, `SMALL,GR.5,RACE`, `OFFROAD,RALLY`, dealers (`CINDER`, `MYLENE`, `RAVEN`, `NANWOLF`, `AALIA`, `ENHORNING`, …), models (`444T GR.A`, `STIEGLITZ GR.5`, …) |
| 0x211 (529) | **Weather** | `CLEAR`, `RAIN`, `SNOW`, `STORM`, `FOG` |
| 0x380… | rarely | junk float bits caught by the heuristic |

Records sit **adjacent**: a restriction block (tags → dealer → model →
weather → pilot) comes right before or right after the event-name record.
Layout order varies by record kind; nothing is length-delimited globally, so
**parse with the anchor + K heuristic, never with fixed offsets**. Both menu
copies in level0 contain the same records (offsets differ by ~1.8 MB) —
deduplicate by name.

Other notable raw strings in the menu: `Championships`, `Rallyes`,
`Weekends`, `TIMETRIAL`, `USA HILLCLIMB` (screen headers), `RALLY`,
`RACE,MUSCLECAR`, `CONCEPT`, `INSERT COIN`. A pastebin URL
(`https://pastebin.com/raw/fPQXy9gs`) is embedded near a record with K=0x3a
(probably the in-game "news"/update feed; do not fetch it).

Non-ASCII: some strings contain UTF-8 (`é`, `ñ`), e.g. `Classic Coupé Showdown`,
`Proaño Speedway`, `DIANA PROAÑO`, `Börje Fornberg`. All extraction code must
be UTF-8-aware.

**TimeTrial** records seem to reference the venues (K=271) — the TimeTrial
screen probably allows any venue; no separate event list identified yet.

## Managed/Assembly-CSharp.dll

Mono assembly, 653 typedefs. Important classes (metadata verified with
dnfile; IL NOT decompiled yet):

`SaveObject`, `SaveSlot`, `GarageSlot`, `LeagueStatus`, `WeekendStatus`,
`RallyStatus`, `ArcadeStatus`, `ArcadeRouteStatus`, `StatsData`, `Keynames`,
`GhostSaveObject`, `SaveFileDebuggerr` (sic), `WeekendData`,
`WorldChampionShipData`, `WorldRallyData`, `WeekendInfoScr_`,
`WeekendStatus_`, `ArcadeRoundData`, `LeagueData`, `LeagueRound`, `RallyData`,
`CircuitDb` / `CircuitDataBase_` / `CircuitProfile_` / `TimeTrial_`,
`InfoEndurance`, `InfoArcade`, `ButtonChamp`, `ButtonRally`,
`MainLeaderBoard`, `StatsDataWatcher`, …

### Field signature quirk

Integer field signatures in the .NET metadata are stored as `ELEMENT_TYPE_I2/
U2/I4/U4/...` etc. For the *save stream*, what matters is the NRBF
`MemberPrimitiveTyped` width actually written (e.g. `Coins` shows 3 in the
save as an int32). Trust the stream, not the metadata signature.

### Obfuscated types

Some types are renamed to garbage (`.bc^aZa`a`bc`b]` etc.) — the CameraFilterPack
third-party filters. Ignore them; game classes use readable names.

## Mono runtime / Direct3D

- The game runs on **Mono** (`MonoBleedingEdge\EmbedRuntime\mono-2.0-bdwgc.dll`)
  and **Direct3D 11** (`d3d11.dll` + `dxgi.dll` confirmed loaded in a running
  game; Unity 2021.3.29f1).
- `mono-2.0-bdwgc.dll` exports the Mono embed API (`mono_get_root_domain`,
  `mono_domain_assembly_open`, `mono_image_get_name`, `mono_class_from_name`,
  `mono_runtime_invoke`, `mono_thread_attach`, `mono_method_get_header`,
  `mono_class_get_method_from_name`, …) — plenty for native interop/injection.
- The in-game mod uses **Unity IMGUI** for an on-screen `OnGUI` overlay rendered by
  the game itself — no DX hook, no external window. Delivery: the tool injects
  `swgp2_mod.dll` (native loader, `mod/loader/`), which loads `Swgp2Mod.dll`
  into the game's Mono domain and calls `Swgp2Mod.Entry.Main()`.
  The mod is keyboard-only by design: **F7** repair car (Life back to the
  race-start snapshot), **F8**/**F9** increase/decrease the acceleration
  multiplier by 0.5x (0.5..5.0), applied every frame to `MainCar.CarPower`;
  the current multiplier appears in the center of the screen (white text,
  ~3 seconds, then fades out). The player's car vs AI cars are told apart by
  `Rival==false` (via `FindObjectsOfType(Type)`). Logging to
  `%APPDATA%\swgp2-tool\mod.log`.
- x86 Mono: `mono-2.0-bdwgc.dll` exports the whole embed API (1231 exports).

## Save files are the only writable game state we touch

Everything else under `StreamingAssets` is read-only for the tool (we never
write there). Editing `elf.dat`/`doplhin.dat`/levels is out of scope — the
analyzer only reads them.
