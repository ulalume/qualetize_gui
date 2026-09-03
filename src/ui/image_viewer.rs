use crate::engine::QuantEngine;
use crate::types::app_state::AppStateRequest;
use crate::types::{AppState, ImageData, app_state::Toast};
use egui::{Align2, Color32, ColorImage, FontId, Id, Pos2, Rect, Vec2};

/// Gap between the image panels.
const PANEL_MARGIN: f32 = 4.0;

/// Zoom, pan, and background shared by every panel drawn in a frame, so each
/// [`ImagePanel`] literal only needs to name what actually differs between
/// panels.
struct ViewParams {
    zoom: f32,
    pan_offset: Vec2,
    background: Color32,
}

/// Fields an [`ImagePanel`] usually leaves at their default: most panels have
/// no spinner, overlay text, notice, or palette overlay.
#[derive(Default)]
struct PanelExtras<'a> {
    has_spinner: bool,
    /// Percent shown next to the spinner. Only tilepalquant reports progress;
    /// Qualetize has no equivalent, so this stays `None` for it.
    progress_percent: Option<u8>,
    overlay_text: Option<&'a str>,
    /// Persistent warning drawn in the top left corner of the panel.
    notice: Option<&'a str>,
    palettes: Option<&'a [Vec<Color32>]>,
}

/// Everything one image panel needs to draw itself.
struct ImagePanel<'a> {
    title: &'a str,
    image: Option<&'a ImageData>,
    size: Vec2,
    view: &'a ViewParams,
    extras: PanelExtras<'a>,
}

pub fn draw_image_view(ui: &mut egui::Ui, state: &mut AppState, qualetize_processing: bool) {
    let available_size = ui.available_size();
    let toast = live_toast(&mut state.tile_reduce_toast, ui.ctx());
    let fit_toast = live_toast(&mut state.tile_fit_toast, ui.ctx());
    let fit_notice = state.tile_fit_notice();
    // The Original view shows what the pipeline actually consumes, so the added
    // border is visible right next to the notice explaining it, and all four
    // panels share the same dimensions at a given zoom.
    let original_image = state.processing_input();

    let view = ViewParams {
        zoom: state.zoom,
        pan_offset: state.pan_offset,
        background: state
            .preferences
            .background_color
            .unwrap_or(Color32::from_gray(64)),
    };
    let mut view_change = ViewChange::default();

    // Left column holds the inputs, right column the outputs. The optional
    // panels appear exactly when their pass is enabled.
    let show_color_corrected = state.color_correction.enabled;
    let show_tile_reduced = state.settings.tile_reduce_post_enabled;

    // The palette is identical for both output panels, so it is only drawn on
    // the Qualetized one to keep its position stable.
    let palettes: Option<&[Vec<Color32>]> = if state.preferences.show_palettes {
        state
            .output_palette_sorted_indexed_image
            .as_ref()
            .or_else(|| state.output_image.as_ref().and_then(|i| i.indexed.as_ref()))
            .map(|indexed| indexed.palettes_for_ui.as_slice())
    } else {
        None
    };

    let column_width = (available_size.x - PANEL_MARGIN) / 2.0;
    let input_height = column_height(available_size.y, 1 + usize::from(show_color_corrected));
    let output_height = column_height(available_size.y, 1 + usize::from(show_tile_reduced));

    ui.horizontal(|ui| {
        ui.style_mut().spacing.item_spacing = egui::vec2(PANEL_MARGIN, 0.0);

        // Inputs
        ui.vertical(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(0.0, PANEL_MARGIN);

            draw_image_panel(
                ui,
                ImagePanel {
                    title: "Original",
                    image: original_image,
                    size: Vec2::new(column_width, input_height),
                    view: &view,
                    extras: PanelExtras {
                        overlay_text: fit_toast.as_deref(),
                        notice: fit_notice.as_deref(),
                        ..Default::default()
                    },
                },
                &mut view_change,
            );

            if show_color_corrected {
                draw_image_panel(
                    ui,
                    ImagePanel {
                        title: "Color corrected",
                        image: state.color_corrected_image.as_ref(),
                        size: Vec2::new(column_width, input_height),
                        view: &view,
                        extras: PanelExtras {
                            has_spinner: state.color_corrected_image.is_none(),
                            ..Default::default()
                        },
                    },
                    &mut view_change,
                );
            }
        });

        // Outputs
        ui.vertical(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(0.0, PANEL_MARGIN);

            draw_image_panel(
                ui,
                ImagePanel {
                    title: "Qualetized",
                    image: state.base_output_image.as_ref(),
                    size: Vec2::new(column_width, output_height),
                    view: &view,
                    extras: PanelExtras {
                        has_spinner: qualetize_processing,
                        // Qualetize does not report progress, so this only
                        // ever shows a percent for tilepalquant.
                        progress_percent: (qualetize_processing
                            && state.engine == QuantEngine::TilePalQuant)
                            .then_some(state.quantize_progress)
                            .flatten(),
                        palettes,
                        ..Default::default()
                    },
                },
                &mut view_change,
            );

            if show_tile_reduced {
                draw_image_panel(
                    ui,
                    ImagePanel {
                        title: "Tile reduced",
                        image: state.output_image.as_ref(),
                        size: Vec2::new(column_width, output_height),
                        view: &view,
                        extras: PanelExtras {
                            // A new quantization invalidates the reduced result too.
                            has_spinner: qualetize_processing || state.tile_reduce_processing,
                            overlay_text: toast.as_deref(),
                            ..Default::default()
                        },
                    },
                    &mut view_change,
                );
            }
        });
    });

    state.pan_offset += view_change.pan;
    if let Some(zoom) = view_change.zoom {
        state.zoom = zoom;
    }
}

