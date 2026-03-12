// gui.rs — Professional SCIF Terminal UI
// ZERO direct crypto calls. Crypto runs via spawned thread + mpsc channel.
// Color system: Linear.app meets military SCIF terminal aesthetic

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
// use rand_core::RngCore;
use zeroize::Zeroizing;

use neuron_encrypt_core::crypto::{self, ProgressReporter, ThrottledReporter};

// ═══════════════════════════════════════════════════════════════════════════════
// COLOR SYSTEM — STRICT PALETTE
// ═══════════════════════════════════════════════════════════════════════════════
struct Palette;
#[allow(dead_code)]
impl Palette {
    const BG_DEEP:    egui::Color32 = egui::Color32::from_rgb(0x05, 0x08, 0x0D);
    const BG_BASE:    egui::Color32 = egui::Color32::from_rgb(0x09, 0x0E, 0x15);
    const BG_SURFACE: egui::Color32 = egui::Color32::from_rgb(0x0E, 0x15, 0x20);
    const BG_RAISED:  egui::Color32 = egui::Color32::from_rgb(0x13, 0x1D, 0x2B);
    const BORDER_DIM: egui::Color32 = egui::Color32::from_rgb(0x1C, 0x2A, 0x3A);
    const BORDER_MID: egui::Color32 = egui::Color32::from_rgb(0x24, 0x35, 0x48);
    const TEXT_BRIGHT: egui::Color32 = egui::Color32::from_rgb(0xE2, 0xEA, 0xF4);
    const TEXT_MID:   egui::Color32 = egui::Color32::from_rgb(0x7A, 0x92, 0xAA);
    const TEXT_DIM:   egui::Color32 = egui::Color32::from_rgb(0x3D, 0x52, 0x68);
    const CYAN:       egui::Color32 = egui::Color32::from_rgb(0x0E, 0xA5, 0xE9);
    const CYAN_GLOW:  egui::Color32 = egui::Color32::from_rgb(0x38, 0xBD, 0xF8);
    const CYAN_DIM:   egui::Color32 = egui::Color32::from_rgb(0x0C, 0x2D, 0x3F);
    const GREEN:      egui::Color32 = egui::Color32::from_rgb(0x10, 0xB9, 0x81);
    const GREEN_GLOW: egui::Color32 = egui::Color32::from_rgb(0x34, 0xD3, 0x99);
    const GREEN_DIM:  egui::Color32 = egui::Color32::from_rgb(0x0A, 0x1F, 0x16);
    const RED:        egui::Color32 = egui::Color32::from_rgb(0xEF, 0x44, 0x44);
    const AMBER:      egui::Color32 = egui::Color32::from_rgb(0xF5, 0x9E, 0x0B);
}

// ═══════════════════════════════════════════════════════════════════════════════
// BRIDGE: ProgressReporter → mpsc channel
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
// LOG ENTRY
// ═══════════════════════════════════════════════════════════════════════════════
struct LogEntry {
    timestamp: String,
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

fn evaluate_strength(pw: &str) -> (Strength, usize) {
    if pw.is_empty() {
        return (Strength::None, 0);
    }
    let len = pw.len();
    let has_lower = pw.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = pw.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = pw.chars().any(|c| c.is_ascii_digit());
    let has_sym   = pw.chars().any(|c| !c.is_alphanumeric());

    let variety = [has_lower, has_upper, has_digit, has_sym]
        .iter()
        .filter(|&&b| b)
        .count();

    let score = match (len, variety) {
        (0..=3, _)          => 1,
        (4..=5, 0..=1)      => 2,
        (4..=5, _)          => 3,
        (6..=7, 0..=2)      => 4,
        (6..=7, _)          => 5,
        (8..=11, 0..=2)     => 5,
        (8..=11, 3)         => 7,
        (8..=11, _)         => 8,
        (12..=15, 0..=2)    => 7,
        (12..=15, _)        => 9,
        (_, 0..=2)          => 8,
        (_, _)              => 10,
    };

    let level = match score {
        0..=2  => Strength::Weak,
        3..=4  => Strength::Fair,
        5..=7  => Strength::Strong,
        _      => Strength::Elite,
    };

    (level, score.min(10))
}

fn strength_label(strength: &Strength) -> &'static str {
    match strength {
        Strength::None  => "",
        Strength::Weak  => "WEAK",
        Strength::Fair  => "FAIR",
        Strength::Strong => "STRONG",
        Strength::Elite => "ELITE",
    }
}

