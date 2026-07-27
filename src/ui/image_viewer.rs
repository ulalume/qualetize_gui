use super::styles::UiMarginExt;
use crate::types::{AppState, ImageData};
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Vec2};

/// Gap between the image panels.
const PANEL_MARGIN: f32 = 4.0;

/// Everything one image panel needs to draw itself.
struct ImagePanel<'a> {
    title: &'a str,
    image: &'a Option<ImageData>,
    size: Vec2,
    zoom: f32,
    pan_offset: Vec2,
    background: Color32,
    has_spinner: bool,
    overlay_text: Option<&'a str>,
    palettes: Option<&'a Vec<Vec<Color32>>>,
}

pub fn draw_image_view(ui: &mut egui::Ui, state: &mut AppState, qualetize_processing: bool) {
    let available_size = ui.available_size();
    let toast = live_toast(state, ui.ctx());

    let zoom = state.zoom;
    let pan_offset = state.pan_offset;
    let background = state
        .preferences
        .background_color
        .unwrap_or(Color32::from_gray(64));
    let mut pan_changed = Vec2::ZERO;

    // Left column holds the inputs, right column the outputs. The optional
    // panels appear exactly when their pass is enabled.
    let show_color_corrected = state.color_correction.enabled;
    let show_tile_reduced = state.settings.tile_reduce_post_enabled;

    // The palette is identical for both output panels, so it is only drawn on
    // the Qualetized one to keep its position stable.
    let palettes = state
        .preferences
        .show_palettes
        .then(|| {
            state
                .output_palette_sorted_indexed_image
                .as_ref()
                .or_else(|| state.output_image.as_ref().and_then(|i| i.indexed.as_ref()))
                .map(|indexed| &indexed.palettes_for_ui)
        })
        .flatten();

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
                    image: &state.input_image,
                    size: Vec2::new(column_width, input_height),
                    zoom,
                    pan_offset,
                    background,
                    has_spinner: false,
                    overlay_text: None,
                    palettes: None,
                },
                &mut pan_changed,
            );

            if show_color_corrected {
                draw_image_panel(
                    ui,
                    ImagePanel {
                        title: "Color Corrected",
                        image: &state.color_corrected_image,
                        size: Vec2::new(column_width, input_height),
                        zoom,
                        pan_offset,
                        background,
                        has_spinner: state.color_corrected_image.is_none(),
                        overlay_text: None,
                        palettes: None,
                    },
                    &mut pan_changed,
                );
            }
        });

        // Outputs, or the warning that replaces them
        if state.tile_size_warning {
            draw_status_panel(ui, state, column_width, available_size.y);
            return;
        }

        ui.vertical(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(0.0, PANEL_MARGIN);

            draw_image_panel(
                ui,
                ImagePanel {
                    title: "Qualetized",
                    image: &state.base_output_image,
                    size: Vec2::new(column_width, output_height),
                    zoom,
                    pan_offset,
                    background,
                    has_spinner: qualetize_processing,
                    overlay_text: None,
                    palettes,
                },
                &mut pan_changed,
            );

            if show_tile_reduced {
                draw_image_panel(
                    ui,
                    ImagePanel {
                        title: "Tile Reduced",
                        image: &state.output_image,
                        size: Vec2::new(column_width, output_height),
                        zoom,
                        pan_offset,
                        background,
                        // A new quantization invalidates the reduced result too.
                        has_spinner: qualetize_processing || state.tile_reduce_processing,
                        overlay_text: toast.as_deref(),
                        palettes: None,
                    },
                    &mut pan_changed,
                );
            }
        });
    });

    if pan_changed != Vec2::ZERO {
        state.pan_offset += pan_changed;
    }

    if ui.ui_contains_pointer() {
        let scroll_delta = ui.ctx().input(|i| i.raw_scroll_delta.y);
        if scroll_delta != 0.0 {
            let zoom_factor = 1.0 + scroll_delta * 0.001;
            state.zoom = (state.zoom * zoom_factor).clamp(0.1, 20.0);
        }
    }
}

/// Height of one panel in a column of `panel_count` stacked panels.
fn column_height(available_height: f32, panel_count: usize) -> f32 {
    let gaps = PANEL_MARGIN * (panel_count.saturating_sub(1)) as f32;
    (available_height - gaps) / panel_count as f32
}

