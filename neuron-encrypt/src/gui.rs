// gui.rs — Clean Minimal Modern UI
// ZERO direct crypto calls. Crypto runs via spawned thread + mpsc channel.

use std::path::{Path, PathBuf};
use crossbeam_channel as mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use zeroize::Zeroizing;
use rand_core::{OsRng, RngCore};

use neuron_encrypt_core::crypto::{self, ProgressReporter, ThrottledReporter};
use neuron_encrypt_core::error::CryptoError;

// ═══════════════════════════════════════════════════════════════════════════════
// COLOR SYSTEM — PREMIUM MINIMAL PALETTE
// ═══════════════════════════════════════════════════════════════════════════════
struct Palette;
impl Palette {
    const BG:             egui::Color32 = egui::Color32::from_rgb(0x0A, 0x0A, 0x0A);
    const SURFACE:        egui::Color32 = egui::Color32::from_rgb(0x14, 0x14, 0x14);
    const BORDER:         egui::Color32 = egui::Color32::from_rgb(0x22, 0x22, 0x22);

    const PRIMARY:        egui::Color32 = egui::Color32::from_rgb(0x4F, 0x46, 0xE5);
    const SECONDARY:      egui::Color32 = egui::Color32::from_rgb(0x81, 0x8C, 0xFB);
    const TEXT_PRIMARY:   egui::Color32 = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
    const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0xD1, 0xD5, 0xDB);
    const TEXT_MUTED:     egui::Color32 = egui::Color32::from_rgb(0x6B, 0x72, 0x80);
    const SUCCESS:        egui::Color32 = egui::Color32::from_rgb(0x10, 0xB9, 0x81);
    const ERROR:          egui::Color32 = egui::Color32::from_rgb(0xF4, 0x3F, 0x5E);
    const WARNING:        egui::Color32 = egui::Color32::from_rgb(0xF5, 0x9E, 0x0B);
}

// ═══════════════════════════════════════════════════════════════════════════════
// APP FLOW STATE
// ═══════════════════════════════════════════════════════════════════════════════
#[derive(PartialEq, Clone, Copy)]
enum AppFlow {
    FileDrop,
    Configure,
    Processing,
    Success,
    Failure,
}

// ═══════════════════════════════════════════════════════════════════════════════
// BRIDGE: ProgressReporter -> mpsc channel
// ═══════════════════════════════════════════════════════════════════════════════
enum GuiMsg {
    Progress(f32, String),
    Done(String),
    Error(CryptoError), // FIX BUG-025: Use CryptoError directly
}

struct MpscReporter {
    tx: mpsc::Sender<GuiMsg>,
}

