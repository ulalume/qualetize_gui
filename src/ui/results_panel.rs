//! The Results side panel: the recorded outputs, newest first. Each entry
//! shows its image and the palettes it uses, with the full settings in a
//! hover tooltip. Clicking the image puts those settings back in use;
//! the overlay in its top-right corner removes the entry.
//!
//! A result reaching the top of the list is animated: the space for it opens
//! up first, then it fades in.

use crate::engine::QuantEngine;
use crate::settings_manager::SettingsBundle;
use crate::time::Instant;
use crate::types::app_state::{AppStateRequest, ResultTextures};
use crate::types::image::SortMode;
use crate::types::results::{
    ChangeKind, ResultsAnimation, StoredResult, THUMBNAIL_SIZE, detect_change,
};
use crate::types::tilepalquant::TpqDitherMode;
use crate::types::{AppState, FirstColor};
use crate::ui::styles::UiMarginExt;
use egui::{Align, Color32, Layout, Rect, Sense, TextureOptions, UiBuilder, Vec2};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::time::Duration;

/// Largest palette swatch, matching the palette overlay of the main view.
const SWATCH_MAX: f32 = 16.0;
/// Gap between two swatches, matching the main view.
const SWATCH_SPACING: f32 = 1.0;
/// Full resolution textures held at once. One costs as much memory as the
/// image itself, so only the topmost visible rows get one.
const MAX_FULL_TEXTURES: usize = 8;
/// Side length of the "remove" overlay drawn over an image's top-right corner.
const REMOVE_OVERLAY_SIZE: f32 = 18.0;
/// Gap between the "remove" overlay and the image's top and right edges.
const REMOVE_OVERLAY_INSET: f32 = 2.0;
/// Gap between an image and its palette strip.
const IMAGE_STRIP_GAP: f32 = 1.0;
/// Tallest an entry's image is drawn: a tall image is shrunk to this, and a
/// widened panel enlarges an image no further than this.
const MAX_IMAGE_HEIGHT: f32 = 320.0;
/// Vertical gap after an entry. The list drops the item spacing between its
/// entries, so this is the whole gap.
const ENTRY_SPACING: f32 = 9.0;
/// Length of each of the two phases of an order change: the slots resize,
/// then the entry that reached the top fades in.
const ANIM_PHASE: Duration = Duration::from_millis(300);

/// What the entries being drawn share: the texture cache, which rows turned
/// out to be visible, how many full resolution textures are in use, and
/// where clicks send their requests.
struct Panel<'a> {
    textures: &'a mut HashMap<u64, ResultTextures>,
    visible: Vec<u64>,
    full_textures: usize,
    sender: &'a Sender<AppStateRequest>,
}

/// What an entry reacts to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Interaction {
    /// Clicking the image applies the entry's settings, the overlay removes it.
    Apply,
    /// The entry's settings are the ones in use, so only the overlay reacts.
    RemoveOnly,
    /// Nothing reacts.
    Inert,
}

pub fn draw_results_panel(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading_with_margin("Results");

    let in_use = state.settings_bundle();
    let AppState {
        results,
        results_textures,
        results_view,
        app_state_request_sender,
        ..
    } = state;

    if results.is_empty() {
        results_textures.clear();
        results_view.last_order.clear();
        results_view.animation = None;
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("No results yet").small().weak());
        });
        return;
    }

    let order: Vec<u64> = results.entries().iter().map(|entry| entry.hash).collect();
    if order != results_view.last_order {
        results_view.animation =
            detect_change(&results_view.last_order, &order).map(|kind| ResultsAnimation {
                kind,
                started: Instant::now(),
            });
        results_view.last_order = order;
    }
    // An animation describes the current list only while the entry it moves
    // is at the top of it.
    let animation = results_view.animation.as_ref().and_then(|animation| {
        let elapsed = animation.started.elapsed().as_secs_f32();
        let at_top =
            results.entries().first().map(|entry| entry.hash) == Some(animation.kind.hash());
        (at_top && elapsed < 2.0 * ANIM_PHASE.as_secs_f32()).then_some((animation, elapsed))
    });

    let mut panel = Panel {
        textures: results_textures,
        visible: Vec::new(),
        full_textures: 0,
        sender: app_state_request_sender,
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        // Every entry takes exactly its `entry_height`, so an animated slot
        // and the entry it stands for occupy the same space.
        ui.spacing_mut().item_spacing.y = 0.0;
        match animation {
            Some((animation, elapsed)) => {
                let width = ui.available_width();
                draw_animated(ui, results.entries(), &mut panel, animation, elapsed, width);
            }
            None => {
                for entry in results.entries() {
                    let interaction = if entry.settings.matches(&in_use) {
                        Interaction::RemoveOnly
                    } else {
                        Interaction::Apply
                    };
                    draw_entry(ui, entry, &mut panel, 1.0, interaction);
                }
            }
        }
    });

    if animation.is_some() {
        ui.ctx().request_repaint();
    } else {
        results_view.animation = None;
    }

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

