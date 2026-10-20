use super::{ImageInfo, OutputFormat, ResizeBounds};
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Appearance {
    Light,
    Dark,
}

impl Default for Appearance {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Clone, Copy)]
pub(super) struct ThemeColors {
    pub(super) canvas: Color32,
    pub(super) panel: Color32,
    pub(super) surface: Color32,
    pub(super) surface_alt: Color32,
    pub(super) border: Color32,
    pub(super) text: Color32,
    pub(super) muted: Color32,
    pub(super) accent: Color32,
    pub(super) accent_hover: Color32,
    pub(super) selected: Color32,
    pub(super) success: Color32,
    pub(super) warning: Color32,
    pub(super) danger: Color32,
}

impl ThemeColors {
    pub(super) fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Dark => Self {
                canvas: Color32::from_rgb(17, 19, 25),
                panel: Color32::from_rgb(21, 24, 31),
                surface: Color32::from_rgb(28, 32, 41),
                surface_alt: Color32::from_rgb(36, 41, 52),
                border: Color32::from_rgb(48, 54, 68),
                text: Color32::from_rgb(238, 240, 247),
                muted: Color32::from_rgb(157, 164, 179),
                accent: Color32::from_rgb(145, 105, 230),
                accent_hover: Color32::from_rgb(164, 128, 244),
                selected: Color32::from_rgb(52, 43, 73),
                success: Color32::from_rgb(89, 205, 163),
                warning: Color32::from_rgb(245, 184, 90),
                danger: Color32::from_rgb(238, 112, 130),
            },
            Appearance::Light => Self {
                canvas: Color32::from_rgb(246, 247, 251),
                panel: Color32::from_rgb(255, 255, 255),
                surface: Color32::from_rgb(255, 255, 255),
                surface_alt: Color32::from_rgb(241, 243, 248),
                border: Color32::from_rgb(224, 227, 235),
                text: Color32::from_rgb(31, 37, 51),
                muted: Color32::from_rgb(104, 113, 132),
                accent: Color32::from_rgb(111, 73, 194),
                accent_hover: Color32::from_rgb(91, 55, 169),
                selected: Color32::from_rgb(241, 235, 252),
                success: Color32::from_rgb(26, 137, 106),
                warning: Color32::from_rgb(171, 100, 13),
                danger: Color32::from_rgb(190, 64, 84),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    pub(super) output_dir: Option<PathBuf>,
    pub(super) preset: String,
    pub(super) formats: Vec<OutputFormat>,
    pub(super) quality: u8,
    pub(super) resize_enabled: bool,
    pub(super) resize_width: u32,
    pub(super) resize_height: u32,
    pub(super) webp_lossless: bool,
    pub(super) strip_metadata: bool,
    pub(super) recursive: bool,
    pub(super) jpeg_background: [u8; 3],
    pub(super) appearance: Appearance,
    pub(super) output_suffix: String,
    pub(super) skip_existing: bool,
    pub(super) preserve_folders: bool,
    pub(super) saved_presets: Vec<SavedPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct SavedPreset {
    pub(super) name: String,
    pub(super) formats: Vec<OutputFormat>,
    pub(super) quality: u8,
    pub(super) resize_enabled: bool,
    pub(super) resize_width: u32,
    pub(super) resize_height: u32,
    pub(super) webp_lossless: bool,
    pub(super) strip_metadata: bool,
    pub(super) jpeg_background: [u8; 3],
}

impl Default for SavedPreset {
    fn default() -> Self {
        Self {
            name: String::new(),
            formats: vec![OutputFormat::Webp],
            quality: 85,
            resize_enabled: false,
            resize_width: 1920,
            resize_height: 1080,
            webp_lossless: false,
            strip_metadata: true,
            jpeg_background: [255; 3],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct ImageOverrides {
    pub(super) formats: Option<Vec<OutputFormat>>,
    pub(super) resize: Option<Option<ResizeBounds>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum WorkspaceTab {
    #[default]
    Queue,
    Compare,
    Exports,
}

#[derive(Serialize, Deserialize)]
pub(super) struct SettingsFile {
    pub(super) schema_version: u32,
    pub(super) settings: Settings,
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
            appearance: Appearance::Dark,
            output_suffix: String::new(),
            skip_existing: false,
            preserve_folders: true,
            saved_presets: Vec::new(),
        }
    }
}

pub(super) struct PreviewData {
    pub(super) path: PathBuf,
    pub(super) is_output: bool,
    pub(super) estimate_key: Option<String>,
    pub(super) result: Result<PreviewPayload, String>,
}

pub(super) struct PreviewPayload {
    pub(super) info: ImageInfo,
    pub(super) size: [usize; 2],
    pub(super) rgba: Vec<u8>,
    pub(super) estimates: Vec<(OutputFormat, u64, u32, u32)>,
}
