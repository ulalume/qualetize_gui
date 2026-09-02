use super::styles::{UiMarginExt, error_color, warning_color};
use super::widgets;
use crate::color_processor::{
    display_value_to_gamma, format_gamma, format_percentage, gamma_to_display_value,
};
use crate::engine::QuantEngine;
use crate::types::qualetize::validate_0_255_array;
use crate::types::tilepalquant::{
    ColorZero, DITHER_WEIGHT_RANGE, DitherPattern, FRACTION_OF_PIXELS_RANGE, TpqDitherMode,
};
use crate::types::{
    AppState, ClearColor, ColorSpace, DitherMode,
    color_correction::ColorCorrectionPreset,
    image::{SortMode, SortOrder},
};
use std::ops::RangeInclusive;

/// Channel names in the order they appear in an RGBA depth string.
const RGBA_CHANNELS: [&str; 4] = ["R", "G", "B", "A"];

pub fn draw_settings_panel(ui: &mut egui::Ui, state: &mut AppState) -> (bool, bool) {
    let mut settings_changed = false;
    let mut tile_reduce_changed = false;

    // Basic settings: engine picker plus the Palettes/Colors shared by both engines.
    settings_changed |= draw_basic_settings(ui, state);

    // Engine-specific sections. Advanced Settings below stays shared: it holds
    // the tile size and depth settings both engines read, plus the
    // Qualetize-only ones nested inside it.
    match state.engine {
        QuantEngine::Qualetize => {
            settings_changed |= draw_transparency_settings(ui, state);
            ui.separator();
            settings_changed |= draw_color_space_settings(ui, state);
            ui.separator();
            settings_changed |= draw_dithering_settings(ui, state);
            ui.separator();
        }
        QuantEngine::TilePalQuant => {
            settings_changed |= draw_tpq_color_zero_settings(ui, state);
            ui.separator();
            settings_changed |= draw_tpq_dithering_settings(ui, state);
            ui.separator();
        }
    }

    // Advanced settings, collapsed to their heading until shown
    settings_changed |= draw_advanced_settings(ui, state);
    ui.separator();

    // Color correction settings. These edit `state.color_correction`, not
    // `state.settings`, and app.rs already detects those changes itself, so
    // this does not feed into `settings_changed`.
    draw_color_correction_settings(ui, state);
    ui.separator();

    tile_reduce_changed |= draw_tile_reduce_settings(ui, state);
    ui.separator();
    draw_palette_sort_settings(ui, state);

    (settings_changed, tile_reduce_changed)
}

fn draw_advanced_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    // Subheading, not heading: this is a subsection of the engine-specific
    // settings above, same as the "Color Space" and "Dithering" sections
    // beside it. Tile size and depth are shared by both engines; the rest
    // (transparent color, clustering, alpha handling) is Qualetize-only.
    widgets::subsection_header(
        ui,
        "Advanced Settings",
        &mut state.preferences.show_advanced,
        "Show",
        Some(
            "Tile size and output bit depth, plus (Qualetize only) transparent color, clustering passes and alpha handling.",
        ),
    );
    if !state.preferences.show_advanced {
        return settings_changed;
    }

    settings_changed |= draw_tile_settings(ui, state);

    ui.separator();
    settings_changed |= draw_depth_settings(ui, state);

    if state.engine == QuantEngine::Qualetize {
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
                if ui.button("Use top-left color").clicked()
                    && let Some(color_corrected_image) = &state.color_corrected_image
                {
                    [*r, *g, *b, _] = color_corrected_image.top_left_pixel();
                    settings_changed = true;
                }
                ui.label(format!("#{:02X}{:02X}{:02X}", *r, *g, *b));
            });
        }

        ui.separator();

        settings_changed |= draw_clustering_settings(ui, state);

        ui.separator();
        settings_changed |= widgets::checkbox(
            ui,
            &mut state.settings.premul_alpha,
            "Premultiplied Alpha",
            Some(
                "Alpha is pre-multiplied (y/n)\nWhile most formats generally pre-multiply the colors by the alpha value,\n32-bit BMP files generally do not.\nNote that if this option is set, then output colors in the palette will also be pre-multiplied.",
            ),
        );
    }

    if state.engine == QuantEngine::TilePalQuant {
        ui.separator();
        settings_changed |= draw_tpq_misc_settings(ui, state);
    }

    settings_changed
}

