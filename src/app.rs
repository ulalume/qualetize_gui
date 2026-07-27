use std::path::Path;

use crate::exporter::{save_indexed_bmp, save_indexed_png, save_rgba_image};
use crate::image_processor::ImageProcessor;
use crate::settings_manager::SettingsBundle;
use crate::types::ImageData;
use crate::types::app_state::{AppStateRequest, AppearanceMode, QualetizeRequest};
use crate::types::image::{ImageDataIndexed, SortMode, TileCountOptions};
use crate::types::{AppState, ExportFormat};
use crate::ui::{
    draw_footer, draw_header, draw_image_view, draw_main_content, draw_settings_panel,
};
use eframe::egui;
use egui::{ColorImage, Margin};
use rfd::FileDialog;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct QualetizeApp {
    state: AppState,
    image_processor: ImageProcessor,
}

impl Default for QualetizeApp {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            image_processor: ImageProcessor::new(),
        }
    }
}

impl QualetizeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;

        crate::ui::styles::init_styles(ctx);

        Self::default()
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());

        if !dropped_files.is_empty()
            && let Some(dropped_file) = dropped_files.first()
            && let Some(path) = &dropped_file.path
        {
            _ = self
                .state
                .app_state_request_sender
                .send(AppStateRequest::LoadImage {
                    path: path.display().to_string(),
                });
        }
    }

    fn load_image_file(&mut self, path: String, ctx: &egui::Context) {
        // Cancel any existing processing
        if self.image_processor.is_processing() {
            self.image_processor.cancel_current_processing();
            self.image_processor = ImageProcessor::new();
        }
        self.image_processor.cancel_tile_reduce();
        self.state.tile_reduce_processing = false;

        match ImageData::load(&path, ctx) {
            Ok(image_data) => {
                self.state.reset_all_images();
                self.state.input_path = Some(path);
                self.state.input_image = Some(image_data);

                // Check tile size compatibility
                self.check_tile_size_compatibility();

                self.state.zoom = 1.0;
                self.state.pan_offset = egui::Vec2::ZERO;
            }
            Err(e) => {
                log::error!("File load Error {e}");
                self.state.reset_all_images();
            }
        }
    }

    fn handle_settings_changes(&mut self) {
        if !self.check_tile_size_compatibility() {
            return;
        }
        let Some(color_corrected_image) = &self.state.color_corrected_image else {
            return;
        };
        // debounce functionality: start preview generation after a certain delay from settings change
        let Some(request) = &self.state.request_update_qualetized_image else {
            return;
        };
        if request.time.elapsed() < self.state.debounce_delay {
            return;
        }
        if self.image_processor.is_processing() {
            return;
        }

        self.state.request_update_qualetized_image = None;
        self.image_processor.cancel_tile_reduce();
        self.state.tile_reduce_processing = false;
        self.image_processor
            .start_qualetize(color_corrected_image, self.state.settings.clone());

        // request tile reduce after qualetize finishes
        self.state.request_update_tile_reduce = self.state.settings.tile_reduce_post_enabled;
    }

    fn check_tile_size_compatibility(&mut self) -> bool {
        let Some(input_image) = &self.state.input_image else {
            return true;
        };

        let image_width = input_image.width as u16;
        let image_height = input_image.height as u16;
        let tile_width = self.state.settings.tile_width;
        let tile_height = self.state.settings.tile_height;

        let width_divisible = image_width.is_multiple_of(tile_width);
        let height_divisible = image_height.is_multiple_of(tile_height);

        log::debug!(
            "Tile size check: image {image_width}×{image_height}, tile {tile_width}×{tile_height}, divisible: width={width_divisible}, height={height_divisible}"
        );

        if !width_divisible || !height_divisible {
            self.state.tile_size_warning = true;
            self.state.output_image = None;
            self.state.invalidate_palette_sort();
            self.state.tile_count.reset();
            log::warn!("Tile size warning");
            false
        } else {
            self.state.tile_size_warning = false;
            log::debug!("No warning - sizes are compatible");
            true
        }
    }

    fn update_color_corrected_image(&mut self, ctx: &egui::Context) {
        if self.state.color_correction_changed() {
            self.apply_color_correct_image(ctx);
            self.state.request_update_qualetized_image = Some(QualetizeRequest {
                time: std::time::Instant::now(),
            });
            self.state.update_color_correction_tracking();
        }
    }

    fn check_preview_completion(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.image_processor.check_preview_complete(ctx) {
            match result {
                Ok(image_data) => {
                    self.state.base_output_image = Some(image_data.clone());
                    self.state.base_tile_count = Self::count_tiles(
                        &image_data,
                        self.state.settings.tile_width,
                        self.state.settings.tile_height,
                        self.state.tile_count.options(),
                    );
                    if !self.state.settings.tile_reduce_post_enabled
                        || self.state.settings.tile_reduce_post_threshold <= 0.0
                    {
                        self.state.output_image = Some(image_data);
                        self.state.reduced_tile_count = self.state.base_tile_count;
                        self.state.tile_reduce_processing = false;
                    } else {
                        self.state.request_update_tile_reduce = true;
                        self.handle_tile_reduce_changes(ctx);
                    }
                    self.state.invalidate_palette_sort();
                    self.state.tile_count.reset();
                }
                Err(e) => {
                    log::error!("Failed to generate preview image: {e}");
                    self.state.reset_qualetize_outputs();
                    self.state.tile_reduce_processing = false;
                }
            }
        }
    }

    fn check_tile_reduce_completion(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.image_processor.check_tile_reduce_complete() {
            match result {
                Ok(res) => {
                    if res.generation_id != self.state.tile_reduce_generation_id {
                        log::debug!("Ignoring stale tile reduce result");
                        return;
                    }
                    let Some(base) = &self.state.base_output_image else {
                        return;
                    };
                    let Some(base_indexed) = &base.indexed else {
                        return;
                    };
                    let mut pixels = Vec::with_capacity((base.width * base.height * 4) as usize);
                    for &pixel_index in &res.indexed_pixels {
                        let palette_index = pixel_index as usize;
                        if let Some(color) = base_indexed.palettes.get(palette_index) {
                            pixels.extend_from_slice(&[color.r, color.g, color.b, color.a]);
                        } else {
                            pixels.extend_from_slice(&[0, 0, 0, 255]);
                        }
                    }
                    let size = [base.width as usize, base.height as usize];
                    let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
                    let texture =
                        ctx.load_texture("output", color_image, egui::TextureOptions::NEAREST);

                    let mut output = base.clone();
                    output.texture = texture;
                    output.rgba_data = pixels;
                    output.indexed = Some(ImageDataIndexed {
                        palettes_for_ui: base_indexed.palettes_for_ui.clone(),
                        palettes: base_indexed.palettes.clone(),
                        indexed_pixels: res.indexed_pixels,
                    });
                    self.state.output_image = Some(output);
                    self.state.invalidate_palette_sort();
                    self.state.reduced_tile_count = Self::count_tiles(
                        self.state.output_image.as_ref().unwrap(),
                        self.state.settings.tile_width,
                        self.state.settings.tile_height,
                        self.state.tile_count.options(),
                    );
                    self.state.tile_reduce_processing = false;
                    let diff = self
                        .state
                        .base_tile_count
                        .and_then(|base| {
                            self.state
                                .reduced_tile_count
                                .map(|reduced| base.saturating_sub(reduced))
                        })
                        .unwrap_or(res.merged);
                    self.state.tile_reduce_toast = Some(crate::types::app_state::TileReduceToast {
                        message: format!("Reduced {} tiles", diff),
                        time: std::time::Instant::now(),
                    });
                    self.state.tile_count.mark_dirty();
                    log::info!("Tile reduce completed: merged {}", res.merged);
                }
                Err(e) => {
                    log::error!("Tile reduce failed: {e}");
                    self.state.tile_reduce_processing = false;
                }
            }
        }
    }
    fn apply_theme(&self, ctx: &egui::Context) {
        let visuals = match self.state.preferences.appearance_mode {
            AppearanceMode::Dark => egui::Visuals::dark(),
            AppearanceMode::Light => egui::Visuals::light(),
            AppearanceMode::System => match ctx.system_theme() {
                Some(egui::Theme::Dark) => egui::Visuals::dark(),
                Some(egui::Theme::Light) => egui::Visuals::light(),
                None => egui::Visuals::dark(),
            },
        };
        if ctx.style().visuals != visuals {
            ctx.set_visuals(visuals);
        }
    }

    fn apply_color_correct_image(&mut self, ctx: &egui::Context) {
        if let Some(image) = &self.state.input_image {
            let color_corrected_image = image.color_corrected(&self.state.color_correction, ctx);
            self.state.color_corrected_image = Some(color_corrected_image);
        }
    }

    fn handle_tile_reduce_changes(&mut self, ctx: &egui::Context) {
        if !self.state.request_update_tile_reduce {
            return;
        }
        self.state.request_update_tile_reduce = false;

        let Some(base) = &self.state.base_output_image else {
            return;
        };

        // Snapshot everything the worker needs before mutating state below.
        let base_image = base.clone();
        let reduce_input = base
            .indexed
            .as_ref()
            .map(|indexed| (indexed.indexed_pixels.clone(), indexed.palettes.clone()));
        let (base_width, base_height) = (base.width, base.height);

        self.state.output_image = Some(base_image);
        self.state.invalidate_palette_sort();
        self.state.reduced_tile_count = self.state.base_tile_count;
        self.state.tile_count.mark_dirty();
        if !self.state.settings.tile_reduce_post_enabled
            || self.state.settings.tile_reduce_post_threshold <= 0.0
        {
            self.state.tile_reduce_processing = false;
            return;
        }

        let Some((indexed_pixels, palettes)) = reduce_input else {
            self.state.tile_reduce_processing = false;
            return;
        };

        let opts = crate::image_processor::TileReduceOptions {
            tile_width: self.state.settings.tile_width,
            tile_height: self.state.settings.tile_height,
            threshold: self.state.settings.tile_reduce_post_threshold,
            allow_flip_x: self.state.settings.tile_reduce_allow_flip_x,
            allow_flip_y: self.state.settings.tile_reduce_allow_flip_y,
            use_blur: true,
        };

        let generation_id = self.image_processor.start_tile_reduce(
            indexed_pixels,
            palettes,
            base_width,
            base_height,
            opts,
        );
        self.state.tile_reduce_generation_id = generation_id;
        self.state.tile_reduce_processing = true;
        ctx.request_repaint();
    }

    fn count_tiles(
        image: &ImageData,
        tile_w: u16,
        tile_h: u16,
        options: TileCountOptions,
    ) -> Option<usize> {
        let indexed = image.indexed.as_ref()?;
        ImageData::count_unique_tiles(indexed, image.width, image.height, tile_w, tile_h, options)
    }

    /// Run a native file dialog on a worker thread so the UI keeps repainting,
    /// and feed whatever the user picked back into the request channel.
    ///
    /// `file_dialog_open` is held for the lifetime of the dialog so drag & drop
    /// is ignored while it is up.
    fn spawn_file_dialog<F>(&self, pick: F)
    where
        F: FnOnce(FileDialog) -> Option<AppStateRequest> + Send + 'static,
    {
        let sender = self.state.app_state_request_sender.clone();
        let dialog_flag = self.state.file_dialog_open.clone();

        std::thread::spawn(move || {
            let _guard = FileDialogGuard::new(dialog_flag);
            if let Some(request) = pick(FileDialog::new()) {
                _ = sender.send(request);
            }
        });
    }

    fn handle_requests(&mut self, ctx: &egui::Context) {
        // Check for file dialog results first
        let Ok(app_state_request) = &self.state.app_state_request_receiver.try_recv() else {
            return;
        };
        match app_state_request {
            AppStateRequest::LoadImage { path } => {
                self.load_image_file(path.clone(), ctx);
                self.apply_color_correct_image(ctx);
                self.state.request_update_qualetized_image = Some(QualetizeRequest {
                    time: std::time::Instant::now(),
                });
                self.state.update_color_correction_tracking();
            }
            AppStateRequest::ColorCorrectedPng { output_path } => {
                // Use ImageData pixels directly
                let Some(color_corrected_image) = &self.state.color_corrected_image else {
                    log::error!("No color corrected image data available in memory");
                    return;
                };

                let output_path = output_path.clone();
                let rgba_data = color_corrected_image.rgba_data.clone();
                let width = color_corrected_image.width;
                let height = color_corrected_image.height;
                std::thread::spawn(move || {
                    match save_rgba_image(
                        &output_path,
                        &rgba_data,
                        width,
                        height,
                        crate::types::ExportFormat::Png,
                    ) {
                        Ok(()) => {
                            log::info!(
                                "Color corrected PNG export completed successfully (from memory)"
                            );
                        }
                        Err(e) => {
                            log::error!("Color corrected PNG export failed: {e}");
                        }
                    }
                });
            }
            AppStateRequest::QualetizedIndexed {
                output_path,
                format,
            } => {
                let Some(output_image) = &self.state.output_image else {
                    log::error!("Qualetized export failed: output image is None");
                    return;
                };

                // Prefer the sorted palette when one is active.
                let Some(indexed) = self
                    .state
                    .output_palette_sorted_indexed_image
                    .as_ref()
                    .or(output_image.indexed.as_ref())
                else {
                    return;
                };

                match format {
                    crate::types::ExportFormat::Png => {
                        log::error!("Qualetized export failed: Unexpected format");
                    }
                    crate::types::ExportFormat::Bmp => {
                        match save_indexed_bmp(
                            output_path,
                            &indexed.indexed_pixels,
                            &indexed.palettes,
                            output_image.width,
                            output_image.height,
                        ) {
                            Ok(()) => {
                                log::info!("Qualetized indexed BMP export completed successfully");
                            }
                            Err(e) => {
                                log::error!("Qualetized indexed export failed: {e}");
                            }
                        }
                    }
                    crate::types::ExportFormat::PngIndexed => {
                        match save_indexed_png(
                            output_path,
                            &indexed.indexed_pixels,
                            &indexed.palettes,
                            output_image.width,
                            output_image.height,
                        ) {
                            Ok(()) => {
                                log::info!("Qualetized indexed PNG export completed successfully");
                            }
                            Err(e) => {
                                log::error!("Qualetized indexed export failed: {e}");
                            }
                        }
                    }
                }
            }
            AppStateRequest::SaveSettings { path } => {
                let settings_bundle = SettingsBundle::new(
                    self.state.settings.clone(),
                    self.state.color_correction.clone(),
                    self.state.palette_sort_settings,
                );

                match settings_bundle.save_to_file(path) {
                    Ok(()) => {
                        log::info!("Settings saved successfully to: {path}");
                    }
                    Err(e) => {
                        log::error!("Failed to save settings: {e}");
                    }
                }
            }
            AppStateRequest::LoadSettings { path } => {
                match SettingsBundle::load_from_file(path) {
                    Ok(settings_bundle) => {
                        // Cancel any existing processing
                        if self.image_processor.is_processing() {
                            self.image_processor.cancel_current_processing();
                            self.image_processor = ImageProcessor::new();
                        }

                        // Apply loaded settings
                        self.state.settings = settings_bundle.qualetize_settings;
                        self.state.color_correction = settings_bundle.color_correction;
                        self.state.palette_sort_settings = settings_bundle.sort_settings;

                        self.state.request_update_qualetized_image = Some(QualetizeRequest {
                            time: std::time::Instant::now(),
                        });

                        if let Some(input_image) = &self.state.input_image {
                            self.state.color_corrected_image = Some(
                                input_image.color_corrected(&self.state.color_correction, ctx),
                            );
                        } else {
                            self.state.color_corrected_image = None;
                        }

                        // Update tracking
                        self.state.update_color_correction_tracking();

                        log::info!("Settings loaded successfully from: {path}");
                    }
                    Err(e) => {
                        log::error!("Failed to load settings: {e}");
                    }
                }
            }
            AppStateRequest::OpenImageDialog => {
                self.spawn_file_dialog(|dialog| {
                    let path = dialog
                        .add_filter("Image files", &["png", "jpg", "jpeg", "bmp", "tga", "tiff"])
                        .pick_file()?;
                    Some(AppStateRequest::LoadImage {
                        path: path.display().to_string(),
                    })
                });
            }
            AppStateRequest::ExportImageDialog { format, suffix } => {
                let Some(input_path) = self.state.input_path.clone() else {
                    return;
                };
                let default_path = get_export_path(&input_path, format, suffix.as_deref());
                let format = *format;

                self.spawn_file_dialog(move |dialog| {
                    let mut dialog = dialog.add_filter(
                        format!("{} files", format.display_name()),
                        &[format.extension()],
                    );
                    if let Some(file_name) = default_path.file_name() {
                        dialog = dialog.set_file_name(file_name.to_string_lossy().to_string());
                    }
                    if let Some(parent) = default_path.parent() {
                        dialog = dialog.set_directory(parent);
                    }

                    let output_path = dialog.save_file()?.display().to_string();
                    Some(match format {
                        ExportFormat::Png => AppStateRequest::ColorCorrectedPng { output_path },
                        _ => AppStateRequest::QualetizedIndexed {
                            output_path,
                            format,
                        },
                    })
                });
            }
            AppStateRequest::SaveSettingsDialog => {
                self.spawn_file_dialog(|dialog| {
                    let path = settings_file_dialog(dialog)
                        .set_file_name("qualetize_settings.qset")
                        .save_file()?;
                    Some(AppStateRequest::SaveSettings {
                        path: path.display().to_string(),
                    })
                });
            }
            AppStateRequest::LoadSettingsDialog => {
                self.spawn_file_dialog(|dialog| {
                    let path = settings_file_dialog(dialog).pick_file()?;
                    Some(AppStateRequest::LoadSettings {
                        path: path.display().to_string(),
                    })
                });
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

        let col0_is_clear = self.state.settings.col0_is_clear;
        let sorted = self
            .state
            .output_image
            .as_ref()
            .and_then(|image| image.indexed.as_ref())
            .map(|indexed| indexed.sorted(settings.mode, settings.order, col0_is_clear));

        self.state.output_palette_sorted_indexed_image = sorted;
    }
}

impl Drop for QualetizeApp {
    fn drop(&mut self) {
        // Cancel any ongoing processing
        self.image_processor.cancel_current_processing();
        log::debug!("QualetizeApp dropped, resources cleaned up");
    }
}

impl eframe::App for QualetizeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let image_processing =
            self.image_processor.is_processing() || self.state.tile_reduce_processing;

        // apply theme
        self.apply_theme(ctx);

        // Handle drag and drop first
        if !self.state.file_dialog_open.load(Ordering::Relaxed) {
            self.handle_dropped_files(ctx);
        }

        // Check preview completion
        self.check_preview_completion(ctx);
        // Check tile reduce completion
        self.check_tile_reduce_completion(ctx);

        // Update color corrected image if needed
        self.update_color_corrected_image(ctx);

        // Handle settings changes after checking completion
        self.handle_settings_changes();

        // Apply tile reduce changes without re-qualetizing
        self.handle_tile_reduce_changes(ctx);

        self.update_palette_sort_settings();

        // Handle export requests
        self.handle_requests(ctx);

        // Save preferences
        self.state.check_and_save_preferences();

        let mut settings_changed = false;
        let mut tile_reduce_changed = false;

        // Panels have to be declared before the central panel, otherwise the
        // central panel is sized as if they were not there.

        // Top (menu)
        egui::TopBottomPanel::top("menu_panel").show(ctx, |ui| {
            egui::Frame::NONE
                .inner_margin(Margin::symmetric(0, 4))
                .show(ui, |ui| {
                    settings_changed |= draw_header(ui, &mut self.state);
                });
        });

        // Bottom (zoom / export / tile count)
        if self.state.input_image.is_some() {
            egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
                egui::Frame::NONE
                    .inner_margin(Margin::symmetric(0, 4))
                    .show(ui, |ui| {
                        draw_footer(ui, &mut self.state);
                    });
            });
        }

        // Left (settings)
        egui::SidePanel::left("settings_panel")
            .default_width(260.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (settings, tile_reduce) = draw_settings_panel(ui, &mut self.state);
                    settings_changed |= settings;
                    tile_reduce_changed |= tile_reduce;
                });
            });

        // Center (images)
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .inner_margin(0.0)
                    .fill(ctx.style().visuals.window_fill()),
            )
            .show(ctx, |ui| {
                if self.state.input_path.is_none() {
                    draw_main_content(ui);
                } else {
                    draw_image_view(ui, &mut self.state, image_processing);
                }
            });

        if settings_changed {
            self.state.request_update_qualetized_image = Some(QualetizeRequest {
                time: std::time::Instant::now(),
            });
        }
        if tile_reduce_changed {
            self.state.request_update_tile_reduce = true;
        }

        // Repaint drawing while updating image
        if self.image_processor.is_processing()
            || self.state.tile_reduce_processing
            || self.state.request_update_qualetized_image.is_some()
            || self.state.request_update_tile_reduce
        {
            ctx.request_repaint();
        }
    }
}

