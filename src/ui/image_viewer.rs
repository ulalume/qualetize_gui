use crate::types::AppState;
use egui::{Align2, Color32, FontId, Pos2, Rect, Vec2};

pub fn draw_image_view(ui: &mut egui::Ui, state: &mut AppState) {
    const HORIZONTAL_MARGIN: f32 = 4.0;
    let mut available_size = ui.available_size();
    available_size.y -= 34.0; // footer size

    let zoom = state.zoom;
    let pan_offset = state.pan_offset;
    let mut pan_changed = egui::Vec2::ZERO;

    let split_x = if state.show_original_image || state.show_color_corrected_image {
        (available_size.x - HORIZONTAL_MARGIN) / 2.0
    } else {
        available_size.x
    };
    let split_y = if state.show_original_image && state.show_color_corrected_image {
        (available_size.y - HORIZONTAL_MARGIN) / 2.0
    } else {
        available_size.y
    };

    ui.horizontal(|ui| {
        ui.style_mut().spacing.item_spacing = egui::vec2(HORIZONTAL_MARGIN, 0.0);
        // Left panel - Original image
        ui.vertical(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(0.0, HORIZONTAL_MARGIN);
            if state.show_original_image {
                draw_image_panel(
                    ui,
                    split_x,
                    split_y,
                    &state.input_image,
                    zoom,
                    pan_offset,
                    &mut pan_changed,
                    &state,
                    false,
                    "Original",
                );
            }
            if state.show_color_corrected_image {
                draw_image_panel(
                    ui,
                    split_x,
                    split_y,
                    &state.color_corrected_image,
                    zoom,
                    pan_offset,
                    &mut pan_changed,
                    &state,
                    false,
                    "Color Corrected",
                );
            }
        });

        // Right panel
        if state.tile_size_warning || !state.preview_ready {
            // Status/ Warning message
            draw_status_panel(ui, state, split_x, available_size.y);
        } else {
            // Output image with palettes
            draw_image_panel(
                ui,
                split_x,
                available_size.y,
                &state.output_image,
                zoom,
                pan_offset,
                &mut pan_changed,
                &state,
                state.preview_processing,
                "Qualetized",
            );
        }
    });

    // Apply changes back to state (this block is common to both views)
    if pan_changed != egui::Vec2::ZERO {
        state.pan_offset += pan_changed;
    }

    // Handle mouse interaction (this block is also common)
    if ui.ui_contains_pointer() {
        let ctx = ui.ctx();
        let scroll_delta = ctx.input(|i| i.raw_scroll_delta.y);
        if scroll_delta != 0.0 {
            let zoom_factor = 1.0 + scroll_delta * 0.001;
            state.zoom = (state.zoom * zoom_factor).clamp(0.1, 20.0);
        }
    }
}

pub fn draw_main_content(ui: &mut egui::Ui, state: &AppState) {
    if state.input_path.is_none() {
        ui.centered_and_justified(|ui| {
            ui.heading("📁 Drop an image file here or use 'File > Open Image...'");
        });
    } else if !state.preview_ready {
        ui.centered_and_justified(|ui| {
            ui.heading("⏳ Processing...");
        });
    }
}

fn draw_processing_message(ui: &mut egui::Ui, state: &AppState) {
    ui.label("⏳");
    ui.label("Processing...");
    if !state.result_message.is_empty() {
        ui.label(&state.result_message);
    }
}