impl ProgressReporter for MpscReporter {
    fn report(&self, progress: f32, message: &str) {
        // FIX BUG-038: Use try_send for progress updates to avoid blocking
        let _ = self.tx.try_send(GuiMsg::Progress(progress, message.to_string()));
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
// FIX BUG-028: Add Clone and Copy derives
#[derive(PartialEq, Clone, Copy)]
enum Strength {
    None,
    Weak,
    Fair,
    Strong,
    Elite,
}

fn has_sequential_pattern(pw: &str) -> bool {
    let chars: Vec<char> = pw.chars().collect();
    if chars.len() < 4 { return false; }
    for window in chars.windows(4) {
        let diffs: Vec<i32> = window.windows(2).map(|w| (w[1] as i32) - (w[0] as i32)).collect();
        if diffs.iter().all(|&d| d == 1) || diffs.iter().all(|&d| d == -1) {
            return true;
        }
    }
    false
}

fn evaluate_strength(pw: &str) -> (Strength, f32) {
    if pw.is_empty() { return (Strength::None, 0.0); }

    let variety = [
        pw.chars().any(|c| c.is_ascii_lowercase()),
        pw.chars().any(|c| c.is_ascii_uppercase()),
        pw.chars().any(|c| c.is_ascii_digit()),
        pw.chars().any(|c| !c.is_alphanumeric()),
    ].iter().filter(|&&b| b).count();

    // FIX BUG-014: Enhanced strength check
    let unique_chars: std::collections::HashSet<char> = pw.chars().collect();
    let unique_ratio = unique_chars.len() as f32 / pw.len() as f32;
    let is_repetitive = unique_ratio < 0.3;
    let is_sequential = has_sequential_pattern(pw);

    let mut score = match (pw.len(), variety) {
        (0..=3, _) => 1,
        (4..=7, 0..=2) => 3,
        (4..=7, _) => 5,
        (8..=11, 0..=2) => 5,
        (8..=11, _) => 8,
        (_, _) => 10,
    };

    // Adjust score for repetitive/sequential patterns
    if is_repetitive || is_sequential {
        score = score.min(5);
    }

    match score {
        1..=2 => (Strength::Weak, 0.25),
        3..=5 => (Strength::Fair, 0.50),
        6..=8 => (Strength::Strong, 0.75),
        _ => (Strength::Elite, 1.0),
    }
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
    flow: AppFlow,
    selected_file: Option<PathBuf>,
    dest_path: Option<PathBuf>, // FIX BUG-008: Store destination path
    password: Zeroizing<String>,
    confirm_password: Zeroizing<String>, // FIX BUG-005: Password confirmation
    show_password: bool,
    crypto_rx: Option<mpsc::Receiver<GuiMsg>>,
    progress: f32,
    status: Option<StatusMessage>,
    spinner_index: usize,
    last_spinner_tick: Instant,
    last_clock_update: Instant,
    current_time: String,
    scramble_text: String,
}

/// FIX BUG-007: Case-insensitive VX2 check
fn is_vx2_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("vx2"))
        .unwrap_or(false)
}

