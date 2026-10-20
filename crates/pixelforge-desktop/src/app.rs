use directories::ProjectDirs;
use eframe::egui::{self, Color32, RichText, TextureHandle};
use pixelforge_core::{
    ConversionOptions, ConversionReport, FORMAT_CAPABILITIES, ImageInfo, OutputFormat,
    ResizeBounds, inspect, read_image,
};
use pixelforge_jobs::{BatchRequest, JobState, Worker, WorkerEvent};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    output_dir: Option<PathBuf>,
    preset: String,
    formats: Vec<OutputFormat>,
    quality: u8,
    resize_enabled: bool,
    resize_width: u32,
    resize_height: u32,
    webp_lossless: bool,
    strip_metadata: bool,
    recursive: bool,
    jpeg_background: [u8; 3],
}

#[derive(Serialize, Deserialize)]
struct SettingsFile {
    schema_version: u32,
    settings: Settings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: None,
            preset: "Web optimized".into(),
            formats: vec![OutputFormat::Webp],
            quality: 85,
            resize_enabled: false,
            resize_width: 1920,
            resize_height: 1080,
            webp_lossless: false,
            strip_metadata: true,
            recursive: true,
            jpeg_background: [255; 3],
        }
    }
}

struct PreviewData {
    path: PathBuf,
    result: Result<(ImageInfo, [usize; 2], Vec<u8>), String>,
}

pub struct PixelForgeApp {
    settings: Settings,
    settings_path: Option<PathBuf>,
    last_saved: Vec<u8>,
    queue: Vec<PathBuf>,
    states: Vec<JobState>,
    selected: Option<usize>,
    worker: Worker,
    running: bool,
    paused: bool,
    progress: (usize, usize),
    current_item: Option<PathBuf>,
    summary: String,
    last_reports: Vec<ConversionReport>,
    preview_sender: Sender<PreviewData>,
    preview_receiver: Receiver<PreviewData>,
    preview_path: Option<PathBuf>,
    preview: Option<(ImageInfo, TextureHandle)>,
    preview_error: Option<String>,
}

