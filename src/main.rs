use eframe::egui;
use egui::{CentralPanel, Context, TextEdit, TopBottomPanel};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_extras::StripBuilder;
use rfd::FileDialog;
use std::fs;


fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "md-echo - edit/preview",
        options,
        Box::new(|_cc| Box::<MarkdownApp>::default()),
    )
}

struct MarkdownApp {
    content: String,
    original_content: String,
    file_path: Option<String>,
    cache: CommonMarkCache,
    modified: bool,
    show_exit_confirm: bool,
    pending_new: bool,
    pending_open: bool,
    pending_save: bool,
    pending_save_as: bool,
    pending_exit: bool,
    scroll_left: f32,
    scroll_right: f32,
    current_line: usize,
}



impl Default for MarkdownApp {
    fn default() -> Self {
        Self {
            content: String::new(),
            original_content: String::new(),
            file_path: None,
            cache: CommonMarkCache::default(),
            modified: false,
            show_exit_confirm: false,
            pending_new: false,
            pending_open: false,
            pending_save: false,
            pending_save_as: false,
            pending_exit: false,
            scroll_left: 0.0,
            scroll_right: 0.0,
            current_line: 0,
        }
    }
}

impl eframe::App for MarkdownApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Handle hotkeys
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Q) && i.modifiers.ctrl {
                if self.modified {
                    self.show_exit_confirm = true;
                } else {
                    self.pending_exit = true;
                }
            }
            if i.key_pressed(egui::Key::N) && i.modifiers.ctrl {
                self.pending_new = true;
            }
            if i.key_pressed(egui::Key::O) && i.modifiers.ctrl {
                self.pending_open = true;
            }
            if i.key_pressed(egui::Key::S) && i.modifiers.ctrl {
                if i.modifiers.shift {
                    self.pending_save_as = true;
                } else {
                    self.pending_save = true;
                }
            }
        });

        // ==== MENU BAR ====
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        if self.confirm_discard(ui) {
                            self.content.clear();
                            self.original_content.clear();
                            self.file_path = None;
                            self.modified = false;
                        }
                        ui.close_menu();
                    }

                    if ui.button("Open").clicked() {
                        if self.confirm_discard(ui) {
                            if let Some(path) = FileDialog::new().pick_file() {
                                if let Ok(data) = fs::read_to_string(&path) {
                                    self.content = data.clone();
                                    self.original_content = data;
                                    self.file_path = Some(path.display().to_string());
                                    self.modified = false;
                                }
                            }
                        }
                        ui.close_menu();
                    }

                    if ui.button("Save").clicked() {
                        self.save_file(false);
                        ui.close_menu();
                    }

                    if ui.button("Save As...").clicked() {
                        self.save_file(true);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("Exit").clicked() {
                        if self.modified {
                            self.show_exit_confirm = true;
                        } else {
                            std::process::exit(0);
                        }
                    }
                });
            });
        });

        // ==== CENTRAL PANEL ====
        CentralPanel::default().show(ctx, |ui| {
            // Handle pending actions from hotkeys
            if self.pending_exit {
                self.pending_exit = false;
                std::process::exit(0);
            }
            if self.pending_new {
                self.pending_new = false;
                if self.confirm_discard(ui) {
                    self.content.clear();
                    self.original_content.clear();
                    self.file_path = None;
                    self.modified = false;
                }
            }
            if self.pending_open {
                self.pending_open = false;
                if self.confirm_discard(ui) {
                    if let Some(path) = FileDialog::new().pick_file() {
                        if let Ok(data) = fs::read_to_string(&path) {
                            self.content = data.clone();
                            self.original_content = data;
                            self.file_path = Some(path.display().to_string());
                            self.modified = false;
                        }
                    }
                }
            }
            if self.pending_save {
                self.pending_save = false;
                self.save_file(false);
            }
            if self.pending_save_as {
                self.pending_save_as = false;
                self.save_file(true);
            }

            StripBuilder::new(ui)
                .size(egui_extras::Size::relative(0.5))
                .size(egui_extras::Size::relative(0.5))
                .horizontal(|mut strip| {
                    // LEFT: Editor inside ScrollArea
                    strip.cell(|ui| {
                        let scroll =
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    let editor_output = TextEdit::multiline(&mut self.content)
                                        .desired_width(f32::INFINITY)
                                        .code_editor()
                                        .show(ui);

                                    let response = editor_output.response;

                                    if response.has_focus() {
                                        if let Some(cursor_range) = editor_output.cursor_range {
                                            self.current_line = cursor_range.primary.rcursor.row;
                                        }
                                    }

                                    if response.changed() {
                                        self.modified = self.content != self.original_content;
                                    }
                                });
                        self.scroll_left = scroll.state.offset.y;
                    });

                    strip.cell(|ui| {
                        let line_height = ui.text_style_height(&egui::TextStyle::Body);
                        let target_scroll_y = self.current_line as f32 * line_height;
                        let scroll = egui::ScrollArea::vertical()
                            .auto_shrink([false; 2])
                            .scroll_offset(egui::vec2(0.0, target_scroll_y))
                            .show(ui, |ui| {
                                CommonMarkViewer::new("full_markdown").show(
                                    ui,
                                    &mut self.cache,
                                    &self.content,
                                );
                            });
                        self.scroll_right = scroll.state.offset.y;
                    });
                });
        });

        // ==== STATUS BAR ====
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let name = self.file_path.as_deref().unwrap_or("Untitled");
                ui.label(format!(
                    "{}{}",
                    if self.modified { "🟡 " } else { "🟢 " },
                    name
                ));
                ui.separator();
                ui.label(format!("✍️ {} chars", self.content.len()));
            });
        });

        // ==== EXIT CONFIRMATION ====
        if self.show_exit_confirm {
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("You have unsaved changes. Save before exiting?");
                    ui.horizontal(|ui| {
                        if ui.button("Save and Exit").clicked() {
                            self.save_file(false);
                            std::process::exit(0);
                        }
                        if ui.button("Discard and Exit").clicked() {
                            std::process::exit(0);
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_exit_confirm = false;
                        }
                    });
                });
        }
    }
}

impl MarkdownApp {
    fn save_file(&mut self, save_as: bool) {
        if save_as || self.file_path.is_none() {
            if let Some(path) = FileDialog::new().save_file() {
                if let Err(err) = fs::write(&path, &self.content) {
                    eprintln!("Save error: {}", err);
                } else {
                    self.file_path = Some(path.display().to_string());
                    self.original_content = self.content.clone();
                    self.modified = false;
                }
            }
        } else if let Some(path) = &self.file_path {
            if let Err(err) = fs::write(path, &self.content) {
                eprintln!("Save error: {}", err);
            } else {
                self.original_content = self.content.clone();
                self.modified = false;
            }
        }
    }

    fn confirm_discard(&self, ui: &mut egui::Ui) -> bool {
        if self.modified {
            let mut confirmed = false;
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("Discard unsaved changes?");
                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            confirmed = true;
                        }
                        if ui.button("No").clicked() {
                            confirmed = false;
                        }
                    });
                });
            confirmed
        } else {
            true
        }
    }
}
