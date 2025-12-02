use super::styles::UiMarginExt;
use crate::color_processor::{
    display_value_to_gamma, format_gamma, format_percentage, gamma_to_display_value,
};
use crate::types::qualetize::validate_0_255_array;
use crate::types::{
    AppState, ClearColor, ColorSpace, DitherMode,
    color_correction::ColorCorrection,
    image::{SortMode, SortOrder},
};
use egui::Color32;
use regex::Regex;

pub fn draw_settings_panel(ui: &mut egui::Ui, state: &mut AppState) -> (bool, bool) {
    let mut settings_changed = false;
    let mut tile_reduce_changed = false;

    // Basic settings
    settings_changed |= draw_basic_settings(ui, state);

    settings_changed |= draw_transparency_settings(ui, state);

    ui.separator();

    // Color space settings
    settings_changed |= draw_color_space_settings(ui, state);

    ui.separator();

    // Dithering settings
    settings_changed |= draw_dithering_settings(ui, state);

    ui.separator();

    // Advanced clustering settings (if enabled)
    if state.preferences.show_advanced {
        settings_changed |= draw_advanced_settings(ui, state);
        ui.separator();
    }

    // Color correction settings
    settings_changed |= draw_color_correction_settings(ui, state);
    ui.separator();

    tile_reduce_changed |= draw_tile_reduce_settings(ui, state);
    ui.separator();
    draw_palette_sort_settings(ui, state);

    if state.preferences.show_debug_info {
        // Debug information display
        ui.separator();
        draw_status_section(ui, state);
    }
    (settings_changed, tile_reduce_changed)
}

fn draw_advanced_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.heading("Qualetize Advanced");
    ui.add_space(4.0);

    settings_changed |= draw_tile_settings(ui, state);

    ui.separator();
    settings_changed |= draw_depth_settings(ui, state);

    ui.separator();

    let mut has_clear_color = matches!(state.settings.clear_color, ClearColor::Rgb(_, _, _));
    if ui
        .checkbox(&mut has_clear_color, "Set Color of Transparent Pixels")
        .on_hover_text("Note that as long as the RGB values match the clear color,\nthen the pixel will be made fully transparent, regardless of any alpha information.")
        .changed()
    {
        if has_clear_color {
            state.settings.clear_color = ClearColor::Rgb(255, 0, 255); // Default magenta
        } else {
            state.settings.clear_color = ClearColor::None;
        }
        settings_changed = true;
    }

    if has_clear_color
        && let ClearColor::Rgb(ref mut r, ref mut g, ref mut b) = state.settings.clear_color
    {
        ui.horizontal(|ui| {
            ui.add_space(16.0); // Indent the color picker

            let mut color_array = [*r, *g, *b];
            if ui.color_edit_button_srgb(&mut color_array).changed() {
                *r = color_array[0];
                *g = color_array[1];
                *b = color_array[2];
                settings_changed = true;
            }
            if ui.button("Use Top-Left Pixel Color").clicked()
                && let Some(color_corrected_image) = &state.color_corrected_image
                && let Some(color) = color_corrected_image.get_top_left_pixel_color()
            {
                *r = color.r();
                *g = color.g();
                *b = color.b();
                settings_changed = true;
            }
            ui.label(format!("#{:02X}{:02X}{:02X}", *r, *g, *b));
        });
    }

    ui.separator();

    settings_changed |= draw_clustering_settings(ui, state);

    ui.separator();
    if ui
        .checkbox(&mut state.settings.premul_alpha, "Premultiplied Alpha")
        .on_hover_text("Alpha is pre-multiplied (y/n)\nWhile most formats generally pre-multiply the colors by the alpha value,\n32-bit BMP files generally do not.\nNote that if this option is set, then output colors in the palette will also be pre-multiplied.")
        .changed()
    {
        settings_changed = true;
    }

    settings_changed
}

