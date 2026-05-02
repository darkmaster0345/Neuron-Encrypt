use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use crossbeam_channel as mpsc;
use eframe::egui::{
    self, vec2, Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke,
    ViewportCommand,
};
use neuron_encrypt_core::crypto::{self, ProgressReporter};
use neuron_encrypt_core::error::CryptoError;
use rand_core::{OsRng, RngCore};
use zeroize::Zeroizing;

struct Palette;
impl Palette {
    const BG: Color32 = Color32::from_rgb(0x08, 0x08, 0x08);
    const SURFACE_0: Color32 = Color32::from_rgb(0x0F, 0x0F, 0x0F);
    const SURFACE_1: Color32 = Color32::from_rgb(0x16, 0x16, 0x16);
    const SURFACE_2: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
    const BORDER_SUBTLE: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
    const BORDER_MED: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);
    const BORDER_STRONG: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);
    const ACCENT: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);
    const ACCENT_HOVER: Color32 = Color32::from_rgb(0x81, 0x8C, 0xF8);
    const ACCENT_MUTED: Color32 = Color32::from_rgba_premultiplied(99, 102, 241, 30);
    const ACCENT_DIM: Color32 = Color32::from_rgba_premultiplied(99, 102, 241, 15);
    const SUCCESS: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
    const SUCCESS_MUTED: Color32 = Color32::from_rgba_premultiplied(16, 185, 129, 30);
    const ERROR: Color32 = Color32::from_rgb(0xF4, 0x3F, 0x5E);
    const ERROR_MUTED: Color32 = Color32::from_rgba_premultiplied(244, 63, 94, 30);
    const WARNING: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
    const WARNING_MUTED: Color32 = Color32::from_rgba_premultiplied(245, 158, 11, 30);
    const TEXT_HI: Color32 = Color32::from_rgb(0xF8, 0xF8, 0xF8);
    const TEXT_MED: Color32 = Color32::from_rgb(0xA0, 0xA0, 0xA0);
    const TEXT_LO: Color32 = Color32::from_rgb(0x50, 0x50, 0x50);
    const TEXT_ACCENT: Color32 = Color32::from_rgb(0x81, 0x8C, 0xF8);
}

#[derive(PartialEq, Clone, Copy)]
enum AppFlow {
    FileDrop,
    Configure,
    Processing,
    Success,
    Failure,
}
#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Encrypt,
    Decrypt,
}
#[derive(PartialEq, Clone, Copy)]
enum Strength {
    None,
    Weak,
    Fair,
    Strong,
    Elite,
}

enum GuiMsg {
    Progress(f32, String),
    Done(String),
    Error(CryptoError),
}
struct MpscReporter {
    tx: mpsc::Sender<GuiMsg>,
}
impl ProgressReporter for MpscReporter {
    fn report(&self, progress: f32, message: &str) {
        let _ = self
            .tx
            .try_send(GuiMsg::Progress(progress, message.to_string()));
    }
}
#[derive(Clone)]
struct StatusMessage {
    text: String,
    color: Color32,
}

pub struct NeuronEncryptApp {
    mode: Mode,
    last_mode: Option<Mode>,
    flow: AppFlow,
    selected_file: Option<PathBuf>,
    dest_path: Option<PathBuf>,
    password: Zeroizing<String>,
    confirm_password: Zeroizing<String>,
    show_password: bool,
    crypto_rx: Option<mpsc::Receiver<GuiMsg>>,
    progress: f32,
    status: Option<StatusMessage>,
    spinner_index: usize,
    last_spinner_tick: Instant,
    scramble_text: String,
    reencrypt_confirmed: bool,
    stay_on_top: bool,
    display_strength_frac: f32,
    display_progress_frac: f32,
    anim_check_progress: f32,
    anim_error_progress: f32,
    cancel_flag: Arc<AtomicBool>,
}

