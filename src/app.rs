use crate::exporter::{encode_indexed_bmp, encode_indexed_png, encode_rgba_image};
use crate::image_processor::{ImageProcessor, TileReduceOptions};
use crate::platform::{self, DialogContext};
use crate::settings_manager::SettingsBundle;
use crate::types::ImageData;
use crate::types::app_state::{AppStateRequest, AppearanceMode, ExportSource, FittedInput, Toast};
use crate::types::image::{ImageDataIndexed, SortMode};
use crate::types::{AppState, ExportFormat};
use crate::ui::{
    draw_footer, draw_header, draw_image_view, draw_main_content, draw_settings_panel,
};
use eframe::egui;
use egui::Margin;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub struct QualetizeApp {
    state: AppState,
    image_processor: ImageProcessor,
}

impl QualetizeApp {
    /// `initial` is handled on the first frame, e.g. an image to open.
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<AppStateRequest>) -> Self {
        crate::ui::styles::init_styles(&cc.egui_ctx);
        let app = Self::default();
        if let Some(request) = initial {
            _ = app.state.app_state_request_sender.send(request);
        }
        app
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());

        let Some(file) = dropped_files.first() else {
            return;
        };
        // Native drops carry a path; browser drops carry the bytes.
        let request = if let Some(path) = &file.path {
            AppStateRequest::LoadImage {
                path: path.display().to_string(),
            }
        } else if let Some(bytes) = &file.bytes {
            AppStateRequest::LoadImageBytes {
                name: file.name.clone(),
                bytes: bytes.to_vec(),
            }
        } else {
            return;
        };
        _ = self.state.app_state_request_sender.send(request);
    }

    fn load_image_file(&mut self, path: String, ctx: &egui::Context) {
        let loaded = ImageData::load(&path, ctx);
        self.install_input_image(path, loaded);
    }

    fn load_image_bytes(&mut self, name: String, bytes: &[u8], ctx: &egui::Context) {
        let loaded = ImageData::load_from_bytes(bytes, &name, ctx);
        self.install_input_image(name, loaded);
    }

    /// Replace the input image (and everything derived from it) with `loaded`.
    fn install_input_image(&mut self, name: String, loaded: Result<ImageData, String>) {
        self.image_processor.cancel_all();
        self.state.reset_all_images();

        match loaded {
            Ok(image_data) => {
                self.state.input_path = Some(name);
                self.state.input_image = Some(image_data);
                self.state.zoom = 1.0;
                self.state.pan_offset = egui::Vec2::ZERO;
            }
            Err(e) => log::error!("File load Error {e}"),
        }
    }

    /// Start the pending quantization once the debounce delay has passed and
    /// the previous run is done. Tile reduction is requested again by
    /// [`Self::check_quantize_completion`] once the new base image exists.
    fn handle_settings_changes(&mut self) {
        let Some(color_corrected_image) = &self.state.color_corrected_image else {
            return;
        };
        let Some(request) = &self.state.request_update_qualetized_image else {
            return;
        };
        if request.time.elapsed() < self.state.debounce_delay
            || self.image_processor.is_quantizing()
        {
            return;
        }

        self.state.request_update_qualetized_image = None;
        self.image_processor.cancel_tile_reduce();
        if self.state.tpq_settings.randomize_seed {
            self.state.tpq_settings.rand_seed = rand_seed_from_clock();
        }
        self.state.quantize_progress = Some(0);
        self.image_processor.start_quantize(
            &color_corrected_image.rgba_data,
            color_corrected_image.width,
            color_corrected_image.height,
            self.state.engine,
            self.state.settings.clone(),
            self.state.tpq_settings.clone(),
        );
    }

    /// Show what the running quantization has reported since the last frame.
    fn check_quantize_progress(&mut self, ctx: &egui::Context) {
        let Some(progress) = self.image_processor.poll_quantize_progress() else {
            return;
        };
        self.state.quantize_progress = Some(progress.percent);
        if let Some(preview) = progress.preview {
            let indexed = ImageDataIndexed::new(
                preview.palette_data,
                preview.colors_per_palette,
                preview.indexed_data,
            );
            self.state.base_output_image = Some(ImageData::from_indexed(
                indexed,
                preview.width,
                preview.height,
                ctx,
            ));
        }
    }

    /// Extend the input image so both sides are a multiple of the tile size,
    /// filling the added area with the top-left pixel color.
    ///
    /// The loaded image is never modified, so changing the tile size later
    /// re-derives the extension from it instead of compounding.
    /// Returns true when the image the pipeline runs on changed.
    fn update_tile_fit(&mut self, ctx: &egui::Context) -> bool {
        let Some(input) = &self.state.input_image else {
            return self.state.tile_fitted_input.take().is_some();
        };

        let target = tile_fit_target(
            (input.width, input.height),
            self.state.settings.tile_width,
            self.state.settings.tile_height,
        );

        if target == (input.width, input.height) {
            return self.state.tile_fitted_input.take().is_some();
        }
        if let Some(fitted) = &self.state.tile_fitted_input
            && (fitted.image.width, fitted.image.height) == target
        {
            return false;
        }

        log::info!(
            "Extending {}×{} to {}×{} to fit the tile grid",
            input.width,
            input.height,
            target.0,
            target.1
        );
        let image = input.extended_to(target.0, target.1, input.top_left_pixel(), ctx);
        self.state.tile_fitted_input = Some(FittedInput {
            image,
            original_size: (input.width, input.height),
        });
        self.state.tile_fit_toast =
            Some(Toast::new(format!("Extended to {}×{}", target.0, target.1)));
        true
    }

    /// Rebuild the color corrected image from the current pipeline input and
    /// queue a re-quantization of it.
    fn refresh_color_corrected_image(&mut self, ctx: &egui::Context) {
        let Some(image) = self.state.processing_input() else {
            self.state.color_corrected_image = None;
            return;
        };

        // With color correction off the input is passed through untouched, which
        // also skips a full pixel pass and a texture upload.
        self.state.color_corrected_image = Some(if self.state.color_correction.enabled {
            image.color_corrected(&self.state.color_correction, ctx)
        } else {
            image.clone()
        });
        self.state.update_color_correction_tracking();
        self.state.request_qualetize();
    }

    fn check_quantize_completion(&mut self, ctx: &egui::Context) {
        let Some(result) = self.image_processor.poll_quantize() else {
            return;
        };
        self.state.quantize_progress = None;
        match result {
            Ok(res) => {
                let indexed = ImageDataIndexed::new(
                    res.palette_data,
                    res.colors_per_palette,
                    res.indexed_data,
                );
                let image = ImageData::from_indexed(indexed, res.width, res.height, ctx);
                self.state.base_output_image = Some(image);
                // Shows the base image right away and starts the reduction
                // pass on it when that is enabled.
                self.state.request_update_tile_reduce = true;
            }
            Err(e) => {
                log::error!("Failed to generate preview image: {e}");
                self.state.reset_qualetize_outputs();
            }
        }
    }

    fn check_tile_reduce_completion(&mut self, ctx: &egui::Context) {
        let Some(res) = self.image_processor.poll_tile_reduce() else {
            return;
        };
        let Some(base_indexed) = self
            .state
            .base_output_image
            .as_ref()
            .and_then(|base| base.indexed.as_ref())
        else {
            return;
        };
        let base = self.state.base_output_image.as_ref().unwrap();

        let indexed = ImageDataIndexed::new(
            base_indexed.palettes.clone(),
            base_indexed.colors_per_palette(),
            res.indexed_pixels,
        );
        let output = ImageData::from_indexed(indexed, base.width, base.height, ctx);
        self.state.output_image = Some(output);
        self.state.invalidate_palette_sort();
        self.state.tile_count.mark_dirty();
        self.update_tile_counts();

        let diff = match (self.state.base_tile_count, self.state.reduced_tile_count) {
            (Some(base), Some(reduced)) => base.saturating_sub(reduced),
            _ => res.merged,
        };
        self.state.tile_reduce_toast = Some(Toast::new(format!("Reduced {diff} tiles")));
        log::info!("Tile reduce completed: merged {}", res.merged);
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let visuals = match self.state.preferences.appearance_mode {
            AppearanceMode::Dark => egui::Visuals::dark(),
            AppearanceMode::Light => egui::Visuals::light(),
            AppearanceMode::System => match ctx.system_theme() {
                Some(egui::Theme::Light) => egui::Visuals::light(),
                _ => egui::Visuals::dark(),
            },
        };
        if ctx.global_style().visuals != visuals {
            ctx.set_visuals(visuals);
        }
    }

    /// Show the base image and, when enabled, start the tile reduction pass on it.
    fn handle_tile_reduce_changes(&mut self, ctx: &egui::Context) {
        if !self.state.request_update_tile_reduce {
            return;
        }
        self.state.request_update_tile_reduce = false;
        self.image_processor.cancel_tile_reduce();

        let Some(base) = &self.state.base_output_image else {
            return;
        };
        let reduce_input = base
            .indexed
            .as_ref()
            .map(|indexed| (indexed.indexed_pixels.clone(), indexed.palettes.clone()));
        let (width, height) = (base.width, base.height);

        self.state.output_image = Some(base.clone());
        self.state.invalidate_palette_sort();
        self.state.tile_count.mark_dirty();

        let settings = &self.state.settings;
        if !settings.tile_reduce_post_enabled || settings.tile_reduce_post_threshold <= 0.0 {
            return;
        }
        let Some((indexed_pixels, palettes)) = reduce_input else {
            return;
        };

        let opts = TileReduceOptions {
            tile_width: settings.tile_width,
            tile_height: settings.tile_height,
            threshold: settings.tile_reduce_post_threshold,
            allow_flip_x: settings.tile_reduce_allow_flip_x,
            allow_flip_y: settings.tile_reduce_allow_flip_y,
        };
        self.image_processor
            .start_tile_reduce(indexed_pixels, palettes, width, height, opts);
        ctx.request_repaint();
    }

    /// Recount the unique tiles of both output images once something they
    /// depend on changed: the images themselves or the counting options.
    /// Both numbers are derived here, in one place, so the footer count and
    /// the "Reduced N tiles" label can never disagree about the options.
    fn update_tile_counts(&mut self) {
        if !self.state.tile_count.dirty {
            return;
        }
        self.state.tile_count.dirty = false;

        let count = |image: &Option<ImageData>| {
            let image = image.as_ref()?;
            ImageData::count_unique_tiles(
                image.indexed.as_ref()?,
                image.width,
                image.height,
                self.state.settings.tile_width,
                self.state.settings.tile_height,
                self.state.tile_count.options(),
            )
        };
        self.state.base_tile_count = count(&self.state.base_output_image);
        self.state.reduced_tile_count = count(&self.state.output_image);
    }

    /// Encode `source` in `format`, ready to be written out.
    fn encode_export(&self, source: ExportSource, format: ExportFormat) -> Result<Vec<u8>, String> {
        if source == ExportSource::ColorCorrected {
            let image = self
                .state
                .color_corrected_image
                .as_ref()
                .ok_or("no color corrected image in memory")?;
            return encode_rgba_image(&image.rgba_data, image.width, image.height, format);
        }

        let (indexed, width, height) = self
            .state
            .indexed_for_export(source)
            .ok_or_else(|| format!("no indexed image available for {source:?}"))?;
        let pixels = &indexed.indexed_pixels;
        let palettes = &indexed.palettes;
        match format {
            ExportFormat::Bmp => encode_indexed_bmp(pixels, palettes, width, height),
            ExportFormat::PngIndexed => encode_indexed_png(pixels, palettes, width, height),
            ExportFormat::Png => Err("indexed export needs an indexed format".to_string()),
        }
    }

    /// What a file dialog needs to report its result back to the app.
    fn dialog_context(&self, ctx: &egui::Context) -> DialogContext {
        DialogContext {
            sender: self.state.app_state_request_sender.clone(),
            dialog_open: self.state.file_dialog_open.clone(),
            egui_ctx: ctx.clone(),
        }
    }

    /// Replace the settings in use with `bundle`.
    fn apply_settings_bundle(&mut self, bundle: SettingsBundle, ctx: &egui::Context) {
        self.image_processor.cancel_all();
        self.state.engine = bundle.engine;
        self.state.settings = bundle.qualetize_settings;
        self.state.tpq_settings = bundle.tpq_settings;
        self.state.color_correction = bundle.color_correction;
        self.state.palette_sort_settings = bundle.sort_settings;
        self.update_tile_fit(ctx);
        self.refresh_color_corrected_image(ctx);
    }

    fn handle_requests(&mut self, ctx: &egui::Context) {
        let Ok(request) = self.state.app_state_request_receiver.try_recv() else {
            return;
        };
        match request {
            AppStateRequest::LoadImage { path } => {
                self.load_image_file(path, ctx);
                self.update_tile_fit(ctx);
                self.refresh_color_corrected_image(ctx);
            }
            AppStateRequest::LoadImageBytes { name, bytes } => {
                self.load_image_bytes(name, &bytes, ctx);
                self.update_tile_fit(ctx);
                self.refresh_color_corrected_image(ctx);
            }
            AppStateRequest::LoadSettings { path } => match SettingsBundle::load_from_file(&path) {
                Ok(bundle) => {
                    self.apply_settings_bundle(bundle, ctx);
                    log::info!("Settings loaded successfully from: {path}");
                }
                Err(e) => log::error!("Failed to load settings: {e}"),
            },
            AppStateRequest::LoadSettingsBytes { name, bytes } => {
                match String::from_utf8(bytes)
                    .map_err(|e| format!("Settings file is not text: {e}"))
                    .and_then(|json| SettingsBundle::from_json(&json))
                {
                    Ok(bundle) => {
                        self.apply_settings_bundle(bundle, ctx);
                        log::info!("Settings loaded successfully from: {name}");
                    }
                    Err(e) => log::error!("Failed to load settings: {e}"),
                }
            }
            AppStateRequest::OpenImageDialog => platform::pick_image(self.dialog_context(ctx)),
            AppStateRequest::ExportImageDialog { source, format } => {
                let Some(input_path) = self.state.input_path.clone() else {
                    return;
                };
                let bytes = match self.encode_export(source, format) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        log::error!("Export failed: {e}");
                        return;
                    }
                };
                let default_path =
                    platform::export_path(&input_path, format, Some(source.file_suffix()));
                platform::export_image(
                    bytes,
                    default_path.display().to_string(),
                    format,
                    self.dialog_context(ctx),
                );
            }
            AppStateRequest::SaveSettingsDialog => match self.state.settings_bundle().to_json() {
                Ok(json) => platform::save_settings(
                    json,
                    platform::DEFAULT_SETTINGS_FILE_NAME,
                    self.dialog_context(ctx),
                ),
                Err(e) => log::error!("Failed to save settings: {e}"),
            },
            AppStateRequest::LoadSettingsDialog => {
                platform::pick_settings_file(self.dialog_context(ctx))
            }
        }
    }

    fn update_palette_sort_settings(&mut self) {
        if !self.state.palette_sort_needs_update() {
            return;
        }
        self.state.clear_palette_sort_dirty();

        let settings = self.state.palette_sort_settings;
        if settings.mode == SortMode::None {
            self.state.output_palette_sorted_indexed_image = None;
            return;
        }

        let pin_first = self.state.first_color_pinned();
        self.state.output_palette_sorted_indexed_image = self
            .state
            .output_image
            .as_ref()
            .and_then(|image| image.indexed.as_ref())
            .map(|indexed| {
                indexed.sorted(
                    settings.mode,
                    settings.order,
                    settings.ramps_hue_gap,
                    pin_first,
                )
            });
    }
}

