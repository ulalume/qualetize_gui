//! The Results side panel: the recorded outputs, newest first. Each entry
//! shows its image, the palettes it uses and a summary of the settings that
//! produced it, and can put those settings back in use or write the image
//! out without re-running the pipeline.

use crate::engine::QuantEngine;
use crate::settings_manager::SettingsBundle;
use crate::types::app_state::{AppStateRequest, ResultTextures};
use crate::types::image::SortMode;
use crate::types::results::{StoredResult, THUMBNAIL_SIZE};
use crate::types::tilepalquant::TpqDitherMode;
use crate::types::{AppState, FirstColor};
use crate::ui::styles::UiMarginExt;
use egui::{Color32, Rect, Sense, TextureOptions, Vec2};
use std::collections::HashMap;
use std::sync::mpsc::Sender;

/// Largest palette swatch, matching the palette overlay of the main view.
const SWATCH_MAX: f32 = 16.0;
/// Gap between two swatches, matching the main view.
const SWATCH_SPACING: f32 = 1.0;
/// Full resolution textures held at once. One costs as much memory as the
/// image itself, so only the topmost visible rows get one.
const MAX_FULL_TEXTURES: usize = 8;

/// What the entries being drawn share: the texture cache, which rows turned
/// out to be visible, how many full resolution textures are in use, and where
/// the buttons send their requests.
struct Panel<'a> {
    textures: &'a mut HashMap<u64, ResultTextures>,
    visible: Vec<u64>,
    full_textures: usize,
    sender: &'a Sender<AppStateRequest>,
}

pub fn draw_results_panel(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading_with_margin("Results");

    let current = state.settings_bundle();
    let AppState {
        results,
        results_textures,
        app_state_request_sender,
        ..
    } = state;

    if results.is_empty() {
        results_textures.clear();
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("No results yet").small().weak());
        });
        return;
    }

    let mut panel = Panel {
        textures: results_textures,
        visible: Vec::new(),
        full_textures: 0,
        sender: app_state_request_sender,
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        for entry in results.entries() {
            draw_entry(ui, entry, entry.settings == current, &mut panel);
            ui.add_space(4.0);
        }
    });

    let visible = panel.visible;
    let live: std::collections::HashSet<u64> =
        results.entries().iter().map(|entry| entry.hash).collect();
    results_textures.retain(|hash, textures| {
        if !live.contains(hash) {
            return false;
        }
        if !visible.contains(hash) {
            textures.full = None;
        }
        true
    });
}

/// One entry: image, palette strip, summary line and buttons, inside a frame
/// that carries the accent stroke while its settings are the ones in use.
fn draw_entry(ui: &mut egui::Ui, entry: &StoredResult, selected: bool, panel: &mut Panel) {
    let mut frame = egui::Frame::group(ui.style());
    if selected {
        frame = frame.stroke(egui::Stroke::new(1.0, ui.visuals().selection.stroke.color));
    }

    let drawn = frame.show(ui, |ui| {
        draw_entry_image(ui, entry, panel);
        ui.add_space(2.0);
        draw_palette_strip(ui, entry);
        ui.add_space(2.0);
        ui.label(egui::RichText::new(summary(&entry.settings)).small());
        draw_buttons(ui, entry.hash, panel.sender)
    });

    // The buttons carry their own tooltips; the entry's would stack on top.
    if !drawn.inner {
        drawn.response.on_hover_text(describe(&entry.settings));
    }
}

