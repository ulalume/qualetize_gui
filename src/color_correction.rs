use crate::types::ColorCorrection;
use image::{ImageBuffer, Rgba, RgbaImage};

pub struct ColorProcessor;

impl ColorProcessor {
    pub fn apply_corrections(image: &RgbaImage, corrections: &ColorCorrection) -> RgbaImage {
        let (width, height) = image.dimensions();
        let mut output = ImageBuffer::new(width, height);

        for (x, y, pixel) in image.enumerate_pixels() {
            let corrected = Self::apply_pixel_corrections(pixel, corrections);
            output.put_pixel(x, y, corrected);
        }

        output
    }

    fn apply_pixel_corrections(pixel: &Rgba<u8>, corrections: &ColorCorrection) -> Rgba<u8> {
        let [r, g, b, a] = pixel.0;

        // Convert to float 0.0-1.0 range
        let mut rf = r as f32 / 255.0;
        let mut gf = g as f32 / 255.0;
        let mut bf = b as f32 / 255.0;

        // Apply gamma correction first
        rf = Self::apply_gamma(rf, corrections.gamma);
        gf = Self::apply_gamma(gf, corrections.gamma);
        bf = Self::apply_gamma(bf, corrections.gamma);

        // Apply brightness
        rf += corrections.brightness;
        gf += corrections.brightness;
        bf += corrections.brightness;

        // Apply contrast
        rf = Self::apply_contrast(rf, corrections.contrast);
        gf = Self::apply_contrast(gf, corrections.contrast);
        bf = Self::apply_contrast(bf, corrections.contrast);

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
        let luminance = 0.299 * rf + 0.587 * gf + 0.114 * bf;

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

        Rgba([
            (rf * 255.0) as u8,
            (gf * 255.0) as u8,
            (bf * 255.0) as u8,
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

    fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
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

    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
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

    // Reset all corrections to default values
    pub fn reset_corrections() -> ColorCorrection {
        ColorCorrection::default()
    }

    // Check if corrections are at default (no changes)
    pub fn has_corrections(corrections: &ColorCorrection) -> bool {
        let default = ColorCorrection::default();
        corrections != &default
    }

    pub fn preset_dark() -> ColorCorrection {
        ColorCorrection {
            contrast: 1.75,
            gamma: 0.28,
            saturation: 0.30,
            hue_shift: 100.0,
            ..ColorCorrection::default()
        }
    }

    pub fn preset_vibrant() -> ColorCorrection {
        ColorCorrection {
            saturation: 1.3,
            contrast: 1.1,
            ..ColorCorrection::default()
        }
    }

    pub fn preset_retro_warm() -> ColorCorrection {
        ColorCorrection {
            hue_shift: 10.0,
            saturation: 1.2,
            brightness: 0.05,
            highlights: -0.1,
            ..ColorCorrection::default()
        }
    }

    pub fn preset_retro_cool() -> ColorCorrection {
        ColorCorrection {
            hue_shift: -15.0,
            saturation: 0.9,
            shadows: 0.1,
            highlights: -0.05,
            ..ColorCorrection::default()
        }
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
    format!("{:.2}", gamma)
}
