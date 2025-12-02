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

struct ClusterMember {
    indices: Vec<u8>,
    blurred_colors: Vec<[u8; 4]>,
}

struct ClusterRep {
    indices: Vec<u8>,
    blurred_colors: Vec<[u8; 4]>,
    members: Vec<ClusterMember>,
    insert_cursor: usize,
}

pub struct TileReduceOptions {
    pub tile_width: u16,
    pub tile_height: u16,
    pub threshold: f32,
    pub allow_flip_x: bool,
    pub allow_flip_y: bool,
    pub use_blur: bool,
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

    pub fn reduce_tiles_indexed(
        indexed: &mut [u8],
        palette: &[BGRA8],
        width: u32,
        height: u32,
        opts: &TileReduceOptions,
    ) -> usize {
        // Quality/speed tuning
        let medoid_recompute_interval = 8;
        let max_members_tracked = 64;

        if opts.tile_width == 0
            || opts.tile_height == 0
            || !width.is_multiple_of(opts.tile_width as u32)
            || !height.is_multiple_of(opts.tile_height as u32)
        {
            log::warn!("Tile reduce post-process skipped due to incompatible dimensions");
            return 0;
        }

        let tiles_x = width / opts.tile_width as u32;
        let tiles_y = height / opts.tile_height as u32;
        let tile_w = opts.tile_width as usize;
        let tile_h = opts.tile_height as usize;
        let tile_area = tile_w * tile_h;
        let stride = width as usize;
        let orientation_maps =
            Orientation::maps(tile_w, tile_h, opts.allow_flip_x, opts.allow_flip_y);

        let mut tile_indices_buf = vec![0u8; tile_area];
        let mut tile_colors_buf = vec![[0u8; 4]; tile_area];
        let mut tile_blur_buf = vec![[0u8; 4]; tile_area];

        let mut representatives: Vec<ClusterRep> = Vec::new();
        let mut merged = 0usize;
        let mut oriented_tiles: Vec<Vec<[u8; 4]>> = Vec::with_capacity(orientation_maps.len());

        let mut coords: Vec<(u32, u32, f32)> = Vec::with_capacity((tiles_x * tiles_y) as usize);
        let center_x = tiles_x as f32 / 2.0;
        let center_y = tiles_y as f32 / 2.0;
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let cx = tx as f32 + 0.5;
                let cy = ty as f32 + 0.5;
                let dist2 = (cx - center_x).powi(2) + (cy - center_y).powi(2);
                coords.push((tx, ty, dist2));
            }
        }
        coords.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        for (tx, ty, _) in coords {
            let tile_indices = &mut tile_indices_buf;
            for y in 0..tile_h {
                let offset = ((ty as usize * tile_h + y) * stride) + (tx as usize * tile_w);
                tile_indices[y * tile_w..(y + 1) * tile_w]
                    .copy_from_slice(&indexed[offset..offset + tile_w]);
            }

            Self::expand_indices_to_colors_into(tile_indices, palette, &mut tile_colors_buf);
            if opts.use_blur {
                Self::blur_tile_colors_into(&tile_colors_buf, &mut tile_blur_buf, tile_w, tile_h);
            } else {
                tile_blur_buf.copy_from_slice(&tile_colors_buf);
            }

            oriented_tiles.clear();
            for map in &orientation_maps {
                let oriented = Self::orient_tile_to_rep(&tile_blur_buf, &map.map);
                oriented_tiles.push(oriented);
            }

            let mut matched: Option<(usize, Orientation)> = None;
            for (idx, rep) in representatives.iter().enumerate() {
                let (best_mse, best_orient) =
                    Self::best_orientation_mse_preoriented(&rep.blurred_colors, &oriented_tiles, &orientation_maps);
                if best_mse <= opts.threshold {
                    matched = Some((idx, best_orient));
                    break;
                }
            }

            if let Some((rep_idx, orientation)) = matched {
                let rep = &representatives[rep_idx];
                let map = orientation_maps
                    .iter()
                    .find(|m| m.orientation == orientation)
                    .map(|m| &m.map)
                    .unwrap_or(&orientation_maps[0].map);
                for y in 0..tile_h {
                    let offset =
                        ((ty as usize * tile_h + y) * stride) + (tx as usize * tile_w);
                    Self::write_rep_with_orientation(
                        &mut indexed[offset..offset + tile_w],
                        &rep.indices,
                        tile_w,
                        tile_h,
                        y,
                        map,
                    );
                }
                // add member and update representative by medoid
                if let Some(rep) = representatives.get_mut(rep_idx) {
                    let member = ClusterMember {
                        indices: tile_indices.to_vec(),
                        blurred_colors: tile_blur_buf.to_vec(),
                    };
                    if rep.members.len() < max_members_tracked {
                        rep.members.push(member);
                    } else {
                        let pos = rep.insert_cursor % max_members_tracked;
                        rep.members[pos] = member;
                        rep.insert_cursor = rep.insert_cursor.wrapping_add(1);
                    }

                    if rep.members.len() % medoid_recompute_interval == 0 {
                        Self::recompute_medoid(rep, &orientation_maps);
                    }
                }
                merged += 1;
            } else {
                representatives.push(ClusterRep {
                    indices: tile_indices.to_vec(),
                    blurred_colors: tile_blur_buf.to_vec(),
                    members: vec![ClusterMember {
                        indices: tile_indices.to_vec(),
                        blurred_colors: tile_blur_buf.to_vec(),
                    }],
                    insert_cursor: 1,
                });
            }
        }

        merged
    }

    fn expand_indices_to_colors_into(indices: &[u8], palette: &[BGRA8], out: &mut [[u8; 4]]) {
        for (dst, &idx) in out.iter_mut().zip(indices.iter()) {
            if let Some(color) = palette.get(idx as usize) {
                *dst = [color.r, color.g, color.b, color.a];
            } else {
                *dst = [0, 0, 0, 0];
            }
        }
    }

    fn best_orientation_mse_maps(
        rep_colors: &[[u8; 4]],
        tile_colors: &[[u8; 4]],
        maps: &[OrientationMap],
    ) -> (f32, Orientation) {
        let mut best = f32::MAX;
        let mut best_orientation = Orientation::None;
        for map in maps {
            let mse = Self::tile_mse_rgba_with_map(rep_colors, tile_colors, &map.map);
            if mse < best {
                best = mse;
                best_orientation = map.orientation;
            }
        }
        (best, best_orientation)
    }

    fn tile_mse_rgba_with_map(rep_colors: &[[u8; 4]], tile_colors: &[[u8; 4]], map: &[usize]) -> f32 {
        if rep_colors.len() != tile_colors.len() || rep_colors.is_empty() || map.len() != rep_colors.len() {
            return f32::MAX;
        }
        let mut error = 0.0f64;
        for (dst_idx, &src_idx) in map.iter().enumerate() {
            let rep_px = &rep_colors[src_idx];
            let tile_px = &tile_colors[dst_idx];
            for c in 0..4 {
                let diff = rep_px[c] as f64 - tile_px[c] as f64;
                error += diff * diff;
            }
        }
        (error / (rep_colors.len() as f64 * 4.0)) as f32
    }

    fn best_orientation_mse_preoriented(
        rep_colors: &[[u8; 4]],
        oriented_tiles: &[Vec<[u8; 4]>],
        maps: &[OrientationMap],
    ) -> (f32, Orientation) {
        let mut best = f32::MAX;
        let mut best_orientation = Orientation::None;
        for (orient_buf, map) in oriented_tiles.iter().zip(maps.iter()) {
            if orient_buf.len() != rep_colors.len() {
                continue;
            }
            let mse = Self::tile_mse_rgba_fast(rep_colors, orient_buf, best);
            if mse < best {
                best = mse;
                best_orientation = map.orientation;
            }
        }
        (best, best_orientation)
    }

    fn tile_mse_rgba_fast(rep_colors: &[[u8; 4]], tile_colors: &[[u8; 4]], stop_if_over: f32) -> f32 {
        if rep_colors.len() != tile_colors.len() || rep_colors.is_empty() {
            return f32::MAX;
        }
        let mut error = 0.0f64;
        let stop = (stop_if_over as f64) * (rep_colors.len() as f64 * 4.0);
        for (rep_px, tile_px) in rep_colors.iter().zip(tile_colors.iter()) {
            for c in 0..4 {
                let diff = rep_px[c] as f64 - tile_px[c] as f64;
                error += diff * diff;
            }
            if error > stop {
                return f32::MAX;
            }
        }
        (error / (rep_colors.len() as f64 * 4.0)) as f32
    }

    fn orient_tile_to_rep(tile: &[[u8; 4]], map: &[usize]) -> Vec<[u8; 4]> {
        let mut oriented = vec![[0u8; 4]; tile.len()];
        for (dst_idx, &rep_idx) in map.iter().enumerate() {
            oriented[rep_idx] = tile[dst_idx];
        }
        oriented
    }

    fn blur_tile_colors_into(src: &[[u8; 4]], dst: &mut [[u8; 4]], tile_w: usize, tile_h: usize) {
        let idx = |x: usize, y: usize| y * tile_w + x;
        for y in 0..tile_h {
            for x in 0..tile_w {
                let mut acc = [0u32; 4];
                let mut count = 0u32;
                for dy in y.saturating_sub(1)..=(y + 1).min(tile_h - 1) {
                    for dx in x.saturating_sub(1)..=(x + 1).min(tile_w - 1) {
                        let px = &src[idx(dx, dy)];
                        for c in 0..4 {
                            acc[c] += px[c] as u32;
                        }
                        count += 1;
                    }
                }
                let dst_px = &mut dst[idx(x, y)];
                for c in 0..4 {
                    dst_px[c] = (acc[c] / count) as u8;
                }
            }
        }
    }

    fn recompute_medoid(rep: &mut ClusterRep, maps: &[OrientationMap]) {
        if rep.members.len() <= 1 {
            return;
        }
        let mut best_sum = f32::MAX;
        let mut best_idx = 0usize;

        for (i, a) in rep.members.iter().enumerate() {
            let mut sum = 0.0f32;
            for (j, b) in rep.members.iter().enumerate() {
                if i == j {
                    continue;
                }
                let (mse, _) = Self::best_orientation_mse_maps(
                    &a.blurred_colors,
                    &b.blurred_colors,
                    maps,
                );
                sum += mse;
            }
            if sum < best_sum {
                best_sum = sum;
                best_idx = i;
            }
        }

        if let Some(best) = rep.members.get(best_idx) {
            rep.indices = best.indices.clone();
            rep.blurred_colors = best.blurred_colors.clone();
        }
    }

    fn write_rep_with_orientation(
        dest_row: &mut [u8],
        rep_indices: &[u8],
        tile_w: usize,
        _tile_h: usize,
        y: usize,
        orientation_map: &[usize],
    ) {
        let row_offset = y * tile_w;
        for x in 0..tile_w {
            let src_idx = orientation_map[row_offset + x];
            dest_row[x] = rep_indices[src_idx];
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Orientation {
    None,
    FlipX,
    FlipY,
    FlipXY,
}

impl Orientation {
    fn available(allow_flip_x: bool, allow_flip_y: bool) -> Vec<Orientation> {
        let mut v = vec![Orientation::None];
        if allow_flip_x {
            v.push(Orientation::FlipX);
        }
        if allow_flip_y {
            v.push(Orientation::FlipY);
        }
        if allow_flip_x && allow_flip_y {
            v.push(Orientation::FlipXY);
        }
        v
    }

    fn maps(tile_w: usize, tile_h: usize, allow_flip_x: bool, allow_flip_y: bool) -> Vec<OrientationMap> {
        let orientations = Orientation::available(allow_flip_x, allow_flip_y);
        orientations
            .into_iter()
            .map(|orientation| {
                let mut map = Vec::with_capacity(tile_w * tile_h);
                for y in 0..tile_h {
                    for x in 0..tile_w {
                        let idx = match orientation {
                            Orientation::None => y * tile_w + x,
                            Orientation::FlipX => y * tile_w + (tile_w - 1 - x),
                            Orientation::FlipY => (tile_h - 1 - y) * tile_w + x,
                            Orientation::FlipXY => (tile_h - 1 - y) * tile_w + (tile_w - 1 - x),
                        };
                        map.push(idx);
                    }
                }
                OrientationMap { orientation, map }
            })
            .collect()
    }
}

struct OrientationMap {
    orientation: Orientation,
    map: Vec<usize>,
}
