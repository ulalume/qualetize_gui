use egui::{TextureHandle, Vec2};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ColorSpace {
    Srgb,
    RgbLinear,
    Ycbcr,
    Ycocg,
    Cielab,
    Ictcp,
    Oklab,
    RgbPsy,
    YcbcrPsy,
    YcocgPsy,
}

impl ColorSpace {
    pub fn display_name(&self) -> &'static str {
        match self {
            ColorSpace::Srgb => "sRGB",
            ColorSpace::RgbLinear => "RGB Linear",
            ColorSpace::Ycbcr => "YCbCr",
            ColorSpace::Ycocg => "YCoCg",
            ColorSpace::Cielab => "CIELAB",
            ColorSpace::Ictcp => "ICtCp",
            ColorSpace::Oklab => "OkLab",
            ColorSpace::RgbPsy => "RGB + Psyopt",
            ColorSpace::YcbcrPsy => "YCbCr + Psyopt",
            ColorSpace::YcocgPsy => "YCoCg + Psyopt",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ColorSpace::Srgb => "Standard RGB color space",
            ColorSpace::RgbLinear => "Linear RGB color space",
            ColorSpace::Ycbcr => "Luma + Chroma color space",
            ColorSpace::Ycocg => "Luma + Co/Cg color space",
            ColorSpace::Cielab => {
                "CIE L*a*b* color space\nNOTE: CIELAB has poor performance in most cases"
            }
            ColorSpace::Ictcp => "ITU-R Rec. 2100 ICtCp color space",
            ColorSpace::Oklab => "OkLab perceptual color space",
            ColorSpace::RgbPsy => {
                "RGB with psychovisual optimization\n(Non-linear light, weighted components)"
            }
            ColorSpace::YcbcrPsy => {
                "YCbCr with psychovisual optimization\n(Non-linear luma, weighted chroma)"
            }
            ColorSpace::YcocgPsy => "YCoCg with psychovisual optimization\n(Non-linear luma)",
        }
    }

    pub fn to_id(&self) -> u8 {
        match self {
            ColorSpace::Srgb => 0,
            ColorSpace::RgbLinear => 1,
            ColorSpace::Ycbcr => 2,
            ColorSpace::Ycocg => 3,
            ColorSpace::Cielab => 4,
            ColorSpace::Ictcp => 5,
            ColorSpace::Oklab => 6,
            ColorSpace::RgbPsy => 7,
            ColorSpace::YcbcrPsy => 8,
            ColorSpace::YcocgPsy => 9,
        }
    }

    pub fn all() -> &'static [ColorSpace] {
        &[
            ColorSpace::Srgb,
            ColorSpace::RgbLinear,
            ColorSpace::Ycbcr,
            ColorSpace::Ycocg,
            ColorSpace::Cielab,
            ColorSpace::Ictcp,
            ColorSpace::Oklab,
            ColorSpace::RgbPsy,
            ColorSpace::YcbcrPsy,
            ColorSpace::YcocgPsy,
        ]
    }
}

impl Default for ColorSpace {
    fn default() -> Self {
        ColorSpace::RgbLinear
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DitherMode {
    None,
    Floyd,
    Atkinson,
    Checker,
    Ord2,
    Ord4,
    Ord8,
    Ord16,
    Ord32,
    Ord64,
}

impl DitherMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            DitherMode::None => "None",
            DitherMode::Floyd => "Floyd-Steinberg",
            DitherMode::Atkinson => "Atkinson",
            DitherMode::Checker => "Checkerboard",
            DitherMode::Ord2 => "2x2 Ordered",
            DitherMode::Ord4 => "4x4 Ordered",
            DitherMode::Ord8 => "8x8 Ordered",
            DitherMode::Ord16 => "16x16 Ordered",
            DitherMode::Ord32 => "32x32 Ordered",
            DitherMode::Ord64 => "64x64 Ordered",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            DitherMode::None => "No dithering",
            DitherMode::Floyd => "Floyd-Steinberg error diffusion (default level: 0.5)",
            DitherMode::Atkinson => "Atkinson error diffusion (default level: 0.5)",
            DitherMode::Checker => "Checkerboard dithering (default level: 1.0)",
            DitherMode::Ord2 => "2x2 ordered dithering (default level: 1.0)",
            DitherMode::Ord4 => "4x4 ordered dithering (default level: 1.0)",
            DitherMode::Ord8 => "8x8 ordered dithering (default level: 1.0)",
            DitherMode::Ord16 => "16x16 ordered dithering (default level: 1.0)",
            DitherMode::Ord32 => "32x32 ordered dithering (default level: 1.0)",
            DitherMode::Ord64 => "64x64 ordered dithering (default level: 1.0)",
        }
    }

    pub fn to_id(&self) -> u8 {
        match self {
            DitherMode::None => 0,
            DitherMode::Floyd => 0xFE,
            DitherMode::Atkinson => 0xFD,
            DitherMode::Checker => 0xFF,
            DitherMode::Ord2 => 2,
            DitherMode::Ord4 => 4,
            DitherMode::Ord8 => 6,
            DitherMode::Ord16 => 7,
            DitherMode::Ord32 => 8,
            DitherMode::Ord64 => 9,
        }
    }

    pub fn all() -> &'static [DitherMode] {
        &[
            DitherMode::None,
            DitherMode::Floyd,
            DitherMode::Atkinson,
            DitherMode::Checker,
            DitherMode::Ord2,
            DitherMode::Ord4,
            DitherMode::Ord8,
            DitherMode::Ord16,
            DitherMode::Ord32,
            DitherMode::Ord64,
        ]
    }
}

