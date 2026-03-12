// gui.rs — V1 Hex Grid UI + Explicit File Save Prompts
// ZERO direct crypto calls. Crypto runs via spawned thread + mpsc channel.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use rand_core::RngCore;
use zeroize::Zeroizing;

use neuron_encrypt_core::crypto::{self, ProgressReporter, ThrottledReporter};

// ── Bridge: ProgressReporter → mpsc channel ────────────────────
// This is the ONLY platform-specific glue between crypto and GUI.
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

// ── Colours ────────────────────────────────────────────────────
struct Palette;
#[allow(dead_code)]
impl Palette {
    const BG:      egui::Color32 = egui::Color32::from_rgb(0x08, 0x0C, 0x10);
    const CARD:    egui::Color32 = egui::Color32::from_rgb(0x0F, 0x15, 0x1C);
    const PANEL:   egui::Color32 = egui::Color32::from_rgb(0x0D, 0x11, 0x17);
    const BORDER:  egui::Color32 = egui::Color32::from_rgb(0x1A, 0x25, 0x35);
    const CYAN:    egui::Color32 = egui::Color32::from_rgb(0x00, 0xD4, 0xFF);
    const GREEN:   egui::Color32 = egui::Color32::from_rgb(0x00, 0xFF, 0x9D);
    const RED:     egui::Color32 = egui::Color32::from_rgb(0xFF, 0x3D, 0x5A);
    const AMBER:   egui::Color32 = egui::Color32::from_rgb(0xFF, 0xB0, 0x20);
    const TEXT:    egui::Color32 = egui::Color32::from_rgb(0xC0, 0xD0, 0xE0);
    const DIM:     egui::Color32 = egui::Color32::from_rgb(0x30, 0x40, 0x50);

    const CYAN_DIM:  egui::Color32 = egui::Color32::from_rgb(0x00, 0x35, 0x45);
    const GREEN_DIM: egui::Color32 = egui::Color32::from_rgb(0x00, 0x40, 0x28);
}

// ── Hex cell for animation ─────────────────────────────────────
struct HexCell {
    vertices: Vec<egui::Pos2>,         // 6 vertex positions (computed once)
    lit_until: Option<Instant>,        // when the cell should go dark again
    lit_color: egui::Color32,
}

// ── Log entry ──────────────────────────────────────────────────
struct LogEntry {
    text: String,
    color: egui::Color32,
}

// ── Mode ───────────────────────────────────────────────────────
#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Encrypt,
    Decrypt,
}

// ── Password strength level ────────────────────────────────────
#[derive(PartialEq)]
enum Strength {
    None,
    Weak,
    Fair,
    Good,
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
        5..=7  => Strength::Good,
        _      => Strength::Elite,
    };

    (level, score.min(10))
}

// ── Main application state ─────────────────────────────────────
pub struct NeuronEncryptApp {
    mode: Mode,
    selected_file: Option<PathBuf>,
    password: Zeroizing<String>,       // ALWAYS Zeroizing — never plain String
    show_password: bool,

    // Crypto channel
    crypto_rx: Option<mpsc::Receiver<GuiMsg>>,
    is_processing: bool,
    progress: f32,

    // System log (3 lines max)
    log_entries: Vec<LogEntry>,

    // Hex animation (computed once at startup)
    hex_cells: Vec<HexCell>,
    last_hex_tick: Instant,

    // Clock
    #[allow(dead_code)]
    start_time: Instant,
}