fn strength_color(strength: &Strength) -> egui::Color32 {
    match strength {
        Strength::None  => Palette::TEXT_DIM,
        Strength::Weak  => Palette::RED,
        Strength::Fair  => Palette::AMBER,
        Strength::Strong => Palette::GREEN,
        Strength::Elite => Palette::CYAN,
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

    // System log (3 lines max)
    log_entries: Vec<LogEntry>,

    // Animation state
    hex_rotation: f32,
    last_frame_time: Instant,
    hex_pulse_start: Option<Instant>,
    status_pulse_phase: f32,
    button_dots_phase: usize,
    shimmer_offset: f32,

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
            log_entries: Vec::new(),
            hex_rotation: 0.0,
            last_frame_time: now,
            hex_pulse_start: None,
            status_pulse_phase: 0.0,
            button_dots_phase: 0,
            shimmer_offset: 0.0,
            last_clock_update: now,
            current_time: time_str,
        };

        app.log("System initialized. Ready.", Palette::GREEN);
        app
    }

    fn log(&mut self, msg: &str, color: egui::Color32) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.log_entries.push(LogEntry {
            timestamp,
            text: msg.to_string(),
            color,
        });
        while self.log_entries.len() > 3 {
            self.log_entries.remove(0);
        }
    }

    fn hex_rpm(&self) -> f32 {
        if self.hex_pulse_start.is_some() {
            3.0 // 3 RPM during pulse
        } else if self.is_processing {
            3.0 // 3 RPM during processing
        } else {
            1.0 // 1 RPM idle
        }
    }

    fn update_animations(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Hex rotation: 1 RPM = 360°/60s = 6°/s
        let rpm = self.hex_rpm();
        let degrees_per_second = rpm * 6.0;
        self.hex_rotation += degrees_per_second * dt;
        if self.hex_rotation >= 360.0 {
            self.hex_rotation -= 360.0;
        }

        // Status dot pulse: 2 second cycle
        let pulse_duration = 2.0f32;
        let elapsed = now.elapsed().as_secs_f32();
        self.status_pulse_phase = (elapsed % pulse_duration) / pulse_duration;

        // Button dots cycle: 400ms
        let dots_duration = 0.4f32;
        let dots_phase = (elapsed % (dots_duration * 3.0)) / dots_duration;
        self.button_dots_phase = dots_phase as usize % 3;

        // Shimmer sweep: 1.2 seconds
        let shimmer_duration = 1.2f32;
        self.shimmer_offset = (elapsed % shimmer_duration) / shimmer_duration;

        // Clock update: every second
        if now.duration_since(self.last_clock_update) >= Duration::from_secs(1) {
            self.last_clock_update = now;
            self.current_time = chrono::Local::now().format("%H:%M:%S").to_string();
        }

        // Check if pulse should end (800ms duration)
        if let Some(start) = self.hex_pulse_start {
            if now.duration_since(start) >= Duration::from_millis(800) {
                self.hex_pulse_start = None;
            }
        }

        // Request 60fps repaint
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn trigger_hex_pulse(&mut self) {
        self.hex_pulse_start = Some(Instant::now());
    }

    // ═══════════════════════════════════════════════════════════════════════════════
    // CRYPTO EXECUTION
    // ═══════════════════════════════════════════════════════════════════════════════
    fn execute(&mut self, ctx: &egui::Context) {
        let Some(ref file_path) = self.selected_file else {
            self.log("ERROR: No file selected.", Palette::RED);
            return;
        };
        if self.password.is_empty() {
            self.log("ERROR: Passphrase is empty.", Palette::RED);
            return;
        }

        let path = file_path.clone();

        // Validate mode vs file
        if self.mode == Mode::Decrypt && !path.to_string_lossy().ends_with(".vx2") {
            self.log("ERROR: File must have .vx2 extension for decryption.", Palette::RED);
            return;
        }

        // Prompt for output file destination
        let dest_path = if self.mode == Mode::Encrypt {
            let default_name = format!("{}.vx2", path.file_name().unwrap_or_default().to_string_lossy());
            rfd::FileDialog::new()
                .set_title("Save Encrypted File As...")
                .set_file_name(&default_name)
                .add_filter("VaultX Encrypted", &["vx2"])
                .save_file()
        } else {
            let src_str = path.to_string_lossy();
            let default_name = if src_str.ends_with(".vx2") {
                src_str[..src_str.len() - 4].to_string()
            } else {
                format!("{}.dec", src_str)
            };
            let default_file_name = PathBuf::from(default_name);
            rfd::FileDialog::new()
                .set_title("Save Decrypted File As...")
                .set_file_name(default_file_name.file_name().unwrap_or_default().to_string_lossy().to_string())
                .save_file()
        };

        if dest_path.is_none() {
            self.log("Operation cancelled by user.", Palette::TEXT_MID);
            return;
        }
        let dest_path = dest_path.unwrap();

        let (tx, rx) = mpsc::channel::<GuiMsg>();
        self.crypto_rx = Some(rx);
        self.is_processing = true;
        self.progress = 0.0;

        let pw = self.password.clone();
        let mode = self.mode;
        let ctx_clone = ctx.clone();

        self.log(
            match mode {
                Mode::Encrypt => "Starting encryption...",
                Mode::Decrypt => "Starting decryption...",
            },
            Palette::CYAN,
        );

        let tx_done = tx.clone();

        std::thread::spawn(move || {
            let reporter = ThrottledReporter::new(MpscReporter { tx });
            let result = match mode {
                Mode::Encrypt => crypto::encrypt_file(&path, &dest_path, &pw, &reporter),
                Mode::Decrypt => crypto::decrypt_file(&path, &dest_path, &pw, &reporter),
            };
            match result {
                Ok(_) => { let _ = tx_done.send(GuiMsg::Done("Operation complete.".into())); }
                Err(e) => { let _ = tx_done.send(GuiMsg::Error(e.to_string())); }
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
                    self.log(&text, Palette::TEXT_BRIGHT);
                }
                GuiMsg::Done(text) => {
                    self.progress = 1.0;
                    self.log(&text, Palette::GREEN);
                    self.is_processing = false;
                    self.trigger_hex_pulse();
                    ctx.request_repaint();
                }
                GuiMsg::Error(text) => {
                    self.progress = 0.0;
                    self.log(&format!("ERROR: {}", text), Palette::RED);
                    self.is_processing = false;
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Handle drag-and-drop
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            if let Some(file) = dropped_files.first() {
                if let Some(path) = &file.path {
                    if !self.is_processing {
                        self.selected_file = Some(path.clone());
                        let ext = path.extension().unwrap_or_default().to_string_lossy().to_string();
                        if ext == "vx2" {
                            self.mode = Mode::Decrypt;
                        } else {
                            self.mode = Mode::Encrypt;
                        }
                    }
                }
            }
        }

        // Poll crypto and update animations
        self.poll_crypto(ctx);
        self.update_animations(ctx);

        // Central panel with custom frame
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Palette::BG_DEEP))
            .show(ctx, |ui| {
                let available = ui.available_rect_before_wrap();

                // Layer 1: Background hex watermark
                self.draw_hex_watermark(ui, available);

                // Layer 2: Custom title bar (Windows/Linux only)
                #[cfg(not(target_os = "macos"))]
                self.draw_custom_title_bar(ui, frame);

                // Calculate content area
                let content_top = if cfg!(target_os = "macos") { 8.0 } else { 44.0 };
                let content_rect = egui::Rect::from_min_max(
                    egui::pos2(available.min.x, available.min.y + content_top),
                    available.max,
                );

                ui.allocate_ui_at_rect(content_rect, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());

                    ui.add_space(8.0);
                    self.draw_status_strip(ui);
                    ui.add_space(16.0);
                    self.draw_hero_section(ui);
                    ui.add_space(16.0);
                    self.draw_mode_selector(ui);
                    ui.add_space(12.0);
                    self.draw_file_drop_zone(ui);
                    ui.add_space(12.0);
                    self.draw_password_section(ui);
                    ui.add_space(12.0);
                    self.draw_execute_button(ui, ctx);
                    ui.add_space(12.0);
                    self.draw_system_log(ui);
                    ui.add_space(8.0);
                    self.draw_progress_bar(ui);
                    ui.add_space(12.0);
                    self.draw_footer(ui);
                });
            });
    }
}

