#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum DitherMode {
    None,
    #[default]
    Floyd,
    Atkinson,
    Checker,
    Ord2,
    Ord4,
    Ord8,
    Ord16,
    Ord32,
    Ord64,
}

impl DitherMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            DitherMode::None => "None",
            DitherMode::Floyd => "Floyd-Steinberg",
            DitherMode::Atkinson => "Atkinson",
            DitherMode::Checker => "Checkerboard",
            DitherMode::Ord2 => "2x2 Ordered",
            DitherMode::Ord4 => "4x4 Ordered",
            DitherMode::Ord8 => "8x8 Ordered",
            DitherMode::Ord16 => "16x16 Ordered",
            DitherMode::Ord32 => "32x32 Ordered",
            DitherMode::Ord64 => "64x64 Ordered",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            DitherMode::None => "No dithering",
            DitherMode::Floyd => "Floyd-Steinberg error diffusion (default level: 0.5)",
            DitherMode::Atkinson => "Atkinson error diffusion (default level: 0.5)",
            DitherMode::Checker => "Checkerboard dithering (default level: 1.0)",
            DitherMode::Ord2 => "2x2 ordered dithering (default level: 1.0)",
            DitherMode::Ord4 => "4x4 ordered dithering (default level: 1.0)",
            DitherMode::Ord8 => "8x8 ordered dithering (default level: 1.0)",
            DitherMode::Ord16 => "16x16 ordered dithering (default level: 1.0)",
            DitherMode::Ord32 => "32x32 ordered dithering (default level: 1.0)",
            DitherMode::Ord64 => "64x64 ordered dithering (default level: 1.0)",
        }
    }

    pub fn to_id(&self) -> u8 {
        match self {
            DitherMode::None => 0,
            DitherMode::Floyd => 0xFE,
            DitherMode::Atkinson => 0xFD,
            DitherMode::Checker => 0xFF,
            DitherMode::Ord2 => 2,
            DitherMode::Ord4 => 4,
            DitherMode::Ord8 => 6,
            DitherMode::Ord16 => 7,
            DitherMode::Ord32 => 8,
            DitherMode::Ord64 => 9,
        }
    }

    pub fn all() -> &'static [DitherMode] {
        &[
            DitherMode::None,
            DitherMode::Floyd,
            DitherMode::Atkinson,
            DitherMode::Checker,
            DitherMode::Ord2,
            DitherMode::Ord4,
            DitherMode::Ord8,
            DitherMode::Ord16,
            DitherMode::Ord32,
            DitherMode::Ord64,
        ]
    }
}