fn draw_basic_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    settings_changed |= widgets::heading_with_combo(
        ui,
        "Quantization",
        widgets::EnumCombo::new(
            "quant_engine",
            QuantEngine::all(),
            QuantEngine::display_name,
        ),
        &mut state.engine,
    );

    let is_tpq = state.engine == QuantEngine::TilePalQuant;
    // tilepalquant requires at least 2 colors per palette (index 0 plus at
    // least one more) and caps the palette count at 64.
    let min_colors = if is_tpq { 2 } else { 1 };
    let max_palette_count = if is_tpq { 64 } else { u16::MAX };

    ui.horizontal(|ui| {
        ui.label("Palettes:")
            .on_hover_text("Set number of palettes available");

        // Limit max palettes based on color count
        let max_palettes = (256 / state.settings.n_colors.max(1)).min(max_palette_count);
        // Limit max colors based on palette count
        let max_colors = 256 / state.settings.n_palettes.max(1);

        settings_changed |= widgets::drag_u16(
            ui,
            &mut state.settings.n_palettes,
            1..=max_palettes,
            "Number of palettes available",
        );

        ui.label("*");

        ui.label("Colors:")
            .on_hover_text("Set number of colors per palette\nNote that this value times the number of palettes must be less than or equal to 256.");

        settings_changed |= widgets::drag_u16(
            ui,
            &mut state.settings.n_colors,
            min_colors..=max_colors,
            "Number of colors per palette",
        );

        ui.label("=");
        ui.label(egui::RichText::new(format!("{}", state.settings.n_colors as u32 * state.settings.n_palettes as u32))
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
                    egui::Stroke::new(1.0_f32, error_color(ui.visuals())),
                    egui::StrokeKind::Outside,
                );
            }

            response = response.on_hover_text(
                "Comma-separated integers between 0 and 255 (e.g., 0,49,87,119,146,174,206,255)",
            );
            // Invalid text is still being edited, so it must not trigger a
            // re-quantization: the C library would silently drop the channel.
            settings_changed |= response.changed() && is_valid;

            if !is_valid {
                ui.label(egui::RichText::new("⚠").color(warning_color(ui.visuals())))
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
                settings_changed |= ui
                    .selectable_value(&mut mode_is_custom, false, "Linear")
                    .changed();
                settings_changed |= ui
                    .selectable_value(&mut mode_is_custom, true, "Custom")
                    .changed();
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
                egui::Stroke::new(1.0_f32, error_color(ui.visuals())),
                egui::StrokeKind::Outside,
            );
        }

        response = response.on_hover_text(
            "RGBA bit depth (e.g., 8888, 5551, 3331)\nR: 1-8, G: 1-8, B: 1-8, A: 1-8",
        );

        // Invalid text is still being edited, so it must not trigger a
        // re-quantization: the C library would silently drop a channel.
        settings_changed |= response.changed() && error.is_none();

        if let Some(error) = error {
            ui.label(egui::RichText::new("⚠").color(warning_color(ui.visuals())))
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
        settings_changed |= widgets::drag_u16(
            ui,
            &mut state.settings.tile_width,
            1..=64,
            "Width of processing tiles",
        );
        ui.label("Height:")
            .on_hover_text("Set tile height for processing");
        settings_changed |= widgets::drag_u16(
            ui,
            &mut state.settings.tile_height,
            1..=64,
            "Height of processing tiles",
        );
    });

    ui.horizontal(|ui| {
        ui.label("Quick presets:");
        for n in [8u16, 16, 32] {
            if ui.small_button(format!("{n}x{n}")).clicked() {
                state.settings.tile_width = n;
                state.settings.tile_height = n;
                settings_changed = true;
            }
        }
    });

    settings_changed
}

fn draw_color_space_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    ui.subheading_with_margin("Color Space");
    widgets::EnumCombo::new("color_space", ColorSpace::all(), ColorSpace::display_name)
        .description(ColorSpace::description)
        .hover("Set colorspace\nDifferent colorspaces may give better/worse results depending on the input image,\nand it may be necessary to experiment to find the optimal one.")
        .show(ui, &mut state.settings.color_space)
}

