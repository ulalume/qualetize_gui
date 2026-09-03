use super::styles::{UiMarginExt, error_color, warning_color};
use super::widgets;
use crate::color_processor::{
    display_value_to_gamma, format_gamma, format_percentage, gamma_to_display_value,
};
use crate::engine::QuantEngine;
use crate::types::qualetize::validate_0_255_array;
use crate::types::tilepalquant::{
    DITHER_WEIGHT_RANGE, DitherPattern, FRACTION_OF_PIXELS_RANGE, TpqDitherMode,
};
use crate::types::{
    AppState, ColorSpace, DitherMode, FirstColor, QualetizePreset,
    app_state::TopLeftPixel,
    color_correction::ColorCorrectionPreset,
    image::{SortMode, SortOrder},
};
use std::ops::RangeInclusive;

/// Channel names in the order they appear in an RGBA depth string.
const RGBA_CHANNELS: [&str; 4] = ["R", "G", "B", "A"];

pub fn draw_settings_panel(ui: &mut egui::Ui, state: &mut AppState) -> (bool, bool) {
    let settings_changed = draw_quantization_settings(ui, state);
    ui.separator();

    // Color correction settings edit `state.color_correction`, not
    // `state.settings`, and app.rs already detects those changes itself, so
    // this does not feed into `settings_changed`.
    draw_color_correction_settings(ui, state);
    ui.separator();

    let tile_reduce_changed = draw_tile_reduce_settings(ui, state);
    ui.separator();

    draw_palette_sort_settings(ui, state);

    (settings_changed, tile_reduce_changed)
}

/// Half the height of one control row, used as the gap between two groups of
/// rows inside a section.
fn group_space(ui: &mut egui::Ui) {
    ui.add_space(ui.spacing().interact_size.y / 2.0);
}

/// The quantization section: the engine picker on its heading, the palette
/// size shared by both engines, the engine-specific controls, and the
/// advanced settings behind their checkbox.
fn draw_quantization_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    settings_changed |= widgets::header_row(
        ui,
        |ui| {
            ui.heading("Qualetization");
        },
        |ui| {
            let mut applied = false;
            ui.menu_button("Load preset…", |ui| {
                for preset in QualetizePreset::all() {
                    if ui.button(preset.display_name()).clicked() {
                        state.apply_qualetize_preset(preset.qualetize_settings());
                        applied = true;
                        ui.close();
                    }
                }
            });
            applied
        },
    );

    ui.horizontal(|ui| {
        ui.label("Qualetization engine:");
        for engine in QuantEngine::all() {
            settings_changed |= ui
                .radio_value(&mut state.engine, *engine, engine.display_name())
                .changed();
        }
    });
    group_space(ui);

    settings_changed |= draw_palette_size_settings(ui, state);

    group_space(ui);

    match state.engine {
        QuantEngine::Qualetize => {
            settings_changed |= draw_first_color_settings(ui, state);
            group_space(ui);
            settings_changed |= draw_dithering_settings(ui, state);
            group_space(ui);
            settings_changed |= draw_color_space_settings(ui, state);
        }
        QuantEngine::TilePalQuant => {
            settings_changed |= draw_first_color_settings(ui, state);
            group_space(ui);
            settings_changed |= draw_tpq_dithering_settings(ui, state);
        }
    }

    group_space(ui);

    settings_changed |= draw_advanced_settings(ui, state);

    settings_changed
}

/// Tile size and output bit depth, shared by both engines, plus the
/// engine-specific advanced controls. Hidden behind its own checkbox.
fn draw_advanced_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    // A preference, not a setting: toggling it changes nothing about the
    // output, so it does not report back a change. Right-aligned on its own
    // row, the same way a section heading right-aligns its toggle.
    // The horizontal row bounds the height; the nested layout only fills the
    // row from the right.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            _ = widgets::checkbox(
                ui,
                &mut state.preferences.show_advanced,
                "Advanced",
                Some(
                    "Tile size and output bit depth, plus clustering passes and alpha handling (Qualetize) or the iteration budget, seed and progress preview (tilepalquant).",
                ),
            );
        });
    });
    if !state.preferences.show_advanced {
        return settings_changed;
    }

    settings_changed |= draw_tile_settings(ui, state);

    group_space(ui);
    settings_changed |= draw_depth_settings(ui, state);

    match state.engine {
        QuantEngine::Qualetize => {
            group_space(ui);
            settings_changed |= draw_clustering_settings(ui, state);

            group_space(ui);
            settings_changed |= widgets::checkbox(
                ui,
                &mut state.settings.premul_alpha,
                "Premultiplied alpha",
                Some(
                    "Alpha is pre-multiplied (y/n)\nWhile most formats generally pre-multiply the colors by the alpha value,\n32-bit BMP files generally do not.\nNote that if this option is set, then output colors in the palette will also be pre-multiplied.",
                ),
            );
        }
        QuantEngine::TilePalQuant => {
            group_space(ui);
            settings_changed |= draw_tpq_misc_settings(ui, state);
        }
    }

    settings_changed
}

