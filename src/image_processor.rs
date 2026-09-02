use crate::engine::{self, Progress, QuantEngine, QuantizeResult};
use crate::types::tilepalquant::TpqSettings;
use crate::types::{BGRA8, QualetizeSettings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

/// One background computation whose result is polled from the UI thread.
///
/// Starting a job replaces the receivers, so a result from an earlier thread
/// can never be observed: its send fails against the dropped receiver and the
/// thread simply exits. No generation counter or join bookkeeping is needed.
///
/// `P` is what the worker reports while it runs; `()` for workers that
/// report nothing.
struct Job<T, P = ()> {
    receiver: Option<mpsc::Receiver<T>>,
    progress: Option<mpsc::Receiver<P>>,
    cancel: Arc<AtomicBool>,
}

impl<T, P> Default for Job<T, P> {
    fn default() -> Self {
        Self {
            receiver: None,
            progress: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl<T: Send + 'static, P: Send + 'static> Job<T, P> {
    /// Cancel the running job, if any, and run `work` on a new thread.
    /// `work` returns `None` when it stopped early because of the cancel flag.
    fn start(
        &mut self,
        work: impl FnOnce(&AtomicBool, &mpsc::Sender<P>) -> Option<T> + Send + 'static,
    ) {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let (progress_sender, progress_receiver) = mpsc::channel();
        let flag = cancel.clone();
        std::thread::spawn(move || {
            if let Some(result) = work(&flag, &progress_sender) {
                let _ = sender.send(result);
            }
        });
        self.receiver = Some(receiver);
        self.progress = Some(progress_receiver);
        self.cancel = cancel;
    }

    /// Ask the worker to stop and forget about its result.
    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.receiver = None;
        self.progress = None;
    }

    fn is_running(&self) -> bool {
        self.receiver.is_some()
    }

    /// The finished result, once. A worker that exited without one (cancelled
    /// or panicked) just ends the job.
    fn poll(&mut self) -> Option<T> {
        let receiver = self.receiver.as_ref()?;
        match receiver.try_recv() {
            Ok(result) => {
                self.receiver = None;
                self.progress = None;
                Some(result)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver = None;
                self.progress = None;
                None
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    /// The most recent progress report, dropping any older ones queued
    /// behind it.
    fn poll_progress(&mut self) -> Option<P> {
        let receiver = self.progress.as_ref()?;
        let mut latest = None;
        while let Ok(report) = receiver.try_recv() {
            latest = Some(report);
        }
        latest
    }
}

#[derive(Default)]
pub struct ImageProcessor {
    quantize: Job<Result<QuantizeResult, String>, Progress>,
    tile_reduce: Job<TileReduceResult>,
}

pub struct TileReduceOptions {
    pub tile_width: u16,
    pub tile_height: u16,
    pub threshold: f32,
    pub allow_flip_x: bool,
    pub allow_flip_y: bool,
}

pub struct TileReduceResult {
    pub indexed_pixels: Vec<u8>,
    pub merged: usize,
}

impl ImageProcessor {
    pub fn start_quantize(
        &mut self,
        rgba_data: &[u8],
        width: u32,
        height: u32,
        engine: QuantEngine,
        settings: QualetizeSettings,
        tpq: TpqSettings,
    ) {
        let rgba_data = rgba_data.to_vec();
        self.quantize.start(move |cancel, progress| {
            let ctx = engine::RunContext {
                cancel,
                progress: Some(progress),
            };
            engine::run(engine, &rgba_data, width, height, &settings, &tpq, &ctx)
        });
    }

    pub fn poll_quantize(&mut self) -> Option<Result<QuantizeResult, String>> {
        self.quantize.poll()
    }

    pub fn poll_quantize_progress(&mut self) -> Option<Progress> {
        self.quantize.poll_progress()
    }

    pub fn is_quantizing(&self) -> bool {
        self.quantize.is_running()
    }

    pub fn cancel_quantize(&mut self) {
        self.quantize.cancel();
    }

    pub fn start_tile_reduce(
        &mut self,
        indexed: Vec<u8>,
        palettes: Vec<BGRA8>,
        width: u32,
        height: u32,
        opts: TileReduceOptions,
    ) {
        self.tile_reduce.start(move |cancel, _| {
            let mut indexed_pixels = indexed;
            let merged =
                reduce_tiles_indexed(&mut indexed_pixels, &palettes, width, height, &opts, cancel)?;
            Some(TileReduceResult {
                indexed_pixels,
                merged,
            })
        });
    }

    pub fn poll_tile_reduce(&mut self) -> Option<TileReduceResult> {
        self.tile_reduce.poll()
    }

    pub fn is_tile_reducing(&self) -> bool {
        self.tile_reduce.is_running()
    }

    pub fn cancel_tile_reduce(&mut self) {
        self.tile_reduce.cancel();
    }

    pub fn cancel_all(&mut self) {
        self.cancel_quantize();
        self.cancel_tile_reduce();
    }
}

/// One tile as the reducer sees it: its palette indices, and the blurred
/// colors those indices resolve to, which is what tiles are compared on.
#[derive(Clone)]
struct Tile {
    indices: Vec<u8>,
    blurred_colors: Vec<[u8; 4]>,
}

struct Cluster {
    /// The tile written in place of every member.
    rep: Tile,
    /// Recent members, kept so the representative can be re-chosen as the
    /// medoid of what actually joined the cluster.
    members: Vec<Tile>,
    insert_cursor: usize,
}

/// Merge tiles whose blurred colors are within `opts.threshold` MSE of an
/// earlier tile (in any allowed flip), rewriting `indexed` in place.
///
/// Returns the number of tiles replaced, or `None` when `cancel` was raised.
pub fn reduce_tiles_indexed(
    indexed: &mut [u8],
    palette: &[BGRA8],
    width: u32,
    height: u32,
    opts: &TileReduceOptions,
    cancel: &AtomicBool,
) -> Option<usize> {
    // Quality/speed tuning
    const MEDOID_RECOMPUTE_INTERVAL: usize = 8;
    const MAX_MEMBERS_TRACKED: usize = 64;

    if opts.tile_width == 0
        || opts.tile_height == 0
        || !width.is_multiple_of(opts.tile_width as u32)
        || !height.is_multiple_of(opts.tile_height as u32)
    {
        log::warn!("Tile reduce post-process skipped due to incompatible dimensions");
        return Some(0);
    }

    let tiles_x = width / opts.tile_width as u32;
    let tiles_y = height / opts.tile_height as u32;
    let tile_w = opts.tile_width as usize;
    let tile_h = opts.tile_height as usize;
    let tile_area = tile_w * tile_h;
    let stride = width as usize;
    let orientation_maps = Orientation::maps(tile_w, tile_h, opts.allow_flip_x, opts.allow_flip_y);

    let mut tile_indices = vec![0u8; tile_area];
    let mut tile_colors = vec![[0u8; 4]; tile_area];
    let mut tile_blur = vec![[0u8; 4]; tile_area];

    let mut clusters: Vec<Cluster> = Vec::new();
    let mut merged = 0usize;
    let mut oriented_tiles: Vec<Vec<[u8; 4]>> = Vec::with_capacity(orientation_maps.len());

    // Visit tiles from the image center outwards so the representatives that
    // win are the ones in the middle of the picture.
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
    coords.sort_by(|a, b| a.2.total_cmp(&b.2));

    for (tx, ty, _) in coords {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let row_offset = |y: usize| ((ty as usize * tile_h + y) * stride) + (tx as usize * tile_w);
        for y in 0..tile_h {
            let offset = row_offset(y);
            tile_indices[y * tile_w..(y + 1) * tile_w]
                .copy_from_slice(&indexed[offset..offset + tile_w]);
        }

        expand_indices_to_colors_into(&tile_indices, palette, &mut tile_colors);
        blur_tile_colors_into(&tile_colors, &mut tile_blur, tile_w, tile_h);

        oriented_tiles.clear();
        oriented_tiles.extend(
            orientation_maps
                .iter()
                .map(|map| orient_tile_to_rep(&tile_blur, &map.map)),
        );

        let matched = clusters.iter().enumerate().find_map(|(idx, cluster)| {
            let (best_mse, best_orient) = best_orientation_mse_preoriented(
                &cluster.rep.blurred_colors,
                &oriented_tiles,
                &orientation_maps,
            );
            (best_mse <= opts.threshold).then_some((idx, best_orient))
        });

        let tile = Tile {
            indices: tile_indices.clone(),
            blurred_colors: tile_blur.clone(),
        };

        let Some((cluster_idx, orientation)) = matched else {
            clusters.push(Cluster {
                rep: tile.clone(),
                members: vec![tile],
                insert_cursor: 1,
            });
            continue;
        };

        let cluster = &mut clusters[cluster_idx];
        let map = orientation_maps
            .iter()
            .find(|m| m.orientation == orientation)
            .map_or(&orientation_maps[0].map, |m| &m.map);
        for y in 0..tile_h {
            let offset = row_offset(y);
            write_rep_row(
                &mut indexed[offset..offset + tile_w],
                &cluster.rep.indices,
                &map[y * tile_w..(y + 1) * tile_w],
            );
        }

        if cluster.members.len() < MAX_MEMBERS_TRACKED {
            cluster.members.push(tile);
        } else {
            let pos = cluster.insert_cursor % MAX_MEMBERS_TRACKED;
            cluster.members[pos] = tile;
            cluster.insert_cursor = cluster.insert_cursor.wrapping_add(1);
        }
        if cluster
            .members
            .len()
            .is_multiple_of(MEDOID_RECOMPUTE_INTERVAL)
        {
            recompute_medoid(cluster, &orientation_maps);
        }
        merged += 1;
    }

    Some(merged)
}

fn expand_indices_to_colors_into(indices: &[u8], palette: &[BGRA8], out: &mut [[u8; 4]]) {
    for (dst, &idx) in out.iter_mut().zip(indices) {
        *dst = palette
            .get(idx as usize)
            .map_or([0, 0, 0, 0], |c| [c.r, c.g, c.b, c.a]);
    }
}

/// Mean squared error over all channels between `rep` and `tile` read
/// through `map` (`tile[i]` is compared with `rep[map[i]]`).
fn tile_mse_with_map(rep: &[[u8; 4]], tile: &[[u8; 4]], map: &[usize]) -> f32 {
    if rep.len() != tile.len() || rep.is_empty() || map.len() != rep.len() {
        return f32::MAX;
    }
    let error: u64 = map
        .iter()
        .zip(tile)
        .map(|(&src_idx, tile_px)| squared_diff(&rep[src_idx], tile_px))
        .sum();
    error as f32 / (rep.len() * 4) as f32
}

/// Like [`tile_mse_with_map`] on already oriented tiles, giving up as soon as
/// the error exceeds `stop_if_over` since the caller only wants the minimum.
fn tile_mse_fast(rep: &[[u8; 4]], tile: &[[u8; 4]], stop_if_over: f32) -> f32 {
    if rep.len() != tile.len() || rep.is_empty() {
        return f32::MAX;
    }
    let samples = (rep.len() * 4) as f32;
    let stop = (stop_if_over * samples) as u64;
    let mut error = 0u64;
    for (rep_px, tile_px) in rep.iter().zip(tile) {
        error += squared_diff(rep_px, tile_px);
        if error > stop {
            return f32::MAX;
        }
    }
    error as f32 / samples
}

fn squared_diff(a: &[u8; 4], b: &[u8; 4]) -> u64 {
    a.iter()
        .zip(b)
        .map(|(&x, &y)| {
            let d = x as i32 - y as i32;
            (d * d) as u64
        })
        .sum()
}

fn best_orientation_mse_maps(
    rep: &[[u8; 4]],
    tile: &[[u8; 4]],
    maps: &[OrientationMap],
) -> (f32, Orientation) {
    maps.iter()
        .map(|map| (tile_mse_with_map(rep, tile, &map.map), map.orientation))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap_or((f32::MAX, Orientation::None))
}

fn best_orientation_mse_preoriented(
    rep: &[[u8; 4]],
    oriented_tiles: &[Vec<[u8; 4]>],
    maps: &[OrientationMap],
) -> (f32, Orientation) {
    let mut best = f32::MAX;
    let mut best_orientation = Orientation::None;
    for (oriented, map) in oriented_tiles.iter().zip(maps) {
        let mse = tile_mse_fast(rep, oriented, best);
        if mse < best {
            best = mse;
            best_orientation = map.orientation;
        }
    }
    (best, best_orientation)
}

fn orient_tile_to_rep(tile: &[[u8; 4]], map: &[usize]) -> Vec<[u8; 4]> {
    let mut oriented = vec![[0u8; 4]; tile.len()];
    for (dst_idx, &rep_idx) in map.iter().enumerate() {
        oriented[rep_idx] = tile[dst_idx];
    }
    oriented
}

/// 3x3 box blur, clamped at the tile edges.
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

/// Make the member closest to all the others the cluster's representative.
fn recompute_medoid(cluster: &mut Cluster, maps: &[OrientationMap]) {
    if cluster.members.len() <= 1 {
        return;
    }
    let medoid = cluster
        .members
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let sum: f32 = cluster
                .members
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, b)| {
                    best_orientation_mse_maps(&a.blurred_colors, &b.blurred_colors, maps).0
                })
                .sum();
            (sum, i)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i);
    if let Some(i) = medoid {
        cluster.rep = cluster.members[i].clone();
    }
}

/// Write one row of the representative into `dest_row`, `map_row[x]` naming
/// the representative pixel that lands at column `x`.
fn write_rep_row(dest_row: &mut [u8], rep_indices: &[u8], map_row: &[usize]) {
    for (dst, &src_idx) in dest_row.iter_mut().zip(map_row) {
        *dst = rep_indices[src_idx];
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

    /// For every allowed orientation, the source pixel index for each
    /// destination pixel of a `tile_w` x `tile_h` tile.
    fn maps(
        tile_w: usize,
        tile_h: usize,
        allow_flip_x: bool,
        allow_flip_y: bool,
    ) -> Vec<OrientationMap> {
        Orientation::available(allow_flip_x, allow_flip_y)
            .into_iter()
            .map(|orientation| {
                let mut map = Vec::with_capacity(tile_w * tile_h);
                for y in 0..tile_h {
                    for x in 0..tile_w {
                        let (sx, sy) = match orientation {
                            Orientation::None => (x, y),
                            Orientation::FlipX => (tile_w - 1 - x, y),
                            Orientation::FlipY => (x, tile_h - 1 - y),
                            Orientation::FlipXY => (tile_w - 1 - x, tile_h - 1 - y),
                        };
                        map.push(sy * tile_w + sx);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiles are 4x1 so the 3x3 blur (which averages a whole 2x2 tile into
    /// one color) still leaves a mirrored tile distinguishable from the original.
    fn opts(threshold: f32, flip_x: bool, flip_y: bool) -> TileReduceOptions {
        TileReduceOptions {
            tile_width: 4,
            tile_height: 1,
            threshold,
            allow_flip_x: flip_x,
            allow_flip_y: flip_y,
        }
    }

    fn gray_palette() -> Vec<BGRA8> {
        (0..8u8)
            .map(|i| BGRA8 {
                b: i * 30,
                g: i * 30,
                r: i * 30,
                a: 255,
            })
            .collect()
    }

    /// 8x1 image, 4x1 tiles: the right tile is the horizontal mirror of the left one.
    #[rustfmt::skip]
    fn mirrored_pair() -> Vec<u8> {
        vec![0, 0, 7, 7,   7, 7, 0, 0]
    }

    fn reduce(pixels: &mut [u8], opts: &TileReduceOptions, cancelled: bool) -> Option<usize> {
        reduce_tiles_indexed(
            pixels,
            &gray_palette(),
            8,
            1,
            opts,
            &AtomicBool::new(cancelled),
        )
    }

    #[test]
    fn identical_tiles_are_merged_with_a_zero_threshold() {
        #[rustfmt::skip]
        let mut pixels = vec![1, 2, 3, 4,   1, 2, 3, 4];
        assert_eq!(
            reduce(&mut pixels, &opts(0.0, false, false), false),
            Some(1)
        );
    }

    #[test]
    fn a_mirrored_tile_is_only_merged_when_the_flip_is_allowed() {
        let mut pixels = mirrored_pair();
        assert_eq!(
            reduce(&mut pixels, &opts(0.0, false, false), false),
            Some(0)
        );
        assert_eq!(pixels, mirrored_pair(), "nothing rewritten");

        let mut pixels = mirrored_pair();
        assert_eq!(reduce(&mut pixels, &opts(0.0, true, false), false), Some(1));
        // The merged tile is written as the flipped representative, so the
        // image resolves to the same colors it had before.
        assert_eq!(pixels, mirrored_pair());
    }

    #[test]
    fn a_raised_cancel_flag_stops_the_reduction() {
        let mut pixels = mirrored_pair();
        assert_eq!(reduce(&mut pixels, &opts(0.0, true, true), true), None);
    }

    #[test]
    fn indivisible_dimensions_are_skipped() {
        let mut pixels = vec![0; 6];
        let merged = reduce_tiles_indexed(
            &mut pixels,
            &gray_palette(),
            6,
            1,
            &opts(0.0, false, false),
            &AtomicBool::new(false),
        );
        assert_eq!(merged, Some(0));
    }

    #[test]
    fn mse_helpers_agree_and_flipped_maps_line_up() {
        let maps = Orientation::maps(2, 2, true, true);
        let rep = vec![
            [10, 0, 0, 255],
            [20, 0, 0, 255],
            [30, 0, 0, 255],
            [40, 0, 0, 255],
        ];
        // rep flipped horizontally
        let tile = vec![
            [20, 0, 0, 255],
            [10, 0, 0, 255],
            [40, 0, 0, 255],
            [30, 0, 0, 255],
        ];

        let (mse, orientation) = best_orientation_mse_maps(&rep, &tile, &maps);
        assert_eq!(mse, 0.0);
        assert!(orientation == Orientation::FlipX);

        let oriented: Vec<_> = maps
            .iter()
            .map(|m| orient_tile_to_rep(&tile, &m.map))
            .collect();
        let (mse, orientation) = best_orientation_mse_preoriented(&rep, &oriented, &maps);
        assert_eq!(mse, 0.0);
        assert!(orientation == Orientation::FlipX);

        // Identity orientation: every pixel differs by 10 in one channel.
        assert_eq!(tile_mse_with_map(&rep, &tile, &maps[0].map), 100.0 / 4.0);
        assert_eq!(tile_mse_fast(&rep, &tile, f32::MAX), 100.0 / 4.0);
    }
}
