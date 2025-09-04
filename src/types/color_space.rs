#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, Default)]
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
            ColorSpace::RgbLinear => "RGB Linear",
            ColorSpace::Ycbcr => "YCbCr",
            ColorSpace::Ycocg => "YCoCg",
            ColorSpace::Cielab => "CIELAB",
            ColorSpace::Ictcp => "ICtCp",
            ColorSpace::Oklab => "OkLab",
            ColorSpace::RgbPsy => "RGB + Psyopt",
            ColorSpace::YcbcrPsy => "YCbCr + Psyopt",
            ColorSpace::YcocgPsy => "YCoCg + Psyopt",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ColorSpace::Srgb => "Standard RGB color space",
            ColorSpace::RgbLinear => "Linear RGB color space",
            ColorSpace::Ycbcr => "Luma + Chroma color space",
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

    pub fn to_id(&self) -> u8 {
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
