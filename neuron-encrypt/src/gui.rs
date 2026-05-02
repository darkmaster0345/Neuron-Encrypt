use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use crossbeam_channel as mpsc;
use eframe::egui::{
    self, vec2, Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, ViewportCommand,
};
use neuron_encrypt_core::crypto::{self, ProgressReporter};
use neuron_encrypt_core::error::CryptoError;
use rand_core::{OsRng, RngCore};
use zeroize::Zeroizing;

struct Palette;
impl Palette {
    const BG: Color32 = Color32::from_rgb(0x08, 0x08, 0x08);
    const SURFACE: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);
    const SURFACE_HI: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
    const BORDER: Color32 = Color32::from_rgb(0x28, 0x28, 0x28);
    const BORDER_FOCUS: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);
    const ACCENT: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);
    const ACCENT_DIM: Color32 = Color32::from_rgba_unmultiplied(99, 102, 241, 18);
    const TEXT_HI: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
    const TEXT_MED: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x9A);
    const TEXT_LO: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x4A);
    const SUCCESS: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
    const ERROR: Color32 = Color32::from_rgb(0xF4, 0x3F, 0x5E);
    const WARNING: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);

    const SURFACE_0: Color32 = Color32::from_rgb(0x0F, 0x0F, 0x0F);
    const SURFACE_1: Color32 = Color32::from_rgb(0x16, 0x16, 0x16);
    const SURFACE_2: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
    const BORDER_SUBTLE: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
    const BORDER_MED: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);
    const BORDER_STRONG: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);
    const ACCENT_HOVER: Color32 = Color32::from_rgb(0x81, 0x8C, 0xF8);
    const ACCENT_MUTED: Color32 = Color32::from_rgba_unmultiplied(99, 102, 241, 30);
    const SUCCESS_MUTED: Color32 = Color32::from_rgba_unmultiplied(16, 185, 129, 30);
    const ERROR_MUTED: Color32 = Color32::from_rgba_unmultiplied(244, 63, 94, 30);
    const WARNING_MUTED: Color32 = Color32::from_rgba_unmultiplied(245, 158, 11, 30);
    const TEXT_ACCENT: Color32 = Color32::from_rgb(0x81, 0x8C, 0xF8);
}

#[derive(PartialEq, Clone, Copy)]
enum AppFlow { FileDrop, Configure, Processing, Success, Failure }
#[derive(PartialEq, Clone, Copy)]
enum Mode { Encrypt, Decrypt }
#[derive(PartialEq, Clone, Copy)]
enum Strength { None, Weak, Fair, Strong, Elite }

enum GuiMsg { Progress(f32, String), Done(String), Error(CryptoError) }
struct MpscReporter { tx: mpsc::Sender<GuiMsg> }
impl ProgressReporter for MpscReporter {
    fn report(&self, progress: f32, message: &str) { let _ = self.tx.try_send(GuiMsg::Progress(progress, message.to_owned())); }
}

pub struct NeuronEncryptApp {
    mode: Mode,
    flow: AppFlow,
    selected_file: Option<PathBuf>,
    dest_path: Option<PathBuf>,
    password: Zeroizing<String>,
    confirm_password: Zeroizing<String>,
    show_password: bool,
    crypto_rx: Option<mpsc::Receiver<GuiMsg>>,
    progress: f32,
    status: Option<String>,
    spinner_index: usize,
    last_spinner_tick: Instant,
    scramble_text: String,
    reencrypt_confirmed: bool,
    stay_on_top: bool,
    strength_frac: f32,
    prog_frac: f32,
    check_anim: f32,
    cancel_flag: Arc<AtomicBool>,
}