// 
// DRAWING METHODS
// 
impl NeuronEncryptApp {
    // Layer 1: Hex watermark (rotating wireframe hexagon)
    fn draw_hex_watermark(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        let center = rect.center();
        let radius = 210.0; // 420px diameter
        let rotation_rad = self.hex_rotation.to_radians();

        // Calculate hexagon vertices
        let mut vertices = Vec::with_capacity(6);
        for i in 0..6 {
            let angle = rotation_rad + (std::f32::consts::PI / 3.0) * i as f32;
            vertices.push(egui::pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            ));
        }

        // Determine color based on pulse state
        let base_color = Palette::BORDER_DIM;
        let color = if let Some(start) = self.hex_pulse_start {
            let elapsed = start.elapsed().as_secs_f32();
            let pulse_progress = elapsed / 0.8; // 800ms pulse
            if pulse_progress < 0.5 {
                // Flash to cyan-dim
                let t = pulse_progress * 2.0;
                egui::Color32::from_rgb(
                    (0x1C as f32 * (1.0 - t) + 0x0C as f32 * t) as u8,
                    (0x2A as f32 * (1.0 - t) + 0x2D as f32 * t) as u8,
                    (0x3A as f32 * (1.0 - t) + 0x3F as f32 * t) as u8,
                )
            } else {
                // Fade back
                let t = (pulse_progress - 0.5) * 2.0;
                egui::Color32::from_rgb(
                    (0x0C as f32 * (1.0 - t) + 0x1C as f32 * t) as u8,
                    (0x2D as f32 * (1.0 - t) + 0x2A as f32 * t) as u8,
                    (0x3F as f32 * (1.0 - t) + 0x3A as f32 * t) as u8,
                )
            }
        } else {
            base_color
        };