impl Default for DitherMode {
    fn default() -> Self {
        DitherMode::Floyd
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExportFormat {
    PngIndexed,
    Bmp,
}

impl ExportFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            ExportFormat::PngIndexed => "PNG",
            ExportFormat::Bmp => "BMP",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::PngIndexed => "png",
            ExportFormat::Bmp => "bmp",
        }
    }

    pub fn all() -> &'static [ExportFormat] {
        &[ExportFormat::Bmp, ExportFormat::PngIndexed]
    }
}

impl Default for ExportFormat {
    fn default() -> Self {
        ExportFormat::PngIndexed
    }
}

#[derive(Clone)]
pub struct ImageData {
    pub texture: Option<TextureHandle>,
    pub size: Vec2,
    pub palettes: Vec<Vec<egui::Color32>>, // パレット情報を追加
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorCorrection {
    pub brightness: f32, // -1.0 to 1.0
    pub contrast: f32,   // 0.0 to 2.0
    pub gamma: f32,      // 0.1 to 3.0
    pub saturation: f32, // 0.0 to 2.0
    pub hue_shift: f32,  // -180.0 to 180.0 degrees
    pub shadows: f32,    // -1.0 to 1.0
    pub highlights: f32, // -1.0 to 1.0
}

impl Default for ColorCorrection {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 1.0,
            gamma: 1.0,
            saturation: 1.0,
            hue_shift: 0.0,
            shadows: 0.0,
            highlights: 0.0,
        }
    }
}

impl Default for ImageData {
    fn default() -> Self {
        Self {
            texture: None,
            size: Vec2::ZERO,
            palettes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct QualetizeSettings {
    pub tile_width: u16,
    pub tile_height: u16,
    pub n_palettes: u16,
    pub n_colors: u16,
    pub rgba_depth: String,
    pub premul_alpha: bool,
    pub color_space: ColorSpace,
    pub dither_mode: DitherMode,
    pub dither_level: f32,
    pub tile_passes: u32,
    pub color_passes: u32,
    pub split_ratio: f32,
    pub col0_is_clear: bool,
    pub clear_color: String,
}

impl Default for QualetizeSettings {
    fn default() -> Self {
        Self {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 1,
            n_colors: 16,
            rgba_depth: "3331".to_string(),
            premul_alpha: false,
            color_space: ColorSpace::default(),
            dither_mode: DitherMode::default(),
            dither_level: 0.5,
            tile_passes: 1000,
            color_passes: 100,
            split_ratio: -1.0,
            col0_is_clear: false,
            clear_color: "none".to_string(),
        }
    }
}

pub struct AppState {
    // ファイル管理
    pub input_path: Option<String>,
    pub output_path: Option<String>,
    pub output_name: String,
    pub input_image: ImageData,
    pub output_image: ImageData,

    // UI状態
    pub show_advanced: bool,
    pub preview_ready: bool,
    pub selected_export_format: ExportFormat,

    // display options
    pub show_original_image: bool,
    pub show_palettes: bool,

    // 画像表示制御
    pub zoom: f32,
    pub pan_offset: Vec2,

    // 設定
    pub settings: QualetizeSettings,
    pub color_correction: ColorCorrection,

    // 処理状態
    // pub processing: bool,
    pub preview_processing: bool,
    pub result_message: String,
    pub settings_changed: bool,

    // 警告状態
    pub tile_size_warning: bool,
    pub tile_size_warning_message: String,

    // デバウンス機能
    pub last_settings_change_time: Option<std::time::Instant>,
    pub debounce_delay: std::time::Duration,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            input_path: None,
            output_path: None,
            output_name: String::new(),
            input_image: ImageData::default(),
            output_image: ImageData::default(),

            show_advanced: false,
            preview_ready: false,
            selected_export_format: ExportFormat::default(),

            show_original_image: true,
            show_palettes: true,

            zoom: 1.0,
            pan_offset: Vec2::ZERO,

            settings: QualetizeSettings::default(),
            color_correction: ColorCorrection::default(),

            // processing: false,
            preview_processing: false,
            result_message: String::new(),
            settings_changed: false,

            // 警告状態
            tile_size_warning: false,
            tile_size_warning_message: String::new(),

            // デバウンス機能 - 100msの遅延（応答性向上）
            last_settings_change_time: None,
            debounce_delay: std::time::Duration::from_millis(100),
        }
    }
}
