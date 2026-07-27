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
use std::ops::RangeInclusive;

/// Channel names in the order they appear in an RGBA depth string.
const RGBA_CHANNELS: [&str; 4] = ["R", "G", "B", "A"];

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

    for (idx, label) in RGBA_CHANNELS.iter().enumerate() {
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
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(255, 150, 150)),
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
        let error = get_rgba_depth_error(&state.settings.rgba_depth);
        let is_empty = state.settings.rgba_depth.is_empty();

        let mut response = ui.add_sized(
            [60.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut state.settings.rgba_depth),
        );

        if error.is_some() && !is_empty {
            response = response.highlight();
            ui.painter().rect_stroke(
                response.rect,
                2.0,
                egui::Stroke::new(1.0_f32, Color32::from_rgb(255, 150, 150)),
                egui::StrokeKind::Outside,
            );
        }

        response = response.on_hover_text(
            "RGBA bit depth (e.g., 8888, 5551, 3331)\nR: 1-8, G: 1-8, B: 1-8, A: 1-8",
        );

        if response.changed() {
            settings_changed = true;
        }

        if let Some(error) = error {
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
                    .selectable_value(&mut state.settings.color_space, *color_space, color_space.display_name())
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
                    .selectable_value(&mut state.settings.dither_mode, *dither_mode, dither_mode.display_name())
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
    ui.heading_with_margin("Tile Reduction");

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
            if ui.add(slider).changed() {
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
        ui.label(egui::RichText::new(reduced_text).strong());
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

/// Height of one row in the color correction grid.
const ROW_HEIGHT: f32 = 24.0;

const BRIGHTNESS_RANGE: RangeInclusive<f32> = -1.0..=1.0;
const CONTRAST_RANGE: RangeInclusive<f32> = 0.0..=2.0;
const SATURATION_RANGE: RangeInclusive<f32> = 0.0..=2.0;
const HUE_SHIFT_RANGE: RangeInclusive<f32> = -180.0..=180.0;
const SHADOWS_RANGE: RangeInclusive<f32> = -1.0..=1.0;
const HIGHLIGHTS_RANGE: RangeInclusive<f32> = -1.0..=1.0;
const GAMMA_RANGE: RangeInclusive<f32> = 0.1..=3.0;
const GAMMA_DISPLAY_RANGE: RangeInclusive<f32> = -100.0..=100.0;

/// How the numeric field next to a color correction slider is shown and parsed.
#[derive(Clone, Copy)]
enum ValueFormat {
    /// `0.5` is shown as `+50%`; both `50%` and `0.5` are accepted as input.
    Percentage,
    /// Plain number with two decimals.
    Decimal,
    /// Whole degrees with a `°` suffix.
    Degrees,
}

impl ValueFormat {
    fn drag_speed(self) -> f64 {
        match self {
            ValueFormat::Percentage | ValueFormat::Decimal => 0.01,
            ValueFormat::Degrees => 1.0,
        }
    }

    fn apply(self, drag: egui::DragValue<'_>) -> egui::DragValue<'_> {
        match self {
            ValueFormat::Percentage => drag
                .custom_formatter(|n, _| format_percentage(n as f32))
                .custom_parser(parse_percentage),
            ValueFormat::Decimal => drag.fixed_decimals(2),
            ValueFormat::Degrees => drag.suffix("°").fixed_decimals(0),
        }
    }
}

/// Accepts either a percentage (`-50%`) or the raw factor (`-0.5`).
fn parse_percentage(text: &str) -> Option<f64> {
    match text.strip_suffix('%') {
        Some(number) => number.parse::<f64>().ok().map(|value| value / 100.0),
        None => text.parse::<f64>().ok(),
    }
}

fn draw_row_label(ui: &mut egui::Ui, label: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(format!("{label}:"));
    });
}

/// One `label | slider | number` row of the color correction grid.
/// Returns true if the value was edited.
fn draw_slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    format: ValueFormat,
    slider_width: f32,
) -> bool {
    draw_row_label(ui, label);

    let mut changed = ui
        .add_sized(
            [slider_width, ROW_HEIGHT],
            egui::Slider::new(value, range.clone()).show_value(false),
        )
        .changed();

    let drag = egui::DragValue::new(value)
        .range(range)
        .speed(format.drag_speed());
    changed |= ui.add(format.apply(drag)).changed();

    ui.end_row();
    changed
}

