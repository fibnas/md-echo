use directories::ProjectDirs;
use eframe::egui;
use egui::{CentralPanel, Color32, Context, TextEdit, TopBottomPanel, Visuals};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_extras::StripBuilder;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{Builder, NamedTempFile};

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
    config: AppConfig,
    cache: CommonMarkCache,
    modified: bool,
    show_exit_confirm: bool,
    pending_new: bool,
    pending_open: bool,
    pending_save: bool,
    pending_save_as: bool,
    pending_exit: bool,
    pending_lint: bool,
    pending_format: bool,
    tool_output: Option<String>,
    show_tool_output: bool,
    theme_applied: bool,
    scroll_left: f32,
    scroll_right: f32,
    current_line: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct AppConfig {
    working_dir: Option<PathBuf>,
    theme: ThemeConfig,
    tools: ToolsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            working_dir: None,
            theme: ThemeConfig::default(),
            tools: ToolsConfig::default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct ThemeConfig {
    base: String,
    background: Option<String>,
    panel: Option<String>,
    text: Option<String>,
    accent: Option<String>,
    hyperlink: Option<String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            base: "dark".to_string(),
            background: None,
            panel: None,
            text: None,
            accent: None,
            hyperlink: None,
        }
    }
}

impl ThemeConfig {
    fn to_visuals(&self) -> Visuals {
        let mut visuals = match self.base.to_lowercase().as_str() {
            "light" => Visuals::light(),
            _ => Visuals::dark(),
        };

        if let Some(color) = self
            .background
            .as_deref()
            .and_then(|value| parse_color(value))
        {
            visuals.window_fill = color;
            visuals.panel_fill = color;
            visuals.extreme_bg_color = color;
        }

        if let Some(color) = self.panel.as_deref().and_then(|value| parse_color(value)) {
            visuals.panel_fill = color;
        }

        if let Some(color) = self.text.as_deref().and_then(|value| parse_color(value)) {
            visuals.override_text_color = Some(color);
        }

        if let Some(color) = self.accent.as_deref().and_then(|value| parse_color(value)) {
            visuals.selection.bg_fill = color;
            visuals.widgets.active.bg_fill = color;
            visuals.widgets.hovered.bg_fill = color;
            visuals.hyperlink_color = color;
        }

        if let Some(color) = self
            .hyperlink
            .as_deref()
            .and_then(|value| parse_color(value))
        {
            visuals.hyperlink_color = color;
        }

        visuals
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct ToolsConfig {
    lint: Option<Vec<String>>,
    lint_use_open_file: bool,
    format: Option<Vec<String>>,
    format_use_open_file: bool,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            lint: default_lint_command(),
            lint_use_open_file: false,
            format: default_format_command(),
            format_use_open_file: false,
        }
    }
}

fn default_lint_command() -> Option<Vec<String>> {
    Some(vec!["rumdl".to_string(), "check".to_string()])
}

fn default_format_command() -> Option<Vec<String>> {
    Some(vec!["rumdl".to_string(), "fmt".to_string()])
}

fn parse_color(value: &str) -> Option<Color32> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color32::from_rgb(r, g, b))
    } else if hex.len() == 8 {
        let a = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let r = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let g = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let b = u8::from_str_radix(&hex[6..8], 16).ok()?;
        Some(Color32::from_rgba_unmultiplied(r, g, b, a))
    } else {
        None
    }
}

impl Default for MarkdownApp {
    fn default() -> Self {
        let (mut config, config_path) = MarkdownApp::load_config();
        let working_dir = MarkdownApp::initial_working_directory(&mut config);
        let app = Self {
            content: String::new(),
            original_content: String::new(),
            file_path: None,
            working_dir,
            config_path,
            config,
            cache: CommonMarkCache::default(),
            modified: false,
            show_exit_confirm: false,
            pending_new: false,
            pending_open: false,
            pending_save: false,
            pending_save_as: false,
            pending_exit: false,
            pending_lint: false,
            pending_format: false,
            tool_output: None,
            show_tool_output: false,
            theme_applied: false,
            scroll_left: 0.0,
            scroll_right: 0.0,
            current_line: 0,
        };

        if app
            .config_path
            .as_ref()
            .map(|path| !path.exists())
            .unwrap_or(false)
        {
            app.save_config();
        }

        app
    }
}

