use crate::color_correction::ColorProcessor;
use crate::types::{ColorCorrection, ImageData, QualetizeSettings};
use egui::{ColorImage, Context};
use std::sync::mpsc;

#[repr(C)]
struct Vec4f {
    f32: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct BGRA8 {
    b: u8,
    g: u8,
    r: u8,
    a: u8,
}

#[repr(C)]
struct QualetizePlan {
    tile_width: u16,
    tile_height: u16,
    n_palette_colours: u16,
    n_tile_palettes: u16,
    colourspace: u8,
    first_colour_is_transparent: u8,
    premultiplied_alpha: u8,
    dither_type: u8,
    dither_level: f32,
    split_ratio: f32,
    n_tile_cluster_passes: u32,
    n_colour_cluster_passes: u32,
    colour_depth: Vec4f,
    transparent_colour: BGRA8,
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
struct QualetizeResult {
    image_data: Vec<u8>,
    palette_data: Vec<BGRA8>,
    settings: QualetizeSettings,
    width: u32,
    height: u32,
    generation_id: u64, // 処理の世代ID
}

pub struct ImageProcessor {
    preview_thread: Option<std::thread::JoinHandle<()>>,
    preview_receiver: Option<mpsc::Receiver<Result<QualetizeResult, String>>>,
    cancel_sender: Option<mpsc::Sender<()>>,
    current_generation_id: u64,                       // 現在の処理世代ID
    active_threads: Vec<std::thread::JoinHandle<()>>, // アクティブなスレッドのリスト
}

impl Default for ImageProcessor {
    fn default() -> Self {
        Self {
            preview_thread: None,
            preview_receiver: None,
            cancel_sender: None,
            current_generation_id: 0,
            active_threads: Vec::new(),
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

    fn parse_clear_color(clear_color: &str) -> BGRA8 {
        if clear_color == "none" {
            BGRA8 {
                b: 0,
                g: 0,
                r: 0,
                a: 0,
            }
        } else if let Ok(color_val) = u32::from_str_radix(clear_color.trim_start_matches("0x"), 16)
        {
            BGRA8 {
                b: (color_val & 0xFF) as u8,
                g: ((color_val >> 8) & 0xFF) as u8,
                r: ((color_val >> 16) & 0xFF) as u8,
                a: ((color_val >> 24) & 0xFF) as u8,
            }
        } else {
            BGRA8 {
                b: 0,
                g: 0,
                r: 0,
                a: 0,
            }
        }
    }

    pub fn load_image_from_path(path: &str, ctx: &Context) -> Result<ImageData, String> {
        let img = image::open(path).map_err(|e| format!("Image loading error: {}", e))?;
        let rgba_img = img.to_rgba8();
        let size = [rgba_img.width() as usize, rgba_img.height() as usize];
        let pixels = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ctx.load_texture("input", color_image, egui::TextureOptions::NEAREST);

        Ok(ImageData {
            texture: Some(texture),
            size: egui::Vec2::new(size[0] as f32, size[1] as f32),
            palettes: Vec::new(), // 入力画像にはパレット情報なし
        })
    }

    pub fn start_preview_generation(
        &mut self,
        input_path: String,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
    ) {
        // Cancel any existing processing
        self.cancel_current_processing();

        let (result_sender, result_receiver) = mpsc::channel();
        let (cancel_sender, cancel_receiver) = mpsc::channel();
        let generation_id = self.current_generation_id;

        self.preview_receiver = Some(result_receiver);
        self.cancel_sender = Some(cancel_sender);

        let thread = std::thread::spawn(move || {
            let result = Self::generate_preview_internal(
                input_path,
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
            image_data,
            palette_data,
            settings,
            width,
            height,
            generation_id: _,
        } = result;

        // インデックスカラーデータをRGBA画像に変換
        let mut rgba_pixels = Vec::with_capacity((width * height * 4) as usize);
        for &pixel_index in &image_data {
            let palette_index = pixel_index as usize;
            if palette_index < palette_data.len() {
                let color = &palette_data[palette_index];
                rgba_pixels.extend_from_slice(&[color.r, color.g, color.b, color.a]);
            } else {
                rgba_pixels.extend_from_slice(&[0, 0, 0, 255]);
            }
        }

        let size = [width as usize, height as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba_pixels);
        let texture = ctx.load_texture("output", color_image, egui::TextureOptions::NEAREST);

        // パレット情報を直接変換
        let palettes = Self::convert_palette_data(&palette_data, &settings);

        Ok(ImageData {
            texture: Some(texture),
            size: egui::Vec2::new(width as f32, height as f32),
            palettes,
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
        let clear_color = Self::parse_clear_color(&settings.clear_color);

        Ok(QualetizePlan {
            tile_width: settings.tile_width,
            tile_height: settings.tile_height,
            n_palette_colours: settings.n_colors,
            n_tile_palettes: settings.n_palettes,
            colourspace: settings.color_space.to_id(),
            first_colour_is_transparent: if settings.col0_is_clear { 1 } else { 0 },
            premultiplied_alpha: if settings.premul_alpha { 1 } else { 0 },
            dither_type: settings.dither_mode.to_id(),
            dither_level: settings.dither_level,
            split_ratio: settings.split_ratio,
            n_tile_cluster_passes: settings.tile_passes,
            n_colour_cluster_passes: settings.color_passes,
            colour_depth: Vec4f {
                f32: [rgba_depth[0], rgba_depth[1], rgba_depth[2], rgba_depth[3]],
            },
            transparent_colour: clear_color,
        })
    }

    fn generate_preview_internal(
        input_path: String,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
        cancel_receiver: mpsc::Receiver<()>,
        generation_id: u64,
    ) -> Result<QualetizeResult, String> {
        log::info!(
            "Starting preview generation for: {} (generation {})",
            input_path,
            generation_id
        );

        // キャンセルチェック
        if cancel_receiver.try_recv().is_ok() {
            log::info!("Processing cancelled for generation {}", generation_id);
            return Err("Processing cancelled".to_string());
        }

        // Load and process image
        let img = image::open(&input_path).map_err(|e| format!("Image loading error: {}", e))?;
        let mut rgba_img = img.to_rgba8();

        // Apply color corrections if any are active
        if ColorProcessor::has_corrections(&color_correction) {
            log::debug!("Applying color corrections: {:?}", color_correction);
            rgba_img = ColorProcessor::apply_corrections(&rgba_img, &color_correction);
        }

        let width = rgba_img.width();
        let height = rgba_img.height();
        let input_data = rgba_img.into_raw();

        // キャンセルチェック
        if cancel_receiver.try_recv().is_ok() {
            return Err("Processing cancelled".to_string());
        }

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

        // 結果を構造体として返す
        Ok(QualetizeResult {
            image_data: output_data,
            palette_data: output_palette,
            settings,
            width,
            height,
            generation_id,
        })
    }

    fn create_bmp_from_indexed(
        index_data: &[u8],
        palette_data: &[BGRA8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
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
                if pixel_idx < index_data.len() {
                    bmp_data.push(index_data[pixel_idx]);
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

        Ok(bmp_data)
    }

    pub fn export_image(
        input_path: String,
        output_path: String,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
        export_format: crate::types::ExportFormat,
    ) -> Result<(), String> {
        // Load and process image
        let img = image::open(&input_path).map_err(|e| format!("Image loading error: {}", e))?;
        let mut rgba_img = img.to_rgba8();

        // Apply color corrections if any are active
        if ColorProcessor::has_corrections(&color_correction) {
            rgba_img = ColorProcessor::apply_corrections(&rgba_img, &color_correction);
        }

        let width = rgba_img.width();
        let height = rgba_img.height();
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

        // インデックスカラーデータをRGBA画像に変換
        let mut output_rgba: Vec<u8> = Vec::with_capacity(output_size * 4);
        for &pixel_index in &output_data {
            let palette_index = pixel_index as usize;
            if palette_index < output_palette.len() {
                let color = &output_palette[palette_index];
                output_rgba.extend_from_slice(&[color.r, color.g, color.b, color.a]);
            } else {
                output_rgba.extend_from_slice(&[0, 0, 0, 255]);
            }
        }

        // 選択されたフォーマットで保存
        Self::save_image_by_format(
            &output_path,
            &output_data,
            &output_palette,
            width,
            height,
            export_format,
        )?;

        log::info!("Export completed successfully to: {}", output_path);
        Ok(())
    }

    fn save_image_by_format(
        output_path: &str,
        index_data: &[u8],
        palette_data: &[BGRA8],
        width: u32,
        height: u32,
        export_format: crate::types::ExportFormat,
    ) -> Result<(), String> {
        use crate::types::ExportFormat;

        match export_format {
            ExportFormat::PngIndexed => {
                Self::save_indexed_png(output_path, index_data, palette_data, width, height)?;
            }
            ExportFormat::Bmp => {
                let bmp_data =
                    Self::create_bmp_from_indexed(index_data, palette_data, width, height)?;
                std::fs::write(output_path, bmp_data)
                    .map_err(|e| format!("Failed to write BMP file: {}", e))?;
            }
        }

        log::info!(
            "Export completed successfully to: {} (format: {})",
            output_path,
            export_format.display_name()
        );
        Ok(())
    }

    fn save_indexed_png(
        output_path: &str,
        index_data: &[u8],
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
            .write_image_data(index_data)
            .map_err(|e| format!("Failed to write PNG image data: {}", e))?;

        Ok(())
    }
}