const TOAST_DURATION: std::time::Duration = std::time::Duration::from_secs(3);

/// Return the tile reduce toast while it is still within its display window,
/// dropping it once it has expired. Schedules the repaint that makes it vanish.
fn live_toast(state: &mut AppState, ctx: &egui::Context) -> Option<String> {
    let toast = state.tile_reduce_toast.as_ref()?;
    let Some(remaining) = TOAST_DURATION.checked_sub(toast.time.elapsed()) else {
        state.tile_reduce_toast = None;
        return None;
    };

    let message = toast.message.clone();
    ctx.request_repaint_after(remaining);
    Some(message)
}

pub fn draw_main_content(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.heading_with_margin("📁 Drop an image file here or use 'File > Open Image...'");
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

    for yi in 0.. {
        let y = (yi as f32 + 0.5) * MAGNIFICATION_PIXEL_SIZE;
        if y > canvas.height() + MAGNIFICATION_PIXEL_SIZE {
            break;
        }
        for xi in 0.. {
            let x = (xi as f32 + 0.5) * MAGNIFICATION_PIXEL_SIZE;
            if x > canvas.width() + MAGNIFICATION_PIXEL_SIZE {
                break;
            }
            painter.circle_filled(
                canvas.center()
                    + egui::vec2(x, y)
                    + egui::vec2(-canvas_min_x, -canvas_min_y)
                    + egui::vec2(-canvas.width() / 2.0, -canvas.height() / 2.0),
                pixel_radius,
                pixel_color,
            );
        }
    }
}

fn draw_main_image(
    painter: &egui::Painter,
    canvas: Rect,
    image_data: &Option<ImageData>,
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

fn draw_title(painter: &egui::Painter, canvas: Rect, title: &str, ui_ctx: &egui::Context) {
    if title.is_empty() {
        return;
    }

    let visuals = &ui_ctx.style().visuals;
    let window_color = visuals.window_fill();
    let bg_color =
        Color32::from_rgba_unmultiplied(window_color.r(), window_color.g(), window_color.b(), 178);
    let text_color = visuals.override_text_color.unwrap_or(visuals.text_color());

    let galley =
        ui_ctx.fonts(|f| f.layout_no_wrap(title.to_string(), FontId::default(), text_color));

    let pos = canvas.left_bottom() + Vec2::new(4.0, -20.0);
    let rect = Align2::LEFT_TOP.align_size_within_rect(
        galley.size() + egui::vec2(4.0, 2.0),
        Rect::from_min_size(
            pos - egui::vec2(2.0, 1.0),
            galley.size() + egui::vec2(4.0, 2.0),
        ),
    );
    painter.rect_filled(rect, 0.0, bg_color);
    painter.galley(pos, galley, text_color);
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

fn draw_overlay_text(painter: &egui::Painter, canvas: Rect, ui_ctx: &egui::Context, text: &str) {
    let visuals = &ui_ctx.style().visuals;
    let panel = visuals.panel_fill;
    // Use theme-aware background with slight translucency
    let bg_color = Color32::from_rgba_unmultiplied(panel.r(), panel.g(), panel.b(), 200);
    let text_color = visuals.strong_text_color();
    let font_id = egui::FontId::proportional(15.0); // slightly larger to emphasize toast text

    let galley = ui_ctx.fonts(|f| f.layout_no_wrap(text.to_string(), font_id, text_color));
    let rect = Align2::CENTER_CENTER.align_size_within_rect(
        galley.size() + egui::vec2(12.0, 6.0),
        Rect::from_center_size(canvas.center(), galley.size() + egui::vec2(12.0, 6.0)),
    );
    painter.rect_filled(rect, 4.0, bg_color);
    painter.galley(rect.center() - galley.size() * 0.5, galley, text_color);
}

fn draw_image_panel(ui: &mut egui::Ui, panel: ImagePanel, pan_changed: &mut Vec2) {
    ui.allocate_ui_with_layout(
        panel.size,
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            let (response, painter) =
                ui.allocate_painter(panel.size, egui::Sense::click_and_drag());
            let canvas = response.rect;

            draw_background_and_pixels(&painter, canvas, panel.background);
            draw_main_image(&painter, canvas, panel.image, panel.zoom, panel.pan_offset);
            draw_title(&painter, canvas, panel.title, ui.ctx());

            if let Some(palettes) = panel.palettes {
                draw_palettes_overlay(&painter, canvas, palettes);
            }
            if panel.has_spinner {
                draw_spinner(&painter, canvas, ui.ctx());
            }
            if let Some(text) = panel.overlay_text {
                draw_overlay_text(&painter, canvas, ui.ctx(), text);
            }

            // Panning
            if response.dragged() {
                *pan_changed += response.drag_delta();
            }
        },
    );
}

fn draw_status_panel(ui: &mut egui::Ui, state: &AppState, width: f32, height: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            let (_, painter) = ui.allocate_painter(Vec2::new(width, height), egui::Sense::hover());

            // Draw background
            painter.rect_filled(painter.clip_rect(), 0.0, Color32::from_gray(64));

            ui.scope_builder(
                egui::UiBuilder::new().max_rect(Rect::from_center_size(
                    painter.clip_rect().center(),
                    Vec2::new(300.0, 150.0),
                )),
                |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        if state.tile_size_warning {
                            draw_warning_message(ui, state);
                        }
                    });
                },
            );
        },
    );
}

