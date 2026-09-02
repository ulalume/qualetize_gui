#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ExportFormat {
    #[default]
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

    pub fn mime(&self) -> &'static str {
        match self {
            ExportFormat::PngIndexed | ExportFormat::Png => "image/png",
            ExportFormat::Bmp => "image/bmp",
        }
    }

    /// Formats offered in the export UI. `Png` is reached only through the
    /// dedicated "Color Corrected PNG" menu entry.
    pub fn indexed_list() -> &'static [ExportFormat] {
        &[ExportFormat::Bmp, ExportFormat::PngIndexed]
    }
}