fn is_vx2_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("vx2"))
        .unwrap_or(false)
}
fn constant_time_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut r = 0;
    for (x, y) in ab.iter().zip(bb.iter()) {
        r |= x ^ y;
    }
    r == 0
}
fn evaluate_strength(pw: &str) -> (Strength, f32) {
    if pw.is_empty() {
        return (Strength::None, 0.0);
    }

    let chars: Vec<char> = pw.chars().collect();
    let len = chars.len();

    let has_upper = chars.iter().any(|c| c.is_ascii_uppercase());
    let has_lower = chars.iter().any(|c| c.is_ascii_lowercase());
    let has_digit = chars.iter().any(|c| c.is_ascii_digit());
    let has_symbol = chars.iter().any(|c| !c.is_alphanumeric());

    let mut score: f32 = 0.0;

    if len >= 8 {
        score += 1.0;
    }
    if len >= 12 {
        score += 1.0;
    }
    if len >= 16 {
        score += 1.0;
    }
    if len >= 24 {
        score += 1.0;
    }

    if has_lower {
        score += 0.5;
    }
    if has_upper {
        score += 0.5;
    }
    if has_digit {
        score += 0.5;
    }
    if has_symbol {
        score += 0.5;
    }

    let mut sorted = chars.clone();
    sorted.sort_unstable();
    let mut unique_count = 1;
    for i in 1..sorted.len() {
        if sorted[i] != sorted[i - 1] {
            unique_count += 1;
        }
    }
    let unique_ratio = unique_count as f32 / len as f32;
    if unique_ratio < 0.5 {
        score -= 1.0;
    } else if unique_ratio > 0.75 {
        score += 0.5;
    }

    let mut max_run = 1;
    let mut current_run = 1;
    let mut max_seq_run = 1;
    let mut current_seq_run = 1;

    for i in 1..chars.len() {
        if chars[i] == chars[i - 1] {
            current_run += 1;
        } else {
            if current_run > max_run {
                max_run = current_run;
            }
            current_run = 1;
        }

        let prev = chars[i - 1] as i32;
        let curr = chars[i] as i32;
        if curr == prev + 1 || curr == prev - 1 {
            current_seq_run += 1;
        } else {
            if current_seq_run > max_seq_run {
                max_seq_run = current_seq_run;
            }
            current_seq_run = 1;
        }
    }
    if current_run > max_run {
        max_run = current_run;
    }
    if current_seq_run > max_seq_run {
        max_seq_run = current_seq_run;
    }

    if max_run >= 3 {
        score -= 0.5;
    }
    if max_seq_run >= 4 {
        score -= 0.5;
    }

    let lower = pw.to_ascii_lowercase();
    const COMMON_PATTERNS: [&str; 6] =
        ["password", "qwerty", "letmein", "12345", "123456", "admin"];
    if COMMON_PATTERNS.iter().any(|p| lower.contains(p)) {
        score -= 1.5;
    }

    score = score.clamp(0.0, 6.0);

    let strength = if score < 2.0 {
        Strength::Weak
    } else if score < 3.5 {
        Strength::Fair
    } else if score < 5.0 {
        Strength::Strong
    } else {
        Strength::Elite
    };

    (strength, score / 6.0)
}
fn strength_color(s: Strength) -> Color32 {
    match s {
        Strength::None => Palette::BORDER_MED,
        Strength::Weak => Palette::ERROR,
        Strength::Fair => Palette::WARNING,
        Strength::Strong => Palette::ACCENT,
        Strength::Elite => Palette::SUCCESS,
    }
}
fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

