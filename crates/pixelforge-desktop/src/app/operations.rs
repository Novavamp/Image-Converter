use super::*;

impl PixelForgeApp {
    pub(super) fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let mut added = Vec::new();
        for path in paths {
            let root = if path.is_dir() {
                path.clone()
            } else {
                path.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            };
            let candidates = if path.is_dir() {
                collect_images(&path, self.settings.recursive)
            } else {
                vec![path]
            };
            for candidate in candidates {
                if candidate.is_file() && !self.queue.contains(&candidate) {
                    self.queue.push(candidate.clone());
                    self.states.push(JobState::Pending);
                    self.queue_roots.insert(candidate.clone(), root.clone());
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

    pub(super) fn remove_selected(&mut self) {
        if self.running {
            return;
        }
        if let Some(index) = self.selected.filter(|index| *index < self.queue.len()) {
            let removed = self.queue.remove(index);
            self.states.remove(index);
            self.queue_roots.remove(&removed);
            self.image_overrides.remove(&removed);
            self.item_errors.remove(&removed);
            self.item_successes.remove(&removed);
            self.item_skips.remove(&removed);
            self.selected = if self.queue.is_empty() {
                None
            } else {
                Some(index.min(self.queue.len() - 1))
            };
            self.preview_path = None;
            self.refresh_preview();
        }
    }

    pub(super) fn apply_preset(&mut self, name: &str) {
        if self.running {
            return;
        }
        if let Some(preset) = self
            .settings
            .saved_presets
            .iter()
            .find(|preset| preset.name == name)
            .cloned()
        {
            self.settings.preset = preset.name;
            self.settings.formats = preset.formats;
            self.settings.quality = preset.quality;
            self.settings.resize_enabled = preset.resize_enabled;
            self.settings.resize_width = preset.resize_width;
            self.settings.resize_height = preset.resize_height;
            self.settings.webp_lossless = preset.webp_lossless;
            self.settings.strip_metadata = preset.strip_metadata;
            self.settings.jpeg_background = preset.jpeg_background;
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
            "Archive quality" => {
                self.settings.formats = vec![OutputFormat::Png, OutputFormat::Tiff];
                self.settings.quality = 100;
                self.settings.webp_lossless = true;
                self.settings.resize_enabled = false;
                self.settings.strip_metadata = false;
            }
            _ => self.settings.preset = "Custom".into(),
        }
    }

    pub(super) fn save_current_preset(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() || self.running {
            return;
        }
        if name.len() > 40 {
            self.summary = "Preset names can be up to 40 characters.".into();
            return;
        }
        if [
            "Web optimized",
            "Smallest practical",
            "Lossless conversion",
            "Transparent assets",
            "Email attachment",
            "Archive quality",
            "Custom",
        ]
        .iter()
        .any(|builtin| builtin.eq_ignore_ascii_case(name))
        {
            self.summary = "Choose a different name from the built-in presets.".into();
            return;
        }
        let preset = SavedPreset {
            name: name.to_owned(),
            formats: self.settings.formats.clone(),
            quality: self.settings.quality,
            resize_enabled: self.settings.resize_enabled,
            resize_width: self.settings.resize_width,
            resize_height: self.settings.resize_height,
            webp_lossless: self.settings.webp_lossless,
            strip_metadata: self.settings.strip_metadata,
            jpeg_background: self.settings.jpeg_background,
        };
        if let Some(existing) = self
            .settings
            .saved_presets
            .iter_mut()
            .find(|saved| saved.name.eq_ignore_ascii_case(name))
        {
            *existing = preset;
        } else {
            self.settings.saved_presets.push(preset);
        }
        self.settings.preset = name.to_owned();
        self.preset_name_draft.clear();
    }

    pub(super) fn move_selected(&mut self, offset: isize) {
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

    pub(super) fn retry_failed(&mut self) {
        if self.running {
            return;
        }
        let inputs: Vec<PathBuf> = self
            .queue
            .iter()
            .filter(|path| self.item_errors.contains_key(*path))
            .cloned()
            .collect();
        self.submit_inputs(inputs, true);
    }

    pub(super) fn retry_item(&mut self, input: &Path) {
        if self.running || !self.item_errors.contains_key(input) {
            return;
        }
        self.submit_inputs(vec![input.to_path_buf()], true);
    }

    fn conversion_options(&self) -> ConversionOptions {
        ConversionOptions {
            quality: self.settings.quality,
            resize: self.settings.resize_enabled.then_some(ResizeBounds {
                width: self.settings.resize_width,
                height: self.settings.resize_height,
            }),
            jpeg_background: self.settings.jpeg_background,
            webp_lossless: self.settings.webp_lossless,
            strip_metadata: self.settings.strip_metadata,
            output_suffix: self.settings.output_suffix.clone(),
            skip_existing: self.settings.skip_existing,
        }
    }

    pub(super) fn conversion_options_for(&self, input: &Path) -> ConversionOptions {
        let mut options = self.conversion_options();
        if let Some(resize) = self
            .image_overrides
            .get(input)
            .and_then(|overrides| overrides.resize)
        {
            options.resize = resize;
        }
        options
    }

    pub(super) fn formats_for(&self, input: &Path) -> Vec<OutputFormat> {
        self.image_overrides
            .get(input)
            .and_then(|overrides| overrides.formats.clone())
            .unwrap_or_else(|| self.settings.formats.clone())
    }

    fn batch_request(&self, inputs: &[PathBuf], retry_failed_only: bool) -> BatchRequest {
        let formats = self.settings.formats.clone();
        let options = self.conversion_options();
        let per_input = inputs
            .iter()
            .map(|input| {
                let item_options = self.conversion_options_for(input);
                let mut item_formats = self.formats_for(input);
                if retry_failed_only && let Some(errors) = self.item_errors.get(input) {
                    let mut failed_formats = Vec::new();
                    for (format, _) in errors {
                        if !failed_formats.contains(format) {
                            failed_formats.push(*format);
                        }
                    }
                    if !failed_formats.is_empty() {
                        item_formats = failed_formats;
                    }
                }
                let output_dir = self.output_dir_for(input);
                (
                    input.clone(),
                    InputOverrides {
                        output_dir: Some(output_dir),
                        formats: Some(item_formats),
                        options: Some(item_options),
                    },
                )
            })
            .collect();
        BatchRequest {
            inputs: inputs.to_vec(),
            output_dir: self.output_dir(),
            formats,
            options,
            per_input,
        }
    }

    pub(super) fn submit_inputs(&mut self, inputs: Vec<PathBuf>, retry_failed_only: bool) {
        if inputs.is_empty() {
            self.summary = "There are no images to convert.".into();
            return;
        }
        let batch = self.batch_request(&inputs, retry_failed_only);
        if batch.inputs.iter().any(|input| {
            batch
                .per_input
                .get(input)
                .is_none_or(|item| item.formats.as_ref().is_none_or(Vec::is_empty))
        }) {
            self.summary = "Choose at least one output format for each image.".into();
            return;
        }
        match self.worker.submit(batch) {
            Ok(()) => {
                self.running = true;
                self.paused = false;
                for input in &inputs {
                    if let Some(index) = self.queue.iter().position(|path| path == input) {
                        self.states[index] = JobState::Pending;
                    }
                    self.item_errors.remove(input);
                    self.item_successes.remove(input);
                    self.item_skips.remove(input);
                }
                self.summary = if retry_failed_only {
                    format!("Retrying {} image{}.", inputs.len(), plural(inputs.len()))
                } else {
                    "Preparing conversions…".into()
                };
            }
            Err(error) => self.summary = error.to_string(),
        }
    }

    pub(super) fn poll_worker(&mut self) {
        while let Ok(event) = self.worker.try_recv() {
            match event {
                WorkerEvent::BatchStarted { total } => {
                    self.running = true;
                    self.paused = false;
                    self.progress = (0, total);
                    self.last_reports.clear();
                    self.refresh_preview();
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
                    format,
                    result,
                    skipped,
                    completed,
                    total,
                } => {
                    self.progress = (completed, total);
                    match result {
                        Ok(report) => {
                            self.last_reports.push(report);
                            *self.item_successes.entry(input).or_default() += 1;
                        }
                        Err(error) if skipped => {
                            *self.item_skips.entry(input).or_default() += 1;
                            self.summary = format!("Already exists: {error}");
                        }
                        Err(error) => {
                            self.item_errors
                                .entry(input.clone())
                                .or_default()
                                .push((format, error.clone()));
                            if let Some(index) = self.queue.iter().position(|path| *path == input) {
                                self.states[index] = JobState::Failed;
                            }
                            self.summary = format!(
                                "{} · {}: {error}",
                                input.file_name().unwrap_or_default().to_string_lossy(),
                                format.label(),
                            );
                        }
                    }
                    self.refresh_preview();
                }
                WorkerEvent::BatchFinished {
                    succeeded,
                    failed,
                    skipped,
                    cancelled,
                } => {
                    self.running = false;
                    self.paused = false;
                    self.current_item = None;
                    for (index, input) in self.queue.iter().enumerate() {
                        self.states[index] = if self.item_errors.contains_key(input) {
                            JobState::Failed
                        } else if self.item_successes.get(input).copied().unwrap_or(0) > 0 {
                            JobState::Completed
                        } else if self.item_skips.get(input).copied().unwrap_or(0) > 0 {
                            JobState::Skipped
                        } else if cancelled {
                            JobState::Cancelled
                        } else {
                            self.states[index].clone()
                        };
                    }
                    self.summary = if cancelled {
                        format!(
                            "Stopped · {succeeded} converted · {failed} failed · {skipped} skipped."
                        )
                    } else {
                        format!(
                            "Finished · {succeeded} converted · {failed} failed · {skipped} skipped."
                        )
                    };
                    self.refresh_preview();
                }
            }
        }
    }

    pub(super) fn start_conversion(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let inputs = self.queue.clone();
        self.item_errors.clear();
        self.item_successes.clear();
        self.item_skips.clear();
        self.submit_inputs(inputs, false);
    }

    pub(super) fn has_valid_output_formats(&self) -> bool {
        !self.running
            && !self.queue.is_empty()
            && self.queue.iter().all(|input| {
                !self
                    .image_overrides
                    .get(input)
                    .and_then(|overrides| overrides.formats.as_ref())
                    .unwrap_or(&self.settings.formats)
                    .is_empty()
            })
    }

    pub(super) fn output_dir(&self) -> PathBuf {
        self.settings.output_dir.clone().unwrap_or_else(|| {
            self.queue
                .first()
                .and_then(|path| self.queue_roots.get(path).cloned())
                .or_else(|| {
                    self.queue
                        .first()
                        .and_then(|path| path.parent())
                        .map(Path::to_path_buf)
                })
                .unwrap_or_else(|| Path::new(".").to_path_buf())
                .join("PixelForge Output")
        })
    }

    pub(super) fn output_dir_for(&self, input: &Path) -> PathBuf {
        let base = self.output_dir();
        if !self.settings.preserve_folders {
            return base;
        }
        let Some(root) = self.queue_roots.get(input) else {
            return base;
        };
        let Some(relative_parent) = input.strip_prefix(root).ok().and_then(Path::parent) else {
            return base;
        };
        if relative_parent.as_os_str().is_empty() || relative_parent == Path::new(".") {
            base
        } else {
            base.join(relative_parent)
        }
    }
}