impl PixelForgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings_path = ProjectDirs::from("com", "PixelForge", "PixelForge")
            .map(|dirs| dirs.config_dir().join("settings.json"));
        let saved = settings_path.as_ref().and_then(|path| fs::read(path).ok());
        let stored = saved
            .as_ref()
            .and_then(|data| serde_json::from_slice::<SettingsFile>(data).ok());
        let settings = stored
            .as_ref()
            .filter(|file| file.schema_version == 1)
            .map(|file| file.settings.clone())
            .unwrap_or_default();
        set_theme(&cc.egui_ctx);
        let last_saved = if stored.as_ref().is_some_and(|file| file.schema_version == 1) {
            saved.unwrap_or_default()
        } else {
            Vec::new()
        };
        let (preview_sender, preview_receiver) = mpsc::channel();
        Self {
            settings,
            settings_path,
            last_saved,
            queue: Vec::new(),
            states: Vec::new(),
            selected: None,
            worker: Worker::new(),
            running: false,
            paused: false,
            progress: (0, 0),
            current_item: None,
            summary: "Drop images here or add files to get started.".into(),
            last_reports: Vec::new(),
            preview_sender,
            preview_receiver,
            preview_path: None,
            preview: None,
            preview_error: None,
        }
    }

    fn persist(&mut self) {
        let Some(path) = &self.settings_path else {
            return;
        };
        let Ok(bytes) = serde_json::to_vec_pretty(&SettingsFile {
            schema_version: 1,
            settings: self.settings.clone(),
        }) else {
            return;
        };
        if bytes == self.last_saved {
            return;
        }
        if let Some(parent) = path.parent()
            && fs::create_dir_all(parent).is_ok()
            && fs::write(path, &bytes).is_ok()
        {
            self.last_saved = bytes;
        }
    }

    fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let mut added = Vec::new();
        for path in paths {
            let candidates = if path.is_dir() {
                collect_images(&path, self.settings.recursive)
            } else {
                vec![path]
            };
            for candidate in candidates {
                if candidate.is_file() && !self.queue.contains(&candidate) {
                    self.queue.push(candidate.clone());
                    self.states.push(JobState::Pending);
                    added.push(candidate);
                }
            }
        }
        if let Some(path) = added.last() {
            self.selected = self.queue.iter().position(|item| item == path);
            self.summary = format!(
                "Added {} image{} to the queue.",
                added.len(),
                plural(added.len())
            );
            self.refresh_preview();
        }
    }

    fn remove_selected(&mut self) {
        if self.running {
            return;
        }
        if let Some(index) = self.selected.filter(|index| *index < self.queue.len()) {
            self.queue.remove(index);
            self.states.remove(index);
            self.selected = if self.queue.is_empty() {
                None
            } else {
                Some(index.min(self.queue.len() - 1))
            };
            self.preview_path = None;
            self.refresh_preview();
        }
    }

    fn apply_preset(&mut self, name: &str) {
        if self.running {
            return;
        }
        self.settings.preset = name.to_owned();
        match name {
            "Web optimized" => {
                self.settings.formats = vec![OutputFormat::Webp, OutputFormat::Avif];
                self.settings.quality = 82;
                self.settings.webp_lossless = false;
                self.settings.resize_enabled = false;
            }
            "Smallest practical" => {
                self.settings.formats = vec![OutputFormat::Avif];
                self.settings.quality = 58;
                self.settings.webp_lossless = false;
                self.settings.resize_enabled = false;
            }
            "Lossless conversion" => {
                self.settings.formats = vec![OutputFormat::Png, OutputFormat::Webp];
                self.settings.quality = 100;
                self.settings.webp_lossless = true;
                self.settings.resize_enabled = false;
            }
            "Transparent assets" => {
                self.settings.formats = vec![OutputFormat::Png, OutputFormat::Webp];
                self.settings.quality = 90;
                self.settings.webp_lossless = true;
                self.settings.resize_enabled = false;
            }
            "Email attachment" => {
                self.settings.formats = vec![OutputFormat::Jpeg];
                self.settings.quality = 68;
                self.settings.resize_enabled = true;
                self.settings.resize_width = 1600;
                self.settings.resize_height = 1200;
            }
            _ => self.settings.preset = "Custom".into(),
        }
    }

    fn move_selected(&mut self, offset: isize) {
        if self.running {
            return;
        }
        let Some(index) = self.selected else {
            return;
        };
        let target = index.saturating_add_signed(offset);
        if target >= self.queue.len() {
            return;
        }
        self.queue.swap(index, target);
        self.states.swap(index, target);
        self.selected = Some(target);
    }

    fn retry_failed(&mut self) {
        if self.running {
            return;
        }
        let inputs: Vec<PathBuf> = self
            .queue
            .iter()
            .zip(&self.states)
            .filter(|(_, state)| **state == JobState::Failed)
            .map(|(path, _)| path.clone())
            .collect();
        if inputs.is_empty() {
            return;
        }
        let options = ConversionOptions {
            quality: self.settings.quality,
            resize: self.settings.resize_enabled.then_some(ResizeBounds {
                width: self.settings.resize_width,
                height: self.settings.resize_height,
            }),
            jpeg_background: self.settings.jpeg_background,
            webp_lossless: self.settings.webp_lossless,
            strip_metadata: self.settings.strip_metadata,
        };
        match self.worker.submit(BatchRequest {
            inputs: inputs.clone(),
            output_dir: self.output_dir(),
            formats: self.settings.formats.clone(),
            options,
        }) {
            Ok(()) => {
                self.running = true;
                self.paused = false;
                for input in &inputs {
                    if let Some(index) = self.queue.iter().position(|path| path == input) {
                        self.states[index] = JobState::Pending;
                    }
                }
                self.summary = format!(
                    "Retrying {} failed image{}.",
                    inputs.len(),
                    plural(inputs.len())
                );
            }
            Err(error) => self.summary = error.to_string(),
        }
    }

    fn refresh_preview(&mut self) {
        let Some(path) = self
            .selected
            .and_then(|index| self.queue.get(index))
            .cloned()
        else {
            self.preview = None;
            self.preview_error = None;
            self.preview_path = None;
            return;
        };
        if self.preview_path.as_ref() == Some(&path) {
            return;
        }
        self.preview_path = Some(path.clone());
        self.preview = None;
        self.preview_error = None;
        let sender = self.preview_sender.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let info = inspect(&path).map_err(|error| error.to_string())?;
                let (_, image) = read_image(&path).map_err(|error| error.to_string())?;
                let thumb = image.thumbnail(720, 460).to_rgba8();
                let size = [thumb.width() as usize, thumb.height() as usize];
                Ok((info, size, thumb.into_raw()))
            })();
            let _ = sender.send(PreviewData { path, result });
        });
    }

    fn poll_previews(&mut self, ctx: &egui::Context) {
        while let Ok(data) = self.preview_receiver.try_recv() {
            if self.preview_path.as_ref() != Some(&data.path) {
                continue;
            }
            match data.result {
                Ok((info, size, rgba)) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                    let texture = ctx.load_texture(
                        "selected-image-preview",
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.preview = Some((info, texture));
                    self.preview_error = None;
                }
                Err(error) => self.preview_error = Some(error),
            }
        }
    }

    fn poll_worker(&mut self) {
        while let Ok(event) = self.worker.try_recv() {
            match event {
                WorkerEvent::BatchStarted { total } => {
                    self.running = true;
                    self.paused = false;
                    self.progress = (0, total);
                    self.last_reports.clear();
                }
                WorkerEvent::ItemStarted { input, total, .. } => {
                    self.current_item = Some(input.clone());
                    if let Some(index) = self.queue.iter().position(|path| *path == input) {
                        self.states[index] = JobState::Processing;
                    }
                    self.progress.1 = total;
                }
                WorkerEvent::ItemFinished {
                    input,
                    result,
                    completed,
                    total,
                    ..
                } => {
                    self.progress = (completed, total);
                    match result {
                        Ok(report) => {
                            self.last_reports.push(report);
                            if let Some(index) = self.queue.iter().position(|path| *path == input) {
                                self.states[index] = JobState::Completed;
                            }
                        }
                        Err(error) => {
                            if let Some(index) = self.queue.iter().position(|path| *path == input) {
                                self.states[index] = JobState::Failed;
                            }
                            self.summary = format!(
                                "{}: {error}",
                                input.file_name().unwrap_or_default().to_string_lossy()
                            );
                        }
                    }
                }
                WorkerEvent::BatchFinished {
                    succeeded,
                    failed,
                    cancelled,
                } => {
                    self.running = false;
                    self.paused = false;
                    self.current_item = None;
                    self.summary = if cancelled {
                        format!(
                            "Stopped after {succeeded} conversion{} and {failed} failure{}.",
                            plural(succeeded),
                            plural(failed)
                        )
                    } else {
                        format!(
                            "Finished: {succeeded} conversion{} completed, {failed} conversion{} failed.",
                            plural(succeeded),
                            plural(failed)
                        )
                    };
                    if cancelled {
                        for state in &mut self.states {
                            if !state.terminal() {
                                *state = JobState::Cancelled;
                            }
                        }
                    }
                }
            }
        }
    }

    fn start_conversion(&mut self) {
        if self.queue.is_empty() || self.settings.formats.is_empty() {
            return;
        }
        let options = ConversionOptions {
            quality: self.settings.quality,
            resize: self.settings.resize_enabled.then_some(ResizeBounds {
                width: self.settings.resize_width,
                height: self.settings.resize_height,
            }),
            jpeg_background: self.settings.jpeg_background,
            webp_lossless: self.settings.webp_lossless,
            strip_metadata: self.settings.strip_metadata,
        };
        match self.worker.submit(BatchRequest {
            inputs: self.queue.clone(),
            output_dir: self.output_dir(),
            formats: self.settings.formats.clone(),
            options,
        }) {
            Ok(()) => {
                self.running = true;
                self.paused = false;
                self.states.fill(JobState::Pending);
                self.summary = "Preparing conversions…".into();
            }
            Err(error) => self.summary = error.to_string(),
        }
    }

    fn output_dir(&self) -> PathBuf {
        self.settings.output_dir.clone().unwrap_or_else(|| {
            self.queue
                .first()
                .and_then(|path| path.parent())
                .unwrap_or_else(|| Path::new("."))
                .join("PixelForge Output")
        })
    }
}

