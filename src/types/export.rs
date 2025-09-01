#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExportFormat {
    PngIndexed,
    Png,
    Bmp,
}

impl ExportFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            ExportFormat::PngIndexed => "PNG",
            ExportFormat::Png => "PNG32",
            ExportFormat::Bmp => "BMP",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::PngIndexed => "png",
            ExportFormat::Png => "png",
            ExportFormat::Bmp => "bmp",
        }
    }

    pub fn indexed_list() -> &'static [ExportFormat] {
        &[ExportFormat::Bmp, ExportFormat::PngIndexed]
    }

    // pub fn all() -> &'static [ExportFormat] {
    //     &[
    //         ExportFormat::Bmp,
    //         ExportFormat::Png,
    //         ExportFormat::PngIndexed,
    //     ]
    // }
}

impl Default for ExportFormat {
    fn default() -> Self {
        ExportFormat::PngIndexed
    }
}