/// What the panels asked to change about the shared view this frame:
/// drags accumulate into `pan`, a scroll over a panel sets `zoom` and adds
/// the pan that keeps the pixel under the cursor in place.
#[derive(Default)]
struct ViewChange {
    pan: Vec2,
    zoom: Option<f32>,
}

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 20.0;

/// Zoom in or out around `cursor`, which is given relative to the image's
/// current center on screen. Returns the new zoom and the pan adjustment
/// that keeps the image point under the cursor where it is.
///
/// The factor is exponential in the scroll distance so scrolling up and
/// then down by the same amount lands back on the same zoom, and a large
/// downward scroll can never flip the factor negative.
fn zoom_about(zoom: f32, scroll_delta: f32, cursor: Vec2) -> (f32, Vec2) {
    let new_zoom = (zoom * (scroll_delta * 0.001).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
    // The point under the cursor is `cursor / zoom` image pixels from the
    // center; after zooming it would sit at `cursor * new_zoom / zoom`, so
    // the center has to move by the difference.
    let pan = cursor * (1.0 - new_zoom / zoom);
    (new_zoom, pan)
}

/// Height of one panel in a column of `panel_count` stacked panels.
fn column_height(available_height: f32, panel_count: usize) -> f32 {
    let gaps = PANEL_MARGIN * (panel_count.saturating_sub(1)) as f32;
    (available_height - gaps) / panel_count as f32
}

const TOAST_DURATION: std::time::Duration = std::time::Duration::from_secs(3);

/// Return a toast while it is still within its display window, dropping it once
/// it has expired. Schedules the repaint that makes it vanish.
fn live_toast(slot: &mut Option<Toast>, ctx: &egui::Context) -> Option<String> {
    let toast = slot.as_ref()?;
    let Some(remaining) = TOAST_DURATION.checked_sub(toast.time.elapsed()) else {
        *slot = None;
        return None;
    };

    let message = toast.message.clone();
    ctx.request_repaint_after(remaining);
    Some(message)
}

/// The sample images offered on the welcome screen, with their file names.
const SAMPLES: [(&str, &[u8]); 2] = [
    ("cat.png", include_bytes!("../../assets/sample/cat.png")),
    ("lenna.png", include_bytes!("../../assets/sample/lenna.png")),
];
/// Longest side of a sample thumbnail, in points.
const SAMPLE_THUMBNAIL_SIZE: f32 = 128.0;

/// The welcome screen shown while no image is loaded: how to open one, and
/// samples to try.
pub fn draw_main_content(ui: &mut egui::Ui, state: &mut AppState) {
    if state.sample_thumbnails.is_empty() {
        state.sample_thumbnails = SAMPLES
            .iter()
            .map(|(name, bytes)| {
                let image = image::load_from_memory(bytes)
                    .expect("the bundled sample images decode")
                    .to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let color_image = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                ui.ctx()
                    .load_texture(*name, color_image, egui::TextureOptions::NEAREST)
            })
            .collect();
    }
    let thumbnails = state.sample_thumbnails.clone();

    let thumbnail_size = |texture: &egui::TextureHandle| {
        let [w, h] = texture.size();
        let scale = SAMPLE_THUMBNAIL_SIZE / w.max(h) as f32;
        Vec2::new(w as f32 * scale, h as f32 * scale)
    };

    // The whole block is centered in the view.
    let block_height = 24.0 + 24.0 + 20.0 + 8.0 + SAMPLE_THUMBNAIL_SIZE;
    ui.add_space(((ui.available_height() - block_height) / 2.0).max(0.0));
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            let row_width = 250.0 + 100.0;
            ui.add_space(((ui.available_width() - row_width) / 2.0).max(0.0));
            ui.heading("📁 Drop an image file here or");
            if ui.button("Open image...").clicked() {
                _ = state
                    .app_state_request_sender
                    .send(AppStateRequest::OpenImageDialog);
            }
        });
        ui.add_space(24.0);
        ui.label("or try a sample image");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let row_width: f32 = thumbnails.iter().map(|t| thumbnail_size(t).x + 8.0).sum();
            ui.add_space(((ui.available_width() - row_width) / 2.0).max(0.0));
            for ((name, bytes), texture) in SAMPLES.iter().zip(&thumbnails) {
                let button = egui::Button::image(
                    egui::Image::new(texture).fit_to_exact_size(thumbnail_size(texture)),
                );
                if ui.add(button).on_hover_text(*name).clicked() {
                    _ = state
                        .app_state_request_sender
                        .send(AppStateRequest::LoadImageBytes {
                            name: (*name).to_string(),
                            bytes: bytes.to_vec(),
                        });
                }
            }
        });
    });
}