impl NeuronEncryptApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            mode: Mode::Encrypt,
            last_mode: None,
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
            scramble_text: "0x0000...0000".to_string(),
            reencrypt_confirmed: false,
            stay_on_top: false,
            display_strength_frac: 0.0,
            display_progress_frac: 0.0,
            anim_check_progress: 0.0,
            anim_error_progress: 0.0,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
    fn secure_wipe_session(&mut self) {
        self.password = Zeroizing::new(String::new());
        self.confirm_password = Zeroizing::new(String::new());
        self.selected_file = None;
        self.dest_path = None;
        self.status = None;
        self.flow = AppFlow::FileDrop;
        self.reencrypt_confirmed = false;
        self.display_strength_frac = 0.0;
        self.display_progress_frac = 0.0;
        self.anim_check_progress = 0.0;
        self.anim_error_progress = 0.0;
        self.cancel_flag = Arc::new(AtomicBool::new(false));
    }
    fn poll_crypto(&mut self) {
        let mut clear = false;
        if let Some(rx) = &self.crypto_rx {
            let mut last = None;
            while let Ok(msg) = rx.try_recv() {
                last = Some(msg);
            }
            if let Some(msg) = last {
                match msg {
                    GuiMsg::Progress(p, t) => {
                        self.progress = p;
                        self.status = Some(StatusMessage {
                            text: t,
                            color: Palette::TEXT_LO,
                        });
                    }
                    GuiMsg::Done(t) => {
                        if !self.cancel_flag.load(Ordering::SeqCst) {
                            self.flow = AppFlow::Success;
                            self.status = Some(StatusMessage {
                                text: t,
                                color: Palette::SUCCESS,
                            });
                            self.anim_check_progress = 0.0;
                        }
                        clear = true;
                    }
                    GuiMsg::Error(e) => {
                        self.flow = AppFlow::Failure;
                        self.status = Some(StatusMessage {
                            text: format!("{}", e),
                            color: Palette::ERROR,
                        });
                        self.anim_error_progress = 0.0;
                        clear = true;
                    }
                }
            }
        }
        if clear {
            self.crypto_rx = None;
        }
    }
    fn execute(&mut self, ctx: &egui::Context) {
        let Some(file_path) = self.selected_file.clone() else {
            self.status = Some(StatusMessage {
                text: "Select a file first.".to_string(),
                color: Palette::WARNING,
            });
            return;
        };

        if self.password.chars().count() < crypto::MIN_PASSWORD_LEN {
            self.status = Some(StatusMessage {
                text: format!(
                    "Passphrase must be at least {} characters.",
                    crypto::MIN_PASSWORD_LEN
                ),
                color: Palette::WARNING,
            });
            return;
        }

        if self.mode == Mode::Encrypt && !constant_time_eq(&self.password, &self.confirm_password) {
            self.status = Some(StatusMessage {
                text: "Passphrase confirmation does not match.".to_string(),
                color: Palette::WARNING,
            });
            return;
        }

        if self.mode == Mode::Encrypt && is_vx2_file(&file_path) && !self.reencrypt_confirmed {
            self.status = Some(StatusMessage {
                text: "Confirm re-encryption of an existing .vx2 file.".to_string(),
                color: Palette::WARNING,
            });
            return;
        }

        self.status = None;
        self.last_mode = Some(self.mode);
        self.cancel_flag.store(false, Ordering::SeqCst);

        let name = file_path.file_name().unwrap_or_default().to_string_lossy();
        let dest_name = if self.mode == Mode::Encrypt {
            format!("{}{}", name, crypto::EXTENSION)
        } else if name.to_lowercase().ends_with(crypto::EXTENSION) {
            name[..name.len() - crypto::EXTENSION.len()].to_string()
        } else {
            name.to_string()
        };

        let Some(dest) = rfd::FileDialog::new()
            .set_directory(file_path.parent().unwrap_or(Path::new(".")))
            .set_file_name(&dest_name)
            .save_file()
        else {
            return;
        };

        self.dest_path = Some(dest.clone());
        let (tx, rx) = mpsc::unbounded();
        self.crypto_rx = Some(rx);
        self.flow = AppFlow::Processing;
        self.progress = 0.0;
        self.display_progress_frac = 0.0;

        let password = self.password.clone();
        let mode = self.mode;
        let cancel = Arc::clone(&self.cancel_flag);
        let ctxc = ctx.clone();

        std::thread::spawn(move || {
            let reporter = MpscReporter { tx: tx.clone() };
            let result = if mode == Mode::Encrypt {
                crypto::encrypt_file(&file_path, &dest, password.as_bytes(), &reporter)
            } else {
                crypto::decrypt_file(&file_path, &dest, password.as_bytes(), &reporter)
            };

            if cancel.load(Ordering::SeqCst) {
                let _ = tx.try_send(GuiMsg::Done("Operation cancelled".to_string()));
            } else {
                match result {
                    Ok(_) => {
                        let _ = tx.try_send(GuiMsg::Done("Operation complete".to_string()));
                    }
                    Err(e) => {
                        let _ = tx.try_send(GuiMsg::Error(e));
                    }
                }
            }

            ctxc.request_repaint();
        });
    }
}

