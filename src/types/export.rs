#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExportFormat {
    PngIndexed,
    Bmp,
}

impl ExportFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            ExportFormat::PngIndexed => "PNG",
            ExportFormat::Bmp => "BMP",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::PngIndexed => "png",
            ExportFormat::Bmp => "bmp",
        }
    }

    pub fn all() -> &'static [ExportFormat] {
        &[ExportFormat::Bmp, ExportFormat::PngIndexed]
    }
}

impl Default for ExportFormat {
    fn default() -> Self {
        ExportFormat::PngIndexed
    }
}