fn draw_dithering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.subheading_with_margin("Dithering");
    settings_changed |= widgets::EnumCombo::new("dithering_mode", DitherMode::all(), DitherMode::display_name)
        .description(DitherMode::description)
        .hover("Set dither mode and level for output\nThis can reduce some of the banding artifacts caused when the colors per palette is very small,\nat the expense of added \"noise\".")
        .show(ui, &mut state.settings.dither_mode);

    if state.settings.dither_mode != DitherMode::None {
        ui.horizontal(|ui| {
            ui.label("Dither Level:")
                .on_hover_text("Dithering intensity level");
            settings_changed |= ui
                .add(egui::Slider::new(
                    &mut state.settings.dither_level,
                    0.0..=2.0,
                ))
                .on_hover_text("Adjust dithering intensity (0.0 = no dithering)")
                .changed();
        });
    }

    settings_changed
}

/// UI-only grouping over [`ColorZero`] for the combo box: the two transparent
/// variants collapse into a single "Transparent" entry, since which one
/// applies is picked by the radio buttons shown below the combo.
#[derive(Clone, Copy, PartialEq)]
enum ColorZeroKind {
    Unique,
    Shared,
    Transparent,
}

impl ColorZeroKind {
    fn from(color_zero: ColorZero) -> Self {
        match color_zero {
            ColorZero::Unique => ColorZeroKind::Unique,
            ColorZero::Shared => ColorZeroKind::Shared,
            ColorZero::TransparentFromAlpha | ColorZero::TransparentFromColor => {
                ColorZeroKind::Transparent
            }
        }
    }

    /// Applies the picked kind to `color_zero`. Picking "Transparent" while
    /// already on one of the two transparent variants preserves that
    /// sub-choice instead of resetting it.
    fn apply(self, color_zero: &mut ColorZero) {
        *color_zero = match self {
            ColorZeroKind::Unique => ColorZero::Unique,
            ColorZeroKind::Shared => ColorZero::Shared,
            ColorZeroKind::Transparent if color_zero.is_transparent() => *color_zero,
            ColorZeroKind::Transparent => ColorZero::TransparentFromAlpha,
        };
    }

    fn display_name(&self) -> &'static str {
        match self {
            ColorZeroKind::Unique => "Unique",
            ColorZeroKind::Shared => "Shared color",
            ColorZeroKind::Transparent => "Transparent",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            ColorZeroKind::Unique => ColorZero::Unique.description(),
            ColorZeroKind::Shared => ColorZero::Shared.description(),
            ColorZeroKind::Transparent => "Index 0 is transparent; choose how below",
        }
    }

    fn all() -> &'static [ColorZeroKind] {
        &[
            ColorZeroKind::Unique,
            ColorZeroKind::Shared,
            ColorZeroKind::Transparent,
        ]
    }
}

/// tilepalquant-only: what goes into index 0 of every palette, and the color
/// that mode needs.
fn draw_tpq_color_zero_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.subheading_with_margin("Color index zero");

    let mut kind = ColorZeroKind::from(state.tpq_settings.color_zero);
    ui.horizontal(|ui| {
        if widgets::EnumCombo::new(
            "tpq_color_zero",
            ColorZeroKind::all(),
            ColorZeroKind::display_name,
        )
        .description(ColorZeroKind::description)
        .show(ui, &mut kind)
        {
            kind.apply(&mut state.tpq_settings.color_zero);
            settings_changed = true;
        }

        if kind == ColorZeroKind::Shared {
            settings_changed |= ui
                .color_edit_button_srgb(&mut state.tpq_settings.shared_color)
                .changed();
            if ui.button("Use top-left color").clicked()
                && let Some(color_corrected_image) = &state.color_corrected_image
            {
                let [r, g, b, _] = color_corrected_image.top_left_pixel();
                state.tpq_settings.shared_color = [r, g, b];
                settings_changed = true;
            }
        }
    });

    if kind == ColorZeroKind::Transparent {
        settings_changed |= ui
            .radio_value(
                &mut state.tpq_settings.color_zero,
                ColorZero::TransparentFromAlpha,
                "from transparent pixels",
            )
            .on_hover_text(ColorZero::TransparentFromAlpha.description())
            .changed();
        ui.horizontal(|ui| {
            settings_changed |= ui
                .radio_value(
                    &mut state.tpq_settings.color_zero,
                    ColorZero::TransparentFromColor,
                    "from color",
                )
                .on_hover_text(ColorZero::TransparentFromColor.description())
                .changed();

            if state.tpq_settings.color_zero == ColorZero::TransparentFromColor {
                settings_changed |= ui
                    .color_edit_button_srgb(&mut state.tpq_settings.transparent_color)
                    .changed();
                if ui.button("Use top-left color").clicked()
                    && let Some(color_corrected_image) = &state.color_corrected_image
                {
                    let [r, g, b, _] = color_corrected_image.top_left_pixel();
                    state.tpq_settings.transparent_color = [r, g, b];
                    settings_changed = true;
                }
            }
        });
    }

    settings_changed
}