/// The image at the panel's width, never past 1:1. The texture is only
/// uploaded once the row has scrolled into view, and the full resolution one
/// only while the display size is past the thumbnail's.
fn draw_entry_image(ui: &mut egui::Ui, entry: &StoredResult, panel: &mut Panel) {
    if entry.width == 0 || entry.height == 0 {
        return;
    }

    let width = ui.available_width().min(entry.width as f32).max(1.0);
    let height = width * entry.height as f32 / entry.width as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    panel.visible.push(entry.hash);

    let wants_full = width > THUMBNAIL_SIZE as f32 && panel.full_textures < MAX_FULL_TEXTURES;
    let ctx = ui.ctx().clone();
    let textures = panel.textures.entry(entry.hash).or_default();

    let mut handle = None;
    if wants_full {
        if textures.full.is_none() {
            textures.full = full_texture(&ctx, entry);
        }
        if let Some(full) = &textures.full {
            panel.full_textures += 1;
            handle = Some(full.clone());
        }
    }
    let handle = handle.unwrap_or_else(|| {
        textures
            .thumbnail
            .get_or_insert_with(|| thumbnail_texture(&ctx, entry))
            .clone()
    });

    ui.painter().image(
        handle.id(),
        rect,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

/// One row per palette, shrunk so a whole palette fits the panel width.
fn draw_palette_strip(ui: &mut egui::Ui, entry: &StoredResult) {
    let per_palette = entry.colors_per_palette;
    if per_palette == 0 || entry.palettes.is_empty() {
        return;
    }

    let width = ui.available_width();
    let size = swatch_size(width, per_palette);
    let rows = entry.palettes.len().div_ceil(per_palette);
    let step = size + SWATCH_SPACING;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, rows as f32 * step - SWATCH_SPACING),
        Sense::hover(),
    );
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();
    for (index, color) in entry.palettes.iter().enumerate() {
        let min = rect.min
            + Vec2::new(
                (index % per_palette) as f32 * step,
                (index / per_palette) as f32 * step,
            );
        painter.rect_filled(
            Rect::from_min_size(min, Vec2::splat(size)),
            0.0,
            Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a),
        );
    }
}

/// Apply / Export / Remove. Returns whether one of them is hovered.
fn draw_buttons(ui: &mut egui::Ui, hash: u64, sender: &Sender<AppStateRequest>) -> bool {
    let mut hovered = false;
    ui.horizontal(|ui| {
        let apply = ui.small_button("Apply").on_hover_text("Use these settings");
        if apply.clicked() {
            _ = sender.send(AppStateRequest::ApplyResult { hash });
        }
        hovered |= apply.hovered();

        let export = ui
            .small_button("Export")
            .on_hover_text("Export this image in the selected format");
        if export.clicked() {
            _ = sender.send(AppStateRequest::ExportResult { hash });
        }
        hovered |= export.hovered();

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let remove = ui.small_button("×").on_hover_text("Remove");
            if remove.clicked() {
                _ = sender.send(AppStateRequest::RemoveResult { hash });
            }
            hovered |= remove.hovered();
        });
    });
    hovered
}

/// Swatch size for `colors` swatches in one row of `width`, never above the
/// size the main view uses.
fn swatch_size(width: f32, colors: usize) -> f32 {
    let colors = colors.max(1) as f32;
    let by_width = (width - (colors - 1.0) * SWATCH_SPACING) / colors;
    SWATCH_MAX.min(by_width).max(1.0)
}

fn thumbnail_texture(ctx: &egui::Context, entry: &StoredResult) -> egui::TextureHandle {
    let thumbnail = &entry.thumbnail;
    let size = [thumbnail.width as usize, thumbnail.height as usize];
    ctx.load_texture(
        format!("result-thumb-{:016x}", entry.hash),
        egui::ColorImage::from_rgba_unmultiplied(size, &thumbnail.rgba),
        TextureOptions::NEAREST,
    )
}

fn full_texture(ctx: &egui::Context, entry: &StoredResult) -> Option<egui::TextureHandle> {
    let indexed = match entry.decode() {
        Ok(indexed) => indexed,
        Err(e) => {
            log::error!("Failed to decode a stored result: {e}");
            return None;
        }
    };
    let size = [entry.width as usize, entry.height as usize];
    Some(ctx.load_texture(
        format!("result-full-{:016x}", entry.hash),
        egui::ColorImage::from_rgba_unmultiplied(size, &indexed.to_rgba()),
        TextureOptions::NEAREST,
    ))
}

/// The one line under an entry: engine, palette size and dithering.
fn summary(settings: &SettingsBundle) -> String {
    let qualetize = &settings.qualetize_settings;
    format!(
        "{} {}×{} {}",
        settings.engine.display_name(),
        qualetize.n_palettes,
        qualetize.n_colors,
        dither_name(settings),
    )
}