/// Palettes times colors per palette, the one target-format setting both
/// engines show outside the advanced settings.
fn draw_palette_size_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

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
          .strong()).on_hover_text("Palettes * colors per palette must be <= 256");
        ui.label("(max: 256)");
    });

    settings_changed
}

fn draw_custom_level_inputs(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;
    ui.label("Per-channel levels (0-255, comma separated)");

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
                    .on_hover_text("Enter comma-separated integers between 0 and 255");
            }
        });
    }

    settings_changed
}

fn draw_depth_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;
    ui.horizontal(|ui| {
        ui.label("RGBA depth:")
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

    ui.indent("rgba_depth_value", |ui| {
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
    });
    settings_changed
}

fn draw_tile_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.horizontal(|ui| {
        ui.label("Tile width:")
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

    ui.indent("tile_presets", |ui| {
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
    });

    settings_changed
}

fn draw_color_space_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.label("Color space:")
        .on_hover_text("Set colorspace\nDifferent colorspaces may give better/worse results depending on the input image,\nand it may be necessary to experiment to find the optimal one.");

    ui.indent("color_space_radios", |ui| {
        ui.horizontal_wrapped(|ui| {
            for space in ColorSpace::all() {
                settings_changed |= ui
                    .radio_value(
                        &mut state.settings.color_space,
                        *space,
                        space.display_name(),
                    )
                    .on_hover_text(space.description())
                    .changed();
            }
        });
    });

    settings_changed
}

fn draw_dithering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.horizontal(|ui| {
        ui.label("Dithering:");
        settings_changed |= widgets::EnumCombo::new("dithering_mode", DitherMode::all(), DitherMode::display_name)
            .description(DitherMode::description)
            .hover("Set dither mode and level for output\nThis can reduce some of the banding artifacts caused when the colors per palette is very small,\nat the expense of added \"noise\".")
            .show(ui, &mut state.settings.dither_mode);
    });

    if state.settings.dither_mode != DitherMode::None {
        settings_changed |= ui
            .indent("qualetize_dither_level", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Dither level:")
                        .on_hover_text("Dithering intensity level");
                    ui.add(egui::Slider::new(
                        &mut state.settings.dither_level,
                        0.0..=2.0,
                    ))
                    .on_hover_text("Adjust dithering intensity (0.0 = no dithering)")
                    .changed()
                })
                .inner
            })
            .inner;
    }

    settings_changed
}

/// Hover text on both engines' "Use top-left color" buttons.
const TOP_LEFT_HOVER: &str = "Use the top-left pixel as the key color; a transparent top-left pixel selects transparency from pixels instead";

/// What the transparency radios asked for this frame.
#[derive(Default)]
struct TransparencyEdit {
    /// "from transparent pixels" was picked, either on its radio or by the
    /// top-left button landing on a transparent pixel.
    from_pixels: bool,
    /// "from color" was picked.
    from_color: bool,
    /// The key color itself was edited.
    color_changed: bool,
}

