//! swgp2 - graphical save editor for Super Woden GP 2. No flags, just run it.

use eframe::egui;
use serde_json::Value as Json;
use std::path::PathBuf;
use swgp2_save_editor::app;

fn main() -> eframe::Result<()> {
    if std::env::args().len() > 1 {
        eprintln!("swgp2 takes no arguments - just launch it");
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 640.0])
            .with_min_inner_size([460.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SWGP2 Save Editor",
        options,
        Box::new(|_cc| Ok(Box::new(Editor::new()))),
    )
}

struct Field {
    path: String,
    label: String,
    text: String,
}

struct Editor {
    slots: Vec<PathBuf>,
    slot: usize,
    save_path: Option<PathBuf>,
    fields: Vec<Field>,
    advanced_path: String,
    advanced_value: String,
    dump_view: bool,
    dump_text: String,
    backups: Vec<PathBuf>,
    backup_sel: usize,
    message: (String, egui::Color32),
}

impl Editor {
    fn new() -> Self {
        let slots = app::slot_choices();
        let mut ed = Editor {
            slots,
            slot: 0,
            save_path: None,
            fields: Vec::new(),
            advanced_path: String::new(),
            advanced_value: String::new(),
            dump_view: false,
            dump_text: String::new(),
            backups: Vec::new(),
            backup_sel: 0,
            message: (String::new(), egui::Color32::GRAY),
        };
        ed.reload();
        ed
    }

    fn msg(&mut self, text: impl Into<String>, ok: bool) {
        self.message = (
            text.into(),
            if ok { egui::Color32::from_rgb(80, 200, 120) } else { egui::Color32::from_rgb(235, 90, 90) },
        );
    }

    fn reload(&mut self) {
        self.save_path = self.slots.get(self.slot).cloned();
        self.fields.clear();
        self.refresh_backups();
        let Some(path) = self.save_path.clone() else {
            self.msg("no save found - has the game been run yet?", false);
            return;
        };
        match app::load(&path) {
            Ok(g) => {
                self.fields = app::COMMON_FIELDS
                    .iter()
                    .map(|(p, l)| Field {
                        path: p.to_string(),
                        label: l.to_string(),
                        text: app::field_text(&g, p),
                    })
                    .collect();
                self.msg(format!("loaded {}", path.display()), true);
            }
            Err(e) => self.msg(format!("cannot load save: {e}"), false),
        }
    }