/// Build the default export path for `input_path`.
///
/// The extension is appended rather than set via [`std::path::Path::with_extension`],
/// which would treat everything after the last dot of the *new* name as an extension
/// and silently truncate it (`hero.idle.png` -> `hero.png`).
fn get_export_path(
    input_path: &str,
    format: &ExportFormat,
    suffix: Option<&str>,
) -> std::path::PathBuf {
    let path = Path::new(input_path);

    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_stem()
        .unwrap_or(std::ffi::OsStr::new("output"))
        .to_string_lossy();
    let extension = format.extension();
    let file_name = match suffix {
        Some(suffix) => format!("{stem}_{suffix}.{extension}"),
        None => format!("{stem}.{extension}"),
    };
    parent.join(file_name)
}

/// Filter and starting directory shared by the settings load/save dialogs.
fn settings_file_dialog(dialog: FileDialog) -> FileDialog {
    let mut dialog = dialog.add_filter(
        "QualetizeGUI Settings",
        &[SettingsBundle::get_settings_file_extension()],
    );
    if let Ok(settings_dir) = SettingsBundle::get_default_settings_dir() {
        dialog = dialog.set_directory(&settings_dir);
    }
    dialog
}

pub struct FileDialogGuard {
    flag: Arc<AtomicBool>,
}

