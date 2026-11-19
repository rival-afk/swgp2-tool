# SAVE FORMAT — internals of Supersave<N>.sav

Verified 2026-11-19 against a real save (`%USERPROFILE%\AppData\LocalLow\ViJuDa\
Super Woden GP 2\Save\Supersave0.sav`, Supersave0 = the auto-save slot).

## Crypto layer

```
.sav = AES-128-CBC( KEY="W9JU3JASLM7765D2" (16 ASCII bytes), IV=0x00*16, PKCS7 )
     → .NET BinaryFormatter (MS-NRBF) stream, little-endian
```

Decrypted size = encrypted size − 16 (PKCS7). There is no signature, no version,
no wrapper — plain BinaryFormatter after decryption.

## NRBF stream layout (MS-NRBF subset)

Record types present: `SerializedStreamHeader, BinaryLibrary,
ClassWithMembersAndTypes / SystemClassWithMembersAndTypes, ClassWithId,
BinaryObjectString, BinaryArray, ArraySinglePrimitive / ArraySingleString /
ArraySingleObject, MemberPrimitiveTyped, MemberReference, ObjectNull,
MessageEnd`.

Assembly reference in every save: `Assembly-CSharp, Version=0.0.0.0,
Culture=neutral, PublicKeyToken=null`. Root object: class `SaveObject`.

Parsing is lossless: parse → emit → parse produces identical data
(`Writer::write` is byte-identical for its own parse output; roundtrip tests
verify dump equality). **`nrbf.rs` is ~40 KB and is the heart of the tool —
touch it only with the roundtrip test green.**

## The SaveObject schema (from Assembly-CSharp.dll metadata)

Field types below are the real signature blobs (decoded with dnfile); `int16`
etc. names reflect the actual storage classes in the stream (values observed
in a real save are unbounded in some fields — see ranges).

### SaveObject (root)
| Field | Type | Save value | Meaning / range |
|---|---|---|---|
| `WinningRounds` | object (null) | null | legacy/unused |
| `Analog` | bool | true | analog (vs digital) controls |
| `Coins` | int | 3 | coins, 0 … 999 999 (cheat button sets 999 999) |
| `Economy` | int | 161 254 | credits (Cr); cheat sets 999 999 999 |
| `QualityLevel` | int | 2 | graphics preset index; seen 0..2 (Low/Medium/High implied by game; save holds int) |
| `CornerWarning` | bool | true | show corner-cut / track-limits warning |
| `ShowRecords` | bool | true | show lap records in UI |
| `Difficulty` | int | 1 | race difficulty (0..2, normal = 1) |
| `Miles` | bool | false | distance in miles instead of km |
| `Vibration` | bool | true | controller vibration |
| `Language` | string | "RUS" | UI language code (RUS observed; equal-length patch only) |
| `GarageSlots` | List\<GarageSlot\> (23) | see below | every car instance owned by the player |
| `Myslot` | int | 20 | active garage slot id |
| `SortMode` | int | 3 | garage list sort order (0..3+) |
| `VideoMode` | int | 2 | video mode index (windowed/borderless/fullscreen-ish) |
| `LeagueStatus_` | LeagueStatus | | circuit championship progress |
| `WeekendStatus_` | WeekendStatus | | weekend-event progress |
| `RallyStatus_` | RallyStatus | | rally championship progress |
| `ArcadeStatus_` | ArcadeStatus | | arcade mode progress |
| `RouteStatus_` | List\<ArcadeRouteStatus\> | | arcade route progress |
| `WeekendStarGroups` | List\<int\> (50) | | weekend star ratings per event |
| `Gamemode` | string | "RALLY" | current game mode; `"TIMETRIAL"` also seen in `doplhin.dat` (fresh-save template) |
| `LeagueStars` | object (null) | null | |
| `HistoricSlot` | int | 26 | historic-race slot index |
| `StatsData_` | StatsData | | lifetime statistics |
| `Gametime` | float | 13 992.45 | total game seconds played |
| `Damage` | bool | true | car damage enabled |
| `Keynames` | List\<Keynames\> (4) | | 4 key bindings (rebindable; list of 20 keys) |