impl NeuronEncryptApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            mode: Mode::Encrypt,
            flow: AppFlow::FileDrop,
            selected_file: None,
            dest_path: None,
            password: Zeroizing::new(String::new()),
            confirm_password: Zeroizing::new(String::new()),
            show_password: false,
            crypto_rx: None,
            progress: 0.0,
            status: None,
            spinner_index: 0,
            last_spinner_tick: Instant::now(),
            last_clock_update: Instant::now(),
            current_time: chrono::Local::now().format("%H:%M:%S").to_string(),
            scramble_text: "INITIALIZING...".to_string(),
        }
    }

    fn spinner_char(&self) -> char {
        let chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        chars[self.spinner_index % 10]
    }

    fn poll_crypto(&mut self) {
        let should_clear = if let Some(ref rx) = self.crypto_rx {
            let mut last_msg = None;
            while let Ok(msg) = rx.try_recv() {
                last_msg = Some(msg);
                if let Some(GuiMsg::Done(_)) | Some(GuiMsg::Error(_)) = last_msg {
                    break;
                }
            }

            if let Some(msg) = last_msg {
                match msg {
                    GuiMsg::Progress(p, text) => {
                        self.progress = p;
                        self.status = Some(StatusMessage { text, color: Palette::TEXT_SECONDARY });
                        false
                    }
                    GuiMsg::Done(text) => {
                        self.flow = AppFlow::Success;
                        self.status = Some(StatusMessage { text, color: Palette::SUCCESS });
                        true
                    }
                    GuiMsg::Error(err) => {
                        self.flow = AppFlow::Failure;
                        // FIX BUG-025: Handle structured errors
                        let err_text = match err {
                            CryptoError::DecryptionFailed => "ERROR: Decryption Failed. Wrong password?".to_string(),
                            CryptoError::InvalidMagic => "ERROR: Invalid File Format.".to_string(),
                            CryptoError::FileAlreadyExists(p) => format!("ERROR: File Already Exists: {}", p.display()),
                            CryptoError::NotAFile(_) => "ERROR: Not a regular file.".to_string(),
                            _ => format!("ERROR: {}", err),
                        };
                        self.status = Some(StatusMessage { text: err_text, color: Palette::ERROR });
                        true
                    }
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_clear {
            self.crypto_rx = None;
        }
    }

    fn update_clock_and_spinner(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if now.duration_since(self.last_clock_update) >= Duration::from_secs(1) {
            self.last_clock_update = now;
            self.current_time = chrono::Local::now().format("%H:%M:%S").to_string();
        }
        if now.duration_since(self.last_spinner_tick) >= Duration::from_millis(80) {
            self.last_spinner_tick = now;
            self.spinner_index = (self.spinner_index + 1) % 10;
            if self.flow == AppFlow::Processing {
                let mut rng = OsRng;
                let s: String = (0..16).map(|_| {
                    let v = rng.next_u32() % 16;
                    std::char::from_digit(v, 16).unwrap()
                }).collect();
                // FIX BUG-027: Safe UTF-8 slicing
                let prefix: String = s.chars().take(6).collect();
                let suffix: String = s.chars().skip(10).take(6).collect();
                self.scramble_text = format!("{}...{}", prefix, suffix);
            }
        }
        ctx.request_repaint_after(if self.flow == AppFlow::Processing { Duration::from_millis(16) } else { Duration::from_millis(500) });
    }

    fn execute(&mut self, ctx: &egui::Context) {
        let Some(ref file_path) = self.selected_file else { return; };

        // FIX BUG-037: Enforce minimum password length
        const MIN_PASSWORD_LEN: usize = 8;
        if self.password.len() < MIN_PASSWORD_LEN {
            self.status = Some(StatusMessage { text: format!("Passphrase must be at least {} characters.", MIN_PASSWORD_LEN), color: Palette::ERROR });
            return;
        }

        // FIX BUG-005: Validate password match
        if self.mode == Mode::Encrypt && *self.password != *self.confirm_password {
            self.status = Some(StatusMessage { text: "Passphrases do not match.".to_string(), color: Palette::ERROR });
            return;
        }

        // FIX BUG-041: Double encryption warning
        if self.mode == Mode::Encrypt && is_vx2_file(file_path) {
             self.status = Some(StatusMessage { text: "WARNING: You are encrypting an already protected .vx2 file.".to_string(), color: Palette::WARNING });
        }

        let mode = self.mode;
        let src = file_path.clone();

        // FIX BUG-031: Fix stripping suffix
        let default_name = file_path.file_name().unwrap_or_default().to_string_lossy();
        let dest_name = if mode == Mode::Encrypt {
            format!("{}{}", default_name, crypto::EXTENSION)
        } else {
            default_name.strip_suffix(crypto::EXTENSION).unwrap_or(&default_name).to_string()
        };

        let mut dialog = rfd::FileDialog::new()
            .set_directory(file_path.parent().unwrap_or(Path::new(".")))
            .set_file_name(&dest_name);

        if mode == Mode::Decrypt {
             dialog = dialog.add_filter("Decrypted File", &["*"]);
        } else {
             dialog = dialog.add_filter("VaultX Container", &["vx2"]);
        }

        let Some(dest) = dialog.save_file() else { return; };
        self.dest_path = Some(dest.clone());

        let password = self.password.clone();
        let (tx, rx) = mpsc::unbounded();
        self.crypto_rx = Some(rx);
        self.flow = AppFlow::Processing;
        self.progress = 0.0;

        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let binding = MpscReporter { tx: tx.clone() };
            let reporter = ThrottledReporter::new(&binding);
            let result = if mode == Mode::Encrypt {
                crypto::encrypt_file(&src, &dest, password.as_bytes(), &reporter)
            } else {
                crypto::decrypt_file(&src, &dest, password.as_bytes(), &reporter)
            };

            match result {
                Ok(_) => { let _ = tx.send(GuiMsg::Done("Operation complete".into())); }
                Err(e) => { let _ = tx.send(GuiMsg::Error(e)); }
            }
            ctx_clone.request_repaint();
        });
    }

    fn secure_wipe_session(&mut self) {
        self.password = Zeroizing::new(String::new());
        self.confirm_password = Zeroizing::new(String::new());
        self.selected_file = None;
        self.dest_path = None;
        self.status = None;
        self.flow = AppFlow::FileDrop;
    }
}

