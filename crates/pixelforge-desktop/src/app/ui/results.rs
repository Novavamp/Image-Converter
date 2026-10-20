use super::super::{PixelForgeApp, ThemeColors, format_bytes, open_path};
use eframe::egui::{self, RichText};
pub(super) fn compare_view(app: &PixelForgeApp, ui: &mut egui::Ui, colors: &ThemeColors) {
    let Some(path) = app.selected.and_then(|index| app.queue.get(index)) else {
        empty_message(ui, colors, "Choose an image in the queue to compare it.");
        return;
    };
    ui.columns(2, |columns| {
        preview_card(
            &mut columns[0],
            "Original",
            app.preview.as_ref(),
            app.preview_error.as_deref(),
            colors,
        );
        preview_card(
            &mut columns[1],
            "Converted",
            app.output_preview.as_ref(),
            app.output_preview_error.as_deref(),
            colors,
        );
    });
    ui.add_space(12.0);
    if let Some(estimates) = &app.estimated_sizes {
        egui::Frame::new()
            .fill(colors.surface_alt)
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(12, 9))
            .show(ui, |ui| {
                ui.label(RichText::new("Approximate output estimates").strong().color(colors.text));
                ui.label(RichText::new("Estimated from a reduced sample; actual file sizes vary with image content.").size(11.0).color(colors.muted));
                for (format, bytes, width, height) in estimates {
                    ui.label(RichText::new(format!("{} · ~{} · {} × {}", format.label(), format_bytes(*bytes), width, height))
                        .size(12.0).color(colors.text));
                }
                if estimates.is_empty() {
                    ui.label(RichText::new("Select a writable output format to estimate its size.").size(11.0).color(colors.muted));
                }
            });
    }
    let reports: Vec<_> = app
        .last_reports
        .iter()
        .filter(|report| report.input == *path)
        .collect();
    if reports.is_empty() {
        ui.add_space(8.0);
        empty_message(
            ui,
            colors,
            "Convert this image to see the before-and-after result here.",
        );
    } else {
        for report in reports {
            let saved = report.input_bytes as i128 - report.output_bytes as i128;
            let saved_label = if saved >= 0 {
                format!(
                    "{} saved ({:.1}%)",
                    format_bytes(saved as u64),
                    report.size_change_percent()
                )
            } else {
                format!(
                    "{} larger",
                    format_bytes(saved.unsigned_abs().min(u64::MAX as u128) as u64)
                )
            };
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · {} × {} · {} · {}",
                        report
                            .output
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        report.width,
                        report.height,
                        format_bytes(report.output_bytes),
                        saved_label,
                    ))
                    .size(12.0)
                    .color(colors.text),
                );
                if ui.button("Open file").clicked() {
                    open_path(&report.output);
                }
            });
        }
    }
}

fn preview_card(
    ui: &mut egui::Ui,
    title: &str,
    preview: Option<&(pixelforge_core::ImageInfo, eframe::egui::TextureHandle)>,
    error: Option<&str>,
    colors: &ThemeColors,
) {
    egui::Frame::new()
        .fill(colors.surface)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(RichText::new(title).size(14.0).strong().color(colors.text));
            ui.add_space(7.0);
            if let Some((info, texture)) = preview {
                ui.add(egui::Image::new(texture).max_size(egui::vec2(ui.available_width(), 300.0)));
                ui.label(
                    RichText::new(format!(
                        "{} × {} · {}",
                        info.width,
                        info.height,
                        format_bytes(info.bytes)
                    ))
                    .size(12.0)
                    .color(colors.muted),
                );
            } else if let Some(error) = error {
                ui.label(RichText::new(error).size(12.0).color(colors.danger));
            } else {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 230.0),
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.label(
                            RichText::new("Preview is loading or no conversion is available yet.")
                                .size(12.0)
                                .color(colors.muted),
                        );
                    },
                );
            }
        });
}

pub(super) fn exports_view(app: &PixelForgeApp, ui: &mut egui::Ui, colors: &ThemeColors) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("Latest conversion results").size(16.0).strong().color(colors.text));
            ui.label(RichText::new("Open a file or its output folder. These results stay available for this session.")
                .size(12.0).color(colors.muted));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !app.last_reports.is_empty() && ui.button("Open output folder").clicked() {
                open_path(&app.output_dir());
            }
        });
    });
    ui.add_space(10.0);
    if app.last_reports.is_empty() {
        empty_message(
            ui,
            colors,
            "Converted files will appear here with their actual size and savings.",
        );
        return;
    }
    for report in app.last_reports.iter().rev() {
        let delta = report.input_bytes as i128 - report.output_bytes as i128;
        let size_summary = if delta >= 0 {
            format!(
                "{} saved · {:.1}%",
                format_bytes(delta as u64),
                report.size_change_percent(),
            )
        } else {
            format!(
                "{} larger",
                format_bytes(delta.unsigned_abs().min(u64::MAX as u128) as u64),
            )
        };
        egui::Frame::new()
            .fill(colors.surface)
            .stroke(egui::Stroke::new(1.0, colors.border))
            .corner_radius(egui::CornerRadius::same(9))
            .inner_margin(egui::Margin::symmetric(12, 9))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("✓").size(15.0).strong().color(colors.success));
                    ui.vertical(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    report
                                        .output
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy(),
                                )
                                .size(13.0)
                                .strong()
                                .color(colors.text),
                            )
                            .truncate(),
                        )
                        .on_hover_text(report.output.display().to_string());
                        ui.label(
                            RichText::new(format!(
                                "{} → {} · {} × {} · {} · {} ms",
                                format_bytes(report.input_bytes),
                                format_bytes(report.output_bytes),
                                report.width,
                                report.height,
                                size_summary,
                                report.elapsed_ms,
                            ))
                            .size(11.0)
                            .color(colors.muted),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Open file").clicked() {
                            open_path(&report.output);
                        }
                        if let Some(parent) = report.output.parent()
                            && ui.button("Folder").clicked()
                        {
                            open_path(parent);
                        }
                    });
                });
            });
        ui.add_space(5.0);
    }
}

fn empty_message(ui: &mut egui::Ui, colors: &ThemeColors, message: &str) {
    egui::Frame::new()
        .fill(colors.surface)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(egui::CornerRadius::same(11))
        .inner_margin(egui::Margin::symmetric(15, 13))
        .show(ui, |ui| {
            ui.label(RichText::new(message).size(12.0).color(colors.muted));
        });
}
