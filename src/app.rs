use crate::exporter::{save_indexed_bmp, save_indexed_png, save_rgba_image};
use crate::image_processing::ImageProcessor;
use crate::settings_manager::SettingsBundle;
use crate::types::AppState;
use crate::types::ImageData;
use crate::types::app_state::{AppStateRequest, AppearanceMode, QualetizeRequest};
use crate::ui::UI;
use eframe::egui;
use egui::Margin;

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
        if !dropped_files.is_empty() {
            if let Some(dropped_file) = dropped_files.first() {
                if let Some(path) = &dropped_file.path {
                    self.state.pending_app_state_request = Some(AppStateRequest::LoadImage {
                        path: path.display().to_string(),
                    });
                }
            }
        }
    }

    fn load_image_file(&mut self, path: String, ctx: &egui::Context) {
        // Cancel any existing processing
        if self.image_processor.is_processing() {
            self.image_processor.cancel_current_processing();
            self.image_processor = ImageProcessor::new();
        }

        match ImageData::load(&path, ctx) {
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
                log::error!("File load Error {}", e);
                self.state.input_path = None;
                self.state.input_image = Default::default();
                self.state.color_corrected_image = None;
            }
        }
    }

    fn handle_settings_changes(&mut self, ctx: &egui::Context) {
        if !self.check_tile_size_compatibility() || self.state.color_corrected_image.is_none() {
            return;
        }
        // Debounce functionality: start preview generation after a certain delay from settings change
        if let Some(request) = &self.state.request_update_qualetized_image {
            let elapsed = request.time.elapsed();
            ctx.request_repaint();
            println!(
                "elapsed: {:?}, debounce_delay: {:?}",
                elapsed, self.state.debounce_delay
            );
            if elapsed >= self.state.debounce_delay {
                self.start_preview_generation();
            }
        }
    }

    fn check_tile_size_compatibility(&mut self) -> bool {
        let Some(input_image) = &self.state.input_image else {
            return true;
        };

        let image_width = input_image.width as u16;
        let image_height = input_image.height as u16;
        let tile_width = self.state.settings.tile_width;
        let tile_height = self.state.settings.tile_height;

        let width_divisible = image_width % tile_width == 0;
        let height_divisible = image_height % tile_height == 0;

        log::debug!(
            "Tile size check: image {}×{}, tile {}×{}, divisible: width={}, height={}",
            image_width,
            image_height,
            tile_width,
            tile_height,
            width_divisible,
            height_divisible
        );

        if !width_divisible || !height_divisible {
            self.state.tile_size_warning = true;
            self.state.output_image = None;
            log::warn!("Tile size warning");
            false
        } else {
            self.state.tile_size_warning = false;
            log::debug!("No warning - sizes are compatible");
            true
        }
    }

    fn start_preview_generation(&mut self) {
        if let Some(color_corrected_image) = &self.state.color_corrected_image {
            if !self.image_processor.is_processing() {
                self.state.request_update_qualetized_image = None;
                self.image_processor
                    .start_qualetize(color_corrected_image, self.state.settings.clone());
            }
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
                    self.state.output_image = Some(image_data);
                }
                Err(e) => {
                    log::error!("Failed to generate preview image: {}", e);
                    self.state.output_image = None;
                }
            }
        }
    }
    fn apply_theme(&self, ctx: &egui::Context) {
        match self.state.preferences.appearance_mode {
            AppearanceMode::Dark => ctx.set_visuals(egui::Visuals::dark()),
            AppearanceMode::Light => ctx.set_visuals(egui::Visuals::light()),
            AppearanceMode::System => {
                if let Some(theme) = ctx.system_theme() {
                    match theme {
                        egui::Theme::Dark => ctx.set_visuals(egui::Visuals::dark()),
                        egui::Theme::Light => ctx.set_visuals(egui::Visuals::light()),
                    }
                } else {
                    ctx.set_visuals(egui::Visuals::dark());
                }
            }
        }
    }

    fn apply_color_correct_image(&mut self, ctx: &egui::Context) {
        if let Some(image) = &self.state.input_image {
            let color_corrected_image = image.color_corrected(&self.state.color_correction, ctx);
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
                        let width = color_corrected_image.width.clone();
                        let height = color_corrected_image.height.clone();
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
                                    log::error!("Color corrected PNG export failed: {}", e);
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
                    let Some(indexed_data) = &output_image.indexed else {
                        log::error!("Qualetized export failed: indexed is None");
                        return;
                    };
                    match format {
                        crate::types::ExportFormat::Png => {
                            log::error!("Qualetized export failed: Unexpected format");
                        }
                        crate::types::ExportFormat::Bmp => {
                            match save_indexed_bmp(
                                &output_path,
                                &indexed_data.indexed_pixels,
                                &indexed_data.palettes,
                                output_image.width,
                                output_image.height,
                            ) {
                                Ok(()) => {
                                    log::info!(
                                        "Qualetized indexed BMP export completed successfully"
                                    );
                                }
                                Err(e) => {
                                    log::error!("Qualetized indexed export failed: {}", e);
                                }
                            }
                        }
                        crate::types::ExportFormat::PngIndexed => {
                            match save_indexed_png(
                                &output_path,
                                &indexed_data.indexed_pixels,
                                &indexed_data.palettes,
                                output_image.width,
                                output_image.height,
                            ) {
                                Ok(()) => {
                                    log::info!(
                                        "Qualetized indexed PNG export completed successfully"
                                    );
                                }
                                Err(e) => {
                                    log::error!("Qualetized indexed export failed: {}", e);
                                }
                            }
                        }
                    }
                }
                AppStateRequest::SaveSettings { path } => {
                    let settings_bundle = SettingsBundle::new(
                        self.state.settings.clone(),
                        self.state.color_correction.clone(),
                    );

                    match settings_bundle.save_to_file(&path) {
                        Ok(()) => {
                            log::info!("Settings saved successfully to: {}", path);
                        }
                        Err(e) => {
                            log::error!("Failed to save settings: {}", e);
                        }
                    }
                }
                AppStateRequest::LoadSettings { path } => {
                    match SettingsBundle::load_from_file(&path) {
                        Ok(settings_bundle) => {
                            // // Cancel any existing processing
                            if self.image_processor.is_processing() {
                                self.image_processor.cancel_current_processing();
                                self.image_processor = ImageProcessor::new();
                            }

                            // Apply loaded settings
                            self.state.settings = settings_bundle.qualetize_settings;
                            self.state.color_correction = settings_bundle.color_correction;
                            self.state.request_update_qualetized_image = Some(QualetizeRequest {
                                time: std::time::Instant::now(),
                            });

                            // Invalidate caches when settings change
                            self.state.color_corrected_image = None;

                            // Update tracking
                            self.state.update_color_correction_tracking();

                            log::info!("Settings loaded successfully from: {}", path);
                        }
                        Err(e) => {
                            log::error!("Failed to load settings: {}", e);
                        }
                    }
                }
            }
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
        self.handle_settings_changes(ctx);

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
            println!("settings_changed: {}", settings_changed);
            self.state.request_update_qualetized_image = Some(QualetizeRequest {
                time: std::time::Instant::now(),
            });
        }
    }
}
