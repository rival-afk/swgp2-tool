// GUI-subsystem binary on Windows: no console window is spawned.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! swgp2-tool - all-in-one tool for Super Woden GP 2. No flags, just run it.
//!
//! Layout: menu bar -> toolbar (Load/Reload/Save + quick actions)
//! -> left section rail + central panel -> status bar.

use eframe::egui;
use serde_json::Value as Json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use swgp2_tool::app;
use swgp2_tool::save;
use swgp2_tool::tool::{cars, game, reset, settings, skins};

fn main() -> eframe::Result<()> {
    if std::env::args().len() > 1 {
        eprintln!("swgp2-tool takes no arguments - just launch it");
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 660.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SWGP2 Tool",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(white_visuals());
            Ok(Box::new(Tool::new(&cc.egui_ctx)))
        }),
    )
}

/// Light/white theme.
fn white_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    let white = egui::Color32::WHITE;
    let off_white = egui::Color32::from_rgb(250, 250, 250);
    let selection = egui::Color32::from_rgb(204, 228, 247); // light blue (Win10-ish)
    let accent = egui::Color32::from_rgb(0, 120, 215);

    v.window_fill = off_white;
    v.panel_fill = white;
    v.extreme_bg_color = white;
    v.faint_bg_color = off_white;
    v.code_bg_color = off_white;
    v.selection.bg_fill = selection;
    v.selection.stroke = egui::Stroke::new(1.0_f32, accent);
    v.hyperlink_color = accent;
    v.override_text_color = Some(egui::Color32::from_gray(20));
    v
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Section {
    Save,
    Cars,
    Skins,
    Wiki,
    Game,
    Reset,
    About,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Section::Save => "Save Editor",
            Section::Cars => "Cars",
            Section::Skins => "Car Skins",
            Section::Wiki => "Wiki",
            Section::Game => "Game",
            Section::Reset => "Reset Saves",
            Section::About => "About",
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum WikiTab {
    Cars,
    Championships,
    Tracks,
    Weekends,
    Endurance,
    Rallies,
    Tips,
}

impl WikiTab {
    fn label(self) -> &'static str {
        match self {
            WikiTab::Cars => "Cars",
            WikiTab::Championships => "Championships",
            WikiTab::Tracks => "Tracks",
            WikiTab::Weekends => "Weekends",
            WikiTab::Endurance => "Endurance",
            WikiTab::Rallies => "Rallies",
            WikiTab::Tips => "Tips",
        }
    }
}

struct Field {
    path: String,
    label: String,
    hint: String,
    text: String,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum SortKey {
    Rank,
    Cv,
    TopSpeed,
    Year,
    Name,
}

struct Tool {
    // --- global
    section: Section,
    message: (String, bool), // text, is_ok
    game_dir: Option<PathBuf>,
    settings: settings::Settings,

    // --- save editor
    save_path: Option<PathBuf>,
    fields: Vec<Field>,
    dirty: bool,
    advanced_path: String,
    advanced_value: String,
    backups: Vec<PathBuf>,
    backup_sel: usize,
    dump_view: bool,
    dump_text: String,
    show_json: bool,
    show_advanced: bool,
    show_backups: bool,
    show_crypto: bool,

    // --- cars & skins
    cars: Vec<cars::CarEntry>,
    car_preview_cache: HashMap<usize, Option<PathBuf>>,
    car_filter: String,
    car_sort: SortKey,
    car_sort_desc: bool,
    car_page: usize,
    car_detail_open: bool,
    selected_car: Option<usize>,
    skins_list: Vec<skins::SkinEntry>,
    skin_filter: String,
    hide_masks: bool,
    selected_skin: Option<usize>,
    textures: HashMap<PathBuf, egui::TextureHandle>,
    selected_skin_images: Vec<PathBuf>, // images of selected car
    selected_skin_image: usize,

    // --- cached game info
    game_info: Option<Result<game::GameInfo, String>>,

    // --- reset saves
    reset_armed: bool,
    archives: Vec<reset::ResetArchive>,
    archive_sel: usize,

    // --- wiki
    wiki_tab: WikiTab,
    wiki_car_filter: String,
    wiki_dealer_filter: String,
    wiki_catalog: Vec<swgp2_tool::tool::catalog::CatalogCar>,
    wiki_events: Option<Vec<swgp2_tool::tool::events::GameEvent>>,
    wiki_events_err: Option<String>,
    event_search: String,
    endurance_only: bool,
    wiki_selected_car: Option<usize>,
    wiki_skin_idx: usize,
    wiki_detail_open: bool,
}

impl Tool {
    fn new(ctx: &egui::Context) -> Self {
        let settings = settings::load();
        let (game_dir, game_info) = match settings.game_dir.clone().filter(|d| d.join(game::EXE_NAME).is_file()).or_else(game::detect_game_dir) {
            Some(dir) => (Some(dir.clone()), Some(game::game_info(&dir))),
            None => (None, None),
        };

        let mut t = Tool {
            section: Section::Save,
            message: ("ready".into(), true),
            game_dir: game_dir.clone(),
            settings: settings::Settings { game_dir: game_dir.clone() },
            save_path: None,
            fields: Vec::new(),
            dirty: false,
            advanced_path: String::new(),
            advanced_value: String::new(),
            backups: Vec::new(),
            backup_sel: 0,
            dump_view: false,
            dump_text: String::new(),
            show_json: false,
            show_advanced: false,
            show_backups: false,
            show_crypto: false,
            cars: Vec::new(),
            car_preview_cache: HashMap::new(),
            car_filter: String::new(),
            car_sort: SortKey::Rank,
            car_sort_desc: true,
            car_page: 0,
            car_detail_open: false,
            selected_car: None,
            skins_list: Vec::new(),
            skin_filter: String::new(),
            hide_masks: true,
            selected_skin: None,
            textures: HashMap::new(),
            selected_skin_images: Vec::new(),
            selected_skin_image: 0,
            game_info,
            reset_armed: false,
            archives: Vec::new(),
            archive_sel: 0,
            wiki_tab: WikiTab::Cars,
            wiki_car_filter: String::new(),
            wiki_dealer_filter: String::new(),
            wiki_catalog: Vec::new(),
            wiki_events: None,
            wiki_events_err: None,
            event_search: String::new(),
            endurance_only: false,
            wiki_selected_car: None,
            wiki_skin_idx: 0,
            wiki_detail_open: false,
        };
        t.refresh_archives();
        t.reload_save_slots();
        t.load_cars_and_skins();
        if game_dir.is_some() {
            let _ = settings::save(&t.settings);
        }
        let _ = ctx;
        t
    }

    // ------------------------------------------------------------------
    // messaging / persistence helpers
    // ------------------------------------------------------------------
    fn msg(&mut self, text: impl Into<String>, ok: bool) {
        self.message = (text.into(), ok);
    }

    fn refresh_archives(&mut self) {
        self.archives = reset::save_dir().map(|d| reset::list_archives(&d)).unwrap_or_default();
        if self.archive_sel >= self.archives.len() {
            self.archive_sel = 0;
        }
    }

    fn do_full_reset(&mut self) {
        let Some(dir) = reset::save_dir() else {
            self.msg("cannot locate the save folder", false);
            return;
        };
        match reset::reset_all(&dir) {
            Ok((archive, n)) => {
                self.msg(
                    format!("reset done - {n} save(s) archived to {}", archive.dir.display()),
                    true,
                );
                self.reset_armed = false;
                self.save_path = None;
                self.fields.clear();
                self.dirty = false;
                self.refresh_archives();
                self.refresh_game_info();
            }
            Err(e) => {
                self.msg(format!("reset failed: {e}"), false);
                self.reset_armed = false;
                self.refresh_archives();
            }
        }
    }

    fn restore_selected_archive(&mut self) {
        let Some(dir) = reset::save_dir() else { return };
        let Some(archive) = self.archives.get(self.archive_sel).cloned() else {
            self.msg("no archive selected", false);
            return;
        };
        match reset::restore_archive(&dir, &archive) {
            Ok(n) => {
                self.msg(format!("restored {n} save(s) from {}", archive.dir.display()), true);
                self.reload_save();
                self.refresh_archives();
                self.refresh_game_info();
            }
            Err(e) => self.msg(format!("restore failed: {e}"), false),
        }
    }

    fn delete_selected_archive(&mut self) {
        let Some(archive) = self.archives.get(self.archive_sel).cloned() else { return };
        match reset::delete_archive(&archive) {
            Ok(()) => {
                self.msg(format!("archive {} deleted forever", file_name(&archive.dir)), true);
                self.refresh_archives();
                self.refresh_game_info();
            }
            Err(e) => self.msg(format!("delete failed: {e}"), false),
        }
    }

