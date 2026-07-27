use crate::types::color_correction::ColorCorrection;
use image::{ImageBuffer, Rgba, RgbaImage};

pub struct ColorProcessor;

impl ColorProcessor {
    pub fn apply_pixels_correction(
        pixels: &[u8],
        width: u32,
        height: u32,
        corrections: &ColorCorrection,
    ) -> RgbaImage {
        let tone_curve = Self::tone_curve(corrections);
        let mut output: RgbaImage = ImageBuffer::new(width, height);

        // chunks_exact and pixels_mut walk the buffers in the same row-major
        // order, so no per-pixel coordinate arithmetic is needed.
        for (source, target) in pixels.chunks_exact(4).zip(output.pixels_mut()) {
            *target = Self::apply_pixel_corrections(
                &Rgba([source[0], source[1], source[2], source[3]]),
                corrections,
                &tone_curve,
            );
        }

        output
    }

    /// Gamma, brightness and contrast are applied per channel and depend only on
    /// the input byte, so they collapse into a single 256 entry table. The table
    /// is bit-exact with the per-pixel computation because the input is always
    /// `i as f32 / 255.0`.
    fn tone_curve(corrections: &ColorCorrection) -> [f32; 256] {
        std::array::from_fn(|i| {
            let value = Self::apply_gamma(i as f32 / 255.0, corrections.gamma);
            Self::apply_contrast(value + corrections.brightness, corrections.contrast)
        })
    }

    fn apply_pixel_corrections(
        pixel: &Rgba<u8>,
        corrections: &ColorCorrection,
        tone_curve: &[f32; 256],
    ) -> Rgba<u8> {
        let [r, g, b, a] = pixel.0;

        // Gamma, brightness and contrast in one table lookup per channel
        let rf = tone_curve[r as usize];
        let gf = tone_curve[g as usize];
        let bf = tone_curve[b as usize];

        // Convert to HSV for saturation and hue adjustments
        let (mut h, mut s, v) = Self::rgb_to_hsv(rf, gf, bf);

        // Apply saturation
        s *= corrections.saturation;
        s = s.clamp(0.0, 1.0);

        // Apply hue shift
        h += corrections.hue_shift;
        h = ((h % 360.0) + 360.0) % 360.0; // Normalize to 0-360

        // Convert back to RGB
        let (mut rf, mut gf, mut bf) = Self::hsv_to_rgb(h, s, v);

        // Apply shadows/highlights
        let luminance = Self::rgb_f32_to_luminance(rf, gf, bf);

        if luminance < 0.5 {
            // Apply shadows adjustment to darker areas
            let shadow_factor = 1.0 + corrections.shadows * (1.0 - 2.0 * luminance);
            rf *= shadow_factor;
            gf *= shadow_factor;
            bf *= shadow_factor;
        } else {
            // Apply highlights adjustment to brighter areas
            let highlight_factor = 1.0 + corrections.highlights * (2.0 * luminance - 1.0);
            rf *= highlight_factor;
            gf *= highlight_factor;
            bf *= highlight_factor;
        }

        // Clamp and convert back to u8
        rf = rf.clamp(0.0, 1.0);
        gf = gf.clamp(0.0, 1.0);
        bf = bf.clamp(0.0, 1.0);

        // Round rather than truncate: truncation biased every channel downwards,
        // so even a neutral correction could shift a value by one.
        Rgba([
            (rf * 255.0).round() as u8,
            (gf * 255.0).round() as u8,
            (bf * 255.0).round() as u8,
            a, // Keep original alpha
        ])
    }

    fn apply_gamma(value: f32, gamma: f32) -> f32 {
        if value <= 0.0 {
            0.0
        } else {
            value.powf(1.0 / gamma)
        }
    }

    fn apply_contrast(value: f32, contrast: f32) -> f32 {
        ((value - 0.5) * contrast + 0.5).clamp(0.0, 1.0)
    }

    pub fn rgb_f32_to_luminance(rf: f32, gf: f32, bf: f32) -> f32 {
        0.299 * rf + 0.587 * gf + 0.114 * bf
    }

    pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        let max_val = r.max(g).max(b);
        let min_val = r.min(g).min(b);
        let delta = max_val - min_val;

        let v = max_val;
        let s = if max_val == 0.0 { 0.0 } else { delta / max_val };

