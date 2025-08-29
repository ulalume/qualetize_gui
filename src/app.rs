use crate::image_processing::ImageProcessor;
use crate::types::AppState;
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
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
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
        self.state.zoom = 1.0;
        self.state.pan_offset = egui::Vec2::ZERO;
        self.state.result_message = "Loading image...".to_string();
        self.state.last_settings_change_time = Some(std::time::Instant::now());

        // Cancel any existing processing
        if self.image_processor.is_processing() {
            // Force cancel existing processing
            self.image_processor = ImageProcessor::new();
        }

        match ImageProcessor::load_image_from_path(&path, ctx) {
            Ok(image_data) => {
                self.state.input_path = Some(path.clone());
                self.state.input_image = image_data;
                self.state.result_message = "Image loaded, generating preview...".to_string();

                // Set default output path and name
                self.set_default_output_settings(&path);

                // Check tile size compatibility
                self.check_tile_size_compatibility();

                // Trigger preview generation
                self.state.settings_changed = true;
            }
            Err(e) => {
                self.state.result_message = format!("Image loading error: {}", e);
                self.state.input_path = None;
                self.state.input_image = Default::default();
                self.state.output_path = None;
                self.state.output_name = String::new();
            }
        }
    }

    fn set_default_output_settings(&mut self, input_path: &str) {
        use std::path::Path;

        let path = Path::new(input_path);

        // Set output path to the same directory as input
        if let Some(parent) = path.parent() {
            self.state.output_path = Some(parent.to_string_lossy().to_string());
        } else {
            self.state.output_path = Some(".".to_string());
        }

        // Set output name to [input_name]_qualetized
        if let Some(stem) = path.file_stem() {
            self.state.output_name = format!("{}_qualetized.bmp", stem.to_string_lossy());
        } else {
            self.state.output_name = "output_qualetized.bmp".to_string();
        }
    }

    fn handle_file_selection(&mut self, ctx: &egui::Context) {
        // Check if a new file was selected via dialog but not yet loaded
        if let Some(path) = &self.state.input_path.clone() {
            // Only load if we don't have an existing image or if it's a new path
            let should_load = self.state.input_image.texture.is_none()
                || self.state.result_message == "File selected, loading...";

            if should_load {
                self.load_image_file(path.clone(), ctx);
            }
        }
    }

    fn handle_settings_changes(&mut self) {
        // 設定が変更された場合は常にタイルサイズをチェック
        if self.state.settings_changed {
            self.check_tile_size_compatibility();
        }

        // デバウンス機能：設定変更から一定時間経過後にプレビュー生成を開始
        if self.state.settings_changed && self.state.input_path.is_some() {
            if let Some(last_change_time) = self.state.last_settings_change_time {
                let elapsed = last_change_time.elapsed();
                if elapsed >= self.state.debounce_delay {
                    // 進行中の処理があってもキャンセルして新しい処理を開始
                    self.start_preview_generation();
                }
            }
        }
    }

    fn check_tile_size_compatibility(&mut self) -> bool {
        if self.state.input_image.texture.is_none() {
            self.state.tile_size_warning = false;
            self.state.tile_size_warning_message.clear();
            return true; // 画像がない場合は問題なし
        }

        let image_width = self.state.input_image.size.x as u16;
        let image_height = self.state.input_image.size.y as u16;
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
            // 警告が発生した場合はプレビュー状態をリセット
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

    fn start_preview_generation(&mut self) {
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
                );
                self.state.preview_processing = true;
                self.state.settings_changed = false;
                self.state.last_settings_change_time = None; // リセット
                self.state.result_message = "Generating preview...".to_string();
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
}

impl eframe::App for QualetizeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut settings_changed = false;
        // Handle drag and drop first
        self.handle_dropped_files(ctx);

        // Handle file selection from dialog
        self.handle_file_selection(ctx);

        // Check preview completion
        self.check_preview_completion(ctx);

        // Handle settings changes after checking completion
        self.handle_settings_changes();

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
            .default_width(320.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    settings_changed |= UI::draw_settings_panel(ui, &mut self.state);
                });
            });

        // Main（Images）
        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(0.0))
            .show(ctx, |ui| {
                // Main
                if self.state.input_path.is_none() {
                    UI::draw_main_content(ui, &self.state);
                } else if self.state.tile_size_warning {
                    // 警告がある場合は常に警告表示（プレビューより優先）
                    UI::draw_input_only_view(ui, &mut self.state);
                } else if self.state.preview_ready {
                    UI::draw_image_view(ui, &mut self.state);
                } else if self.state.input_image.texture.is_some() {
                    // Show input image only while processing
                    UI::draw_input_only_view(ui, &mut self.state);
                } else {
                    UI::draw_main_content(ui, &self.state);
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
