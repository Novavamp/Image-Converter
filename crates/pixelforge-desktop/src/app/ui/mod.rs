use super::{Appearance, JobState, PixelForgeApp, ThemeColors, format_bytes, set_theme};
use eframe::egui::{self, Color32, RichText};
use std::{path::PathBuf, time::Duration};

mod queue;
mod results;
mod settings;
mod widgets;

use queue::{queue_panel, workflow_steps};
use settings::settings_panel;
use widgets::*;
impl eframe::App for PixelForgeApp {
    #[allow(clippy::collapsible_if)]
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_worker();
        self.poll_previews(&ctx);
        self.refresh_preview();
        let mut colors = ThemeColors::for_appearance(self.settings.appearance);

        let can_use_shortcuts = !ctx.egui_wants_keyboard_input();
        let shortcuts = ctx.input(|input| {
            let command = input.modifiers.command;
            (
                can_use_shortcuts
                    && command
                    && input.key_pressed(egui::Key::O)
                    && !input.modifiers.shift,
                can_use_shortcuts
                    && command
                    && input.key_pressed(egui::Key::O)
                    && input.modifiers.shift,
                can_use_shortcuts && input.key_pressed(egui::Key::Delete),
                can_use_shortcuts && command && input.key_pressed(egui::Key::Enter),
                can_use_shortcuts
                    && command
                    && input.modifiers.shift
                    && input.key_pressed(egui::Key::ArrowUp),
                can_use_shortcuts
                    && command
                    && input.modifiers.shift
                    && input.key_pressed(egui::Key::ArrowDown),
            )
        });
        if shortcuts.0
            && !self.running
            && let Some(paths) = pick_images()
        {
            self.add_paths(paths);
        }
        if shortcuts.1
            && !self.running
            && let Some(path) = pick_folder()
        {
            self.add_paths([path]);
        }
        if shortcuts.2 {
            self.remove_selected();
        }
        if shortcuts.3 && !self.running {
            if self.item_errors.is_empty() {
                self.start_conversion();
            } else {
                self.retry_failed();
            }
        }
        if shortcuts.4 {
            self.move_selected(-1);
        }
        if shortcuts.5 {
            self.move_selected(1);
        }

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