fn is_vx2_file(path: &Path) -> bool { path.extension().and_then(|e| e.to_str()).map(|s| s.eq_ignore_ascii_case("vx2")).unwrap_or(false) }
fn constant_time_eq(a: &str, b: &str) -> bool { let (ab, bb) = (a.as_bytes(), b.as_bytes()); if ab.len() != bb.len() { return false; } let mut r = 0; for (x,y) in ab.iter().zip(bb.iter()) { r |= x ^ y; } r == 0 }
fn truncate_chars(s: &str, n: usize) -> String { let mut out = s.chars().take(n).collect::<String>(); if s.chars().count() > n { out.push('…'); } out }
fn format_size(b: u64) -> String { if b < 1024 { format!("{} B", b) } else if b < 1024*1024 { format!("{:.1} KB", b as f64/1024.0) } else if b < 1024*1024*1024 { format!("{:.1} MB", b as f64/1024.0/1024.0) } else { format!("{:.1} GB", b as f64/1024.0/1024.0/1024.0) } }
fn strength_color(s: Strength) -> Color32 { match s { Strength::None => Palette::BORDER, Strength::Weak => Palette::ERROR, Strength::Fair => Palette::WARNING, Strength::Strong => Palette::ACCENT, Strength::Elite => Palette::SUCCESS } }
fn eval_strength(pw: &str) -> (Strength, f32, &'static str) {
    if pw.is_empty() { return (Strength::None, 0.0, "None"); }
    let mut score = 0.0;
    let len = pw.chars().count();
    if len >= 8 { score += 1.0; } if len >= 12 { score += 1.0; } if len >= 16 { score += 1.0; }
    if pw.chars().any(|c| c.is_ascii_uppercase()) { score += 1.0; }
    if pw.chars().any(|c| c.is_ascii_digit()) { score += 1.0; }
    if pw.chars().any(|c| !c.is_alphanumeric()) { score += 1.0; }
    score = score.clamp(0.0, 6.0);
    if score < 2.0 { (Strength::Weak, score/6.0, "Weak") } else if score < 3.5 { (Strength::Fair, score/6.0, "Fair") } else if score < 5.0 { (Strength::Strong, score/6.0, "Strong") } else { (Strength::Elite, score/6.0, "Elite") }
}

impl NeuronEncryptApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self { Self { mode: Mode::Encrypt, flow: AppFlow::FileDrop, selected_file: None, dest_path: None, password: Zeroizing::new(String::new()), confirm_password: Zeroizing::new(String::new()), show_password: false, crypto_rx: None, progress: 0.0, status: None, spinner_index: 0, last_spinner_tick: Instant::now(), scramble_text: String::from("0x0000...0000"), reencrypt_confirmed: false, stay_on_top: false, strength_frac: 0.0, prog_frac: 0.0, check_anim: 0.0, cancel_flag: Arc::new(AtomicBool::new(false)) } }
    fn secure_wipe_session(&mut self) { self.password = Zeroizing::new(String::new()); self.confirm_password = Zeroizing::new(String::new()); self.selected_file = None; self.dest_path = None; self.status = None; self.flow = AppFlow::FileDrop; self.reencrypt_confirmed = false; self.strength_frac = 0.0; self.prog_frac = 0.0; self.check_anim = 0.0; self.cancel_flag.store(false, Ordering::SeqCst); }
    fn execute(&mut self, ctx: &egui::Context) {
        let Some(file_path) = self.selected_file.clone() else { return; };
        if self.password.chars().count() < crypto::MIN_PASSWORD_LEN { self.status = Some(format!("Passphrase must be at least {} characters.", crypto::MIN_PASSWORD_LEN)); return; }
        if self.mode == Mode::Encrypt && !constant_time_eq(&self.password, &self.confirm_password) { self.status = Some("Passphrases don't match".to_owned()); return; }
        if self.mode == Mode::Encrypt && is_vx2_file(&file_path) && !self.reencrypt_confirmed { self.status = Some("Re-encrypt acknowledgement required.".to_owned()); return; }
        let name = file_path.file_name().unwrap_or_default().to_string_lossy();
        let dst_name = if self.mode == Mode::Encrypt { format!("{}{}", name, crypto::EXTENSION) } else if name.to_lowercase().ends_with(crypto::EXTENSION) { name[..name.len()-crypto::EXTENSION.len()].to_owned() } else { name.to_string() };
        let Some(dest) = rfd::FileDialog::new().set_directory(file_path.parent().unwrap_or(Path::new("."))).set_file_name(&dst_name).save_file() else { return; };
        self.dest_path = Some(dest.clone()); self.progress = 0.0; self.prog_frac = 0.0; self.cancel_flag.store(false, Ordering::SeqCst);
        let (tx, rx) = mpsc::unbounded(); self.crypto_rx = Some(rx); self.flow = AppFlow::Processing;
        let password = self.password.clone(); let mode = self.mode; let cancel = Arc::clone(&self.cancel_flag); let ctxc = ctx.clone();
        std::thread::spawn(move || {
            let reporter = MpscReporter { tx: tx.clone() };
            let result = if mode == Mode::Encrypt { crypto::encrypt_file(&file_path, &dest, password.as_bytes(), &reporter) } else { crypto::decrypt_file(&file_path, &dest, password.as_bytes(), &reporter) };
            match result { Ok(_) => { let _ = tx.try_send(GuiMsg::Done("Operation complete".to_owned())); }, Err(e) => { let _ = tx.try_send(GuiMsg::Error(e)); } }
            if cancel.load(Ordering::SeqCst) { let _ = tx.try_send(GuiMsg::Done("Cancelled".to_owned())); }
            ctxc.request_repaint();
        });
    }
}

