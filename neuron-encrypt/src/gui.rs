// gui.rs — Clean Minimal Modern UI
// ZERO direct crypto calls. Crypto runs via spawned thread + mpsc channel.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use zeroize::Zeroizing;

use neuron_encrypt_core::crypto::{self, ProgressReporter, ThrottledReporter};

// ═══════════════════════════════════════════════════════════════════════════════
// COLOR SYSTEM — CLEAN MINIMAL PALETTE
// ═══════════════════════════════════════════════════════════════════════════════
struct Palette;
impl Palette {
    const BG:             egui::Color32 = egui::Color32::from_rgb(0x0F, 0x0F, 0x0F);
    const SURFACE:        egui::Color32 = egui::Color32::from_rgb(0x1A, 0x1A, 0x1A);
    const BORDER:         egui::Color32 = egui::Color32::from_rgb(0x2A, 0x2A, 0x2A);

    const PRIMARY:        egui::Color32 = egui::Color32::from_rgb(0x63, 0x66, 0xF1);
    const SECONDARY:      egui::Color32 = egui::Color32::from_rgb(0x8B, 0x5C, 0xF6);
    const TEXT_PRIMARY:   egui::Color32 = egui::Color32::from_rgb(0xF5, 0xF5, 0xF5);
    const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0xA0, 0xA0, 0xA0);
    const TEXT_MUTED:     egui::Color32 = egui::Color32::from_rgb(0x55, 0x55, 0x55);
    const SUCCESS:        egui::Color32 = egui::Color32::from_rgb(0x10, 0xB9, 0x81);
    const ERROR:          egui::Color32 = egui::Color32::from_rgb(0xEF, 0x44, 0x44);
    const WARNING:        egui::Color32 = egui::Color32::from_rgb(0xF5, 0x9E, 0x0B);
    const BTN_MID:        egui::Color32 = egui::Color32::from_rgb(0x73, 0x67, 0xF3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// BRIDGE: ProgressReporter -> mpsc channel
// ═══════════════════════════════════════════════════════════════════════════════
enum GuiMsg {
    Progress(f32, String),
    Done(String),
    Error(String),
}

struct MpscReporter {
    tx: mpsc::Sender<GuiMsg>,
}

impl ProgressReporter for MpscReporter {
    fn report(&self, progress: f32, message: &str) {
        let _ = self.tx.send(GuiMsg::Progress(progress, message.to_string()));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATUS MESSAGE
// ═══════════════════════════════════════════════════════════════════════════════
#[derive(Clone)]
struct StatusMessage {
    text: String,
    color: egui::Color32,

}

// ═══════════════════════════════════════════════════════════════════════════════
// MODE
// ═══════════════════════════════════════════════════════════════════════════════
#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Encrypt,
    Decrypt,
}

// ═══════════════════════════════════════════════════════════════════════════════
// PASSWORD STRENGTH
// ═══════════════════════════════════════════════════════════════════════════════
#[derive(PartialEq)]
enum Strength {
    None,
    Weak,
    Fair,
    Strong,
    Elite,
}

fn evaluate_strength(pw: &str) -> (Strength, f32) {
    if pw.is_empty() {
        return (Strength::None, 0.0);
    }
    let len = pw.len();
    let has_lower = pw.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = pw.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = pw.chars().any(|c| c.is_ascii_digit());
    let has_sym = pw.chars().any(|c| !c.is_alphanumeric());

    let variety = [has_lower, has_upper, has_digit, has_sym]
        .iter()
        .filter(|&&b| b)
        .count();

    let score = match (len, variety) {
        (0..=3, _) => 1,
        (4..=5, 0..=1) => 2,
        (4..=5, _) => 3,
        (6..=7, 0..=2) => 4,
        (6..=7, _) => 5,
        (8..=11, 0..=2) => 5,
        (8..=11, 3) => 7,
        (8..=11, _) => 8,
        (12..=15, 0..=2) => 7,
        (12..=15, _) => 9,
        (_, 0..=2) => 8,
        (_, _) => 10,
    };

    let (level, fraction) = match score {
        0 => unreachable!("score 0 cannot occur: empty passwords return early"),
        1..=2 => (Strength::Weak, 0.25),
        3..=4 => (Strength::Fair, 0.50),
        5..=7 => (Strength::Strong, 0.75),
        _ => (Strength::Elite, 1.0),
    };

    (level, fraction)
}

fn strength_color(strength: &Strength) -> egui::Color32 {
    match strength {
        Strength::None => Palette::BORDER,
        Strength::Weak => Palette::ERROR,
        Strength::Fair => Palette::WARNING,
        Strength::Strong => Palette::PRIMARY,
        Strength::Elite => Palette::SUCCESS,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MAIN APPLICATION STATE
// ═══════════════════════════════════════════════════════════════════════════════
pub struct NeuronEncryptApp {
    mode: Mode,
    selected_file: Option<PathBuf>,
    password: Zeroizing<String>,
    show_password: bool,

    // Crypto channel
    crypto_rx: Option<mpsc::Receiver<GuiMsg>>,
    is_processing: bool,
    progress: f32,

    // Status message (single line)
    status: Option<StatusMessage>,

    // Spinner state
    spinner_index: usize,
    last_spinner_tick: Instant,

    // Clock
    last_clock_update: Instant,
    current_time: String,
}

impl NeuronEncryptApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let now = Instant::now();
        let time_str = chrono::Local::now().format("%H:%M:%S").to_string();

        let mut app = Self {
            mode: Mode::Encrypt,
            selected_file: None,
            password: Zeroizing::new(String::new()),
            show_password: false,
            crypto_rx: None,
            is_processing: false,
            progress: 0.0,
            status: None,
            spinner_index: 0,
            last_spinner_tick: now,
            last_clock_update: now,
            current_time: time_str,
        };

        app.set_status("Ready", Palette::PRIMARY);
        app
    }

    fn set_status(&mut self, msg: &str, color: egui::Color32) {
        self.status = Some(StatusMessage {
            text: msg.to_string(),
            color,

        });
    }

    fn spinner_char(&self) -> char {
        const FRAMES: &[char] = &['\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}', '\u{2807}', '\u{280F}'];
        FRAMES[self.spinner_index % FRAMES.len()]
    }

    fn update_clock_and_spinner(&mut self, ctx: &egui::Context) {
        let now = Instant::now();

        // Clock: update every second
        if now.duration_since(self.last_clock_update) >= Duration::from_secs(1) {
            self.last_clock_update = now;
            self.current_time = chrono::Local::now().format("%H:%M:%S").to_string();
        }

        // Spinner: cycle at ~100ms during processing
        if self.is_processing && now.duration_since(self.last_spinner_tick) >= Duration::from_millis(100) {
            self.last_spinner_tick = now;
            self.spinner_index = (self.spinner_index + 1) % 10;
        }

        // Repaint interval: fast during processing, slow during idle
        let interval = if self.is_processing {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(500)
        };
        ctx.request_repaint_after(interval);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // CRYPTO EXECUTION
    // ═══════════════════════════════════════════════════════════════════════════
    fn execute(&mut self, ctx: &egui::Context) {
        let Some(ref file_path) = self.selected_file else {
            self.set_status("No file selected", Palette::ERROR);
            return;
        };
        if self.password.is_empty() {
            self.set_status("Passphrase is empty", Palette::ERROR);
            return;
        }

        let path = file_path.clone();

        if self.mode == Mode::Decrypt && !path.to_string_lossy().ends_with(".vx2") {
            self.set_status("File must have .vx2 extension for decryption", Palette::ERROR);
            return;
        }

        // Prompt for output file destination
        let dest_path = if self.mode == Mode::Encrypt {
            let default_name = format!(
                "{}.vx2",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            rfd::FileDialog::new()
                .set_title("Save Encrypted File As...")
                .set_file_name(&default_name)
                .add_filter("VaultX Encrypted", &["vx2"])
                .save_file()
        } else {
            let src_str = path.to_string_lossy();
            let default_name = if let Some(stripped) = src_str.strip_suffix(".vx2") {
                stripped.to_string()
            } else {
                format!("{}.dec", src_str)
            };
            let default_file_name = PathBuf::from(default_name);
            rfd::FileDialog::new()
                .set_title("Save Decrypted File As...")
                .set_file_name(
                    default_file_name
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                )
                .save_file()
        };

        if dest_path.is_none() {
            self.set_status("Operation cancelled", Palette::TEXT_SECONDARY);
            return;
        }
        let dest_path = dest_path.unwrap();

        let (tx, rx) = mpsc::channel::<GuiMsg>();
        self.crypto_rx = Some(rx);
        self.is_processing = true;
        self.progress = 0.0;

        let pw_bytes = Zeroizing::new(self.password.as_bytes().to_vec());
        let mode = self.mode;
        let ctx_clone = ctx.clone();

        self.set_status(
            match mode {
                Mode::Encrypt => "Starting encryption...",
                Mode::Decrypt => "Starting decryption...",
            },
            Palette::PRIMARY,
        );

        let tx_done = tx.clone();

        std::thread::spawn(move || {
            let reporter = ThrottledReporter::new(MpscReporter { tx });
            let result = match mode {
                Mode::Encrypt => crypto::encrypt_file(&path, &dest_path, &pw_bytes, &reporter),
                Mode::Decrypt => crypto::decrypt_file(&path, &dest_path, &pw_bytes, &reporter),
            };
            match result {
                Ok(_) => {
                    let _ = tx_done.send(GuiMsg::Done("Operation complete".into()));
                }
                Err(e) => {
                    let _ = tx_done.send(GuiMsg::Error(e.to_string()));
                }
            }
            ctx_clone.request_repaint();
        });
    }

    fn poll_crypto(&mut self, ctx: &egui::Context) {
        let mut msgs = Vec::new();
        if let Some(ref rx) = self.crypto_rx {
            while let Ok(msg) = rx.try_recv() {
                msgs.push(msg);
            }
        }

        for msg in msgs {
            match msg {
                GuiMsg::Progress(p, text) => {
                    self.progress = p;
                    self.set_status(&text, Palette::PRIMARY);
                }
                GuiMsg::Done(text) => {
                    self.progress = 1.0;
                    self.set_status(&text, Palette::SUCCESS);
                    self.is_processing = false;
                    self.crypto_rx = None;
                    ctx.request_repaint();
                }
                GuiMsg::Error(text) => {
                    self.progress = 0.0;
                    self.set_status(&text, Palette::ERROR);
                    self.is_processing = false;
                    self.crypto_rx = None;
                    ctx.request_repaint();
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EGUI APP IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════════
impl eframe::App for NeuronEncryptApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle drag-and-drop
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            if let Some(file) = dropped_files.first() {
                if let Some(path) = &file.path {
                    if !self.is_processing {
                        self.selected_file = Some(path.clone());
                        let ext = path
                            .extension()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        if ext == "vx2" {
                            self.mode = Mode::Decrypt;
                        } else {
                            self.mode = Mode::Encrypt;
                        }
                    }
                }
            }
        }

        // Poll crypto and update timers
        self.poll_crypto(ctx);
        self.update_clock_and_spinner(ctx);

        // Main panel
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Palette::BG))
            .show(ctx, |ui| {
                // Title bar (Windows/Linux only)
                #[cfg(not(target_os = "macos"))]
                self.draw_title_bar(ui);

                #[cfg(not(target_os = "macos"))]
                let content_top = 40.0;
                #[cfg(target_os = "macos")]
                let content_top = 0.0;

                let content_rect = egui::Rect::from_min_max(
                    egui::pos2(ui.max_rect().min.x, ui.max_rect().min.y + content_top),
                    ui.max_rect().max,
                );

                ui.allocate_ui_at_rect(content_rect, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());

                    // Center the card horizontally
                    let card_width = 560.0f32.min(ui.available_width() - 32.0);
                    let side_pad = (ui.available_width() - card_width) / 2.0;

                    ui.add_space(16.0);

                    ui.horizontal(|ui| {
                        ui.add_space(side_pad);
                        ui.vertical(|ui| {
                            ui.set_width(card_width);
                            self.draw_card(ui, card_width, ctx);
                        });
                    });

                    // Footer below the card
                    ui.add_space(12.0);
                    self.draw_footer(ui);
                });
            });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DRAWING METHODS
// ═══════════════════════════════════════════════════════════════════════════════
impl NeuronEncryptApp {
    // Title bar (40px)
    #[cfg(not(target_os = "macos"))]
    fn draw_title_bar(&mut self, ui: &mut egui::Ui) {
        let title_height = 40.0;
        let (title_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), title_height),
            egui::Sense::click_and_drag(),
        );

        // Background
        ui.painter().rect_filled(title_rect, 0.0, Palette::SURFACE);
        // Bottom border
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(title_rect.min.x, title_rect.max.y - 1.0),
                title_rect.max,
            ),
            0.0,
            Palette::BORDER,
        );

        // Left: app name + version
        ui.painter().text(
            egui::pos2(title_rect.min.x + 16.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "Neuron Encrypt",
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
            Palette::TEXT_PRIMARY,
        );
        ui.painter().text(
            egui::pos2(title_rect.min.x + 120.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "v1.0.0",
            egui::FontId::new(10.0, egui::FontFamily::Proportional),
            Palette::TEXT_MUTED,
        );

        // Center: tech labels in monospace
        ui.painter().text(
            egui::pos2(title_rect.center().x, title_rect.center().y),
            egui::Align2::CENTER_CENTER,
            "AES-256-GCM-SIV  \u{00B7}  ARGON2ID  \u{00B7}  HKDF-SHA512",
            egui::FontId::new(9.0, egui::FontFamily::Monospace),
            Palette::TEXT_MUTED,
        );

        // Right side: clock + minimize + close
        let btn_radius = 8.0;
        let btn_y = title_rect.center().y;

        // Clock
        ui.painter().text(
            egui::pos2(title_rect.max.x - 90.0, btn_y),
            egui::Align2::RIGHT_CENTER,
            &self.current_time,
            egui::FontId::new(10.0, egui::FontFamily::Monospace),
            Palette::TEXT_MUTED,
        );

        // Minimize button (yellow circle)
        let min_center = egui::pos2(title_rect.max.x - 52.0, btn_y);
        let min_rect = egui::Rect::from_center_size(min_center, egui::vec2(16.0, 16.0));
        let min_resp = ui.interact(min_rect, egui::Id::new("min_btn"), egui::Sense::click());
        let min_color = if min_resp.hovered() {
            egui::Color32::from_rgb(0xFF, 0xBD, 0x2E) // bright yellow
        } else {
            egui::Color32::from_rgb(0xCC, 0x99, 0x00) // muted yellow
        };
        ui.painter().circle_filled(min_center, btn_radius, min_color);
        if min_resp.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        // Close button (red circle)
        let close_center = egui::pos2(title_rect.max.x - 24.0, btn_y);
        let close_rect = egui::Rect::from_center_size(close_center, egui::vec2(16.0, 16.0));
        let close_resp = ui.interact(close_rect, egui::Id::new("close_btn"), egui::Sense::click());
        let close_color = if close_resp.hovered() {
            egui::Color32::from_rgb(0xFF, 0x5F, 0x57) // bright red
        } else {
            egui::Color32::from_rgb(0xCC, 0x33, 0x33) // muted red
        };
        ui.painter()
            .circle_filled(close_center, btn_radius, close_color);
        if close_resp.clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Drag to move window
        let drag_resp = ui.interact(title_rect, egui::Id::new("title_drag"), egui::Sense::drag());
        if drag_resp.drag_started() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }

    // Main card
    fn draw_card(&mut self, ui: &mut egui::Ui, card_width: f32, ctx: &egui::Context) {
        let card_padding = 24.0;

        // Draw card background
        let card_start = ui.cursor().min;
        // We'll draw the background after laying out content to know the height.
        // For now, use a frame approach.

        let card_frame = egui::Frame::none()
            .fill(Palette::SURFACE)
            .stroke(egui::Stroke::new(1.0, Palette::BORDER))
            .rounding(12.0)
            .inner_margin(egui::Margin::same(card_padding));

        card_frame.show(ui, |ui| {
            ui.set_width(card_width - card_padding * 2.0);

            // Mode selector
            self.draw_mode_selector(ui);
            ui.add_space(16.0);

            // File drop zone
            self.draw_file_drop_zone(ui);
            ui.add_space(16.0);

            // Password field
            self.draw_password_field(ui);
            ui.add_space(16.0);

            // Action button
            self.draw_action_button(ui, ctx);

            // Progress bar (only during operation)
            if self.is_processing || self.progress > 0.0 {
                ui.add_space(8.0);
                self.draw_progress_bar(ui);
            }

            // Status line
            ui.add_space(12.0);
            self.draw_status_line(ui);
        });

        let _ = card_start; // suppress unused warning
    }

    // Mode selector tabs
    fn draw_mode_selector(&mut self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let btn_height = 40.0;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, btn_height),
            egui::Sense::hover(),
        );

        let half_w = available_width / 2.0;

        // Encrypt tab
        let enc_rect = egui::Rect::from_min_size(rect.min, egui::vec2(half_w, btn_height));
        let enc_resp = ui.interact(enc_rect, egui::Id::new("mode_enc"), egui::Sense::click());

        if self.mode == Mode::Encrypt {
            ui.painter()
                .rect_filled(enc_rect, 8.0, Palette::PRIMARY);
            ui.painter().text(
                enc_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Encrypt",
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
                Palette::TEXT_PRIMARY,
            );
        } else {
            let text_color = if enc_resp.hovered() {
                Palette::TEXT_PRIMARY
            } else {
                Palette::TEXT_SECONDARY
            };
            ui.painter().text(
                enc_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Encrypt",
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
                text_color,
            );
        }

        if enc_resp.clicked() && !self.is_processing {
            self.mode = Mode::Encrypt;
        }

        // Decrypt tab
        let dec_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + half_w, rect.min.y),
            egui::vec2(half_w, btn_height),
        );
        let dec_resp = ui.interact(dec_rect, egui::Id::new("mode_dec"), egui::Sense::click());

        if self.mode == Mode::Decrypt {
            ui.painter()
                .rect_filled(dec_rect, 8.0, Palette::PRIMARY);
            ui.painter().text(
                dec_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Decrypt",
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
                Palette::TEXT_PRIMARY,
            );
        } else {
            let text_color = if dec_resp.hovered() {
                Palette::TEXT_PRIMARY
            } else {
                Palette::TEXT_SECONDARY
            };
            ui.painter().text(
                dec_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Decrypt",
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
                text_color,
            );
        }

        if dec_resp.clicked() && !self.is_processing {
            self.mode = Mode::Decrypt;
        }
    }

    // File drop zone
    fn draw_file_drop_zone(&mut self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let zone_height = 100.0;
        let (zone_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, zone_height),
            egui::Sense::click(),
        );

        let zone_resp = ui.interact(zone_rect, egui::Id::new("drop_zone"), egui::Sense::click());
        let is_hovered = zone_resp.hovered();

        if self.selected_file.is_none() {
            // Empty state with dashed border
            let border_color = if is_hovered {
                Palette::PRIMARY
            } else {
                Palette::BORDER
            };
            let bg_color = if is_hovered {
                egui::Color32::from_rgba_premultiplied(0x63, 0x66, 0xF1, 13) // 5% tint
            } else {
                egui::Color32::TRANSPARENT
            };

            ui.painter().rect_filled(zone_rect, 10.0, bg_color);

            // Dashed border (simulated with stroke)
            ui.painter()
                .rect_stroke(zone_rect, 10.0, egui::Stroke::new(1.0, border_color));

            // Upload arrow
            ui.painter().text(
                egui::pos2(zone_rect.center().x, zone_rect.center().y - 16.0),
                egui::Align2::CENTER_CENTER,
                "\u{2191}", // ↑
                egui::FontId::new(24.0, egui::FontFamily::Proportional),
                Palette::TEXT_SECONDARY,
            );

            // "Drop file here or"
            ui.painter().text(
                egui::pos2(zone_rect.center().x - 20.0, zone_rect.center().y + 10.0),
                egui::Align2::CENTER_CENTER,
                "Drop file here or",
                egui::FontId::new(13.0, egui::FontFamily::Proportional),
                Palette::TEXT_SECONDARY,
            );

            // "Browse" as clickable indigo text
            let browse_x = zone_rect.center().x + 60.0;
            let browse_rect = egui::Rect::from_center_size(
                egui::pos2(browse_x, zone_rect.center().y + 10.0),
                egui::vec2(50.0, 20.0),
            );
            let browse_resp =
                ui.interact(browse_rect, egui::Id::new("browse_btn"), egui::Sense::click());
            let browse_color = if browse_resp.hovered() {
                Palette::SECONDARY
            } else {
                Palette::PRIMARY
            };
            ui.painter().text(
                browse_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Browse",
                egui::FontId::new(13.0, egui::FontFamily::Proportional),
                browse_color,
            );

            // Handle browse click
            if (browse_resp.clicked() || (zone_resp.clicked() && !browse_resp.hovered()))
                && !self.is_processing
            {
                let filter = if self.mode == Mode::Decrypt {
                    rfd::FileDialog::new().add_filter("VaultX Encrypted", &["vx2"])
                } else {
                    rfd::FileDialog::new().add_filter("All Files", &["*"])
                };
                if let Some(path) = filter.pick_file() {
                    let ext = path
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if ext == "vx2" {
                        self.mode = Mode::Decrypt;
                    } else {
                        self.mode = Mode::Encrypt;
                    }
                    self.selected_file = Some(path);
                }
            }
        } else {
            // File selected state
            let path = self.selected_file.as_ref().unwrap();
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Background
            ui.painter()
                .rect_filled(zone_rect, 10.0, egui::Color32::TRANSPARENT);
            ui.painter().rect_stroke(
                zone_rect,
                10.0,
                egui::Stroke::new(1.0, Palette::BORDER),
            );

            // File icon (small rounded rect)
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(zone_rect.min.x + 16.0, zone_rect.center().y - 12.0),
                egui::vec2(24.0, 24.0),
            );
            ui.painter()
                .rect_filled(icon_rect, 4.0, Palette::PRIMARY);
            ui.painter().text(
                icon_rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{2191}", // ↑
                egui::FontId::new(12.0, egui::FontFamily::Proportional),
                Palette::TEXT_PRIMARY,
            );

            // Filename (truncated)
            let truncated: String = file_name.chars().take(42).collect();
            let display_name = if file_name.chars().count() > 42 {
                format!("{}...", truncated)
            } else {
                file_name.clone()
            };
            ui.painter().text(
                egui::pos2(zone_rect.min.x + 52.0, zone_rect.center().y - 8.0),
                egui::Align2::LEFT_CENTER,
                &display_name,
                egui::FontId::new(13.0, egui::FontFamily::Proportional),
                Palette::TEXT_PRIMARY,
            );

            // File size
            if let Ok(meta) = std::fs::metadata(path) {
                let size = meta.len();
                let size_display = if size >= 1_073_741_824 {
                    format!("{:.2} GB", size as f64 / 1_073_741_824.0)
                } else if size >= 1_048_576 {
                    format!("{:.2} MB", size as f64 / 1_048_576.0)
                } else {
                    format!("{:.1} KB", size as f64 / 1024.0)
                };
                ui.painter().text(
                    egui::pos2(zone_rect.min.x + 52.0, zone_rect.center().y + 10.0),
                    egui::Align2::LEFT_CENTER,
                    &size_display,
                    egui::FontId::new(11.0, egui::FontFamily::Proportional),
                    Palette::TEXT_MUTED,
                );
            }

            // Clear (X) button
            let clear_center = egui::pos2(zone_rect.max.x - 24.0, zone_rect.center().y);
            let clear_rect =
                egui::Rect::from_center_size(clear_center, egui::vec2(24.0, 24.0));
            let clear_resp =
                ui.interact(clear_rect, egui::Id::new("clear_file"), egui::Sense::click());
            let clear_color = if clear_resp.hovered() {
                Palette::TEXT_PRIMARY
            } else {
                Palette::TEXT_MUTED
            };
            ui.painter().text(
                clear_center,
                egui::Align2::CENTER_CENTER,
                "\u{2715}", // ✕
                egui::FontId::new(16.0, egui::FontFamily::Proportional),
                clear_color,
            );

            if clear_resp.clicked() && !self.is_processing {
                self.selected_file = None;
            }
        }
    }

    // Password field with strength bar
    fn draw_password_field(&mut self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let input_height = 44.0;

        // Input container
        let (input_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, input_height),
            egui::Sense::hover(),
        );

        // Background
        ui.painter()
            .rect_filled(input_rect, 8.0, Palette::BG);
        ui.painter().rect_stroke(
            input_rect,
            8.0,
            egui::Stroke::new(1.0, Palette::BORDER),
        );

        // Lock icon on the left
        ui.painter().text(
            egui::pos2(input_rect.min.x + 16.0, input_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "#",
            egui::FontId::new(16.0, egui::FontFamily::Monospace),
            Palette::TEXT_MUTED,
        );

        // Eye toggle on the right
        let eye_rect = egui::Rect::from_center_size(
            egui::pos2(input_rect.max.x - 20.0, input_rect.center().y),
            egui::vec2(28.0, 28.0),
        );
        let eye_resp = ui.interact(eye_rect, egui::Id::new("eye_toggle"), egui::Sense::click());
        let eye_text = if self.show_password { "O" } else { "\u{2014}" }; // O for show, em-dash for hide
        let eye_color = if eye_resp.hovered() {
            Palette::TEXT_SECONDARY
        } else {
            Palette::TEXT_MUTED
        };
        ui.painter().text(
            eye_rect.center(),
            egui::Align2::CENTER_CENTER,
            eye_text,
            egui::FontId::new(14.0, egui::FontFamily::Monospace),
            eye_color,
        );
        if eye_resp.clicked() && !self.is_processing {
            self.show_password = !self.show_password;
        }

        // Password text edit
        let text_rect = egui::Rect::from_min_max(
            egui::pos2(input_rect.min.x + 40.0, input_rect.min.y + 4.0),
            egui::pos2(input_rect.max.x - 40.0, input_rect.max.y - 4.0),
        );
        ui.allocate_ui_at_rect(text_rect, |ui| {
            let mut text_edit = egui::TextEdit::singleline(&mut *self.password)
                .font(egui::FontId::new(14.0, egui::FontFamily::Proportional))
                .text_color(Palette::TEXT_PRIMARY)
                .frame(false);
            if !self.show_password {
                text_edit = text_edit.password(true);
            }
            ui.add_sized(
                ui.available_size(),
                text_edit.hint_text("Enter passphrase..."),
            );
        });

        // Strength bar (3px, immediately below input, sharing border radius bottom)
        let (strength, fraction) = evaluate_strength(&self.password);
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(input_rect.min.x, input_rect.max.y),
            egui::pos2(input_rect.max.x, input_rect.max.y + 3.0),
        );
        // Background
        ui.painter().rect_filled(
            bar_rect,
            egui::Rounding {
                nw: 0.0,
                ne: 0.0,
                sw: 8.0,
                se: 8.0,
            },
            Palette::BORDER,
        );
        // Fill
        if strength != Strength::None {
            let fill_width = bar_rect.width() * fraction;
            let fill_rect = egui::Rect::from_min_max(
                bar_rect.min,
                egui::pos2(bar_rect.min.x + fill_width, bar_rect.max.y),
            );
            let fill_rounding = if fraction >= 0.99 {
                egui::Rounding {
                    nw: 0.0,
                    ne: 0.0,
                    sw: 8.0,
                    se: 8.0,
                }
            } else {
                egui::Rounding {
                    nw: 0.0,
                    ne: 0.0,
                    sw: 8.0,
                    se: 0.0,
                }
            };
            ui.painter().rect_filled(
                fill_rect,
                fill_rounding,
                strength_color(&strength),
            );
        }

        // Reserve space for the bar
        ui.allocate_exact_size(egui::vec2(available_width, 3.0), egui::Sense::hover());
    }

    // Action button
    fn draw_action_button(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let available_width = ui.available_width();
        let btn_height = 48.0;
        let (btn_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, btn_height),
            egui::Sense::hover(),
        );

        let resp = ui.interact(btn_rect, egui::Id::new("action_btn"), egui::Sense::click());

        // Button colors
        let bg_color = if self.is_processing {
            Palette::BTN_MID.gamma_multiply(0.5)
        } else if resp.hovered() {
            Palette::BTN_MID.gamma_multiply(0.9)
        } else {
            Palette::BTN_MID
        };

        ui.painter().rect_filled(btn_rect, 10.0, bg_color);

        // Button text
        let btn_text = if self.is_processing {
            let action = match self.mode {
                Mode::Encrypt => "Encrypting",
                Mode::Decrypt => "Decrypting",
            };
            format!("{} {}...", self.spinner_char(), action)
        } else {
            match self.mode {
                Mode::Encrypt => "Encrypt File \u{2192}".to_string(), // →
                Mode::Decrypt => "Decrypt File \u{2192}".to_string(),
            }
        };

        ui.painter().text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            &btn_text,
            egui::FontId::new(15.0, egui::FontFamily::Proportional),
            Palette::TEXT_PRIMARY,
        );

        if resp.clicked() && !self.is_processing {
            self.execute(ctx);
        }
    }

    // Progress bar
    fn draw_progress_bar(&self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let bar_height = 16.0;
        let (bar_area, _) = ui.allocate_exact_size(
            egui::vec2(available_width, bar_height),
            egui::Sense::hover(),
        );

        // Percentage text
        let percent = (self.progress * 100.0) as i32;
        ui.painter().text(
            egui::pos2(bar_area.max.x, bar_area.min.y),
            egui::Align2::RIGHT_TOP,
            format!("{}%", percent),
            egui::FontId::new(10.0, egui::FontFamily::Monospace),
            Palette::TEXT_MUTED,
        );

        // Track
        let track_rect = egui::Rect::from_min_max(
            egui::pos2(bar_area.min.x, bar_area.min.y + 10.0),
            egui::pos2(bar_area.max.x, bar_area.min.y + 13.0),
        );
        ui.painter()
            .rect_filled(track_rect, 2.0, Palette::BORDER);

        // Fill
        if self.progress > 0.0 {
            let fill_width = track_rect.width() * self.progress.min(1.0);
            let fill_rect = egui::Rect::from_min_max(
                track_rect.min,
                egui::pos2(track_rect.min.x + fill_width, track_rect.max.y),
            );
            ui.painter()
                .rect_filled(fill_rect, 2.0, Palette::PRIMARY);
        }
    }

    // Status line (single message)
    fn draw_status_line(&self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let line_height = 32.0;
        let (line_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, line_height),
            egui::Sense::hover(),
        );

        if let Some(ref status) = self.status {
            // Colored prefix dot
            ui.painter().circle_filled(
                egui::pos2(line_rect.min.x + 6.0, line_rect.center().y),
                3.0,
                status.color,
            );

            // Message text in monospace
            ui.painter().text(
                egui::pos2(line_rect.min.x + 18.0, line_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &status.text,
                egui::FontId::new(11.0, egui::FontFamily::Monospace),
                Palette::TEXT_SECONDARY,
            );
        }
    }

    // Footer
    fn draw_footer(&self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let (footer_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, 36.0),
            egui::Sense::hover(),
        );

        ui.painter().text(
            egui::pos2(footer_rect.center().x, footer_rect.center().y - 7.0),
            egui::Align2::CENTER_CENTER,
            "Passphrase cannot be recovered",
            egui::FontId::new(10.0, egui::FontFamily::Proportional),
            Palette::TEXT_MUTED,
        );
        ui.painter().text(
            egui::pos2(footer_rect.center().x, footer_rect.center().y + 7.0),
            egui::Align2::CENTER_CENTER,
            "Files encrypted with AES-256-GCM-SIV",
            egui::FontId::new(10.0, egui::FontFamily::Proportional),
            Palette::TEXT_MUTED,
        );
    }
}