        egui::Panel::top("brand_bar")
            .frame(
                egui::Frame::new()
                    .fill(colors.panel)
                    .stroke(egui::Stroke::new(1.0, colors.border))
                    .inner_margin(egui::Margin::symmetric(22, 11)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::new()
                        .fill(colors.accent)
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(8))
                        .show(ui, |ui| {
                            ui.label(RichText::new("P").size(19.0).strong().color(Color32::WHITE));
                        });
                    ui.add_space(3.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("PixelForge")
                                .size(20.0)
                                .strong()
                                .color(colors.text),
                        );
                        ui.label(
                            RichText::new("Image workspace")
                                .size(12.0)
                                .color(colors.muted),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let next_appearance = match self.settings.appearance {
                            Appearance::Dark => Appearance::Light,
                            Appearance::Light => Appearance::Dark,
                        };
                        let appearance_label = match self.settings.appearance {
                            Appearance::Dark => "☼  Light mode",
                            Appearance::Light => "☾  Dark mode",
                        };
                        if ui.button(appearance_label).clicked() {
                            self.settings.appearance = next_appearance;
                            set_theme(&ctx, next_appearance);
                        }
                        ui.add_space(12.0);
                        status_pill(
                            ui,
                            if self.running { "PROCESSING" } else { "READY" },
                            if self.running {
                                colors.warning
                            } else {
                                colors.success
                            },
                            &colors,
                        );
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("LOCAL · OFFLINE")
                                .size(11.0)
                                .strong()
                                .color(colors.muted),
                        );
                    });
                });
            });

        colors = ThemeColors::for_appearance(self.settings.appearance);

        egui::Panel::bottom("actions")
            .frame(
                egui::Frame::new()
                    .fill(colors.panel)
                    .stroke(egui::Stroke::new(1.0, colors.border))
                    .inner_margin(egui::Margin::symmetric(18, 9)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let done = !self.queue.is_empty()
                        && self.states.len() == self.queue.len()
                        && self.states.iter().all(JobState::terminal)
                        && self.progress.1 > 0;
                    let failed = !self.item_errors.is_empty();
                    let conversion_summary = if done && !self.last_reports.is_empty() {
                        let delta: i128 = self
                            .last_reports
                            .iter()
                            .map(|report| report.input_bytes as i128 - report.output_bytes as i128)
                            .sum();
                        let size = format_bytes(delta.unsigned_abs().min(u64::MAX as u128) as u64);
                        if delta >= 0 {
                            format!("{} outputs · {size} saved", self.last_reports.len())
                        } else {
                            format!("{} outputs · {size} larger", self.last_reports.len())
                        }
                    } else {
                        self.summary.clone()
                    };
                    ui.add(
                        egui::Label::new(
                            RichText::new(conversion_summary)
                                .size(12.0)
                                .color(colors.muted),
                        )
                        .truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.running {
                            if ui
                                .button(if self.paused {
                                    "▶  Resume"
                                } else {
                                    "Ⅱ  Pause"
                                })
                                .clicked()
                            {
                                self.paused = !self.paused;
                                if self.paused {
                                    self.worker.pause();
                                } else {
                                    self.worker.resume();
                                }
                            }
                            if ui.button("Cancel batch").clicked() {
                                self.worker.cancel();
                                self.summary = "Cancelling after the current image…".into();
                            }
                        } else {
                            let label = if failed {
                                "Retry failed  →"
                            } else if done {
                                "Convert again  →"
                            } else {
                                "Convert images  →"
                            };
                            if ui
                                .add_enabled(
                                    self.has_valid_output_formats(),
                                    primary_button(label, &colors),
                                )
                                .clicked()
                            {
                                if failed {
                                    self.retry_failed();
                                } else {
                                    self.start_conversion();
                                }
                            }
                        }
                        if !self.last_reports.is_empty()
                            && ui.button("Open output folder").clicked()
                        {
                            super::open_path(&self.output_dir());
                        }
                    });
                });
                if self.progress.1 > 0 {
                    ui.add_space(6.0);
                    ui.add(
                        egui::ProgressBar::new(self.progress.0 as f32 / self.progress.1 as f32)
                            .fill(if self.running {
                                colors.accent
                            } else {
                                colors.success
                            })
                            .text(format!("{} / {} outputs", self.progress.0, self.progress.1)),
                    );
                }
            });

        if self.settings_visible {
            egui::Panel::left("settings_v2")
                .resizable(true)
                .default_size(330.0)
                .frame(
                    egui::Frame::new()
                        .fill(colors.panel)
                        .stroke(egui::Stroke::new(1.0, colors.border))
                        .inner_margin(egui::Margin::symmetric(16, 14)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| settings_panel(self, ui, &colors));
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(colors.canvas)
                    .inner_margin(egui::Margin::symmetric(16, 13)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(if self.settings_visible {
                            "‹  Hide settings"
                        } else {
                            "⚙  Show settings"
                        })
                        .clicked()
                    {
                        self.settings_visible = !self.settings_visible;
                    }
                    ui.separator();
                    if ui
                        .add_enabled(!self.running, primary_button("＋  Add images", &colors))
                        .on_hover_text("Add images · Ctrl+O")
                        .clicked()
                        && let Some(paths) = pick_images()
                    {
                        self.add_paths(paths);
                    }
                    if ui
                        .add_enabled(!self.running, egui::Button::new("▧  Add folder"))
                        .on_hover_text("Add a folder · Ctrl+Shift+O")
                        .clicked()
                        && let Some(path) = pick_folder()
                    {
                        self.add_paths([path]);
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.running && self.selected.is_some(),
                            egui::Button::new("Remove"),
                        )
                        .on_hover_text("Remove selected image · Delete")
                        .clicked()
                    {
                        self.remove_selected();
                    }
                    if ui
                        .add_enabled(
                            !self.running && self.selected.is_some_and(|index| index > 0),
                            egui::Button::new("↑"),
                        )
                        .on_hover_text("Move selected image up · Ctrl+Shift+↑")
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
                            egui::Button::new("↓"),
                        )
                        .on_hover_text("Move selected image down · Ctrl+Shift+↓")
                        .clicked()
                    {
                        self.move_selected(1);
                    }
                    if ui
                        .add_enabled(
                            !self.running && !self.item_errors.is_empty(),
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
                        self.queue_roots.clear();
                        self.image_overrides.clear();
                        self.item_errors.clear();
                        self.item_successes.clear();
                        self.item_skips.clear();
                        self.selected = None;
                        self.refresh_preview();
                        self.summary = "Queue cleared. Add images to start another batch.".into();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.checkbox(&mut self.settings.recursive, "Include subfolders");
                    });
                });
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        workflow_steps(self, ui, &colors);
                        ui.add_space(13.0);
                        queue_panel(self, ui, &colors);
                    });
            });

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
        self.persist();
    }
}