/// Pattern combo entries for the tilepalquant Dithering section: `None`
/// stands for `TpqDitherMode::Off`, in display order (which differs from
/// [`DitherPattern::all`]'s declaration order).
const TPQ_DITHER_PATTERN_OPTIONS: [Option<DitherPattern>; 7] = [
    None,
    Some(DitherPattern::Diagonal2),
    Some(DitherPattern::Diagonal4),
    Some(DitherPattern::Horizontal2),
    Some(DitherPattern::Horizontal4),
    Some(DitherPattern::Vertical2),
    Some(DitherPattern::Vertical4),
];

fn tpq_dither_pattern_option_name(option: &Option<DitherPattern>) -> &'static str {
    match option {
        None => "None",
        Some(pattern) => pattern.display_name(),
    }
}

/// tilepalquant-only: dither pattern (or off), and (when enabled) the
/// fast/slow mode and weight.
fn draw_tpq_dithering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.subheading_with_margin("Dithering");

    let mut pattern_option = (state.tpq_settings.dither_mode != TpqDitherMode::Off)
        .then_some(state.tpq_settings.dither_pattern);
    ui.horizontal(|ui| {
        ui.label("Pattern:");
        if widgets::EnumCombo::new(
            "tpq_dither_pattern",
            &TPQ_DITHER_PATTERN_OPTIONS,
            tpq_dither_pattern_option_name,
        )
        .show(ui, &mut pattern_option)
        {
            match pattern_option {
                None => state.tpq_settings.dither_mode = TpqDitherMode::Off,
                Some(pattern) => {
                    state.tpq_settings.dither_pattern = pattern;
                    if state.tpq_settings.dither_mode == TpqDitherMode::Off {
                        state.tpq_settings.dither_mode = TpqDitherMode::Fast;
                    }
                }
            }
            settings_changed = true;
        }
    });

    if state.tpq_settings.dither_mode != TpqDitherMode::Off {
        ui.horizontal(|ui| {
            settings_changed |= ui
                .radio_value(
                    &mut state.tpq_settings.dither_mode,
                    TpqDitherMode::Fast,
                    "fast",
                )
                .on_hover_text(TpqDitherMode::Fast.description())
                .changed();
            settings_changed |= ui
                .radio_value(
                    &mut state.tpq_settings.dither_mode,
                    TpqDitherMode::Slow,
                    "slow",
                )
                .on_hover_text(TpqDitherMode::Slow.description())
                .changed();
        });

        ui.horizontal(|ui| {
            ui.label("Weight:");
            settings_changed |= ui
                .add(
                    egui::Slider::new(&mut state.tpq_settings.dither_weight, DITHER_WEIGHT_RANGE)
                        .fixed_decimals(2),
                )
                .changed();
        });
    }

    settings_changed
}