fn draw_image_panel(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    image_data: &crate::types::ImageData,
    zoom: f32,
    pan_offset: Vec2,
    pan_changed: &mut Vec2,
    state: &AppState,
    has_spinner: bool,
    title: &str,
) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            let (response, painter) =
                ui.allocate_painter(Vec2::new(width, height), egui::Sense::click_and_drag());

            let canvas = response.rect;

            // Draw background
            let base_color = state.background_color.unwrap_or(Color32::from_gray(64));
            painter.rect_filled(canvas, 0.0, base_color);

            // Draw pixel centers:
            const MAGNIFICATION_PIXEL_SIZE: f32 = 24.0;
            let canvas_min_x = canvas.min.x % MAGNIFICATION_PIXEL_SIZE;
            let canvas_min_y = canvas.min.y % MAGNIFICATION_PIXEL_SIZE;
            let pixel_radius = 1.25;
            let pixel_color = Color32::from_rgba_unmultiplied(
                (base_color.r() as f32 * 1.5) as u8,
                (base_color.g() as f32 * 1.5) as u8,
                (base_color.b() as f32 * 1.5) as u8,
                base_color.a(), // alphaはそのまま
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

            if let Some(texture) = image_data.texture.as_ref() {
                let original_size = egui::vec2(image_data.width as f32, image_data.height as f32);

                let image_rect = calculate_image_rect(&canvas, original_size, zoom, pan_offset);

                painter.image(
                    texture.id(),
                    image_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                // Draw palettes overlay for output image
                if !image_data.palettes.is_empty() && state.show_palettes {
                    draw_palettes_overlay(&painter, &canvas, &image_data.palettes);
                }
            }

            // Draw title
            if !title.is_empty() {
                let visuals = &ui.ctx().style().visuals;
                let window_color = visuals.window_fill();
                let bg_color = Color32::from_rgba_unmultiplied(
                    window_color.r(),
                    window_color.g(),
                    window_color.b(),
                    178,
                );
                let text_color = visuals.override_text_color.unwrap_or(visuals.text_color());

                let galley = ui
                    .fonts(|f| f.layout_no_wrap(title.to_string(), FontId::default(), text_color));

                let pos = canvas.left_bottom() + Vec2::new(4.0, -20.0);
                let rect = Align2::LEFT_TOP.align_size_within_rect(
                    galley.size() + egui::vec2(4.0, 2.0),
                    Rect::from_min_size(
                        pos - egui::vec2(2.0, 1.0),
                        galley.size() + egui::vec2(4.0, 2.0),
                    ),
                );
                painter.rect_filled(rect, 0.0, bg_color);

                // その上に文字を描画
                painter.galley(pos, galley, text_color);
            }

            if has_spinner {
                let center = canvas.center();
                let radius = 16.0;
                let num_lines = 12;
                let time = ui.ctx().input(|i| i.time) as f32;
                for i in 0..num_lines {
                    let angle = i as f32 / num_lines as f32 * std::f32::consts::TAU + time;
                    let start = center + egui::vec2(angle.cos(), angle.sin()) * radius * 0.5;
                    let end = center + egui::vec2(angle.cos(), angle.sin()) * radius;
                    painter.line_segment([start, end], (2.5, Color32::LIGHT_GRAY));
                }
                ui.ctx().request_repaint();
            }

            // Handle pan
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
                        } else {
                            draw_processing_message(ui, state);
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
        egui::RichText::new(&state.tile_size_warning_message)
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

fn draw_palettes_overlay(painter: &egui::Painter, rect: &Rect, palettes: &[Vec<egui::Color32>]) {
    if palettes.is_empty() {
        return;
    }

    let palette_margin = 8.0;
    let palette_spacing = 1.0;
    let palette_size = calculate_palette_size(rect, palettes, palette_margin, palette_spacing);

    let start_x = rect.max.x - palette_margin;
    let mut current_y = rect.min.y + palette_margin;

    for palette in palettes {
        draw_single_palette(
            painter,
            palette,
            start_x,
            current_y,
            palette_size,
            palette_spacing,
        );
        current_y += palette_size + palette_spacing;
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
    palette: &[egui::Color32],
    start_x: f32,
    y: f32,
    palette_size: f32,
    palette_spacing: f32,
) {
    let palette_width = (palette.len() as f32) * (palette_size + palette_spacing) - palette_spacing;

    for (color_idx, &color) in palette.iter().enumerate() {
        let x = start_x - palette_width + (color_idx as f32) * (palette_size + palette_spacing);
        let color_rect =
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(palette_size, palette_size));

        painter.rect_filled(color_rect, 0.0, color);
        painter.rect_stroke(
            color_rect,
            0.0,
            egui::Stroke::new(1.0, Color32::from_gray(48)),
            egui::StrokeKind::Middle,
        );
    }
}