impl Drop for QualetizeApp {
    fn drop(&mut self) {
        self.image_processor.cancel_all();
        // Flush a change made in the last moments before quitting
        self.state.save_session_now();
    }
}

impl eframe::App for QualetizeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;

        self.apply_theme(ctx);

        // Handle drag and drop first
        if !self.state.file_dialog_open.load(Ordering::Relaxed) {
            self.handle_dropped_files(ctx);
        }

        self.check_quantize_progress(ctx);
        self.check_quantize_completion(ctx);
        self.check_tile_reduce_completion(ctx);

        // Re-extend if the tile size changed, then update the corrected image
        if self.update_tile_fit(ctx) || self.state.color_correction_changed() {
            self.refresh_color_corrected_image(ctx);
        }

        // Handle settings changes after checking completion
        self.handle_settings_changes();

        // Apply tile reduce changes without re-qualetizing
        self.handle_tile_reduce_changes(ctx);

        self.update_palette_sort_settings();
        self.update_tile_counts();

        self.handle_requests(ctx);

        // Mirror preferences and settings to disk so they survive a restart
        self.state.check_and_save_preferences();
        self.state.check_and_save_session(ctx);

        // The Qualetized and Tile Reduced panels show their own spinners.
        let qualetize_processing = self.image_processor.is_quantizing();
        self.state.tile_reduce_processing = self.image_processor.is_tile_reducing();

        let mut settings_changed = false;
        let mut tile_reduce_changed = false;

        // Panels have to be declared before the central panel, otherwise the
        // central panel is sized as if they were not there.

        // Top (menu)
        egui::Panel::top("menu_panel").show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(Margin::symmetric(0, 4))
                .show(ui, |ui| {
                    let (settings, tile_reduce) = draw_header(ui, &mut self.state);
                    settings_changed |= settings;
                    tile_reduce_changed |= tile_reduce;
                });
        });

        // Bottom (zoom / export / tile count)
        if self.state.input_image.is_some() {
            egui::Panel::bottom("footer").show(ui, |ui| {
                egui::Frame::NONE
                    .inner_margin(Margin::symmetric(0, 4))
                    .show(ui, |ui| {
                        draw_footer(ui, &mut self.state);
                    });
            });
        }

        // Left (settings)
        egui::Panel::left("settings_panel")
            .default_size(260.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (settings, tile_reduce) = draw_settings_panel(ui, &mut self.state);
                    settings_changed |= settings;
                    tile_reduce_changed |= tile_reduce;
                });
            });

        // Center (images)
        egui::CentralPanel::default_margins()
            .frame(
                egui::Frame::default()
                    .inner_margin(0.0)
                    .fill(ctx.global_style().visuals.window_fill()),
            )
            .show(ui, |ui| {
                if self.state.input_path.is_none() {
                    draw_main_content(ui);
                } else {
                    draw_image_view(ui, &mut self.state, qualetize_processing);
                }
            });

        if settings_changed {
            self.state.request_qualetize();
        }
        if tile_reduce_changed {
            self.state.request_update_tile_reduce = true;
        }

        // Keep repainting while work is pending or in flight
        if qualetize_processing
            || self.state.tile_reduce_processing
            || self.state.request_update_qualetized_image.is_some()
            || self.state.request_update_tile_reduce
        {
            ctx.request_repaint();
        }
    }
}