fn draw_basic_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.heading_with_margin("Qualetize");

    ui.horizontal(|ui| {
        ui.label("Palettes:")
            .on_hover_text("Set number of palettes available");

        // Limit max palettes based on color count
        let max_palettes = 256 / state.settings.n_colors.max(1);
        // Limit max colors based on palette count
        let max_colors = 256 / state.settings.n_palettes.max(1);

        if ui
            .add(egui::DragValue::new(&mut state.settings.n_palettes).range(1..=max_palettes))
            .on_hover_text("Number of palettes available")
            .changed()
        {
            settings_changed = true;
        }

        ui.label("*");

        ui.label("Colors:")
            .on_hover_text("Set number of colors per palette\nNote that this value times the number of palettes must be less than or equal to 256.");

        if ui
            .add(egui::DragValue::new(&mut state.settings.n_colors).range(1..=max_colors))
            .on_hover_text("Number of colors per palette")
            .changed()
        {
            settings_changed = true;
        }

        ui.label("=");
        ui.label(egui::RichText::new(format!("{}", state.settings.n_colors * state.settings.n_palettes))
          .strong()).on_hover_text("Palettes * Colors per palette must be <= 256");
        ui.label("(max: 256)");
    });

    settings_changed
}

fn draw_custom_level_inputs(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;
    ui.label("Per-channel levels (0-255, comma separated, max 255 entries)");

    let channel_labels = ["R", "G", "B", "A"];
    for (idx, label) in channel_labels.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{label}:"));
            let mut response = ui.add_sized(
                [260.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut state.settings.custom_levels[idx]),
            );

            let is_valid = validate_0_255_array(&state.settings.custom_levels[idx]);
            if !is_valid {
                response = response.highlight();
                ui.painter().rect_stroke(
                    response.rect,
                    2.0,
                    egui::Stroke::new(1.0, Color32::from_rgb(255, 150, 150)),
                    egui::StrokeKind::Outside,
                );
            }

            response = response.on_hover_text(
                "Comma-separated integers between 0 and 255 (e.g., 0,49,87,119,146,174,206,255)",
            );
            settings_changed |= response.changed();

            if !is_valid {
                ui.label(egui::RichText::new("⚠").color(Color32::from_rgb(255, 180, 0)))
                    .on_hover_text(
                        "Enter comma-separated integers between 0 and 255 (max 255 entries)",
                    );
            }
        });
    }

    settings_changed
}

fn draw_depth_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;
    ui.horizontal(|ui| {
        ui.label("RGBA Depth:")
            .on_hover_text("Set RGBA bit depth\nRGBA = 8888 is standard for BMP (24-bit color + 8-bit alpha)\nFor retro targets, RGBA = 5551 is common");

        let mut mode_is_custom = state.settings.use_custom_levels;
        egui::ComboBox::from_id_salt("quant_mode")
            .selected_text(if mode_is_custom { "Custom" } else { "Linear" })
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut mode_is_custom, false, "Linear")
                    .clicked()
                {
                    settings_changed = true;
                }
                if ui
                    .selectable_value(&mut mode_is_custom, true, "Custom")
                    .clicked()
                {
                    settings_changed = true;
                }
            })
            .response
            .on_hover_text("Choose Linear (bit depth) or Custom per-channel levels");

        state.settings.use_custom_levels = mode_is_custom;
    });

    if state.settings.use_custom_levels {
        settings_changed |= draw_custom_level_inputs(ui, state);
    } else {
        let is_valid = validate_rgba_depth(&state.settings.rgba_depth);
        let is_empty = state.settings.rgba_depth.is_empty();

        let mut response = ui.add_sized(
            [60.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut state.settings.rgba_depth),
        );

        if !is_valid && !is_empty {
            response = response.highlight();
            ui.painter().rect_stroke(
                response.rect,
                2.0,
                egui::Stroke::new(1.0, Color32::from_rgb(255, 150, 150)),
                egui::StrokeKind::Outside,
            );
        }

        response = response.on_hover_text(
            "RGBA bit depth (e.g., 8888, 5551, 3331)\nR: 1-8, G: 1-8, B: 1-8, A: 1-8",
        );

        if response.changed() {
            settings_changed = true;
        }

        if let Some(error) = get_rgba_depth_error(&state.settings.rgba_depth) {
            ui.label(egui::RichText::new("⚠").color(Color32::from_rgb(255, 180, 0)))
                .on_hover_text(format!("{error}\nExamples: 8888, 5551, 3331"));
        }
    }

    settings_changed
}