/// Gamma needs its own row: the slider edits an intuitive -100..100 display
/// value while the number field edits the gamma exponent directly.
fn draw_gamma_row(ui: &mut egui::Ui, gamma: &mut f32, slider_width: f32) -> bool {
    draw_row_label(ui, "Gamma");

    let mut changed = false;
    let mut display = gamma_to_display_value(*gamma);
    if ui
        .add_sized(
            [slider_width, ROW_HEIGHT],
            egui::Slider::new(&mut display, GAMMA_DISPLAY_RANGE).show_value(false),
        )
        .changed()
    {
        *gamma = display_value_to_gamma(display);
        changed = true;
    }

    changed |= ui
        .add(
            egui::DragValue::new(gamma)
                .range(GAMMA_RANGE)
                .speed(0.01)
                .custom_formatter(|n, _| format_gamma(n as f32))
                .custom_parser(|s| s.parse::<f64>().ok()),
        )
        .changed();

    ui.end_row();
    changed
}

fn draw_color_correction_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.heading_with_margin("Color Correction");

    egui::Grid::new("color_correction_grid")
        .num_columns(3)
        .spacing([4.0, 6.0])
        .show(ui, |ui| {
            let slider_width = (ui.available_width() * 0.6).max(180.0);
            ui.style_mut().spacing.slider_width = slider_width;

            let correction = &mut state.color_correction;
            let mut row = |label, value: &mut f32, range, format| {
                draw_slider_row(ui, label, value, range, format, slider_width)
            };

            settings_changed |= row(
                "Brightness",
                &mut correction.brightness,
                BRIGHTNESS_RANGE,
                ValueFormat::Percentage,
            );
            settings_changed |= row(
                "Contrast",
                &mut correction.contrast,
                CONTRAST_RANGE,
                ValueFormat::Decimal,
            );
            settings_changed |= row(
                "Saturation",
                &mut correction.saturation,
                SATURATION_RANGE,
                ValueFormat::Decimal,
            );
            settings_changed |= row(
                "Hue Shift",
                &mut correction.hue_shift,
                HUE_SHIFT_RANGE,
                ValueFormat::Degrees,
            );
            settings_changed |= row(
                "Shadows",
                &mut correction.shadows,
                SHADOWS_RANGE,
                ValueFormat::Percentage,
            );
            settings_changed |= row(
                "Highlights",
                &mut correction.highlights,
                HIGHLIGHTS_RANGE,
                ValueFormat::Percentage,
            );

            settings_changed |= draw_gamma_row(ui, &mut correction.gamma, slider_width);
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
                .add_sized([button_width, ROW_HEIGHT], egui::Button::new(label))
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

/// Exactly 4 digits, each from 1-8.
/// Returns the reason `rgba_str` is not a valid RGBA depth, or `None` if it is.
fn get_rgba_depth_error(rgba_str: &str) -> Option<String> {
    if rgba_str.is_empty() {
        return Some("RGBA depth is required".to_string());
    }

    let digit_count = rgba_str.chars().count();
    if digit_count != RGBA_CHANNELS.len() {
        return Some(format!(
            "Expected {} digits, got {digit_count}",
            RGBA_CHANNELS.len()
        ));
    }

    for (channel, ch) in RGBA_CHANNELS.iter().zip(rgba_str.chars()) {
        let Some(digit) = ch.to_digit(10) else {
            return Some(format!("{channel} component '{ch}' is not a digit"));
        };
        if !(1..=8).contains(&digit) {
            return Some(format!("{channel} component {digit} must be 1-8"));
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
                        *sort_mode,
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
                            *sort_order,
                            sort_order.display_name(),
                        );
                    }
                });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_rgba_depths_report_no_error() {
        for depth in ["8888", "5551", "3331", "1111"] {
            assert_eq!(get_rgba_depth_error(depth), None, "{depth} should be valid");
        }
    }

    #[test]
    fn invalid_rgba_depths_report_an_error() {
        for depth in ["", "888", "88888", "8x88", "0888", "8898"] {
            assert!(
                get_rgba_depth_error(depth).is_some(),
                "{depth} should be invalid"
            );
        }
    }

    #[test]
    fn rgba_depth_error_names_the_offending_channel() {
        assert!(get_rgba_depth_error("88x8").unwrap().starts_with('B'));
        assert!(get_rgba_depth_error("0888").unwrap().starts_with('R'));
    }
}
