use crate::color_correction::{
    ColorProcessor, display_value_to_gamma, format_gamma, format_percentage, gamma_to_display_value,
};
use crate::types::AppState;
use egui::{Color32, Frame, Margin, Pos2, Rect, Vec2};
use rfd::FileDialog;
use std::path::Path;

pub struct UI;

impl UI {
    pub fn draw_settings_panel(ui: &mut egui::Ui, state: &mut AppState) -> bool {
        let mut settings_changed = false;

        ui.add_space(10.0);
        // ファイル選択
        ui.horizontal(|ui| {
            if ui.button("📁 Select Input File").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("Image files", &["png", "jpg", "jpeg", "bmp", "tga", "tiff"])
                    .pick_file()
                {
                    // Signal that we need to load this file
                    // This will be handled in the app update loop
                    let path_str = path.display().to_string();
                    state.input_path = Some(path_str.clone());
                    state.preview_ready = false;
                    state.preview_processing = false;
                    state.output_image = Default::default();
                    state.zoom = 0.8;
                    state.pan_offset = egui::Vec2::ZERO;
                    state.result_message = "File selected, loading...".to_string();

                    // Set default output settings
                    if let Some(parent) = path.parent() {
                        state.output_path = Some(parent.to_string_lossy().to_string());
                    } else {
                        state.output_path = Some(".".to_string());
                    }

                    if let Some(stem) = path.file_stem() {
                        state.output_name = format!("{}_qualetized.bmp", stem.to_string_lossy());
                    } else {
                        state.output_name = "output_qualetized.bmp".to_string();
                    }

                    settings_changed = true;
                }
            }

            if let Some(path) = &state.input_path {
                ui.label(format!(
                    "📄 {}",
                    Path::new(path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                ));
            }
        });

        ui.separator();
        // 高度な設定トグル
        ui.checkbox(&mut state.show_advanced, "🔧 Show Advanced Settings");

        ui.separator();
        // 基本設定
        ui.heading("Basic");

        ui.horizontal(|ui| {
            ui.label("Palettes:");
            if ui
                .add(egui::DragValue::new(&mut state.settings.n_palettes).clamp_range(1..=256))
                .changed()
            {
                settings_changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Colors per Palette:");
            if ui
                .add(egui::DragValue::new(&mut state.settings.n_colors).clamp_range(1..=256))
                .changed()
            {
                settings_changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("RGBA Depth:");
            if ui
                .text_edit_singleline(&mut state.settings.rgba_depth)
                .changed()
            {
                settings_changed = true;
            }
        });

        if state.show_advanced {
            Frame::none()
                .fill(Color32::from_rgb(208, 208, 208)) // 内側の余白（margin
                .inner_margin(Margin::same(4.0))
                .outer_margin(Margin::same(4.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Tile Width:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.tile_width)
                                    .clamp_range(1..=64),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }
                        ui.label("Height:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.tile_height)
                                    .clamp_range(1..=64),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });

                    if ui
                        .checkbox(&mut state.settings.premul_alpha, "Premultiplied Alpha")
                        .changed()
                    {
                        settings_changed = true;
                    }
                });
        }

        ui.separator();

