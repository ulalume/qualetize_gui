//! Widget helpers for the settings panel: each wraps an `egui` control and
//! returns whether its value changed.

use std::ops::RangeInclusive;

/// A `DragValue` bound to `range`, with a hover tooltip on the control itself.
/// Returns whether the drag changed the value.
pub fn drag_u16(
    ui: &mut egui::Ui,
    value: &mut u16,
    range: RangeInclusive<u16>,
    hover: &str,
) -> bool {
    ui.add(egui::DragValue::new(value).range(range))
        .on_hover_text(hover)
        .changed()
}

/// A checkbox with an optional hover tooltip. Returns whether it was toggled.
pub fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str, hover: Option<&str>) -> bool {
    let response = ui.checkbox(value, label);
    let response = match hover {
        Some(hover) => response.on_hover_text(hover),
        None => response,
    };
    response.changed()
}

/// A `ComboBox` over every value of an enum, labelled by `name`.
///
/// Built up with the optional parts a call site wants (per-entry tooltips,
/// a tooltip on the closed control, a fixed width), then shown with
/// [`Self::show`], which returns whether the selection changed.
pub struct EnumCombo<'a, T: 'static> {
    id: &'a str,
    all: &'a [T],
    name: fn(&T) -> &'static str,
    description: Option<fn(&T) -> &'static str>,
    hover: Option<&'a str>,
    width: Option<f32>,
}

impl<'a, T: Copy + PartialEq + 'static> EnumCombo<'a, T> {
    pub fn new(id: &'a str, all: &'a [T], name: fn(&T) -> &'static str) -> Self {
        Self {
            id,
            all,
            name,
            description: None,
            hover: None,
            width: None,
        }
    }

    /// Tooltip shown on each entry of the open list.
    pub fn description(mut self, description: fn(&T) -> &'static str) -> Self {
        self.description = Some(description);
        self
    }

    /// Tooltip shown on the closed control.
    pub fn hover(mut self, hover: &'a str) -> Self {
        self.hover = Some(hover);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Draw the combo bound to `value`; true when a different entry was picked.
    pub fn show(self, ui: &mut egui::Ui, value: &mut T) -> bool {
        let mut changed = false;

        let mut combo = egui::ComboBox::from_id_salt(self.id).selected_text((self.name)(value));
        if let Some(width) = self.width {
            combo = combo.width(width);
        }

        let response = combo
            .show_ui(ui, |ui| {
                for item in self.all {
                    let mut item_response = ui.selectable_value(value, *item, (self.name)(item));
                    if let Some(description) = self.description {
                        item_response = item_response.on_hover_text(description(item));
                    }
                    // `changed()` rather than `clicked()`: re-picking the current
                    // entry must not trigger a re-quantization.
                    changed |= item_response.changed();
                }
            })
            .response;

        if let Some(hover) = self.hover {
            response.on_hover_text(hover);
        }

        changed
    }
}

/// A section heading that is itself the toggle governing the section: a
/// checkbox whose label is the title, drawn in heading style, so the switch
/// sits on the title instead of taking a row of its own. Clicking the title
/// text toggles it, since egui checkbox labels are clickable.
/// Returns whether the toggle changed.
pub fn section_header(
    ui: &mut egui::Ui,
    title: &str,
    toggle: &mut bool,
    hover: Option<&str>,
) -> bool {
    let mut changed = false;
    header_row(
        ui,
        |ui| {
            // The checkbox square scales with the heading text instead of
            // using the body-text size `ui.checkbox` defaults to.
            let heading_height = ui.text_style_height(&egui::TextStyle::Heading);
            ui.scope(|ui| {
                ui.spacing_mut().icon_width = heading_height;
                ui.spacing_mut().icon_width_inner = heading_height * 0.6;
                let mut response = ui.checkbox(toggle, egui::RichText::new(title).heading());
                if let Some(hover) = hover {
                    response = response.on_hover_text(hover);
                }
                changed = response.changed();
            });
        },
        |_ui| false,
    );
    changed
}

/// Shared layout for [`section_header`] and the Qualetization heading: draws
/// `draw_title` on the left and a right-aligned control on the same row,
/// inside the Frame margin the heading level already uses elsewhere.
/// Returns whatever `draw_right` returns.
pub fn header_row(
    ui: &mut egui::Ui,
    draw_title: impl FnOnce(&mut egui::Ui),
    draw_right: impl FnOnce(&mut egui::Ui) -> bool,
) -> bool {
    let mut changed = false;
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 4,
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                draw_title(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    changed = draw_right(ui);
                });
            });
        });
    changed
}