impl NeuronEncryptApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let hex_cells = Self::build_hex_grid();
        let mut app = Self {
            mode: Mode::Encrypt,
            selected_file: None,
            password: Zeroizing::new(String::new()),
            show_password: false,
            crypto_rx: None,
            is_processing: false,
            progress: 0.0,
            log_entries: Vec::new(),
            hex_cells,
            last_hex_tick: Instant::now(),
            start_time: Instant::now(),
        };
        app.log("System initialised. Ready.", Palette::GREEN);
        app
    }

    // Build hex grid ONCE — max 200 cells
    fn build_hex_grid() -> Vec<HexCell> {
        let hex_size: f32 = 18.0;
        let cols = 26;
        let rows = 7;
        let total = (cols * rows).min(200);

        let mut cells = Vec::with_capacity(total);
        for idx in 0..total {
            let col = idx % cols;
            let row = idx / cols;

            let x = col as f32 * hex_size * 1.55 + 30.0;
            let mut y = row as f32 * hex_size * 1.75 + hex_size + 4.0;
            if col % 2 == 1 {
                y += hex_size * 0.875;
            }

            let r = hex_size * 0.82;
            let mut verts = Vec::with_capacity(6);
            for i in 0..6 {
                let angle = std::f32::consts::PI / 3.0 * i as f32;
                verts.push(egui::pos2(x + r * angle.cos(), y + r * angle.sin()));
            }

            cells.push(HexCell {
                vertices: verts,
                lit_until: None,
                lit_color: Palette::CYAN_DIM,
            });
        }
        cells
    }

    fn log(&mut self, msg: &str, color: egui::Color32) {
        self.log_entries.push(LogEntry {
            text: msg.to_string(),
            color,
        });
        // Keep only the last 3
        while self.log_entries.len() > 3 {
            self.log_entries.remove(0);
        }
    }

    // ── Spawn crypto on background thread ──────────────────────
    fn execute(&mut self) {
        let Some(ref file_path) = self.selected_file else {
            self.log("ERROR: No file selected.", Palette::RED);
            return;
        };
        if self.password.is_empty() {
            self.log("ERROR: Password is empty.", Palette::RED);
            return;
        }

        let path = file_path.clone();

        // Validate mode vs file
        if self.mode == Mode::Decrypt && !path.to_string_lossy().ends_with(".vx2") {
            self.log("ERROR: File must have .vx2 extension for decryption.", Palette::RED);
            return;
        }

        // Prompt for output file destination natively (V2 functionality brought to V1 UI!)
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
            self.log("Operation cancelled by user.", Palette::TEXT);
            return;
        }
        let dest_path = dest_path.unwrap();


        let (tx, rx) = mpsc::channel::<GuiMsg>();
        self.crypto_rx = Some(rx);
        self.is_processing = true;
        self.progress = 0.0;

        let pw = self.password.clone();
        let mode = self.mode;
        
        self.log(
            match mode {
                Mode::Encrypt => "Starting encryption…",
                Mode::Decrypt => "Starting decryption…",
            },
            Palette::CYAN,
        );

        let tx_done = tx.clone();
        
        // Spawn — crypto NEVER runs on GUI thread
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
        });
    }

    // ── Poll channel every frame ───────────────────────────────
    fn poll_crypto(&mut self) {
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
                    self.log(&text, Palette::TEXT);
                }
                GuiMsg::Done(text) => {
                    self.progress = 1.0;
                    self.log(&text, Palette::GREEN);
                    self.is_processing = false;
                }
                GuiMsg::Error(text) => {
                    self.progress = 0.0;
                    self.log(&format!("ERROR: {}", text), Palette::RED);
                    self.is_processing = false;
                }
            }
        }
    }

    // ── Hex animation tick ─────────────────────────────────────
    fn tick_hex_animation(&mut self) {
        let now = Instant::now();

        // Every 150ms: light 1-2 random cells
        if now.duration_since(self.last_hex_tick) >= Duration::from_millis(150) {
            self.last_hex_tick = now;
            let count = if rand_core::OsRng.next_u32() % 3 == 0 { 2 } else { 1 };
            for _ in 0..count {
                if !self.hex_cells.is_empty() {
                    let idx = (rand_core::OsRng.next_u32() as usize) % self.hex_cells.len();
                    let color = if rand_core::OsRng.next_u32() % 2 == 0 {
                        Palette::CYAN_DIM
                    } else {
                        Palette::GREEN_DIM
                    };
                    self.hex_cells[idx].lit_color = color;
                    self.hex_cells[idx].lit_until = Some(now + Duration::from_millis(500));
                }
            }
        }

        // Restore expired cells
        for cell in &mut self.hex_cells {
            if let Some(until) = cell.lit_until {
                if now >= until {
                    cell.lit_until = None;
                }
            }
        }
    }
}

