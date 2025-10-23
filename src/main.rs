use directories::ProjectDirs;
use eframe::egui;
use egui::{CentralPanel, Context, TextEdit, TopBottomPanel};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_extras::StripBuilder;
use rfd::FileDialog;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> eframe::Result<()> {
    let args: Vec<String> = env::args().collect();
    let app = if args.len() > 1 {
        let file_path = &args[1];
        match fs::read_to_string(file_path) {
            Ok(content) => MarkdownApp {
                content: content.clone(),
                original_content: content,
                file_path: Some(file_path.clone()),
                ..Default::default()
            },
            Err(e) => {
                eprintln!("Error reading file '{}': {}", file_path, e);
                MarkdownApp::default()
            }
        }
    } else {
        MarkdownApp::default()
    };
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "md-echo - edit/preview",
        options,
        Box::new(move |_cc| Box::new(app)),
    )
}

struct MarkdownApp {
    content: String,
    original_content: String,
    file_path: Option<String>,
    working_dir: PathBuf,
    config_path: Option<PathBuf>,
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
        let (working_dir, config_path) = MarkdownApp::initial_working_directory();
        Self {
            content: String::new(),
            original_content: String::new(),
            file_path: None,
            working_dir,
            config_path,
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
                                self.open_file_from_path(&path);
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
                        self.open_file_from_path(&path);
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
                .size(egui_extras::Size::relative(0.25).at_least(160.0))
                .size(egui_extras::Size::relative(0.35))
                .size(egui_extras::Size::relative(0.40))
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        ui.vertical(|ui| {
                            self.show_file_tree(ui);
                        });
                    });

                    // MIDDLE: Editor inside ScrollArea
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

                    // RIGHT: Preview
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
    fn initial_working_directory() -> (PathBuf, Option<PathBuf>) {
        let default_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let config_path = MarkdownApp::config_file_path();

        if let Some(path) = &config_path {
            if let Ok(contents) = fs::read_to_string(path) {
                let candidate = PathBuf::from(contents.trim());
                if candidate.is_dir() {
                    return (candidate, config_path);
                }
            }
        }

        (default_dir, config_path)
    }

    fn config_file_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "fibnas", "md-echo")
            .map(|dirs| dirs.config_dir().join("settings.txt"))
    }

    fn save_working_directory(&self) {
        if let Some(config_path) = &self.config_path {
            if let Some(parent) = config_path.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    eprintln!("Config directory error: {}", err);
                    return;
                }
            }

            if let Err(err) = fs::write(config_path, self.working_dir.display().to_string()) {
                eprintln!("Config write error: {}", err);
            }
        }
    }

    fn set_working_directory(&mut self, new_dir: PathBuf) {
        if new_dir.is_dir() {
            self.working_dir = new_dir;
            self.save_working_directory();
        } else {
            eprintln!("Invalid working directory: {}", new_dir.display());
        }
    }

    fn show_file_tree(&mut self, ui: &mut egui::Ui) {
        ui.label("Working Directory");
        ui.monospace(self.working_dir.display().to_string());

        if ui.button("Change...").clicked() {
            let mut dialog = FileDialog::new();
            if self.working_dir.is_dir() {
                dialog = dialog.set_directory(&self.working_dir);
            }
            if let Some(path) = dialog.pick_folder() {
                self.set_working_directory(path);
            }
        }

        ui.separator();

        if !self.working_dir.is_dir() {
            ui.label("Working directory is unavailable.");
            return;
        }

        let root_dir = self.working_dir.clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.render_directory(ui, &root_dir, true);
            });
    }

    fn render_directory(&mut self, ui: &mut egui::Ui, path: &Path, is_root: bool) {
        let name = if is_root {
            path.display().to_string()
        } else {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string())
        };

        let header = egui::CollapsingHeader::new(name)
            .id_source(path.display().to_string())
            .default_open(is_root);

        header.show(ui, |ui| match fs::read_dir(path) {
            Ok(entries) => {
                let mut directories = Vec::new();
                let mut files = Vec::new();

                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        directories.push(entry_path);
                    } else {
                        files.push(entry_path);
                    }
                }

                directories.sort();
                files.sort();

                for dir in directories {
                    self.render_directory(ui, &dir, false);
                }

                for file in files {
                    let file_name = file
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| file.display().to_string());

                    let file_path_string = file.display().to_string();
                    let is_selected = self
                        .file_path
                        .as_deref()
                        .map(|current| current == file_path_string.as_str())
                        .unwrap_or(false);

                    let response = ui
                        .selectable_label(is_selected, file_name)
                        .on_hover_text(file_path_string);
                    if response.clicked() {
                        if !self.modified || self.confirm_discard(ui) {
                            self.open_file_from_path(&file);
                        }
                    }
                }
            }
            Err(err) => {
                ui.label(format!("Cannot read {}: {}", path.display(), err));
            }
        });
    }

    fn open_file_from_path(&mut self, path: &Path) {
        match fs::read_to_string(path) {
            Ok(data) => {
                self.content = data.clone();
                self.original_content = data;
                self.file_path = Some(path.display().to_string());
                self.modified = false;
            }
            Err(err) => {
                eprintln!("Error reading file '{}': {}", path.display(), err);
            }
        }
    }

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
