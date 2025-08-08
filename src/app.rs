use crate::image_processing::ImageProcessor;
use crate::types::AppState;
use crate::ui::UI;
use eframe::egui;

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
        self.state.zoom = 0.8;
        self.state.pan_offset = egui::Vec2::ZERO;
        self.state.result_message = "Loading image...".to_string();

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

    fn handle_color_correction_changes(&mut self) {
        if self.state.settings_changed
            && self.state.input_path.is_some()
            && !self.image_processor.is_processing()
        {
            self.start_preview_generation();
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
        if self.state.settings_changed
            && self.state.input_path.is_some()
            && !self.image_processor.is_processing()
        {
            self.start_preview_generation();
        }
    }

    fn start_preview_generation(&mut self) {
        if let Some(input_path) = &self.state.input_path {
            if !self.image_processor.is_processing() {
                self.image_processor.start_preview_generation(
                    input_path.clone(),
                    self.state.settings.clone(),
                    self.state.color_correction.clone(),
                );
                self.state.preview_processing = true;
                self.state.settings_changed = false;
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
        self.state.preview_processing || self.state.settings_changed
    }
}

impl eframe::App for QualetizeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle drag and drop first
        self.handle_dropped_files(ctx);

        // Handle file selection from dialog
        self.handle_file_selection(ctx);

        // Check preview completion
        self.check_preview_completion(ctx);

        // Handle settings changes after checking completion
        self.handle_settings_changes();

        // Handle color correction changes
        self.handle_color_correction_changes();

        // サイドパネル（設定）
        egui::SidePanel::left("settings_panel")
            .default_width(320.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let settings_changed = UI::draw_settings_panel(ui, &mut self.state);
                    if settings_changed && !self.state.preview_processing {
                        self.state.settings_changed = true;
                    }
                });
            });

        // メインパネル（画像表示）
        egui::CentralPanel::default().show(ctx, |ui| {
            // Main content area
            let available_rect = ui.available_rect_before_wrap();
            let footer_height = 40.0;
            let content_rect = egui::Rect::from_min_size(
                available_rect.min,
                egui::Vec2::new(
                    available_rect.width(),
                    available_rect.height() - footer_height,
                ),
            );
            let footer_rect = egui::Rect::from_min_size(
                egui::Pos2::new(available_rect.min.x, available_rect.max.y - footer_height),
                egui::Vec2::new(available_rect.width(), footer_height),
            );

            // Main content
            ui.allocate_ui_at_rect(content_rect, |ui| {
                if self.state.input_path.is_none() {
                    UI::draw_main_content(ui, &self.state);
                } else if self.state.preview_ready {
                    UI::draw_image_view(ui, &mut self.state);
                } else if self.state.input_image.texture.is_some() {
                    // Show input image only while processing
                    UI::draw_input_only_view(ui, &mut self.state);
                } else {
                    UI::draw_main_content(ui, &self.state);
                }
            });

            // Footer
            if self.state.input_image.texture.is_some() {
                ui.allocate_ui_at_rect(footer_rect, |ui| {
                    ui.separator();
                    UI::draw_footer(ui, &mut self.state);
                });
            }
        });

        // 再描画が必要かチェック
        if self.should_repaint() {
            ctx.request_repaint();
        }
    }
}
