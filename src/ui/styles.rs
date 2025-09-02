use egui::Color32;
pub const COLOR_TINT: Color32 = Color32::from_rgb(240, 100, 156);
pub const COLOR_TINT_ACTIVE: Color32 = Color32::from_rgb(131, 100, 144);

pub fn init_styles(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "medium".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/Inter-Medium.ttf")).into(),
    );

    fonts.font_data.insert(
        "extra_bold".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/Inter-ExtraBold.ttf"))
            .into(),
    );

    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "medium".to_owned());

    // Create a custom font family for bold fonts
    fonts.families.insert(
        egui::FontFamily::Name("extra_bold".into()),
        vec!["extra_bold".to_owned()],
    );

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Name("Subheading".into()),
        egui::FontId::new(12.0, egui::FontFamily::Name("extra_bold".into())),
    );

    ctx.set_style(style);
}

pub trait RichTextExt {
    fn subheading(self) -> Self;
}

impl RichTextExt for egui::RichText {
    fn subheading(self) -> Self {
        self.text_style(egui::TextStyle::Name("Subheading".into()))
    }
}

// Extension trait for Ui to add convenient margin methods
pub trait UiMarginExt {
    fn heading_with_margin(&mut self, text: &str);
    fn heading_with_margin_custom(&mut self, text: &str, margin: egui::Margin);
    fn subheading_with_margin(&mut self, text: &str);
    fn subheading_with_margin_custom(&mut self, text: &str, margin: egui::Margin);
}

impl UiMarginExt for egui::Ui {
    fn heading_with_margin(&mut self, text: &str) {
        self.heading_with_margin_custom(
            text,
            egui::Margin {
                left: 0,
                right: 0,
                top: 2,
                bottom: 4,
            },
        );
    }

    fn heading_with_margin_custom(&mut self, text: &str, margin: egui::Margin) {
        egui::Frame::NONE.inner_margin(margin).show(self, |ui| {
            ui.heading(text);
        });
    }

    fn subheading_with_margin(&mut self, text: &str) {
        self.subheading_with_margin_custom(
            text,
            egui::Margin {
                left: 0,
                right: 0,
                top: 2,
                bottom: 4,
            },
        );
    }

    fn subheading_with_margin_custom(&mut self, text: &str, margin: egui::Margin) {
        egui::Frame::NONE.inner_margin(margin).show(self, |ui| {
            ui.label(egui::RichText::new(text).subheading());
        });
    }
}