fn draw_tile_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

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
            .add(egui::DragValue::new(&mut state.settings.tile_height).range(1..=64))
            .on_hover_text("Height of processing tiles")
            .changed()
        {
            settings_changed = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Quick presets:");
        if ui.small_button("8x8").clicked() {
            state.settings.tile_width = 8;
            state.settings.tile_height = 8;
            settings_changed = true;
        }
        if ui.small_button("16x16").clicked() {
            state.settings.tile_width = 16;
            state.settings.tile_height = 16;
            settings_changed = true;
        }
        if ui.small_button("32x32").clicked() {
            state.settings.tile_width = 32;
            state.settings.tile_height = 32;
            settings_changed = true;
        }
    });

    settings_changed
}

fn draw_color_space_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.subheading_with_margin("Color Space");
    egui::ComboBox::from_id_salt("color_space")
        .selected_text(state.settings.color_space.display_name())
        .show_ui(ui, |ui| {
            for color_space in ColorSpace::all() {
                if ui
                    .selectable_value(&mut state.settings.color_space, color_space.clone(), color_space.display_name())
                    .on_hover_text(color_space.description())
                    .clicked()
                {
                    settings_changed = true;
                }
            }
        })
        .response
        .on_hover_text("Set colorspace\nDifferent colorspaces may give better/worse results depending on the input image,\nand it may be necessary to experiment to find the optimal one.");

    settings_changed
}

fn draw_dithering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.subheading_with_margin("Dithering");
    egui::ComboBox::from_id_salt("dithering_mode")
        .selected_text(state.settings.dither_mode.display_name())
        .show_ui(ui, |ui| {
            for dither_mode in DitherMode::all() {
                if ui
                    .selectable_value(&mut state.settings.dither_mode, dither_mode.clone(), dither_mode.display_name())
                    .on_hover_text(dither_mode.description())
                    .clicked()
                {
                    settings_changed = true;
                }
            }
        })
        .response
        .on_hover_text("Set dither mode and level for output\nThis can reduce some of the banding artifacts caused when the colors per palette is very small,\nat the expense of added \"noise\".");

    ui.horizontal(|ui| {
        ui.label("Dither Level:")
            .on_hover_text("Dithering intensity level");
        if ui
            .add(egui::Slider::new(
                &mut state.settings.dither_level,
                0.0..=2.0,
            ))
            .on_hover_text("Adjust dithering intensity (0.0 = no dithering)")
            .changed()
        {
            settings_changed = true;
        }
    });

    settings_changed
}

fn draw_tile_reduce_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;
    ui.heading_with_margin("Tile Reduce");

    if ui
        .checkbox(&mut state.settings.tile_reduce_post_enabled, "Enable (heavy)")
        .on_hover_text(
            "Merge similar tiles after quantization using palette-aligned MSE.\nKeep threshold low to avoid visible changes.\nThis option increases processing time.",
        )
        .changed()
    {
        settings_changed = true;
    }

    ui.add_enabled_ui(state.settings.tile_reduce_post_enabled, |ui| {
        ui.horizontal(|ui| {
            if ui
                .checkbox(
                    &mut state.settings.tile_reduce_allow_flip_x,
                    "Allowed X Flips",
                )
                .changed()
            {
                settings_changed = true;
            }
            if ui
                .checkbox(
                    &mut state.settings.tile_reduce_allow_flip_y,
                    "Allowed Y Flips",
                )
                .changed()
            {
                settings_changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Threshold:")
                .on_hover_text("Average per-channel MSE per pixel after quantization.");

            let slider =
                egui::Slider::new(&mut state.settings.tile_reduce_post_threshold, 1.0..=500.0)
                    .logarithmic(false)
                    .show_value(false);
            if ui
                .add(slider)
                .changed()
            {
                settings_changed = true;
            }

            if ui
                .add(
                    egui::DragValue::new(&mut state.settings.tile_reduce_post_threshold)
                        .range(1.0..=500.0)
                        .speed(5.0),
                )
                .changed()
            {
                settings_changed = true;
            }
        });

        let reduced_text = if let (Some(base), Some(reduced)) =
            (state.base_tile_count, state.reduced_tile_count)
        {
            let diff = base.saturating_sub(reduced);
            format!("Reduced {} tiles", diff)
        } else {
            "Reduced -- tiles".to_string()
        };
        ui.label(reduced_text);
    });

    settings_changed
}