    fn refresh_game_info(&mut self) {
        self.game_info = self.game_dir.as_ref().map(|d| game::game_info(d));
    }

    fn set_game_dir(&mut self, dir: PathBuf) {
        self.game_dir = Some(dir.clone());
        self.settings.game_dir = Some(dir);
        let _ = settings::save(&self.settings);
        self.refresh_game_info();
        self.load_cars_and_skins();
    }

    fn load_cars_and_skins(&mut self) {
        self.cars.clear();
        self.car_preview_cache.clear();
        self.skins_list.clear();
        self.wiki_catalog.clear();
        self.wiki_events = None; // re-parse events for the (possibly) new game folder
        self.wiki_events_err = None;
        self.selected_car = None;
        self.selected_skin = None;
        self.wiki_selected_car = None;
        self.wiki_detail_open = false;
        self.car_page = 0;
        self.car_detail_open = false;
        let Some(gd) = self.game_dir.clone() else { return };
        let data_txt = game::streaming_data_dir(&gd).join("uselesslist.txt");
        let mut cars = cars::CarEntry::parse_file(&data_txt);
        cars::attach_skin_dirs(&mut cars, &game::skins_root(&gd));
        self.cars = cars;
        self.skins_list = skins::scan(&game::skins_root(&gd));
        self.wiki_catalog = swgp2_tool::tool::catalog::load_catalog(&gd);
    }

    /// Resolve the save path suggested on startup.
    fn reload_save_slots(&mut self) {
        self.save_path = save::default_save_path(0).filter(|p| p.exists()).or_else(|| save::list_slots().into_iter().next());
        self.reload_save();
    }

    // ------------------------------------------------------------------
    // save editor
    // ------------------------------------------------------------------
    fn reload_save(&mut self) {
        self.fields.clear();
        self.dirty = false;
        self.refresh_backups();
        let Some(path) = self.save_path.clone() else {
            self.msg("no save found - has the game been run?", false);
            return;
        };
        match app::load(&path) {
            Ok(g) => {
                self.fields = app::COMMON_FIELDS
                    .iter()
                    .map(|f| Field {
                        path: f.path.to_string(),
                        label: f.label.to_string(),
                        hint: f.hint.to_string(),
                        text: app::field_text(&g, f.path),
                    })
                    .collect();
                self.msg(format!("loaded {}", path.display()), true);
            }
            Err(e) => self.msg(format!("load failed: {e}"), false),
        }
    }

    fn refresh_backups(&mut self) {
        self.backups = match &self.save_path {
            Some(p) => save::list_backups(p).unwrap_or_default(),
            None => Vec::new(),
        };
        if self.backup_sel >= self.backups.len() {
            self.backup_sel = 0;
        }
    }

    fn save_all(&mut self) {
        let Some(path) = self.save_path.clone() else { return };
        let edits: Vec<(String, Json)> = self
            .fields
            .iter()
            .map(|f| (f.path.clone(), app::parse_value(&f.text)))
            .collect();
        match app::patch_save(&path, &edits) {
            Ok(backup) => {
                self.msg(format!("saved (backup: {})", file_name(&backup)), true);
                self.dirty = false;
                self.reload_save();
            }
            Err(e) => self.msg(format!("save failed: {e}"), false),
        }
    }

    fn set_path_value(&mut self, path: &str, value: Json) {
        let Some(p) = self.save_path.clone() else { return };
        match app::set_single(&p, path, value) {
            Ok(b) => {
                self.msg(format!("set {path} (backup: {})", file_name(&b)), true);
                self.reload_save();
            }
            Err(e) => self.msg(format!("set failed: {e}"), false),
        }
    }

    fn verify(&mut self) {
        let Some(path) = self.save_path.clone() else { return };
        let raw = match std::fs::read(&path) {
            Ok(r) => r,
            Err(e) => {
                self.msg(format!("read failed: {e}"), false);
                return;
            }
        };
        let check = (|| -> Result<(), String> {
            let plain = swgp2_tool::crypto::decrypt_save(&raw)?;
            let g1 = swgp2_tool::nrbf::parse(&plain).map_err(|e| e.to_string())?;
            let re = swgp2_tool::nrbf::Writer::new().write(&g1).map_err(|e| e.to_string())?;
            let g2 = swgp2_tool::nrbf::parse(&re).map_err(|e| e.to_string())?;
            let d1 = app::render_json(&save::dump(&g1).map_err(|e| e.to_string())?);
            let d2 = app::render_json(&save::dump(&g2).map_err(|e| e.to_string())?);
            if d1 == d2 {
                Ok(())
            } else {
                Err("roundtrip mismatch".into())
            }
        })();
        match check {
            Ok(()) => self.msg("save verified - parse/emit roundtrip OK", true),
            Err(e) => self.msg(format!("verify FAILED: {e}"), false),
        }
    }

    // ------------------------------------------------------------------
    // rendering
    // ------------------------------------------------------------------
    fn texture(&mut self, ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
        if let Some(t) = self.textures.get(path) {
            return Some(t.clone());
        }
        let img = load_image(path)?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        if size[0] == 0 || size[1] == 0 {
            return None;
        }
        let color = egui::ColorImage::from_rgba_unmultiplied(size, &img);
        let tex = ctx.load_texture(path.to_string_lossy(), color, egui::TextureOptions::LINEAR);
        self.textures.insert(path.to_path_buf(), tex.clone());
        Some(tex)
    }
}

/// Load an image from disk; the game stores dealership thumbnails as PNGs with a `.SW` extension.
fn load_image(path: &Path) -> Option<image::DynamicImage> {
    if path.extension().map(|e| e.eq_ignore_ascii_case("sw")).unwrap_or(false) {
        let bytes = std::fs::read(path).ok()?;
        image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()
    } else {
        image::open(path).ok()
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

fn fmt_size(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.2} MB", n as f64 / (1024.0 * 1024.0))
    }
}

fn fmt_time(t: Option<SystemTime>) -> String {
    t.map(|st| {
        let secs = st.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let days = secs / 86400;
        let rem = secs % 86400;
        let (h, mi) = (rem / 3600, (rem % 3600) / 60);
        let z = days as i64 + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            y + if m <= 2 { 1 } else { 0 },
            m,
            d,
            h,
            mi
        )
    })
    .unwrap_or_else(|| "?".into())
}

impl eframe::App for Tool {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ---------------------------------------------------------- menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Load save…").clicked() {
                        self.pick_save(ctx);
                        ui.close_menu();
                    }
                    if ui.button("Open save folder").clicked() {
                        self.open_save_folder();
                        ui.close_menu();
                    }
                    if ui.button("Launch game").clicked() {
                        self.launch_game();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Tools", |ui| {
                    if ui.button("Verify save").clicked() {
                        self.verify();
                        ui.close_menu();
                    }
                    if ui.button("Backup now").clicked() {
                        self.backup_now();
                        ui.close_menu();
                    }
                    if ui.button("Backup ALL save slots").clicked() {
                        self.backup_all();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Reset ALL saves…").clicked() {
                        self.section = Section::Reset;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Dump save to JSON…").clicked() {
                        self.dump_to_file();
                        ui.close_menu();
                    }
                    if ui.button("Apply JSON from file…").clicked() {
                        self.apply_from_file();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About / Credits").clicked() {
                        self.section = Section::About;
                        ui.close_menu();
                    }
                    if ui.button("Repo page").clicked() {
                        open_path_or_url("https://github.com/rival-afk/swgp2-tool");
                        ui.close_menu();
                    }
                });
            });
        });

        // ---------------------------------------------------------- toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if big_button(ui, "Load…").clicked() {
                    self.pick_save(ctx);
                }
                if big_button(ui, "Reload").clicked() {
                    self.reload_save();
                    self.msg("reloaded from disk", true);
                }
                let save_btn = egui::Button::new(egui::RichText::new("Save").strong())
                    .min_size(egui::vec2(72.0, 40.0));
                if ui.add_enabled(self.save_path.is_some(), save_btn).clicked() {
                    self.save_all();
                }
                ui.separator();
                if big_button(ui, "Verify").clicked() {
                    self.verify();
                }
                ui.separator();
                // right side: game presence
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match &self.game_dir {
                        Some(d) => {
                            ui.colored_label(egui::Color32::from_rgb(0, 150, 60), "game found");
                            ui.small(d.display().to_string());
                        }
                        None => {
                            if ui.button("Locate game folder…").clicked() {
                                self.choose_game_dir();
                            } else {
                                ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "game folder not set");
                            }
                        }
                    }
                });
            });
            ui.add_space(4.0);
        });

        // -------------------------------------------------------- status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                let save_txt = match &self.save_path {
                    Some(p) => {
                        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                        let mark = if self.dirty { " *" } else { "" };
                        format!("{}{} - {}", file_name(p), mark, fmt_size(size))
                    }
                    None => "no save".to_string(),
                };
                ui.label(egui::RichText::new(save_txt).monospace());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.small(format!("SWGP2 Tool v{}", env!("CARGO_PKG_VERSION")));
                    ui.separator();
                    let (txt, ok) = &self.message;
                    let color = if *ok {
                        egui::Color32::from_rgb(0, 150, 60)
                    } else {
                        egui::Color32::from_rgb(200, 60, 60)
                    };
                    ui.colored_label(color, txt);
                });
            });
            ui.add_space(3.0);
        });

        // ------------------------------------------------------------ rail
        egui::SidePanel::left("rail")
            .exact_width(132.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                for s in [Section::Save, Section::Cars, Section::Skins, Section::Wiki, Section::Game, Section::Reset, Section::About] {
                    let selected = self.section == s;
                    let btn = egui::Button::new(egui::RichText::new(s.label()).size(15.0))
                        .min_size(egui::vec2(120.0, 34.0))
                        .selected(selected);
                    if ui.add(btn).clicked() {
                        self.section = s;
                    }
                    ui.add_space(4.0);
                }
                ui.separator();
                ui.small(" no flags,\n just run");
            });

        // ------------------------------------------------------------ main
        egui::CentralPanel::default().show(ctx, |ui| match self.section {
            Section::Save => self.save_section(ui),
            Section::Cars => self.cars_section(ui, ctx),
            Section::Skins => self.skins_section(ui, ctx),
            Section::Wiki => self.wiki_section(ui, ctx),
            Section::Game => self.game_section(ui),
            Section::Reset => self.reset_section(ui),
            Section::About => self.about_section(ui),
        });
    }
}