/// The settings of an entry, one per line, for its tooltip.
fn describe(settings: &SettingsBundle) -> String {
    let qualetize = &settings.qualetize_settings;
    let mut lines = vec![
        settings.engine.display_name().to_string(),
        format!(
            "{} palettes × {} colors",
            qualetize.n_palettes, qualetize.n_colors
        ),
        format!(
            "Tile size: {}×{}",
            qualetize.tile_width, qualetize.tile_height
        ),
        format!(
            "Palette index 0: {}",
            first_color_name(qualetize.first_color())
        ),
        format!("Dithering: {}", dither_name(settings)),
        format!(
            "Color correction: {}",
            on_off(settings.color_correction.enabled)
        ),
    ];
    lines.push(if qualetize.tile_reduce_post_enabled {
        format!(
            "Tile reduction: on, threshold {:.1}",
            qualetize.tile_reduce_post_threshold
        )
    } else {
        "Tile reduction: off".to_string()
    });
    lines.push(match settings.sort_settings.mode {
        SortMode::None => "Palette order: off".to_string(),
        mode => format!(
            "Palette order: {}, {}",
            mode.display_name(),
            settings.sort_settings.order.display_name().to_lowercase()
        ),
    });
    lines.join("\n")
}

/// The dithering of whichever engine the entry used.
fn dither_name(settings: &SettingsBundle) -> String {
    match settings.engine {
        QuantEngine::Qualetize => settings
            .qualetize_settings
            .dither_mode
            .display_name()
            .to_string(),
        QuantEngine::TilePalQuant => match settings.tpq_settings.dither_mode {
            TpqDitherMode::Off => TpqDitherMode::Off.display_name().to_string(),
            mode => format!(
                "{} {}",
                mode.display_name(),
                settings.tpq_settings.dither_pattern.display_name()
            ),
        },
    }
}

fn first_color_name(first_color: FirstColor) -> &'static str {
    match first_color {
        FirstColor::Unique => "Unique",
        FirstColor::Shared => "Shared color",
        FirstColor::TransparentFromAlpha => "Transparent, from alpha",
        FirstColor::TransparentFromColor => "Transparent, from color",
    }
}

fn on_off(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::color_correction::ColorCorrection;
    use crate::types::image::PaletteSortSettings;
    use crate::types::qualetize::QualetizeSettings;
    use crate::types::tilepalquant::DitherPattern;

    fn bundle() -> SettingsBundle {
        SettingsBundle::new(
            QualetizeSettings::default(),
            ColorCorrection::default(),
            PaletteSortSettings::default(),
        )
    }

    #[test]
    fn the_summary_names_the_engine_the_palette_size_and_the_dithering() {
        let mut settings = bundle();
        settings.qualetize_settings.n_palettes = 4;
        settings.qualetize_settings.n_colors = 16;
        settings.qualetize_settings.dither_mode = crate::types::DitherMode::Atkinson;
        assert_eq!(summary(&settings), "qualetize 4×16 Atkinson");
    }

    /// The tilepalquant summary names the dither mode and its pattern, since
    /// that engine's dithering is two settings rather than one.
    #[test]
    fn the_summary_of_a_tilepalquant_result_names_the_dither_pattern() {
        let mut settings = bundle();
        settings.engine = QuantEngine::TilePalQuant;
        settings.tpq_settings.dither_mode = TpqDitherMode::Slow;
        settings.tpq_settings.dither_pattern = DitherPattern::Horizontal4;
        assert!(
            summary(&settings).ends_with("Slow Horizontal 4"),
            "{}",
            summary(&settings)
        );
    }

    #[test]
    fn the_description_lists_the_tile_size_and_the_tile_reduction_threshold() {
        let mut settings = bundle();
        settings.qualetize_settings.tile_width = 8;
        settings.qualetize_settings.tile_height = 16;
        settings.qualetize_settings.tile_reduce_post_enabled = true;
        settings.qualetize_settings.tile_reduce_post_threshold = 12.5;

        let described = describe(&settings);
        let lines: Vec<&str> = described.lines().collect();
        assert!(lines.contains(&"Tile size: 8×16"), "{lines:?}");
        assert!(
            lines.contains(&"Tile reduction: on, threshold 12.5"),
            "{lines:?}"
        );
    }

    #[test]
    fn the_description_reports_a_disabled_palette_order_as_off() {
        let mut settings = bundle();
        settings.sort_settings.mode = SortMode::None;
        assert!(
            describe(&settings)
                .lines()
                .any(|l| l == "Palette order: off")
        );
    }

    /// Swatches shrink to fit a narrow panel and never grow past the size the
    /// main view uses.
    #[test]
    fn swatches_fit_the_panel_width_without_growing_past_the_maximum() {
        assert_eq!(swatch_size(1000.0, 16), SWATCH_MAX);

        let size = swatch_size(120.0, 16);
        assert!(size < SWATCH_MAX, "{size}");
        assert!(
            16.0 * size + 15.0 * SWATCH_SPACING <= 120.0 + 1e-3,
            "{size}"
        );
    }
}