impl eframe::App for PixelForgeApp {
    #[allow(clippy::collapsible_if)] // Keep file dialogs visibly nested under the button clicks.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_worker();
        self.poll_previews(&ctx);
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_paths(dropped);
        }

        egui::Panel::top("brand_bar").show(ui, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("◆")
                        .size(24.0)
                        .color(Color32::from_rgb(164, 113, 255)),
                );
                ui.vertical(|ui| {
                    ui.label(RichText::new("PixelForge").size(21.0).strong());
                    ui.label(
                        RichText::new("Private, offline image conversion")
                            .size(12.0)
                            .color(Color32::GRAY),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(if self.running {
                            "●  PROCESSING"
                        } else {
                            "●  READY"
                        })
                        .size(11.0)
                        .color(if self.running {
                            Color32::from_rgb(255, 188, 86)
                        } else {
                            Color32::from_rgb(105, 222, 166)
                        }),
                    );
                });
            });
            ui.add_space(5.0);
        });

        egui::Panel::bottom("actions").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.summary).color(Color32::from_rgb(190, 196, 210)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.running {
                        if ui
                            .button(if self.paused { "Resume" } else { "Pause" })
                            .clicked()
                        {
                            self.paused = !self.paused;
                            if self.paused {
                                self.worker.pause();
                            } else {
                                self.worker.resume();
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.worker.cancel();
                            self.summary = "Cancelling after the current image…".into();
                        }
                    } else if ui
                        .add_enabled(
                            !self.queue.is_empty() && !self.settings.formats.is_empty(),
                            egui::Button::new(RichText::new("Convert images  →").strong()),
                        )
                        .clicked()
                    {
                        self.start_conversion();
                    }
                });
            });
            if self.progress.1 > 0 {
                ui.add_space(4.0);
                ui.add(
                    egui::ProgressBar::new(self.progress.0 as f32 / self.progress.1 as f32)
                        .text(format!("{} / {} outputs", self.progress.0, self.progress.1)),
                );
            }
            ui.add_space(4.0);
        });

        egui::Panel::left("settings")
            .resizable(true)
            .default_size(320.0)
            .show(ui, |ui| {
                settings_panel(self, ui);
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.running, egui::Button::new("＋  Add images"))
                    .clicked()
                {
                    if let Some(paths) = rfd::FileDialog::new()
                        .add_filter(
                            "Images",
                            &[
                                "png", "jpg", "jpeg", "webp", "avif", "gif", "bmp", "tif", "tiff",
                                "ico", "qoi", "tga", "pnm", "ppm", "pgm", "hdr", "exr", "ff",
                                "dds",
                            ],
                        )
                        .pick_files()
                    {
                        self.add_paths(paths);
                    }
                }
                if ui
                    .add_enabled(!self.running, egui::Button::new("▣  Add folder"))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.add_paths([path]);
                    }
                }
                if ui
                    .add_enabled(
                        !self.running && self.selected.is_some(),
                        egui::Button::new("Remove selected"),
                    )
                    .clicked()
                {
                    self.remove_selected();
                }
                if ui
                    .add_enabled(
                        !self.running && self.selected.is_some_and(|index| index > 0),
                        egui::Button::new("Move up"),
                    )
                    .clicked()
                {
                    self.move_selected(-1);
                }
                if ui
                    .add_enabled(
                        !self.running
                            && self
                                .selected
                                .is_some_and(|index| index + 1 < self.queue.len()),
                        egui::Button::new("Move down"),
                    )
                    .clicked()
                {
                    self.move_selected(1);
                }
                if ui
                    .add_enabled(
                        !self.running && self.states.contains(&JobState::Failed),
                        egui::Button::new("Retry failed"),
                    )
                    .clicked()
                {
                    self.retry_failed();
                }
                if ui
                    .add_enabled(
                        !self.running && !self.queue.is_empty(),
                        egui::Button::new("Clear queue"),
                    )
                    .clicked()
                {
                    self.queue.clear();
                    self.states.clear();
                    self.selected = None;
                    self.preview_path = None;
                    self.refresh_preview();
                }
                ui.separator();
                ui.checkbox(&mut self.settings.recursive, "Include subfolders");
            });
            ui.add_space(8.0);
            queue_panel(self, ui);
        });

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
        self.persist();
    }
}