impl eframe::App for NeuronEncryptApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        // FIX BUG-017 & BUG-043: Handle file drops in multiple flows and multiple files
        if !dropped_files.is_empty() && (self.flow == AppFlow::FileDrop || self.flow == AppFlow::Configure) {
            if dropped_files.len() > 1 {
                self.status = Some(StatusMessage { text: "Only one file can be processed at a time.".to_string(), color: Palette::WARNING });
            }
            if let Some(path) = dropped_files[0].path.clone() {
                self.selected_file = Some(path.clone());
                self.flow = AppFlow::Configure;
                self.mode = if is_vx2_file(&path) { Mode::Decrypt } else { Mode::Encrypt };
            }
        }
        self.poll_crypto();
        self.update_clock_and_spinner(ctx);
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Palette::BG)).show(ctx, |ui| {
            #[cfg(not(target_os = "macos"))] self.draw_title_bar(ui);
            let content_top = if cfg!(target_os = "macos") { 0.0 } else { 40.0 };
            let content_rect = egui::Rect::from_min_max(egui::pos2(ui.max_rect().min.x, ui.max_rect().min.y + content_top), ui.max_rect().max);
            ui.allocate_ui_at_rect(content_rect, |ui| {
                let card_width = 560.0f32.min(ui.available_width() - 32.0);
                ui.add_space(32.0);
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - card_width) / 2.0);
                    ui.vertical(|ui| { ui.set_width(card_width); self.draw_card(ui, card_width, ctx); });
                });
                ui.add_space(20.0);
                self.draw_footer(ui);
            });
        });
    }
}

