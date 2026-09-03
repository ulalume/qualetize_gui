//! Completed outputs kept alongside the settings that produced them.
//!
//! An entry holds the full resolution indexed image (deflate compressed) so
//! it can be exported without re-running the pipeline, a thumbnail for the
//! list, and the [`SettingsBundle`] the image came from.

use crate::settings_manager::SettingsBundle;
use crate::time::Instant;
use crate::types::BGRA8;
use crate::types::image::ImageDataIndexed;

/// Maximum number of entries kept; the oldest are dropped past this.
pub const CAP: usize = 50;

/// Longest side of a thumbnail, in pixels.
pub const THUMBNAIL_SIZE: u32 = 160;

/// Deflate level used for the indexed pixels: a compromise between the
/// time spent on the UI thread and the memory an entry occupies.
const COMPRESSION_LEVEL: u8 = 6;

/// A downscaled RGBA copy of a result, for the list.
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One completed output.
pub struct StoredResult {
    /// Of the indexed pixels and the palettes, for duplicate detection.
    pub hash: u64,
    /// The settings the image was produced with.
    pub settings: SettingsBundle,
    pub width: u32,
    pub height: u32,
    pub colors_per_palette: usize,
    pub palettes: Vec<BGRA8>,
    /// Indexed pixels, deflate compressed; [`StoredResult::decode`] restores them.
    pixels: Vec<u8>,
    pub thumbnail: Thumbnail,
    pub created: Instant,
}

impl StoredResult {
    /// Rebuild the full resolution indexed image.
    pub fn decode(&self) -> Result<ImageDataIndexed, String> {
        let pixels = miniz_oxide::inflate::decompress_to_vec(&self.pixels)
            .map_err(|e| format!("Failed to decompress result pixels: {e:?}"))?;
        let expected = self.width as usize * self.height as usize;
        if pixels.len() != expected {
            return Err(format!(
                "Result pixels are {} bytes, expected {expected}",
                pixels.len()
            ));
        }
        Ok(ImageDataIndexed::new(
            self.palettes.clone(),
            self.colors_per_palette,
            pixels,
        ))
    }

    /// Size of the compressed pixels in bytes.
    pub fn compressed_len(&self) -> usize {
        self.pixels.len()
    }
}

/// The list as it was drawn last frame, and the animation the current order
/// started.
#[derive(Default)]
pub struct ResultsView {
    /// Hashes in the order the entries were drawn in.
    pub last_order: Vec<u64>,
    pub animation: Option<ResultsAnimation>,
}

/// The order change being animated, and when it started.
pub struct ResultsAnimation {
    pub kind: ChangeKind,
    pub started: Instant,
}

/// A change of the list order that the panel animates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// A result that was not in the list is now at its top.
    Added { hash: u64 },
    /// The result drawn at `old_index` is now at the top.
    Moved { hash: u64, old_index: usize },
}

impl ChangeKind {
    /// The result that reached the top.
    pub fn hash(self) -> u64 {
        match self {
            Self::Added { hash } | Self::Moved { hash, old_index: _ } => hash,
        }
    }
}

/// How the order `now` differs from the order `last`, for the two changes the
/// panel animates. Anything else -- a removal, an emptied list, several
/// changes at once -- is [`None`].
pub fn detect_change(last: &[u64], now: &[u64]) -> Option<ChangeKind> {
    let (&hash, rest) = now.split_first()?;
    match last.iter().position(|&other| other == hash) {
        None if rest == last => Some(ChangeKind::Added { hash }),
        Some(old_index) if old_index > 0 && skipping(last, old_index).eq(rest.iter()) => {
            Some(ChangeKind::Moved { hash, old_index })
        }
        _ => None,
    }
}

/// `order` without the hash at `index`.
fn skipping(order: &[u64], index: usize) -> impl Iterator<Item = &u64> {
    order
        .iter()
        .enumerate()
        .filter(move |(at, _)| *at != index)
        .map(|(_, hash)| hash)
}

/// The recorded results, newest first.
pub struct Results {
    entries: Vec<StoredResult>,
    cap: usize,
}

impl Default for Results {
    fn default() -> Self {
        Self::new(CAP)
    }
}

impl Results {
    pub fn new(cap: usize) -> Self {
        Self {
            entries: Vec::new(),
            cap: cap.max(1),
        }
    }

    /// Record `indexed` as the newest entry.
    ///
    /// An image already in the list (same hash) is moved to the front with
    /// its settings and timestamp refreshed rather than stored twice;
    /// `true` means a new entry was added.
    pub fn record(
        &mut self,
        indexed: &ImageDataIndexed,
        rgba: &[u8],
        width: u32,
        height: u32,
        settings: SettingsBundle,
        now: Instant,
    ) -> bool {
        let hash = hash_result(&indexed.indexed_pixels, &indexed.palettes);

        if let Some(index) = self.entries.iter().position(|entry| entry.hash == hash) {
            let mut entry = self.entries.remove(index);
            entry.settings = settings;
            entry.created = now;
            self.entries.insert(0, entry);
            return false;
        }

        let entry = StoredResult {
            hash,
            settings,
            width,
            height,
            colors_per_palette: indexed.colors_per_palette(),
            palettes: indexed.palettes.clone(),
            pixels: miniz_oxide::deflate::compress_to_vec(
                &indexed.indexed_pixels,
                COMPRESSION_LEVEL,
            ),
            thumbnail: thumbnail_of(rgba, width, height),
            created: now,
        };
        self.entries.insert(0, entry);
        self.entries.truncate(self.cap);
        true
    }

