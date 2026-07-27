use egui::Vec2;
use std::sync::{Arc, atomic::AtomicBool, mpsc};

use super::{
    color_correction::ColorCorrection,
    export::ExportFormat,
    image::{ImageData, ImageDataIndexed, PaletteSortSettings, SortMode},
    preferences::UserPreferences,
    qualetize::QualetizeSettings,
};
use crate::types::image::TileCountOptions;
use std::time::Instant;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Default,
)]
pub enum AppearanceMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy)]
pub struct TileCountSettings {
    pub visible_only: bool,
    pub allow_flip_x: bool,
    pub allow_flip_y: bool,
}

impl Default for TileCountSettings {
    fn default() -> Self {
        Self {
            visible_only: true,
            allow_flip_x: true,
            allow_flip_y: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TileCountState {
    pub settings: TileCountSettings,
    pub last_count: Option<usize>,
    pub dirty: bool,
}

impl TileCountState {
    pub fn options(&self) -> TileCountOptions {
        TileCountOptions {
            visible_only: self.settings.visible_only,
            allow_flip_x: self.settings.allow_flip_x,
            allow_flip_y: self.settings.allow_flip_y,
        }
    }

    /// Force a recount on the next draw, keeping the stale number visible until then.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Force a recount and clear the displayed number, for when the image itself is gone.
    pub fn reset(&mut self) {
        self.last_count = None;
        self.dirty = true;
    }
}

/// Which stage of the pipeline an export refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportSource {
    /// The input image after color correction, in full color.
    ColorCorrected,
    /// The indexed quantization result, before tile reduction.
    Qualetized,
    /// The indexed result after the tile reduction pass.
    TileReduced,
}

impl ExportSource {
    /// Appended to the input file name to build the default export name.
    pub fn file_suffix(self) -> &'static str {
        match self {
            Self::ColorCorrected => "color_corrected",
            Self::Qualetized => "qualetized",
            Self::TileReduced => "tile_reduced",
        }
    }
}

// Export request types
#[derive(Debug, Clone)]
pub enum AppStateRequest {
    LoadImage {
        path: String,
    },
    ExportImage {
        source: ExportSource,
        format: ExportFormat,
        output_path: String,
    },
    SaveSettings {
        path: String,
    },
    LoadSettings {
        path: String,
    },

    OpenImageDialog,
    ExportImageDialog {
        source: ExportSource,
        format: ExportFormat,
    },
    SaveSettingsDialog,
    LoadSettingsDialog,
}

#[derive(Debug, Clone)]
pub struct QualetizeRequest {
    pub time: std::time::Instant,
}

pub struct AppState {
    // Image management
    pub input_path: Option<String>,
    pub input_image: Option<ImageData>,
    pub color_corrected_image: Option<ImageData>,
    pub base_output_image: Option<ImageData>,
    pub output_image: Option<ImageData>,
    pub output_palette_sorted_indexed_image: Option<ImageDataIndexed>,
    pub base_tile_count: Option<usize>,
    pub reduced_tile_count: Option<usize>,
    pub tile_reduce_processing: bool,
    pub tile_reduce_generation_id: u64,
    pub tile_reduce_toast: Option<TileReduceToast>,

    // View Settings
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub preferences: UserPreferences,
    last_preferences: UserPreferences,

    // Qualetize Settings
    pub settings: QualetizeSettings,
    pub request_update_qualetized_image: Option<QualetizeRequest>,
    pub request_update_tile_reduce: bool,
    pub debounce_delay: std::time::Duration,

    // Color Correction Settings
    pub color_correction: ColorCorrection,
    last_color_correction: ColorCorrection,

    // Palette Sort Settings
    pub palette_sort_settings: PaletteSortSettings,
    last_palette_sort_settings: PaletteSortSettings,
    /// Set when the source of the sorted palette changed and it has to be recomputed.
    /// `output_palette_sorted_indexed_image` cannot be used for this on its own, because
    /// `None` is also the legitimate result of `SortMode::None`.
    palette_sort_dirty: bool,

    pub tile_count: TileCountState,

    // warning
    pub tile_size_warning: bool,

    // Export requests
    pub app_state_request_receiver: mpsc::Receiver<AppStateRequest>,
    pub app_state_request_sender: mpsc::Sender<AppStateRequest>,

