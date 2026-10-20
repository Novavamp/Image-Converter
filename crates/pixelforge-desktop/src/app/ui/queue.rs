use super::super::{
    JobState, OutputFormat, PixelForgeApp, ResizeBounds, ThemeColors, WorkspaceTab, format_bytes,
    open_path, plural, yes_no,
};
use super::results::{compare_view, exports_view};
use super::widgets::{job_state_appearance, pick_folder, pick_images, primary_button, status_pill};
use eframe::egui::{self, Color32, RichText};
use std::{fs, path::PathBuf};

pub(super) fn workflow_steps(app: &PixelForgeApp, ui: &mut egui::Ui, colors: &ThemeColors) {
    let has_images = !app.queue.is_empty();
    let formats_selected = !app.settings.formats.is_empty();
    let ready = has_images && formats_selected;
    ui.columns(3, |columns| {
        step_card(
            &mut columns[0],
            "01",
            "Add images",
            if has_images {
                format!("{} in queue", app.queue.len())
            } else {
                "Drop files or browse".into()
            },
            has_images,
            !has_images,
            colors,
        );
        step_card(
            &mut columns[1],
            "02",
            "Choose output",
            if formats_selected {
                format!(
                    "{} format{} selected",
                    app.settings.formats.len(),
                    plural(app.settings.formats.len())
                )
            } else {
                "Select at least one format".into()
            },
            formats_selected,
            has_images && !formats_selected,
            colors,
        );
        step_card(
            &mut columns[2],
            "03",
            if app.running { "Converting" } else { "Convert" },
            if app.running {
                app.current_item
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| format!("Working on {}", name.to_string_lossy()))
                    .unwrap_or_else(|| "Your files stay on this device".into())
            } else if ready {
                "Your files stay on this device".into()
            } else {
                "Ready after steps 1 and 2".into()
            },
            !app.running && ready && app.states.iter().all(JobState::terminal),
            ready && !app.running,
            colors,
        );
    });
}

fn step_card(
    ui: &mut egui::Ui,
    number: &str,
    title: &str,
    detail: String,
    complete: bool,
    active: bool,
    colors: &ThemeColors,
) {
    let fill = if active {
        colors.selected
    } else {
        colors.surface
    };
    let stroke_color = if active { colors.accent } else { colors.border };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(
            if active { 1.4 } else { 1.0 },
            stroke_color,
        ))
        .corner_radius(egui::CornerRadius::same(11))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                egui::Frame::new()
                    .fill(if complete {
                        colors.success
                    } else if active {
                        colors.accent
                    } else {
                        colors.surface_alt
                    })
                    .corner_radius(egui::CornerRadius::same(7))
                    .inner_margin(egui::Margin::symmetric(7, 5))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(if complete { "✓" } else { number })
                                .size(11.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                    });
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).size(13.0).strong().color(colors.text));
                    ui.label(RichText::new(detail).size(11.0).color(colors.muted));
                });
            });
        });
}

pub(super) fn queue_panel(app: &mut PixelForgeApp, ui: &mut egui::Ui, colors: &ThemeColors) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading(
                RichText::new("Your workspace")
                    .size(20.0)
                    .color(colors.text),
            );
            ui.label(
                RichText::new("Select an image to inspect it or customize its output.")
                    .size(12.0)
                    .color(colors.muted),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            status_pill(
                ui,
                &format!(
                    "{} IMAGE{}",
                    app.queue.len(),
                    if app.queue.len() == 1 { "" } else { "S" }
                ),
                colors.muted,
                colors,
            );
        });
    });
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(
            &mut app.workspace_tab,
            WorkspaceTab::Queue,
            format!("Queue  ·  {}", app.queue.len()),
        );
        ui.selectable_value(
            &mut app.workspace_tab,
            WorkspaceTab::Compare,
            "Before & after",
        );
        ui.selectable_value(
            &mut app.workspace_tab,
            WorkspaceTab::Exports,
            format!("Recent exports  ·  {}", app.last_reports.len()),
        );
    });
    ui.separator();
    ui.add_space(8.0);

    match app.workspace_tab {
        WorkspaceTab::Queue => queue_view(app, ui, colors),
        WorkspaceTab::Compare => compare_view(app, ui, colors),
        WorkspaceTab::Exports => exports_view(app, ui, colors),
    }
}