fn draw_transparency_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    if ui
        .checkbox(&mut state.settings.col0_is_clear, "First Color is Transparent")
        .on_hover_text("First color of every palette is transparent\nNote that this affects both input AND output images.\nTo set transparency in a direct-color input bitmap, an alpha channel must be used (32-bit input);\ntranslucent alpha values are supported by this tool.")
        .changed()
    {
        settings_changed = true;
    }
    settings_changed
}

fn draw_clustering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;
    ui.subheading_with_margin("Clustering");
    ui.horizontal(|ui| {
        ui.horizontal(|ui| {
            ui.label("Tile Passes:")
                .on_hover_text("Set tile cluster passes (0 = default)");
            if ui
                .add(egui::DragValue::new(&mut state.settings.tile_passes).range(0..=1000))
                .on_hover_text("Number of tile clustering passes (0 to 1000)")
                .changed()
            {
                settings_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Color Passes:")
                .on_hover_text("Set color cluster passes (0 = default)\nMost of the processing time will be spent in the loop that clusters the colors together.\nIf processing is taking excessive amounts of time, this option may be adjusted\n(e.g., for 256-color palettes, set to ~4; for 16-color palettes, set to 32-64)");
            if ui
                .add(egui::DragValue::new(&mut state.settings.color_passes).range(0..=100))
                .on_hover_text("Number of color passes (0 to 100)")
                .changed()
            {
                settings_changed = true;
            }
        });
    });

    settings_changed
}

