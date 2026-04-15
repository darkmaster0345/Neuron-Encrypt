// gui.rs — Clean Minimal Modern UI
// ZERO direct crypto calls. Crypto runs via spawned thread + mpsc channel.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use zeroize::Zeroizing;
use rand::{Rng, thread_rng};

use neuron_encrypt_core::crypto::{self, ProgressReporter, ThrottledReporter};

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
    if pw.is_empty() { return (Strength::None, 0.0); }
    let variety = [
        pw.chars().any(|c| c.is_ascii_lowercase()),
        pw.chars().any(|c| c.is_ascii_uppercase()),
        pw.chars().any(|c| c.is_ascii_digit()),
        pw.chars().any(|c| !c.is_alphanumeric()),
    ].iter().filter(|&&b| b).count();

    let score = match (pw.len(), variety) {
        (0..=3, _) => 1,
        (4..=7, 0..=2) => 3,
        (4..=7, _) => 5,
        (8..=11, 0..=2) => 5,
        (8..=11, _) => 8,
        (_, _) => 10,
    };

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
    password: Zeroizing<String>,
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

impl NeuronEncryptApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let now = Instant::now();
        Self {
            mode: Mode::Encrypt,
            flow: AppFlow::FileDrop,
            selected_file: None,
            password: Zeroizing::new(String::new()),
            show_password: false,
            crypto_rx: None,
            progress: 0.0,
            status: None,
            spinner_index: 0,
            last_spinner_tick: now,
            last_clock_update: now,
            current_time: chrono::Local::now().format("%H:%M:%S").to_string(),
            scramble_text: String::new(),
        }
    }

    fn set_status(&mut self, msg: &str, color: egui::Color32) {
        self.status = Some(StatusMessage { text: msg.to_string(), color });
    }

    fn spinner_char(&self) -> char {
        const FRAMES: &[char] = &['\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}', '\u{2807}', '\u{280F}'];
        FRAMES[self.spinner_index % FRAMES.len()]
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
                let mut rng = thread_rng();
                let s: String = (0..16).map(|_| {
                    let v = rng.gen_range(0..16);
                    std::char::from_digit(v, 16).unwrap()
                }).collect();
                self.scramble_text = format!("{}...{}", &s[..6], &s[10..]);
            }
        }
        ctx.request_repaint_after(if self.flow == AppFlow::Processing { Duration::from_millis(16) } else { Duration::from_millis(500) });
    }

    fn execute(&mut self, ctx: &egui::Context) {
        let Some(ref file_path) = self.selected_file else { return; };
        if self.password.is_empty() { return; }
        let path = file_path.clone();
        let dest_path = if self.mode == Mode::Encrypt {
            rfd::FileDialog::new()
                .set_title("Save Encrypted File")
                .set_file_name(format!("{}.vx2", path.file_name().unwrap_or_default().to_string_lossy()))
                .add_filter("VaultX", &["vx2"]).save_file()
        } else {
            let default_name = path.to_string_lossy().strip_suffix(".vx2").unwrap_or(&path.to_string_lossy()).to_string();
            rfd::FileDialog::new()
                .set_title("Save Decrypted File")
                .set_file_name(PathBuf::from(default_name).file_name().unwrap_or_default().to_string_lossy().into_owned()).save_file()
        };
        if let Some(dest) = dest_path {
            let (tx, rx) = mpsc::channel();
            self.crypto_rx = Some(rx);
            self.flow = AppFlow::Processing;
            self.progress = 0.0;
            let mut pw_bytes = Zeroizing::new(vec![0u8; self.password.len()]);
            pw_bytes.copy_from_slice(self.password.as_bytes());
            let mode = self.mode;
            let ctx_clone = ctx.clone();
            std::thread::spawn(move || {
                let mpsc_reporter = MpscReporter { tx: tx.clone() };
                let reporter = ThrottledReporter::new(&mpsc_reporter);
                let res = match mode {
                    Mode::Encrypt => crypto::encrypt_file(&path, &dest, &pw_bytes, &reporter),
                    Mode::Decrypt => crypto::decrypt_file(&path, &dest, &pw_bytes, &reporter),
                };
                match res {
                    Ok(_) => { let _ = tx.send(GuiMsg::Done("Operation complete".into())); }
                    Err(e) => { let _ = tx.send(GuiMsg::Error(e.to_string())); }
                }
                ctx_clone.request_repaint();
            });
        }
    }


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
                    self.set_status(&text, Palette::PRIMARY);
                }
                GuiMsg::Done(text) => {
                    self.progress = 1.0;
                    self.set_status(&text, Palette::SUCCESS);
                    self.flow = AppFlow::Success;
                    self.crypto_rx = None;
                }
                GuiMsg::Error(text) => {
                    self.progress = 0.0;
                    self.set_status(&text, Palette::ERROR);
                    self.flow = AppFlow::Failure;
                    self.crypto_rx = None;
                }
            }
        }
    }

    fn secure_wipe_session(&mut self) {
        self.password = Zeroizing::new(String::new());
        self.selected_file = None;
        self.progress = 0.0;
        self.status = None;
        self.flow = AppFlow::FileDrop;
    }
}

