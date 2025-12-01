use crate::types::qualetize::{Qualetize, QualetizePlanOwned, Vec4f};
use crate::types::{BGRA8, ImageData, QualetizeSettings};
use egui::Context;
use std::sync::mpsc;

#[derive(Debug)]
pub struct QualetizeResult {
    pub indexed_data: Vec<u8>,
    pub palette_data: Vec<BGRA8>,
    pub settings: QualetizeSettings,
    pub width: u32,
    pub height: u32,
    pub generation_id: u64,
}

#[derive(Default)]
pub struct ImageProcessor {
    preview_thread: Option<std::thread::JoinHandle<()>>,
    preview_receiver: Option<mpsc::Receiver<Result<QualetizeResult, String>>>,
    cancel_sender: Option<mpsc::Sender<()>>,
    current_generation_id: u64,
    active_threads: Vec<std::thread::JoinHandle<()>>,
}

impl ImageProcessor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_qualetize(
        &mut self,
        color_corrected_image: &ImageData,
        settings: QualetizeSettings,
    ) {
        // Cancel any existing processing
        self.cancel_current_processing();

        // Pre-generate BGRA data to improve responsiveness and avoid redundancy
        let bgra_result = self.generate_bgra_data(color_corrected_image);
        let (bgra_data, width, height) = match bgra_result {
            Ok(data) => data,
            Err(e) => {
                log::error!("Failed to generate BGRA data: {e}");
                return;
            }
        };

        let (result_sender, result_receiver) = mpsc::channel();
        let (cancel_sender, cancel_receiver) = mpsc::channel();
        let generation_id = self.current_generation_id;

        self.preview_receiver = Some(result_receiver);
        self.cancel_sender = Some(cancel_sender);

        let thread = std::thread::spawn(move || {
            let result = Self::generate_preview(
                bgra_data,
                width,
                height,
                settings,
                cancel_receiver,
                generation_id,
            );
            let _ = result_sender.send(result);
        });
        self.preview_thread = Some(thread);
    }

    pub fn generate_bgra_data(
        &mut self,
        color_corrected_image: &ImageData,
    ) -> Result<(Vec<BGRA8>, u32, u32), String> {
        // Convert to BGRA
        let width = color_corrected_image.width;
        let height = color_corrected_image.height;

        let input_data = &color_corrected_image.rgba_data;

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
        Ok((bgra_data, width, height))
    }

    pub fn check_preview_complete(&mut self, ctx: &Context) -> Option<Result<ImageData, String>> {
        self.cleanup_finished_threads();

        if let Some(receiver) = &mut self.preview_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.preview_thread = None;
            self.preview_receiver = None;

            return Some(match result {
                Ok(qualetize_result) => {
                    if qualetize_result.generation_id == self.current_generation_id {
                        log::debug!(
                            "Accepting result from generation {}",
                            qualetize_result.generation_id
                        );
                        match ImageData::create_from_qualetize_result(qualetize_result, ctx) {
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
        self.preview_thread = None;
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

    fn generate_preview(
        bgra_data: Vec<BGRA8>,
        width: u32,
        height: u32,
        settings: QualetizeSettings,
        cancel_receiver: mpsc::Receiver<()>,
        generation_id: u64,
    ) -> Result<QualetizeResult, String> {
        log::info!("Starting preview generation from BGRA data (generation {generation_id})");

        // Check for cancellation
        if cancel_receiver.try_recv().is_ok() {
            log::info!("Processing cancelled for generation {generation_id}");
            return Err("Processing cancelled".to_string());
        }

        // Use the common qualetize processing function
        let mut qualetize_result =
            Self::perform_qualetize_processing(bgra_data, width, height, settings)?;

        // Set the generation ID for preview tracking
        qualetize_result.generation_id = generation_id;

        Ok(qualetize_result)
    }

    pub fn perform_qualetize_processing(
        bgra_data: Vec<BGRA8>,
        width: u32,
        height: u32,
        settings: QualetizeSettings,
    ) -> Result<QualetizeResult, String> {
        // Create qualetize plan
        let plan = QualetizePlanOwned::from(settings.clone());

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
                plan.as_ptr(),
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
}
