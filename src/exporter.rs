use crate::types::BGRA8;

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
    let file =
        File::create(output_path).map_err(|e| format!("Failed to create output file: {e}"))?;
    let w = BufWriter::new(file);

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
        .map_err(|e| format!("Failed to write PNG header: {e}"))?;

    writer
        .write_image_data(indexed_pixel_data)
        .map_err(|e| format!("Failed to write PNG image data: {e}"))?;

    Ok(())
}
pub fn save_indexed_bmp(
    output_path: &str,
    indexed_pixel_data: &[u8],
    palette_data: &[BGRA8],
    width: u32,
    height: u32,
) -> Result<(), String> {
    if indexed_pixel_data.len() != (width * height) as usize {
        return Err(format!(
            "indexed data has {} pixels, expected {}x{}",
            indexed_pixel_data.len(),
            width,
            height
        ));
    }
    // Create 8-bit indexed BMP with palette (always 256 entries)
    let palette_size = palette_data.len().min(256); // Max 256 colors for 8-bit
    let row_size = width.div_ceil(4) * 4; // 4-byte aligned for 8-bit data
    let image_size = row_size * height;
    let palette_bytes = 256 * 4; // Always 256 palette entries * 4 bytes each (BGRA)
    let data_offset = 54 + palette_bytes; // Header + palette
    let file_size = data_offset + image_size;

    let mut bmp_data = Vec::with_capacity(file_size as usize);

    // BMP File Header (14 bytes)
    bmp_data.extend_from_slice(b"BM"); // Signature
    bmp_data.extend_from_slice(&file_size.to_le_bytes()); // File size
    bmp_data.extend_from_slice(&[0, 0, 0, 0]); // Reserved
    bmp_data.extend_from_slice(&data_offset.to_le_bytes()); // Data offset

    // BMP Info Header (40 bytes)
    bmp_data.extend_from_slice(&40u32.to_le_bytes()); // Header size
    bmp_data.extend_from_slice(&(width as i32).to_le_bytes()); // Width
    bmp_data.extend_from_slice(&(height as i32).to_le_bytes()); // Height
    bmp_data.extend_from_slice(&1u16.to_le_bytes()); // Planes
    bmp_data.extend_from_slice(&8u16.to_le_bytes()); // Bits per pixel (8-bit indexed)
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Compression
    bmp_data.extend_from_slice(&image_size.to_le_bytes()); // Image size
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // X pixels per meter
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Y pixels per meter
    bmp_data.extend_from_slice(&256u32.to_le_bytes()); // Colors used (always 256 for 8-bit)
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Important colors

    // Color palette (BGRA format, 4 bytes per color)
    for color in palette_data.iter().take(palette_size) {
        bmp_data.push(color.b); // Blue
        bmp_data.push(color.g); // Green
        bmp_data.push(color.r); // Red
        bmp_data.push(color.a); // Alpha (reserved in BMP, usually 0)
    }

    // Fill remaining palette entries if less than 256
    for _ in palette_size..256 {
        bmp_data.extend_from_slice(&[0, 0, 0, 0]);
    }

    // Image data (bottom-up, 8-bit indexed), each row padded to 4 bytes
    let padding = (row_size - width) as usize;
    for row in indexed_pixel_data.chunks_exact(width as usize).rev() {
        bmp_data.extend_from_slice(row);
        bmp_data.extend(std::iter::repeat_n(0, padding));
    }

    std::fs::write(output_path, bmp_data).map_err(|e| format!("File write error: {e}"))?;

    Ok(())
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

    let format = match export_format {
        crate::types::ExportFormat::Png => image::ImageFormat::Png,
        crate::types::ExportFormat::Bmp => image::ImageFormat::Bmp,
        crate::types::ExportFormat::PngIndexed => {
            return Err("indexed PNG needs palette data, use save_indexed_png".to_string());
        }
    };
    dynamic_img
        .save_with_format(output_path, format)
        .map_err(|e| format!("{format:?} save error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(v: u8) -> BGRA8 {
        BGRA8 {
            b: v,
            g: v,
            r: v,
            a: 255,
        }
    }

    /// Rows are stored bottom-up and padded to a multiple of four bytes, which
    /// is where a hand-rolled BMP writer usually goes wrong.
    #[test]
    fn bmp_rows_are_bottom_up_and_padded() {
        let dir = std::env::temp_dir().join(format!("qualetize_gui_bmp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.bmp");

        // 3x2 image: top row 1,2,3 / bottom row 4,5,6
        let pixels = [1, 2, 3, 4, 5, 6];
        save_indexed_bmp(path.to_str().unwrap(), &pixels, &[gray(0), gray(10)], 3, 2).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let data_offset = 54 + 256 * 4;
        assert_eq!(bytes.len(), data_offset + 2 * 4);
        assert_eq!(&bytes[data_offset..data_offset + 4], &[4, 5, 6, 0]);
        assert_eq!(&bytes[data_offset + 4..], &[1, 2, 3, 0]);
        // second palette entry, BGRA
        assert_eq!(&bytes[54 + 4..54 + 8], &[10, 10, 10, 255]);

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bmp_rejects_a_pixel_buffer_of_the_wrong_size() {
        assert!(save_indexed_bmp("unused.bmp", &[0; 5], &[gray(0)], 3, 2).is_err());
    }
}
