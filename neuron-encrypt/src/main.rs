#![windows_subsystem = "windows"]
mod gui;

use eframe::egui::{self, Color32, Stroke};

struct Palette;
impl Palette {
    const BG: Color32 = Color32::from_rgb(0x08, 0x08, 0x08);
    const SURFACE: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);
    const SURFACE_HI: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
    const ACCENT: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);
    fn accent_dim() -> Color32 {
        Color32::from_rgba_unmultiplied(99, 102, 241, 18)
    }
    const TEXT_HI: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
}

fn load_icon() -> eframe::egui::IconData {
    // TODO: replace this blank icon with actual icon loading once icon.png is in assets/
    eframe::egui::IconData {
        rgba: vec![0; 4],
        width: 1,
        height: 1,
    }
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Neuron Encrypt")
        .with_inner_size(egui::vec2(620.0, 540.0))
        .with_resizable(false)
        .with_maximize_button(false);
    #[cfg(not(target_os = "macos"))]
    {
        viewport = viewport.with_decorations(false);
    }
    #[cfg(target_os = "macos")]
    {
        viewport = viewport
            .with_decorations(true)
            .with_titlebar_shown(true)
            .with_titlebar_buttons_shown(true);
    }
    viewport = viewport.with_icon(load_icon());

    eframe::run_native(
        "Neuron Encrypt",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| {
            let font_data = egui::FontData::from_static(include_bytes!(
                "../assets/fonts/JetBrainsMono-Regular.ttf"
            ));
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("JetBrainsMono".to_owned(), font_data);
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "JetBrainsMono".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("JetBrainsMono".to_owned());
            cc.egui_ctx.set_fonts(fonts);
            apply_custom_theme(&cc.egui_ctx);
            Box::new(gui::NeuronEncryptApp::new(cc))
        }),
    )
}

fn apply_custom_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Palette::BG;
    visuals.window_fill = Palette::SURFACE;
    visuals.widgets.noninteractive.bg_fill = Palette::SURFACE;
    visuals.widgets.inactive.bg_fill = Palette::SURFACE_HI;
    visuals.widgets.hovered.bg_fill = Palette::SURFACE_HI;
    visuals.widgets.active.bg_fill = Palette::SURFACE_HI;
    visuals.override_text_color = Some(Palette::TEXT_HI);
    visuals.selection.bg_fill = Palette::accent_dim();
    visuals.selection.stroke = Stroke::new(1.0, Palette::ACCENT);
    ctx.set_visuals(visuals);
}