        // 色空間設定
        ui.heading("Color Space");
        egui::ComboBox::from_label("Color Space")
            .selected_text(&state.settings.color_space)
            .show_ui(ui, |ui| {
                let mut changed = false;
                changed |= ui
                    .selectable_value(&mut state.settings.color_space, "srgb".to_string(), "sRGB")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ycbcr".to_string(),
                        "YCbCr",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ycocg".to_string(),
                        "YCoCg",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "cielab".to_string(),
                        "CIELAB",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ictcp".to_string(),
                        "ICtCp",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "oklab".to_string(),
                        "OkLab",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "rgb-psy".to_string(),
                        "RGB + Psyopt",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ycbcr-psy".to_string(),
                        "YCbCr + Psyopt",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ycocg-psy".to_string(),
                        "YCoCg + Psyopt",
                    )
                    .clicked();
                if changed {
                    settings_changed = true;
                }
            });

        ui.separator();

        // Dithering Settings (moved out of advanced)
        ui.heading("Dithering");
        egui::ComboBox::from_label("Dithering Mode")
            .selected_text(&state.settings.dither_mode)
            .show_ui(ui, |ui| {
                let mut changed = false;
                changed |= ui
                    .selectable_value(&mut state.settings.dither_mode, "none".to_string(), "None")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "floyd".to_string(),
                        "Floyd-Steinberg",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "atkinson".to_string(),
                        "Atkinson",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "checker".to_string(),
                        "Checkerboard",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord2".to_string(),
                        "2x2 Ordered",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord4".to_string(),
                        "4x4 Ordered",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord8".to_string(),
                        "8x8 Ordered",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord16".to_string(),
                        "16x16 Ordered",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord32".to_string(),
                        "32x32 Ordered",
                    )
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord64".to_string(),
                        "64x64 Ordered",
                    )
                    .clicked();
                if changed {
                    settings_changed = true;
                }
            });

        ui.horizontal(|ui| {
            ui.label("Dither Level:");
            if ui
                .add(egui::Slider::new(
                    &mut state.settings.dither_level,
                    0.0..=2.0,
                ))
                .changed()
            {
                settings_changed = true;
            }
        });

        ui.separator();

        // Transparency Settings (moved out of advanced)
        ui.heading("Transparency");
        if ui
            .checkbox(
                &mut state.settings.col0_is_clear,
                "First Color is Transparent",
            )
            .changed()
        {
            settings_changed = true;
        }

        if state.show_advanced {
            Frame::none()
                .fill(Color32::from_rgb(208, 208, 208)) // 内側の余白（margin
                .inner_margin(Margin::same(4.0))
                .outer_margin(Margin::same(4.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Clear Color:");
                        if ui
                            .text_edit_singleline(&mut state.settings.clear_color)
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });
                });
        }

        ui.separator();

        if state.show_advanced {
            // Clustering Settings (moved to advanced)
            Frame::none()
                .fill(Color32::from_rgb(208, 208, 208)) // 内側の余白（margin
                .inner_margin(Margin::same(4.0))
                .show(ui, |ui| {
                    ui.heading("Clustering");
                    ui.horizontal(|ui| {
                        ui.label("Tile Passes:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.tile_passes)
                                    .clamp_range(0..=10000),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Color Passes:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.color_passes)
                                    .clamp_range(0..=1000),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Split Ratio:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.split_ratio)
                                    .clamp_range(-1.0..=1.0),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });
                });

            ui.separator();
        }

        // カラー補正設定
        ui.heading("Color Correction");

        egui::Grid::new("color_correction_grid")
            .num_columns(3)
            .spacing([4.0, 6.0])
            .show(ui, |ui| {
                // Calculate available slider width
                let available_width = ui.available_width();
                let slider_width = (available_width * 0.6).max(120.0);

                // Brightness
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Brightness:");
                });
                if ui
                    .add_sized(
                        [slider_width, 24.0],
                        egui::Slider::new(&mut state.color_correction.brightness, -1.0..=1.0)
                            .show_value(false),
                    )
                    .changed()
                {
                    settings_changed = true;
                }
                ui.label(format_percentage(state.color_correction.brightness));
                ui.end_row();

                // Contrast
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Contrast:");
                });
                if ui
                    .add_sized(
                        [slider_width, 24.0],
                        egui::Slider::new(&mut state.color_correction.contrast, 0.0..=2.0)
                            .show_value(false),
                    )
                    .changed()
                {
                    settings_changed = true;
                }
                ui.label(format!("{:.2}", state.color_correction.contrast));
                ui.end_row();

                // Gamma
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Gamma:");
                });
                let mut gamma_display = gamma_to_display_value(state.color_correction.gamma);
                if ui
                    .add_sized(
                        [slider_width, 24.0],
                        egui::Slider::new(&mut gamma_display, -100.0..=100.0).show_value(false),
                    )
                    .changed()
                {
                    state.color_correction.gamma = display_value_to_gamma(gamma_display);
                    settings_changed = true;
                }
                ui.label(format_gamma(state.color_correction.gamma));
                ui.end_row();

                // Saturation
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Saturation:");
                });
                if ui
                    .add_sized(
                        [slider_width, 24.0],
                        egui::Slider::new(&mut state.color_correction.saturation, 0.0..=2.0)
                            .show_value(false),
                    )
                    .changed()
                {
                    settings_changed = true;
                }
                ui.label(format!("{:.2}", state.color_correction.saturation));
                ui.end_row();

                // Hue Shift
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Hue Shift:");
                });
                if ui
                    .add_sized(
                        [slider_width, 24.0],
                        egui::Slider::new(&mut state.color_correction.hue_shift, -180.0..=180.0)
                            .show_value(false),
                    )
                    .changed()
                {
                    settings_changed = true;
                }
                ui.label(format!("{:.0}°", state.color_correction.hue_shift));
                ui.end_row();

                // Shadows
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Shadows:");
                });
                if ui
                    .add_sized(
                        [slider_width, 24.0],
                        egui::Slider::new(&mut state.color_correction.shadows, -1.0..=1.0)
                            .show_value(false),
                    )
                    .changed()
                {
                    settings_changed = true;
                }
                ui.label(format_percentage(state.color_correction.shadows));
                ui.end_row();

                // Highlights
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Highlights:");
                });
                if ui
                    .add_sized(
                        [slider_width, 24.0],
                        egui::Slider::new(&mut state.color_correction.highlights, -1.0..=1.0)
                            .show_value(false),
                    )
                    .changed()
                {
                    settings_changed = true;
                }
                ui.label(format_percentage(state.color_correction.highlights));
                ui.end_row();
            });

        // カラー補正プリセット
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let button_width = (ui.available_width() - 24.0) / 4.0;

            if ui
                .add_sized([button_width, 24.0], egui::Button::new("🔄 Reset"))
                .clicked()
            {
                state.color_correction = ColorProcessor::reset_corrections();
                settings_changed = true;
            }

            if ui
                .add_sized([button_width, 24.0], egui::Button::new("✨ Vibrant"))
                .clicked()
            {
                state.color_correction = ColorProcessor::preset_vibrant();
                settings_changed = true;
            }

            if ui
                .add_sized([button_width, 24.0], egui::Button::new("🌅 Warm"))
                .clicked()
            {
                state.color_correction = ColorProcessor::preset_retro_warm();
                settings_changed = true;
            }

            if ui
                .add_sized([button_width, 24.0], egui::Button::new("❄️ Cool"))
                .clicked()
            {
                state.color_correction = ColorProcessor::preset_retro_cool();
                settings_changed = true;
            }
        });

        // Status display
        ui.heading("Status");
        if state.preview_processing {
            ui.label("🔄 Generating preview...");
        } else if !state.result_message.is_empty() {
            ui.label(&state.result_message);
        }

        // Debug information
        ui.collapsing("🔍 Debug Info", |ui| {
            ui.label(format!("Input path: {:?}", state.input_path.is_some()));
            ui.label(format!(
                "Input texture: {:?}",
                state.input_image.texture.is_some()
            ));
            ui.label(format!(
                "Output texture: {:?}",
                state.output_image.texture.is_some()
            ));
            ui.label(format!("Preview ready: {}", state.preview_ready));
            ui.label(format!("Preview processing: {}", state.preview_processing));
            ui.label(format!("Settings changed: {}", state.settings_changed));
        });

        settings_changed
    }

    pub fn draw_image_view(ui: &mut egui::Ui, state: &mut AppState) {
        let available_size = ui.available_size();

        // Split the available area into two halves
        let split_x = available_size.x / 2.0;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);
            // Left panel - Input image
            ui.allocate_ui_with_layout(
                Vec2::new(split_x, available_size.y),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    let (response, painter) = ui.allocate_painter(
                        Vec2::new(split_x, available_size.y),
                        egui::Sense::click_and_drag(),
                    );

                    // Draw background
                    painter.rect_filled(response.rect, 0.0, Color32::from_gray(64));

                    if let Some(input_texture) = state.input_image.texture.as_ref() {
                        let original_size = state.input_image.size;
                        let available_rect = response.rect;

                        // Scale to fit while maintaining aspect ratio
                        let scale_x = available_rect.width() / original_size.x;
                        let scale_y = available_rect.height() / original_size.y;
                        let base_scale = scale_x.min(scale_y);
                        let scale = (base_scale * state.zoom).min(10.0);

                        let display_size = original_size * scale;
                        let view_center = response.rect.center() + state.pan_offset;

                        let image_rect = Rect::from_center_size(view_center, display_size);

                        painter.image(
                            input_texture.id(),
                            image_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }

                    // Handle mouse operations for left panel
                    if response.dragged() {
                        state.pan_offset += response.drag_delta();
                    }

                    if response.hovered() {
                        let scroll_delta = ui.ctx().input(|i| i.raw_scroll_delta.y);
                        if scroll_delta != 0.0 {
                            let zoom_factor = 1.0 + scroll_delta * 0.001;
                            state.zoom = (state.zoom * zoom_factor).clamp(0.1, 10.0);
                        }
                    }
                },
            );

            // Right panel - Output image
            ui.allocate_ui_with_layout(
                Vec2::new(split_x, available_size.y),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    let (response, painter) = ui.allocate_painter(
                        Vec2::new(split_x, available_size.y),
                        egui::Sense::click_and_drag(),
                    );

                    // Draw background
                    painter.rect_filled(response.rect, 0.0, Color32::from_gray(64));

                    if let Some(output_texture) = state.output_image.texture.as_ref() {
                        let original_size = state.input_image.size; // Use input size for consistency
                        let available_rect = response.rect;

                        // Use same scaling as left panel
                        let scale_x = available_rect.width() / original_size.x;
                        let scale_y = available_rect.height() / original_size.y;
                        let base_scale = scale_x.min(scale_y);
                        let scale = (base_scale * state.zoom).min(10.0);

                        let display_size = original_size * scale;
                        let view_center = response.rect.center() + state.pan_offset;

                        let image_rect = Rect::from_center_size(view_center, display_size);

                        painter.image(
                            output_texture.id(),
                            image_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }

                    // Handle mouse operations for right panel (sync with left)
                    if response.dragged() {
                        state.pan_offset += response.drag_delta();
                    }

                    if response.hovered() {
                        let scroll_delta = ui.ctx().input(|i| i.raw_scroll_delta.y);
                        if scroll_delta != 0.0 {
                            let zoom_factor = 1.0 + scroll_delta * 0.001;
                            state.zoom = (state.zoom * zoom_factor).clamp(0.1, 10.0);
                        }
                    }
                },
            );
        });
    }

    pub fn draw_main_content(ui: &mut egui::Ui, state: &AppState) {
        if state.input_path.is_none() {
            ui.centered_and_justified(|ui| {
                ui.heading("📁 Drop an image file here or use 'Select Input File'");
            });
        } else if state.preview_ready {
            // Show operation hints
            ui.allocate_ui_at_rect(
                Rect::from_min_size(
                    ui.min_rect().min + Vec2::new(10.0, 10.0),
                    Vec2::new(200.0, 60.0),
                ),
                |ui| {
                    ui.label("💡 Left: Original | Right: Processed");
                    ui.label("🖱️ Drag to pan, scroll to zoom");
                },
            );
        } else {
            ui.centered_and_justified(|ui| {
                ui.heading("⏳ Processing...");
            });
        }
    }

    pub fn draw_input_only_view(ui: &mut egui::Ui, state: &mut AppState) {
        let available_size = ui.available_size();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);
            let split_x = available_size.x / 2.0;

            // Left panel - Input image
            ui.allocate_ui_with_layout(
                Vec2::new(split_x, available_size.y),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    let (response, painter) = ui.allocate_painter(
                        Vec2::new(split_x, available_size.y),
                        egui::Sense::click_and_drag(),
                    );

                    // Draw background
                    painter.rect_filled(response.rect, 0.0, Color32::from_gray(64));

                    if let Some(input_texture) = state.input_image.texture.as_ref() {
                        let original_size = state.input_image.size;
                        let available_rect = response.rect;

                        // Scale to fit while maintaining aspect ratio
                        let scale_x = available_rect.width() / original_size.x;
                        let scale_y = available_rect.height() / original_size.y;
                        let base_scale = scale_x.min(scale_y);
                        let scale = (base_scale * state.zoom).min(10.0);

                        let display_size = original_size * scale;
                        let view_center = response.rect.center() + state.pan_offset;

                        let image_rect = Rect::from_center_size(view_center, display_size);

                        painter.image(
                            input_texture.id(),
                            image_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }

                    if response.dragged() {
                        state.pan_offset += response.drag_delta();
                    }

                    if response.hovered() {
                        let scroll_delta = ui.ctx().input(|i| i.raw_scroll_delta.y);
                        if scroll_delta != 0.0 {
                            let zoom_factor = 1.0 + scroll_delta * 0.001;
                            state.zoom = (state.zoom * zoom_factor).clamp(0.1, 10.0);
                        }
                    }
                },
            );

            // Right panel - Processing message
            ui.allocate_ui_with_layout(
                Vec2::new(split_x, available_size.y),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    let (_, painter) = ui.allocate_painter(
                        Vec2::new(split_x, available_size.y),
                        egui::Sense::hover(),
                    );

                    // Draw background
                    painter.rect_filled(painter.clip_rect(), 0.0, Color32::from_gray(64));

                    // Show processing status in the center
                    ui.allocate_ui_at_rect(
                        Rect::from_center_size(
                            painter.clip_rect().center(),
                            Vec2::new(200.0, 100.0),
                        ),
                        |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.label("⏳");
                                ui.label("Processing...");
                                if !state.result_message.is_empty() {
                                    ui.label(&state.result_message);
                                }
                            });
                        },
                    );
                },
            );
        });
    }

    pub fn draw_footer(ui: &mut egui::Ui, state: &mut AppState) -> bool {
        let export_clicked = false;

        ui.horizontal(|ui| {
            // Left: Reset View button
            if ui.button("🔄 Reset View").clicked() {
                state.zoom = 0.8;
                state.pan_offset = Vec2::ZERO;
            }

            ui.label(format!("🔍 Zoom: {:.1}x", state.zoom));

            // Center: Operation hints
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.separator();
                ui.label("💡 Left: Original | Right: Processed");
                ui.separator();
                ui.label("🖱️ Drag to pan, scroll to zoom");
                ui.separator();
            });

            // Right: Export Image button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("💾 Export Image").clicked() && state.preview_ready {
                    Self::show_export_dialog(state);
                }
            });
        });

        export_clicked
    }

    fn show_export_dialog(state: &AppState) {
        if let Some(input_path) = &state.input_path {
            // Use default output settings if available
            let default_path = if let (Some(output_path), output_name) =
                (&state.output_path, &state.output_name)
            {
                if !output_name.is_empty() {
                    Some(Path::new(output_path).join(output_name))
                } else {
                    None
                }
            } else {
                None
            };

            let mut dialog = FileDialog::new().add_filter("BMP files", &["bmp"]);

            // Set default filename if we have one
            if let Some(default_path) = default_path {
                dialog = dialog.set_file_name(
                    default_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                );
                if let Some(parent) = default_path.parent() {
                    dialog = dialog.set_directory(parent);
                }
            }

            if let Some(output_path) = dialog.save_file() {
                let settings = state.settings.clone();
                let color_correction = state.color_correction.clone();
                let input_path = input_path.clone();
                let output_path = output_path.display().to_string();

                std::thread::spawn(move || {
                    let _ = crate::image_processing::ImageProcessor::export_image(
                        input_path,
                        output_path,
                        settings,
                        color_correction,
                    );
                });
            }
        }
    }
}
