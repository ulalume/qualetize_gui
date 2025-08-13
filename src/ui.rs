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
            ui.label("Palettes:")
                .on_hover_text("Set number of palettes available");
            if ui
                .add(egui::DragValue::new(&mut state.settings.n_palettes).range(1..=256))
                .on_hover_text("Number of palettes available")
                .changed()
            {
                settings_changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Colors per Palette:")
                .on_hover_text("Set number of colours per palette\nNote that this value times the number of palettes must be less than or equal to 256.");
            if ui
                .add(egui::DragValue::new(&mut state.settings.n_colors).range(1..=256))
                .on_hover_text("Number of colours per palette")
                .changed()
            {
                settings_changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("RGBA Depth:")
                .on_hover_text("Set RGBA bit depth\nRGBA = 8888 is standard for BMP (24-bit colour + 8-bit alpha)\nFor retro targets, RGBA = 5551 is common");
            if ui
                .text_edit_singleline(&mut state.settings.rgba_depth)
                .on_hover_text("RGBA bit depth (e.g., 8888, 5551, 3331)")
                .changed()
            {
                settings_changed = true;
            }
        });

        if state.show_advanced {
            Frame::NONE
                .fill(Color32::from_rgb(208, 208, 208)) // 内側の余白（margin
                .inner_margin(Margin::same(4))
                .outer_margin(Margin::same(4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Tile Width:")
                            .on_hover_text("Set tile width for processing");
                        if ui
                            .add(egui::DragValue::new(&mut state.settings.tile_width).range(1..=64))
                            .on_hover_text("Width of processing tiles")
                            .changed()
                        {
                            settings_changed = true;
                        }
                        ui.label("Height:")
                            .on_hover_text("Set tile height for processing");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.tile_height).range(1..=64),
                            )
                            .on_hover_text("Height of processing tiles")
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });

                    if ui
                        .checkbox(&mut state.settings.premul_alpha, "Premultiplied Alpha")
                        .on_hover_text("Alpha is pre-multiplied (y/n)\nWhile most formats generally pre-multiply the colours by the alpha value,\n32-bit BMP files generally do not.\nNote that if this option is set, then output colours in the palette will also be pre-multiplied.")
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
                    .on_hover_text("Standard RGB color space")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ycbcr".to_string(),
                        "YCbCr",
                    )
                    .on_hover_text("Luma + Chroma color space")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ycocg".to_string(),
                        "YCoCg",
                    )
                    .on_hover_text("Luma + Co/Cg color space")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "cielab".to_string(),
                        "CIELAB",
                    )
                    .on_hover_text("CIE L*a*b* color space\nNOTE: CIELAB has poor performance in most cases")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ictcp".to_string(),
                        "ICtCp",
                    )
                    .on_hover_text("ITU-R Rec. 2100 ICtCp color space")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "oklab".to_string(),
                        "OkLab",
                    )
                    .on_hover_text("OkLab perceptual color space")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "rgb-psy".to_string(),
                        "RGB + Psyopt",
                    )
                    .on_hover_text("RGB with psychovisual optimization\n(Non-linear light, weighted components)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ycbcr-psy".to_string(),
                        "YCbCr + Psyopt",
                    )
                    .on_hover_text("YCbCr with psychovisual optimization\n(Non-linear luma, weighted chroma)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.color_space,
                        "ycocg-psy".to_string(),
                        "YCoCg + Psyopt",
                    )
                    .on_hover_text("YCoCg with psychovisual optimization\n(Non-linear luma)")
                    .clicked();
                if changed {
                    settings_changed = true;
                }
            })
            .response
            .on_hover_text("Set colourspace\nDifferent colourspaces may give better/worse results depending on the input image,\nand it may be necessary to experiment to find the optimal one.");

        ui.separator();

        // Dithering Settings (moved out of advanced)
        ui.heading("Dithering");
        egui::ComboBox::from_label("Dithering Mode")
            .selected_text(&state.settings.dither_mode)
            .show_ui(ui, |ui| {
                let mut changed = false;
                changed |= ui
                    .selectable_value(&mut state.settings.dither_mode, "none".to_string(), "None")
                    .on_hover_text("No dithering")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "floyd".to_string(),
                        "Floyd-Steinberg",
                    )
                    .on_hover_text("Floyd-Steinberg error diffusion (default level: 0.5)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "atkinson".to_string(),
                        "Atkinson",
                    )
                    .on_hover_text("Atkinson error diffusion (default level: 0.5)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "checker".to_string(),
                        "Checkerboard",
                    )
                    .on_hover_text("Checkerboard dithering (default level: 1.0)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord2".to_string(),
                        "2x2 Ordered",
                    )
                    .on_hover_text("2x2 ordered dithering (default level: 1.0)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord4".to_string(),
                        "4x4 Ordered",
                    )
                    .on_hover_text("4x4 ordered dithering (default level: 1.0)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord8".to_string(),
                        "8x8 Ordered",
                    )
                    .on_hover_text("8x8 ordered dithering (default level: 1.0)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord16".to_string(),
                        "16x16 Ordered",
                    )
                    .on_hover_text("16x16 ordered dithering (default level: 1.0)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord32".to_string(),
                        "32x32 Ordered",
                    )
                    .on_hover_text("32x32 ordered dithering (default level: 1.0)")
                    .clicked();
                changed |= ui
                    .selectable_value(
                        &mut state.settings.dither_mode,
                        "ord64".to_string(),
                        "64x64 Ordered",
                    )
                    .on_hover_text("64x64 ordered dithering (default level: 1.0)")
                    .clicked();
                if changed {
                    settings_changed = true;
                }
            })
            .response
            .on_hover_text("Set dither mode and level for output\nThis can reduce some of the banding artifacts caused when the colours per palette is very small,\nat the expense of added \"noise\".");

        ui.horizontal(|ui| {
            ui.label("Dither Level:")
                .on_hover_text("Dithering intensity level");
            if ui
                .add(egui::Slider::new(
                    &mut state.settings.dither_level,
                    0.0..=2.0,
                ))
                .on_hover_text("Adjust dithering intensity (0.0 = no dithering, 2.0 = maximum)")
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
            .on_hover_text("First colour of every palette is transparent\nNote that this affects both input AND output images.\nTo set transparency in a direct-colour input bitmap, an alpha channel must be used (32-bit input);\ntranslucent alpha values are supported by this tool.")
            .changed()
        {
            settings_changed = true;
        }

        if state.show_advanced {
            Frame::NONE
                .fill(Color32::from_rgb(208, 208, 208)) // 内側の余白（margin
                .inner_margin(Margin::same(4))
                .outer_margin(Margin::same(4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Clear Color:")
                            .on_hover_text("Set colour of transparent pixels.\nSet colour of transparent pixels.\nNote that as long as the RGB values match the clear colour,\nthen the pixel will be made fully transparent, regardless of any alpha information.\nCan be 'none', or a '#RRGGBB' hex triad.");
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
            Frame::NONE
                .fill(Color32::from_rgb(208, 208, 208)) // 内側の余白（margin
                .inner_margin(Margin::same(4))
                .show(ui, |ui| {
                    ui.heading("Clustering");
                    ui.horizontal(|ui| {
                        ui.label("Tile Passes:")
                            .on_hover_text("Set tile cluster passes (0 = default)");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.tile_passes)
                                    .range(0..=1000),
                            )
                            .on_hover_text("Number of tile clustering passes (0 to 1000)")
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Color Passes:")
                            .on_hover_text("Set colour cluster passes (0 = default)\nMost of the processing time will be spent in the loop that clusters the colours together.\nIf processing is taking excessive amounts of time, this option may be adjusted\n(e.g., for 256-colour palettes, set to ~4; for 16-colour palettes, set to 32-64)");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.color_passes)
                                    .range(0..=100),
                            )
                            .on_hover_text("Number of color passes (0 to 100)")
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Split Ratio:")
                            .on_hover_text("Set the cluster splitting ratio\nClusters will stop splitting after splitting all clusters with a total distortion higher than this ratio times the global distortion.\nA value of 1.0 will split all clusters simultaneously (best performance, lower quality),\nwhile a value of 0.0 will split only one cluster at a time (worst performance, best quality).\nA value of -1 will set the ratio automatically based on the number of colours;\nRatio = 1 - 2^(1-k/16).");
                        if ui
                            .add(
                                egui::DragValue::new(&mut state.settings.split_ratio)
                                    .range(-1.0..=1.0),
                            )
                            .on_hover_text("Split Ratio (-1.0 to 1.0)")
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

                        // palettes
                        Self::draw_palettes_overlay(
                            &painter,
                            &response.rect,
                            &state.output_image.palettes,
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
        } else if !state.preview_ready {
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

                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(Rect::from_center_size(
                            painter.clip_rect().center(),
                            Vec2::new(200.0, 100.0),
                        )),
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

    fn draw_palettes_overlay(
        painter: &egui::Painter,
        rect: &Rect,
        palettes: &[Vec<egui::Color32>],
    ) {
        if palettes.is_empty() {
            return;
        }

        let palette_size = 16.0;
        let palette_spacing = 1.0;
        let palette_margin = 8.0;

        // パレット表示領域を右下に配置
        let start_x = rect.max.x - palette_margin;
        let mut current_y = rect.min.y + palette_margin;

        for (_palette_idx, palette) in palettes.iter().enumerate() {
            let palette_width =
                (palette.len() as f32) * (palette_size + palette_spacing) - palette_spacing;

            for (color_idx, &color) in palette.iter().enumerate() {
                let x =
                    start_x - palette_width + (color_idx as f32) * (palette_size + palette_spacing);
                let color_rect = Rect::from_min_size(
                    Pos2::new(x, current_y),
                    Vec2::new(palette_size, palette_size),
                );

                painter.rect_filled(color_rect, 2.0, color);
            }

            current_y += palette_size + palette_spacing; // 次のパレット行へ
        }
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