/// tilepalquant-only: the iteration budget, PRNG seed and progress preview.
/// Drawn at the bottom of Advanced Settings, not in the engine-specific
/// block above, since these are secondary/advanced controls.
fn draw_tpq_misc_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.horizontal(|ui| {
        ui.label("Fraction of pixels:");
        settings_changed |= ui
            .add(
                egui::DragValue::new(&mut state.tpq_settings.fraction_of_pixels)
                    .range(FRACTION_OF_PIXELS_RANGE)
                    .speed(0.01)
                    .fixed_decimals(2),
            )
            .on_hover_text(
                "Iteration budget relative to the pixel count. Lower is faster; 0.05 is usually indistinguishable from 0.1.",
            )
            .changed();
    });

    ui.horizontal(|ui| {
        ui.label("Random seed:");
        settings_changed |= ui
            .add(egui::DragValue::new(&mut state.tpq_settings.rand_seed))
            .changed();
        settings_changed |= widgets::checkbox(
            ui,
            &mut state.tpq_settings.randomize_seed,
            "Randomize each run",
            Some(
                "Pick a new seed for every run and store it here so the result can be reproduced.",
            ),
        );
    });

    settings_changed |= widgets::checkbox(
        ui,
        &mut state.tpq_settings.show_progress,
        "Show progress",
        Some("Show the palettes converging while quantizing (slightly slower)."),
    );

    settings_changed
}

fn draw_tile_reduce_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    settings_changed |= widgets::section_header(
        ui,
        "Tile Reduction",
        &mut state.settings.tile_reduce_post_enabled,
        "Enable",
        Some(
            "Merge similar tiles after quantization using palette-aligned MSE.\nKeep threshold low to avoid visible changes.\nThis pass is heavy and increases processing time.",
        ),
    );

    // The settings only exist once the pass is turned on.
    if !state.settings.tile_reduce_post_enabled {
        return settings_changed;
    }

    ui.horizontal(|ui| {
        settings_changed |= widgets::checkbox(
            ui,
            &mut state.settings.tile_reduce_allow_flip_x,
            "Allowed X Flips",
            None,
        );
        settings_changed |= widgets::checkbox(
            ui,
            &mut state.settings.tile_reduce_allow_flip_y,
            "Allowed Y Flips",
            None,
        );
    });

    ui.horizontal(|ui| {
        ui.label("Threshold:")
            .on_hover_text("Average per-channel MSE per pixel after quantization.");

        let slider = egui::Slider::new(&mut state.settings.tile_reduce_post_threshold, 1.0..=500.0)
            .logarithmic(false)
            .show_value(false);
        settings_changed |= ui.add(slider).changed();

        settings_changed |= ui
            .add(
                egui::DragValue::new(&mut state.settings.tile_reduce_post_threshold)
                    .range(1.0..=500.0)
                    .speed(5.0),
            )
            .changed();
    });

    let reduced_text = match (state.base_tile_count, state.reduced_tile_count) {
        (Some(base), Some(reduced)) => format!("Reduced {} tiles", base.saturating_sub(reduced)),
        _ => "Reduced -- tiles".to_string(),
    };
    ui.label(egui::RichText::new(reduced_text).strong());

    ui.add_space(4.0);
    if ui
        .add_sized([80.0, ROW_HEIGHT], egui::Button::new("🔄 Reset"))
        .on_hover_text("Restore the threshold and flip options to their defaults")
        .clicked()
    {
        state.settings.reset_tile_reduce();
        settings_changed = true;
    }

    settings_changed
}

fn draw_transparency_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    widgets::checkbox(
        ui,
        &mut state.settings.col0_is_clear,
        "First Color is Transparent",
        Some(
            "First color of every palette is transparent\nNote that this affects both input AND output images.\nTo set transparency in a direct-color input bitmap, an alpha channel must be used (32-bit input);\ntranslucent alpha values are supported by this tool.",
        ),
    )
}