### GarageSlot (each garage car)
`SlotID` (int), `CarID` (int), `SkinID` (int), `DirtyNess` (int), `Oil`
(int), `Kms` (float), `RimsID` (int), `Gift` (bool), `Engine`, `Brakes`,
`Chassis`, `Gearbox`, `Suspension`, `Tyres` (ints, component condition levels).
`CarID` == the `IMD###` id from `StreamingAssets/CarThumbnails` and from
`Original_names.txt`. `Kms` is wear; `Oil`/`DirtyNess` are service counters.

### LeagueStatus / WeekendStatus / RallyStatus (championship progress)
- `LeagueStatus`: `ChampIndex` (int), `GarSlot` (int), `Round` (int),
  `ChampPrices` List\<int\> (12, descending entry prices — last two are 0 = free),
  `Points` List\<int\> (12), `Stars` List\<int\> (100, per-round stars?).
- `WeekendStatus`: `ChampIndex` (int — index into weekend events), `Round`,
  `ChampPrices` List\<int\> (14), stars groups live in `WeekendStarGroups`.
- `RallyStatus`: `Round`, `RallyIndex` (int), `GarSlot`, `ChampPrices`
  List\<int\> (10), `PlayerRoundTimes` List\<float\>, `OpponentTimes`
  List\<float\>, `OpponentTotalTimes` List\<float\>, `Stars` List\<int\>.
  OpponentTimes/OpponentTotalTimes hold per-round AI target times — editing
  these changes the difficulty of rivals' pace.
- `ArcadeStatus`: `Opened` (bool), `Round`, `RouteIndex`, `HasContinued`
  (bool), `ArcadeCars`/`ArcadeCarsIndex`/`ArcadeSkins` (List\<int\>, 4).

### StatsData (lifetime stats — uint16/int family)
`RacesCompleted`, `Victories`, `ArcadeRaces`, `MoneySpent`, `MoneyWon`,
`OilChanges`, `CarsCleaned`, `TotalKms` (float), `InvitationKms`, `Breakdowns`,
`MasterVolume` (float dB, negative), `RaceMusic`, `MenuMusic`, `RaceSFX`,
`MenuSFX` (float dB), `AirTime` (float s), `FixedMinimap` (bool), `Streak`,
`Invitationkey`, `Invitationtries` (List\<int\>), `MenuCycleAnimation` (bool),
`ArcadeStageTimes`, `ArcadeRound`, `ArcadeKms` (float), `GhostViewMode`
(int 0..2), `ResizeHud` (bool), `Flare` (bool), `Vsync` (int 0/1),
`MultiPlayerCollisions` (bool), `FXAA` (bool), `SSAO` (bool), `Tutorial_01`
(bool), `CustomMusic` (bool), `Country` (int 21 = RU), `Creditsrolled` (bool),
`Unlocked` (List\<bool\>, 999), `AchievementsClear` (List\<bool\>, 50).
`Gametime` (float) duplicates the root-level one.

### Keynames
List of 4 objects: `Key` List\<string\> of 20 XNA key names
(`LeftArrow`, `A`, `CapsLock`, …). Save stores the binding as strings;
patchable only at equal byte length.

## What is patchable and what isn't

Everything above is a primitive/string scalar → byte-patchable in place:
- **ints/uints/bools/floats** → fixed size, always patchable.
- **strings** → only equal-byte-length (e.g. `RUS`→`ENG`).
- **null ↔ value** → NOT patchable (no bytes to overwrite in place reliably).
  `LeagueStars`, `WinningRounds` are null in saves — cannot be turned into
  objects by patching; editing them is unsupported.
- **lists/objects** → not patchable at all; only their scalar *leaves* are.
- Patch tooling: `save::patch_target`, `save::prim_from_json`,
  `save::encode_prim`; whole-save verify + auto-restore in
  `app::patch_save`. Front-ends must go through `app::*`.

## Fresh-save template

`StreamingAssets/Data/doplhin.dat` is a **default SaveObject** serialized as
NRBF (not encrypted). It parses with our parser as-is. Differences vs a
played save: `Economy=15000`, `Coins=0`, `Gamemode="TIMETRIAL"`,
`GarageSlots=[]`, `Gametime≈3.3`. Useful as a key for editors/tests.

## Integrity / verification

`app::patch_save` re-decrypts and re-parses the written file and compares the
JSON dump of the in-memory expected graph with the on-disk result. If they
differ, the backup is restored automatically. `verify` command in the GUI
does the same round-trip check on demand. The game itself accepts saves whose
byte layout it wrote (we never change structure), which is why patching works.