fn draw_color_correction_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.heading_with_margin("Color Correction");

    // Define ranges to avoid duplication
    const BRIGHTNESS_RANGE: std::ops::RangeInclusive<f32> = -1.0..=1.0;
    const CONTRAST_RANGE: std::ops::RangeInclusive<f32> = 0.0..=2.0;
    const SATURATION_RANGE: std::ops::RangeInclusive<f32> = 0.0..=2.0;
    const HUE_SHIFT_RANGE: std::ops::RangeInclusive<f32> = -180.0..=180.0;
    const SHADOWS_RANGE: std::ops::RangeInclusive<f32> = -1.0..=1.0;
    const HIGHLIGHTS_RANGE: std::ops::RangeInclusive<f32> = -1.0..=1.0;
    const GAMMA_RANGE: std::ops::RangeInclusive<f32> = 0.1..=3.0;
    const GAMMA_DISPLAY_RANGE: std::ops::RangeInclusive<f32> = -100.0..=100.0;

    egui::Grid::new("color_correction_grid")
        .num_columns(3)
        .spacing([4.0, 6.0])
        .show(ui, |ui| {
            let available_width = ui.available_width();
            let slider_width = (available_width * 0.6).max(180.0);

            ui.style_mut().spacing.slider_width = slider_width;

            // Brightness
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Brightness:");
            });
            if ui
                .add_sized(
                    [slider_width, 24.0],
                    egui::Slider::new(&mut state.color_correction.brightness, BRIGHTNESS_RANGE)
                        .show_value(false),
                )
                .changed()
            {
                settings_changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut state.color_correction.brightness)
                        .range(BRIGHTNESS_RANGE)
                        .speed(0.01)
                        .custom_formatter(|n, _| format_percentage(n as f32))
                        .custom_parser(|s| {
                            // Try to parse as percentage first
                            if let Some(s) = s.strip_suffix('%') {
                                s.parse::<f64>().map(|v| v / 100.0).ok()
                            } else {
                                s.parse::<f64>().ok()
                            }
                        }),
                )
                .changed()
            {
                settings_changed = true;
            }
            ui.end_row();

            // Contrast
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Contrast:");
            });
            if ui
                .add_sized(
                    [slider_width, 24.0],
                    egui::Slider::new(&mut state.color_correction.contrast, CONTRAST_RANGE)
                        .show_value(false),
                )
                .changed()
            {
                settings_changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut state.color_correction.contrast)
                        .range(CONTRAST_RANGE)
                        .speed(0.01)
                        .fixed_decimals(2),
                )
                .changed()
            {
                settings_changed = true;
            }
            ui.end_row();

            // Saturation
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Saturation:");
            });
            if ui
                .add_sized(
                    [slider_width, 24.0],
                    egui::Slider::new(&mut state.color_correction.saturation, SATURATION_RANGE)
                        .show_value(false),
                )
                .changed()
            {
                settings_changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut state.color_correction.saturation)
                        .range(SATURATION_RANGE)
                        .speed(0.01)
                        .fixed_decimals(2),
                )
                .changed()
            {
                settings_changed = true;
            }
            ui.end_row();

            // Hue Shift
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Hue Shift:");
            });
            if ui
                .add_sized(
                    [slider_width, 24.0],
                    egui::Slider::new(&mut state.color_correction.hue_shift, HUE_SHIFT_RANGE)
                        .show_value(false),
                )
                .changed()
            {
                settings_changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut state.color_correction.hue_shift)
                        .range(HUE_SHIFT_RANGE)
                        .speed(1.0)
                        .suffix("°")
                        .fixed_decimals(0),
                )
                .changed()
            {
                settings_changed = true;
            }
            ui.end_row();

            // Shadows
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Shadows:");
            });
            if ui
                .add_sized(
                    [slider_width, 24.0],
                    egui::Slider::new(&mut state.color_correction.shadows, SHADOWS_RANGE)
                        .show_value(false),
                )
                .changed()
            {
                settings_changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut state.color_correction.shadows)
                        .range(SHADOWS_RANGE)
                        .speed(0.01)
                        .custom_formatter(|n, _| format_percentage(n as f32))
                        .custom_parser(|s| {
                            // Try to parse as percentage first
                            if let Some(s) = s.strip_suffix('%') {
                                s.parse::<f64>().map(|v| v / 100.0).ok()
                            } else {
                                s.parse::<f64>().ok()
                            }
                        }),
                )
                .changed()
            {
                settings_changed = true;
            }
            ui.end_row();

            // Highlights
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Highlights:");
            });
            if ui
                .add_sized(
                    [slider_width, 24.0],
                    egui::Slider::new(&mut state.color_correction.highlights, HIGHLIGHTS_RANGE)
                        .show_value(false),
                )
                .changed()
            {
                settings_changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut state.color_correction.highlights)
                        .range(HIGHLIGHTS_RANGE)
                        .speed(0.01)
                        .custom_formatter(|n, _| format_percentage(n as f32))
                        .custom_parser(|s| {
                            // Try to parse as percentage first
                            if let Some(s) = s.strip_suffix('%') {
                                s.parse::<f64>().map(|v| v / 100.0).ok()
                            } else {
                                s.parse::<f64>().ok()
                            }
                        }),
                )
                .changed()
            {
                settings_changed = true;
            }
            ui.end_row();

            // Gamma (special handling)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Gamma:");
            });
            let mut gamma_display = gamma_to_display_value(state.color_correction.gamma);
            if ui
                .add_sized(
                    [slider_width, 24.0],
                    egui::Slider::new(&mut gamma_display, GAMMA_DISPLAY_RANGE).show_value(false),
                )
                .changed()
            {
                state.color_correction.gamma = display_value_to_gamma(gamma_display);
                settings_changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut state.color_correction.gamma)
                        .range(GAMMA_RANGE)
                        .speed(0.01)
                        .custom_formatter(|n, _| format_gamma(n as f32))
                        .custom_parser(|s| s.parse::<f64>().ok()),
                )
                .changed()
            {
                settings_changed = true;
            }
            ui.end_row();
        });

    // Color correction presets
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let button_width = (ui.available_width() - (4.0 * 8.0)) / 5.0;

        let presets = [
            ("🔄 Reset", ColorCorrection::default()),
            ("Vibrant", ColorCorrection::preset_vibrant()),
            ("Warm", ColorCorrection::preset_retro_warm()),
            ("Cool", ColorCorrection::preset_retro_cool()),
            ("Dark", ColorCorrection::preset_dark()),
        ];

        for (label, preset) in presets {
            if ui
                .add_sized([button_width, 24.0], egui::Button::new(label))
                .clicked()
            {
                state.color_correction = preset;
                settings_changed = true;
            }
        }
    });

    settings_changed
}

