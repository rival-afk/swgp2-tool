# PROGRESS — где мы (2026-11-19)

Текущая стадия: **незавершённая работа (WIP), не релиз**. Сейв-редактор не тронут:
все пишущие пути (`app::patch_save`) не менялись; `cargo test --release` зелёный
(в т.ч. `patch_noop_is_byte_identical`). Пуш в `origin/main`.

## Сделано в этой серии работ (2026-11-19)

### Документация (новая, обязательно читать)
- `docs/ARCHITECTURE.md` — как устроен репозиторий, **главный подводный камень
  сборки** (`CARGO_TARGET_DIR="C:\Users\stude\swgp2-target"`, т.к. репо в OneDrive
  и сборка в нём падает с "cannot find .rlib"), safety model, конвенции.
- `docs/SAVE_FORMAT.md` — крипта, NRBF, схема SaveObject (все поля, типы,
  значения по умолчанию), что патчится байт-патчами, что нельзя (null↔value,
  строки только equal-length), шаблон новой игры `doplhin.dat`.
- `docs/GAME_FILES.md` — карта всех файлов игры: `uselesslist.txt`,
  `Original_names.txt`, `CarThumbnails/IMD*.SW` (PNG с расширением .SW),
  `CAR_SKINS`, `_DIPSWITCHES.txt` (`ARCADE_MODE_FORCE_CRT`,
  `MENUS_POSTPROCESSING_FX`), `_pilot_names.txt` (25 пилотов с кодами стран).
  Ключевое: **записи событий в `level*`** — `01 00 00 00 01 00 00 00 <K:u32>`
  + `u32 len` + имя; K: 602=уикенд, 271=трасса, 262=пилот, 86=кубок,
  53=класс/дилер-ограничение, 211/529=погода. Два списка меню в level0 —
  дедупликация по имени.

### Задача 3 — убрать «7z style»
Убрано из README/REPO_GRAPH/комментариев GUI. В CHANGELOG упоминание осталось
намеренно (историческая запись о v3.0.0, историю не переписываем).

### Задача 2(a) — подсказки для полей сейв-редактора
- `app::COMMON_FIELDS` теперь `FieldMeta { path, label, hint }`; подсказки
  «все возможные значения / диапазон» для всех 9 полей.
- `app::FIELD_HINTS` + `app::field_hint()` — подсказки для остальных путей
  сейва (GarageSlots.N.*, LeagueStatus_.*, WeekendStatus_.*, RallyStatus_.*,
  StatsData_.* и т.д., с нормализацией индексов массива к `N`). Показываются
  в окне «Advanced» живого ввода.
- GUI: подсказка под лейблом каждого поля + в Advanced. TUI: подсказка под
  каждой строкой поля.

### Задача 4 — каталог машин
GUI → `Cars` теперь: сетка 4 колонки × N страниц (12/страница), листалка
(Prev/Next + счётчик страниц/машин), фильтр + сортировка (combobox + asc/desc,
сохранены прежние ключи Rank/CV/km/h/Year/Name), карточки с превью скина
(кэш превью `car_preview_cache`), клик по карточке открывает плавающее окно
со всеми скинами машины. Прямоугольная таблица заменена полностью.

### Задачи 5+6 — Вики «живая»: Weekends / Endurance / Rallies + живые треки
- Новый парсер `src/tool/events.rs`: сканирует `level0` (anchors + lp-строки,
  UTF-8-aware), классифицирует записи, собирает события с ограничениями
  (tags/dealer/model/pilot/weather/класс), дедуплицирует две копии меню.
  Протестировано на реальных данных игры: **77 событий** (27 уикендов,
  16 кубков/чемпионатов, 49+1... точнее: недели включают все 6H/8H/12H/24H).
  Неизвестные kind-коды, но имя похоже на событие → Cup/Weekend по эвристике.
- Вики: новые вкладки **Weekends** (фильтр "endurance only"),
  **Endurance** (все гонки на выносливость), **Rallies** (чемпионат
  редактированный + ралли-этапы, парсятся живьём), **Tracks** (все трассы из
  игры, с fallback на статический список при ошибке парсинга).