impl eframe::App for NeuronEncryptApp { fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    if let Some(path) = dropped.first().and_then(|f| f.path.clone()) { self.selected_file = Some(path.clone()); self.flow = AppFlow::Configure; self.mode = if is_vx2_file(&path) { Mode::Decrypt } else { Mode::Encrypt }; }
    if let Some(rx) = &self.crypto_rx { while let Ok(msg) = rx.try_recv() { match msg { GuiMsg::Progress(p,t) => { self.progress = p; self.status = Some(truncate_chars(&t, 50)); }, GuiMsg::Done(_) => { if !self.cancel_flag.load(Ordering::SeqCst) { self.flow = AppFlow::Success; self.check_anim = 0.0; } else { self.secure_wipe_session(); } self.crypto_rx = None; }, GuiMsg::Error(e) => { if !self.cancel_flag.load(Ordering::SeqCst) { self.flow = AppFlow::Failure; self.status = Some(e.to_string()); } else { self.secure_wipe_session(); } self.crypto_rx = None; } } } }
    let (_s,target,label) = eval_strength(&self.password); self.strength_frac += (target - self.strength_frac)*0.18; if (target-self.strength_frac).abs() > 0.003 { ctx.request_repaint_after(Duration::from_millis(32)); }
    if self.flow == AppFlow::Processing { if Instant::now().duration_since(self.last_spinner_tick) >= Duration::from_millis(80) { self.last_spinner_tick = Instant::now(); self.spinner_index = (self.spinner_index+1)%10; let mut rng = OsRng; let s:String=(0..32).map(|_|std::char::from_digit(rng.next_u32()%16,16).unwrap_or('0')).collect(); self.scramble_text = format!("0x{}…{}", &s[..12], &s[20..]); } self.prog_frac += (self.progress-self.prog_frac)*0.15; ctx.request_repaint_after(Duration::from_millis(16)); }

    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Palette::BG)).show(ctx, |ui| {
        self.draw_title_bar(ui);
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            egui::Frame::NONE.fill(Palette::SURFACE).stroke(Stroke::new(1.0, Palette::BORDER)).corner_radius(10.0).inner_margin(24.0).show(ui, |ui| {
                ui.set_width(500.0);
                match self.flow { AppFlow::FileDrop => self.draw_file_drop(ui), AppFlow::Configure => self.draw_configure(ui, ctx, label), AppFlow::Processing => self.draw_processing(ui), AppFlow::Success => self.draw_result(ui, true), AppFlow::Failure => self.draw_result(ui, false) }
            });
            ui.add_space(16.0);
            ui.label(egui::RichText::new(format!("AES-256-GCM-SIV · Argon2id · HKDF-SHA512 · v{}", env!("CARGO_PKG_VERSION"))).font(FontId::new(11.0, FontFamily::Monospace)).color(Palette::TEXT_LO));
        });
    });
} }