#[allow(clippy::collapsible_if)] // Keep the folder picker visibly nested under its button click.
fn settings_panel(app: &mut PixelForgeApp, ui: &mut egui::Ui) {
    ui.heading("Export settings");
    ui.add_space(5.0);
    let mut preset = app.settings.preset.clone();
    ui.add_enabled_ui(!app.running, |ui| {
        egui::ComboBox::from_label("Preset")
            .selected_text(&preset)
            .show_ui(ui, |ui| {
                for name in [
                    "Web optimized",
                    "Smallest practical",
                    "Lossless conversion",
                    "Transparent assets",
                    "Email attachment",
                    "Custom",
                ] {
                    ui.selectable_value(&mut preset, name.to_owned(), name);
                }
            });
    });
    if preset != app.settings.preset {
        app.apply_preset(&preset);
    }
    ui.label(
        RichText::new("OUTPUT FORMATS")
            .size(10.0)
            .strong()
            .color(Color32::GRAY),
    );
    ui.horizontal_wrapped(|ui| {
        for format in OutputFormat::PRIMARY {
            let mut checked = app.settings.formats.contains(&format);
            if ui
                .add_enabled(
                    !app.running,
                    egui::Checkbox::new(&mut checked, format.label()),
                )
                .changed()
            {
                if checked {
                    app.settings.formats.push(format);
                } else {
                    app.settings.formats.retain(|item| *item != format);
                }
                app.settings.preset = "Custom".into();
            }
        }
    });
    ui.add_space(6.0);
    ui.add_enabled(
        !app.running,
        egui::Checkbox::new(
            &mut app.settings.strip_metadata,
            "Remove AI tags & metadata",
        ),
    );
    ui.label(
        RichText::new("Extra cleanup removes EXIF/GPS, XMP/IPTC, comments, profiles, and unknown chunks from PNG, JPEG, and WebP.")
            .size(10.0)
            .color(Color32::GRAY),
    );
    ui.label(
        RichText::new("Every export is re-encoded from pixels; this cannot guarantee removal of pixel-embedded marks.")
            .size(10.0)
            .color(Color32::GRAY),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new("OUTPUT FOLDER")
            .size(10.0)
            .strong()
            .color(Color32::GRAY),
    );
    let folder = app.output_dir();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(folder.to_string_lossy())
                .size(10.0)
                .color(Color32::LIGHT_GRAY),
        );
        if ui
            .add_enabled(!app.running, egui::Button::new("Choose…"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new().set_directory(&folder).pick_folder() {
                app.settings.output_dir = Some(path);
            }
        }
    });
    ui.add_space(6.0);
    ui.label(
        RichText::new("QUALITY")
            .size(10.0)
            .strong()
            .color(Color32::GRAY),
    );
    let quality_is_used = app.settings.formats.iter().any(|format| {
        matches!(format, OutputFormat::Jpeg | OutputFormat::Avif)
            || (*format == OutputFormat::Webp && !app.settings.webp_lossless)
    });
    ui.add_enabled(
        !app.running && quality_is_used,
        egui::Slider::new(&mut app.settings.quality, 1..=100).suffix("%"),
    );
    ui.label(
        RichText::new("JPEG, lossy WebP, and AVIF")
            .size(10.0)
            .color(Color32::GRAY),
    );
    if app.settings.formats.contains(&OutputFormat::Webp) {
        ui.add_enabled(
            !app.running,
            egui::Checkbox::new(&mut app.settings.webp_lossless, "WebP lossless"),
        );
    }
    ui.add_space(5.0);
    ui.add_enabled(
        !app.running,
        egui::Checkbox::new(&mut app.settings.resize_enabled, "Resize to fit"),
    );
    ui.horizontal(|ui| {
        ui.add_enabled(
            app.settings.resize_enabled && !app.running,
            egui::DragValue::new(&mut app.settings.resize_width)
                .range(1..=20_000)
                .prefix("W ")
                .suffix(" px"),
        );
        ui.add_enabled(
            app.settings.resize_enabled && !app.running,
            egui::DragValue::new(&mut app.settings.resize_height)
                .range(1..=20_000)
                .prefix("H ")
                .suffix(" px"),
        );
    });
    ui.label(
        RichText::new("Keeps the aspect ratio and never enlarges.")
            .size(10.0)
            .color(Color32::GRAY),
    );
    if app.settings.formats.contains(&OutputFormat::Jpeg) {
        ui.add_space(5.0);
        ui.label(RichText::new("JPEG transparency background").size(11.0));
        ui.color_edit_button_srgb(&mut app.settings.jpeg_background);
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Format capabilities").show(ui, |ui| {
        egui::ScrollArea::vertical()
            .max_height(150.0)
            .show(ui, |ui| {
                for capability in FORMAT_CAPABILITIES {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(capability.name).strong().size(11.0));
                        ui.label(
                            RichText::new(if capability.writes {
                                "read · write"
                            } else {
                                "read only"
                            })
                            .size(10.0)
                            .color(Color32::GRAY),
                        );
                    });
                    ui.label(
                        RichText::new(format!(
                            "Alpha {} · lossless {} · quality {} · {}",
                            yes_no(capability.alpha),
                            yes_no(capability.lossless),
                            yes_no(capability.quality_control),
                            capability.bit_depth,
                        ))
                        .size(10.0),
                    );
                    ui.label(
                        RichText::new(capability.notes)
                            .size(10.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(3.0);
                }
            });
    });
    ui.add_space(8.0);
    ui.separator();
    ui.heading("Selected image");
    if let Some((info, texture)) = &app.preview {
        ui.add(egui::Image::new(texture).max_size(egui::vec2(ui.available_width(), 210.0)));
        ui.add_space(3.0);
        ui.label(
            RichText::new(info.path.file_name().unwrap_or_default().to_string_lossy())
                .strong()
                .size(12.0),
        );
        ui.label(format!(
            "{}  ·  {} × {}  ·  {}  ·  {}",
            info.format
                .map(|f| format!("{f:?}"))
                .unwrap_or_else(|| "Unknown".into()),
            info.width,
            info.height,
            format_bytes(info.bytes),
            info.bit_depth
        ));
        ui.label(if info.has_alpha {
            "Transparency: yes"
        } else {
            "Transparency: no"
        });
        ui.label("Metadata is stripped when exporting.");
    } else if let Some(error) = &app.preview_error {
        ui.label(RichText::new(error).color(Color32::LIGHT_RED).size(11.0));
    } else {
        ui.label(
            RichText::new("Choose an image to see its preview and details.")
                .color(Color32::GRAY)
                .size(11.0),
        );
    }
}