        // Draw hexagon outline with 40% opacity using line segments
        let stroke = egui::Stroke::new(1.5, color.gamma_multiply(0.4));
        for i in 0..6 {
            let start = vertices[i];
            let end = vertices[(i + 1) % 6];
            ui.painter().line_segment([start, end], stroke);
        }
    }

    // Layer 2: Custom title bar for Windows/Linux
    #[cfg(not(target_os = "macos"))]
    fn draw_custom_title_bar(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let title_height = 36.0;
        let (title_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), title_height),
            egui::Sense::click_and_drag(),
        );

        // Background
        ui.painter().rect_filled(title_rect, 0.0, Palette::BG_SURFACE);
        // Bottom border
        let bottom_line = egui::Rect::from_min_max(
            egui::pos2(title_rect.min.x, title_rect.max.y - 1.0),
            title_rect.max,
        );
        ui.painter().rect_filled(bottom_line, 0.0, Palette::BORDER_DIM);

        // Left side: hex icon + title
        let icon_size = 12.0;
        let icon_center = egui::pos2(title_rect.min.x + 16.0 + icon_size/2.0, title_rect.center().y);
        self.draw_small_hexagon(ui, icon_center, icon_size/2.0, Palette::CYAN);

        ui.painter().text(
            egui::pos2(title_rect.min.x + 32.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "NEURON",
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
            Palette::CYAN,
        );

        ui.painter().text(
            egui::pos2(title_rect.min.x + 75.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "ENCRYPT",
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
            Palette::TEXT_MID,
        );

        ui.painter().text(
            egui::pos2(title_rect.min.x + 125.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "  ",
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
            Palette::TEXT_DIM,
        );

        ui.painter().text(
            egui::pos2(title_rect.min.x + 135.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "v1.0.0",
            egui::FontId::new(11.0, egui::FontFamily::Proportional),
            Palette::TEXT_DIM,
        );

        // Right side: window controls
        let btn_size = 12.0;
        let btn_y = title_rect.center().y;
        let close_x = title_rect.max.x - 16.0 - btn_size/2.0;
        let max_x = close_x - 20.0;
        let min_x = max_x - 20.0;

        // Minimize button
        let min_rect = egui::Rect::from_center_size(egui::pos2(min_x, btn_y), egui::vec2(btn_size, btn_size));
        let min_resp = ui.interact(min_rect, egui::Id::new("min_btn"), egui::Sense::click());
        let min_color = if min_resp.hovered() { Palette::TEXT_MID } else { Palette::TEXT_DIM };
        ui.painter().circle_filled(min_rect.center(), btn_size/2.0, min_color);
        if min_resp.clicked() {
            // Minimize not available in eframe 0.27
        }

        // Maximize button (disabled)
        let max_rect = egui::Rect::from_center_size(egui::pos2(max_x, btn_y), egui::vec2(btn_size, btn_size));
        ui.painter().circle_filled(max_rect.center(), btn_size/2.0, Palette::TEXT_DIM);

        // Close button
        let close_rect = egui::Rect::from_center_size(egui::pos2(close_x, btn_y), egui::vec2(btn_size, btn_size));
        let close_resp = ui.interact(close_rect, egui::Id::new("close_btn"), egui::Sense::click());
        let close_color = if close_resp.hovered() { Palette::RED } else { Palette::TEXT_DIM };
        ui.painter().circle_filled(close_rect.center(), btn_size/2.0, close_color);
        if close_resp.clicked() {
            // Close not directly available - user can use OS close
        }

        // Handle drag to move window - drag_window not available in 0.27
        let _drag_area = title_rect;
    }

    fn draw_small_hexagon(&self, ui: &mut egui::Ui, center: egui::Pos2, radius: f32, color: egui::Color32) {
        let mut points = Vec::with_capacity(7);
        for i in 0..6 {
            let angle = (std::f32::consts::PI / 3.0) * i as f32;
            points.push(egui::pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            ));
        }
        // Draw hexagon outline
        for i in 0..6 {
            let start = points[i];
            let end = points[(i + 1) % 6];
            ui.painter().line_segment([start, end], egui::Stroke::new(1.5, color));
        }
    }

    // Layer 3A: Status strip
    fn draw_status_strip(&self, ui: &mut egui::Ui) {
        let strip_height = 32.0;
        let (strip_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), strip_height),
            egui::Sense::hover(),
        );

        // Calculate pulsing opacity
        let pulse = (self.status_pulse_phase * std::f32::consts::PI).sin();
        let dot_opacity = 0.4 + 0.6 * (pulse.abs());
        let dot_color = Palette::GREEN.gamma_multiply(dot_opacity);

        // Left: status dot + SECURE
        ui.painter().circle_filled(
            egui::pos2(strip_rect.min.x + 8.0, strip_rect.center().y),
            4.0,
            dot_color,
        );

        ui.painter().text(
            egui::pos2(strip_rect.min.x + 18.0, strip_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "SECURE",
            egui::FontId::new(10.0, egui::FontFamily::Proportional),
            Palette::GREEN,
        );

        // Center: algorithm info
        let algo_text = "AES-256-GCM-SIV    ARGON2ID    HKDF-SHA512";
        ui.painter().text(
            egui::pos2(strip_rect.center().x, strip_rect.center().y),
            egui::Align2::CENTER_CENTER,
            algo_text,
            egui::FontId::new(10.0, egui::FontFamily::Monospace),
            Palette::TEXT_DIM,
        );

        // Right: clock
        ui.painter().text(
            egui::pos2(strip_rect.max.x - 8.0, strip_rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &self.current_time,
            egui::FontId::new(11.0, egui::FontFamily::Monospace),
            Palette::TEXT_DIM,
        );
    }

    // Layer 3B: Hero section
    fn draw_hero_section(&self, ui: &mut egui::Ui) {
        let section_height = 120.0;
        let (section_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), section_height),
            egui::Sense::hover(),
        );

        // Center the content
        let center_y = section_rect.center().y - 10.0;

        // NEURON | ENCRYPT with separator
        let neuron_text = "NEURON";
        let encrypt_text = "ENCRYPT";

        // Calculate positions for centered layout
        let total_width = 150.0 + 20.0 + 150.0; // approx
        let start_x = section_rect.center().x - total_width / 2.0;

        // NEURON
        ui.painter().text(
            egui::pos2(start_x + 75.0, center_y - 15.0),
            egui::Align2::CENTER_CENTER,
            neuron_text,
            egui::FontId::new(32.0, egui::FontFamily::Proportional),
            Palette::TEXT_BRIGHT,
        );

        // Vertical separator line
        let sep_x = start_x + 150.0 + 10.0;
        ui.painter().line_segment(
            [egui::pos2(sep_x, center_y - 27.0), egui::pos2(sep_x, center_y - 3.0)],
            egui::Stroke::new(1.0, Palette::BORDER_MID),
        );

        // ENCRYPT (cyan)
        ui.painter().text(
            egui::pos2(sep_x + 75.0, center_y - 15.0),
            egui::Align2::CENTER_CENTER,
            encrypt_text,
            egui::FontId::new(32.0, egui::FontFamily::Proportional),
            Palette::CYAN,
        );

        // Subtitle
        ui.painter().text(
            egui::pos2(section_rect.center().x, center_y + 18.0),
            egui::Align2::CENTER_CENTER,
            "Military-Grade File Encryption  //  Rust Edition",
            egui::FontId::new(11.0, egui::FontFamily::Monospace),
            Palette::TEXT_DIM,
        );

        // Gradient line
        let line_y = center_y + 35.0;
        let line_left = section_rect.min.x + 20.0;
        let line_right = section_rect.max.x - 20.0;

        // Draw gradient line
        for x in (line_left as i32)..=(line_right as i32) {
            let t = (x as f32 - line_left) / (line_right - line_left);
            let alpha = ((1.0 - t) * 255.0) as u8;
            ui.painter().line_segment(
                [egui::pos2(x as f32, line_y), egui::pos2((x + 1) as f32, line_y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(0x0E, 0xA5, 0xE9, alpha)),
            );
        }
    }

    // Layer 3C: Mode selector (pill buttons)
    fn draw_mode_selector(&mut self, ui: &mut egui::Ui) {
        let selector_height = 48.0;
        let available_width = ui.available_width();
        let (selector_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, selector_height),
            egui::Sense::click(),
        );

        // Container background
        ui.painter().rect_filled(
            selector_rect,
            999.0, // Full rounding for pill shape
            Palette::BG_SURFACE,
        );
        ui.painter().rect_stroke(
            selector_rect,
            999.0,
            egui::Stroke::new(1.0, Palette::BORDER_DIM),
        );

        let padding = 4.0;
        let inner_width = selector_rect.width() - padding * 2.0;
        let btn_width = (inner_width - 4.0) / 2.0;

        // ENCRYPT pill
        let enc_rect = egui::Rect::from_min_size(
            egui::pos2(selector_rect.min.x + padding, selector_rect.min.y + padding),
            egui::vec2(btn_width, selector_rect.height() - padding * 2.0),
        );
        let enc_resp = ui.interact(enc_rect, egui::Id::new("enc_btn"), egui::Sense::click());

        if self.mode == Mode::Encrypt {
            ui.painter().rect_filled(enc_rect, 999.0, Palette::CYAN_DIM);
            ui.painter().rect_stroke(enc_rect, 999.0, egui::Stroke::new(1.0, Palette::CYAN));
        }

        let enc_text_color = if self.mode == Mode::Encrypt { Palette::CYAN } else { Palette::TEXT_DIM };
        ui.painter().text(
            enc_rect.center(),
            egui::Align2::CENTER_CENTER,
            "ENCRYPT",
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
            enc_text_color,
        );

        if enc_resp.clicked() && !self.is_processing {
            self.mode = Mode::Encrypt;
        }

        // DECRYPT pill
        let dec_rect = egui::Rect::from_min_size(
            egui::pos2(selector_rect.min.x + padding + btn_width + 4.0, selector_rect.min.y + padding),
            egui::vec2(btn_width, selector_rect.height() - padding * 2.0),
        );
        let dec_resp = ui.interact(dec_rect, egui::Id::new("dec_btn"), egui::Sense::click());

        if self.mode == Mode::Decrypt {
            ui.painter().rect_filled(dec_rect, 999.0, Palette::GREEN_DIM);
            ui.painter().rect_stroke(dec_rect, 999.0, egui::Stroke::new(1.0, Palette::GREEN));
        }

        let dec_text_color = if self.mode == Mode::Decrypt { Palette::GREEN } else { Palette::TEXT_DIM };
        ui.painter().text(
            dec_rect.center(),
            egui::Align2::CENTER_CENTER,
            "DECRYPT",
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
            dec_text_color,
        );

        if dec_resp.clicked() && !self.is_processing {
            self.mode = Mode::Decrypt;
        }
    }

    // Layer 3D: File drop zone
    fn draw_file_drop_zone(&mut self, ui: &mut egui::Ui) {
        let zone_height = 80.0;
        let available_width = ui.available_width();
        let (zone_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, zone_height),
            egui::Sense::click(),
        );

        // Background
        ui.painter().rect_filled(zone_rect, 8.0, Palette::BG_SURFACE);

        if self.selected_file.is_none() {
            // Empty state: dashed border
            let stroke = egui::Stroke::new(1.5, Palette::BORDER_MID);
            ui.painter().rect_stroke(zone_rect, 8.0, stroke);

            // Plus icon
            let center = zone_rect.center();
            ui.painter().text(
                egui::pos2(center.x, center.y - 10.0),
                egui::Align2::CENTER_CENTER,
                "",
                egui::FontId::new(20.0, egui::FontFamily::Monospace),
                Palette::TEXT_DIM,
            );

            // Drop text + browse link
            let _text = "Drop file here  or  ";
            let text_width = 100.0; // approximate
            let browse_x = center.x + text_width / 2.0;

            ui.painter().text(
                egui::pos2(center.x, center.y + 15.0),
                egui::Align2::CENTER_CENTER,
                "Drop file here  or  ",
                egui::FontId::new(12.0, egui::FontFamily::Proportional),
                Palette::TEXT_DIM,
            );

            // BROWSE as interactive button
            let browse_rect = egui::Rect::from_center_size(
                egui::pos2(browse_x + 30.0, center.y + 15.0),
                egui::vec2(60.0, 20.0),
            );
            let browse_resp = ui.interact(browse_rect, egui::Id::new("browse_btn"), egui::Sense::click());

            let browse_color = if browse_resp.hovered() { Palette::CYAN_GLOW } else { Palette::CYAN };
            ui.painter().text(
                browse_rect.center(),
                egui::Align2::CENTER_CENTER,
                "BROWSE",
                egui::FontId::new(12.0, egui::FontFamily::Proportional),
                browse_color,
            );

            if browse_resp.clicked() && !self.is_processing {
                let filter = if self.mode == Mode::Decrypt {
                    rfd::FileDialog::new().add_filter("VaultX Encrypted", &["vx2"])
                } else {
                    rfd::FileDialog::new().add_filter("All Files", &["*"])
                };
                if let Some(path) = filter.pick_file() {
                    self.selected_file = Some(path.clone());
                    let ext = path.extension().unwrap_or_default().to_string_lossy().to_string();
                    if ext == "vx2" {
                        self.mode = Mode::Decrypt;
                    } else {
                        self.mode = Mode::Encrypt;
                    }
                }
            }
        } else {
            // Selected state
            let path = self.selected_file.as_ref().unwrap();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            // File icon (small square)
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(zone_rect.min.x + 16.0, zone_rect.center().y - 8.0),
                egui::vec2(16.0, 16.0),
            );
            ui.painter().rect_filled(icon_rect, 2.0, Palette::CYAN);

            // Filename
            ui.painter().text(
                egui::pos2(zone_rect.min.x + 40.0, zone_rect.center().y - 5.0),
                egui::Align2::LEFT_CENTER,
                file_name.chars().take(45).collect::<String>(),
                egui::FontId::new(13.0, egui::FontFamily::Monospace),
                Palette::TEXT_BRIGHT,
            );

            // File size and directory
            if let Ok(meta) = std::fs::metadata(path) {
                let size_kb = meta.len() as f64 / 1024.0;
                let size_display = if size_kb > 1024.0 {
                    format!("{:.2} MB", size_kb / 1024.0)
                } else {
                    format!("{:.1} KB", size_kb)
                };
                let parent = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

                ui.painter().text(
                    egui::pos2(zone_rect.min.x + 40.0, zone_rect.center().y + 12.0),
                    egui::Align2::LEFT_CENTER,
                    format!("{}    {}", size_display, parent),
                    egui::FontId::new(10.0, egui::FontFamily::Monospace),
                    Palette::TEXT_DIM,
                );
            }

            // Clear button (X)
            let clear_rect = egui::Rect::from_center_size(
                egui::pos2(zone_rect.max.x - 20.0, zone_rect.center().y),
                egui::vec2(24.0, 24.0),
            );
            let clear_resp = ui.interact(clear_rect, egui::Id::new("clear_btn"), egui::Sense::click());

            let clear_color = if clear_resp.hovered() { Palette::TEXT_MID } else { Palette::TEXT_DIM };
            ui.painter().text(
                clear_rect.center(),
                egui::Align2::CENTER_CENTER,
                "",
                egui::FontId::new(16.0, egui::FontFamily::Monospace),
                clear_color,
            );

            if clear_resp.clicked() && !self.is_processing {
                self.selected_file = None;
            }
        }
    }

    // Layer 3E: Password section
    fn draw_password_section(&mut self, ui: &mut egui::Ui) {
        let section_height = 96.0;
        let available_width = ui.available_width();
        let (section_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, section_height),
            egui::Sense::click(),
        );

        // Label
        ui.painter().text(
            egui::pos2(section_rect.min.x, section_rect.min.y),
            egui::Align2::LEFT_TOP,
            "PASSPHRASE",
            egui::FontId::new(9.0, egui::FontFamily::Proportional),
            Palette::TEXT_DIM,
        );

        // Password input container
        let input_rect = egui::Rect::from_min_max(
            egui::pos2(section_rect.min.x, section_rect.min.y + 18.0),
            egui::pos2(section_rect.max.x, section_rect.min.y + 54.0),
        );

        // Input background - no focus tracking, simple border
        let border_color = Palette::BORDER_DIM;
        ui.painter().rect_filled(input_rect, 6.0, Palette::BG_RAISED);
        ui.painter().rect_stroke(input_rect, 6.0, egui::Stroke::new(1.0, border_color));

        // Lock icon
        ui.painter().text(
            egui::pos2(input_rect.min.x + 12.0, input_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "",
            egui::FontId::new(14.0, egui::FontFamily::Monospace),
            Palette::TEXT_DIM,
        );

        // Eye toggle button
        let eye_rect = egui::Rect::from_center_size(
            egui::pos2(input_rect.max.x - 20.0, input_rect.center().y),
            egui::vec2(28.0, 28.0),
        );
        let eye_resp = ui.interact(eye_rect, egui::Id::new("eye_btn"), egui::Sense::click());

        let eye_text = if self.show_password { "" } else { "" };
        ui.painter().text(
            eye_rect.center(),
            egui::Align2::CENTER_CENTER,
            eye_text,
            egui::FontId::new(14.0, egui::FontFamily::Monospace),
            Palette::TEXT_DIM,
        );

        if eye_resp.clicked() {
            self.show_password = !self.show_password;
        }

        // Password text input area (using egui's text edit)
        let text_edit_rect = egui::Rect::from_min_max(
            egui::pos2(input_rect.min.x + 32.0, input_rect.min.y + 4.0),
            egui::pos2(input_rect.max.x - 40.0, input_rect.max.y - 4.0),
        );

        ui.allocate_ui_at_rect(text_edit_rect, |ui| {
            let is_empty = self.password.is_empty();
            let mut text_edit = egui::TextEdit::singleline(&mut *self.password)
                .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                .text_color(Palette::TEXT_BRIGHT);

            if !self.show_password {
                text_edit = text_edit.password(true);
            }

            if is_empty {
                ui.add_sized(ui.available_size(), text_edit.hint_text("Enter passphrase..."));
            } else {
                ui.add_sized(ui.available_size(), text_edit);
            }
        });

        // Strength bar
        let (strength, score) = evaluate_strength(&self.password);
        let strength_label_text = strength_label(&strength);
        let strength_label_color = strength_color(&strength);

        let bar_y = section_rect.min.y + 72.0;
        let bar_height = 4.0;
        let bar_width = available_width - 80.0;

        // Track background
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(section_rect.min.x, bar_y),
                egui::pos2(section_rect.min.x + bar_width, bar_y + bar_height),
            ),
            999.0,
            Palette::BG_RAISED,
        );

        // Fill
        if score > 0 {
            let fill_width = bar_width * (score as f32 / 10.0);
            let fill_color = strength_color(&strength);
            ui.painter().rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(section_rect.min.x, bar_y),
                    egui::pos2(section_rect.min.x + fill_width, bar_y + bar_height),
                ),
                999.0,
                fill_color,
            );
        }

        // Strength label
        ui.painter().text(
            egui::pos2(section_rect.max.x, bar_y - 2.0),
            egui::Align2::RIGHT_BOTTOM,
            strength_label_text,
            egui::FontId::new(9.0, egui::FontFamily::Proportional),
            strength_label_color,
        );
    }

    // Layer 3F: Execute button
    fn draw_execute_button(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let btn_height = 56.0;
        let padding = 32.0;
        let available_width = ui.available_width();
        let (btn_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width - padding * 2.0, btn_height),
            egui::Sense::click(),
        );

        // Center the button with padding
        let centered_rect = btn_rect.translate(egui::vec2(padding, 0.0));

        let (gradient_start, gradient_end, border_color, btn_text) = match self.mode {
            Mode::Encrypt => {
                if self.is_processing {
                    (Palette::CYAN_DIM, Palette::CYAN_DIM, Palette::BORDER_DIM, "ENCRYPTING")
                } else {
                    (Palette::CYAN, egui::Color32::from_rgb(0x02, 0x84, 0xC7), Palette::CYAN_GLOW, "ENCRYPT")
                }
            }
            Mode::Decrypt => {
                if self.is_processing {
                    (Palette::GREEN_DIM, Palette::GREEN_DIM, Palette::BORDER_DIM, "DECRYPTING")
                } else {
                    (Palette::GREEN, egui::Color32::from_rgb(0x05, 0x96, 0x69), Palette::GREEN_GLOW, "DECRYPT")
                }
            }
        };

        // Draw gradient background (simulated with rect)
        ui.painter().rect_filled(centered_rect, 8.0, gradient_start);

        // Inner glow border effect
        let inner_rect = centered_rect.shrink(1.0);
        ui.painter().rect_stroke(inner_rect, 8.0, egui::Stroke::new(1.0, border_color));

        // Button text with animated dots while processing
        let display_text = if self.is_processing {
            let dots = match self.button_dots_phase {
                0 => "   ",
                1 => "  ",
                _ => " ",
            };
            format!("{}{}", btn_text, dots)
        } else {
            format!("{}  ", btn_text)
        };

        let text_color = Palette::BG_DEEP;
        ui.painter().text(
            centered_rect.center(),
            egui::Align2::CENTER_CENTER,
            &display_text,
            egui::FontId::new(14.0, egui::FontFamily::Proportional),
            text_color,
        );

        if !self.is_processing {
            let resp = ui.interact(centered_rect, egui::Id::new("exec_btn"), egui::Sense::click());

            // Hover effect: slight Y offset
            if resp.hovered() {
                ui.painter().rect_filled(centered_rect.translate(egui::vec2(0.0, -1.0)), 8.0, gradient_end);
                ui.painter().text(
                    centered_rect.center() + egui::vec2(0.0, -1.0),
                    egui::Align2::CENTER_CENTER,
                    &display_text,
                    egui::FontId::new(14.0, egui::FontFamily::Proportional),
                    text_color,
                );
            }

            if resp.clicked() {
                self.execute(ctx);
            }
        }
    }

    // Layer 3G: System log
    fn draw_system_log(&self, ui: &mut egui::Ui) {
        let log_height = 72.0;
        let available_width = ui.available_width();
        let (log_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, log_height),
            egui::Sense::hover(),
        );

        // Background
        ui.painter().rect_filled(log_rect, 6.0, Palette::BG_SURFACE);
        ui.painter().rect_stroke(log_rect, 6.0, egui::Stroke::new(1.0, Palette::BORDER_DIM));

        // Left accent bar
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(log_rect.min.x, log_rect.min.y),
                egui::pos2(log_rect.min.x + 2.0, log_rect.max.y),
            ),
            0.0,
            Palette::CYAN,
        );

        // Log entries
        let text_x = log_rect.min.x + 12.0;
        let mut text_y = log_rect.min.y + 6.0;

        // Show entries in reverse order (newest first)
        let entries_to_show: Vec<_> = self.log_entries.iter().rev().take(3).collect();

        for (idx, entry) in entries_to_show.iter().enumerate() {
            let text_color = match idx {
                0 => entry.color, // Newest: original color
                1 => Palette::TEXT_MID,
                _ => Palette::TEXT_DIM,
            };

            let log_line = format!("[{}] {}", entry.timestamp, entry.text);
            ui.painter().text(
                egui::pos2(text_x, text_y),
                egui::Align2::LEFT_TOP,
                &log_line,
                egui::FontId::new(10.0, egui::FontFamily::Monospace),
                text_color,
            );
            text_y += 18.0;
        }

        // Pad remaining lines
        for _ in entries_to_show.len()..3 {
            ui.painter().text(
                egui::pos2(text_x, text_y),
                egui::Align2::LEFT_TOP,
                " ",
                egui::FontId::new(10.0, egui::FontFamily::Monospace),
                Palette::TEXT_DIM,
            );
            text_y += 18.0;
        }
    }

    // Layer 3H: Progress bar
    fn draw_progress_bar(&self, ui: &mut egui::Ui) {
        let bar_height = 20.0;
        let available_width = ui.available_width();
        let (bar_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, bar_height),
            egui::Sense::hover(),
        );

        // Percentage label above bar
        let percent = (self.progress * 100.0) as i32;
        ui.painter().text(
            egui::pos2(bar_rect.max.x, bar_rect.min.y - 2.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("{}%", percent),
            egui::FontId::new(9.0, egui::FontFamily::Monospace),
            Palette::TEXT_DIM,
        );

        // Track background
        let track_rect = egui::Rect::from_min_max(
            egui::pos2(bar_rect.min.x, bar_rect.min.y + 6.0),
            egui::pos2(bar_rect.max.x, bar_rect.min.y + 12.0),
        );
        ui.painter().rect_filled(track_rect, 999.0, Palette::BG_RAISED);

        // Fill
        if self.progress > 0.0 {
            let fill_width = track_rect.width() * self.progress;
            let fill_rect = egui::Rect::from_min_max(
                track_rect.min,
                egui::pos2(track_rect.min.x + fill_width, track_rect.max.y),
            );
            let fill_color = if self.mode == Mode::Encrypt { Palette::CYAN } else { Palette::GREEN };
            ui.painter().rect_filled(fill_rect, 999.0, fill_color);

            // Shimmer effect while processing
            if self.is_processing {
                let shimmer_width = fill_width * 0.3;
                let shimmer_x = track_rect.min.x + (fill_width - shimmer_width) * self.shimmer_offset;
                let shimmer_rect = egui::Rect::from_min_max(
                    egui::pos2(shimmer_x, track_rect.min.y),
                    egui::pos2((shimmer_x + shimmer_width).min(track_rect.max.x), track_rect.max.y),
                );

                // Semi-transparent white overlay
                if shimmer_rect.width() > 0.0 {
                    ui.painter().rect_filled(shimmer_rect, 999.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 60));
                }
            }
        }
    }

    // Layer 3I: Footer
    fn draw_footer(&self, ui: &mut egui::Ui) {
        let footer_height = 24.0;
        let (footer_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), footer_height),
            egui::Sense::hover(),
        );

        ui.painter().text(
            footer_rect.center(),
            egui::Align2::CENTER_CENTER,
            "  Passphrase cannot be recovered    Files encrypted with AES-256-GCM-SIV",
            egui::FontId::new(9.0, egui::FontFamily::Proportional),
            Palette::TEXT_DIM,
        );
    }
}
