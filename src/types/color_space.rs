#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ColorSpace {
    Srgb,
    #[default]
    RgbLinear,
    Ycbcr,
    Ycocg,
    Cielab,
    Ictcp,
    Oklab,
    RgbPsy,
    YcbcrPsy,
    YcocgPsy,
}

impl ColorSpace {
    pub fn display_name(&self) -> &'static str {
        match self {
            ColorSpace::Srgb => "sRGB",
            ColorSpace::RgbLinear => "RGB linear",
            ColorSpace::Ycbcr => "YCbCr",
            ColorSpace::Ycocg => "YCoCg",
            ColorSpace::Cielab => "CIELAB",
            ColorSpace::Ictcp => "ICtCp",
            ColorSpace::Oklab => "OkLab",
            ColorSpace::RgbPsy => "RGB + psyopt",
            ColorSpace::YcbcrPsy => "YCbCr + psyopt",
            ColorSpace::YcocgPsy => "YCoCg + psyopt",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ColorSpace::Srgb => "Standard RGB color space",
            ColorSpace::RgbLinear => "Linear RGB color space",
            ColorSpace::Ycbcr => "Luma + chroma color space",
            ColorSpace::Ycocg => "Luma + Co/Cg color space",
            ColorSpace::Cielab => {
                "CIE L*a*b* color space\nNOTE: CIELAB has poor performance in most cases"
            }
            ColorSpace::Ictcp => "ITU-R Rec. 2100 ICtCp color space",
            ColorSpace::Oklab => "OkLab perceptual color space",
            ColorSpace::RgbPsy => {
                "RGB with psychovisual optimization\n(Non-linear light, weighted components)"
            }
            ColorSpace::YcbcrPsy => {
                "YCbCr with psychovisual optimization\n(Non-linear luma, weighted chroma)"
            }
            ColorSpace::YcocgPsy => "YCoCg with psychovisual optimization\n(Non-linear luma)",
        }
    }

    pub fn to_id(self) -> u8 {
        match self {
            ColorSpace::Srgb => 0,
            ColorSpace::RgbLinear => 1,
            ColorSpace::Ycbcr => 2,
            ColorSpace::Ycocg => 3,
            ColorSpace::Cielab => 4,
            ColorSpace::Ictcp => 5,
            ColorSpace::Oklab => 6,
            ColorSpace::RgbPsy => 7,
            ColorSpace::YcbcrPsy => 8,
            ColorSpace::YcocgPsy => 9,
        }
    }

    pub fn all() -> &'static [ColorSpace] {
        &[
            ColorSpace::Srgb,
            ColorSpace::RgbLinear,
            ColorSpace::Ycbcr,
            ColorSpace::Ycocg,
            ColorSpace::Cielab,
            ColorSpace::Ictcp,
            ColorSpace::Oklab,
            ColorSpace::RgbPsy,
            ColorSpace::YcbcrPsy,
            ColorSpace::YcocgPsy,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins each variant's id to the `COLOURSPACE_*` constants in
    /// `external/qualetize/include/Qualetize.h`.
    #[test]
    fn ids_match_the_header_constants() {
        assert_eq!(ColorSpace::Srgb.to_id(), 0);
        assert_eq!(ColorSpace::RgbLinear.to_id(), 1);
        assert_eq!(ColorSpace::Ycbcr.to_id(), 2);
        assert_eq!(ColorSpace::Ycocg.to_id(), 3);
        assert_eq!(ColorSpace::Cielab.to_id(), 4);
        assert_eq!(ColorSpace::Ictcp.to_id(), 5);
        assert_eq!(ColorSpace::Oklab.to_id(), 6);
        assert_eq!(ColorSpace::RgbPsy.to_id(), 7);
        assert_eq!(ColorSpace::YcbcrPsy.to_id(), 8);
        assert_eq!(ColorSpace::YcocgPsy.to_id(), 9);
    }
}
