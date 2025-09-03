use crate::color_processor::ColorProcessor;
use crate::types::image::ImageDataIndexed;
use crate::types::{BGRA8, ColorCorrection, ImageData, QualetizeSettings};
use egui::{ColorImage, Context};
use std::sync::mpsc;

#[repr(C)]
struct Vec4f {
    f32: [f32; 4],
}

struct ColorCorrectedCache {
    image_data: ImageData,
    input_path: String,
    color_correction: ColorCorrection,
}

// New cache for BGRA data
#[derive(Clone)]
struct BGRACache {
    bgra_data: Vec<BGRA8>,
    width: u32,
    height: u32,
    input_path: String,
    color_correction: ColorCorrection,
}

#[repr(C)]
struct QualetizePlan {
    tile_width: u16,
    tile_height: u16,
    n_palette_colors: u16,
    n_tile_palettes: u16,
    colorspace: u8,
    first_color_is_transparent: u8,
    premultiplied_alpha: u8,
    dither_type: u8,
    dither_level: f32,
    split_ratio: f32,
    n_tile_cluster_passes: u32,
    n_color_cluster_passes: u32,
    color_depth: Vec4f,
    transparent_color: BGRA8,
}

unsafe extern "C" {
    fn Qualetize(
        output_px_data: *mut u8,
        output_palette: *mut BGRA8,
        input_bitmap: *const BGRA8,
        input_palette: *const BGRA8,
        input_width: u32,
        input_height: u32,
        plan: *const QualetizePlan,
        rmse: *mut Vec4f,
    ) -> u8;
}

// Qualetizeの処理結果を格納する構造体
#[derive(Debug)]
pub struct QualetizeResult {
    pub indexed_data: Vec<u8>,
    pub palette_data: Vec<BGRA8>,
    pub settings: QualetizeSettings,
    pub width: u32,
    pub height: u32,
    pub generation_id: u64, // 処理の世代ID
}

pub struct ImageProcessor {
    preview_thread: Option<std::thread::JoinHandle<()>>,
    preview_receiver: Option<mpsc::Receiver<Result<QualetizeResult, String>>>,
    cancel_sender: Option<mpsc::Sender<()>>,
    current_generation_id: u64,                         // 現在の処理世代ID
    active_threads: Vec<std::thread::JoinHandle<()>>,   // アクティブなスレッドのリスト
    color_corrected_cache: Option<ColorCorrectedCache>, // Color corrected image cache
    bgra_cache: Option<BGRACache>,                      // BGRA data cache
}

impl Default for ImageProcessor {
    fn default() -> Self {
        Self {
            preview_thread: None,
            preview_receiver: None,
            cancel_sender: None,
            current_generation_id: 0,
            active_threads: Vec::new(),
            color_corrected_cache: None,
            bgra_cache: None,
        }
    }
}

impl ImageProcessor {
    pub fn new() -> Self {
        Self::default()
    }

    fn parse_rgba_depth(rgba_depth: &str) -> [f32; 4] {
        if rgba_depth.len() == 4 {
            let chars: Vec<char> = rgba_depth.chars().collect();
            [
                Self::char_to_depth(chars[0]),
                Self::char_to_depth(chars[1]),
                Self::char_to_depth(chars[2]),
                Self::char_to_depth(chars[3]),
            ]
        } else {
            [255.0, 255.0, 255.0, 255.0] // Default to 8-bit
        }
    }

    fn char_to_depth(c: char) -> f32 {
        match c {
            '1' => 1.0,
            '2' => 3.0,
            '3' => 7.0,
            '4' => 15.0,
            '5' => 31.0,
            '6' => 63.0,
            '7' => 127.0,
            '8' => 255.0,
            _ => 255.0,
        }
    }

    pub fn get_or_generate_color_corrected_image(
        &mut self,
        input_path: &str,
        color_correction: &ColorCorrection,
        ctx: &Context,
    ) -> Result<ImageData, String> {
        // Check if we have a valid cache
        if let Some(ref cache) = self.color_corrected_cache {
            if cache.input_path == input_path && cache.color_correction == *color_correction {
                log::debug!("Using cached color corrected image");
                return Ok(cache.image_data.clone());
            }
        }

        log::debug!("Generating new color corrected image");
        let corrected_image =
            Self::generate_color_corrected_image(input_path, color_correction, ctx)?;

        // Update cache
        self.color_corrected_cache = Some(ColorCorrectedCache {
            image_data: corrected_image.clone(),
            input_path: input_path.to_string(),
            color_correction: color_correction.clone(),
        });

        Ok(corrected_image)
    }

