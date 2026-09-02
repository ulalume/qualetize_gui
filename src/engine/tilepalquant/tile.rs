//! Splitting the image into tiles, the unit a palette is chosen for.
//!
//! A tile holds its distinct colors with the number of pixels on each, which
//! is what the palette search measures against, plus the pixels themselves,
//! which the dithered search and the iteration need.

use super::color::Rgb;

/// One pixel of a tile, carrying the index of the tile it came from so it can
/// be handed around on its own.
#[derive(Clone, Copy, Debug)]
pub struct Pixel {
    pub tile_id: usize,
    pub color: Rgb,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Tile {
    /// The distinct colors of the tile, in the order first seen.
    pub colors: Vec<Rgb>,
    /// How many pixels carry each of `colors`.
    pub counts: Vec<i32>,
    pub pixels: Vec<Pixel>,
}

/// Which pixels index 0 claims, and which the tiles therefore leave out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transparency {
    /// Index 0 is an ordinary color; every pixel takes part.
    None,
    /// Pixels with alpha below 255 are transparent.
    FromAlpha,
    /// Pixels whose RGB equals this color are transparent.
    FromColor(Rgb),
}

/// An RGBA8 image, as the engine reads it.
pub struct SourceImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl SourceImage {
    pub fn new(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
        }
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        4 * (x as usize + self.width as usize * y as usize)
    }

    pub fn color(&self, x: u32, y: u32) -> Rgb {
        let index = self.offset(x, y);
        Rgb::new(
            f64::from(self.data[index]),
            f64::from(self.data[index + 1]),
            f64::from(self.data[index + 2]),
        )
    }

    pub fn alpha(&self, x: u32, y: u32) -> u8 {
        self.data[self.offset(x, y) + 3]
    }

    fn is_transparent(&self, x: u32, y: u32, transparency: Transparency) -> bool {
        match transparency {
            Transparency::None => false,
            Transparency::FromAlpha => self.alpha(x, y) < 255,
            Transparency::FromColor(color) => self.color(x, y) == color,
        }
    }
}

/// The tile at `start_x`, `start_y`, clamped to the image at the right and
/// bottom edges.
pub fn extract_tile(
    image: &SourceImage,
    start_x: u32,
    start_y: u32,
    tile_width: u32,
    tile_height: u32,
    transparency: Transparency,
    tile_id: usize,
) -> Tile {
    let mut tile = Tile::default();
    let end_x = (start_x + tile_width).min(image.width);
    let end_y = (start_y + tile_height).min(image.height);
    for y in start_y..end_y {
        for x in start_x..end_x {
            if image.is_transparent(x, y, transparency) {
                continue;
            }
            let color = image.color(x, y);
            tile.pixels.push(Pixel {
                tile_id,
                color,
                x,
                y,
            });
            match tile.colors.iter().position(|&known| known == color) {
                Some(index) => tile.counts[index] += 1,
                None => {
                    tile.colors.push(color);
                    tile.counts.push(1);
                }
            }
        }
    }
    tile
}

/// Every tile of the image that has at least one pixel left. A tile whose
/// pixels are all transparent is dropped, and does not take up a tile id.
pub fn extract_tiles(
    image: &SourceImage,
    tile_width: u32,
    tile_height: u32,
    transparency: Transparency,
) -> Vec<Tile> {
    let mut tiles = Vec::new();
    for start_y in (0..image.height).step_by(tile_height as usize) {
        for start_x in (0..image.width).step_by(tile_width as usize) {
            let tile = extract_tile(
                image,
                start_x,
                start_y,
                tile_width,
                tile_height,
                transparency,
                tiles.len(),
            );
            if tile.colors.is_empty() {
                continue;
            }
            tiles.push(tile);
        }
    }
    tiles
}

/// The pixels of every tile, back to back.
pub fn extract_all_pixels(tiles: &[Tile]) -> Vec<Pixel> {
    tiles
        .iter()
        .flat_map(|tile| tile.pixels.iter())
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4x2 image: two red pixels, one green, one blue, then four
    /// half-transparent magenta ones.
    fn image() -> SourceImage {
        let mut data = Vec::new();
        for rgba in [
            [255, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
        ] {
            data.extend_from_slice(&rgba);
        }
        for _ in 0..4 {
            data.extend_from_slice(&[255, 0, 255, 128]);
        }
        SourceImage::new(data, 4, 2)
    }

    #[test]
    fn a_tile_counts_repeated_colors_once() {
        let tile = extract_tile(&image(), 0, 0, 4, 1, Transparency::None, 0);
        assert_eq!(tile.colors.len(), 3);
        assert_eq!(tile.counts, [2, 1, 1]);
        assert_eq!(tile.pixels.len(), 4);
        assert_eq!(tile.colors[0], Rgb::new(255.0, 0.0, 0.0));
    }

    #[test]
    fn transparency_from_alpha_leaves_those_pixels_out() {
        let tiles = extract_tiles(&image(), 4, 1, Transparency::FromAlpha);
        assert_eq!(tiles.len(), 1, "the fully transparent row is dropped");
        assert_eq!(tiles[0].pixels.len(), 4);
        assert!(tiles[0].pixels.iter().all(|pixel| pixel.y == 0));
    }

    #[test]
    fn transparency_from_color_matches_on_rgb_alone() {
        let magenta = Transparency::FromColor(Rgb::new(255.0, 0.0, 255.0));
        let tiles = extract_tiles(&image(), 4, 1, magenta);
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].pixels.len(), 4);
    }

    #[test]
    fn an_opaque_image_keeps_every_pixel() {
        let tiles = extract_tiles(&image(), 4, 1, Transparency::None);
        assert_eq!(tiles.len(), 2);
        assert_eq!(extract_all_pixels(&tiles).len(), 8);
    }

    #[test]
    fn dropped_tiles_do_not_take_up_a_tile_id() {
        // The first row is transparent, so the second row's tile is tile 0.
        let mut data = vec![0u8; 4 * 4 * 2];
        for x in 0..4 {
            data[(4 + x) * 4 + 3] = 255;
        }
        let image = SourceImage::new(data, 4, 2);
        let tiles = extract_tiles(&image, 4, 1, Transparency::FromAlpha);
        assert_eq!(tiles.len(), 1);
        assert!(tiles[0].pixels.iter().all(|pixel| pixel.tile_id == 0));
        assert_eq!(extract_all_pixels(&tiles)[0].y, 1);
    }

    #[test]
    fn a_tile_at_the_edge_is_clamped_to_the_image() {
        let tile = extract_tile(&image(), 2, 0, 8, 8, Transparency::None, 0);
        assert_eq!(tile.pixels.len(), 4);
        assert!(tile.pixels.iter().all(|pixel| pixel.x >= 2));
    }

    #[test]
    fn pixels_carry_their_position_for_the_dither_pattern() {
        let tiles = extract_tiles(&image(), 2, 2, Transparency::None);
        let pixels = extract_all_pixels(&tiles);
        assert_eq!(pixels.len(), 8);
        assert_eq!((pixels[0].x, pixels[0].y), (0, 0));
        assert_eq!((pixels[1].x, pixels[1].y), (1, 0));
        assert_eq!((pixels[2].x, pixels[2].y), (0, 1));
    }
}