fn draw_clustering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;
    ui.subheading_with_margin("Clustering");
    ui.horizontal(|ui| {
        ui.horizontal(|ui| {
            ui.label("Tile Passes:")
                .on_hover_text("Set tile cluster passes (0 = default)");
            settings_changed |= ui
                .add(egui::DragValue::new(&mut state.settings.tile_passes).range(0..=1000))
                .on_hover_text("Number of tile clustering passes (0 to 1000)")
                .changed();
        });
        ui.horizontal(|ui| {
            ui.label("Color Passes:")
                .on_hover_text("Set color cluster passes (0 = default)\nMost of the processing time will be spent in the loop that clusters the colors together.\nIf processing is taking excessive amounts of time, this option may be adjusted\n(e.g., for 256-color palettes, set to ~4; for 16-color palettes, set to 32-64)");
            settings_changed |= ui
                .add(egui::DragValue::new(&mut state.settings.color_passes).range(0..=100))
                .on_hover_text("Number of color passes (0 to 100)")
                .changed();
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
/// Gamma is edited in hundredths, like the other decimal fields.
const GAMMA_STEP: f32 = 0.01;

/// Round `value` to the nearest multiple of `step`, so a slider drag lands on
/// the same values the number field can show and type.
fn snap(value: f32, step: f32) -> f32 {
    // `+ 0.0` normalizes a negative zero, which would otherwise be saved to
    // the session file and shown as "-0%".
    (value / step).round() * step + 0.0
}

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
    /// Granularity of the value: one percent, one hundredth, or one degree.
    /// Both the slider and the number field move in these steps.
    fn step(self) -> f32 {
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

    let step = format.step();
    let mut changed = ui
        .add_sized(
            [slider_width, ROW_HEIGHT],
            egui::Slider::new(value, range.clone())
                .step_by(step as f64)
                .show_value(false),
        )
        .changed();

    let drag = egui::DragValue::new(value).range(range).speed(step as f64);
    changed |= ui.add(format.apply(drag)).changed();

    if changed {
        *value = snap(*value, step);
    }
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
                .speed(GAMMA_STEP as f64)
                .custom_formatter(|n, _| format_gamma(n as f32))
                .custom_parser(|s| s.parse::<f64>().ok()),
        )
        .changed();

    if changed {
        *gamma = snap(*gamma, GAMMA_STEP).clamp(*GAMMA_RANGE.start(), *GAMMA_RANGE.end());
    }
    ui.end_row();
    changed
}

/// Draws the color correction controls. These edit `state.color_correction`
/// rather than `state.settings`, and app.rs detects that change on its own
/// (comparing against the previous frame's value) to rebuild the corrected
/// image, so none of this needs to report back a "settings changed" flag.
fn draw_color_correction_settings(ui: &mut egui::Ui, state: &mut AppState) {
    widgets::section_header(
        ui,
        "Color Correction",
        &mut state.color_correction.enabled,
        "Enable",
        Some(
            "Adjust the image before quantization.\nWhen off the input image is passed through untouched.",
        ),
    );

    // The settings only exist once the pass is turned on.
    if !state.color_correction.enabled {
        return;
    }

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

            row(
                "Brightness",
                &mut correction.brightness,
                BRIGHTNESS_RANGE,
                ValueFormat::Percentage,
            );
            row(
                "Contrast",
                &mut correction.contrast,
                CONTRAST_RANGE,
                ValueFormat::Decimal,
            );
            row(
                "Saturation",
                &mut correction.saturation,
                SATURATION_RANGE,
                ValueFormat::Decimal,
            );
            row(
                "Hue Shift",
                &mut correction.hue_shift,
                HUE_SHIFT_RANGE,
                ValueFormat::Degrees,
            );
            row(
                "Shadows",
                &mut correction.shadows,
                SHADOWS_RANGE,
                ValueFormat::Percentage,
            );
            row(
                "Highlights",
                &mut correction.highlights,
                HIGHLIGHTS_RANGE,
                ValueFormat::Percentage,
            );

            draw_gamma_row(ui, &mut correction.gamma, slider_width);
        });

    // Color correction presets
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let button_width = (ui.available_width() - (4.0 * 8.0)) / 5.0;

        for preset in ColorCorrectionPreset::all() {
            // The reset entry keeps its distinct icon label; the rest use the
            // preset's own display name.
            let label = match preset {
                ColorCorrectionPreset::None => "🔄 Reset",
                _ => preset.display_name(),
            };
            if ui
                .add_sized([button_width, ROW_HEIGHT], egui::Button::new(label))
                .clicked()
            {
                state
                    .color_correction
                    .apply_preset(preset.color_correction());
            }
        }
    });
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
    fn snap_rounds_to_the_nearest_step() {
        assert_eq!(snap(0.123, 0.01), 0.12);
        assert_eq!(snap(0.126, 0.01), 0.13);
        assert_eq!(snap(-0.007, 0.01), -0.01);
        assert_eq!(snap(37.4, 1.0), 37.0);
        assert_eq!(snap(1.0, 0.01), 1.0);
        assert!(snap(-0.001, 0.01).is_sign_positive(), "no negative zero");
    }

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
