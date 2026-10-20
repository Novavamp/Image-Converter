use super::*;

impl PixelForgeApp {
    pub(super) fn refresh_preview(&mut self) {
        let Some(path) = self
            .selected
            .and_then(|index| self.queue.get(index))
            .cloned()
        else {
            self.preview = None;
            self.preview_error = None;
            self.preview_path = None;
            self.estimate_key = None;
            self.estimated_sizes = None;
            self.output_preview = None;
            self.output_preview_error = None;
            self.output_preview_path = None;
            return;
        };
        let path_changed = self.preview_path.as_ref() != Some(&path);
        if path_changed {
            self.preview_path = Some(path.clone());
            self.preview = None;
            self.preview_error = None;
        }
        let estimate_key = self.make_estimate_key(&path);
        if path_changed || self.estimate_key.as_ref() != Some(&estimate_key) {
            self.estimate_key = Some(estimate_key.clone());
            self.estimated_sizes = None;
            self.spawn_preview(path.clone(), false, Some(estimate_key));
        }

        let output_path = self
            .last_reports
            .iter()
            .rev()
            .find(|report| report.input == path)
            .map(|report| report.output.clone());
        if self.output_preview_path != output_path {
            self.output_preview_path = output_path.clone();
            self.output_preview = None;
            self.output_preview_error = None;
            if let Some(output_path) = output_path {
                self.spawn_preview(output_path, true, None);
            }
        }
    }

    fn make_estimate_key(&self, path: &Path) -> String {
        let options = self.conversion_options_for(path);
        format!(
            "{:?}|{}|{:?}|{:?}|{}|{}",
            self.formats_for(path),
            options.quality,
            options.resize.map(|bounds| (bounds.width, bounds.height)),
            options.jpeg_background,
            options.webp_lossless,
            options.strip_metadata,
        )
    }

    fn spawn_preview(&self, path: PathBuf, is_output: bool, estimate_key: Option<String>) {
        let sender = self.preview_sender.clone();
        let options = self.conversion_options_for(&path);
        let formats = if is_output {
            Vec::new()
        } else {
            self.formats_for(&path)
        };
        std::thread::spawn(move || {
            let result = (|| {
                let info = inspect(&path).map_err(|error| error.to_string())?;
                let (_, image) = read_image(&path).map_err(|error| error.to_string())?;
                let thumb = image.thumbnail(900, 600).to_rgba8();
                let size = [thumb.width() as usize, thumb.height() as usize];
                let estimates = formats
                    .iter()
                    .filter_map(|format| {
                        estimate_output_size(&image, *format, &options)
                            .ok()
                            .map(|(bytes, width, height)| (*format, bytes, width, height))
                    })
                    .collect();
                Ok(PreviewPayload {
                    info,
                    size,
                    rgba: thumb.into_raw(),
                    estimates,
                })
            })();
            let _ = sender.send(PreviewData {
                path,
                is_output,
                estimate_key,
                result,
            });
        });
    }

    pub(super) fn poll_previews(&mut self, ctx: &egui::Context) {
        while let Ok(data) = self.preview_receiver.try_recv() {
            let current_path = if data.is_output {
                self.output_preview_path.as_ref()
            } else {
                self.preview_path.as_ref()
            };
            if current_path != Some(&data.path) {
                continue;
            }
            if !data.is_output && data.estimate_key.as_ref() != self.estimate_key.as_ref() {
                continue;
            }
            match data.result {
                Ok(payload) => {
                    let image =
                        egui::ColorImage::from_rgba_unmultiplied(payload.size, &payload.rgba);
                    let texture = ctx.load_texture(
                        if data.is_output {
                            "converted-preview"
                        } else {
                            "source-preview"
                        },
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    if data.is_output {
                        self.output_preview = Some((payload.info, texture));
                        self.output_preview_error = None;
                    } else {
                        self.preview = Some((payload.info, texture));
                        self.estimated_sizes = Some(payload.estimates);
                        self.preview_error = None;
                    }
                }
                Err(error) => {
                    if data.is_output {
                        self.output_preview_error = Some(error);
                    } else {
                        self.preview_error = Some(error);
                    }
                }
            }
        }
    }
}
