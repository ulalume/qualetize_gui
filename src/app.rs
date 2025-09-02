use crate::image_processing::ImageProcessor;
use crate::settings_manager::SettingsBundle;
use crate::types::AppState;
use crate::types::app_state::{AppearanceMode, SettingsRequest};
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
                    self.load_image_file(path.display().to_string(), ctx);
                }
            }
        }
    }

    fn load_image_file(&mut self, path: String, ctx: &egui::Context) {
        // Reset state first
        self.state.preview_ready = false;
        self.state.preview_processing = false;
        self.state.output_image = Default::default();
        self.state.color_corrected_image = Default::default();
        self.state.zoom = 1.0;
        self.state.pan_offset = egui::Vec2::ZERO;
        self.state.result_message = "Loading image...".to_string();
        self.state.last_settings_change_time = Some(std::time::Instant::now());

        // Cancel any existing processing
        if self.image_processor.is_processing() {
            self.image_processor.cancel_current_processing();
            self.image_processor = ImageProcessor::new();
        }

        match ImageProcessor::load_image_from_path(&path, ctx) {
            Ok(image_data) => {
                self.state.input_path = Some(path.clone());
                self.state.input_image = image_data;
                self.state.result_message = "Image loaded, generating preview...".to_string();

                // Invalidate caches when new image is loaded
                self.image_processor.invalidate_color_corrected_cache();
                self.state.invalidate_color_corrected_image();

                // Check tile size compatibility
                self.check_tile_size_compatibility();

                // Trigger preview generation
                self.state.settings_changed = true;
            }
            Err(e) => {
                self.state.result_message = format!("Image loading error: {}", e);
                self.state.input_path = None;
                self.state.input_image = Default::default();
            }
        }
    }

    fn handle_file_selection(&mut self, ctx: &egui::Context) {
        // Check if a new file was selected via dialog but not yet loaded
        if let Some(path) = &self.state.input_path {
            // Only load if we don't have an existing image or if it's a new path
            let should_load = self.state.input_image.texture.is_none()
                || self.state.result_message == "File selected, loading...";

            if should_load {
                self.load_image_file(path.clone(), ctx);
            }
        }
    }

    fn handle_settings_changes(&mut self, ctx: &egui::Context) {
        // Check for color correction changes
        let color_correction_changed = self.state.color_correction_changed();

        // 設定が変更された場合は常にタイルサイズをチェック
        if self.state.settings_changed {
            self.check_tile_size_compatibility();
        }

        // デバウンス機能：設定変更から一定時間経過後にプレビュー生成を開始
        // Color correction changes also trigger preview generation
        if (self.state.settings_changed || color_correction_changed)
            && self.state.input_path.is_some()
        {
            if let Some(last_change_time) = self.state.last_settings_change_time {
                let elapsed = last_change_time.elapsed();
                if elapsed >= self.state.debounce_delay {
                    // 進行中の処理があってもキャンセルして新しい処理を開始
                    self.start_preview_generation(ctx);
                }
            } else if color_correction_changed {
                // If only color correction changed, start immediately
                self.start_preview_generation(ctx);
            }
        }
    }

    fn check_tile_size_compatibility(&mut self) -> bool {
        if self.state.input_image.texture.is_none() {
            self.state.tile_size_warning = false;
            self.state.tile_size_warning_message.clear();
            return true;
        }

        let image_width = self.state.input_image.width as u16;
        let image_height = self.state.input_image.height as u16;
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
            self.state.tile_size_warning_message = format!(
                "Image size ({}×{}) is not divisible by tile size ({}×{}). Qualetize processing cannot proceed.",
                image_width, image_height, tile_width, tile_height
            );
            self.state.preview_ready = false;
            log::warn!(
                "Tile size warning: {}",
                self.state.tile_size_warning_message
            );
            false
        } else {
            self.state.tile_size_warning = false;
            self.state.tile_size_warning_message.clear();
            log::debug!("No warning - sizes are compatible");
            true
        }
    }

    fn start_preview_generation(&mut self, ctx: &egui::Context) {
        // タイルサイズの互換性をチェック
        if !self.check_tile_size_compatibility() {
            self.state.preview_processing = false;
            self.state.settings_changed = false;
            self.state.last_settings_change_time = None;
            self.state.result_message =
                "Cannot process: Image size incompatible with tile size".to_string();
            return;
        }

        if let Some(input_path) = self.state.input_path.clone() {
            if !self.image_processor.is_processing() {
                self.image_processor.start_preview_generation(
                    input_path,
                    self.state.settings.clone(),
                    self.state.color_correction.clone(),
                    ctx,
                );
                self.state.preview_processing = true;
                self.state.settings_changed = false;
                self.state.last_settings_change_time = None; // リセット
                self.state.result_message = "Generating preview...".to_string();

                // Update tracking
                self.state.update_color_correction_tracking();
            }
        }
    }

    fn update_color_corrected_image(&mut self, ctx: &egui::Context) {
        if let Some(input_path) = self.state.input_path.clone() {
            // Check if color correction changed
            if self.state.color_correction_changed() {
                log::debug!("Color correction changed, invalidating cache");
                self.image_processor.invalidate_color_corrected_cache();
                self.state.invalidate_color_corrected_image();
            }

            // Generate color corrected image if needed
            if self.state.needs_color_correction_update() {
                match self.image_processor.get_or_generate_color_corrected_image(
                    &input_path,
                    &self.state.color_correction,
                    ctx,
                ) {
                    Ok(corrected_image) => {
                        self.state.color_corrected_image = corrected_image;
                        self.state.update_color_correction_tracking();
                        log::debug!("Color corrected image updated successfully");
                    }
                    Err(e) => {
                        log::error!("Failed to generate color corrected image: {}", e);
                        self.state.result_message = format!("Color correction failed: {}", e);
                    }
                }
            }
        }
    }

    fn check_preview_completion(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.image_processor.check_preview_complete(ctx) {
            self.state.preview_processing = false;

            match result {
                Ok(image_data) => {
                    self.state.output_image = image_data;
                    self.state.preview_ready = true;
                    self.state.result_message = "Preview complete".to_string();
                }
                Err(e) => {
                    self.state.result_message = format!("Preview error: {}", e);
                    self.state.preview_ready = false;
                }
            }
        }
    }

    fn should_repaint(&self) -> bool {
        self.state.preview_processing
            || self.state.settings_changed
            || self.state.last_settings_change_time.is_some()
            || self.state.tile_size_warning // 警告状態の変更時も再描画
    }

    fn prepare_settings_change(&mut self) {
        self.state.settings_changed = true;
        self.state.last_settings_change_time = Some(std::time::Instant::now());

        // タイルサイズの互換性をチェック
        self.check_tile_size_compatibility();

        // 進行中の処理があれば即座にキャンセル
        if self.image_processor.is_processing() || self.state.preview_processing {
            self.image_processor.cancel_current_processing();
            self.state.preview_processing = false;
            self.state.result_message =
                "Previous processing cancelled, will update soon...".to_string();
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        match self.state.appearance_mode {
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

    fn handle_export_requests(&mut self) {
        if let Some(export_request) = self.state.pending_export_request.take() {
            match export_request {
                crate::types::app_state::ExportRequest::ColorCorrectedPng { output_path } => {
                    // Use ImageData pixels directly
                    if !self.state.color_corrected_image.pixels.is_empty() {
                        let rgba_data = self.state.color_corrected_image.pixels.clone();
                        let width = self.state.color_corrected_image.width.clone();
                        let height = self.state.color_corrected_image.height.clone();
                        std::thread::spawn(move || {
                            match ImageProcessor::save_rgba_image(
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
                crate::types::app_state::ExportRequest::QualetizedIndexed {
                    output_path,
                    format,
                } => {
                    let Some(indexed_data) = self.state.output_image.indexed.clone() else {
                        log::error!("Qualetized export failed: indexed is None");
                        return;
                    };
                    match format {
                        crate::types::ExportFormat::Png => {
                            log::error!("Qualetized export failed: Unexpected format");
                        }
                        crate::types::ExportFormat::Bmp => {
                            match ImageProcessor::save_indexed_bmp(
                                &output_path,
                                &indexed_data,
                                &self.state.output_image.palettes_raw,
                                self.state.output_image.width,
                                self.state.output_image.height,
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
                            match ImageProcessor::save_indexed_png(
                                &output_path,
                                &indexed_data,
                                &self.state.output_image.palettes_raw,
                                self.state.output_image.width,
                                self.state.output_image.height,
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
            }
        }
    }

    fn handle_settings_requests(&mut self) {
        if let Some(settings_request) = self.state.pending_settings_request.take() {
            match settings_request {
                SettingsRequest::Save { path } => {
                    let settings_bundle = SettingsBundle::new(
                        self.state.settings.clone(),
                        self.state.color_correction.clone(),
                    );

                    match settings_bundle.save_to_file(&path) {
                        Ok(()) => {
                            self.state.result_message = format!("Settings saved to: {}", path);
                            log::info!("Settings saved successfully to: {}", path);
                        }
                        Err(e) => {
                            self.state.result_message = format!("Failed to save settings: {}", e);
                            log::error!("Failed to save settings: {}", e);
                        }
                    }
                }
                SettingsRequest::Load { path } => {
                    match SettingsBundle::load_from_file(&path) {
                        Ok(settings_bundle) => {
                            // // Cancel any existing processing
                            if self.image_processor.is_processing() {
                                self.image_processor.cancel_current_processing();
                                self.image_processor = ImageProcessor::new();
                            }
                            // Reset state first
                            self.state.preview_ready = false;
                            self.state.preview_processing = false;
                            self.state.result_message = "Loading image...".to_string();
                            self.state.last_settings_change_time = Some(std::time::Instant::now());

                            // Apply loaded settings
                            self.state.settings = settings_bundle.qualetize_settings;
                            self.state.color_correction = settings_bundle.color_correction;
                            self.state.settings_changed = true;

                            // Invalidate caches when settings change
                            self.image_processor.invalidate_color_corrected_cache();
                            self.state.invalidate_color_corrected_image();

                            // Update tracking
                            self.state.update_color_correction_tracking();

                            self.state.result_message = format!("Settings loaded from: {}", path);
                            log::info!("Settings loaded successfully from: {}", path);
                        }
                        Err(e) => {
                            self.state.result_message = format!("Failed to load settings: {}", e);
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
        let mut settings_changed = false;

        // apply thme
        self.apply_theme(ctx);

        // Handle drag and drop first
        self.handle_dropped_files(ctx);

        // Handle file selection from dialog
        self.handle_file_selection(ctx);

        // Handle settings save/load requests
        self.handle_settings_requests();

        // Check preview completion
        self.check_preview_completion(ctx);

        // Update color corrected image if needed
        self.update_color_corrected_image(ctx);

        // Handle settings changes after checking completion
        self.handle_settings_changes(ctx);

        // Handle export requests
        self.handle_export_requests();

        // Save preferences
        self.state.check_and_save_preferences();

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
                    UI::draw_main_content(ui, &self.state);
                } else {
                    UI::draw_image_view(ui, &mut self.state);
                }

                // Footer
                if self.state.input_image.texture.is_some() {
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
            self.prepare_settings_change();
        }

        // 再描画が必要かチェック
        if self.should_repaint() {
            ctx.request_repaint();
        }
    }
}