/// The list while its order animates. The entry that reached the top gets a
/// slot of its own there; the others are drawn in the order they had before,
/// and no entry reacts to the pointer.
///
/// In the first phase the top slot grows from nothing, which pushes the
/// entries below it down; the entry that moved shrinks away at the place it
/// held, so the entries under that place stay where they are. In the second
/// phase the entry fades in inside the top slot.
fn draw_animated(
    ui: &mut egui::Ui,
    entries: &[StoredResult],
    panel: &mut Panel,
    animation: &ResultsAnimation,
    elapsed: f32,
    width: f32,
) {
    let Some((moved, rest)) = entries.split_first() else {
        return;
    };
    let phase = ANIM_PHASE.as_secs_f32();
    let height = entry_height(moved, width);

    if elapsed >= phase {
        let alpha = (elapsed - phase) / phase;
        slot(ui, width, height, |ui| {
            draw_entry(ui, moved, panel, alpha, Interaction::Inert);
        });
        for entry in rest {
            draw_entry(ui, entry, panel, 1.0, Interaction::Inert);
        }
        return;
    }

    let progress = elapsed / phase;
    let opened = ease_out(progress);
    slot(ui, width, height * opened, |_| {});
    let old_index = match animation.kind {
        ChangeKind::Added { hash: _ } => None,
        ChangeKind::Moved { hash: _, old_index } => Some(old_index.min(rest.len())),
    };
    for index in 0..=rest.len() {
        if old_index == Some(index) {
            slot(ui, width, height * (1.0 - opened), |ui| {
                draw_entry(ui, moved, panel, 1.0 - progress, Interaction::Inert);
            });
        }
        if let Some(entry) = rest.get(index) {
            draw_entry(ui, entry, panel, 1.0, Interaction::Inert);
        }
    }
}