- Лучшие машины подбираются алгоритмом: фильтр по ограничению события
  (дилер/модель/класс BTC/DTC/JTC/GR.5/GR.A/…) → сортировка по CV → по
  скорости (idea 6). Для ралли-этапов пул ралли-каров.
- Championships теперь тоже фильтруются классом (class_token: BTC/DTC/JTC),
  рейтинг по CV/скорости вместо «выбирать по цене».
- `WikiTab` расширена; живые события кэшируются в `wiki_events` (парс один раз
  на папку игры, сбрасывается при смене Game Folder).

### In-game меню (задача 2, часть b) — **РАБОТАЕТ, проверено**
- `mod/Swgp2Mod.cs` → `mod/Swgp2Mod.dll` (**закоммичен**, собирается Roslyn'ом
  `E:\Soft\Visual Studio\MSBuild\Current\Bin\Roslyn\csc.exe` против
  mscorlib/CoreModule/IMGUIModule/InputLegacyModule/TextRenderingModule/Assembly-CSharp игры).
- Меню НЕТ — только горячие клавиши (меню по просьбе пользователя убран):
  **F7** — починить машину (Life восстанавливается до снапшота);
  **F8** — ускорение +0.5x (до 5.0x);
  **F9** — ускорение −0.5x (до 0.5x). Множитель применяется каждый кадр
  к `MainCar.CarPower`. Белый текст по центру экрана, ~3 секунды, плавное
  затухание (`F7` → «car repaired»). `Rival==false` фильтр для выбора
  игроковой машины; reflection-доступ к полям.
- Нативный загрузчик: `mod/loader/` (`swgp2-mod.dll`, cdylib, **ноль зависимостей**):
  DllMain → CreateThread → ждёт `mono-2.0-bdwgc.dll` → `mono_get_root_domain` →
  `mono_thread_attach` → `mono_domain_assembly_open(%APPDATA%\swgp2-tool\mod\Swgp2Mod.dll)`
  → класс `Swgp2Mod.Entry` → `mono_runtime_invoke(Main)`.
- Инжектор: `src/tool/inject.rs` — поиск процесса, инжекция
  (CreateRemoteThread+LoadLibraryW на kernel32!LoadLibraryW), список модулей,
  память (region walk, read/write, wildcard-поиск по шаблону), install/uninstall
  мода из встроенных байт, чтение хвоста mod.log.
- **GUI секция "In-Game Mod"** — кнопки Install mod / Uninstall / Inject now,
  статус игры, хвост mod.log.
- **Проверено вживую**: игра (PID 8276/12128) + `inject_test` → `mod loaded`,
  `car #15 attached (power 195, life 96)`, игра стабильна. Игра
  **Direct3D 11** (d3d11.dll + dxgi.dll), Mono 2.0 (`mono-2.0-bdwgc.dll`).
- Сейв забекаплен: `%USERPROFILE%\AppData\LocalLow\ViJuDa\Super Woden GP 2\Save\Backups\Supersave0.pre-test-*.savbak`.
- Поля `MainCar`: CarPower (ускорение), ActualPower, PowerOffset, CarTopSpeed,
  Life (прочность), LifeRestoreRate, Fuel, Turbo, SpeedMultiplier, Grip,
  ResistanceFactor, SlipStreamLevel, PlayerIndex, Rival (отличить игрока от ИИ).

## Осталось сделать (в порядке очереди)

1. **MCP-сервер** (`swgp2-mcp`, stdio JSON-RPC 2.0): общий набор инструментов
   «агент может делать с игрой то, что не задумывалось»: file_info/hexdump/
   strings/search, nrbf_probe/parse, save_get/set/dump/patch/backup/restore/
   verify, game_detect/info/launch, events_scan, wiki_*, process_find/modules/
   memory_read/memory_write/memory_search, inject_dll, mod_install/mod_inject,
   analyze_file, pe_info, map→glb, bot routes, wiki_best_cars. Реализация:
   `src/mcp.rs` (фреймворк: реестр, схемы, dispatch) +
   `src/bin/swgp2-mcp.rs` (main-loop).