    fn refresh_backups(&mut self) {
        self.backups = match &self.save_path {
            Some(p) => swgp2_save_editor::save::list_backups(p).unwrap_or_default(),
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
        match app::save_with_backup(&path, &edits) {
            Ok(backup) => {
                self.msg(format!("saved - backup: {}", backup.display()), true);
                self.reload();
            }
            Err(e) => self.msg(format!("save failed: {e}"), false),
        }
    }
}

impl eframe::App for Editor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("SWGP2 Save Editor");
                ui.separator();
                egui::ComboBox::from_id_salt("slot")
                    .selected_text(
                        self.save_path
                            .as_ref()
                            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned())
                            .unwrap_or_else(|| "no save".into()),
                    )
                    .show_ui(ui, |ui| {
                        for (i, p) in self.slots.clone().iter().enumerate() {
                            if ui.selectable_value(&mut self.slot, i, p.file_name().unwrap_or_default().to_string_lossy()).changed() {
                                self.reload();
                            }
                        }
                    });
                if ui.button("Open file…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("SWGP2 save", &["sav"])
                        .pick_file()
                    {
                        self.slots = vec![p];
                        self.slot = 0;
                        self.reload();
                    }
                }
            });
            if let Some(p) = &self.save_path {
                ui.small(p.display().to_string());
            }
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.colored_label(self.message.1, &self.message.0);
            ui.small(" Close the game before saving - it overwrites the file on exit. Every write makes a backup first.");
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.strong("Save data");
                    ui.separator();
                    egui::Grid::new("fields").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                        for field in &mut self.fields {
                            ui.label(&field.label);
                            ui.add(egui::TextEdit::singleline(&mut field.text).desired_width(f32::INFINITY));
                            ui.end_row();
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button(egui::RichText::new("Save (auto-backup)").strong()).clicked() {
                            self.save_all();
                        }
                        if ui.button("Reload from disk").clicked() {
                            self.reload();
                        }
                    });
                });

                ui.add_space(8.0);
                egui::CollapsingHeader::new("Advanced - set any path")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label("Path (e.g. GarageSlots.0.CarID, LeagueStatus_.ChampPrices.2):");
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.advanced_path).desired_width(220.0));
                            ui.label("=");
                            ui.add(egui::TextEdit::singleline(&mut self.advanced_value).desired_width(160.0));
                            if ui.button("Set").clicked() {
                                if let Some(p) = self.save_path.clone() {
                                    let value = app::parse_value(&self.advanced_value);
                                    match app::set_single(&p, self.advanced_path.trim(), value) {
                                        Ok(b) => {
                                            self.msg(format!("set {} (backup: {})", self.advanced_path, b.display()), true);
                                            self.reload();
                                        }
                                        Err(e) => self.msg(format!("set failed: {e}"), false),
                                    }
                                }
                            }
                        });
                    });

                ui.add_space(4.0);
                egui::CollapsingHeader::new("Raw JSON")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Dump to file…").clicked() {
                                if let Some(p) = self.save_path.clone() {
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
                            }
                            if ui.button("Apply from file…").clicked() {
                                if let Some(p) = self.save_path.clone() {
                                    if let Some(file) = rfd::FileDialog::new()
                                        .add_filter("JSON", &["json"])
                                        .pick_file()
                                    {
                                        match app::apply_json_file(&p, &file) {
                                            Ok(b) => {
                                                self.msg(format!("applied (backup: {})", b.display()), true);
                                                self.reload();
                                            }
                                            Err(e) => self.msg(format!("apply failed: {e}"), false),
                                        }
                                    }
                                }
                            }
                            if ui.button(if self.dump_view { "Hide preview" } else { "Preview" }).clicked() {
                                self.dump_view = !self.dump_view;
                                if self.dump_view {
                                    if let Some(p) = self.save_path.clone() {
                                        self.dump_text = app::dump_json(&p).unwrap_or_else(|e| format!("error: {e}"));
                                    }
                                }
                            }
                        });
                        if self.dump_view {
                            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                ui.add(egui::TextEdit::multiline(&mut self.dump_text.as_str()).desired_width(f32::INFINITY));
                            });
                        }
                    });

                ui.add_space(4.0);
                egui::CollapsingHeader::new("Backups")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Back up now").clicked() {
                                if let Some(p) = self.save_path.clone() {
                                    match swgp2_save_editor::save::create_backup(&p) {
                                        Ok(b) => {
                                            self.msg(format!("backup created: {}", b.display()), true);
                                            self.refresh_backups();
                                        }
                                        Err(e) => self.msg(format!("backup failed: {e}"), false),
                                    }
                                }
                            }
                            ui.separator();
                            egui::ComboBox::from_id_salt("backup")
                                .selected_text(
                                    self.backups
                                        .get(self.backup_sel)
                                        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned())
                                        .unwrap_or_else(|| "no backups".into()),
                                )
                                .show_ui(ui, |ui| {
                                    for (i, p) in self.backups.clone().iter().enumerate() {
                                        ui.selectable_value(&mut self.backup_sel, i, p.file_name().unwrap_or_default().to_string_lossy());
                                    }
                                });
                            if ui.button("Restore").clicked() {
                                if let (Some(p), Some(b)) = (self.save_path.clone(), self.backups.get(self.backup_sel).cloned()) {
                                    match app::restore_backup(&p, &b) {
                                        Ok(cur) => {
                                            self.msg(format!("restored {} (previous save: {})", b.display(), cur.display()), true);
                                            self.reload();
                                        }
                                        Err(e) => self.msg(format!("restore failed: {e}"), false),
                                    }
                                }
                            }
                        });
                    });
            });
        });
    }
}
