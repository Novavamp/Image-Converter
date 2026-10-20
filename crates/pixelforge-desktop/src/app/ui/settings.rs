use super::super::{FORMAT_CAPABILITIES, OutputFormat, PixelForgeApp, ThemeColors, yes_no};
use super::widgets::{section_heading, step_label};
use eframe::egui::{self, RichText};
#[allow(clippy::collapsible_if)] // Keep the folder picker visibly nested under its button click.
pub(super) fn settings_panel(app: &mut PixelForgeApp, ui: &mut egui::Ui, colors: &ThemeColors) {
    ui.label(
        RichText::new("SETUP")
            .size(10.0)
            .strong()
            .color(colors.accent),
    );
    ui.heading(
        RichText::new("Export settings")
            .size(21.0)
            .color(colors.text),
    );
    ui.label(
        RichText::new("Choose what your images become.")
            .size(12.0)
            .color(colors.muted),
    );
    ui.add_space(17.0);

    section_heading(
        ui,
        "1",
        "Choose formats",
        "Pick one or more outputs",
        colors,
    );
    let mut preset = app.settings.preset.clone();
    let mut preset_names = vec![
        "Web optimized".to_owned(),
        "Smallest practical".to_owned(),
        "Lossless conversion".to_owned(),
        "Transparent assets".to_owned(),
        "Email attachment".to_owned(),
        "Archive quality".to_owned(),
        "Custom".to_owned(),
    ];
    preset_names.extend(
        app.settings
            .saved_presets
            .iter()
            .map(|saved| saved.name.clone()),
    );
    ui.add_enabled_ui(!app.running, |ui| {
        egui::ComboBox::from_id_salt("preset_picker")
            .selected_text(&preset)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for name in &preset_names {
                    ui.selectable_value(&mut preset, name.clone(), name);
                }
            });
    });
    if preset != app.settings.preset {
        app.apply_preset(&preset);
    }
    ui.add_space(5.0);
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
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.preset_name_draft)
                .hint_text("Name this preset")
                .desired_width(145.0),
        );
        if ui
            .add_enabled(
                !app.running && !app.preset_name_draft.trim().is_empty(),
                egui::Button::new("Save preset"),
            )
            .clicked()
        {
            let name = app.preset_name_draft.clone();
            app.save_current_preset(&name);
        }
    });

    ui.add_space(13.0);
    let metadata_changed = egui::Frame::new()
        .fill(colors.surface_alt)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(egui::CornerRadius::same(11))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            let changed = ui
                .add_enabled(
                    !app.running,
                    egui::Checkbox::new(
                        &mut app.settings.strip_metadata,
                        "Remove AI tags & metadata",
                    ),
                )
                .changed();
            ui.label(
                RichText::new(
                    "Extra cleanup removes metadata and unknown chunks from PNG, JPEG, and WebP.",
                )
                .size(10.0)
                .color(colors.muted),
            );
            ui.label(
                RichText::new("Pixel-embedded marks may remain.")
                    .size(10.0)
                    .color(colors.muted),
            );
            changed
        })
        .inner;
    if metadata_changed {
        app.settings.preset = "Custom".into();
    }

    ui.add_space(18.0);
    section_heading(
        ui,
        "2",
        "Fine-tune",
        "Quality, size, and destination",
        colors,
    );

    step_label(ui, "OUTPUT FOLDER", colors);
    let folder = app.output_dir();
    ui.horizontal(|ui| {
        let path_label = ui.add(
            egui::Label::new(
                RichText::new(folder.to_string_lossy())
                    .size(12.0)
                    .color(colors.text),
            )
            .truncate(),
        );
        path_label.on_hover_text(folder.display().to_string());
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(folder.display().to_string());
        }
        if ui
            .add_enabled(!app.running, egui::Button::new("Change"))
            .clicked()
            && let Some(path) = rfd::FileDialog::new().set_directory(&folder).pick_folder()
        {
            app.settings.output_dir = Some(path);
        }
    });

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        step_label(ui, "QUALITY", colors);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::Frame::new()
                .fill(colors.selected)
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{}%", app.settings.quality))
                            .size(11.0)
                            .strong()
                            .color(colors.accent),
                    );
                });
        });
    });

    let quality_is_used = app.settings.formats.iter().any(|format| {
        matches!(format, OutputFormat::Jpeg | OutputFormat::Avif)
            || (*format == OutputFormat::Webp && !app.settings.webp_lossless)
    });
    if ui
        .add_enabled(
            !app.running && quality_is_used,
            egui::Slider::new(&mut app.settings.quality, 1..=100),
        )
        .changed()
    {
        app.settings.preset = "Custom".into();
    }
    ui.label(
        RichText::new("Used by JPEG, lossy WebP, and AVIF.")
            .size(10.0)
            .color(colors.muted),
    );

    if app.settings.formats.contains(&OutputFormat::Webp) {
        ui.add_space(5.0);
        if ui
            .add_enabled(
                !app.running,
                egui::Checkbox::new(&mut app.settings.webp_lossless, "WebP lossless"),
            )
            .changed()
        {
            app.settings.preset = "Custom".into();
        }
    }

    ui.add_space(7.0);
    let resize_changed = egui::Frame::new()
        .fill(colors.surface)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(egui::CornerRadius::same(11))
        .inner_margin(egui::Margin::same(11))
        .show(ui, |ui| {
            let mut changed = ui
                .add_enabled(
                    !app.running,
                    egui::Checkbox::new(&mut app.settings.resize_enabled, "Resize to fit"),
                )
                .changed();
            ui.horizontal(|ui| {
                changed |= ui
                    .add_enabled(
                        app.settings.resize_enabled && !app.running,
                        egui::DragValue::new(&mut app.settings.resize_width)
                            .range(1..=20_000)
                            .prefix("W ")
                            .suffix(" px"),
                    )
                    .changed();
                changed |= ui
                    .add_enabled(
                        app.settings.resize_enabled && !app.running,
                        egui::DragValue::new(&mut app.settings.resize_height)
                            .range(1..=20_000)
                            .prefix("H ")
                            .suffix(" px"),
                    )
                    .changed();
            });
            ui.label(
                RichText::new("Keeps proportions and never enlarges.")
                    .size(10.0)
                    .color(colors.muted),
            );
            changed
        })
        .inner;
    if resize_changed {
        app.settings.preset = "Custom".into();
    }

    ui.add_space(12.0);
    step_label(ui, "FILE NAMING", colors);
    ui.add_enabled(
        !app.running,
        egui::TextEdit::singleline(&mut app.settings.output_suffix)
            .hint_text("Optional suffix, e.g. _web")
            .desired_width(f32::INFINITY),
    );
    ui.add_enabled(
        !app.running,
        egui::Checkbox::new(&mut app.settings.preserve_folders, "Keep source subfolders"),
    );
    ui.add_enabled(
        !app.running,
        egui::Checkbox::new(
            &mut app.settings.skip_existing,
            "Skip files that already exist",
        ),
    );
    ui.label(
        RichText::new(if app.settings.skip_existing {
            "Existing files are left untouched."
        } else {
            "Name conflicts get a numbered copy, such as (1)."
        })
        .size(11.0)
        .color(colors.muted),
    );

    if app.settings.formats.contains(&OutputFormat::Jpeg) {
        ui.add_space(9.0);
        step_label(ui, "JPEG BACKGROUND", colors);
        if ui
            .color_edit_button_srgb(&mut app.settings.jpeg_background)
            .changed()
        {
            app.settings.preset = "Custom".into();
        }
    }

    ui.add_space(12.0);
    egui::CollapsingHeader::new("Format capabilities")
        .default_open(false)
        .show(ui, |ui| {
            ui.add_space(5.0);
            for capability in FORMAT_CAPABILITIES {
                egui::Frame::new()
                    .fill(colors.surface)
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(9, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(capability.name)
                                    .strong()
                                    .size(11.0)
                                    .color(colors.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(if capability.writes {
                                            "READ + WRITE"
                                        } else {
                                            "READ ONLY"
                                        })
                                        .size(9.0)
                                        .strong()
                                        .color(colors.muted),
                                    );
                                },
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
                            .size(10.0)
                            .color(colors.muted),
                        );
                        ui.label(
                            RichText::new(capability.notes)
                                .size(10.0)
                                .color(colors.muted),
                        );
                    });
                ui.add_space(4.0);
            }
        });

    ui.add_space(14.0);
    step_label(ui, "IMAGE PREVIEW", colors);
    ui.label(
        RichText::new(
            "Select an image, then open Before & after to compare the source and export.",
        )
        .size(11.0)
        .color(colors.muted),
    );
    if ui.button("Compare source and output  →").clicked() {
        app.workspace_tab = super::super::WorkspaceTab::Compare;
    }
}