2. **Анализатор файлов (Rust)**:
   - `src/tool/unity.rs` — парсер Unity SerializedFile (format 22):
     заголовок BE-поля (`00 00 00 00 | 00 00 00 16 | dataOffset @20 | fileSize @28`),
     таблица объектов (offset/size/classId). **В процессе** — точный layout
     типов/объектов ещё не подтверждён байтами; проверка «последний объект
     кончается в конце файла».
   - `src/tool/glb.rs` — писатель glTF 2.0 binary (.glb) из Unity-мешей.
   - `src/tool/analyze.rs` — file_info, hexdump, strings, search, PE-дамп
     (секции/импорты/экспорты/машина), NRBF-проба, SW-thumbnail decode.
   - Бот-траектории: маршруты в level (`RouteData_`, `MAINCURVES`,
     `RouteStatus_`) — извлечение vector3-точек из записей по якорям
     (тот же record-формат), экспорт JSON/LineStrip GLB.
   - Карта → .glb: Mesh-объекты (classId 43) + Transform-иерархия +
     GLB писатель. Проверка трианглов через trimesh (Python, pip).
3. **Релизная сборка**: bump версии (Cargo.toml; Cargo.lock уже с поправкой
   3.3.0→3.4.0), CHANGELOG, README, REPO_GRAPH, удалить tag-scratch
   артефакты; задокументировать `mod/` в README.
4. **Проверки перед релизом**: `cargo test --release` зелёный, GUI запускается,
   инъекция мода не крашит игру, сейв не портится (patch_noop_is_byte_identical).

## Операционные заметки

- **Сборка**: `$env:CARGO_TARGET_DIR="C:\Users\stude\swgp2-target"; cargo build --release`
  (репо в OneDrive → `cargo build` в нём периодически падает с "cannot find .rlib";
  target держать вне OneDrive). Несколько раз падало и при `cargo check` в фоне.
- **Игра для тестов**: `C:\Users\stude\Downloads\Super.Woden.GP.2 v1.31b (Build 18323290)\Super Woden GP 2.exe`
  (portable). Перед тестами инъекций **бэкапить сейв** — игра перезаписывает его
  при выходе.
- **Примеры/черновики кода** лежат в `C:\Users\stude\AppData\Local\Temp\opencode`
  (python-скрипты разбора форматов, дампы) — в репозиторий не входят.
- **pycp** (MCP) — быстрый запуск python-скриптов без файлов на диске; для
  тяжёлого анализа применялся `C:\Users\stude\AppData\Local\Temp\opencode`
  (scratch-скрипты удалены; используй pycp `run_python_code` вместо них).
- **git**: origin = https://github.com/rival-afk/swgp2-tool.git, ветка main.
  Имена коммитов v3.x.0 по схеме «v3.4.0: …». `mod/Swgp2Mod.dll` (10.5 KB)
  **коммитится намеренно** — для самодостаточной сборки без Roslyn;
  `mod/loader/swgp2-mod.dll` генерируется `cargo build --release -p swgp2-mod`.
- **Сейв-редактор**: не менять `nrbf.rs` без зелёного roundtrip-теста;
  запись только через `app::patch_save`; игра должна быть закрыта при записи.

## Известные открытые вопросы
- Target для компиляции мода: mscorlib игры — какой профиль (netstandard2.1 /
  net48-equiv)? csc Roslyn'ом компилировать с `/nostdlib` и референсами из
  `Managed/`, проверить запуск (mod.log).
- Точный layout SerializedFile: противоречие endianness полей заголовка
  (BE для version/dataOffset/fileSize) против LE внутри данных; нужен ещё
  один проход по байтам (для разбора удобно использовать pycp
  `run_python_code`, файлы на диске не нужны).
- Имена статических/инстанс полей MainCar, нужные для Freeze именно
  "durability": по metadata это `Life` (player durability) + `LifeRestoreRate`.
  Слайдер уже завязан на `Life`.
