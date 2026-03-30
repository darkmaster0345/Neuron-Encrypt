#![windows_subsystem = "windows"]

// main.rs — eframe entry point
// Neuron Encrypt — AES-256-GCM-SIV · Argon2id · HKDF-SHA512

mod gui;

fn main() -> eframe::Result<()> {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Neuron Encrypt")
        .with_inner_size(eframe::egui::vec2(660.0, 580.0))
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

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Neuron Encrypt",
        options,
        Box::new(|cc| {
            // Load JetBrains Mono for monospace only
            let font_data = eframe::egui::FontData::from_static(include_bytes!(
                "../assets/fonts/JetBrainsMono-Regular.ttf"
            ));

            let mut fonts = eframe::egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("JetBrainsMono".to_owned(), font_data);

            // Set as primary monospace font (used for log line and tech labels)
            fonts
                .families
                .entry(eframe::egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "JetBrainsMono".to_owned());

            // Add as fallback for proportional (for glyph coverage), but keep
            // the system default proportional font as primary
            fonts
                .families
                .entry(eframe::egui::FontFamily::Proportional)
                .or_default()
                .push("JetBrainsMono".to_owned());

            cc.egui_ctx.set_fonts(fonts);

            // Apply clean minimal theme
            apply_custom_theme(&cc.egui_ctx);

            Box::new(gui::NeuronEncryptApp::new(cc))
        }),
    )
}

fn apply_custom_theme(ctx: &eframe::egui::Context) {
    let mut visuals = eframe::egui::Visuals::dark();

    // Background colors
    visuals.panel_fill = eframe::egui::Color32::from_rgb(0x0F, 0x0F, 0x0F);
    visuals.window_fill = eframe::egui::Color32::from_rgb(0x1A, 0x1A, 0x1A);
    visuals.faint_bg_color = eframe::egui::Color32::from_rgb(0x1A, 0x1A, 0x1A);
    visuals.extreme_bg_color = eframe::egui::Color32::from_rgb(0x0F, 0x0F, 0x0F);

    // Widget styling
    visuals.widgets.noninteractive.bg_fill =
        eframe::egui::Color32::from_rgb(0x1A, 0x1A, 0x1A);
    visuals.widgets.noninteractive.fg_stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0xF5, 0xF5, 0xF5),
    );
    visuals.widgets.noninteractive.bg_stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0x2A, 0x2A, 0x2A),
    );

    visuals.widgets.inactive.bg_fill = eframe::egui::Color32::from_rgb(0x1A, 0x1A, 0x1A);
    visuals.widgets.inactive.fg_stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0xA0, 0xA0, 0xA0),
    );
    visuals.widgets.inactive.bg_stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0x2A, 0x2A, 0x2A),
    );

    visuals.widgets.hovered.bg_fill = eframe::egui::Color32::from_rgb(0x2A, 0x2A, 0x2A);
    visuals.widgets.hovered.fg_stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0x63, 0x66, 0xF1),
    );
    visuals.widgets.hovered.bg_stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0x63, 0x66, 0xF1),
    );

    visuals.widgets.active.bg_fill = eframe::egui::Color32::from_rgb(0x2A, 0x2A, 0x2A);
    visuals.widgets.active.fg_stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0x63, 0x66, 0xF1),
    );
    visuals.widgets.active.bg_stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0x8B, 0x5C, 0xF6),
    );

    visuals.selection.bg_fill =
        eframe::egui::Color32::from_rgba_premultiplied(0x63, 0x66, 0xF1, 40);
    visuals.selection.stroke = eframe::egui::Stroke::new(
        1.0,
        eframe::egui::Color32::from_rgb(0x63, 0x66, 0xF1),
    );

    visuals.override_text_color = Some(eframe::egui::Color32::from_rgb(0xF5, 0xF5, 0xF5));

    ctx.set_visuals(visuals);
}