impl eframe::App for MarkdownApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.ensure_theme(ctx);
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
            if i.key_pressed(egui::Key::L) && i.modifiers.ctrl && i.modifiers.shift {
                self.pending_lint = true;
            }
            if i.key_pressed(egui::Key::F) && i.modifiers.ctrl && i.modifiers.shift {
                self.pending_format = true;
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

                ui.menu_button("Tools", |ui| {
                    if ui.button("Lint Markdown	Ctrl+Shift+L").clicked() {
                        self.pending_lint = true;
                        ui.close_menu();
                    }
                    if ui.button("Format Markdown	Ctrl+Shift+F").clicked() {
                        self.pending_format = true;
                        ui.close_menu();
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
            if self.pending_lint {
                self.pending_lint = false;
                self.run_lint_tool();
            }
            if self.pending_format {
                self.pending_format = false;
                self.run_format_tool();
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

        if self.show_tool_output {
            let mut open = true;
            egui::Window::new("Tool Output")
                .open(&mut open)
                .resizable(true)
                .vscroll(true)
                .show(ctx, |ui| {
                    if let Some(output) = &self.tool_output {
                        ui.monospace(output);
                    } else {
                        ui.label("No output available.");
                    }
                });
            if !open {
                self.show_tool_output = false;
            }
        }

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
    fn load_config() -> (AppConfig, Option<PathBuf>) {
        let config_path = MarkdownApp::config_file_path();
        let mut config = AppConfig::default();

        if let Some(path) = &config_path {
            if path.exists() {
                match fs::read_to_string(path) {
                    Ok(contents) => match toml::from_str::<AppConfig>(&contents) {
                        Ok(parsed) => config = parsed,
                        Err(err) => eprintln!("Config parse error ({}): {}", path.display(), err),
                    },
                    Err(err) => eprintln!("Config read error ({}): {}", path.display(), err),
                }
            }
        }

        if config.working_dir.is_none() {
            if let Some(legacy_path) = MarkdownApp::legacy_settings_path() {
                if let Ok(contents) = fs::read_to_string(&legacy_path) {
                    let candidate = PathBuf::from(contents.trim());
                    if candidate.is_dir() {
                        config.working_dir = Some(candidate);
                    }
                }
            }
        }

        (config, config_path)
    }

    fn initial_working_directory(config: &mut AppConfig) -> PathBuf {
        if let Some(dir) = &config.working_dir {
            if dir.is_dir() {
                return dir.clone();
            }
        }

        let default_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        config.working_dir = Some(default_dir.clone());
        default_dir
    }

    fn config_file_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "fibnas", "md-echo")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    fn legacy_settings_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "fibnas", "md-echo")
            .map(|dirs| dirs.config_dir().join("settings.txt"))
    }

    fn save_config(&self) {
        if let Some(config_path) = &self.config_path {
            if let Some(parent) = config_path.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    eprintln!("Config directory error: {}", err);
                    return;
                }
            }

            match toml::to_string_pretty(&self.config) {
                Ok(serialized) => {
                    if let Err(err) = fs::write(config_path, serialized) {
                        eprintln!("Config write error: {}", err);
                    }
                }
                Err(err) => eprintln!("Config serialize error: {}", err),
            }
        }
    }

    fn ensure_theme(&mut self, ctx: &Context) {
        if self.theme_applied {
            return;
        }
        let visuals = self.config.theme.to_visuals();
        ctx.set_visuals(visuals);
        self.theme_applied = true;
    }

    fn set_working_directory(&mut self, new_dir: PathBuf) {
        if new_dir.is_dir() {
            self.working_dir = new_dir.clone();
            self.config.working_dir = Some(new_dir);
            self.save_config();
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

    fn run_lint_tool(&mut self) {
        match self.config.tools.lint.clone() {
            Some(command) => {
                self.run_external_tool(&command, false, self.config.tools.lint_use_open_file)
            }
            None => self.show_tool_message(
                "No lint command configured. Add a [tools] lint entry to config.toml.",
            ),
        }
    }

    fn run_format_tool(&mut self) {
        match self.config.tools.format.clone() {
            Some(command) => {
                self.run_external_tool(&command, true, self.config.tools.format_use_open_file)
            }
            None => self.show_tool_message(
                "No format command configured. Add a [tools] format entry to config.toml.",
            ),
        }
    }

    fn run_external_tool(
        &mut self,
        command: &[String],
        modifies_content: bool,
        use_current_file: bool,
    ) {
        if command.is_empty() {
            self.show_tool_message("Configured tool command is empty.");
            return;
        }

        let mut temp_file: Option<NamedTempFile> = None;
        let target_path = if use_current_file {
            if self.modified {
                self.show_tool_message(
                    "Save before running this tool on the current file, or disable *_use_open_file.",
                );
                return;
            }
            match self.file_path.as_ref() {
                Some(path_str) => {
                    let path = PathBuf::from(path_str);
                    if path.is_file() {
                        path
                    } else {
                        self.show_tool_message(format!(
                            "Current file path '{}' is not a file.",
                            path.display()
                        ));
                        return;
                    }
                }
                None => {
                    self.show_tool_message(
                        "No file is currently open. Save the document first or disable *_use_open_file.",
                    );
                    return;
                }
            }
        } else {
            let file = match self.create_temp_markdown() {
                Ok(file) => file,
                Err(err) => {
                    self.show_tool_message(format!("Failed to prepare temp file: {}", err));
                    return;
                }
            };
            let path = file.path().to_path_buf();
            temp_file = Some(file);
            path
        };

        let mut cmd = Command::new(&command[0]);
        for arg in &command[1..] {
            cmd.arg(arg);
        }
        if self.working_dir.is_dir() {
            cmd.current_dir(&self.working_dir);
        }
        cmd.arg(&target_path);

        let output = match cmd.output() {
            Ok(output) => output,
            Err(err) => {
                self.show_tool_message(format!("Failed to run '{}': {}", command[0], err));
                return;
            }
        };

        if let Some(file) = temp_file.as_mut() {
            if let Err(err) = file.flush() {
                eprintln!("Temp file flush error: {}", err);
            }
        }

        let mut message = String::new();
        message.push_str(&format!(
            "$ {}\n",
            Self::format_command_for_display(command, &target_path)
        ));
        message.push_str(&format!("Status: {:?}\n", output.status));

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            message.push_str("\nstdout:\n");
            message.push_str(stdout.trim_end());
            message.push('\n');
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            message.push_str("\nstderr:\n");
            message.push_str(stderr.trim_end());
            message.push('\n');
        }

        if modifies_content && output.status.success() {
            match fs::read_to_string(&target_path) {
                Ok(new_content) => {
                    if new_content != self.content {
                        self.content = new_content;
                        self.modified = self.content != self.original_content;
                    }
                }
                Err(err) => {
                    message.push_str(&format!(
                        "\nFormat note: failed to read formatter output ({}): {}\n",
                        target_path.display(),
                        err
                    ));
                }
            }
        }

        self.show_tool_message(message);
    }

    fn format_command_for_display(command: &[String], path: &Path) -> String {
        let mut parts = command.to_vec();
        parts.push(path.display().to_string());
        parts.join(" ")
    }

    fn create_temp_markdown(&self) -> std::io::Result<NamedTempFile> {
        let mut file = Builder::new().suffix(".md").tempfile()?;
        file.write_all(self.content.as_bytes())?;
        file.flush()?;
        Ok(file)
    }

    fn show_tool_message<S: Into<String>>(&mut self, message: S) {
        self.tool_output = Some(message.into());
        self.show_tool_output = true;
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