/// Draw `add` in a slot of exactly `height`, clipped to it, so an entry taller
/// than its slot is cut off instead of pushing the next one down.
fn slot(ui: &mut egui::Ui, width: f32, height: f32, add: impl FnOnce(&mut egui::Ui)) {
    if height <= 0.0 {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let clip = rect.intersect(ui.clip_rect());
    let mut inner = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    inner.set_clip_rect(clip);
    add(&mut inner);
}

/// Ease-out cubic.
fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// One entry: image (with its palette strip below) and the remove overlay.
fn draw_entry(
    ui: &mut egui::Ui,
    entry: &StoredResult,
    panel: &mut Panel,
    alpha: f32,
    interaction: Interaction,
) {
    ui.scope(|ui| {
        // Exactly one point between the image and its palette strip.
        ui.spacing_mut().item_spacing.y = 0.0;
        draw_entry_image(ui, entry, panel, alpha, interaction);
        ui.add_space(IMAGE_STRIP_GAP);
        draw_palette_strip(ui, entry, alpha);
    });
    ui.add_space(ENTRY_SPACING);
}

/// The size the image of `entry` is drawn at in a panel `width` wide: the full
/// width, unless that would make it taller than [`MAX_IMAGE_HEIGHT`].
fn image_size(entry: &StoredResult, width: f32) -> Vec2 {
    if entry.width == 0 || entry.height == 0 {
        return Vec2::ZERO;
    }
    let aspect = entry.height as f32 / entry.width as f32;
    let width = width.max(1.0);
    let height = width * aspect;
    if height > MAX_IMAGE_HEIGHT {
        Vec2::new(MAX_IMAGE_HEIGHT / aspect, MAX_IMAGE_HEIGHT)
    } else {
        Vec2::new(width, height)
    }
}

/// The height of the palette strip of `entry` in a panel `width` wide, the
/// gaps between its rows included.
fn strip_height(entry: &StoredResult, width: f32) -> f32 {
    let width = image_size(entry, width).x;
    let per_palette = entry.colors_per_palette;
    if per_palette == 0 || entry.palettes.is_empty() {
        return 0.0;
    }
    let rows = entry.palettes.len().div_ceil(per_palette) as f32;
    rows * (swatch_size(width, per_palette) + SWATCH_SPACING) - SWATCH_SPACING
}

/// The vertical space [`draw_entry`] takes in a panel `width` wide.
fn entry_height(entry: &StoredResult, width: f32) -> f32 {
    image_size(entry, width).y + IMAGE_STRIP_GAP + strip_height(entry, width) + ENTRY_SPACING
}

/// The image at the panel's width, capped in height, with a small "remove"
/// overlay at its top-right corner. Clicking the image applies the entry's
/// settings; clicking the overlay removes it instead. The texture is only
/// uploaded once the row has scrolled into view, and the full resolution one
/// only while the display size is past the thumbnail's.
fn draw_entry_image(
    ui: &mut egui::Ui,
    entry: &StoredResult,
    panel: &mut Panel,
    alpha: f32,
    interaction: Interaction,
) {
    let size = image_size(entry, ui.available_width());
    if size == Vec2::ZERO {
        return;
    }

    let sense = if interaction == Interaction::Apply {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, image_response) = ui.allocate_exact_size(size, sense);
    if !ui.is_rect_visible(rect) {
        return;
    }
    panel.visible.push(entry.hash);

    let wants_full = size.x > THUMBNAIL_SIZE as f32 && panel.full_textures < MAX_FULL_TEXTURES;
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
        Color32::WHITE.gamma_multiply(alpha),
    );

    let removed = interaction != Interaction::Inert && draw_remove_overlay(ui, entry, rect);
    if removed {
        _ = panel
            .sender
            .send(AppStateRequest::RemoveResult { hash: entry.hash });
    } else if image_response.clicked() {
        _ = panel
            .sender
            .send(AppStateRequest::ApplyResult { hash: entry.hash });
    }

    image_response.on_hover_text(describe(&entry.settings));
}

/// The "remove" overlay over the top-right corner of `image`, `true` when it
/// was clicked.
fn draw_remove_overlay(ui: &mut egui::Ui, entry: &StoredResult, image: Rect) -> bool {
    let overlay_rect = Rect::from_min_size(
        egui::pos2(
            image.right() - REMOVE_OVERLAY_SIZE - REMOVE_OVERLAY_INSET,
            image.top() + REMOVE_OVERLAY_INSET,
        ),
        Vec2::splat(REMOVE_OVERLAY_SIZE),
    );
    // Drawn by hand rather than as a widget so the layout cursor stays at
    // the image's bottom edge: a translucent disc with the glyph on it.
    let remove = ui
        .interact(
            overlay_rect,
            ui.make_persistent_id(("result-remove", entry.hash)),
            Sense::click(),
        )
        .on_hover_text("Remove");
    // Fixed colors rather than the theme's: the disc sits on the image, not
    // on the panel, so it has to read the same in both themes.
    let (fill, text_color) = if remove.hovered() {
        (Color32::from_black_alpha(200), Color32::WHITE)
    } else {
        (Color32::from_black_alpha(120), Color32::from_gray(230))
    };
    let painter = ui.painter();
    painter.circle_filled(overlay_rect.center(), REMOVE_OVERLAY_SIZE / 2.0, fill);
    painter.text(
        overlay_rect.center(),
        egui::Align2::CENTER_CENTER,
        "×",
        egui::FontId::proportional(14.0),
        text_color,
    );
    remove.clicked()
}