/// The two radios that pick where transparency comes from, shared by both
/// engines: "from transparent pixels" on its own row, then "from color" with
/// the color picker and the "Use top-left color" button beside it.
///
/// `from_color` is which radio is selected; `color` is the key color, written
/// in place by the picker and by the button. `hovers` holds the tooltip for
/// each radio, which the two engines word differently.
fn draw_transparency_radios(
    ui: &mut egui::Ui,
    from_color: bool,
    color: &mut [u8; 3],
    hovers: (&str, &str),
    top_left: Option<TopLeftPixel>,
) -> TransparencyEdit {
    // `clicked()` on a radio fires for the entry already selected too, so
    // each arm also checks that the selection actually moves.
    let mut from_pixels = ui
        .radio(!from_color, "from transparent pixels")
        .on_hover_text(hovers.0)
        .clicked()
        && from_color;

    let mut picked_from_color = false;
    let mut color_changed = false;

    ui.horizontal(|ui| {
        picked_from_color = ui
            .radio(from_color, "from color")
            .on_hover_text(hovers.1)
            .clicked()
            && !from_color;

        if !from_color {
            return;
        }

        color_changed |= ui.color_edit_button_srgb(color).changed();
        if ui
            .button("Use top-left color")
            .on_hover_text(TOP_LEFT_HOVER)
            .clicked()
        {
            match top_left {
                Some(TopLeftPixel::Color(rgb)) => {
                    *color = rgb;
                    color_changed = true;
                }
                // An image that marks transparency with alpha has no key
                // color to read, so the button switches to reading the alpha.
                Some(TopLeftPixel::Transparent) => from_pixels = true,
                None => {}
            }
        }
    });

    TransparencyEdit {
        from_pixels,
        from_color: picked_from_color,
        color_changed,
    }
}

/// UI-only grouping over [`FirstColor`] for the combo box: the two transparent
/// variants collapse into a single "Transparent" entry, since which one
/// applies is picked by the radio buttons shown below the combo.
#[derive(Clone, Copy, PartialEq)]
enum FirstColorKind {
    Unique,
    Shared,
    Transparent,
}

impl FirstColorKind {
    fn from(first_color: FirstColor) -> Self {
        match first_color {
            FirstColor::Unique => FirstColorKind::Unique,
            FirstColor::Shared => FirstColorKind::Shared,
            FirstColor::TransparentFromAlpha | FirstColor::TransparentFromColor => {
                FirstColorKind::Transparent
            }
        }
    }

    /// The mode this kind stands for, coming from `current`. Picking
    /// "Transparent" while already on one of the two transparent variants
    /// preserves that sub-choice instead of resetting it.
    fn resolve(self, current: FirstColor) -> FirstColor {
        match self {
            FirstColorKind::Unique => FirstColor::Unique,
            FirstColorKind::Shared => FirstColor::Shared,
            FirstColorKind::Transparent if current.is_transparent() => current,
            FirstColorKind::Transparent => FirstColor::TransparentFromAlpha,
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            FirstColorKind::Unique => "Unique",
            FirstColorKind::Shared => "Shared color",
            FirstColorKind::Transparent => "Transparent",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            FirstColorKind::Unique => FirstColor::Unique.description(),
            FirstColorKind::Shared => FirstColor::Shared.description(),
            FirstColorKind::Transparent => "Index 0 is transparent; choose how below",
        }
    }

    fn all() -> &'static [FirstColorKind] {
        &[
            FirstColorKind::Unique,
            FirstColorKind::Shared,
            FirstColorKind::Transparent,
        ]
    }
}

/// Tooltip on Qualetize's "from transparent pixels" radio.
const QUALETIZE_FROM_PIXELS_HOVER: &str =
    "Index 0 is transparent; pixels with an alpha of 0 map to it";
/// Tooltip on Qualetize's "from color" radio.
const QUALETIZE_FROM_COLOR_HOVER: &str =
    "Index 0 is transparent; pixels matching the color beside it map to it, whatever their alpha";

