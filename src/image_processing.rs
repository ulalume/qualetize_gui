use crate::color_correction::ColorProcessor;
use crate::types::{ColorCorrection, ImageData, QualetizeSettings};
use egui::{ColorImage, Context};
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::mpsc;

unsafe extern "C" {
    fn qualetize_cli_entry(argc: i32, argv: *const *const c_char) -> i32;
}

pub struct ImageProcessor {
    preview_thread: Option<std::thread::JoinHandle<()>>,
    preview_receiver: Option<mpsc::Receiver<Result<Vec<u8>, String>>>,
}

impl Default for ImageProcessor {
    fn default() -> Self {
        Self {
            preview_thread: None,
            preview_receiver: None,
        }
    }
}

impl ImageProcessor {
    pub fn new() -> Self {
        Self::default()
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
        })
    }

    pub fn start_preview_generation(
        &mut self,
        input_path: String,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
    ) {
        // Cancel any existing processing
        if self.preview_thread.is_some() {
            self.preview_thread = None;
            self.preview_receiver = None;
        }

        let (sender, receiver) = mpsc::channel();
        self.preview_receiver = Some(receiver);

        let thread = std::thread::spawn(move || {
            let result = Self::generate_preview_internal(input_path, settings, color_correction);
            let _ = sender.send(result);
        });

        self.preview_thread = Some(thread);
    }

    pub fn check_preview_complete(&mut self, ctx: &Context) -> Option<Result<ImageData, String>> {
        if let Some(receiver) = &mut self.preview_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.preview_thread = None;
                self.preview_receiver = None;

                return Some(match result {
                    Ok(image_data) => match Self::create_texture_from_bmp_data(&image_data, ctx) {
                        Ok(image_data) => Ok(image_data),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(e),
                });
            }
        }
        None
    }

    pub fn is_processing(&self) -> bool {
        self.preview_thread.is_some()
    }

    fn create_texture_from_bmp_data(image_data: &[u8], ctx: &Context) -> Result<ImageData, String> {
        let img = image::load_from_memory(image_data)
            .map_err(|e| format!("BMP data loading error: {}", e))?;

        let rgba_img = img.to_rgba8();
        let size = [rgba_img.width() as usize, rgba_img.height() as usize];
        let pixels = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ctx.load_texture("output", color_image, egui::TextureOptions::NEAREST);

        Ok(ImageData {
            texture: Some(texture),
            size: egui::Vec2::new(size[0] as f32, size[1] as f32),
        })
    }

    fn generate_preview_internal(
        input_path: String,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
    ) -> Result<Vec<u8>, String> {
        println!("Starting preview generation for: {}", input_path);

        // Create temporary BMP file
        let img = image::open(&input_path).map_err(|e| format!("Image loading error: {}", e))?;
        let temp_dir = std::env::temp_dir();
        let temp_input = temp_dir.join(format!(
            "temp_input_{}.bmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let temp_output = temp_dir.join(format!(
            "temp_output_{}.bmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));

        // Convert to RGBA and apply color corrections
        let mut rgba_img = img.to_rgba8();

        // Apply color corrections if any are active
        if ColorProcessor::has_corrections(&color_correction) {
            println!("Applying color corrections: {:?}", color_correction);
            rgba_img = ColorProcessor::apply_corrections(&rgba_img, &color_correction);
        }
        rgba_img
            .save(&temp_input)
            .map_err(|e| format!("BMP save error: {}", e))?;

        println!(
            "Running qualetize with temp files: {} -> {}",
            temp_input.display(),
            temp_output.display()
        );

        // Run qualetize
        let result = Self::run_qualetize_with_settings(
            temp_input.to_str().unwrap(),
            temp_output.to_str().unwrap(),
            settings,
        );

        // Read result
        let output_data = match result {
            Ok(_) => {
                println!(
                    "Qualetize succeeded, reading output file: {}",
                    temp_output.display()
                );
                let file_exists = temp_output.exists();
                println!("Output file exists: {}", file_exists);
                if file_exists {
                    let file_size = std::fs::metadata(&temp_output)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    println!("Output file size: {} bytes", file_size);
                }
                std::fs::read(&temp_output).map_err(|e| format!("Output file read error: {}", e))
            }
            Err(e) => {
                println!("Qualetize failed with error: {}", e);
                Err(e)
            }
        };

        // Clean up temporary files
        let _ = std::fs::remove_file(&temp_input);
        let _ = std::fs::remove_file(&temp_output);

        output_data
    }

    fn run_qualetize_with_settings(
        input_path: &str,
        output_path: &str,
        settings: QualetizeSettings,
    ) -> Result<(), String> {
        let mut args = vec![
            "qualetize".to_string(),
            input_path.to_string(),
            output_path.to_string(),
        ];

        // Add option arguments
        args.push(format!("-tw:{}", settings.tile_width));
        args.push(format!("-th:{}", settings.tile_height));
        args.push(format!("-npal:{}", settings.n_palettes));
        args.push(format!("-cols:{}", settings.n_colors));
        args.push(format!("-rgba:{}", settings.rgba_depth));
        args.push(format!(
            "-premulalpha:{}",
            if settings.premul_alpha { "y" } else { "n" }
        ));
        args.push(format!("-colspace:{}", settings.color_space));
        args.push(format!(
            "-dither:{},{}",
            settings.dither_mode, settings.dither_level
        ));
        args.push(format!("-tilepasses:{}", settings.tile_passes));
        args.push(format!("-colourpasses:{}", settings.color_passes));
        args.push(format!("-splitratio:{}", settings.split_ratio));
        args.push(format!(
            "-col0isclear:{}",
            if settings.col0_is_clear { "y" } else { "n" }
        ));
        args.push(format!("-clearcol:{}", settings.clear_color));

        // Call C function
        let c_args: Vec<CString> = args
            .iter()
            .map(|arg| CString::new(arg.as_str()).unwrap())
            .collect();
        let c_ptrs: Vec<*const c_char> = c_args.iter().map(|arg| arg.as_ptr()).collect();

        println!(
            "Calling qualetize_cli_entry with {} arguments",
            c_args.len()
        );
        for (i, arg) in args.iter().enumerate() {
            println!("  Arg {}: {}", i, arg);
        }

        let ret = unsafe { qualetize_cli_entry(c_args.len() as i32, c_ptrs.as_ptr()) };

        println!("qualetize_cli_entry returned: {}", ret);

        if ret == 0 {
            Ok(())
        } else {
            Err(format!("Processing error: exit code {}", ret))
        }
    }

    pub fn export_image(
        input_path: String,
        output_path: String,
        settings: QualetizeSettings,
        color_correction: ColorCorrection,
    ) -> Result<(), String> {
        // Convert input image to BMP
        let img = image::open(&input_path).map_err(|e| format!("Image loading error: {}", e))?;
        let temp_dir = std::env::temp_dir();
        let temp_input = temp_dir.join(format!(
            "temp_export_input_{}.bmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));

        // Convert to RGBA and apply color corrections
        let mut rgba_img = img.to_rgba8();

        // Apply color corrections if any are active
        if ColorProcessor::has_corrections(&color_correction) {
            rgba_img = ColorProcessor::apply_corrections(&rgba_img, &color_correction);
        }
        rgba_img
            .save(&temp_input)
            .map_err(|e| format!("BMP save error: {}", e))?;

        // Run qualetize
        let result =
            Self::run_qualetize_with_settings(temp_input.to_str().unwrap(), &output_path, settings);

        // Clean up temporary file
        let _ = std::fs::remove_file(&temp_input);

        result.map(|_| ())
    }
}