fn queue_panel(app: &mut PixelForgeApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Conversion queue");
        ui.label(
            RichText::new(format!(
                "{} item{}",
                app.queue.len(),
                plural(app.queue.len())
            ))
            .size(12.0)
            .color(Color32::GRAY),
        );
    });
    ui.add_space(4.0);
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if app.queue.is_empty() {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 120.0),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(rect, 10.0, Color32::from_rgb(30, 34, 45));
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Drop images here\nor use Add images",
                    egui::FontId::proportional(14.0),
                    Color32::GRAY,
                );
            }
            for (index, path) in app.queue.iter().enumerate() {
                let selected = app.selected == Some(index);
                egui::Frame::new()
                    .fill(if selected {
                        Color32::from_rgb(48, 42, 67)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(7, 5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let state = app.states.get(index).unwrap_or(&JobState::Pending);
                            let (symbol, color) = match state {
                                JobState::Pending => ("○", Color32::GRAY),
                                JobState::Validating => ("…", Color32::YELLOW),
                                JobState::Processing => ("◌", Color32::LIGHT_BLUE),
                                JobState::Completed => ("✓", Color32::LIGHT_GREEN),
                                JobState::Skipped => ("–", Color32::GRAY),
                                JobState::Failed => ("!", Color32::LIGHT_RED),
                                JobState::Cancelled => ("×", Color32::GRAY),
                            };
                            ui.label(RichText::new(symbol).color(color));
                            let label = path.file_name().unwrap_or_default().to_string_lossy();
                            if ui
                                .selectable_label(selected, RichText::new(label).size(12.0))
                                .clicked()
                            {
                                clicked = Some(index);
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(
                                            path.extension()
                                                .unwrap_or_default()
                                                .to_string_lossy()
                                                .to_ascii_uppercase(),
                                        )
                                        .size(10.0)
                                        .color(Color32::GRAY),
                                    );
                                },
                            );
                        });
                    });
            }
        });
    if let Some(index) = clicked {
        app.selected = Some(index);
        app.refresh_preview();
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("Recent results").strong());
        if !app.last_reports.is_empty() {
            ui.label(
                RichText::new(format!("{} exported", app.last_reports.len()))
                    .size(11.0)
                    .color(Color32::GRAY),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Open output folder").clicked() {
                open_path(&app.output_dir());
            }
        });
    });
    egui::ScrollArea::vertical()
        .max_height(170.0)
        .show(ui, |ui| {
            if app.last_reports.is_empty() && !app.running {
                ui.label(
                    RichText::new("Completed files will appear here.")
                        .color(Color32::GRAY)
                        .size(11.0),
                );
            }
            for report in app.last_reports.iter().rev().take(30) {
                ui.horizontal(|ui| {
                    ui.label("✓");
                    ui.label(
                        RichText::new(
                            report
                                .output
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                        )
                        .size(11.0),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} · {:.1}% size change · {} ms",
                            format_bytes(report.output_bytes),
                            report.size_change_percent(),
                            report.elapsed_ms
                        ))
                        .size(10.0)
                        .color(Color32::GRAY),
                    );
                });
            }
        });
    if let Some(current) = &app.current_item {
        ui.label(
            RichText::new(format!(
                "Working on {}",
                current.file_name().unwrap_or_default().to_string_lossy()
            ))
            .size(11.0)
            .color(Color32::GRAY),
        );
    }
}