impl eframe::App for NeuronEncryptApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle global drag-and-drop seamlessly
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

        // Poll crypto channel
        self.poll_crypto();

        // Tick hex animation
        self.tick_hex_animation();

        // Keep repainting for clock + animation + crypto polling
        ctx.request_repaint_after(Duration::from_millis(100));

        // Apply custom dark theme
        apply_theme(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Palette::BG))
            .show(ctx, |ui| {
                ui.set_min_width(640.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.draw_status_bar(ui);
                    ui.add_space(8.0);
                    self.draw_title(ui);
                    ui.add_space(4.0);
                    self.draw_hex_grid(ui);
                    ui.add_space(12.0);
                    self.draw_mode_selector(ui);
                    ui.add_space(10.0);
                    self.draw_file_selector(ui);
                    ui.add_space(10.0);
                    self.draw_password_input(ui);
                    ui.add_space(10.0);
                    self.draw_system_log(ui);
                    ui.add_space(8.0);
                    self.draw_progress_bar(ui);
                    ui.add_space(10.0);
                    self.draw_execute_button(ui);
                    ui.add_space(10.0);
                    self.draw_footer(ui);
                    ui.add_space(10.0);
                });
            });
    }
}

// ── Theme ──────────────────────────────────────────────────────
fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = Palette::BG;
    visuals.window_fill = Palette::CARD;
    visuals.faint_bg_color = Palette::PANEL;
    visuals.extreme_bg_color = Palette::CARD;

    visuals.widgets.noninteractive.bg_fill = Palette::CARD;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, Palette::TEXT);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Palette::BORDER);

    visuals.widgets.inactive.bg_fill = Palette::PANEL;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, Palette::TEXT);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Palette::BORDER);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x15, 0x1F, 0x2A);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Palette::CYAN);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Palette::CYAN);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0x10, 0x1A, 0x25);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Palette::GREEN);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, Palette::GREEN);

    visuals.selection.bg_fill = egui::Color32::from_rgba_premultiplied(0x00, 0xD4, 0xFF, 40);
    visuals.selection.stroke = egui::Stroke::new(1.0, Palette::CYAN);

    visuals.override_text_color = Some(Palette::TEXT);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(13.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(13.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(22.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(10.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(13.0, egui::FontFamily::Monospace),
    );
    ctx.set_style(style);
}

// ── Drawing methods ────────────────────────────────────────────
impl NeuronEncryptApp {
    // [1] STATUS BAR
    fn draw_status_bar(&self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(Palette::PANEL)
            .inner_margin(egui::Margin::symmetric(16.0, 6.0))
            .rounding(egui::Rounding::same(4.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Green dot
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 5.0, Palette::GREEN);
                    ui.add_space(4.0);

                    ui.colored_label(Palette::GREEN, "● ONLINE");
                    ui.add_space(16.0);
                    ui.colored_label(Palette::TEXT, "AES-256-GCM-SIV · Argon2id · HKDF-SHA512");

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let now = chrono::Local::now();
                        let clock = now.format("%H:%M:%S").to_string();
                        ui.colored_label(Palette::CYAN, &clock);
                    });
                });
            });
    }

    // [2] TITLE
    fn draw_title(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() / 2.0 - 120.0);
                ui.label(
                    egui::RichText::new("NEURON")
                        .font(egui::FontId::new(36.0, egui::FontFamily::Monospace))
                        .color(Palette::CYAN)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new("ENCRYPT")
                        .font(egui::FontId::new(36.0, egui::FontFamily::Monospace))
                        .color(Palette::GREEN)
                        .strong(),
                );
            });
            ui.colored_label(
                Palette::DIM,
                "MILITARY-GRADE FILE ENCRYPTION // RUST EDITION",
            );
        });
    }

    // [3] HEX GRID ANIMATION
    fn draw_hex_grid(&self, ui: &mut egui::Ui) {
        let desired_size = egui::vec2(ui.available_width(), 120.0);
        let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let offset = rect.min.to_vec2();

        for cell in &self.hex_cells {
            let translated: Vec<egui::Pos2> = cell.vertices.iter().map(|v| *v + offset).collect();

            if cell.lit_until.is_some() {
                // Filled glow
                painter.add(egui::Shape::convex_polygon(
                    translated.clone(),
                    cell.lit_color,
                    egui::Stroke::new(1.0, cell.lit_color),
                ));
            } else {
                // Outline only
                let mut outline = translated.clone();
                outline.push(translated[0]); // close the loop
                painter.add(egui::Shape::line(
                    outline,
                    egui::Stroke::new(0.5, Palette::BORDER),
                ));
            }
        }
    }

    // [4] MODE SELECTOR
    fn draw_mode_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let half = (ui.available_width() - 12.0) / 2.0;

            // ENCRYPT button
            let enc_fill = if self.mode == Mode::Encrypt { Palette::CYAN } else { Palette::PANEL };
            let enc_text_color = if self.mode == Mode::Encrypt { Palette::BG } else { Palette::DIM };
            let enc_btn = egui::Button::new(
                egui::RichText::new("⟐  ENCRYPT MODE")
                    .color(enc_text_color)
                    .strong()
                    .size(14.0),
            )
            .fill(enc_fill)
            .min_size(egui::vec2(half, 36.0))
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, if self.mode == Mode::Encrypt { Palette::CYAN } else { Palette::BORDER }));

            if ui.add(enc_btn).clicked() && !self.is_processing {
                self.mode = Mode::Encrypt;
            }

            ui.add_space(4.0);

            // DECRYPT button
            let dec_fill = if self.mode == Mode::Decrypt { Palette::GREEN } else { Palette::PANEL };
            let dec_text_color = if self.mode == Mode::Decrypt { Palette::BG } else { Palette::DIM };
            let dec_btn = egui::Button::new(
                egui::RichText::new("⟐  DECRYPT MODE")
                    .color(dec_text_color)
                    .strong()
                    .size(14.0),
            )
            .fill(dec_fill)
            .min_size(egui::vec2(half, 36.0))
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, if self.mode == Mode::Decrypt { Palette::GREEN } else { Palette::BORDER }));

            if ui.add(dec_btn).clicked() && !self.is_processing {
                self.mode = Mode::Decrypt;
            }
        });
    }

    // [5] FILE SELECTOR
    fn draw_file_selector(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(Palette::CARD)
            .stroke(egui::Stroke::new(1.0, Palette::BORDER))
            .inner_margin(egui::Margin::same(12.0))
            .rounding(egui::Rounding::same(6.0))
            .show(ui, |ui| {
                ui.colored_label(Palette::CYAN, "TARGET FILE");
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    let display_text = self
                        .selected_file
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "No file selected (Drag & Drop anywhere)".to_string());

                    let mut text_buf = display_text.clone();
                    ui.add_sized(
                        egui::vec2(ui.available_width() - 90.0, 28.0),
                        egui::TextEdit::singleline(&mut text_buf)
                            .font(egui::FontId::new(12.0, egui::FontFamily::Monospace))
                            .interactive(false),
                    );

                    let browse_btn = egui::Button::new(
                        egui::RichText::new("BROWSE").color(Palette::BG).strong(),
                    )
                    .fill(Palette::CYAN)
                    .min_size(egui::vec2(80.0, 28.0))
                    .rounding(egui::Rounding::same(4.0));

                    if ui.add(browse_btn).clicked() && !self.is_processing {
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
                });

                // Show file info if selected
                if let Some(ref path) = self.selected_file {
                    ui.add_space(6.0);
                    if let Ok(meta) = std::fs::metadata(path) {
                        let size_kb = meta.len() as f64 / 1024.0;
                        let size_display = if size_kb > 1024.0 {
                            format!("{:.2} MB", size_kb / 1024.0)
                        } else {
                            format!("{:.1} KB", size_kb)
                        };
                        ui.horizontal(|ui| {
                            ui.colored_label(Palette::DIM, "File:");
                            ui.colored_label(
                                Palette::TEXT,
                                path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                            );
                            ui.colored_label(Palette::DIM, "│");
                            ui.colored_label(Palette::TEXT, &size_display);
                        });
                        if let Some(parent) = path.parent() {
                            ui.horizontal(|ui| {
                                ui.colored_label(Palette::DIM, "Dir: ");
                                ui.colored_label(
                                    Palette::DIM,
                                    parent.to_string_lossy().to_string(),
                                );
                            });
                        }
                    }
                }
            });
    }

    // [6] PASSWORD INPUT
    fn draw_password_input(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(Palette::CARD)
            .stroke(egui::Stroke::new(1.0, Palette::BORDER))
            .inner_margin(egui::Margin::same(12.0))
            .rounding(egui::Rounding::same(6.0))
            .show(ui, |ui| {
                ui.colored_label(Palette::CYAN, "ENCRYPTION KEY");
                ui.add_space(6.0);

                // Password field — mutated directly in Zeroizing<String>.
                // NEVER clone into a plain String to avoid memory leaks.
                ui.add_sized(
                    egui::vec2(ui.available_width(), 30.0),
                    egui::TextEdit::singleline(&mut *self.password)
                        .password(!self.show_password)
                        .font(egui::FontId::new(14.0, egui::FontFamily::Monospace)),
                );

                ui.add_space(4.0);

                // Reveal checkbox
                ui.checkbox(&mut self.show_password, "Reveal Key");

                ui.add_space(4.0);

                // Strength meter — 10 coloured squares
                let (strength, score) = evaluate_strength(&self.password);
                let strength_color = match strength {
                    Strength::None  => Palette::DIM,
                    Strength::Weak  => Palette::RED,
                    Strength::Fair  => Palette::AMBER,
                    Strength::Good  => Palette::GREEN,
                    Strength::Elite => Palette::CYAN,
                };
                let label_text = match strength {
                    Strength::None  => "NONE",
                    Strength::Weak  => "WEAK",
                    Strength::Fair  => "FAIR",
                    Strength::Good  => "GOOD",
                    Strength::Elite => "ELITE",
                };

                ui.horizontal(|ui| {
                    for i in 0..10 {
                        let color = if i < score { strength_color } else { Palette::DIM };
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(16.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, color);
                    }
                    ui.add_space(8.0);
                    ui.colored_label(strength_color, label_text);
                });
            });
    }

    // [7] SYSTEM LOG — 3 scrolling lines
    fn draw_system_log(&self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(Palette::CARD)
            .stroke(egui::Stroke::new(1.0, Palette::BORDER))
            .inner_margin(egui::Margin::same(10.0))
            .rounding(egui::Rounding::same(6.0))
            .show(ui, |ui| {
                ui.colored_label(Palette::DIM, "SYSTEM LOG");
                ui.add_space(4.0);
                for entry in &self.log_entries {
                    ui.label(
                        egui::RichText::new(&entry.text)
                            .font(egui::FontId::new(11.0, egui::FontFamily::Monospace))
                            .color(entry.color),
                    );
                }
                // Pad empty lines to keep height stable
                for _ in self.log_entries.len()..3 {
                    ui.label(
                        egui::RichText::new(" ")
                            .font(egui::FontId::new(11.0, egui::FontFamily::Monospace)),
                    );
                }
            });
    }

    // [8] PROGRESS BAR
    fn draw_progress_bar(&self, ui: &mut egui::Ui) {
        let bar = egui::ProgressBar::new(self.progress)
            .show_percentage()
            .animate(self.is_processing);
        ui.add(bar);
    }

    // [9] EXECUTE BUTTON
    fn draw_execute_button(&mut self, ui: &mut egui::Ui) {
        let (label, fill) = match self.mode {
            Mode::Encrypt => ("▶  ENCRYPT FILE", Palette::CYAN),
            Mode::Decrypt => ("▶  DECRYPT FILE", Palette::GREEN),
        };

        let btn = egui::Button::new(
            egui::RichText::new(label)
                .color(Palette::BG)
                .strong()
                .size(16.0),
        )
        .fill(if self.is_processing {
            Palette::DIM
        } else {
            fill
        })
        .min_size(egui::vec2(ui.available_width(), 42.0))
        .rounding(egui::Rounding::same(6.0));

        if ui.add_enabled(!self.is_processing, btn).clicked() {
            self.execute();
        }
    }

    // [10] FOOTER
    fn draw_footer(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.colored_label(
                Palette::AMBER,
                "⚠  LOSS OF PASSWORD = PERMANENT DATA LOSS",
            );
        });
    }
}