    pub file_dialog_open: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct TileReduceToast {
    pub message: String,
    pub time: Instant,
}

impl Default for AppState {
    fn default() -> Self {
        let preferences = UserPreferences::load();
        let (sender, receiver) = mpsc::channel();

        Self {
            input_path: None,
            input_image: None,
            color_corrected_image: None,
            base_output_image: None,
            output_image: None,
            output_palette_sorted_indexed_image: None,
            base_tile_count: None,
            reduced_tile_count: None,
            tile_reduce_processing: false,
            tile_reduce_generation_id: 0,
            tile_reduce_toast: None,

            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            preferences: preferences.clone(),
            last_preferences: preferences.clone(),

            settings: QualetizeSettings::default(),
            request_update_qualetized_image: None,
            request_update_tile_reduce: false,
            debounce_delay: std::time::Duration::from_millis(100),

            last_color_correction: ColorCorrection::default(),
            color_correction: ColorCorrection::default(),

            palette_sort_settings: PaletteSortSettings::default(),
            last_palette_sort_settings: PaletteSortSettings::default(),
            palette_sort_dirty: false,

            tile_count: TileCountState::default(),

            tile_size_warning: false,

            app_state_request_receiver: receiver,
            app_state_request_sender: sender,

            file_dialog_open: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AppState {
    pub fn tile_size_warning_message(&self) -> String {
        let Some(input_image) = &self.input_image else {
            return String::new();
        };
        format!(
            "Image size ({}×{}) is not divisible by tile size ({}×{}). Qualetize processing cannot proceed.",
            input_image.width,
            input_image.height,
            self.settings.tile_width,
            self.settings.tile_height,
        )
    }

    pub fn check_and_save_preferences(&mut self) {
        if self.preferences != self.last_preferences {
            self.last_preferences = self.preferences.clone();
            if let Err(e) = self.preferences.save() {
                eprintln!("Failed to save preferences: {e}");
            }
        }
    }

    /// Drop the cached sorted palette and schedule a recomputation.
    /// Call this whenever `output_image` (or anything `sorted()` reads) changes.
    pub fn invalidate_palette_sort(&mut self) {
        self.output_palette_sorted_indexed_image = None;
        self.palette_sort_dirty = true;
    }

    pub fn palette_sort_needs_update(&self) -> bool {
        self.palette_sort_dirty || self.palette_sort_settings != self.last_palette_sort_settings
    }

    /// Mark the sorted palette as up to date with the current settings.
    pub fn clear_palette_sort_dirty(&mut self) {
        self.last_palette_sort_settings = self.palette_sort_settings;
        self.palette_sort_dirty = false;
    }

    /// Check if color correction settings have changed
    pub fn color_correction_changed(&self) -> bool {
        self.color_correction != self.last_color_correction
    }

    /// Update the tracked color correction settings
    pub fn update_color_correction_tracking(&mut self) {
        self.last_color_correction = self.color_correction.clone();
    }

    pub fn reset_view_settings(&mut self) {
        self.preferences = UserPreferences::default();
    }

    /// Drop the qualetize result and everything derived from it, keeping the
    /// input and color corrected images.
    pub fn reset_qualetize_outputs(&mut self) {
        self.base_output_image = None;
        self.output_image = None;
        self.base_tile_count = None;
        self.reduced_tile_count = None;
        self.request_update_tile_reduce = false;
        self.invalidate_palette_sort();
        self.tile_count.reset();
    }

    /// Whether `source` currently has something to export. The optional passes
    /// are only offered while they are enabled, so the menu entry matches the
    /// view it refers to.
    pub fn can_export(&self, source: ExportSource) -> bool {
        match source {
            ExportSource::ColorCorrected => {
                self.color_correction.enabled && self.color_corrected_image.is_some()
            }
            ExportSource::Qualetized => self.base_output_image.is_some(),
            ExportSource::TileReduced => {
                self.settings.tile_reduce_post_enabled && self.output_image.is_some()
            }
        }
    }

    /// The indexed image to export for `source`, with the current palette sort
    /// applied. Sorting is redone here rather than reusing
    /// `output_palette_sorted_indexed_image`, which only ever describes the
    /// final image and would be wrong for the pre-reduction export.
    pub fn indexed_for_export(&self, source: ExportSource) -> Option<(ImageDataIndexed, u32, u32)> {
        let image = match source {
            ExportSource::Qualetized => self.base_output_image.as_ref()?,
            ExportSource::TileReduced => self.output_image.as_ref()?,
            ExportSource::ColorCorrected => return None,
        };
        let indexed = image.indexed.as_ref()?;

        let sort = self.palette_sort_settings;
        let indexed = if sort.mode == SortMode::None {
            indexed.clone()
        } else {
            indexed.sorted(sort.mode, sort.order, self.settings.col0_is_clear)
        };

        Some((indexed, image.width, image.height))
    }

    /// Drop the loaded image together with everything derived from it.
    pub fn reset_all_images(&mut self) {
        self.input_path = None;
        self.input_image = None;
        self.color_corrected_image = None;
        self.reset_qualetize_outputs();
    }
}