fn draw_background_and_pixels(painter: &egui::Painter, canvas: Rect, base_color: Color32) {
    painter.rect_filled(canvas, 0.0, base_color);

    const MAGNIFICATION_PIXEL_SIZE: f32 = 24.0;
    let canvas_min_x = canvas.min.x % MAGNIFICATION_PIXEL_SIZE;
    let canvas_min_y = canvas.min.y % MAGNIFICATION_PIXEL_SIZE;
    let pixel_radius = 1.25;
    let pixel_color = Color32::from_rgba_unmultiplied(
        (base_color.r() as f32 * 1.5) as u8,
        (base_color.g() as f32 * 1.5) as u8,
        (base_color.b() as f32 * 1.5) as u8,
        base_color.a(),
    );

    // `canvas.center() + vec2(-w/2, -h/2)` is just `canvas.min`; the dot grid
    // is anchored there, offset back by the canvas's own phase within one dot
    // cell so the grid appears to pan continuously under `canvas`.
    let origin = canvas.min - egui::vec2(canvas_min_x, canvas_min_y);
    let nx = (canvas.width() / MAGNIFICATION_PIXEL_SIZE).ceil() as i32 + 1;
    let ny = (canvas.height() / MAGNIFICATION_PIXEL_SIZE).ceil() as i32 + 1;

    for yi in 0..ny {
        for xi in 0..nx {
            painter.circle_filled(
                origin
                    + egui::vec2(
                        (xi as f32 + 0.5) * MAGNIFICATION_PIXEL_SIZE,
                        (yi as f32 + 0.5) * MAGNIFICATION_PIXEL_SIZE,
                    ),
                pixel_radius,
                pixel_color,
            );
        }
    }
}