fn queue_view(app: &mut PixelForgeApp, ui: &mut egui::Ui, colors: &ThemeColors) {
    if app.queue.is_empty() {
        let hover = ui.ctx().input(|input| !input.raw.hovered_files.is_empty());
        egui::Frame::new()
            .fill(if hover {
                colors.selected
            } else {
                colors.surface
            })
            .stroke(egui::Stroke::new(
                1.0,
                if hover { colors.accent } else { colors.border },
            ))
            .corner_radius(egui::CornerRadius::same(16))
            .inner_margin(egui::Margin::same(22))
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 230.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_space(16.0);
                        egui::Frame::new()
                            .fill(colors.selected)
                            .corner_radius(egui::CornerRadius::same(14))
                            .inner_margin(egui::Margin::same(11))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("↑").size(26.0).strong().color(colors.accent),
                                );
                            });
                        ui.add_space(9.0);
                        ui.label(
                            RichText::new("Start with your images")
                                .size(20.0)
                                .strong()
                                .color(colors.text),
                        );
                        ui.label(
                            RichText::new("Drop files here, or browse your computer.")
                                .size(13.0)
                                .color(colors.muted),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.add(primary_button("Browse images", colors)).clicked()
                                && let Some(paths) = pick_images()
                            {
                                app.add_paths(paths);
                            }
                            if ui.button("Choose a folder").clicked()
                                && let Some(path) = pick_folder()
                            {
                                app.add_paths([path]);
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("PNG · JPEG · WebP · AVIF · GIF · TIFF · and more")
                                .size(11.0)
                                .color(colors.muted),
                        );
                    },
                );
            });
        ui.add_space(12.0);
        egui::Frame::new()
            .fill(colors.surface_alt)
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(13, 10))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Your files stay on this device. PixelForge works offline.")
                        .size(12.0)
                        .color(colors.text),
                );
            });
        return;
    }

    let queue = app.queue.clone();
    let states = app.states.clone();
    let selected = app.selected;
    let mut action: Option<(usize, QueueAction)> = None;
    for (index, path) in queue.iter().enumerate() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_uppercase();
        let bytes = fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let (state_label, state_color) = states
            .get(index)
            .map(|state| job_state_appearance(state, colors))
            .unwrap_or(("QUEUED", colors.muted));
        let formats = app.formats_for(path);
        let formats_label = formats
            .iter()
            .map(|format| format.label())
            .collect::<Vec<_>>()
            .join(" · ");
        let is_selected = selected == Some(index);
        let row = ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 68.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::Frame::new()
                    .fill(if is_selected {
                        colors.selected
                    } else {
                        colors.surface
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if is_selected {
                            colors.accent
                        } else {
                            colors.border
                        },
                    ))
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            egui::Frame::new()
                                .fill(colors.surface_alt)
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(8, 7))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(&extension)
                                            .size(10.0)
                                            .strong()
                                            .color(colors.accent),
                                    );
                                });
                            ui.add_space(5.0);
                            ui.vertical(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(name.to_string())
                                            .size(13.0)
                                            .strong()
                                            .color(colors.text),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(path.display().to_string());
                                ui.label(
                                    RichText::new(format!(
                                        "{extension}  ·  {}",
                                        format_bytes(bytes)
                                    ))
                                    .size(11.0)
                                    .color(colors.muted),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    status_pill(ui, state_label, state_color, colors);
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(&formats_label)
                                            .size(11.0)
                                            .color(colors.muted),
                                    );
                                },
                            );
                        });
                    });
            },
        );
        let response = ui
            .interact(
                row.response.rect,
                ui.scope_id().with(("queue-row", index)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Click to inspect · right-click for more actions");
        if response.clicked() {
            app.selected = Some(index);
            app.refresh_preview();
        }
        response.context_menu(|ui| {
            if ui.button("Show source folder").clicked() {
                action = Some((index, QueueAction::OpenSource(path.clone())));
                ui.close();
            }
            if !app.running && ui.button("Remove from queue").clicked() {
                action = Some((index, QueueAction::Remove));
                ui.close();
            }
            if !app.running
                && app.item_errors.contains_key(path)
                && ui.button("Retry this image").clicked()
            {
                action = Some((index, QueueAction::Retry));
                ui.close();
            }
        });

        if let Some(errors) = app.item_errors.get(path) {
            egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(
                    colors.danger.r(),
                    colors.danger.g(),
                    colors.danger.b(),
                    28,
                ))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(11, 7))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} issue{}",
                                errors.len(),
                                plural(errors.len())
                            ))
                            .strong()
                            .color(colors.danger),
                        );
                        let (_, error) = &errors[0];
                        ui.add(
                            egui::Label::new(RichText::new(error).size(11.0).color(colors.text))
                                .truncate(),
                        )
                        .on_hover_text(error);
                        if !app.running && ui.button("Retry").clicked() {
                            action = Some((index, QueueAction::Retry));
                        }
                    });
                });
        }
        ui.add_space(6.0);
    }

    if let Some((index, action)) = action {
        match action {
            QueueAction::Remove => {
                app.selected = Some(index);
                app.remove_selected();
            }
            QueueAction::Retry => {
                if let Some(path) = app.queue.get(index).cloned() {
                    app.retry_item(&path);
                }
            }
            QueueAction::OpenSource(path) => {
                if let Some(parent) = path.parent() {
                    open_path(parent);
                }
            }
        }
    }

    if let Some(index) = app.selected
        && let Some(path) = app.queue.get(index).cloned()
    {
        ui.add_space(8.0);
        selected_image_options(app, ui, colors, &path);
    }
}