fn draw_warning_message(ui: &mut egui::Ui, state: &AppState) {
    ui.label(egui::RichText::new("⚠").size(32.0).color(Color32::YELLOW));
    ui.label(
        egui::RichText::new("Tile Size Warning")
            .size(16.0)
            .color(Color32::YELLOW),
    );
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(state.tile_size_warning_message())
            .size(12.0)
            .color(Color32::WHITE),
    );
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new("Adjust tile width/height in settings to match image dimensions.")
            .size(11.0)
            .color(Color32::LIGHT_GRAY),
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

fn draw_palettes_overlay(painter: &egui::Painter, rect: Rect, palettes: &[Vec<egui::Color32>]) {
    if palettes.is_empty() {
        return;
    }

    let ctx = painter.ctx();
    let pointer_pos = ctx.pointer_hover_pos();
    let mut hovered: Option<(usize, usize)> = None;
    let mut hovered_color: Option<egui::Color32> = None;

    let palette_margin = 8.0;
    let palette_spacing = 1.0;
    let palette_size = calculate_palette_size(&rect, palettes, palette_margin, palette_spacing);

    let start_x = rect.max.x - palette_margin;
    let mut current_y = rect.min.y + palette_margin;

    if let Some(pos) = pointer_pos {
        let mut hover_y = current_y;
        'outer: for (palette_idx, palette) in palettes.iter().enumerate() {
            let palette_width =
                (palette.len() as f32) * (palette_size + palette_spacing) - palette_spacing;
            for (color_idx, &color) in palette.iter().enumerate() {
                let x =
                    start_x - palette_width + (color_idx as f32) * (palette_size + palette_spacing);
                let color_rect = Rect::from_min_size(
                    Pos2::new(x, hover_y),
                    Vec2::new(palette_size, palette_size),
                );
                if color_rect.contains(pos) {
                    hovered = Some((palette_idx, color_idx));
                    hovered_color = Some(color);
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

    if let Some((palette_idx, color_idx)) = hovered
        && let Some(color) = hovered_color
    {
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
                ui.label(format!("Palette {} / Index {}", palette_idx, color_idx));
                ui.label(hex);
                ui.label(rgba);
            });
        }
    }
}

fn calculate_palette_size(
    rect: &Rect,
    palettes: &[Vec<egui::Color32>],
    palette_margin: f32,
    palette_spacing: f32,
) -> f32 {
    if let Some(first_palette) = palettes.first() {
        4.0_f32.max(16.0_f32.min(
            (rect.width()
                - palette_margin * 2.0
                - ((first_palette.len() as f32) - 1.0) * palette_spacing)
                / (first_palette.len() as f32),
        ))
    } else {
        8.0
    }
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
    let palette_width = (palette.len() as f32) * (palette_size + palette_spacing) - palette_spacing;
    let highlight_color = painter.ctx().style().visuals.selection.stroke.color;

    for (color_idx, &color) in palette.iter().enumerate() {
        let x = origin.x - palette_width + (color_idx as f32) * (palette_size + palette_spacing);
        let color_rect = Rect::from_min_size(
            Pos2::new(x, origin.y),
            Vec2::new(palette_size, palette_size),
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
}
