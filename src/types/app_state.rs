use egui::Vec2;
use std::collections::HashMap;
use std::sync::{Arc, atomic::AtomicBool, mpsc};

use super::{
    color_correction::ColorCorrection,
    export::ExportFormat,
    image::{ImageData, ImageDataIndexed, PaletteSortSettings, SortMode},
    preferences::UserPreferences,
    qualetize::QualetizeSettings,
};
use crate::engine::QuantEngine;
use crate::settings_manager::SettingsBundle;
use crate::time::Instant;
use crate::types::FirstColor;
use crate::types::history::SettingsHistory;
use crate::types::image::TileCountOptions;
use crate::types::results::Results;
use crate::types::tilepalquant::TpqSettings;

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

/// How unique tiles are counted, and whether the counts in `AppState` are
/// stale. The counts themselves live next to the images they describe.
#[derive(Debug, Clone, Default)]
pub struct TileCountState {
    pub settings: TileCountSettings,
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

    /// Force a recount on the next frame, keeping the stale number visible until then.
    pub fn mark_dirty(&mut self) {
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
    /// An image that arrived as bytes (drag and drop on the web, a fetched
    /// URL); `name` stands in for the path.
    LoadImageBytes {
        name: String,
        bytes: Vec<u8>,
    },
    LoadSettings {
        path: String,
    },
    /// Settings that arrived as bytes, from a browser file dialog.
    LoadSettingsBytes {
        name: String,
        bytes: Vec<u8>,
    },

    OpenImageDialog,
    /// Drop the loaded image and everything derived from it, after asking.
    RemoveImage,
    ExportImageDialog {
        source: ExportSource,
        format: ExportFormat,
    },
    SaveSettingsDialog,
    LoadSettingsDialog,

    Undo,
    Redo,

    /// Put the settings of the result with this hash back in use.
    ApplyResult {
        hash: u64,
    },
    /// Write the result with this hash out in the selected export format.
    ExportResult {
        hash: u64,
    },
    /// Drop the result with this hash from the list.
    RemoveResult {
        hash: u64,
    },
}

/// The textures a result entry is drawn from, uploaded as rows become
/// visible. `full` only exists while the panel is wide enough to show the
/// image above thumbnail size.
#[derive(Default)]
pub struct ResultTextures {
    pub thumbnail: Option<egui::TextureHandle>,
    pub full: Option<egui::TextureHandle>,
}

#[derive(Debug, Clone)]
pub struct QualetizeRequest {
    pub time: Instant,
}

/// The top-left pixel of the input, as the "Use top-left color" buttons and
/// the presets read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopLeftPixel {
    /// Fully opaque, so its RGB is usable as a key or shared color.
    Color([u8; 3]),
    /// Not fully opaque: the image marks transparency with alpha, not a color.
    Transparent,
}

/// Images with at least this many pixels ask for confirmation before
/// loading: every stage of the pipeline runs over all pixels.
pub const LARGE_IMAGE_PIXELS: u64 = 1024 * 768;

pub struct AppState {
    // Image management
    pub input_path: Option<String>,
    pub input_image: Option<ImageData>,
    pub color_corrected_image: Option<ImageData>,
    pub base_output_image: Option<ImageData>,
    pub output_image: Option<ImageData>,
    pub output_palette_sorted_indexed_image: Option<ImageDataIndexed>,
    /// Unique tiles in `base_output_image` and `output_image`, recounted
    /// whenever `tile_count.dirty` is set.
    pub base_tile_count: Option<usize>,
    pub reduced_tile_count: Option<usize>,
    /// Mirrored from the processor once per frame for the panel spinners.
    pub tile_reduce_processing: bool,
    pub tile_reduce_toast: Option<Toast>,

    /// Set only while the loaded image needs extending; the input image itself
    /// is never modified, so a later tile size change re-derives this from it.
    pub tile_fitted_input: Option<FittedInput>,
    pub tile_fit_toast: Option<Toast>,

    // View Settings
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub preferences: UserPreferences,
    last_preferences: UserPreferences,

    // Quantization settings. `settings` holds the target format shared by
    // both engines plus the Qualetize specific values; `tpq_settings` the
    // tilepalquant specific ones. Both are kept whichever engine is selected.
    pub engine: QuantEngine,
    pub settings: QualetizeSettings,
    pub tpq_settings: TpqSettings,
    /// Percent reported by the running quantization, for the panel.
    pub quantize_progress: Option<u8>,
    /// An image above [`LARGE_IMAGE_PIXELS`] waiting for the user to confirm
    /// loading it, with its size.
    pub pending_large_image: Option<(AppStateRequest, u32, u32)>,
    /// Thumbnails of the bundled sample images, uploaded the first time the
    /// welcome screen is drawn.
    pub sample_thumbnails: Vec<egui::TextureHandle>,
    /// The Remove image confirmation is up.
    pub confirm_remove_image: bool,
    /// The About modal is up.
    pub show_about: bool,
    /// The app icon, small (menu bar) and large (About modal), decoded the
    /// first time either is drawn.
    pub app_icons: Option<(egui::TextureHandle, egui::TextureHandle)>,
    /// Snapshot of what is mirrored to the session file, so it is only rewritten
    /// when something actually changed.
    last_saved_session: SettingsBundle,
    session_save_deadline: Option<Instant>,
    pub request_update_qualetized_image: Option<QualetizeRequest>,
    pub request_update_tile_reduce: bool,
    pub debounce_delay: std::time::Duration,

