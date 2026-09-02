//! Widget helpers for the settings panel: each wraps an `egui` control and
//! returns whether its value changed.

use std::ops::RangeInclusive;

use super::styles::RichTextExt;

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

/// A section heading with a toggle (an "Enable" or "Show" checkbox) aligned
/// to the right edge of the same row, so the switch that governs a section
/// sits on its title instead of taking a row of its own below it.
/// Returns whether the toggle changed.
pub fn section_header(
    ui: &mut egui::Ui,
    title: &str,
    toggle: &mut bool,
    toggle_label: &str,
    hover: Option<&str>,
) -> bool {
    header_with_toggle(ui, toggle, toggle_label, hover, |ui| {
        ui.heading(title);
    })
}

/// Same as [`section_header`], but at the subheading level used for a
/// settings block that is itself a subsection of a larger one (e.g.
/// "Advanced Settings" under "Qualetize"), matching sibling subsections
/// drawn with [`crate::ui::styles::UiMarginExt::subheading_with_margin`].
pub fn subsection_header(
    ui: &mut egui::Ui,
    title: &str,
    toggle: &mut bool,
    toggle_label: &str,
    hover: Option<&str>,
) -> bool {
    header_with_toggle(ui, toggle, toggle_label, hover, |ui| {
        ui.label(egui::RichText::new(title).subheading());
    })
}

/// A heading with an [`EnumCombo`] right-aligned on the same row, e.g. the
/// engine picker atop the settings panel. Same layout as [`section_header`],
/// with the toggle checkbox replaced by a combo box. Returns whether the
/// selection changed.
pub fn heading_with_combo<T: Copy + PartialEq + 'static>(
    ui: &mut egui::Ui,
    title: &str,
    combo: EnumCombo<'_, T>,
    value: &mut T,
) -> bool {
    header_row(
        ui,
        |ui| {
            ui.heading(title);
        },
        |ui| combo.show(ui, value),
    )
}

/// Shared layout for [`section_header`], [`subsection_header`] and
/// [`heading_with_combo`]: draws `draw_title` on the left and a right-aligned
/// control on the same row, inside the Frame margin the heading levels
/// already use elsewhere. Returns whatever `draw_right` returns.
fn header_row(
    ui: &mut egui::Ui,
    draw_title: impl FnOnce(&mut egui::Ui),
    draw_right: impl FnOnce(&mut egui::Ui) -> bool,
) -> bool {
    let mut changed = false;
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 2,
            bottom: 4,
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

/// [`header_row`] with a toggle checkbox as the right-aligned control, used
/// by [`section_header`] and [`subsection_header`]. Returns whether the
/// toggle changed.
fn header_with_toggle(
    ui: &mut egui::Ui,
    toggle: &mut bool,
    toggle_label: &str,
    hover: Option<&str>,
    draw_title: impl FnOnce(&mut egui::Ui),
) -> bool {
    header_row(ui, draw_title, |ui| {
        checkbox(ui, toggle, toggle_label, hover)
    })
}