impl FileDialogGuard {
    pub fn new(flag: Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::Relaxed);
        Self { flag }
    }
}

impl Drop for FileDialogGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Relaxed);
        log::debug!("FileDialogGuard dropped - dialog closed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export_path(input: &str, format: ExportFormat, suffix: Option<&str>) -> String {
        get_export_path(input, &format, suffix)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn export_path_appends_suffix_and_extension() {
        assert_eq!(
            export_path(
                "/img/hero.png",
                ExportFormat::PngIndexed,
                Some("qualetized")
            ),
            "/img/hero_qualetized.png"
        );
        assert_eq!(
            export_path("/img/hero.png", ExportFormat::Bmp, Some("qualetized")),
            "/img/hero_qualetized.bmp"
        );
    }

    #[test]
    fn export_path_without_suffix_keeps_stem() {
        assert_eq!(
            export_path("/img/hero.png", ExportFormat::Bmp, None),
            "/img/hero.bmp"
        );
    }

    /// Regression: `with_extension` used to reduce `hero.idle.png` to `hero.png`,
    /// dropping both the suffix and part of the original file name.
    #[test]
    fn export_path_preserves_dots_in_file_name() {
        assert_eq!(
            export_path(
                "/img/hero.idle.png",
                ExportFormat::PngIndexed,
                Some("qualetized")
            ),
            "/img/hero.idle_qualetized.png"
        );
        assert_eq!(
            export_path("/img/tile.v2.bmp", ExportFormat::Bmp, None),
            "/img/tile.v2.bmp"
        );
    }

    #[test]
    fn export_path_handles_missing_extension_and_parent() {
        assert_eq!(
            export_path("hero", ExportFormat::Bmp, Some("qualetized")),
            "hero_qualetized.bmp"
        );
    }
}
