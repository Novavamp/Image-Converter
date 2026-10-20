use directories::ProjectDirs;
use eframe::egui::{self, Color32, TextureHandle};
use pixelforge_core::{
    ConversionOptions, ConversionReport, FORMAT_CAPABILITIES, ImageInfo, OutputFormat,
    ResizeBounds, estimate_output_size, inspect, read_image,
};
use pixelforge_jobs::{BatchRequest, InputOverrides, JobState, Worker, WorkerEvent};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
};

mod model;
mod operations;
mod preview;
mod ui;

use model::{
    Appearance, ImageOverrides, PreviewData, PreviewPayload, SavedPreset, Settings, SettingsFile,
    ThemeColors, WorkspaceTab,
};

pub struct PixelForgeApp {
    settings: Settings,
    settings_path: Option<PathBuf>,
    last_saved: Vec<u8>,
    queue: Vec<PathBuf>,
    states: Vec<JobState>,
    queue_roots: HashMap<PathBuf, PathBuf>,
    image_overrides: HashMap<PathBuf, ImageOverrides>,
    item_errors: HashMap<PathBuf, Vec<(OutputFormat, String)>>,
    item_successes: HashMap<PathBuf, usize>,
    item_skips: HashMap<PathBuf, usize>,
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
    output_preview_path: Option<PathBuf>,
    output_preview: Option<(ImageInfo, TextureHandle)>,
    output_preview_error: Option<String>,
    estimated_sizes: Option<Vec<(OutputFormat, u64, u32, u32)>>,
    estimate_key: Option<String>,
    preview_error: Option<String>,
    settings_visible: bool,
    workspace_tab: WorkspaceTab,
    preset_name_draft: String,
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
        set_theme(&cc.egui_ctx, settings.appearance);
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
            queue_roots: HashMap::new(),
            image_overrides: HashMap::new(),
            item_errors: HashMap::new(),
            item_successes: HashMap::new(),
            item_skips: HashMap::new(),
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
            output_preview_path: None,
            output_preview: None,
            output_preview_error: None,
            estimated_sizes: None,
            estimate_key: None,
            preview_error: None,
            settings_visible: true,
            workspace_tab: WorkspaceTab::Queue,
            preset_name_draft: String::new(),
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

fn set_theme(ctx: &egui::Context, appearance: Appearance) {
    let colors = ThemeColors::for_appearance(appearance);
    let mut visuals = match appearance {
        Appearance::Dark => egui::Visuals::dark(),
        Appearance::Light => egui::Visuals::light(),
    };
    visuals.window_fill = colors.panel;
    visuals.panel_fill = colors.canvas;
    visuals.extreme_bg_color = colors.surface;
    visuals.faint_bg_color = colors.surface_alt;
    visuals.widgets.noninteractive.bg_fill = colors.panel;
    visuals.widgets.noninteractive.fg_stroke.color = colors.text;
    visuals.widgets.inactive.bg_fill = colors.surface_alt;
    visuals.widgets.inactive.fg_stroke.color = colors.text;
    visuals.widgets.hovered.bg_fill = colors.selected;
    visuals.widgets.hovered.fg_stroke.color = colors.text;
    visuals.widgets.active.bg_fill = colors.accent;
    visuals.widgets.active.fg_stroke.color = Color32::WHITE;
    visuals.selection.bg_fill = colors.accent;
    visuals.selection.stroke.color = Color32::WHITE;
    visuals.hyperlink_color = colors.accent_hover;
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