fn big_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(label).size(15.0)).min_size(egui::vec2(72.0, 40.0)))
}

// ---------------------------------------------------------------------------
// sections
// ---------------------------------------------------------------------------

impl Tool {
    // ---------------------------------------------------- save section
    fn save_section(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);

            // file row: slot picker + open
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong("Save file");
                    let slots = save::list_slots();
                    if !slots.is_empty() {
                        egui::ComboBox::from_id_salt("slot")
                            .selected_text(
                                self.save_path
                                    .as_ref()
                                    .map(|p| file_name(p))
                                    .unwrap_or_else(|| "?".into()),
                            )
                            .show_ui(ui, |ui| {
                                for p in slots {
                                    let sel = self.save_path.as_ref() == Some(&p);
                                    if ui.selectable_label(sel, file_name(&p)).clicked() {
                                        self.save_path = Some(p);
                                        self.reload_save();
                                    }
                                }
                            });
                    }
                    if ui.button("Open…").clicked() {
                        if let Some(p) = rfd::FileDialog::new().add_filter("SWGP2 save", &["sav"]).pick_file() {
                            self.save_path = Some(p);
                            self.reload_save();
                        }
                    }
                    if let Some(p) = &self.save_path {
                        ui.separator();
                        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                        ui.small(format!("{} - {}", fmt_size(size), p.display()));
                    }
                });
            });

            ui.add_space(4.0);

            // quick cheats
            ui.horizontal(|ui| {
                ui.small("Quick:");
                if ui.button("+1,000,000 Cr").clicked() {
                    self.add_credits(1_000_000);
                }
                if ui.button("Cr = 999,999,999").clicked() {
                    self.set_path_value("Economy", Json::from(999_999_999i64));
                }
                if ui.button("Coins = 999,999").clicked() {
                    self.set_path_value("Coins", Json::from(999_999i64));
                }
            });

            ui.add_space(6.0);

            // fields grid
            ui.group(|ui| {
                ui.strong("Save data (shown as in-game)");
                ui.separator();
                egui::Grid::new("fields").num_columns(2).spacing([12.0, 5.0]).show(ui, |ui| {
                    for field in &mut self.fields {
                        ui.vertical(|ui| {
                            ui.label(&field.label);
                            ui.small(egui::RichText::new(&field.hint).color(egui::Color32::from_gray(110)));
                        });
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut field.text).desired_width(f32::INFINITY),
                        );
                        if r.changed() {
                            self.dirty = true;
                        }
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new(egui::RichText::new("Save").strong()).min_size(egui::vec2(72.0, 30.0));
                    if ui.add_enabled(self.dirty, save_btn).clicked() {
                        self.save_all();
                    }
                    if ui.button("Reload").clicked() {
                        self.reload_save();
                        self.msg("reloaded from disk", true);
                    }
                    ui.small("Save always creates a timestamped backup first, then re-verifies the file.");
                });
            });

            ui.add_space(6.0);

            // advanced: set any path
            if egui::CollapsingHeader::new("Advanced - set any path")
                .default_open(self.show_advanced)
                .show(ui, |ui| {
                    ui.label("Path (e.g. GarageSlots.0.CarID, LeagueStatus_.ChampPrices.2):");
                    if let Some(hint) = app::field_hint(self.advanced_path.trim()) {
                        ui.small(egui::RichText::new(hint).color(egui::Color32::from_gray(110)));
                    }
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.advanced_path).desired_width(240.0));
                        ui.label("=");
                        ui.add(egui::TextEdit::singleline(&mut self.advanced_value).desired_width(180.0));
                        if ui.button("Set").clicked() {
                            let value = app::parse_value(&self.advanced_value);
                            let path = self.advanced_path.trim().to_string();
                            self.set_path_value(&path, value);
                        }
                    });
                })
                .header_response
                .clicked()
            {
                self.show_advanced = !self.show_advanced;
            }

            // raw JSON
            if egui::CollapsingHeader::new("Raw JSON (whole save)")
                .default_open(self.show_json)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Dump to file…").clicked() {
                            self.dump_to_file();
                        }
                        if ui.button("Apply from file…").clicked() {
                            self.apply_from_file();
                        }
                        if ui.button(if self.dump_view { "Hide preview" } else { "Preview" }).clicked() {
                            self.dump_view = !self.dump_view;
                            if self.dump_view {
                                if let Some(p) = self.save_path.clone() {
                                    self.dump_text =
                                        app::dump_json(&p).unwrap_or_else(|e| format!("error: {e}"));
                                }
                            }
                        }
                    });
                    if self.dump_view {
                        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.dump_text.as_str())
                                    .desired_width(f32::INFINITY)
                                    .font(egui::TextStyle::Monospace),
                            );
                        });
                    }
                })
                .header_response
                .clicked()
            {
                self.show_json = !self.show_json;
            }

            // backups
            if egui::CollapsingHeader::new("Backups")
                .default_open(self.show_backups)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Back up now").clicked() {
                            self.backup_now();
                        }
                        ui.separator();
                        egui::ComboBox::from_id_salt("backup_list")
                            .selected_text(
                                self.backups
                                    .get(self.backup_sel)
                                    .map(|p| file_name(p))
                                    .unwrap_or_else(|| "no backups".into()),
                            )
                            .show_ui(ui, |ui| {
                                for (i, p) in self.backups.clone().iter().enumerate() {
                                    ui.selectable_value(&mut self.backup_sel, i, file_name(p));
                                }
                            });
                        if ui.button("Restore").clicked() {
                            self.restore_selected_backup();
                        }
                        if ui.button("Delete").clicked() {
                            self.delete_selected_backup();
                        }
                    });
                    ui.small("Backups live in a Backups folder next to the save. Restoring the current save first backs it up, too.");
                })
                .header_response
                .clicked()
            {
                self.show_backups = !self.show_backups;
            }

            // low-level crypto
            if egui::CollapsingHeader::new("Decrypt / Encrypt (raw)")
                .default_open(self.show_crypto)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Decrypt save to file…").clicked() {
                            self.decrypt_to_file();
                        }
                        if ui.button("Encrypt file into save…").clicked() {
                            self.encrypt_into_save();
                        }
                    });
                })
                .header_response
                .clicked()
            {
                self.show_crypto = !self.show_crypto;
            }

            ui.add_space(8.0);
        });
    }

    // ---------------------------------------------------- cars section
    fn cars_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.require_game_dir(ui) {
            return;
        }

        ui.add_space(6.0);

        // stats + filter + sort
        ui.horizontal(|ui| {
            ui.strong(format!("Power ranking - {} cars", self.cars.len()));
            ui.separator();
            ui.label("Filter:");
            if ui.add(egui::TextEdit::singleline(&mut self.car_filter).desired_width(200.0)).changed() {
                self.car_page = 0;
            }
            if ui.button("x").clicked() {
                self.car_filter.clear();
                self.car_page = 0;
            }
            ui.separator();
            ui.label("Sort:");
            egui::ComboBox::from_id_salt("car_sort")
                .selected_text(match self.car_sort {
                    SortKey::Rank => "Rank",
                    SortKey::Cv => "CV",
                    SortKey::TopSpeed => "km/h",
                    SortKey::Year => "Year",
                    SortKey::Name => "Name",
                })
                .show_ui(ui, |ui| {
                    let opts = [
                        (SortKey::Rank, "Rank"),
                        (SortKey::Cv, "CV"),
                        (SortKey::TopSpeed, "km/h"),
                        (SortKey::Year, "Year"),
                        (SortKey::Name, "Name"),
                    ];
                    for (k, label) in opts {
                        ui.selectable_value(&mut self.car_sort, k, label);
                    }
                });
            let dir = if self.car_sort_desc { "v desc" } else { "^ asc" };
            if ui.button(dir).clicked() {
                self.car_sort_desc = !self.car_sort_desc;
            }
        });
        ui.small("Official power ranking (uselesslist.txt). Click a card to open the full card with skins.");
        ui.add_space(6.0);

        // compute the displayed order once per frame
        let mut order: Vec<usize> = self
            .cars
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                if self.car_filter.is_empty() {
                    true
                } else {
                    c.name.to_lowercase().contains(&self.car_filter.to_lowercase())
                }
            })
            .map(|(i, _)| i)
            .collect();
        match self.car_sort {
            SortKey::Rank => {}
            SortKey::Cv => order.sort_by_key(|&i| self.cars[i].cv),
            SortKey::TopSpeed => order.sort_by_key(|&i| self.cars[i].top_speed),
            SortKey::Year => order.sort_by_key(|&i| self.cars[i].year),
            SortKey::Name => order.sort_by(|&a, &b| self.cars[a].name.cmp(&self.cars[b].name)),
        }
        if self.car_sort != SortKey::Rank && !self.car_sort_desc {
            order.reverse();
        }

        const PAGE: usize = 12; // 4 columns x 3 rows
        let total = order.len();
        let pages = total.div_ceil(PAGE).max(1);
        if self.car_page >= pages {
            self.car_page = pages - 1;
        }
        let start = self.car_page * PAGE;
        let end = (start + PAGE).min(total);

        egui::ScrollArea::vertical().id_salt("cars_catalog").show(ui, |ui| {
            egui::Grid::new("cars_grid")
                .num_columns(4)
                .spacing([14.0, 14.0])
                .show(ui, |ui| {
                    let mut col = 0;
                    for &i in &order[start..end] {
                        let (name, cv, top_speed, year, rally, skin_dirs_len, skin_dirs) = {
                            let car = &self.cars[i];
                            (
                                car.name.clone(),
                                car.cv,
                                car.top_speed,
                                car.year,
                                car.rally,
                                car.skin_dirs.len(),
                                car.skin_dirs.clone(),
                            )
                        };
                        let preview = self
                            .car_preview_cache
                            .entry(i)
                            .or_insert_with(|| cars::pick_preview(&skin_dirs))
                            .clone();
                        let rank = i + 1; // file order == official rank
                        let selected = self.selected_car == Some(i);
                        ui.vertical(|ui| {
                            let img_w = 160.0;
                            ui.allocate_ui(egui::vec2(img_w, 104.0), |ui| {
                                match preview.as_ref().and_then(|p| self.texture(ctx, p)) {
                                    Some(tex) => {
                                        let ts = tex.size_vec2();
                                        let scale = (img_w / ts.x).min(104.0 / ts.y);
                                        if ui
                                            .add(egui::Image::new((tex.id(), ts * scale)).sense(egui::Sense::click()))
                                            .clicked()
                                        {
                                            self.select_car(i);
                                            self.car_detail_open = true;
                                        }
                                    }
                                    None => {
                                        ui.label(egui::RichText::new("[no skin]").color(egui::Color32::from_gray(150)));
                                    }
                                }
                            });
                            let title = if rally { format!("{name} (rally)") } else { name };
                            if ui
                                .add(egui::SelectableLabel::new(
                                    selected,
                                    egui::RichText::new(&title).strong(),
                                ))
                                .clicked()
                            {
                                self.select_car(i);
                                self.car_detail_open = true;
                            }
                            ui.small(format!("#{rank}  {cv} CV  {top_speed} km/h  {year}"));
                            ui.small(format!(
                                "{} - {} skin folder(s)",
                                if rally { "Rally" } else { "GT" },
                                skin_dirs_len
                            ));
                        });
                        col += 1;
                        if col == 4 {
                            ui.end_row();
                            col = 0;
                        }
                    }
                    if col != 0 {
                        ui.end_row();
                    }
                });
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.car_page > 0, egui::Button::new("< Prev"))
                .clicked()
            {
                self.car_page -= 1;
            }
            ui.label(format!("Page {} / {}  ({} cars)", self.car_page + 1, pages, total));
            if ui
                .add_enabled(self.car_page + 1 < pages, egui::Button::new("Next >"))
                .clicked()
            {
                self.car_page += 1;
            }
        });

        // floating detail window
        if self.car_detail_open {
            if let Some(i) = self.selected_car {
                let car = &self.cars[i];
                let title = format!("{}{}", car.name, if car.rally { " (rally)" } else { "" });
                let (cv, top_speed, year, skin_dirs_len) = (car.cv, car.top_speed, car.year, car.skin_dirs.len());
                let mut open = self.car_detail_open;
                egui::Window::new(&title)
                    .collapsible(false)
                    .open(&mut open)
                    .resizable(true)
                    .default_width(420.0)
                    .show(ctx, |ui| {
                        ui.small(format!(
                            "#{rank}  {cv} CV  -  {top_speed} km/h  -  {year}  -  {skin_dirs_len} skin folder(s)",
                            rank = i + 1,
                        ));
                        ui.add_space(6.0);
                        self.show_skin_images_preview(ui, ctx);
                    });
                if !open {
                    self.car_detail_open = false;
                }
            } else {
                self.car_detail_open = false;
            }
        }
    }

    // --------------------------------------------------- skins section
    fn skins_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.require_game_dir(ui) {
            return;
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.strong(format!("Car skins - {} files", self.skins_list.len()));
            ui.separator();
            ui.label("Filter:");
            ui.add(egui::TextEdit::singleline(&mut self.skin_filter).desired_width(220.0));
            ui.checkbox(&mut self.hide_masks, "hide Car_bounds masks");
        });
        ui.add_space(6.0);

        let avail = ui.available_size();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(380.0, avail.y),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                let filter = self.skin_filter.to_lowercase();
                egui::ScrollArea::vertical().id_salt("skins_list").show(ui, |ui| {
                    let mut last_brand = String::new();
                    let entries: Vec<usize> = self
                        .skins_list
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| {
                            if self.hide_masks && s.kind == skins::SkinKind::Mask {
                                return false;
                            }
                            filter.is_empty()
                                || s.car.to_lowercase().contains(&filter)
                                || s.brand.to_lowercase().contains(&filter)
                        })
                        .map(|(i, _)| i)
                        .collect();
                    for i in entries {
                        let s = &self.skins_list[i];
                        if s.brand != last_brand {
                            ui.label(egui::RichText::new(&s.brand).strong().color(egui::Color32::from_rgb(0, 90, 160)));
                            last_brand = s.brand.clone();
                        }
                        let name = file_name(&s.file);
                        let selected = self.selected_skin == Some(i);
                        let txt = format!("   {}  ({})", s.car, name);
                        if ui.selectable_label(selected, txt).clicked() {
                            self.selected_skin = Some(i);
                            self.selected_skin_images = vec![s.file.clone()];
                            self.selected_skin_image = 0;
                        }
                    }
                });
            },
            );

            ui.separator();
            ui.vertical(|ui| {
                ui.strong("Preview");
                self.show_skin_images_preview(ui, ctx);
            });
        });
    }

    // ---------------------------------------------------- game section
    fn game_section(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.group(|ui| {
            ui.strong("Game folder");
            ui.separator();
            ui.horizontal(|ui| {
                match &self.game_dir {
                    Some(d) => ui.label(d.display().to_string()),
                    None => ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "not set"),
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Choose…").clicked() {
                        self.choose_game_dir();
                    }
                    if ui.button("Auto-detect").clicked() {
                        match game::detect_game_dir() {
                            Some(d) => {
                                self.set_game_dir(d);
                                self.msg("game folder detected", true);
                            }
                            None => self.msg("auto-detect found nothing - choose the folder manually", false),
                        }
                    }
                });
            });
        });

        ui.add_space(8.0);

        match self.game_info.clone() {
            Some(Ok(info)) => {
                ui.group(|ui| {
                    ui.strong("Game info");
                    ui.separator();
                    egui::Grid::new("game_info").num_columns(2).spacing([16.0, 5.0]).show(ui, |ui| {
                        let rows = vec![
                            ("Product", format!("{} ({})", info.app_product, info.app_company)),
                            ("Executable", format!("{} ({})", file_name(&info.exe), fmt_size(info.exe_size))),
                            ("Exe modified", fmt_time(info.exe_modified)),
                            ("Data folder", info.data_dir.display().to_string()),
                            ("Save folder", info.save_dir.display().to_string()),
                            ("Save slots / backups", format!("{} / {}", info.save_slots, info.backups)),
                            ("Cars with skins", format!("{} cars / {} skin files", info.skin_cars, info.car_skins)),
                        ];
                        for (k, v) in rows {
                            ui.label(egui::RichText::new(k).monospace().small());
                            ui.label(v);
                            ui.end_row();
                        }
                    });
                });

                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.strong("Actions");
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Launch game").clicked() {
                            self.launch_game();
                        }
                        if ui.button("Open game folder").clicked() {
                            open_path_or_url(&info.dir.display().to_string());
                        }
                        if ui.button("Open save folder").clicked() {
                            self.open_save_folder();
                        }
                        if ui.button("Open save Backups").clicked() {
                            let p = info.save_dir.join("Backups");
                            open_path_or_url(&p.display().to_string());
                        }
                        if ui.button("Open CustomMusic").clicked() {
                            let p = game::custom_music_dir(&info.dir);
                            open_path_or_url(&p.display().to_string());
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Back up ALL save slots").clicked() {
                            self.backup_all();
                        }
                        if ui.button("Open uselesslist.txt (car power ranking)").clicked() {
                            let p = game::streaming_data_dir(&info.dir).join("uselesslist.txt");
                            open_path_or_url(&p.display().to_string());
                        }
                        if ui.button("Open game Credits.txt").clicked() {
                            let p = game::streaming_data_dir(&info.dir).join("Credits.txt");
                            open_path_or_url(&p.display().to_string());
                        }
                    });
                });

                if !info.boot_config.is_empty() {
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new("boot.config").show(ui, |ui| {
                        for (k, v) in &info.boot_config {
                            ui.label(egui::RichText::new(format!("{k} = {v}")).monospace().small());
                        }
                    });
                }
            }
            Some(Err(e)) => {
                ui.colored_label(egui::Color32::from_rgb(200, 60, 60), format!("game folder invalid: {e}"));
            }
            None => {
                ui.label("Set the game folder above to see details.");
            }
        }
    }

    // --------------------------------------------------- wiki: cars page
    fn wiki_ensure_events(&mut self) {
        if self.wiki_events.is_some() || self.wiki_events_err.is_some() {
            return;
        }
        let Some(gd) = self.game_dir.clone() else {
            self.wiki_events_err = Some("game folder not set".into());
            return;
        };
        match swgp2_tool::tool::events::load_for_game(&gd) {
            Ok(ev) => self.wiki_events = Some(ev),
            Err(e) => self.wiki_events_err = Some(format!("cannot parse level0: {e}")),
        }
    }

    fn wiki_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for t in [
                WikiTab::Cars,
                WikiTab::Championships,
                WikiTab::Tracks,
                WikiTab::Weekends,
                WikiTab::Endurance,
                WikiTab::Rallies,
                WikiTab::Tips,
            ] {
                let btn = egui::Button::new(egui::RichText::new(t.label()).size(13.0))
                    .min_size(egui::vec2(98.0, 30.0))
                    .selected(self.wiki_tab == t);
                if ui.add(btn).clicked() {
                    self.wiki_tab = t;
                }
                ui.add_space(4.0);
            }
        });
        ui.separator();
        match self.wiki_tab {
            WikiTab::Cars => self.wiki_cars_page(ui, ctx),
            WikiTab::Championships => self.wiki_list_page(ui, swgp2_tool::tool::wiki::CHAMPIONSHIPS, "Championships"),
            WikiTab::Tracks => {
                self.wiki_ensure_events();
                if let Some(err) = self.wiki_events_err.clone() {
                    ui.add_space(4.0);
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 60, 60),
                        format!("level0 parse failed: {err} - showing the static list instead"),
                    );
                    self.wiki_list_page(ui, swgp2_tool::tool::wiki::TRACKS, "Tracks");
                } else {
                    self.wiki_events_page(ui, "Tracks", |e| e.kind == swgp2_tool::tool::events::EventKind::Track)
                }
            }
            WikiTab::Weekends => {
                self.wiki_ensure_events();
                let endurance_only = self.endurance_only;
                self.wiki_events_page(ui, "Weekends", |e| {
                    e.kind == swgp2_tool::tool::events::EventKind::Weekend
                        && (!endurance_only || e.is_endurance())
                })
            }
            WikiTab::Endurance => {
                self.wiki_ensure_events();
                self.wiki_events_page(ui, "Endurance", |e| e.is_endurance())
            }
            WikiTab::Rallies => self.wiki_rallies_page(ui),
            WikiTab::Tips => self.wiki_tips_page(ui),
        }
    }

    /// Live events page (tracks / weekends / endurance), built by parsing the
    /// game's own level0 - no curated list. Falls back to the static TRACKS
    /// list for the Tracks tab if the parse fails.
    fn wiki_events_page(&mut self, ui: &mut egui::Ui, title: &str, pred: impl Fn(&swgp2_tool::tool::events::GameEvent) -> bool) {
        if let Some(err) = self.wiki_events_err.clone() {
            self.wiki_error_box(ui, &err, title);
            return;
        }
        let Some(events) = self.wiki_events.clone() else {
            ui.label("No event data.");
            return;
        };
        let is_weekends = title == "Weekends";

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{title} - built live from the game's own data (level0)")).size(16.0).strong());
            ui.separator();
            ui.label("Search:");
            ui.add(egui::TextEdit::singleline(&mut self.event_search).desired_width(180.0));
            if ui.button("x").clicked() {
                self.event_search.clear();
            }
            if is_weekends {
                ui.checkbox(&mut self.endurance_only, "endurance only");
            }
        });
        ui.small("Restrictions (car class / dealership / model / pilot / weather) are read from the records in the game's data files, as stored by the game. Recommendations: best restricted cars by CV, then top speed.");
        ui.add_space(6.0);

        let filter = self.event_search.trim().to_lowercase();
        let matches = |e: &swgp2_tool::tool::events::GameEvent| {
            filter.is_empty()
                || e.name.to_lowercase().contains(&filter)
                || e.restriction_text().to_lowercase().contains(&filter)
                || e.is_endurance()
                    && "endurance".contains(&filter)
        };
        let shown: Vec<swgp2_tool::tool::events::GameEvent> = events
            .iter()
            .filter(|e| pred(e))
            .filter(|e| matches(e))
            .cloned()
            .collect();
        ui.small(format!("{} event(s)", shown.len()));
        ui.add_space(6.0);

        egui::ScrollArea::vertical().id_salt("events_list").show(ui, |ui| {
            for ev in &shown {
                self.event_card(ui, ev);
                ui.add_space(4.0);
            }
            if shown.is_empty() {
                ui.label("Nothing matches the filter.");
            }
        });
    }

    fn wiki_error_box(&mut self, ui: &mut egui::Ui, err: &str, context: &str) {
        ui.add_space(20.0);
        ui.colored_label(egui::Color32::from_rgb(200, 60, 60), format!("{context}: {err}"));
        ui.small("The live parser reads the game's own level0. It may not match this game build — check docs/GAME_FILES.md.");
    }

    fn wiki_rallies_page(&mut self, ui: &mut egui::Ui) {
        self.wiki_ensure_events();
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Rallies - championships and rally stages (from the game's own data)").size(17.0).strong());
        ui.small("Rally championships are the in-game league series (names verified against level0). Rally stages are the timed rally events, parsed live from the game's files. Recommendations are rally cars ranked by official CV, then top speed.");
        ui.add_space(8.0);

        egui::ScrollArea::vertical().id_salt("rallies_page").show(ui, |ui| {
            ui.label(egui::RichText::new("Rally championships").strong().size(15.0));
            ui.add_space(4.0);
            for ch in swgp2_tool::tool::wiki::CHAMPIONSHIPS
                .iter()
                .filter(|c| c.kind == swgp2_tool::tool::wiki::ChampKind::Rally)
            {
                ui.group(|ui| {
                    ui.label(egui::RichText::new(ch.name).strong());
                    ui.label(ch.note);
                    let picks = swgp2_tool::tool::wiki::best_picks(
                        &self.wiki_catalog,
                        ch.kind,
                        ch.class_token,
                        5,
                    );
                    if picks.is_empty() {
                        ui.small("No rally cars with official stats - pick by price and availability.");
                    } else {
                        let txt = picks
                            .iter()
                            .map(|c| {
                                let spec = c
                                    .cv
                                    .map(|cv| format!("{} CV{}", cv, c.year.map(|y| format!(" ({y})")).unwrap_or_default()))
                                    .unwrap_or_else(|| "?".into());
                                format!("{} ({spec})", swgp2_tool::tool::catalog::display_name(c))
                            })
                            .collect::<Vec<_>>()
                            .join("   |   ");
                        ui.label(egui::RichText::new(format!("Top picks: {txt}")).small());
                    }
                });
                ui.add_space(4.0);
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Rally stages").strong().size(15.0));
            ui.small("Parsed live from the game's level files. Stage restrictions are shown as stored by the game.");
            ui.add_space(4.0);
            match self.wiki_events_err.clone().or_else(|| {
                if self.wiki_events.is_none() {
                    Some("game folder not set".to_string())
                } else {
                    None
                }
            }) {
                Some(err) => {
                    ui.colored_label(egui::Color32::from_rgb(200, 60, 60), format!("level0 parse failed: {err}"));
                }
                None => {
                    let stages: Vec<swgp2_tool::tool::events::GameEvent> = self
                        .wiki_events
                        .as_ref()
                        .expect("checked above")
                        .iter()
                        .filter(|e| e.is_rally_stage())
                        .cloned()
                        .collect();
                    ui.small(format!("{} stage(s)", stages.len()));
                    ui.add_space(4.0);
                    for ev in &stages {
                        self.event_card(ui, ev);
                        ui.add_space(4.0);
                    }
                }
            }
        });
    }

    fn event_card(&mut self, ui: &mut egui::Ui, ev: &swgp2_tool::tool::events::GameEvent) {
        let _kind_label = match ev.kind {
            swgp2_tool::tool::events::EventKind::Weekend => "WEEKEND",
            swgp2_tool::tool::events::EventKind::Cup => "CUP",
            swgp2_tool::tool::events::EventKind::Track => "TRACK",
        };
        let (badge_txt, badge_color) = match ev.kind {
            swgp2_tool::tool::events::EventKind::Weekend => ("WEEKEND", egui::Color32::from_rgb(0, 90, 160)),
            swgp2_tool::tool::events::EventKind::Cup => ("CUP", egui::Color32::from_rgb(160, 90, 0)),
            swgp2_tool::tool::events::EventKind::Track => ("TRACK", egui::Color32::from_rgb(90, 90, 90)),
        };
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&ev.name).strong().size(15.0));
                ui.colored_label(badge_color, badge_txt);
                if ev.is_endurance() {
                    ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "ENDURANCE");
                }
                if ev.is_rally_stage() {
                    ui.colored_label(egui::Color32::from_rgb(0, 150, 60), "RALLY");
                }
                if let Some(c) = &ev.class {
                    ui.colored_label(egui::Color32::from_rgb(120, 60, 180), egui::RichText::new(c).monospace());
                }
            });
            ui.small(format!("Entry restriction: {}", ev.restriction_text()));
            let rally_only = ev.is_rally_stage();
            let picks = swgp2_tool::tool::events::best_cars_for_event(ev, &self.wiki_catalog, 5, rally_only);
            if picks.is_empty() {
                ui.small("No ranked cars match this restriction - pick by price and availability.");
            } else {
                let spec = |c: &swgp2_tool::tool::catalog::CatalogCar| match (c.cv, c.top_speed) {
                    (Some(cv), Some(kmh)) => format!("{cv} CV {kmh} km/h"),
                    (Some(cv), None) => format!("{cv} CV"),
                    _ => "?".to_string(),
                };
                let txt = picks
                    .iter()
                    .map(|c| format!("{} ({})", swgp2_tool::tool::catalog::display_name(c), spec(c)))
                    .collect::<Vec<_>>()
                    .join("   |   ");
                ui.label(egui::RichText::new(format!("Top picks: {txt}")).small());
            }
        });
    }

    fn wiki_cars_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.require_game_dir(ui) {
            return;
        }
        let Some(game_dir) = self.game_dir.clone() else { return };
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.small(format!("{} cars in the game (all shop entries). Click a card to open the full card.", self.wiki_catalog.len()));
            ui.separator();
            ui.label("Filter:");
            ui.add(egui::TextEdit::singleline(&mut self.wiki_car_filter).desired_width(160.0));
            egui::ComboBox::from_id_salt("dealer_filter")
                .selected_text(if self.wiki_dealer_filter.is_empty() { "All dealers".to_string() } else { self.wiki_dealer_filter.clone() })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.wiki_dealer_filter, String::new(), "All dealers");
                    for d in ["AALIA", "CINDER", "DECASTRO", "ENHORNING", "MYLENE", "NANWOLF", "RAVEN", "SOOP", "VIJUDA", "ZAPHIR"] {
                        ui.selectable_value(&mut self.wiki_dealer_filter, d.to_string(), d);
                    }
                });
            if ui.button("x").clicked() {
                self.wiki_car_filter.clear();
                self.wiki_dealer_filter.clear();
            }
        });
        ui.add_space(6.0);

        let filter = self.wiki_car_filter.to_lowercase();
        let dealer = self.wiki_dealer_filter.to_uppercase();
        let order: Vec<usize> = self
            .wiki_catalog
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                (dealer.is_empty() || c.dealer.to_uppercase() == dealer)
                    && (filter.is_empty()
                        || c.model.to_lowercase().contains(&filter)
                        || c.real.to_lowercase().contains(&filter)
                        || c.dealer.to_lowercase().contains(&filter))
            })
            .map(|(i, _)| i)
            .collect();
        ui.small(format!("{} matching car(s)", order.len()));
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .id_salt("wiki_gallery_scroll")
            .show(ui, |ui| {
                egui::Grid::new("wiki_gallery_grid")
                    .num_columns(5)
                    .spacing([12.0, 12.0])
                    .show(ui, |ui| {
                        for &ci in &order {
                            let (car_id, name) = {
                                let car = &self.wiki_catalog[ci];
                                (car.id, swgp2_tool::tool::catalog::display_name(car))
                            };
                            ui.vertical(|ui| {
                                let path = swgp2_tool::tool::catalog::thumbnail_path(&game_dir, car_id);
                                let img_w = 150.0;
                                ui.allocate_ui(egui::vec2(img_w, 100.0), |ui| {
                                    if let Some(tex) = self.texture(ctx, &path) {
                                        let ts = tex.size_vec2();
                                        let scale = (img_w / ts.x).min(100.0 / ts.y);
                                        if ui
                                            .add(egui::Image::new((tex.id(), ts * scale)).sense(egui::Sense::click()))
                                            .clicked()
                                        {
                                            self.wiki_selected_car = Some(ci);
                                            self.wiki_detail_open = true;
                                        }
                                    } else {
                                        ui.label("[no image]");
                                    }
                                });
                                if ui
                                    .add(egui::SelectableLabel::new(
                                        self.wiki_selected_car == Some(ci),
                                        egui::RichText::new(&name).small(),
                                    ))
                                    .clicked()
                                {
                                    self.wiki_selected_car = Some(ci);
                                    self.wiki_detail_open = true;
                                }
                            });
                        }
                    });
            });

        // detail window
        if self.wiki_detail_open {
            if let Some(ci) = self.wiki_selected_car {
                let (car_id, name, dealer, model, rally, real, cv, top_speed, year) = {
                    let car = &self.wiki_catalog[ci];
                    (
                        car.id,
                        swgp2_tool::tool::catalog::display_name(car),
                        car.dealer.to_string(),
                        car.model.to_string(),
                        car.rally,
                        if car.real.is_empty() { String::new() } else { car.real.to_string() },
                        car.cv,
                        car.top_speed,
                        car.year,
                    )
                };
                let mut open = self.wiki_detail_open;
                egui::Window::new(&name)
                    .collapsible(false)
                    .open(&mut open)
                    .resizable(false)
                    .show(ctx, |ui| {
                        let path = swgp2_tool::tool::catalog::thumbnail_path(&game_dir, car_id);
                        if let Some(tex) = self.texture(ctx, &path) {
                            let ts = tex.size_vec2();
                            let scale = (360.0 / ts.x).min(1.5);
                            ui.image((tex.id(), ts * scale));
                        }
                        egui::Grid::new("car_detail")
                            .num_columns(2)
                            .spacing([16.0, 6.0])
                            .show(ui, |ui| {
                                let stats = if cv.is_some() {
                                    format!(
                                        "{} CV / {} km/h / {}",
                                        cv.unwrap_or(0),
                                        top_speed.unwrap_or(0),
                                        year.map(|y| y.to_string()).unwrap_or_else(|| "?".into())
                                    )
                                } else {
                                    "not present in the game's official ranking".to_string()
                                };
                                let rows = vec![
                                    ("Car ID", car_id.to_string()),
                                    ("Dealer", dealer.clone()),
                                    ("Model", model.clone() + if rally { " (rally)" } else { "" }),
                                    (
                                        "Real car",
                                        if real.is_empty() { "not catalogued".to_string() } else { real },
                                    ),
                                    ("Stats", stats),
                                    ("Skin folder", format!("{dealer}/{model}")),
                                ];
                                for (k, v) in rows {
                                    ui.label(egui::RichText::new(k).monospace().small());
                                    ui.label(v);
                                    ui.end_row();
                                }
                            });
                    });
                if !open {
                    self.wiki_detail_open = false;
                }
            } else {
                self.wiki_detail_open = false;
            }
        }
    }

    // --------------------------------------------- wiki: list page (championships & tracks)
    fn wiki_list_page(&mut self, ui: &mut egui::Ui, list: &[swgp2_tool::tool::wiki::Championship], title: &str) {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!("{title} - best cars (ranked by the game's official stats)"))
                .size(17.0)
                .strong(),
        );
        ui.small("Car ranking comes from the game's own uselesslist.txt + shop catalog. Recommendations are filtered by the league's class restriction and sorted by official CV, then top speed. Class-restricted entries show only cars of that class.");
        ui.add_space(8.0);

        let no_picks_msg = "No ranked cars of this class exist in the game's ranking - choose by price and availability.";
        egui::ScrollArea::vertical().id_salt("wiki_list").show(ui, |ui| {
            for ch in list {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(ch.name).strong());
                        let (txt, color) = match ch.kind {
                            swgp2_tool::tool::wiki::ChampKind::Rally => ("RALLY", egui::Color32::from_rgb(0, 150, 60)),
                            swgp2_tool::tool::wiki::ChampKind::Circuit => ("CIRCUIT", egui::Color32::from_rgb(0, 90, 160)),
                            swgp2_tool::tool::wiki::ChampKind::Touring => ("TOURING", egui::Color32::from_rgb(160, 90, 0)),
                            swgp2_tool::tool::wiki::ChampKind::Beginner => ("BEGINNER", egui::Color32::from_rgb(120, 120, 120)),
                        };
                        ui.colored_label(color, txt);
                    });
                    ui.label(ch.note);
                    let picks = swgp2_tool::tool::wiki::best_picks(&self.wiki_catalog, ch.kind, ch.class_token, 5);
                    if picks.is_empty() {
                        ui.small(no_picks_msg);
                    } else {
                        let txt = picks
                            .iter()
                            .map(|c| {
                                let spec = c
                                    .cv
                                    .map(|cv| format!("{} CV{}", cv, c.year.map(|y| format!(" ({y})")).unwrap_or_default()))
                                    .unwrap_or_else(|| "?".into());
                                format!("{} ({spec})", swgp2_tool::tool::catalog::display_name(c))
                            })
                            .collect::<Vec<_>>()
                            .join("   |   ");
                        ui.label(egui::RichText::new(format!("Top picks: {txt}")).small());
                    }
                });
                ui.add_space(4.0);
            }
        });
    }

    // --------------------------------------------- wiki: tips page
    fn wiki_tips_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Tips & notes").size(17.0).strong());
        ui.add_space(6.0);
        for tip in swgp2_tool::tool::wiki::TIPS {
            ui.horizontal(|ui| {
                ui.label("-");
                ui.label(*tip);
            });
        }
        ui.add_space(10.0);
        ui.separator();
        ui.small("Data sources (all from the game's own files):");
        ui.label("All 200 cars: StreamingAssets/BrandLogos/Original_names.txt (id=DEALER#MODEL, leading R = rally)");
        ui.label("Dealership previews: StreamingAssets/CarThumbnails/IMD<id>.SW (id matches the save's CarID)");
        ui.label("Power ranking: StreamingAssets/Data/uselesslist.txt (25 cars, CV / km/h / year, 'R' prefix = rally)");
        ui.label("Skins: StreamingAssets/CAR_SKINS/<BRAND>/<CAR>");
        ui.label("Championship & track names: Super Woden GP 2_Data/level0 (championships screen)");
        ui.small("This wiki is a fan-made reference compiled from the game's data files. Not affiliated with ViJuDa.");
    }

    // --------------------------------------------------- reset section
    fn reset_section(&mut self, ui: &mut egui::Ui) {
        let Some(dir) = reset::save_dir() else {
            ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "cannot locate the save folder");
            return;
        };
        ui.add_space(6.0);

        // current saves
        ui.group(|ui| {
            ui.strong("Save folder");
            ui.separator();
            ui.small(dir.display().to_string());
            ui.add_space(4.0);
            let saves = reset::saves_in(&dir);
            if saves.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(0, 150, 60), "no .sav files - the next game start creates fresh saves");
            } else {
                egui::Grid::new("current_saves").num_columns(4).spacing([14.0, 3.0]).show(ui, |ui| {
                    ui.strong("file");
                    ui.strong("size");
                    ui.strong("modified");
                    ui.end_row();
                    for p in &saves {
                        let meta = std::fs::metadata(p).ok();
                        ui.label(file_name(p));
                        ui.label(meta.as_ref().map(|m| fmt_size(m.len())).unwrap_or_else(|| "?".into()));
                        ui.label(fmt_time(meta.and_then(|m| m.modified().ok())));
                        ui.end_row();
                    }
                });
            }
        });

        ui.add_space(8.0);

        // reset action
        ui.group(|ui| {
            ui.strong("Reset ALL saves");
            ui.separator();
            ui.label("Deletes every .sav in the save folder. Before deleting, all saves are archived");
            ui.label("into Backups\\FullReset-<stamp> - you can restore them below at any time.");
            ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "The game must be CLOSED while resetting, or it will re-save over your fresh state.");
            ui.add_space(6.0);

            if self.reset_armed {
                ui.group(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "Really reset ALL saves?");
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Yes, archive & delete").strong())).clicked() {
                            self.do_full_reset();
                        }
                        if ui.button("Cancel").clicked() {
                            self.reset_armed = false;
                        }
                    });
                });
            } else {
                let saves = reset::saves_in(&dir);
                let btn = egui::Button::new(egui::RichText::new(format!("Reset ALL saves ({} file(s))…", saves.len())).size(14.0))
                    .fill(egui::Color32::from_rgb(245, 220, 220));
                if ui.add_enabled(!saves.is_empty(), btn).clicked() {
                    self.reset_armed = true;
                }
            }
        });

        ui.add_space(8.0);

        // recovery archives
        ui.group(|ui| {
            ui.strong("Recovery archives");
            ui.separator();
            if self.archives.is_empty() {
                ui.label("No reset archives yet.");
            } else {
                egui::ComboBox::from_id_salt("archives")
                    .selected_text(
                        self.archives
                            .get(self.archive_sel)
                            .map(|a| format!("{}  ({} file(s), {})", file_name(&a.dir), a.files.len(), fmt_size(a.total_size)))
                            .unwrap_or_else(|| "?".into()),
                    )
                    .show_ui(ui, |ui| {
                        for (i, a) in self.archives.clone().iter().enumerate() {
                            ui.selectable_value(
                                &mut self.archive_sel,
                                i,
                                format!("{}  ({} file(s), {})", file_name(&a.dir), a.files.len(), fmt_size(a.total_size)),
                            );
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button("Restore selected archive").clicked() {
                        self.restore_selected_archive();
                    }
                    if ui.button("Delete archive forever").clicked() {
                        self.delete_selected_archive();
                    }
                });
                ui.small("Restoring moves your current saves aside into a Pre-restore archive first - nothing is ever silently overwritten.");
            }
        });
    }

    // --------------------------------------------------- about section
    fn about_section(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            ui.heading("SWGP2 Tool");
            ui.label(format!("v{} - Super Woden GP 2 save & data tool", env!("CARGO_PKG_VERSION")));
            ui.add_space(8.0);
            ui.hyperlink_to("github.com/rival-afk/swgp2-tool", "https://github.com/rival-afk/swgp2-tool");
            ui.add_space(4.0);
            ui.label("Unofficial fan tool. Use at your own risk. Not affiliated with ViJuDa.");
        });
        ui.add_space(16.0);
        ui.separator();
        ui.small("Safety model");
        ui.label("Every write creates a timestamped backup next to the save, then re-encrypts and re-parses the file to verify it. If verification fails, the backup is restored automatically. The game must be closed while editing - it overwrites the save on exit.");
        ui.add_space(8.0);
        ui.small("Game credits (StreamingAssets/Data/Credits.txt)");
        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            let text = self
                .game_dir
                .as_ref()
                .and_then(|d| std::fs::read_to_string(game::streaming_data_dir(d).join("Credits.txt")).ok())
                .unwrap_or_else(|| {
                    "(game folder not set - open the Game section and locate it)".to_string()
                });
            ui.label(egui::RichText::new(text).monospace().size(11.0));
        });
    }

    // ------------------------------------------------- shared pieces
    fn require_game_dir(&mut self, ui: &mut egui::Ui) -> bool {
        if self.game_dir.is_some() {
            return true;
        }
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(200, 60, 60),
                "Game folder not set",
            );
            ui.label("The tool can auto-detect a SWGP2 install or you can pick the folder manually.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Auto-detect").clicked() {
                    match game::detect_game_dir() {
                        Some(d) => {
                            self.set_game_dir(d);
                            self.msg("game folder detected", true);
                        }
                        None => self.msg("auto-detect found nothing", false),
                    }
                }
                if ui.button("Choose folder…").clicked() {
                    self.choose_game_dir();
                }
            });
        });
        false
    }

    fn show_skin_images_preview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.selected_skin_images.is_empty() {
            ui.label("No skin images selected.");
            return;
        }
        let idx = self.selected_skin_image.min(self.selected_skin_images.len() - 1);
        self.selected_skin_image = idx;
        let path = self.selected_skin_images[idx].clone();
        if let Some(tex) = self.texture(ctx, &path) {
            let max = egui::vec2(330.0, 240.0);
            let ts = tex.size_vec2();
            let scale = (max.x / ts.x).min(max.y / ts.y).min(2.0).max(0.01);
            ui.image((tex.id(), ts * scale));
        } else {
            ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "cannot decode image");
        }
        ui.small(file_name(&path));
        ui.horizontal(|ui| {
            if self.selected_skin_images.len() > 1 {
                if ui.button("<").clicked() && self.selected_skin_image > 0 {
                    self.selected_skin_image -= 1;
                }
                ui.label(format!("{}/{}", self.selected_skin_image + 1, self.selected_skin_images.len()));
                if ui.button(">").clicked() && self.selected_skin_image + 1 < self.selected_skin_images.len() {
                    self.selected_skin_image += 1;
                }
            }
            if ui.button("Open file").clicked() {
                open_path_or_url(&path.display().to_string());
            }
        });
        ui.add_space(4.0);
        if self.selected_skin_images.len() > 1 {
            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                for (i, p) in self.selected_skin_images.clone().iter().enumerate() {
                    let selected = i == self.selected_skin_image;
                    if ui.selectable_label(selected, file_name(p)).clicked() {
                        self.selected_skin_image = i;
                    }
                }
            });
        }
    }

    // --------------------------------------------------- actions
    fn pick_save(&mut self, ctx: &egui::Context) {
        let _ = ctx;
        if let Some(p) = rfd::FileDialog::new().add_filter("SWGP2 save", &["sav"]).pick_file() {
            self.save_path = Some(p);
            self.reload_save();
        }
    }

    fn backup_now(&mut self) {
        let Some(p) = self.save_path.clone() else { return };
        match save::create_backup(&p) {
            Ok(b) => {
                self.msg(format!("backup created: {}", file_name(&b)), true);
                self.refresh_backups();
            }
            Err(e) => self.msg(format!("backup failed: {e}"), false),
        }
    }

    fn backup_all(&mut self) {
        let slots = save::list_slots();
        let mut n = 0;
        for p in &slots {
            if save::create_backup(p).is_ok() {
                n += 1;
            }
        }
        self.msg(format!("backed up {n} save slot(s)"), true);
        self.refresh_backups();
        self.refresh_game_info();
    }

    fn restore_selected_backup(&mut self) {
        let Some(p) = self.save_path.clone() else { return };
        let Some(b) = self.backups.get(self.backup_sel).cloned() else {
            self.msg("no backup selected", false);
            return;
        };
        match app::restore_backup(&p, &b) {
            Ok(cur) => {
                self.msg(format!("restored {} (previous save: {})", file_name(&b), file_name(&cur)), true);
                self.reload_save();
            }
            Err(e) => self.msg(format!("restore failed: {e}"), false),
        }
    }

    fn delete_selected_backup(&mut self) {
        let Some(b) = self.backups.get(self.backup_sel).cloned() else { return };
        match std::fs::remove_file(&b) {
            Ok(()) => {
                self.msg(format!("deleted {}", file_name(&b)), true);
                self.refresh_backups();
            }
            Err(e) => self.msg(format!("delete failed: {e}"), false),
        }
    }

    fn dump_to_file(&mut self) {
        let Some(p) = self.save_path.clone() else { return };
        if let Some(out) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name("swgp2-dump.json")
            .save_file()
        {
            match app::dump_json(&p).and_then(|j| std::fs::write(&out, j).map_err(|e| e.to_string())) {
                Ok(()) => self.msg(format!("dumped to {}", out.display()), true),
                Err(e) => self.msg(format!("dump failed: {e}"), false),
            }
        }
    }

    fn apply_from_file(&mut self) {
        let Some(p) = self.save_path.clone() else { return };
        if let Some(file) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
            match app::apply_json_file(&p, &file) {
                Ok(b) => {
                    self.msg(format!("applied (backup: {})", file_name(&b)), true);
                    self.reload_save();
                }
                Err(e) => self.msg(format!("apply failed: {e}"), false),
            }
        }
    }

    fn decrypt_to_file(&mut self) {
        let Some(p) = self.save_path.clone() else { return };
        if let Some(out) = rfd::FileDialog::new().set_file_name("save-decrypted.bin").save_file() {
            match std::fs::read(&p)
                .map_err(|e| e.to_string())
                .and_then(|raw| swgp2_tool::crypto::decrypt_save(&raw))
                .and_then(|plain| std::fs::write(&out, plain).map_err(|e| e.to_string()))
            {
                Ok(()) => self.msg(format!("decrypted to {}", out.display()), true),
                Err(e) => self.msg(format!("decrypt failed: {e}"), false),
            }
        }
    }

    fn encrypt_into_save(&mut self) {
        let Some(p) = self.save_path.clone() else { return };
        if let Some(file) = rfd::FileDialog::new().add_filter("BinaryFormatter stream", &["bin"]).pick_file() {
            // back up first, then encrypt over the save, then verify
            match save::create_backup(&p)
                .map_err(|e| e.to_string())
                .and_then(|backup| {
                    let plain = std::fs::read(&file).map_err(|e| e.to_string())?;
                    let enc = swgp2_tool::crypto::encrypt_save(&plain);
                    std::fs::write(&p, enc).map_err(|e| e.to_string())?;
                    Ok::<_, String>(backup)
                }) {
                Ok(backup) => {
                    self.msg(format!("encrypted into save (backup: {})", file_name(&backup)), true);
                    self.reload_save();
                }
                Err(e) => self.msg(format!("encrypt failed: {e}"), false),
            }
        }
    }

    fn add_credits(&mut self, add: i64) {
        let Some(p) = self.save_path.clone() else { return };
        let cur = app::load(&p)
            .ok()
            .and_then(|g| save::get_path(&g, "Economy").ok())
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        self.set_path_value("Economy", Json::from(cur + add));
    }

    fn select_car(&mut self, i: usize) {
        self.selected_car = Some(i);
        let car = &self.cars[i];
        self.selected_skin_images = cars::list_skin_images(&car.skin_dirs);
        // prefer the base render first for a nicer default view
        self.selected_skin_images.sort_by_key(|p| {
            let n = file_name(p).to_lowercase();
            if n.contains("base") { 0 } else { 1 }
        });
        self.selected_skin_image = 0;
    }

    fn open_save_folder(&mut self) {
        let dir = self
            .save_path
            .as_ref()
            .and_then(|p| p.parent().map(PathBuf::from))
            .or_else(|| save::default_save_path(0).and_then(|p| p.parent().map(PathBuf::from)));
        if let Some(d) = dir {
            open_path_or_url(&d.display().to_string());
        }
    }

    fn launch_game(&mut self) {
        let Some(gd) = self.game_dir.clone() else {
            self.msg("game folder not set", false);
            return;
        };
        let exe = gd.join(game::EXE_NAME);
        if self.dirty {
            self.msg("you have unsaved edits - saving is recommended before launching", false);
        }
        match std::process::Command::new(&exe).current_dir(&gd).spawn() {
            Ok(_) => self.msg("game launched", true),
            Err(e) => self.msg(format!("cannot launch {}: {e}", exe.display()), false),
        }
    }

    fn choose_game_dir(&mut self) {
        if let Some(d) = rfd::FileDialog::new().set_title("Choose SWGP2 game folder").pick_folder() {
            if d.join(game::EXE_NAME).is_file() {
                self.set_game_dir(d);
                self.msg("game folder set", true);
            } else {
                self.msg(format!("{} does not contain {EXE}", d.display(), EXE = game::EXE_NAME), false);
            }
        }
    }
}

fn open_path_or_url(s: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(s).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(s).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(s).spawn();
    }
}
