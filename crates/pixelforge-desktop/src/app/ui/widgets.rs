use super::super::{JobState, ThemeColors};
use eframe::egui::{self, Color32, RichText};
use std::path::PathBuf;
pub(super) fn primary_button<'a>(label: &'a str, colors: &ThemeColors) -> egui::Button<'a> {
    egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
        .fill(colors.accent)
        .stroke(egui::Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(8))
}

pub(super) fn status_pill(ui: &mut egui::Ui, label: &str, color: Color32, _colors: &ThemeColors) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            32,
        ))
        .corner_radius(egui::CornerRadius::same(20))
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(9.0).strong().color(color));
        });
}

pub(super) fn section_heading(
    ui: &mut egui::Ui,
    number: &str,
    title: &str,
    detail: &str,
    colors: &ThemeColors,
) {
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(colors.selected)
            .corner_radius(egui::CornerRadius::same(7))
            .inner_margin(egui::Margin::symmetric(7, 5))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(number)
                        .size(10.0)
                        .strong()
                        .color(colors.accent),
                );
            });
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(13.0).strong().color(colors.text));
            ui.label(RichText::new(detail).size(10.0).color(colors.muted));
        });
    });
    ui.add_space(7.0);
}

pub(super) fn step_label(ui: &mut egui::Ui, label: &str, colors: &ThemeColors) {
    ui.label(RichText::new(label).size(9.0).strong().color(colors.muted));
}

pub(super) fn job_state_appearance(
    state: &JobState,
    colors: &ThemeColors,
) -> (&'static str, Color32) {
    match state {
        JobState::Pending => ("QUEUED", colors.muted),
        JobState::Validating => ("CHECKING", colors.warning),
        JobState::Processing => ("CONVERTING", colors.accent),
        JobState::Completed => ("DONE", colors.success),
        JobState::Skipped => ("SKIPPED", colors.muted),
        JobState::Failed => ("FAILED", colors.danger),
        JobState::Cancelled => ("CANCELLED", colors.warning),
    }
}

pub(super) fn pick_images() -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .add_filter(
            "Image files",
            &[
                "png", "jpg", "jpeg", "webp", "avif", "gif", "tif", "tiff", "bmp", "ico", "qoi",
                "tga", "ppm", "pgm", "pbm", "hdr", "exr", "ff", "dds",
            ],
        )
        .pick_files()
}

pub(super) fn pick_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_folder()
}