        let h = if delta == 0.0 {
            0.0
        } else if max_val == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max_val == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };

        let h = if h < 0.0 { h + 360.0 } else { h };

        (h, s, v)
    }

    pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r_prime, g_prime, b_prime) = match h {
            h if h < 60.0 => (c, x, 0.0),
            h if h < 120.0 => (x, c, 0.0),
            h if h < 180.0 => (0.0, c, x),
            h if h < 240.0 => (0.0, x, c),
            h if h < 300.0 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        (r_prime + m, g_prime + m, b_prime + m)
    }
}

// Utility functions for UI display
pub fn gamma_to_display_value(gamma: f32) -> f32 {
    // Convert gamma (0.1-3.0) to a more intuitive display value (-100 to +100)
    if gamma < 1.0 {
        (gamma - 1.0) * 100.0 / 0.9 // -100 to 0
    } else {
        (gamma - 1.0) * 100.0 / 2.0 // 0 to +100
    }
}

pub fn display_value_to_gamma(display: f32) -> f32 {
    // Convert display value (-100 to +100) back to gamma (0.1-3.0)
    if display < 0.0 {
        1.0 + display * 0.9 / 100.0
    } else {
        1.0 + display * 2.0 / 100.0
    }
}

pub fn format_percentage(value: f32) -> String {
    format!("{:+.0}%", value * 100.0)
}

pub fn format_gamma(gamma: f32) -> String {
    format!("{gamma:.2}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presets() -> [ColorCorrection; 5] {
        [
            ColorCorrection::default(),
            ColorCorrection::preset_vibrant(),
            ColorCorrection::preset_retro_warm(),
            ColorCorrection::preset_retro_cool(),
            ColorCorrection::preset_dark(),
        ]
    }

    /// The lookup table replaced three per-pixel operations; it has to produce
    /// bit-identical results, not merely close ones.
    #[test]
    fn tone_curve_is_bit_exact_with_the_per_pixel_computation() {
        for corrections in presets() {
            let curve = ColorProcessor::tone_curve(&corrections);
            for (value, entry) in curve.iter().enumerate() {
                let mut expected = value as f32 / 255.0;
                expected = ColorProcessor::apply_gamma(expected, corrections.gamma);
                expected += corrections.brightness;
                expected = ColorProcessor::apply_contrast(expected, corrections.contrast);

                assert_eq!(
                    entry.to_bits(),
                    expected.to_bits(),
                    "gamma {} value {value}",
                    corrections.gamma
                );
            }
        }
    }

    /// chunks_exact/pixels_mut must visit the buffers in the same order as the
    /// coordinate arithmetic it replaced.
    #[test]
    fn corrected_pixels_land_at_the_right_coordinates() {
        let (width, height) = (3u32, 2u32);
        let pixels: Vec<u8> = (0..(width * height * 4) as u8).map(|i| i * 7).collect();
        let corrections = ColorCorrection::preset_vibrant();
        let curve = ColorProcessor::tone_curve(&corrections);

        let output = ColorProcessor::apply_pixels_correction(&pixels, width, height, &corrections);

        for y in 0..height {
            for x in 0..width {
                let i = ((y * width + x) * 4) as usize;
                let expected = ColorProcessor::apply_pixel_corrections(
                    &Rgba([pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]),
                    &corrections,
                    &curve,
                );
                assert_eq!(*output.get_pixel(x, y), expected, "at ({x}, {y})");
            }
        }
    }

    #[test]
    fn alpha_is_preserved() {
        let pixels = vec![10, 20, 30, 40, 50, 60, 70, 200];
        let output =
            ColorProcessor::apply_pixels_correction(&pixels, 2, 1, &ColorCorrection::preset_dark());

        assert_eq!(output.get_pixel(0, 0).0[3], 40);
        assert_eq!(output.get_pixel(1, 0).0[3], 200);
    }

    #[test]
    fn hsv_round_trip_is_stable() {
        for (r, g, b) in [(0.0, 0.0, 0.0), (1.0, 1.0, 1.0), (0.2, 0.6, 0.9)] {
            let (h, s, v) = ColorProcessor::rgb_to_hsv(r, g, b);
            let (r2, g2, b2) = ColorProcessor::hsv_to_rgb(h, s, v);
            assert!((r - r2).abs() < 1e-5, "{r} vs {r2}");
            assert!((g - g2).abs() < 1e-5, "{g} vs {g2}");
            assert!((b - b2).abs() < 1e-5, "{b} vs {b2}");
        }
    }

    #[test]
    fn gamma_display_conversion_round_trips() {
        for gamma in [0.1, 0.5, 1.0, 2.0, 3.0] {
            let display = gamma_to_display_value(gamma);
            assert!((display_value_to_gamma(display) - gamma).abs() < 1e-4);
        }
    }
}