fn draw_status_section(ui: &mut egui::Ui, state: &AppState) {
    ui.heading_with_margin("Debug Info");
    if let Some(request_qualetize) = &state.request_update_qualetized_image {
        let elapsed = request_qualetize.time.elapsed();
        if elapsed < state.debounce_delay {
            let remaining = state.debounce_delay - elapsed;
            ui.label(format!(
                "⏱ Preview will update in {:.1}s...",
                remaining.as_secs_f32()
            ));
        }
    }
    // Debug information
    ui.label(format!("Input path: {:?}", state.input_path.is_some()));
    ui.label(format!("Input Image: {:?}", state.input_image.is_some()));
    ui.label(format!(
        "Color Corrected Image: {:?}",
        state.color_corrected_image.is_some()
    ));
    ui.label(format!("Output Image: {:?}", state.output_image.is_some()));
    ui.label(format!(
        "Settings changed: {:?}",
        state.request_update_qualetized_image.is_some(),
    ));
}

fn validate_rgba_depth(rgba_str: &str) -> bool {
    if rgba_str.is_empty() {
        return false;
    }

    // Regex to match exactly 4 digits, each from 1-8
    let re = Regex::new(r"^[1-8]{4}$").unwrap();
    re.is_match(rgba_str)
}

fn get_rgba_depth_error(rgba_str: &str) -> Option<String> {
    if rgba_str.is_empty() {
        return Some("RGBA depth is required".to_string());
    }

    if rgba_str.len() != 4 {
        return Some(format!("Expected 4 digits, got {}", rgba_str.len()));
    }

    for (i, ch) in rgba_str.chars().enumerate() {
        if !ch.is_ascii_digit() {
            let component = match i {
                0 => "R",
                1 => "G",
                2 => "B",
                3 => "A",
                _ => "?",
            };
            return Some(format!("{component} component '{ch}' is not a digit"));
        }

        let digit = ch.to_digit(10).unwrap();
        if !(1..=8).contains(&digit) {
            let component = match i {
                0 => "R",
                1 => "G",
                2 => "B",
                3 => "A",
                _ => "?",
            };
            return Some(format!("{component} component {digit} must be 1-8"));
        }
    }

    None
}

fn draw_palette_sort_settings(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading_with_margin("Reorder Palette Colors");

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("sort_mode")
            .selected_text(state.palette_sort_settings.mode.display_name())
            .show_ui(ui, |ui| {
                for sort_mode in SortMode::all() {
                    ui.selectable_value(
                        &mut state.palette_sort_settings.mode,
                        sort_mode.clone(),
                        sort_mode.display_name(),
                    );
                }
            });
        ui.add_enabled_ui(state.palette_sort_settings.mode != SortMode::None, |ui| {
            egui::ComboBox::from_id_salt("sort_order")
                .selected_text(state.palette_sort_settings.order.display_name())
                .show_ui(ui, |ui| {
                    for sort_order in SortOrder::all() {
                        ui.selectable_value(
                            &mut state.palette_sort_settings.order,
                            sort_order.clone(),
                            sort_order.display_name(),
                        );
                    }
                });
        });
    });
}
