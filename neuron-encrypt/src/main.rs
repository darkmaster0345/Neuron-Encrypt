#![windows_subsystem = "windows"]

// main.rs — eframe entry point
// Neuron Encrypt — AES-256-GCM-SIV · Argon2id · HKDF-SHA512

// src/main.rs

mod gui;

fn main() -> eframe::Result<()> {
    // Make sure we have OsRng available directly or via crypto.
    // The App is initialized and eframe handles the window.

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Neuron Encrypt")
            .with_inner_size(eframe::egui::vec2(660.0, 780.0))
            .with_resizable(false)
            .with_maximize_button(false),
        ..Default::default()
    };

    eframe::run_native(
        "Neuron Encrypt",
        options,
        Box::new(|cc| Box::new(gui::NeuronEncryptApp::new(cc))),
    )
}