    pub fn entries(&self) -> &[StoredResult] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop the entry at `index`, ignoring an index past the end.
    pub fn remove(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// FNV-1a over the pixels followed by the palette bytes.
fn hash_result(pixels: &[u8], palettes: &[BGRA8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET;
    let mut eat = |byte: u8| {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    };
    for &pixel in pixels {
        eat(pixel);
    }
    for color in palettes {
        eat(color.b);
        eat(color.g);
        eat(color.r);
        eat(color.a);
    }
    hash
}

/// Downscale `rgba` so its longest side is [`THUMBNAIL_SIZE`], averaging the
/// source pixels that fall into each destination cell. An image already that
/// small is kept as it is.
pub fn thumbnail_of(rgba: &[u8], width: u32, height: u32) -> Thumbnail {
    let longest = width.max(height);
    if width == 0 || height == 0 || longest <= THUMBNAIL_SIZE {
        return Thumbnail {
            width,
            height,
            rgba: rgba.to_vec(),
        };
    }

    let scale = THUMBNAIL_SIZE as f64 / longest as f64;
    let dst_width = ((width as f64 * scale).round() as u32).max(1);
    let dst_height = ((height as f64 * scale).round() as u32).max(1);

    let cells = dst_width as usize * dst_height as usize;
    let mut sums = vec![[0u64; 4]; cells];
    let mut counts = vec![0u64; cells];

    for y in 0..height as usize {
        let dst_y = (((y as f64 + 0.5) * scale) as usize).min(dst_height as usize - 1);
        let row = dst_y * dst_width as usize;
        for x in 0..width as usize {
            let dst_x = (((x as f64 + 0.5) * scale) as usize).min(dst_width as usize - 1);
            let src = (y * width as usize + x) * 4;
            let Some(pixel) = rgba.get(src..src + 4) else {
                continue;
            };
            let cell = row + dst_x;
            for (sum, &channel) in sums[cell].iter_mut().zip(pixel) {
                *sum += channel as u64;
            }
            counts[cell] += 1;
        }
    }

    let mut out = Vec::with_capacity(cells * 4);
    for (sum, count) in sums.iter().zip(&counts) {
        let count = (*count).max(1);
        for channel in sum {
            out.push((channel / count) as u8);
        }
    }

    Thumbnail {
        width: dst_width,
        height: dst_height,
        rgba: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::color_correction::ColorCorrection;
    use crate::types::image::PaletteSortSettings;
    use crate::types::qualetize::QualetizeSettings;

    fn settings() -> SettingsBundle {
        SettingsBundle::new(
            QualetizeSettings::default(),
            ColorCorrection::default(),
            PaletteSortSettings::default(),
        )
    }

    fn color(r: u8, g: u8, b: u8) -> BGRA8 {
        BGRA8 { b, g, r, a: 255 }
    }

    /// A 4x4 image whose every pixel is index `index`, over a two color palette.
    fn image(index: u8) -> ImageDataIndexed {
        ImageDataIndexed::new(
            vec![color(0, 0, 0), color(255, 255, 255)],
            2,
            vec![index; 16],
        )
    }

    fn rgba(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
        pixel
            .iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * 4)
            .collect()
    }

    fn record(results: &mut Results, indexed: &ImageDataIndexed) -> bool {
        results.record(
            indexed,
            &rgba(4, 4, [10, 20, 30, 255]),
            4,
            4,
            settings(),
            Instant::now(),
        )
    }

    #[test]
    fn recording_the_same_image_twice_moves_it_to_the_front() {
        let mut results = Results::default();
        assert!(record(&mut results, &image(0)));
        assert!(record(&mut results, &image(1)));
        assert_eq!(results.len(), 2);

        // The older of the two, recorded again, is not stored a second time.
        assert!(!record(&mut results, &image(0)));
        assert_eq!(results.len(), 2);
        assert_eq!(
            results.entries()[0].hash,
            hash_result(&image(0).indexed_pixels, &image(0).palettes)
        );
    }

    #[test]
    fn different_images_grow_the_list() {
        let mut results = Results::default();
        for index in 0..3u8 {
            assert!(record(&mut results, &image(index)));
        }
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn the_cap_drops_the_oldest_entry() {
        let mut results = Results::new(3);
        for index in 0..5u8 {
            record(&mut results, &image(index));
        }
        assert_eq!(results.len(), 3);
        // Newest first: 4, 3, 2. The first two are gone.
        let newest = &results.entries()[0];
        assert_eq!(newest.decode().expect("decodes").indexed_pixels[0], 4);
        let oldest = &results.entries()[2];
        assert_eq!(oldest.decode().expect("decodes").indexed_pixels[0], 2);
    }

    #[test]
    fn decoding_round_trips_the_pixels() {
        let mut results = Results::default();
        let pixels: Vec<u8> = (0..16u8).collect();
        let indexed = ImageDataIndexed::new(vec![color(1, 2, 3); 16], 16, pixels.clone());
        record(&mut results, &indexed);

        let decoded = results.entries()[0].decode().expect("decodes");
        assert_eq!(decoded.indexed_pixels, pixels);
        assert_eq!(decoded.palettes, indexed.palettes);
        assert_eq!(decoded.colors_per_palette(), 16);
    }

    #[test]
    fn removing_and_clearing_shrink_the_list() {
        let mut results = Results::default();
        for index in 0..3u8 {
            record(&mut results, &image(index));
        }
        results.remove(1);
        assert_eq!(results.len(), 2);
        results.remove(99);
        assert_eq!(results.len(), 2);
        results.clear();
        assert!(results.is_empty());
    }

    #[test]
    fn a_thumbnail_keeps_the_aspect_ratio() {
        let thumbnail = thumbnail_of(&rgba(800, 400, [0, 0, 0, 255]), 800, 400);
        assert_eq!((thumbnail.width, thumbnail.height), (THUMBNAIL_SIZE, 80));
        assert_eq!(
            thumbnail.rgba.len(),
            thumbnail.width as usize * thumbnail.height as usize * 4
        );

        let tall = thumbnail_of(&rgba(400, 800, [0, 0, 0, 255]), 400, 800);
        assert_eq!((tall.width, tall.height), (80, THUMBNAIL_SIZE));
    }

    #[test]
    fn a_small_image_is_not_upscaled() {
        let source = rgba(10, 20, [1, 2, 3, 4]);
        let thumbnail = thumbnail_of(&source, 10, 20);
        assert_eq!((thumbnail.width, thumbnail.height), (10, 20));
        assert_eq!(thumbnail.rgba, source);
    }

    #[test]
    fn a_uniform_image_thumbnails_to_that_color() {
        let pixel = [12, 34, 56, 78];
        let thumbnail = thumbnail_of(&rgba(500, 300, pixel), 500, 300);
        assert!(
            thumbnail
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|c| *c == pixel)
        );
    }

    #[test]
    fn an_unchanged_order_is_no_change() {
        assert_eq!(detect_change(&[1, 2, 3], &[1, 2, 3]), None);
        assert_eq!(detect_change(&[], &[]), None);
    }

    #[test]
    fn a_hash_that_is_new_at_the_top_is_an_addition() {
        assert_eq!(
            detect_change(&[1, 2], &[9, 1, 2]),
            Some(ChangeKind::Added { hash: 9 })
        );
        assert_eq!(
            detect_change(&[], &[9]),
            Some(ChangeKind::Added { hash: 9 })
        );
    }

    #[test]
    fn a_hash_taken_from_further_down_is_a_move() {
        assert_eq!(
            detect_change(&[1, 2, 3, 4], &[3, 1, 2, 4]),
            Some(ChangeKind::Moved {
                hash: 3,
                old_index: 2,
            })
        );
        assert_eq!(
            detect_change(&[1, 2], &[2, 1]),
            Some(ChangeKind::Moved {
                hash: 2,
                old_index: 1,
            })
        );
    }

    #[test]
    fn a_removal_or_an_emptied_list_is_no_change() {
        assert_eq!(detect_change(&[1, 2, 3], &[1, 3]), None);
        assert_eq!(detect_change(&[1, 2, 3], &[2, 3]), None);
        assert_eq!(detect_change(&[1, 2, 3], &[]), None);
    }

    #[test]
    fn several_changes_at_once_are_no_change() {
        // A new entry at the top and the oldest one dropped by the cap.
        assert_eq!(detect_change(&[1, 2, 3], &[9, 1, 2]), None);
        // An entry moved to the top and another one gone.
        assert_eq!(detect_change(&[1, 2, 3], &[3, 1]), None);
        // A reorder that is not a move to the top.
        assert_eq!(detect_change(&[1, 2, 3], &[1, 3, 2]), None);
    }

    #[test]
    fn the_hash_differs_when_a_palette_entry_differs() {
        let pixels = vec![0u8; 16];
        let a = ImageDataIndexed::new(
            vec![color(0, 0, 0), color(255, 255, 255)],
            2,
            pixels.clone(),
        );
        let b = ImageDataIndexed::new(vec![color(0, 0, 0), color(255, 255, 254)], 2, pixels);

        assert_ne!(
            hash_result(&a.indexed_pixels, &a.palettes),
            hash_result(&b.indexed_pixels, &b.palettes)
        );

        let mut results = Results::default();
        assert!(record(&mut results, &a));
        assert!(record(&mut results, &b));
        assert_eq!(results.len(), 2);
    }
}
