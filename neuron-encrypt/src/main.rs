#![windows_subsystem = "windows"]

// main.rs — eframe entry point
// Neuron Encrypt — AES-256-GCM-SIV · Argon2id · HKDF-SHA512
// Custom title bar + JetBrains Mono font embedding

mod gui;

fn main() -> eframe::Result<()> {
    // Window options with platform-specific decorations
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Neuron Encrypt")
        .with_inner_size(eframe::egui::vec2(700.0, 820.0))
        .with_resizable(false)
        .with_maximize_button(false);

    // Platform-specific title bar settings
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

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Neuron Encrypt",
        options,
        Box::new(|cc| {
            // Load and register JetBrains Mono font
            let font_data = eframe::egui::FontData::from_static(include_bytes!(
                "../assets/fonts/JetBrainsMono-Regular.ttf"
            ));

            let mut fonts = eframe::egui::FontDefinitions::default();
            fonts.font_data.insert("JetBrainsMono".to_owned(), font_data);

            // Set as primary monospace font
            fonts
                .families
                .entry(eframe::egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "JetBrainsMono".to_owned());

            // Set as default proportional font fallback
            fonts
                .families
                .entry(eframe::egui::FontFamily::Proportional)
                .or_default()
                .push("JetBrainsMono".to_owned());

            cc.egui_ctx.set_fonts(fonts);

            // Apply custom visuals
            apply_custom_theme(&cc.egui_ctx);

            Box::new(gui::NeuronEncryptApp::new(cc))
        }),
    )
}

fn apply_custom_theme(ctx: &eframe::egui::Context) {
    let mut visuals = eframe::egui::Visuals::dark();

    // Override all default colors with our palette
    visuals.panel_fill = eframe::egui::Color32::from_rgb(0x05, 0x08, 0x0D);
    visuals.window_fill = eframe::egui::Color32::from_rgb(0x0E, 0x15, 0x20);
    visuals.faint_bg_color = eframe::egui::Color32::from_rgb(0x09, 0x0E, 0x15);
    visuals.extreme_bg_color = eframe::egui::Color32::from_rgb(0x05, 0x08, 0x0D);

    // Widget styling - make everything minimal
    visuals.widgets.noninteractive.bg_fill = eframe::egui::Color32::from_rgb(0x0E, 0x15, 0x20);
    visuals.widgets.noninteractive.fg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0xE2, 0xEA, 0xF4));
    visuals.widgets.noninteractive.bg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x1C, 0x2A, 0x3A));

    visuals.widgets.inactive.bg_fill = eframe::egui::Color32::from_rgb(0x13, 0x1D, 0x2B);
    visuals.widgets.inactive.fg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x7A, 0x92, 0xAA));
    visuals.widgets.inactive.bg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x24, 0x35, 0x48));

    visuals.widgets.hovered.bg_fill = eframe::egui::Color32::from_rgb(0x1C, 0x2A, 0x3A);
    visuals.widgets.hovered.fg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x0E, 0xA5, 0xE9));
    visuals.widgets.hovered.bg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x38, 0xBD, 0xF8));

    visuals.widgets.active.bg_fill = eframe::egui::Color32::from_rgb(0x0C, 0x2D, 0x3F);
    visuals.widgets.active.fg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x10, 0xB9, 0x81));
    visuals.widgets.active.bg_stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x34, 0xD3, 0x99));

    visuals.selection.bg_fill = eframe::egui::Color32::from_rgba_premultiplied(0x0E, 0xA5, 0xE9, 40);
    visuals.selection.stroke = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_rgb(0x0E, 0xA5, 0xE9));

    visuals.override_text_color = Some(eframe::egui::Color32::from_rgb(0xE2, 0xEA, 0xF4));

    ctx.set_visuals(visuals);
}