impl eframe::App for NeuronEncryptApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() && matches!(self.flow, AppFlow::FileDrop | AppFlow::Configure) {
            if dropped.len() > 1 {
                self.status = Some(StatusMessage {
                    text: "Only one file can be processed at a time.".to_string(),
                    color: Palette::WARNING,
                });
            }
            if let Some(path) = dropped[0].path.clone() {
                self.selected_file = Some(path.clone());
                self.flow = AppFlow::Configure;
                self.mode = if is_vx2_file(&path) {
                    Mode::Decrypt
                } else {
                    Mode::Encrypt
                };
                self.reencrypt_confirmed = false;
            }
        }
        self.poll_crypto();
        if self.flow == AppFlow::Processing {
            if Instant::now().duration_since(self.last_spinner_tick) >= Duration::from_millis(80) {
                self.last_spinner_tick = Instant::now();
                self.spinner_index = (self.spinner_index + 1) % 10;
                let mut rng = OsRng;
                let s: String = (0..32)
                    .map(|_| std::char::from_digit(rng.next_u32() % 16, 16).unwrap_or('0'))
                    .collect();
                self.scramble_text = format!("0x{}…{}", &s[0..12], &s[20..32]);
            }
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        let (_, target) = evaluate_strength(&self.password);
        self.display_strength_frac += (target - self.display_strength_frac) * 0.15;
        self.display_progress_frac += (self.progress - self.display_progress_frac) * 0.15;
        if (target - self.display_strength_frac).abs() > 0.002
            || (self.progress - self.display_progress_frac).abs() > 0.002
        {
            ctx.request_repaint_after(Duration::from_millis(32));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Palette::BG))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                ui.painter()
                    .rect_stroke(full, 0.0, Stroke::new(1.0, Palette::BORDER_MED));
                #[cfg(not(target_os = "macos"))]
                self.draw_title_bar(ui);
                let y0 = if cfg!(target_os = "macos") { 0.0 } else { 36.0 };
                let content = Rect::from_min_max(Pos2::new(full.min.x, full.min.y + y0), full.max);
                ui.allocate_ui_at_rect(content, |ui| {
                    ui.add_space(24.0);
                    let x = (ui.available_width() - 520.0) / 2.0;
                    ui.horizontal(|ui| {
                        ui.add_space(x.max(0.0));
                        egui::Frame::none()
                            .fill(Palette::SURFACE_0)
                            .stroke(Stroke::new(1.0, Palette::BORDER_MED))
                            .rounding(12.0)
                            .inner_margin(28.0)
                            .show(ui, |ui| {
                                ui.set_width(464.0);
                                match self.flow {
                                    AppFlow::FileDrop => self.draw_file_drop(ui),
                                    AppFlow::Configure => self.draw_configure(ui, ctx),
                                    AppFlow::Processing => self.draw_processing(ui),
                                    AppFlow::Success => self.draw_result(ui, true),
                                    AppFlow::Failure => self.draw_result(ui, false),
                                };
                            });
                    });
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "AES-256-GCM-SIV · Argon2id · HKDF-SHA512 · v{}",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .font(FontId::new(11.0, FontFamily::Monospace))
                            .color(Palette::TEXT_LO),
                        )
                    });
                });
            });
    }
}

