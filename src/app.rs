use crate::exporter::{save_indexed_bmp, save_indexed_png, save_rgba_image};
use crate::image_processor::ImageProcessor;
use crate::settings_manager::SettingsBundle;
use crate::types::AppState;
use crate::types::ImageData;
use crate::types::app_state::{AppStateRequest, AppearanceMode, QualetizeRequest};
use crate::types::image::SortMode;
use crate::ui::UI;
use display_icc::get_primary_display_profile_data;
use eframe::egui;
use egui::Margin;

pub struct QualetizeApp {
    state: AppState,
    image_processor: ImageProcessor,
    display_icc_profile: Option<Vec<u8>>,
}

impl Default for QualetizeApp {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            image_processor: ImageProcessor::new(),
            display_icc_profile: None,
        }
    }
}

impl QualetizeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        crate::ui::styles::init_styles(ctx);

        let mut app = Self::default();
        app.display_icc_profile = get_primary_display_profile_data().ok();

        app
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty()
            && let Some(dropped_file) = dropped_files.first()
            && let Some(path) = &dropped_file.path
        {
            self.state.pending_app_state_request = Some(AppStateRequest::LoadImage {
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

        match ImageData::load(&path, ctx, &self.display_icc_profile) {
            Ok(image_data) => {
                self.state.input_path = Some(path.clone());
                self.state.input_image = Some(image_data);
                self.state.color_corrected_image = None;

                // Check tile size compatibility
                self.check_tile_size_compatibility();

                self.state.zoom = 1.0;
                self.state.pan_offset = egui::Vec2::ZERO;
            }
            Err(e) => {
                log::error!("File load Error {e}");
                self.state.input_path = None;
                self.state.input_image = Default::default();
                self.state.color_corrected_image = None;
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
        self.image_processor
            .start_qualetize(color_corrected_image, self.state.settings.clone());
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
            self.state.output_palette_sorted_indexed_image = None;
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
        if let Some(result) = self
            .image_processor
            .check_preview_complete(ctx, &self.display_icc_profile)
        {
            match result {
                Ok(image_data) => {
                    self.state.output_image = Some(image_data);
                    self.state.output_palette_sorted_indexed_image = None;
                }
                Err(e) => {
                    log::error!("Failed to generate preview image: {e}");
                    self.state.output_image = None;
                    self.state.output_palette_sorted_indexed_image = None;
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
            let color_corrected_image =
                image.color_corrected(&self.state.color_correction, ctx, &self.display_icc_profile);
            self.state.color_corrected_image = Some(color_corrected_image);
        }
    }

    fn handle_requests(&mut self, ctx: &egui::Context) {
        if let Some(app_state_request) = self.state.pending_app_state_request.take() {
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
                    if let Some(color_corrected_image) = &self.state.color_corrected_image {
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
                    } else {
                        log::error!("No color corrected image data available in memory");
                    }
                }
                AppStateRequest::QualetizedIndexed {
                    output_path,
                    format,
                } => {
                    let Some(output_image) = &self.state.output_image else {
                        log::error!("Qualetized export failed: output image is None");
                        return;
                    };

                    let indexed = if self.state.output_palette_sorted_indexed_image.is_some() {
                        &self.state.output_palette_sorted_indexed_image
                    } else if let Some(image) = &self.state.output_image {
                        &image.indexed
                    } else {
                        &None
                    };

                    let Some(indexed) = indexed else {
                        return;
                    };

                    match format {
                        crate::types::ExportFormat::Png => {
                            log::error!("Qualetized export failed: Unexpected format");
                        }
                        crate::types::ExportFormat::Bmp => {
                            match save_indexed_bmp(
                                &output_path,
                                &indexed.indexed_pixels,
                                &indexed.palettes,
                                output_image.width,
                                output_image.height,
                            ) {
                                Ok(()) => {
                                    log::info!(
                                        "Qualetized indexed BMP export completed successfully"
                                    );
                                }
                                Err(e) => {
                                    log::error!("Qualetized indexed export failed: {e}");
                                }
                            }
                        }
                        crate::types::ExportFormat::PngIndexed => {
                            match save_indexed_png(
                                &output_path,
                                &indexed.indexed_pixels,
                                &indexed.palettes,
                                output_image.width,
                                output_image.height,
                            ) {
                                Ok(()) => {
                                    log::info!(
                                        "Qualetized indexed PNG export completed successfully"
                                    );
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
                        self.state.palette_sort_settings.clone(),
                    );

                    match settings_bundle.save_to_file(&path) {
                        Ok(()) => {
                            log::info!("Settings saved successfully to: {path}");
                        }
                        Err(e) => {
                            log::error!("Failed to save settings: {e}");
                        }
                    }
                }
                AppStateRequest::LoadSettings { path } => {
                    match SettingsBundle::load_from_file(&path) {
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
                                self.state.color_corrected_image =
                                    Some(input_image.color_corrected(
                                        &self.state.color_correction,
                                        ctx,
                                        &self.display_icc_profile,
                                    ));
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
            }
        }
    }

    fn update_palette_sort_settings(&mut self) {
        if self.state.output_palette_sorted_indexed_image.is_some()
            && !self.state.palette_sort_settings_changed()
        {
            return;
        }

        // Extract the indexed image data first to avoid borrowing conflicts
        let indexed_image = if let Some(output_image) = &self.state.output_image {
            if let Some(indexed) = &output_image.indexed {
                indexed.clone()
            } else {
                return;
            }
        } else {
            return;
        };

        let palette_sort_settings = self.state.palette_sort_settings.clone();

        self.state.update_palette_sort_settings_tracking();

        if palette_sort_settings.mode == SortMode::None {
            self.state.output_palette_sorted_indexed_image = None;
        } else {
            self.state.output_palette_sorted_indexed_image = Some(indexed_image.sorted(
                palette_sort_settings.mode,
                palette_sort_settings.order,
                self.state.settings.col0_is_clear,
            ));
        }
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
        let image_processing = self.image_processor.is_processing();

        // apply theme
        self.apply_theme(ctx);

        // Handle drag and drop first
        self.handle_dropped_files(ctx);

        // Check preview completion
        self.check_preview_completion(ctx);

        // Update color corrected image if needed
        self.update_color_corrected_image(ctx);

        // Handle settings changes after checking completion
        self.handle_settings_changes();

        self.update_palette_sort_settings();

        // Handle export requests
        self.handle_requests(ctx);

        // Save preferences
        self.state.check_and_save_preferences();

        let mut settings_changed = false;
        // Top（Menu）
        egui::TopBottomPanel::top("menu_panel").show(ctx, |ui| {
            egui::Frame::NONE
                .inner_margin(Margin::symmetric(0, 4))
                .show(ui, |ui| {
                    settings_changed |= UI::draw_header(ui, &mut self.state);
                });
        });

        // Side（Settings）
        egui::SidePanel::left("settings_panel")
            .default_width(260.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    settings_changed |= UI::draw_settings_panel(ui, &mut self.state);
                });
            });

        // Main（Images）
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .inner_margin(0.0)
                    .fill(ctx.style().visuals.window_fill()),
            )
            .show(ctx, |ui| {
                // Main
                if self.state.input_path.is_none() {
                    UI::draw_main_content(ui);
                } else {
                    UI::draw_image_view(ui, &mut self.state, image_processing);
                }

                // Footer
                if self.state.input_image.is_some() {
                    egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
                        egui::Frame::NONE
                            .inner_margin(Margin::symmetric(0, 4))
                            .show(ui, |ui| {
                                UI::draw_footer(ui, &mut self.state);
                            });
                    });
                }
            });

        if settings_changed {
            self.state.request_update_qualetized_image = Some(QualetizeRequest {
                time: std::time::Instant::now(),
            });
        }

        // Repaint drawing while updating image
        if self.image_processor.is_processing()
            || self.state.request_update_qualetized_image.is_some()
        {
            ctx.request_repaint();
        }
    }
}
