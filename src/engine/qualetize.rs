//! The Qualetize C library as an engine.

use super::QuantizeResult;
use crate::types::qualetize::{Qualetize, QualetizePlanOwned, Vec4f};
use crate::types::{BGRA8, QualetizeSettings};

pub fn run(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    settings: &QualetizeSettings,
) -> Result<QuantizeResult, String> {
    let bgra_data = to_bgra(rgba_data);
    let plan = QualetizePlanOwned::from(settings.clone());

    let mut output_data: Vec<u8> = vec![0; (width * height) as usize];
    // usize arithmetic: the u16 product would wrap for out-of-range settings
    // and let the library write past the end of the buffer.
    let palette_size = settings.n_palettes as usize * settings.n_colors as usize;
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

    // SAFETY: every buffer is sized as the plan describes (width*height
    // indices, n_palettes*n_colors palette entries), `plan` owns the custom
    // level arrays it points to for the duration of the call, and the
    // library does not retain any of the pointers.
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

    Ok(QuantizeResult {
        indexed_data: output_data,
        palette_data: output_palette,
        colors_per_palette: settings.n_colors as usize,
        width,
        height,
    })
}

/// Qualetize consumes BGRA, egui produces RGBA.
fn to_bgra(rgba: &[u8]) -> Vec<BGRA8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .map(|&[r, g, b, a]| BGRA8 { b, g, r, a })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_conversion_swaps_red_and_blue() {
        let bgra = to_bgra(&[1, 2, 3, 4]);
        assert_eq!((bgra[0].r, bgra[0].g, bgra[0].b, bgra[0].a), (1, 2, 3, 4));
    }
}