impl NeuronEncryptApp {
    fn draw_title_bar(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
        let p = ui.painter_at(rect); p.rect_filled(rect, 0.0, Palette::BG); p.line_segment([Pos2::new(rect.min.x, rect.max.y-1.0), Pos2::new(rect.max.x, rect.max.y-1.0)], Stroke::new(1.0, Palette::BORDER));
        p.text(Pos2::new(rect.min.x+12.0, rect.center().y), Align2::LEFT_CENTER, "NEURON ENCRYPT", FontId::new(13.0, FontFamily::Monospace), Palette::TEXT_HI);
        let drag = Rect::from_min_max(rect.min, Pos2::new(rect.max.x-72.0, rect.max.y));
        let drag_resp = ui.interact(drag, ui.id().with("drag"), Sense::click_and_drag()); if drag_resp.dragged() { ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag); }
        let mut x = rect.max.x - 12.0 - 8.0;
        for kind in ["close","min","pin"] {
            let c = Pos2::new(x, rect.center().y); let r = Rect::from_center_size(c, vec2(16.0,16.0));
            let resp = ui.interact(r, ui.id().with(kind), Sense::click());
            let col = match kind { "close" if resp.hovered()=>Palette::ERROR, "min" if resp.hovered()=>Palette::WARNING, "pin" if self.stay_on_top=>Palette::ACCENT, _=>Palette::TEXT_LO};
            p.circle_filled(c, 8.0, col);
            if resp.clicked() { match kind { "close"=>ui.ctx().send_viewport_cmd(ViewportCommand::Close), "min"=>ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true)), _=>{ self.stay_on_top=!self.stay_on_top; ui.ctx().send_viewport_cmd(ViewportCommand::WindowLevel(if self.stay_on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal })); } } }
            x -= 26.0;
        }
    }
    fn draw_file_drop(&mut self, ui: &mut egui::Ui) { ui.vertical_centered(|ui| { ui.label(egui::RichText::new("NEURON ENCRYPT").font(FontId::new(18.0, FontFamily::Monospace)).color(Palette::TEXT_HI).strong()); ui.add_space(16.0); }); let hover_drop = ui.ctx().input(|i| !i.raw.hovered_files.is_empty()); let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 130.0), Sense::click()); let p = ui.painter_at(rect); p.rect_filled(rect, 8.0, if hover_drop { Palette::ACCENT_DIM } else { Color32::TRANSPARENT }); p.rect_stroke(rect, 8.0, Stroke::new(1.0, if hover_drop { Palette::ACCENT } else { Palette::BORDER }), StrokeKind::Outside); p.text(rect.center_top()+vec2(0.0,46.0), Align2::CENTER_CENTER, "Drop file here", FontId::new(13.0, FontFamily::Monospace), Palette::TEXT_MED); p.text(rect.center_top()+vec2(0.0,68.0), Align2::CENTER_CENTER, "or click to browse", FontId::new(11.0, FontFamily::Monospace), Palette::TEXT_LO); if resp.clicked() { if let Some(path)=rfd::FileDialog::new().pick_file() { self.selected_file=Some(path.clone()); self.flow=AppFlow::Configure; self.mode = if is_vx2_file(&path){Mode::Decrypt}else{Mode::Encrypt}; } } }
    fn draw_configure(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, strength_label: &str) {
        ui.horizontal(|ui| {
            if ui.add(egui::Label::new(egui::RichText::new("← Back").font(FontId::new(11.0, FontFamily::Monospace)).color(Palette::TEXT_LO)).sense(Sense::click())).clicked() { self.password = Zeroizing::new(String::new()); self.confirm_password = Zeroizing::new(String::new()); self.flow = AppFlow::FileDrop; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { let (r,resp)=ui.allocate_exact_size(vec2(96.0,26.0), Sense::click()); let p=ui.painter_at(r); p.rect_stroke(r,13.0,Stroke::new(1.0,Palette::BORDER),StrokeKind::Outside); let left=Rect::from_min_max(r.min, Pos2::new(r.center().x, r.max.y)); let right=Rect::from_min_max(Pos2::new(r.center().x,r.min.y), r.max); p.rect_filled(if self.mode==Mode::Encrypt{left}else{right},13.0,Palette::ACCENT); p.text(left.center(),Align2::CENTER_CENTER,"ENC",FontId::new(11.0,FontFamily::Monospace),if self.mode==Mode::Encrypt{Palette::TEXT_HI}else{Palette::TEXT_LO}); p.text(right.center(),Align2::CENTER_CENTER,"DEC",FontId::new(11.0,FontFamily::Monospace),if self.mode==Mode::Decrypt{Palette::TEXT_HI}else{Palette::TEXT_LO}); if resp.clicked() { self.mode = if ui.input(|i| i.pointer.latest_pos().unwrap_or(r.center()).x < r.center().x) { Mode::Encrypt } else { Mode::Decrypt }; self.confirm_password=Zeroizing::new(String::new()); } });
        });
        ui.add_space(12.0);
        if let Some(path)=&self.selected_file { let name = path.file_name().unwrap_or_default().to_string_lossy(); ui.label(egui::RichText::new(truncate_chars(&name,40)).font(FontId::new(14.0,FontFamily::Monospace)).color(Palette::TEXT_HI)); if let Ok(meta)=std::fs::metadata(path) { ui.label(egui::RichText::new(format_size(meta.len())).font(FontId::new(11.0,FontFamily::Monospace)).color(Palette::TEXT_LO)); } }
        ui.add_space(14.0); let (d,_) = ui.allocate_exact_size(vec2(ui.available_width(),1.0), Sense::hover()); ui.painter_at(d).line_segment([d.left_center(), d.right_center()], Stroke::new(1.0, Palette::BORDER_SUBTLE)); ui.add_space(14.0);
        ui.label(egui::RichText::new("Passphrase").font(FontId::new(12.0, FontFamily::Monospace)).color(Palette::TEXT_MED)); ui.add_space(6.0);
        self.draw_password_input(ui, true);
        if self.mode==Mode::Encrypt { ui.add_space(8.0); self.draw_password_input(ui, false); if !self.password.is_empty() && !self.confirm_password.is_empty() && !constant_time_eq(&self.password, &self.confirm_password) { ui.label(egui::RichText::new("Passphrases don't match").font(FontId::new(11.0, FontFamily::Monospace)).color(Palette::ERROR)); } }
        ui.add_space(8.0);
        let (br,_) = ui.allocate_exact_size(vec2(ui.available_width()-80.0,3.0), Sense::hover()); let p=ui.painter_at(br); p.rect_filled(br,2.0,Palette::BORDER); let fill=Rect::from_min_max(br.min, Pos2::new(br.min.x+br.width()*self.strength_frac, br.max.y)); p.rect_filled(fill,2.0,strength_color(eval_strength(&self.password).0)); ui.put(Rect::from_min_size(Pos2::new(br.max.x+8.0, br.min.y-6.0), vec2(60.0,16.0)), egui::Label::new(egui::RichText::new(strength_label).font(FontId::new(11.0, FontFamily::Monospace)).color(Palette::TEXT_MED)));
        if self.mode==Mode::Encrypt && self.selected_file.as_ref().is_some_and(|p| is_vx2_file(p)) { ui.add_space(8.0); egui::Frame::NONE.fill(Palette::WARNING_MUTED).stroke(Stroke::new(1.0, Palette::WARNING)).corner_radius(6.0).inner_margin(10.0).show(ui, |ui| { ui.checkbox(&mut self.reencrypt_confirmed, "I understand this will re-encrypt an existing .vx2 file"); }); }
        ui.add_space(20.0);
        let mismatch = self.mode==Mode::Encrypt && !self.password.is_empty() && !self.confirm_password.is_empty() && !constant_time_eq(&self.password,&self.confirm_password);
        let disabled = self.password.chars().count() < crypto::MIN_PASSWORD_LEN || mismatch || (self.mode==Mode::Encrypt && self.selected_file.as_ref().is_some_and(|p| is_vx2_file(p)) && !self.reencrypt_confirmed);
        let (r,resp) = ui.allocate_exact_size(vec2(452.0,42.0), Sense::click()); let p=ui.painter_at(r); let fill = if disabled { Palette::SURFACE_HI } else if resp.hovered() { Palette::ACCENT_HOVER } else { Palette::ACCENT }; p.rect_filled(r,6.0,fill); p.text(r.center(),Align2::CENTER_CENTER,if self.mode==Mode::Encrypt{"Encrypt file →"}else{"Decrypt file →"},FontId::new(13.0,FontFamily::Monospace),if disabled{Palette::TEXT_LO}else{Palette::TEXT_HI}); if resp.clicked() && !disabled { self.execute(ctx); }
    }
    fn draw_password_input(&mut self, ui: &mut egui::Ui, primary: bool) {
        let (rect, _) = ui.allocate_exact_size(vec2(452.0, 38.0), Sense::hover());
        let focus = if primary { ui.memory(|m| m.has_focus(ui.id().with("pw"))) } else { ui.memory(|m| m.has_focus(ui.id().with("cpw"))) };
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6.0, Palette::SURFACE_HI);
        p.rect_stroke(rect, 6.0, Stroke::new(1.0, if focus { Palette::BORDER_FOCUS } else { Palette::BORDER }), StrokeKind::Outside);
        let input = Rect::from_min_max(rect.min + vec2(10.0, 9.0), Pos2::new(rect.max.x - 34.0, rect.max.y - 9.0));
        ui.allocate_ui_at_rect(input, |ui| {
            let mut te = if primary { egui::TextEdit::singleline(&mut *self.password).id(ui.id().with("pw")) } else { egui::TextEdit::singleline(&mut *self.confirm_password).id(ui.id().with("cpw")) };
            ui.add(te.frame(false).password(!self.show_password));
        });
    }
    fn draw_processing(&mut self, ui: &mut egui::Ui) { let chars=["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]; ui.add_space(20.0); ui.vertical_centered(|ui| { ui.label(egui::RichText::new(chars[self.spinner_index]).font(FontId::new(32.0,FontFamily::Monospace)).color(Palette::ACCENT)); ui.add_space(12.0); ui.label(egui::RichText::new(if self.mode==Mode::Encrypt{"Encrypting…"}else{"Decrypting…"}).font(FontId::new(14.0,FontFamily::Monospace)).color(Palette::TEXT_HI)); ui.add_space(8.0); ui.label(egui::RichText::new(truncate_chars(self.status.as_deref().unwrap_or("Processing..."),50)).font(FontId::new(11.0,FontFamily::Monospace)).color(Palette::TEXT_LO)); ui.add_space(14.0); let (r,_) = ui.allocate_exact_size(vec2(452.0,6.0),Sense::hover()); let p=ui.painter_at(r); p.rect_filled(r,3.0,Palette::SURFACE_HI); p.rect_filled(Rect::from_min_max(r.min,Pos2::new(r.min.x+r.width()*self.prog_frac,r.max.y)),3.0,Palette::ACCENT); p.text(Pos2::new(r.max.x+6.0,r.center().y),Align2::LEFT_CENTER,format!("{}%",(self.prog_frac*100.0) as u32),FontId::new(11.0,FontFamily::Monospace),Palette::TEXT_MED); ui.add_space(8.0); ui.label(egui::RichText::new(&self.scramble_text).font(FontId::new(11.0,FontFamily::Monospace)).color(Palette::TEXT_LO)); ui.add_space(12.0); if ui.add(egui::Label::new(egui::RichText::new("Cancel").font(FontId::new(11.0,FontFamily::Monospace)).color(Palette::TEXT_LO)).sense(Sense::click())).clicked() { self.cancel_flag.store(true, Ordering::SeqCst); self.status=Some("Cancelled.".to_owned()); self.flow=AppFlow::FileDrop; } }); }
    fn draw_result(&mut self, ui: &mut egui::Ui, ok: bool) { ui.add_space(20.0); let (r,_) = ui.allocate_exact_size(vec2(80.0,80.0), Sense::hover()); let p=ui.painter_at(r); p.circle_stroke(r.center(),28.0,Stroke::new(2.0,if ok{Palette::SUCCESS}else{Palette::ERROR})); if ok { self.check_anim=(self.check_anim+0.07).min(1.0); let a=Pos2::new(r.min.x+80.0*0.22,r.min.y+80.0*0.52); let b=Pos2::new(r.min.x+80.0*0.42,r.min.y+80.0*0.72); let c=Pos2::new(r.min.x+80.0*0.78,r.min.y+80.0*0.32); p.add(Shape::line(vec![a,b,c],Stroke::new(2.0,Palette::SUCCESS))); } else { p.line_segment([r.center()+vec2(-14.0,-14.0),r.center()+vec2(14.0,14.0)],Stroke::new(2.0,Palette::ERROR)); p.line_segment([r.center()+vec2(14.0,-14.0),r.center()+vec2(-14.0,14.0)],Stroke::new(2.0,Palette::ERROR)); }
        ui.vertical_centered(|ui| { ui.label(egui::RichText::new(if ok{"Done"}else{"Failed"}).font(FontId::new(18.0,FontFamily::Monospace)).color(if ok{Palette::TEXT_HI}else{Palette::ERROR})); if let Some(d)=&self.dest_path { let n = d.file_name().unwrap_or_default().to_string_lossy(); ui.label(egui::RichText::new(truncate_chars(&n,40)).font(FontId::new(11.0,FontFamily::Monospace)).color(Palette::TEXT_MED)); } ui.add_space(16.0); if ok { ui.horizontal(|ui| { let (a,ra)=ui.allocate_exact_size(vec2(220.0,38.0),Sense::click()); ui.painter_at(a).rect_filled(a,6.0,Palette::SURFACE_HI); ui.painter_at(a).rect_stroke(a,6.0,Stroke::new(1.0,Palette::BORDER),StrokeKind::Outside); ui.painter_at(a).text(a.center(),Align2::CENTER_CENTER,"Open folder",FontId::new(12.0,FontFamily::Monospace),Palette::TEXT_MED); if ra.clicked() { if let Some(p)=&self.dest_path { if let Some(parent)=p.parent() { let _=open::that(parent); } } }
                let (b,rb)=ui.allocate_exact_size(vec2(220.0,38.0),Sense::click()); ui.painter_at(b).rect_filled(b,6.0,Palette::ACCENT); ui.painter_at(b).text(b.center(),Align2::CENTER_CENTER,"New file",FontId::new(12.0,FontFamily::Monospace),Palette::TEXT_HI); if rb.clicked() { self.secure_wipe_session(); } }); } else { let msg=truncate_chars(self.status.as_deref().unwrap_or("Unknown error"),120); ui.label(egui::RichText::new(msg).font(FontId::new(11.0,FontFamily::Monospace)).color(Palette::ERROR)); let (b,rb)=ui.allocate_exact_size(vec2(452.0,38.0),Sense::click()); ui.painter_at(b).rect_filled(b,6.0,Palette::ACCENT); ui.painter_at(b).text(b.center(),Align2::CENTER_CENTER,"Try again",FontId::new(12.0,FontFamily::Monospace),Palette::TEXT_HI); if rb.clicked() { self.secure_wipe_session(); } } }); }
}