fn draw_main_image(
    painter: &egui::Painter,
    canvas: Rect,
    image_data: Option<&ImageData>,
    zoom: f32,
    pan_offset: Vec2,
) {
    if let Some(image_data) = image_data {
        let original_size = egui::vec2(image_data.width as f32, image_data.height as f32);
        let image_rect = calculate_image_rect(&canvas, original_size, zoom, pan_offset);

        painter.image(
            image_data.texture.id(),
            image_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

/// A small text chip drawn over the image, used for titles and notices.
fn draw_label_chip(
    painter: &egui::Painter,
    ui_ctx: &egui::Context,
    top_left: Pos2,
    text: &str,
    text_color: Color32,
) {
    let window_color = ui_ctx.global_style().visuals.window_fill();
    let bg_color =
        Color32::from_rgba_unmultiplied(window_color.r(), window_color.g(), window_color.b(), 178);

    let galley =
        ui_ctx.fonts_mut(|f| f.layout_no_wrap(text.to_string(), FontId::default(), text_color));
    let rect = Rect::from_min_size(
        top_left - egui::vec2(2.0, 1.0),
        galley.size() + egui::vec2(4.0, 2.0),
    );

    painter.rect_filled(rect, 0.0, bg_color);
    painter.galley(top_left, galley, text_color);
}

fn draw_title(painter: &egui::Painter, canvas: Rect, title: &str, ui_ctx: &egui::Context) {
    if title.is_empty() {
        return;
    }

    // `text_color()` already honors `override_text_color`, so there is no need
    // to clone `Visuals` just to read it back off.
    let text_color = ui_ctx.global_style().visuals.text_color();
    let pos = canvas.left_bottom() + Vec2::new(4.0, -20.0);
    draw_label_chip(painter, ui_ctx, pos, title, text_color);
}

/// Persistent notice in the top left corner, for state the user should keep
/// seeing rather than a toast that fades.
fn draw_notice(painter: &egui::Painter, canvas: Rect, text: &str, ui_ctx: &egui::Context) {
    let pos = canvas.left_top() + Vec2::new(4.0, 4.0);
    let color = super::styles::warning_color(&ui_ctx.global_style().visuals);
    draw_label_chip(painter, ui_ctx, pos, text, color);
}

fn draw_spinner(painter: &egui::Painter, canvas: Rect, ui_ctx: &egui::Context) {
    let center = canvas.center();
    let radius = 16.0;
    let num_lines = 12;
    let time = ui_ctx.input(|i| i.time) as f32;

    for i in 0..num_lines {
        let angle = i as f32 / num_lines as f32 * std::f32::consts::TAU + time;
        let start = center + egui::vec2(angle.cos(), angle.sin()) * radius * 0.5;
        let end = center + egui::vec2(angle.cos(), angle.sin()) * radius;
        painter.line_segment([start, end], (2.5, Color32::LIGHT_GRAY));
    }
}

/// Percent chip drawn beside the spinner while tilepalquant reports progress.
fn draw_progress_percent(
    painter: &egui::Painter,
    canvas: Rect,
    ui_ctx: &egui::Context,
    percent: u8,
) {
    let text_color = ui_ctx.global_style().visuals.text_color();
    let pos = canvas.center() + Vec2::new(22.0, -7.0);
    draw_label_chip(painter, ui_ctx, pos, &format!("{percent}%"), text_color);
}

fn draw_overlay_text(painter: &egui::Painter, canvas: Rect, ui_ctx: &egui::Context, text: &str) {
    let style = ui_ctx.global_style();
    let visuals = &style.visuals;
    let panel = visuals.panel_fill;
    // Use theme-aware background with slight translucency
    let bg_color = Color32::from_rgba_unmultiplied(panel.r(), panel.g(), panel.b(), 200);
    let text_color = visuals.strong_text_color();
    let font_id = egui::FontId::proportional(15.0); // slightly larger to emphasize toast text

    let galley = ui_ctx.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font_id, text_color));
    let rect = Align2::CENTER_CENTER.align_size_within_rect(
        galley.size() + egui::vec2(12.0, 6.0),
        Rect::from_center_size(canvas.center(), galley.size() + egui::vec2(12.0, 6.0)),
    );
    painter.rect_filled(rect, 4.0, bg_color);
    painter.galley(rect.center() - galley.size() * 0.5, galley, text_color);
}

fn draw_image_panel(ui: &mut egui::Ui, panel: ImagePanel, view_change: &mut ViewChange) {
    ui.allocate_ui_with_layout(
        panel.size,
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            let (response, painter) =
                ui.allocate_painter(panel.size, egui::Sense::click_and_drag());
            let canvas = response.rect;

            draw_background_and_pixels(&painter, canvas, panel.view.background);
            draw_main_image(
                &painter,
                canvas,
                panel.image,
                panel.view.zoom,
                panel.view.pan_offset,
            );
            draw_title(&painter, canvas, panel.title, ui.ctx());
            if let Some(notice) = panel.extras.notice {
                draw_notice(&painter, canvas, notice, ui.ctx());
            }

            if let Some(palettes) = panel.extras.palettes {
                draw_palettes_overlay(&painter, canvas, palettes);
            }
            if panel.extras.has_spinner {
                draw_spinner(&painter, canvas, ui.ctx());
                if let Some(percent) = panel.extras.progress_percent {
                    draw_progress_percent(&painter, canvas, ui.ctx(), percent);
                }
            }
            if let Some(text) = panel.extras.overlay_text {
                draw_overlay_text(&painter, canvas, ui.ctx(), text);
            }

            // Panning
            if response.dragged() {
                view_change.pan += response.drag_delta();
            }

            // Zooming, anchored on the pixel under the cursor
            if let Some(cursor) = response.hover_pos() {
                let scroll_delta = ui.ctx().input(|i| i.smooth_scroll_delta.y);
                if scroll_delta != 0.0 {
                    let image_center = canvas.center() + panel.view.pan_offset;
                    let (zoom, pan) =
                        zoom_about(panel.view.zoom, scroll_delta, cursor - image_center);
                    view_change.zoom = Some(zoom);
                    view_change.pan += pan;
                }
            }
        },
    );
}