impl NeuronEncryptApp {
    #[cfg(not(target_os = "macos"))]
    fn draw_title_bar(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, Palette::SURFACE);
        ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - 1.0), rect.max), 0.0, Palette::BORDER);
        ui.painter().text(egui::pos2(rect.min.x + 16.0, rect.center().y), egui::Align2::LEFT_CENTER, "NEURON ENCRYPT", egui::FontId::new(14.0, egui::FontFamily::Proportional), Palette::TEXT_PRIMARY);
        let close_center = egui::pos2(rect.max.x - 24.0, rect.center().y);
        let close_resp = ui.interact(egui::Rect::from_center_size(close_center, egui::vec2(16.0, 16.0)), egui::Id::new("close_btn"), egui::Sense::click());
        ui.painter().circle_filled(close_center, 6.0, if close_resp.hovered() { Palette::ERROR } else { Palette::ERROR.gamma_multiply(0.6) });
        if close_resp.clicked() { ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close); }
        if ui.interact(rect, egui::Id::new("title_drag"), egui::Sense::drag()).drag_started() { ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag); }
    }

    fn draw_card(&mut self, ui: &mut egui::Ui, card_width: f32, ctx: &egui::Context) {
        match self.flow {
            AppFlow::FileDrop => self.draw_step_file_drop(ui, card_width),
            AppFlow::Configure => self.draw_step_configure(ui, card_width, ctx),
            AppFlow::Processing => self.draw_step_processing(ui, card_width),
            AppFlow::Success => self.draw_step_results(ui, card_width, true),
            AppFlow::Failure => self.draw_step_results(ui, card_width, false),
        }
    }

    fn draw_step_file_drop(&mut self, ui: &mut egui::Ui, _cw: f32) {
        egui::Frame::none().fill(Palette::SURFACE).stroke(egui::Stroke::new(1.0, Palette::BORDER)).rounding(16.0).inner_margin(egui::Margin::same(32.0)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("NEURON").font(egui::FontId::new(40.0, egui::FontFamily::Proportional)).color(Palette::PRIMARY).strong());
                ui.add_space(32.0);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 160.0), egui::Sense::click());
                ui.painter().rect_stroke(rect, 12.0, egui::Stroke::new(1.0, Palette::BORDER));
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "SELECT OR DROP FILE", egui::FontId::new(14.0, egui::FontFamily::Proportional), Palette::TEXT_MUTED);
                if ui.interact(rect, egui::Id::new("drop_zone"), egui::Sense::click()).clicked() {
                   if let Some(path) = rfd::FileDialog::new().pick_file() {
                       self.selected_file = Some(path.clone());
                       self.flow = AppFlow::Configure;
                       self.mode = if is_vx2_file(&path) { Mode::Decrypt } else { Mode::Encrypt };
                   }
                }
                ui.add_space(20.0);
            });
        });
    }

    fn draw_step_configure(&mut self, ui: &mut egui::Ui, _cw: f32, ctx: &egui::Context) {
        egui::Frame::none().fill(Palette::SURFACE).stroke(egui::Stroke::new(1.0, Palette::BORDER)).rounding(16.0).inner_margin(egui::Margin::same(32.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                // FIX BUG-018: Clear password on BACK
                if ui.add(egui::Button::new("← BACK").fill(Palette::SECONDARY.gamma_multiply(0.2))).clicked() {
                    self.password = Zeroizing::new(String::new());
                    self.confirm_password = Zeroizing::new(String::new());
                    self.flow = AppFlow::FileDrop;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.selectable_label(self.mode == Mode::Encrypt, "ENCRYPT").clicked() { self.mode = Mode::Encrypt; }
                    if ui.selectable_label(self.mode == Mode::Decrypt, "DECRYPT").clicked() { self.mode = Mode::Decrypt; }
                });
            });
            ui.add_space(24.0);
            if let Some(path) = &self.selected_file {
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                // FIX BUG-011: Safe UTF-8 truncation
                let display_name = if file_name.chars().count() > 42 {
                    let truncated: String = file_name.chars().take(42).collect();
                    format!("{}...", truncated)
                } else {
                    file_name.to_string()
                };
                ui.label(egui::RichText::new(display_name).font(egui::FontId::new(18.0, egui::FontFamily::Proportional)).color(Palette::TEXT_PRIMARY).strong());
            }
            ui.add_space(32.0);
            ui.label(egui::RichText::new("SET SECURITY PASSPHRASE").color(Palette::TEXT_SECONDARY).strong());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut *self.password)
                    .password(!self.show_password)
                    .hint_text("Passphrase...")
                    .desired_width(ui.available_width() - 40.0));

                // FIX BUG-006: Password show/hide toggle
                if ui.selectable_label(self.show_password, if self.show_password { "👁" } else { "🙈" }).clicked() {
                    self.show_password = !self.show_password;
                }
            });

            // FIX BUG-005: Confirm password field
            if self.mode == Mode::Encrypt {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("CONFIRM PASSPHRASE").color(Palette::TEXT_SECONDARY).strong());
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut *self.confirm_password)
                    .password(!self.show_password)
                    .hint_text("Confirm passphrase...")
                    .desired_width(f32::INFINITY));

                if !self.confirm_password.is_empty() && *self.password != *self.confirm_password {
                    ui.label(egui::RichText::new("⚠ Passphrases do not match").color(Palette::ERROR).font(egui::FontId::new(12.0, egui::FontFamily::Proportional)));
                }
            }

            let (strength, fraction) = evaluate_strength(&self.password);
            ui.add(egui::ProgressBar::new(fraction).fill(strength_color(&strength)));

            // FIX BUG-037: Enforce min length
            const MIN_PASSWORD_LEN: usize = 8;
            if !self.password.is_empty() && self.password.len() < MIN_PASSWORD_LEN {
                ui.label(egui::RichText::new(format!("⚠ Minimum {} characters required", MIN_PASSWORD_LEN)).color(Palette::WARNING).font(egui::FontId::new(12.0, egui::FontFamily::Proportional)));
            }

            ui.add_space(40.0);

            // FIX BUG-039: Dynamic button label
            let btn_label = if self.mode == Mode::Encrypt { "PROTECT FILE" } else { "DECRYPT FILE" };

            // Disable button if validation fails
            let can_proceed = self.password.len() >= MIN_PASSWORD_LEN && (self.mode == Mode::Decrypt || *self.password == *self.confirm_password);

            if ui.add_enabled(can_proceed, egui::Button::new(egui::RichText::new(btn_label).strong())
                .fill(Palette::PRIMARY)
                .rounding(12.0)
                .min_size(egui::vec2(ui.available_width(), 48.0))).clicked() {
                self.execute(ctx);
            }
        });
    }

    fn draw_step_processing(&mut self, ui: &mut egui::Ui, _cw: f32) {
        egui::Frame::none().fill(Palette::SURFACE).stroke(egui::Stroke::new(1.0, Palette::BORDER)).rounding(16.0).inner_margin(egui::Margin::same(32.0)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new(self.spinner_char().to_string()).font(egui::FontId::new(48.0, egui::FontFamily::Monospace)).color(Palette::PRIMARY));
                ui.add_space(16.0);
                ui.label(egui::RichText::new(&self.scramble_text).font(egui::FontId::new(14.0, egui::FontFamily::Monospace)).color(Palette::TEXT_MUTED));
                ui.add_space(32.0);
                ui.add(egui::ProgressBar::new(self.progress).show_percentage());
                ui.add_space(24.0);
                if let Some(ref status) = self.status { ui.label(egui::RichText::new(&status.text).color(status.color).font(egui::FontId::new(12.0, egui::FontFamily::Monospace))); }
            });
        });
    }

    fn draw_step_results(&mut self, ui: &mut egui::Ui, _cw: f32, success: bool) {
        egui::Frame::none().fill(Palette::SURFACE).stroke(egui::Stroke::new(1.0, Palette::BORDER)).rounding(16.0).inner_margin(egui::Margin::same(32.0)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                let title = if success { "Operation Successful" } else { "Operation Failed" };
                ui.label(egui::RichText::new(title).font(egui::FontId::new(24.0, egui::FontFamily::Proportional)).color(if success { Palette::SUCCESS } else { Palette::ERROR }).strong());

                if let Some(ref status) = self.status {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&status.text).color(Palette::TEXT_SECONDARY));
                }

                ui.add_space(24.0);
                if success {
                    // FIX BUG-008 & BUG-009: Correct folder opening
                    if let Some(path) = self.dest_path.as_ref().or(self.selected_file.as_ref()) {
                        if ui.button("OPEN FOLDER").clicked() {
                            if let Some(parent) = path.parent() {
                                #[cfg(target_os = "windows")]
                                let mut cmd = std::process::Command::new("explorer");
                                #[cfg(target_os = "macos")]
                                let mut cmd = std::process::Command::new("open");
                                #[cfg(not(any(target_os = "windows", target_os = "macos")))]
                                let mut cmd = std::process::Command::new("xdg-open");

                                let _ = cmd.arg(parent).spawn();
                            }
                        }
                    }
                }
                ui.add_space(24.0);
                if ui.add(egui::Button::new(egui::RichText::new("SECURE WIPE SESSION").strong()).fill(Palette::PRIMARY).rounding(12.0).min_size(egui::vec2(240.0, 48.0))).clicked() {
                    self.secure_wipe_session();
                }
            });
        });
    }

    fn draw_footer(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("NEURON · PRIVACY FIRST").color(Palette::TEXT_MUTED).font(egui::FontId::new(10.0, egui::FontFamily::Proportional)));
        });
    }
}