    // Color Correction Settings
    pub color_correction: ColorCorrection,
    last_color_correction: ColorCorrection,

    // Palette Sort Settings
    pub palette_sort_settings: PaletteSortSettings,
    last_palette_sort_settings: PaletteSortSettings,
    /// The mode to restore when the "Reorder palette colors" checkbox is
    /// re-enabled after being turned off. Not persisted: only
    /// `palette_sort_settings.mode` (which becomes [`SortMode::None`] while
    /// off) is saved to the session.
    pub palette_sort_mode_memory: SortMode,
    /// Set when the source of the sorted palette changed and it has to be recomputed.
    /// `output_palette_sorted_indexed_image` cannot be used for this on its own, because
    /// `None` is also the legitimate result of `SortMode::None`.
    palette_sort_dirty: bool,

    pub tile_count: TileCountState,

    /// Undo / redo history over the settings (engine, Qualetize settings,
    /// tilepalquant settings, color correction, palette sort).
    pub history: SettingsHistory,

    /// Completed outputs with the settings that produced them, newest first.
    pub results: Results,
    /// Textures of the drawn result entries, keyed by [`StoredResult::hash`].
    ///
    /// [`StoredResult::hash`]: crate::types::results::StoredResult::hash
    pub results_textures: HashMap<u64, ResultTextures>,
    /// A step was committed to the history and the result it belongs to is
    /// still being produced; the next idle frame records it.
    pub record_result_when_idle: bool,

    // Export requests
    pub app_state_request_receiver: mpsc::Receiver<AppStateRequest>,
    pub app_state_request_sender: mpsc::Sender<AppStateRequest>,

    pub file_dialog_open: Arc<AtomicBool>,
}

/// A short lived message drawn over an image panel.
#[derive(Clone)]
pub struct Toast {
    pub message: String,
    pub time: Instant,
}

impl Toast {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            time: Instant::now(),
        }
    }
}

/// The input image extended so its size is a multiple of the tile size.
pub struct FittedInput {
    pub image: ImageData,
    /// Size of the image as it was loaded, for the notice in the Original view.
    pub original_size: (u32, u32),
}