fn calculate_image_rect(
    available_rect: &Rect,
    original_size: Vec2,
    zoom: f32,
    pan_offset: Vec2,
) -> Rect {
    let display_size = original_size * zoom;
    let view_center = available_rect.center() + pan_offset;
    Rect::from_center_size(view_center, display_size)
}

/// Rect of one swatch in a palette row: `idx` within a palette of
/// `palette_len` colors, growing leftwards from `origin` (the row's right
/// edge, at its top).
fn swatch_rect(origin: Pos2, palette_len: usize, idx: usize, size: f32, spacing: f32) -> Rect {
    let width = (palette_len as f32) * (size + spacing) - spacing;
    let x = origin.x - width + (idx as f32) * (size + spacing);
    Rect::from_min_size(Pos2::new(x, origin.y), Vec2::new(size, size))
}

fn draw_palettes_overlay(painter: &egui::Painter, rect: Rect, palettes: &[Vec<egui::Color32>]) {
    if palettes.is_empty() {
        return;
    }

    let ctx = painter.ctx();
    let pointer_pos = ctx.pointer_hover_pos();
    let mut hovered: Option<(usize, usize)> = None;

    let palette_margin = 8.0;
    let palette_spacing = 1.0;
    let palette_size = calculate_palette_size(&rect, palettes, palette_margin, palette_spacing);

    let start_x = rect.max.x - palette_margin;
    let mut current_y = rect.min.y + palette_margin;

    if let Some(pos) = pointer_pos {
        let mut hover_y = current_y;
        'outer: for (palette_idx, palette) in palettes.iter().enumerate() {
            for (color_idx, _) in palette.iter().enumerate() {
                let color_rect = swatch_rect(
                    Pos2::new(start_x, hover_y),
                    palette.len(),
                    color_idx,
                    palette_size,
                    palette_spacing,
                );
                if color_rect.contains(pos) {
                    hovered = Some((palette_idx, color_idx));
                    break 'outer;
                }
            }
            hover_y += palette_size + palette_spacing;
        }
    }

    current_y = rect.min.y + palette_margin;
    for (palette_idx, palette) in palettes.iter().enumerate() {
        draw_single_palette(
            painter,
            palette_idx,
            palette,
            Pos2::new(start_x, current_y),
            palette_size,
            palette_spacing,
            hovered,
        );

        current_y += palette_size + palette_spacing;
    }

    if let Some((palette_idx, color_idx)) = hovered {
        let color = palettes[palette_idx][color_idx];
        let hex = if color.a() == 255 {
            format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
        } else {
            format!(
                "#{:02X}{:02X}{:02X}{:02X}",
                color.a(),
                color.r(),
                color.g(),
                color.b()
            )
        };
        let rgba = if color.a() == 255 {
            format!("RGB({},{},{})", color.r(), color.g(), color.b())
        } else {
            format!(
                "RGBA({},{},{},{})",
                color.r(),
                color.g(),
                color.b(),
                color.a()
            )
        };

        if let Some(pointer_pos) = pointer_pos {
            egui::Tooltip::always_open(
                ctx.clone(),
                painter.layer_id(),
                Id::new("palette_chip_tooltip"),
                pointer_pos,
            )
            .show(|ui| {
                ui.set_width(160.0);
                ui.label(format!("Palette {} / index {}", palette_idx, color_idx));
                ui.label(hex);
                ui.label(rgba);
            });
        }
    }
}