/// One row per palette, shrunk so a whole palette fits the panel width.
fn draw_palette_strip(ui: &mut egui::Ui, entry: &StoredResult, alpha: f32) {
    let panel_width = ui.available_width();
    let height = strip_height(entry, panel_width);
    // As wide as the image above it.
    let width = image_size(entry, panel_width).x;
    if height <= 0.0 {
        return;
    }

    let per_palette = entry.colors_per_palette;
    let size = swatch_size(width, per_palette);
    let step = size + SWATCH_SPACING;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
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
            Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
                .gamma_multiply(alpha),
        );
    }
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
    use crate::types::BGRA8;
    use crate::types::color_correction::ColorCorrection;
    use crate::types::image::{ImageDataIndexed, PaletteSortSettings};
    use crate::types::qualetize::QualetizeSettings;
    use crate::types::results::Results;
    use crate::types::tilepalquant::DitherPattern;

    fn bundle() -> SettingsBundle {
        SettingsBundle::new(
            QualetizeSettings::default(),
            ColorCorrection::default(),
            PaletteSortSettings::default(),
        )
    }

    /// The tilepalquant description names the dither mode and its pattern,
    /// since that engine's dithering is two settings rather than one.
    #[test]
    fn the_description_of_a_tilepalquant_result_names_the_dither_pattern() {
        let mut settings = bundle();
        settings.engine = QuantEngine::TilePalQuant;
        settings.tpq_settings.dither_mode = TpqDitherMode::Slow;
        settings.tpq_settings.dither_pattern = DitherPattern::Horizontal4;
        assert!(
            describe(&settings).contains("Dithering: Slow Horizontal 4"),
            "{}",
            describe(&settings)
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

    /// One recorded result of `width`x`height` over a palette of `colors`
    /// colors laid out `per_palette` to a row.
    fn recorded(width: u32, height: u32, colors: usize, per_palette: usize) -> Results {
        let pixels = width as usize * height as usize;
        let indexed = ImageDataIndexed::new(
            vec![
                BGRA8 {
                    b: 0,
                    g: 0,
                    r: 0,
                    a: 255,
                };
                colors
            ],
            per_palette,
            vec![0; pixels],
        );
        let mut results = Results::default();
        results.record(
            &indexed,
            &vec![0; pixels * 4],
            width,
            height,
            bundle(),
            Instant::now(),
        );
        results
    }

    /// The height an entry is given is the height it takes: its image, the
    /// gap, its palette strip and the spacing after it.
    #[test]
    fn an_entry_is_as_tall_as_the_parts_it_is_drawn_from() {
        let results = recorded(64, 32, 16, 8);
        let entry = &results.entries()[0];

        for width in [200.0, 40.0] {
            let parts = image_size(entry, width).y
                + IMAGE_STRIP_GAP
                + strip_height(entry, width)
                + ENTRY_SPACING;
            assert!((entry_height(entry, width) - parts).abs() < 1e-4, "{width}");
        }

        // 64 points wide in a 200 point panel: 1:1, so 32 points tall. Two
        // rows of 8 swatches at the largest size: 2 * (16 + 1) - 1 = 33.
        assert!((entry_height(entry, 200.0) - (32.0 + 1.0 + 33.0 + ENTRY_SPACING)).abs() < 1e-4);
        // Narrower than the image: half the size, with swatches shrunk to fit.
        let swatch = (40.0 - 7.0 * SWATCH_SPACING) / 8.0;
        let strip = 2.0 * (swatch + SWATCH_SPACING) - SWATCH_SPACING;
        assert!((entry_height(entry, 40.0) - (20.0 + 1.0 + strip + ENTRY_SPACING)).abs() < 1e-4);
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
