use egui::{TextureHandle, Vec2};

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
    pub color_space: String,
    pub dither_mode: String,
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
            n_palettes: 1,                  // デフォルト値: 1
            n_colors: 16,                   // デフォルト値: 16
            rgba_depth: "3331".to_string(), // デフォルト値: 3331
            premul_alpha: false,
            color_space: "srgb".to_string(), // デフォルト値: sRGB
            dither_mode: "floyd".to_string(),
            dither_level: 0.5,
            tile_passes: 1000, // デフォルト値: 1000
            color_passes: 100, // デフォルト値: 100
            split_ratio: -1.0,
            col0_is_clear: true,
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

            zoom: 1.0,
            pan_offset: Vec2::ZERO,

            settings: QualetizeSettings::default(),
            color_correction: ColorCorrection::default(),

            // processing: false,
            preview_processing: false,
            result_message: String::new(),
            settings_changed: false,
            
            // デバウンス機能 - 100msの遅延（応答性向上）
            last_settings_change_time: None,
            debounce_delay: std::time::Duration::from_millis(100),
        }
    }
}