impl eframe::App for NeuronEncryptApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() && self.flow == AppFlow::FileDrop {
            if let Some(path) = dropped_files[0].path.clone() {
                self.selected_file = Some(path.clone());
                self.flow = AppFlow::Configure;
                self.mode = if path.extension().is_some_and(|e| e == "vx2") { Mode::Decrypt } else { Mode::Encrypt };
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
                       self.mode = if path.extension().is_some_and(|e| e == "vx2") { Mode::Decrypt } else { Mode::Encrypt };
                   }
                }
                ui.add_space(20.0);
            });
        });
    }

    fn draw_step_configure(&mut self, ui: &mut egui::Ui, _cw: f32, ctx: &egui::Context) {
        egui::Frame::none().fill(Palette::SURFACE).stroke(egui::Stroke::new(1.0, Palette::BORDER)).rounding(16.0).inner_margin(egui::Margin::same(32.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("← BACK").fill(Palette::SECONDARY.gamma_multiply(0.2))).clicked() { self.flow = AppFlow::FileDrop; }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.selectable_label(self.mode == Mode::Encrypt, "ENCRYPT").clicked() { self.mode = Mode::Encrypt; }
                    if ui.selectable_label(self.mode == Mode::Decrypt, "DECRYPT").clicked() { self.mode = Mode::Decrypt; }
                });
            });
            ui.add_space(24.0);
            if let Some(path) = &self.selected_file {
                ui.label(egui::RichText::new(path.file_name().unwrap_or_default().to_string_lossy()).font(egui::FontId::new(18.0, egui::FontFamily::Proportional)).color(Palette::TEXT_PRIMARY).strong());
            }
            ui.add_space(32.0);
            ui.label(egui::RichText::new("SET SECURITY PASSPHRASE").color(Palette::TEXT_SECONDARY).strong());
            ui.add_space(8.0);
            ui.add(egui::TextEdit::singleline(&mut *self.password).password(!self.show_password).hint_text("Passphrase...").desired_width(f32::INFINITY));
            let (strength, fraction) = evaluate_strength(&self.password);
            ui.add(egui::ProgressBar::new(fraction).fill(strength_color(&strength)));
            ui.add_space(40.0);
            if ui.add_sized(egui::vec2(ui.available_width(), 48.0), egui::Button::new("PROTECT FILE").fill(Palette::PRIMARY)).clicked() {
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
                if !success {
                    if let Some(ref status) = self.status {
                        let err_msg = if status.text.contains("Access is denied") { "ERROR: Access Denied. File in use?" } else if status.text.contains("not a VAULTX02") { "ERROR: Invalid File Format." } else { "ERROR: Decryption Failed. Wrong password?" };
                        ui.label(egui::RichText::new(err_msg).color(Palette::TEXT_SECONDARY));
                    }
                }
                ui.add_space(24.0);
                if success {
                    if let Some(ref path) = self.selected_file {
                        if ui.button("OPEN FOLDER").clicked() {
                            if let Some(parent) = path.parent() {
                                let mut cmd = if cfg!(target_os = "windows") { std::process::Command::new("explorer") } else { std::process::Command::new("xdg-open") };
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