/// What goes into index 0 of every palette, the color the shared mode puts
/// there, and where transparency comes from when index 0 is transparent.
///
/// Shared by both engines: the values live in `state.settings`, and Qualetize
/// runs [`FirstColor::Shared`] as [`FirstColor::Unique`]. Only the wording of
/// the two transparency radios differs between them.
fn draw_first_color_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    let top_left = state.top_left_pixel();
    let mut kind = FirstColorKind::from(state.settings.first_color());
    ui.horizontal(|ui| {
        ui.label("Palette index 0:");
        if widgets::EnumCombo::new(
            "first_color",
            FirstColorKind::all(),
            FirstColorKind::display_name,
        )
        .description(FirstColorKind::description)
        .show(ui, &mut kind)
        {
            let mode = kind.resolve(state.settings.first_color());
            state.settings.set_first_color(mode);
            settings_changed = true;
        }
    });

    if kind == FirstColorKind::Shared {
        ui.indent("shared_color", |ui| {
            ui.horizontal(|ui| {
                ui.label("Shared color:");
                settings_changed |= ui
                    .color_edit_button_srgb(&mut state.settings.shared_color)
                    .changed();
                if ui
                    .button("Use top-left color")
                    .on_hover_text(TOP_LEFT_HOVER)
                    .clicked()
                {
                    match top_left {
                        Some(TopLeftPixel::Color(rgb)) => {
                            state.settings.shared_color = rgb;
                            settings_changed = true;
                        }
                        // A shared color is opaque, so there is nothing for a
                        // transparent top-left pixel to select here.
                        Some(TopLeftPixel::Transparent) => {
                            log::info!(
                                "top-left pixel is transparent; shared color left unchanged"
                            );
                        }
                        None => {}
                    }
                }
            });
        });
    }

    if kind != FirstColorKind::Transparent {
        return settings_changed;
    }

    let hovers = match state.engine {
        QuantEngine::Qualetize => (QUALETIZE_FROM_PIXELS_HOVER, QUALETIZE_FROM_COLOR_HOVER),
        QuantEngine::TilePalQuant => (
            FirstColor::TransparentFromAlpha.description(),
            FirstColor::TransparentFromColor.description(),
        ),
    };

    settings_changed |= ui
        .indent("first_color_transparency", |ui| {
            let from_color = state.settings.first_color() == FirstColor::TransparentFromColor;
            let mut color = state.settings.transparent_color;
            let edit = draw_transparency_radios(ui, from_color, &mut color, hovers, top_left);

            let mut changed = false;
            if edit.color_changed {
                state.settings.set_transparent_color(color);
                changed = true;
            }
            if edit.from_pixels {
                state
                    .settings
                    .set_first_color(FirstColor::TransparentFromAlpha);
                changed = true;
            } else if edit.from_color {
                state
                    .settings
                    .set_first_color(FirstColor::TransparentFromColor);
                changed = true;
            }

            changed
        })
        .inner;

    settings_changed
}

/// Pattern combo entries for the tilepalquant Dithering row: `None` stands
/// for `TpqDitherMode::Off`, in display order (which differs from
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

    let mut pattern_option = (state.tpq_settings.dither_mode != TpqDitherMode::Off)
        .then_some(state.tpq_settings.dither_pattern);
    ui.horizontal(|ui| {
        ui.label("Dithering:");
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
        settings_changed |= ui
            .indent("tpq_dither_details", |ui| {
                let mut changed = false;

                ui.horizontal(|ui| {
                    changed |= ui
                        .radio_value(
                            &mut state.tpq_settings.dither_mode,
                            TpqDitherMode::Fast,
                            "fast",
                        )
                        .on_hover_text(TpqDitherMode::Fast.description())
                        .changed();
                    changed |= ui
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
                    changed |= ui
                        .add(
                            egui::Slider::new(
                                &mut state.tpq_settings.dither_weight,
                                DITHER_WEIGHT_RANGE,
                            )
                            .fixed_decimals(2),
                        )
                        .changed();
                });

                changed
            })
            .inner;
    }

    settings_changed
}

/// tilepalquant-only: the iteration budget, PRNG seed and progress preview.
/// Drawn inside the advanced settings, not in the engine-specific block
/// above, since these are secondary controls.
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
        "Tile reduction",
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
            "Allowed X flips",
            None,
        );
        settings_changed |= widgets::checkbox(
            ui,
            &mut state.settings.tile_reduce_allow_flip_y,
            "Allowed Y flips",
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

fn draw_clustering_settings(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;
    ui.horizontal(|ui| {
        ui.horizontal(|ui| {
            ui.label("Tile passes:")
                .on_hover_text("Set tile cluster passes (0 = default)");
            settings_changed |= ui
                .add(egui::DragValue::new(&mut state.settings.tile_passes).range(0..=1000))
                .on_hover_text("Number of tile clustering passes (0 to 1000)")
                .changed();
        });
        ui.horizontal(|ui| {
            ui.label("Color passes:")
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
        "Color correction",
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
                "Hue shift",
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
    ui.heading_with_margin("Reorder palette colors");

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