impl Default for AppState {
    fn default() -> Self {
        let preferences = UserPreferences::load();
        let session = SettingsBundle::load_session();
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
            tile_reduce_toast: None,
            tile_fitted_input: None,
            tile_fit_toast: None,

            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            preferences: preferences.clone(),
            last_preferences: preferences.clone(),

            engine: session.engine,
            settings: session.qualetize_settings.clone(),
            tpq_settings: session.tpq_settings.clone(),
            quantize_progress: None,
            pending_large_image: None,
            sample_thumbnails: Vec::new(),
            confirm_remove_image: false,
            show_about: false,
            app_icons: None,
            last_saved_session: session.clone(),
            session_save_deadline: None,
            request_update_qualetized_image: None,
            request_update_tile_reduce: false,
            debounce_delay: std::time::Duration::from_millis(100),

            last_color_correction: session.color_correction.clone(),
            color_correction: session.color_correction.clone(),

            palette_sort_settings: session.sort_settings,
            last_palette_sort_settings: session.sort_settings,
            palette_sort_mode_memory: match session.sort_settings.mode {
                SortMode::None => SortMode::Ramps,
                mode => mode,
            },
            palette_sort_dirty: false,

            tile_count: TileCountState::default(),

            // Built the same way as `settings_bundle()`, so the initial
            // history baseline matches what the first `observe` call sees
            // (which stamps the current app version, not the session file's).
            history: SettingsHistory::new(SettingsBundle {
                engine: session.engine,
                tpq_settings: session.tpq_settings.clone(),
                ..SettingsBundle::new(
                    session.qualetize_settings.clone(),
                    session.color_correction.clone(),
                    session.sort_settings,
                )
            }),

            results: Results::default(),
            results_textures: HashMap::new(),
            record_result_when_idle: false,

            app_state_request_receiver: receiver,
            app_state_request_sender: sender,

            file_dialog_open: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AppState {
    /// The image the pipeline actually runs on: the extended one when the
    /// loaded image does not line up with the tile grid, otherwise the input.
    pub fn processing_input(&self) -> Option<&ImageData> {
        self.tile_fitted_input
            .as_ref()
            .map(|fitted| &fitted.image)
            .or(self.input_image.as_ref())
    }

    /// Notice shown in the Original view while the image is being extended.
    pub fn tile_fit_notice(&self) -> Option<String> {
        let fitted = self.tile_fitted_input.as_ref()?;
        let (from_w, from_h) = fitted.original_size;
        Some(format!(
            "⚠ Extended {}×{} → {}×{} to fit {}×{} tiles",
            from_w,
            from_h,
            fitted.image.width,
            fitted.image.height,
            self.settings.tile_width,
            self.settings.tile_height,
        ))
    }

    /// Take the settings currently in use as a bundle, for saving.
    pub fn settings_bundle(&self) -> SettingsBundle {
        SettingsBundle {
            engine: self.engine,
            tpq_settings: self.tpq_settings.clone(),
            ..SettingsBundle::new(
                self.settings.clone(),
                self.color_correction.clone(),
                self.palette_sort_settings,
            )
        }
    }

    /// The top-left pixel of the image the pipeline runs on, classified by
    /// whether its RGB can serve as a key or shared color.
    pub fn top_left_pixel(&self) -> Option<TopLeftPixel> {
        let [r, g, b, a] = self.color_corrected_image.as_ref()?.top_left_pixel();
        Some(if a == 255 {
            TopLeftPixel::Color([r, g, b])
        } else {
            TopLeftPixel::Transparent
        })
    }

    /// Apply a Qualetize preset and, when it asks for a shared first color,
    /// take that color from the image: an opaque top-left pixel becomes the
    /// shared color, a transparent one switches to transparency from pixels.
    pub fn apply_qualetize_preset(&mut self, preset: QualetizeSettings) {
        self.settings.apply_preset(preset);
        self.tpq_settings.reset_dithering();
        if self.settings.first_color() == FirstColor::Shared {
            match self.top_left_pixel() {
                Some(TopLeftPixel::Color(rgb)) => self.settings.shared_color = rgb,
                Some(TopLeftPixel::Transparent) => self
                    .settings
                    .set_first_color(FirstColor::TransparentFromAlpha),
                None => {}
            }
        }
    }

    /// The app icon textures, small (32 px) and large (256 px), decoding the
    /// bundled PNGs the first time either is needed.
    pub fn app_icons(
        &mut self,
        ctx: &egui::Context,
    ) -> &(egui::TextureHandle, egui::TextureHandle) {
        self.app_icons.get_or_insert_with(|| {
            let load = |name: &str, bytes: &[u8]| {
                let image = image::load_from_memory(bytes)
                    .expect("the bundled app icon decodes")
                    .to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
            };
            let small = load("app_icon_small", include_bytes!("../../assets/icon-32.png"));
            let large = load(
                "app_icon_large",
                include_bytes!("../../assets/icon-256.png"),
            );
            (small, large)
        })
    }

    /// Whether index 0 of every palette is reserved and must not be moved
    /// by the palette sort: Qualetize's transparent first color, or any
    /// tilepalquant mode other than Unique.
    pub fn first_color_pinned(&self) -> bool {
        self.settings.first_color().pins_index_zero()
    }

    /// Mirror the settings to the session file so they survive a restart.
    ///
    /// Writes are held back briefly so dragging a slider does not hit the disk
    /// on every frame. A repaint is scheduled for the deadline so the write
    /// still happens when nothing else asks for a frame.
    pub fn check_and_save_session(&mut self, ctx: &egui::Context) {
        const SAVE_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(750);

        if self.settings_bundle().matches(&self.last_saved_session) {
            self.session_save_deadline = None;
            return;
        }

        let deadline = *self
            .session_save_deadline
            .get_or_insert_with(|| Instant::now() + SAVE_DEBOUNCE);

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            self.save_session_now();
        } else {
            ctx.request_repaint_after(remaining);
        }
    }

    /// Write a pending change straight away, for shutdown.
    pub fn save_session_now(&mut self) {
        let current = self.settings_bundle();
        self.session_save_deadline = None;
        if current.matches(&self.last_saved_session) {
            return;
        }

        if let Err(e) = current.save_session() {
            log::error!("Failed to save session settings: {e}");
        }
        self.last_saved_session = current;
    }

    pub fn check_and_save_preferences(&mut self) {
        if self.preferences != self.last_preferences {
            self.last_preferences = self.preferences.clone();
            if let Err(e) = self.preferences.save() {
                log::error!("Failed to save preferences: {e}");
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

    /// Schedule a (debounced) re-quantization of the color corrected image.
    pub fn request_qualetize(&mut self) {
        self.request_update_qualetized_image = Some(QualetizeRequest {
            time: Instant::now(),
        });
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
            indexed.sorted(sort.mode, sort.order, self.first_color_pinned())
        };

        Some((indexed, image.width, image.height))
    }

    /// Drop the loaded image together with everything derived from it.
    pub fn reset_all_images(&mut self) {
        self.input_path = None;
        self.input_image = None;
        self.tile_fitted_input = None;
        self.tile_fit_toast = None;
        self.color_corrected_image = None;
        self.results.clear();
        self.results_textures.clear();
        self.reset_qualetize_outputs();
    }
}