#[derive(Clone)]
enum QueueAction {
    Remove,
    Retry,
    OpenSource(PathBuf),
}

fn selected_image_options(
    app: &mut PixelForgeApp,
    ui: &mut egui::Ui,
    colors: &ThemeColors,
    path: &std::path::Path,
) {
    let defaults = app.settings.formats.clone();
    let mut retry = false;
    egui::CollapsingHeader::new("Selected image settings & details")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                RichText::new(path.display().to_string())
                    .size(11.0)
                    .color(colors.muted),
            );
            let mut formats = app.formats_for(path);
            ui.horizontal_wrapped(|ui| {
                for format in OutputFormat::PRIMARY {
                    let mut checked = formats.contains(&format);
                    if ui
                        .add_enabled(
                            !app.running,
                            egui::Checkbox::new(&mut checked, format.label()),
                        )
                        .changed()
                    {
                        if checked {
                            formats.push(format);
                        } else {
                            formats.retain(|current| *current != format);
                        }
                    }
                }
            });
            if formats != app.formats_for(path) {
                app.image_overrides
                    .entry(path.to_path_buf())
                    .or_default()
                    .formats = Some(formats.clone());
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!app.running, egui::Button::new("Use batch format defaults"))
                    .clicked()
                {
                    if let Some(override_) = app.image_overrides.get_mut(path) {
                        override_.formats = None;
                    }
                }
                if let Some((info, _)) = &app.preview {
                    if info.has_alpha && formats.contains(&OutputFormat::Jpeg) {
                        ui.label(
                            RichText::new("JPEG removes transparency")
                                .size(11.0)
                                .color(colors.warning),
                        );
                    }
                }
            });

            let current_resize = app
                .image_overrides
                .get(path)
                .and_then(|overrides| overrides.resize);
            let mut resize_mode = match current_resize {
                None => 0,
                Some(None) => 1,
                Some(Some(_)) => 2,
            };
            ui.add_enabled_ui(!app.running, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Resize").strong().color(colors.text));
                    ui.selectable_value(&mut resize_mode, 0, "Use batch setting");
                    ui.selectable_value(&mut resize_mode, 1, "Original size");
                    ui.selectable_value(&mut resize_mode, 2, "Custom fit");
                });
            });
            if !app.running {
                let overrides = app.image_overrides.entry(path.to_path_buf()).or_default();
                if resize_mode == 0 {
                    overrides.resize = None;
                } else if resize_mode == 1 {
                    overrides.resize = Some(None);
                } else {
                    let mut bounds = overrides.resize.flatten().unwrap_or(ResizeBounds {
                        width: app.settings.resize_width,
                        height: app.settings.resize_height,
                    });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut bounds.width)
                                .range(1..=20_000)
                                .prefix("W ")
                                .suffix(" px"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut bounds.height)
                                .range(1..=20_000)
                                .prefix("H ")
                                .suffix(" px"),
                        );
                    });
                    overrides.resize = Some(Some(bounds));
                }
            }
            ui.label(
                RichText::new("Fit keeps proportions and never enlarges.")
                    .size(11.0)
                    .color(colors.muted),
            );

            if let Some(errors) = app.item_errors.get(path) {
                ui.add_space(6.0);
                for (format, error) in errors {
                    egui::Frame::new()
                        .fill(colors.surface_alt)
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 7))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{} error", format.label()))
                                    .strong()
                                    .color(colors.danger),
                            );
                            ui.label(RichText::new(error).size(11.0).color(colors.text));
                        });
                }
                if ui
                    .add_enabled(!app.running, primary_button("Retry failed outputs", colors))
                    .clicked()
                {
                    retry = true;
                }
            }
            if let Some((info, _)) = &app.preview {
                ui.label(
                    RichText::new(format!(
                        "{} × {} · {} · alpha {}",
                        info.width,
                        info.height,
                        format_bytes(info.bytes),
                        yes_no(info.has_alpha),
                    ))
                    .size(11.0)
                    .color(colors.muted),
                );
            }
            let _ = &defaults;
        });
    if retry {
        app.retry_item(path);
    }
}