    pub fn invalidate_color_corrected_cache(&mut self) {
        log::debug!("Invalidating color corrected cache");
        self.color_corrected_cache = None;
        // Also invalidate BGRA cache since it depends on color correction
        self.bgra_cache = None;
    }

    pub fn get_or_generate_bgra_data(
        &mut self,
        input_path: &str,
        color_correction: &ColorCorrection,
        ctx: &Context,
    ) -> Result<(Vec<BGRA8>, u32, u32), String> {
        // Check if we have a valid BGRA cache
        if let Some(ref cache) = self.bgra_cache {
            if cache.input_path == input_path && cache.color_correction == *color_correction {
                log::debug!("Using cached BGRA data");
                return Ok((cache.bgra_data.clone(), cache.width, cache.height));
            }
        }

        log::debug!("Generating new BGRA data");

        // First get the color corrected image
        let corrected_image =
            self.get_or_generate_color_corrected_image(input_path, color_correction, ctx)?;

        // Convert to BGRA
        let width = corrected_image.width;
        let height = corrected_image.height;

        // We need to get the RGBA data from the ColorImage
        // For now, reload the image and apply corrections (this could be optimized further)
        let img = image::open(input_path).map_err(|e| format!("Image loading error: {}", e))?;
        let mut rgba_img = img.to_rgba8();

        // Apply color corrections if any are active
        if ColorProcessor::has_corrections(color_correction) {
            rgba_img = ColorProcessor::apply_corrections(&rgba_img, color_correction);
        }

        let input_data = rgba_img.into_raw();

        // Convert RGBA to BGRA for qualetize
        let mut bgra_data: Vec<BGRA8> = Vec::with_capacity((width * height) as usize);
        for chunk in input_data.chunks_exact(4) {
            bgra_data.push(BGRA8 {
                b: chunk[2],
                g: chunk[1],
                r: chunk[0],
                a: chunk[3],
            });
        }

        // Update cache
        self.bgra_cache = Some(BGRACache {
            bgra_data: bgra_data.clone(),
            width,
            height,
            input_path: input_path.to_string(),
            color_correction: color_correction.clone(),
        });

        Ok((bgra_data, width, height))
    }

    pub fn generate_color_corrected_image(
        input_path: &str,
        color_correction: &ColorCorrection,
        ctx: &Context,
    ) -> Result<ImageData, String> {
        let img = image::open(input_path).map_err(|e| format!("Image loading error: {}", e))?;
        let mut rgba_img = img.to_rgba8();

        // Apply color corrections if any are active
        if ColorProcessor::has_corrections(color_correction) {
            rgba_img = ColorProcessor::apply_corrections(&rgba_img, color_correction);
        }

        let size = [rgba_img.width() as usize, rgba_img.height() as usize];
        let rgba_data = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba_data);
        let texture = ctx.load_texture(
            "color_corrected",
            color_image,
            egui::TextureOptions::NEAREST,
        );