fn collect_images(folder: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() && recursive {
                stack.push(path);
            } else if file_type.is_file() && looks_like_image(&path) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn looks_like_image(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            OutputFormat::from_extension(ext).is_some() || ext.eq_ignore_ascii_case("dds")
        })
    {
        return true;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 64];
    let Ok(read) = file.read(&mut header) else {
        return false;
    };
    image::guess_format(&header[..read]).is_ok()
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn open_path(path: &Path) {
    #[cfg(target_os = "windows")]
    let result = Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(path).spawn();
    if let Err(error) = result {
        tracing::warn!(%error, "could not open output folder");
    }
}

fn set_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = Color32::from_rgb(19, 21, 28);
    visuals.panel_fill = Color32::from_rgb(17, 19, 25);
    visuals.extreme_bg_color = Color32::from_rgb(25, 28, 36);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 31, 40);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(39, 42, 53);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(62, 53, 86);
    visuals.widgets.active.bg_fill = Color32::from_rgb(116, 81, 174);
    visuals.selection.bg_fill = Color32::from_rgb(103, 73, 151);
    ctx.set_visuals(visuals);
}

pub fn app_icon() -> egui::IconData {
    let size = 64u32;
    let mut rgba = vec![0; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let index = ((y * size + x) * 4) as usize;
            let inside = (8..56).contains(&x) && (8..56).contains(&y);
            let ring = (x as i32 - 32).pow(2) + (y as i32 - 32).pow(2) < 17 * 17;
            let pixel = if !inside {
                [0, 0, 0, 0]
            } else if ring && (x / 6 + y / 6) % 2 == 0 {
                [247, 183, 83, 255]
            } else {
                [133, 91, 200, 255]
            };
            rgba[index..index + 4].copy_from_slice(&pixel);
        }
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}
