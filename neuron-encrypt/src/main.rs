#![windows_subsystem = "windows"]
mod gui;

fn main() -> eframe::Result<()> {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Neuron Encrypt")
        .with_inner_size(eframe::egui::vec2(660.0, 580.0))
        .with_resizable(false)
        .with_maximize_button(false);
    #[cfg(not(target_os = "macos"))] { viewport = viewport.with_decorations(false); }
    #[cfg(target_os = "macos")] { viewport = viewport.with_decorations(true).with_titlebar_shown(true).with_titlebar_buttons_shown(true); }
    eframe::run_native("Neuron Encrypt", eframe::NativeOptions { viewport, ..Default::default() }, Box::new(|cc| {
        let font_data = eframe::egui::FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"));
        let mut fonts = eframe::egui::FontDefinitions::default();
        fonts.font_data.insert("JetBrainsMono".to_owned(), font_data);
        fonts.families.entry(eframe::egui::FontFamily::Monospace).or_default().insert(0, "JetBrainsMono".to_owned());
        fonts.families.entry(eframe::egui::FontFamily::Proportional).or_default().push("JetBrainsMono".to_owned());
        cc.egui_ctx.set_fonts(fonts);
        apply_custom_theme(&cc.egui_ctx);
        Box::new(gui::NeuronEncryptApp::new(cc))
    }))
}

fn apply_custom_theme(ctx: &eframe::egui::Context) {
    let mut visuals = eframe::egui::Visuals::dark();
    visuals.panel_fill = eframe::egui::Color32::from_rgb(0x08, 0x08, 0x08);
    visuals.window_fill = eframe::egui::Color32::from_rgb(0x0F, 0x0F, 0x0F);
    visuals.faint_bg_color = eframe::egui::Color32::from_rgb(0x16, 0x16, 0x16);
    visuals.extreme_bg_color = eframe::egui::Color32::from_rgb(0x08, 0x08, 0x08);
    visuals.widgets.inactive.bg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x2A, 0x2A, 0x2A));
    visuals.widgets.hovered.bg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x3A, 0x3A, 0x3A));
    visuals.widgets.active.bg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x63, 0x66, 0xF1));
    visuals.selection.bg_fill = eframe::egui::Color32::from_rgba_unmultiplied(99, 102, 241, 30);
    visuals.selection.stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x63, 0x66, 0xF1));
    visuals.override_text_color = Some(eframe::egui::Color32::from_rgb(0xF8, 0xF8, 0xF8));
    ctx.set_visuals(visuals);
}