/// Size of one palette swatch: bounded so a full row fits the panel's width
/// and, since palettes stack vertically, so all `palettes.len()` rows fit its
/// height too.
fn calculate_palette_size(
    rect: &Rect,
    palettes: &[Vec<egui::Color32>],
    palette_margin: f32,
    palette_spacing: f32,
) -> f32 {
    let Some(first_palette) = palettes.first() else {
        return 8.0;
    };

    let by_width = (rect.width()
        - palette_margin * 2.0
        - ((first_palette.len() as f32) - 1.0) * palette_spacing)
        / (first_palette.len() as f32);

    let by_height =
        (rect.height() - palette_margin * 2.0) / (palettes.len() as f32) - palette_spacing;

    4.0_f32.max(16.0_f32.min(by_width.min(by_height)))
}

fn draw_single_palette(
    painter: &egui::Painter,
    palette_idx: usize,
    palette: &[egui::Color32],
    origin: Pos2,
    palette_size: f32,
    palette_spacing: f32,
    hovered: Option<(usize, usize)>,
) {
    let highlight_color = painter.ctx().global_style().visuals.selection.stroke.color;

    for (color_idx, &color) in palette.iter().enumerate() {
        let color_rect = swatch_rect(
            origin,
            palette.len(),
            color_idx,
            palette_size,
            palette_spacing,
        );

        painter.rect_filled(color_rect, 0.0, color);
        painter.rect_stroke(
            color_rect,
            0.0,
            egui::Stroke::new(
                1.0_f32,
                if hovered
                    .map(|(p_idx, c_idx)| p_idx == palette_idx && c_idx == color_idx)
                    .unwrap_or(false)
                {
                    highlight_color
                } else {
                    Color32::from_gray(48)
                },
            ),
            egui::StrokeKind::Middle,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_panel_fills_the_column() {
        assert_eq!(column_height(400.0, 1), 400.0);
    }

    #[test]
    fn two_panels_share_the_column_minus_the_gap() {
        assert_eq!(column_height(404.0, 2), 200.0);
    }

    #[test]
    fn columns_with_different_panel_counts_stay_within_the_view() {
        let available = 500.0;
        for panel_count in 1..=4 {
            let height = column_height(available, panel_count);
            let used =
                height * panel_count as f32 + PANEL_MARGIN * panel_count.saturating_sub(1) as f32;
            assert!(
                (used - available).abs() < 1e-3,
                "{panel_count} panels: {used}"
            );
        }
    }

    /// The image point under the cursor must not move when zooming: its
    /// screen offset from the image center scales with the zoom, and the pan
    /// has to cancel that exactly.
    #[test]
    fn zooming_keeps_the_pixel_under_the_cursor_in_place() {
        let cursor = Vec2::new(120.0, -40.0);
        for (zoom, scroll) in [(1.0, 500.0), (4.0, -800.0), (0.5, 1000.0)] {
            let (new_zoom, pan) = zoom_about(zoom, scroll, cursor);
            let image_point = cursor / zoom;
            let after = pan + image_point * new_zoom;
            assert!(
                (after - cursor).length() < 1e-3,
                "zoom {zoom} scroll {scroll}"
            );
        }
    }

    #[test]
    fn zoom_is_symmetric_and_clamped() {
        let (up, _) = zoom_about(1.0, 700.0, Vec2::ZERO);
        let (back, _) = zoom_about(up, -700.0, Vec2::ZERO);
        assert!((back - 1.0).abs() < 1e-5);

        let (floor, _) = zoom_about(0.2, -100_000.0, Vec2::ZERO);
        assert_eq!(floor, MIN_ZOOM);
        let (ceiling, _) = zoom_about(10.0, 100_000.0, Vec2::ZERO);
        assert_eq!(ceiling, MAX_ZOOM);
    }

    #[test]
    fn palette_size_shrinks_to_fit_many_rows_in_a_short_panel() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 60.0));
        let palettes: Vec<Vec<Color32>> = (0..16).map(|_| vec![Color32::BLACK; 4]).collect();
        let size = calculate_palette_size(&rect, &palettes, 8.0, 1.0);
        // 16 rows in 60px tall (minus margins) leaves well under 16px per row.
        assert!(size < 16.0, "expected a shrunk size, got {size}");
        assert!(
            size >= 4.0,
            "size should not go below the minimum, got {size}"
        );
    }
}
