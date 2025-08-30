use crate::color_correction::{
    ColorProcessor, display_value_to_gamma, format_gamma, format_percentage, gamma_to_display_value,
};
use crate::types::{AppState, ColorSpace, DitherMode};
use egui::{Color32, Frame, Margin};

pub fn draw_settings_panel(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    // Basic settings
    settings_changed |= draw_basic_settings(ui, state);

    ui.separator();

    // Color space settings
    settings_changed |= draw_color_space_settings(ui, state);

    ui.separator();

    // Dithering settings
    settings_changed |= draw_dithering_settings(ui, state);

    ui.separator();

    // Transparency settings
    settings_changed |= draw_transparency_settings(ui, state);

    ui.separator();

    // Advanced clustering settings (if enabled)
    if state.show_advanced {
        settings_changed |= draw_clustering_settings(ui, state);
        ui.separator();
    }

    // Color correction settings
    settings_changed |= draw_color_correction_settings(ui, state);

    ui.separator();

    // Status display
    draw_status_section(ui, state);

    settings_changed
}

fn draw_basic_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.heading("Basic");

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
            .on_hover_text("Set number of colours per palette\nNote that this value times the number of palettes must be less than or equal to 256.");

        if ui
            .add(egui::DragValue::new(&mut state.settings.n_colors).range(1..=max_colors))
            .on_hover_text("Number of colours per palette")
            .changed()
        {
            settings_changed = true;
        }

        ui.label("=");
        ui.label(egui::RichText::new(format!("{}", state.settings.n_colors * state.settings.n_palettes))
          .strong()).on_hover_text("Palettes * Colors per palette must be <= 256");
        ui.label("(max: 256)");
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

    // Advanced tile settings
    if state.show_advanced {
        settings_changed |= draw_tile_settings(ui, state);
    }

    settings_changed
}

fn draw_tile_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 48))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 80)))
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

            if ui
                .checkbox(&mut state.settings.premul_alpha, "Premultiplied Alpha")
                .on_hover_text("Alpha is pre-multiplied (y/n)\nWhile most formats generally pre-multiply the colours by the alpha value,\n32-bit BMP files generally do not.\nNote that if this option is set, then output colours in the palette will also be pre-multiplied.")
                .changed()
            {
                settings_changed = true;
            }
        });

    settings_changed
}

fn draw_color_space_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.heading("Color Space");
    egui::ComboBox::from_label("Color Space")
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
        .on_hover_text("Set colourspace\nDifferent colourspaces may give better/worse results depending on the input image,\nand it may be necessary to experiment to find the optimal one.");

    settings_changed
}

fn draw_dithering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.heading("Dithering");
    egui::ComboBox::from_label("Dithering Mode")
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
        .on_hover_text("Set dither mode and level for output\nThis can reduce some of the banding artifacts caused when the colours per palette is very small,\nat the expense of added \"noise\".");

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

fn draw_transparency_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.heading("Transparency");
    if ui
        .checkbox(&mut state.settings.col0_is_clear, "First Color is Transparent")
        .on_hover_text("First colour of every palette is transparent\nNote that this affects both input AND output images.\nTo set transparency in a direct-colour input bitmap, an alpha channel must be used (32-bit input);\ntranslucent alpha values are supported by this tool.")
        .changed()
    {
        settings_changed = true;
    }

    if state.show_advanced {
        Frame::NONE
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 48))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 80)))
            .inner_margin(Margin::same(4))
            .outer_margin(Margin::same(4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Clear Color:")
                        .on_hover_text("Set colour of transparent pixels.\nNote that as long as the RGB values match the clear colour,\nthen the pixel will be made fully transparent, regardless of any alpha information.\nCan be 'none', or a '#RRGGBB' hex triad.");
                    if ui
                        .text_edit_singleline(&mut state.settings.clear_color)
                        .changed()
                    {
                        settings_changed = true;
                    }
                });
            });
    }

    settings_changed
}

fn draw_clustering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 48))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 80)))
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            ui.heading("Clustering");
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
                    .on_hover_text("Set colour cluster passes (0 = default)\nMost of the processing time will be spent in the loop that clusters the colours together.\nIf processing is taking excessive amounts of time, this option may be adjusted\n(e.g., for 256-colour palettes, set to ~4; for 16-colour palettes, set to 32-64)");
                if ui
                    .add(egui::DragValue::new(&mut state.settings.color_passes).range(0..=100))
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
                    .add(egui::DragValue::new(&mut state.settings.split_ratio).range(-1.0..=1.0))
                    .on_hover_text("Split Ratio (-1.0 to 1.0)")
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

    ui.heading("Color Correction");

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

            // Contrast
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Contrast:");
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

            // Saturation
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Saturation:");
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

            // Highlights
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("Highlights:");
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
            ("🔄 Reset", ColorProcessor::reset_corrections()),
            ("✨ Vibrant", ColorProcessor::preset_vibrant()),
            ("🌅 Warm", ColorProcessor::preset_retro_warm()),
            ("❄ Cool", ColorProcessor::preset_retro_cool()),
            ("🌚 Dark", ColorProcessor::preset_dark()),
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
    ui.heading("Status");
    if state.preview_processing {
        ui.label("🔄 Generating preview...");
    } else if let Some(last_change_time) = state.last_settings_change_time {
        let elapsed = last_change_time.elapsed();
        if elapsed < state.debounce_delay {
            let remaining = state.debounce_delay - elapsed;
            ui.label(format!(
                "⏱️ Preview will update in {:.1}s...",
                remaining.as_secs_f32()
            ));
        }
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
}