impl NeuronEncryptApp {
    /* compact ui funcs omitted for brevity in reasoning */
    #[cfg(not(target_os = "macos"))]
    fn draw_title_bar(&mut self, ui: &mut egui::Ui) {
        let (r, drag) =
            ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click_and_drag());
        let p = ui.painter_at(r);
        p.rect_filled(r, 0.0, Palette::BG);
        let srect = Rect::from_min_size(
            Pos2::new(r.min.x + 10.0, r.center().y - 9.0),
            vec2(18.0, 18.0),
        );
        draw_shield(&p, srect, Palette::ACCENT);
        p.text(
            Pos2::new(srect.max.x + 8.0, r.center().y),
            Align2::LEFT_CENTER,
            "NEURON ENCRYPT",
            FontId::new(20.0, FontFamily::Monospace),
            Palette::TEXT_HI,
        );
        p.line_segment(
            [
                Pos2::new(r.min.x, r.max.y - 1.0),
                Pos2::new(r.max.x, r.max.y - 1.0),
            ],
            Stroke::new(1.0, Palette::BORDER_SUBTLE),
        );
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
    }
    fn draw_file_drop(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Neuron Encrypt")
                .font(FontId::new(20.0, FontFamily::Monospace))
                .color(Palette::TEXT_HI),
        );
        let (rect, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), 144.0), Sense::click());
        let p = ui.painter_at(rect);
        let hover = resp.hovered();
        p.rect_filled(
            rect,
            8.0,
            if hover {
                Palette::ACCENT_MUTED
            } else {
                Palette::ACCENT_DIM
            },
        );
        p.rect_stroke(
            rect,
            8.0,
            Stroke::new(
                1.0,
                if hover {
                    Palette::BORDER_STRONG
                } else {
                    Palette::BORDER_MED
                },
            ),
        );
        draw_upload_arrow(&p, rect.center_top() + vec2(0.0, 42.0), Palette::ACCENT);
        p.text(
            rect.center() + vec2(0.0, 8.0),
            Align2::CENTER_CENTER,
            "Drop file here",
            FontId::new(13.0, FontFamily::Monospace),
            Palette::TEXT_MED,
        );
        if resp.clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                self.selected_file = Some(path.clone());
                self.flow = AppFlow::Configure;
                self.mode = if is_vx2_file(&path) {
                    Mode::Decrypt
                } else {
                    Mode::Encrypt
                };
            }
        }
    }
    fn draw_configure(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let title = if self.mode == Mode::Encrypt {
            "Encrypt file"
        } else {
            "Decrypt file"
        };
        ui.label(
            egui::RichText::new(title)
                .font(FontId::new(20.0, FontFamily::Monospace))
                .color(Palette::TEXT_HI),
        );

        if let Some(path) = &self.selected_file {
            ui.label(egui::RichText::new(path.to_string_lossy()).color(Palette::TEXT_MED));
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Passphrase").color(Palette::TEXT_MED));
        ui.add(egui::TextEdit::singleline(&mut *self.password).password(!self.show_password));

        if self.mode == Mode::Encrypt {
            ui.label(egui::RichText::new("Confirm passphrase").color(Palette::TEXT_MED));
            ui.add(
                egui::TextEdit::singleline(&mut *self.confirm_password)
                    .password(!self.show_password),
            );
        }

        ui.checkbox(&mut self.show_password, "Show passphrase");

        let (strength, _) = evaluate_strength(&self.password);
        ui.label(
            egui::RichText::new(format!(
                "Strength: {:?}",
                match strength {
                    Strength::None => "None",
                    Strength::Weak => "Weak",
                    Strength::Fair => "Fair",
                    Strength::Strong => "Strong",
                    Strength::Elite => "Elite",
                }
            ))
            .color(strength_color(strength)),
        );

        if self.mode == Mode::Encrypt && self.selected_file.as_ref().is_some_and(|p| is_vx2_file(p))
        {
            ui.checkbox(
                &mut self.reencrypt_confirmed,
                "I understand this will re-encrypt a .vx2 file",
            );
        }

        if let Some(status) = &self.status {
            ui.label(egui::RichText::new(&status.text).color(status.color));
        }

        if ui.button(title).clicked() {
            self.execute(ctx);
        }
    }
    fn draw_processing(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Encrypting…").color(Palette::TEXT_HI));
    }
    fn draw_result(&mut self, ui: &mut egui::Ui, success: bool) {
        ui.label(if success { "Done" } else { "Failed" });
        if ui.button("New file").clicked() {
            self.secure_wipe_session();
        }
    }
}

fn draw_shield(p: &Painter, r: Rect, c: Color32) {
    let pts = [
        (0.5, 0.0),
        (1.0, 0.22),
        (1.0, 0.6),
        (0.5, 1.0),
        (0.0, 0.6),
        (0.0, 0.22),
    ]
    .iter()
    .map(|(x, y)| {
        Pos2::new(
            r.min.x + r.width() * (*x as f32),
            r.min.y + r.height() * (*y as f32),
        )
    })
    .collect();
    p.add(Shape::convex_polygon(pts, c, Stroke::NONE));
}
fn draw_upload_arrow(p: &Painter, c: Pos2, color: Color32) {
    p.line_segment(
        [c + vec2(0.0, 10.0), c + vec2(0.0, -8.0)],
        Stroke::new(1.5, color),
    );
    p.line_segment(
        [c + vec2(-6.0, -2.0), c + vec2(0.0, -8.0)],
        Stroke::new(1.5, color),
    );
    p.line_segment(
        [c + vec2(6.0, -2.0), c + vec2(0.0, -8.0)],
        Stroke::new(1.5, color),
    );
}