        Ok(ImageData {
            texture: Some(texture),
            width: size[0] as u32,
            height: size[1] as u32,
            rgba_data,
            indexed: None,
        })
    }

    pub fn load_image_from_path(path: &str, ctx: &Context) -> Result<ImageData, String> {
        let img = image::open(path).map_err(|e| format!("Image loading error: {}", e))?;
        let rgba_img = img.to_rgba8();
        let size = [rgba_img.width() as usize, rgba_img.height() as usize];
        let rgba_data = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba_data);
        let texture = ctx.load_texture("input", color_image, egui::TextureOptions::NEAREST);

        Ok(ImageData {
            texture: Some(texture),
            width: size[0] as u32,
            height: size[1] as u32,
            rgba_data,
            indexed: None,
        })
    }

    pub fn start_preview_generation(
        &mut self,
        input_path: String,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
        ctx: &Context,
    ) {
        // Cancel any existing processing
        self.cancel_current_processing();

        // Pre-generate BGRA data to improve responsiveness and avoid redundancy
        let bgra_result = self.get_or_generate_bgra_data(&input_path, &color_correction, ctx);
        let (bgra_data, width, height) = match bgra_result {
            Ok(data) => data,
            Err(e) => {
                log::error!("Failed to generate BGRA data: {}", e);
                return;
            }
        };

        let (result_sender, result_receiver) = mpsc::channel();
        let (cancel_sender, cancel_receiver) = mpsc::channel();
        let generation_id = self.current_generation_id;

        self.preview_receiver = Some(result_receiver);
        self.cancel_sender = Some(cancel_sender);

        let thread = std::thread::spawn(move || {
            let result = Self::generate_preview_from_bgra(
                bgra_data,
                width,
                height,
                settings,
                color_correction,
                cancel_receiver,
                generation_id,
            );
            let _ = result_sender.send(result);
        });

        self.preview_thread = Some(thread);
    }

    pub fn check_preview_complete(&mut self, ctx: &Context) -> Option<Result<ImageData, String>> {
        // 完了した古いスレッドをクリーンアップ
        self.cleanup_finished_threads();

        if let Some(receiver) = &mut self.preview_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.preview_thread = None;
                self.preview_receiver = None;

                return Some(match result {
                    Ok(qualetize_result) => {
                        // 世代IDをチェックして、古い結果は無視
                        if qualetize_result.generation_id == self.current_generation_id {
                            log::debug!(
                                "Accepting result from generation {}",
                                qualetize_result.generation_id
                            );
                            match Self::create_texture_from_qualetize_result(qualetize_result, ctx)
                            {
                                Ok(image_data) => Ok(image_data),
                                Err(e) => Err(e),
                            }
                        } else {
                            log::debug!(
                                "Ignoring outdated result from generation {} (current: {})",
                                qualetize_result.generation_id,
                                self.current_generation_id
                            );
                            return None; // 古い結果は無視
                        }
                    }
                    Err(e) => {
                        if e.contains("Processing cancelled") {
                            return None; // キャンセルされた処理は無視
                        } else {
                            Err(e)
                        }
                    }
                });
            }
        }
        None
    }

    pub fn is_processing(&self) -> bool {
        self.preview_thread.is_some()
    }

    pub fn cancel_current_processing(&mut self) {
        if let Some(cancel_sender) = &self.cancel_sender {
            let _ = cancel_sender.send(()); // キャンセル信号を送信
        }

        // 古いスレッドをバックグラウンドで実行継続させる（結果は無視）
        if let Some(old_thread) = self.preview_thread.take() {
            self.active_threads.push(old_thread);
        }

        // 現在の処理をクリア
        self.preview_receiver = None;
        self.cancel_sender = None;

        // 世代IDを更新（古い結果を無視するため）
        self.current_generation_id += 1;

        // 完了した古いスレッドをクリーンアップ
        self.cleanup_finished_threads();
    }

    fn cleanup_finished_threads(&mut self) {
        self.active_threads.retain(|thread| !thread.is_finished());
    }

    fn create_texture_from_qualetize_result(
        result: QualetizeResult,
        ctx: &Context,
    ) -> Result<ImageData, String> {
        let QualetizeResult {
            indexed_data,
            palette_data,
            settings,
            width,
            height,
            generation_id: _,
        } = result;

        // インデックスカラーデータをRGBA画像に変換
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for &pixel_index in &indexed_data {
            let palette_index = pixel_index as usize;
            if palette_index < palette_data.len() {
                let color = &palette_data[palette_index];
                pixels.extend_from_slice(&[color.r, color.g, color.b, color.a]);
            } else {
                pixels.extend_from_slice(&[0, 0, 0, 255]);
            }
        }

        let size = [width as usize, height as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ctx.load_texture("output", color_image, egui::TextureOptions::NEAREST);

        // パレット情報を直接変換
        let palettes = Self::convert_palette_data(&palette_data, &settings);

        Ok(ImageData {
            texture: Some(texture),
            width: width,
            height: height,
            rgba_data: pixels,
            indexed: Some(ImageDataIndexed {
                palettes_for_ui: palettes,
                palettes: palette_data,
                indexed_pixels: indexed_data,
            }),
        })
    }

    fn convert_palette_data(
        palette_data: &[BGRA8],
        settings: &QualetizeSettings,
    ) -> Vec<Vec<egui::Color32>> {
        let colors_per_palette = settings.n_colors as usize;
        let mut palettes = Vec::new();

        // パレットデータをegui::Color32に変換し、設定に基づいて分割
        let egui_colors: Vec<egui::Color32> = palette_data
            .iter()
            .map(|bgra| egui::Color32::from_rgba_unmultiplied(bgra.r, bgra.g, bgra.b, bgra.a))
            .collect();

        for chunk in egui_colors.chunks(colors_per_palette) {
            palettes.push(chunk.to_vec());
        }

        // 設定されたパレット数まで調整
        while palettes.len() < settings.n_palettes as usize {
            palettes.push(vec![egui::Color32::BLACK; colors_per_palette]);
        }
        palettes.truncate(settings.n_palettes as usize);

        log::debug!(
            "Converted {} palettes with {} colors each",
            palettes.len(),
            colors_per_palette
        );
        palettes
    }

    fn create_qualetize_plan(
        settings: &QualetizeSettings,
        _color_correction: &ColorCorrection,
    ) -> Result<QualetizePlan, String> {
        let rgba_depth = Self::parse_rgba_depth(&settings.rgba_depth);

        Ok(QualetizePlan {
            tile_width: settings.tile_width,
            tile_height: settings.tile_height,
            n_palette_colors: settings.n_colors,
            n_tile_palettes: settings.n_palettes,
            colorspace: settings.color_space.to_id(),
            first_color_is_transparent: if settings.col0_is_clear { 1 } else { 0 },
            premultiplied_alpha: if settings.premul_alpha { 1 } else { 0 },
            dither_type: settings.dither_mode.to_id(),
            dither_level: settings.dither_level,
            split_ratio: settings.split_ratio,
            n_tile_cluster_passes: settings.tile_passes,
            n_color_cluster_passes: settings.color_passes,
            color_depth: Vec4f {
                f32: [rgba_depth[0], rgba_depth[1], rgba_depth[2], rgba_depth[3]],
            },
            transparent_color: settings.clear_color.to_bgra8(),
        })
    }

    fn generate_preview_from_bgra(
        bgra_data: Vec<BGRA8>,
        width: u32,
        height: u32,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
        cancel_receiver: mpsc::Receiver<()>,
        generation_id: u64,
    ) -> Result<QualetizeResult, String> {
        log::info!(
            "Starting preview generation from BGRA data (generation {})",
            generation_id
        );

        // キャンセルチェック
        if cancel_receiver.try_recv().is_ok() {
            log::info!("Processing cancelled for generation {}", generation_id);
            return Err("Processing cancelled".to_string());
        }

        // Use the common qualetize processing function
        let mut qualetize_result = Self::perform_qualetize_processing(
            bgra_data,
            width,
            height,
            settings,
            color_correction,
        )?;

        // Set the generation ID for preview tracking
        qualetize_result.generation_id = generation_id;

        Ok(qualetize_result)
    }

    pub fn save_rgba_image(
        output_path: &str,
        rgba_data: &[u8],
        width: u32,
        height: u32,
        export_format: crate::types::ExportFormat,
    ) -> Result<(), String> {
        use image::{ImageBuffer, Rgba};

        let img_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_data.to_vec())
            .ok_or_else(|| "Failed to create image buffer from RGBA data".to_string())?;

        let dynamic_img = image::DynamicImage::ImageRgba8(img_buffer);

        match export_format {
            crate::types::ExportFormat::PngIndexed => {
                return Err(
                    "Indexed PNG format requires palette data, use ExportableImageData::Indexed"
                        .to_string(),
                );
            }
            crate::types::ExportFormat::Png => {
                dynamic_img
                    .save_with_format(output_path, image::ImageFormat::Png)
                    .map_err(|e| format!("PNG save error: {}", e))?;
            }
            crate::types::ExportFormat::Bmp => {
                dynamic_img
                    .save_with_format(output_path, image::ImageFormat::Bmp)
                    .map_err(|e| format!("BMP save error: {}", e))?;
            }
        }

        log::info!("RGBA image exported successfully to: {}", output_path);
        Ok(())
    }

    pub fn perform_qualetize_processing(
        bgra_data: Vec<BGRA8>,
        width: u32,
        height: u32,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
    ) -> Result<QualetizeResult, String> {
        // Create qualetize plan
        let plan = Self::create_qualetize_plan(&settings, &color_correction)?;

        // Prepare output buffers
        let output_size = (width * height) as usize;
        let mut output_data: Vec<u8> = vec![0; output_size];
        let palette_size = (settings.n_palettes * settings.n_colors) as usize;
        let mut output_palette: Vec<BGRA8> = vec![
            BGRA8 {
                b: 0,
                g: 0,
                r: 0,
                a: 0
            };
            palette_size
        ];
        let mut rmse = Vec4f { f32: [0.0; 4] };

        // Call qualetize
        let result = unsafe {
            Qualetize(
                output_data.as_mut_ptr(),
                output_palette.as_mut_ptr(),
                bgra_data.as_ptr(),
                std::ptr::null(),
                width,
                height,
                &plan,
                &mut rmse,
            )
        };

        if result == 0 {
            return Err("Qualetize processing failed".to_string());
        }

        log::debug!("Qualetize succeeded, RMSE: {:?}", rmse.f32);

        Ok(QualetizeResult {
            indexed_data: output_data,
            palette_data: output_palette,
            settings,
            width,
            height,
            generation_id: 0, // Not needed for export
        })
    }

    pub fn save_indexed_png(
        output_path: &str,
        indexed_pixel_data: &[u8],
        palette_data: &[BGRA8],
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        use std::fs::File;
        use std::io::BufWriter;

        // Create PNG encoder
        let file = File::create(output_path)
            .map_err(|e| format!("Failed to create output file: {}", e))?;
        let ref mut w = BufWriter::new(file);

        let mut encoder = png::Encoder::new(w, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);

        // Convert palette to PNG format (RGB)
        let png_palette: Vec<u8> = palette_data
            .iter()
            .take(256) // PNG indexed mode supports max 256 colors
            .flat_map(|color| [color.r, color.g, color.b])
            .collect();

        // Create transparency array for alpha channel
        let transparency: Vec<u8> = palette_data.iter().take(256).map(|color| color.a).collect();

        encoder.set_palette(png_palette);
        encoder.set_trns(transparency);

        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("Failed to write PNG header: {}", e))?;

        writer
            .write_image_data(indexed_pixel_data)
            .map_err(|e| format!("Failed to write PNG image data: {}", e))?;

        Ok(())
    }
    pub fn save_indexed_bmp(
        output_path: &str,
        indexed_pixel_data: &[u8],
        palette_data: &[BGRA8],
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        // Create 8-bit indexed BMP with palette (always 256 entries)
        let palette_size = palette_data.len().min(256); // Max 256 colors for 8-bit
        let row_size = ((width + 3) / 4) * 4; // 4-byte aligned for 8-bit data
        let image_size = row_size * height;
        let palette_bytes = 256 * 4; // Always 256 palette entries * 4 bytes each (BGRA)
        let data_offset = 54 + palette_bytes; // Header + palette
        let file_size = data_offset + image_size;

        let mut bmp_data = Vec::with_capacity(file_size as usize);

        // BMP File Header (14 bytes)
        bmp_data.extend_from_slice(b"BM"); // Signature
        bmp_data.extend_from_slice(&(file_size as u32).to_le_bytes()); // File size
        bmp_data.extend_from_slice(&[0, 0, 0, 0]); // Reserved
        bmp_data.extend_from_slice(&(data_offset as u32).to_le_bytes()); // Data offset

        // BMP Info Header (40 bytes)
        bmp_data.extend_from_slice(&40u32.to_le_bytes()); // Header size
        bmp_data.extend_from_slice(&(width as i32).to_le_bytes()); // Width
        bmp_data.extend_from_slice(&(height as i32).to_le_bytes()); // Height
        bmp_data.extend_from_slice(&1u16.to_le_bytes()); // Planes
        bmp_data.extend_from_slice(&8u16.to_le_bytes()); // Bits per pixel (8-bit indexed)
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Compression
        bmp_data.extend_from_slice(&(image_size as u32).to_le_bytes()); // Image size
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // X pixels per meter
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Y pixels per meter
        bmp_data.extend_from_slice(&256u32.to_le_bytes()); // Colors used (always 256 for 8-bit)
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Important colors

        // Color palette (BGRA format, 4 bytes per color)
        for i in 0..palette_size {
            let color = &palette_data[i];
            bmp_data.push(color.b); // Blue
            bmp_data.push(color.g); // Green
            bmp_data.push(color.r); // Red
            bmp_data.push(color.a); // Alpha (reserved in BMP, usually 0)
        }

        // Fill remaining palette entries if less than 256
        for _ in palette_size..256 {
            bmp_data.extend_from_slice(&[0, 0, 0, 0]);
        }

        // Image data (bottom-up, 8-bit indexed)
        for y in (0..height).rev() {
            for x in 0..width {
                let pixel_idx = (y * width + x) as usize;
                if pixel_idx < indexed_pixel_data.len() {
                    bmp_data.push(indexed_pixel_data[pixel_idx]);
                } else {
                    bmp_data.push(0);
                }
            }
            // Add padding if necessary
            let padding = row_size - width;
            for _ in 0..padding {
                bmp_data.push(0);
            }
        }

        std::fs::write(output_path, bmp_data).map_err(|e| format!("File write error: {}", e))?;

        Ok(())
    }
}