/// A seed for a run that asked for a random one. Wall clock nanoseconds are
/// plenty: the seed is recorded in the settings, so any value reproduces.
fn rand_seed_from_clock() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32))
        .unwrap_or(0);
    nanos.max(1)
}

/// Smallest size at or above `size` whose sides are multiples of the tile size.
fn tile_fit_target(size: (u32, u32), tile_width: u16, tile_height: u16) -> (u32, u32) {
    (
        size.0.next_multiple_of(tile_width.max(1) as u32),
        size.1.next_multiple_of(tile_height.max(1) as u32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_already_on_the_tile_grid_is_left_alone() {
        assert_eq!(tile_fit_target((96, 96), 8, 8), (96, 96));
        assert_eq!(tile_fit_target((64, 32), 16, 16), (64, 32));
    }

    #[test]
    fn a_size_off_the_grid_grows_to_the_next_multiple() {
        assert_eq!(tile_fit_target((100, 100), 8, 8), (104, 104));
        assert_eq!(tile_fit_target((100, 100), 7, 7), (105, 105));
        assert_eq!(tile_fit_target((17, 3), 8, 8), (24, 8));
    }

    #[test]
    fn each_axis_uses_its_own_tile_size() {
        assert_eq!(tile_fit_target((100, 100), 8, 16), (104, 112));
    }

    /// An image smaller than one tile still has to grow to a full tile.
    #[test]
    fn an_image_smaller_than_a_tile_grows_to_one_tile() {
        assert_eq!(tile_fit_target((3, 5), 8, 8), (8, 8));
    }
}
