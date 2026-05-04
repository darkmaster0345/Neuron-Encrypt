import re

with open('neuron-encrypt/src/gui.rs', 'r') as f:
    content = f.read()

# Issue 1: Window controls & Dragging
# Remove this line. ViewportCommand::Minimized(true) is the correct egui API for minimizing viewports.
# Replace close command just in case
content = content.replace('ui.ctx().send_viewport_cmd(ViewportCommand::Close)', 'ui.ctx().send_viewport_cmd(ViewportCommand::Close)')

# Add drag region to the dot (circle on left)
# The code has:
# painter.rect_filled(
#     Rect::from_min_max(
#         Pos2::new(rect.min.x + 16.0, rect.min.y + 12.0),
#         Pos2::new(rect.min.x + 24.0, rect.min.y + 20.0),
#     ),
#     3.0,
#     Palette::ACCENT,
# );
# I will wrap it with a drag handle.
dot_pattern = r'painter\.rect_filled\(\s*Rect::from_min_max\(\s*Pos2::new\(rect\.min\.x \+ 16\.0, rect\.min\.y \+ 12\.0\),\s*Pos2::new\(rect\.min\.x \+ 24\.0, rect\.min\.y \+ 20\.0\),\s*\),\s*3\.0,\s*Palette::ACCENT,\s*\);'
dot_replacement = r'''let dot_rect = Rect::from_min_max(
            Pos2::new(rect.min.x + 12.0, rect.min.y + 8.0),
            Pos2::new(rect.min.x + 28.0, rect.min.y + 24.0),
        );
        let dot_res = ui.interact(dot_rect, ui.id().with("dot_drag"), Sense::drag());
        if dot_res.dragged() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.min.x + 16.0, rect.min.y + 12.0),
                Pos2::new(rect.min.x + 24.0, rect.min.y + 20.0),
            ),
            4.0,
            Palette::ACCENT,
        );'''
content = re.sub(dot_pattern, dot_replacement, content)

# Issue 3: Pill shape for badges
# draw_info_chip rounding
content = content.replace('fn draw_info_chip(&self, ui: &mut egui::Ui, label: &str, fill: Color32, text_color: Color32) {',
                         'fn draw_info_chip(&self, ui: &mut egui::Ui, label: &str, fill: Color32, text_color: Color32) {\n        let rounding = 11.0;')
content = content.replace('.rounding(999.0)', '.rounding(rounding)')

# draw_screen_header rounding (for the top badge)
content = content.replace('fn draw_screen_header(&self, ui: &mut egui::Ui) {',
                         'fn draw_screen_header(&self, ui: &mut egui::Ui) {\n        let rounding = 11.0;')
# Ensure we replace the rounding in draw_screen_header too if it uses 999.0
content = content.replace('.rounding(999.0)', '.rounding(rounding)')

# Issue 2: Encrypt/Decrypt buttons in draw_configure
# First, find draw_configure and the part where the button is.
# I'll replace the entire button block at the end of draw_configure.

# The end of draw_configure looks like:
#         let disabled = ...;
#         if self.draw_button(...) .clicked() && !disabled { self.execute(ctx); }

# I will replace the button part.
button_section_pattern = r'let mismatch = self\.mode == Mode::Encrypt.*?if self\s*\.draw_button\(.*?\}\s*\}'
# I need to be careful with the regex to match until the end of the method.
# Actually, I'll just look for the specific call to draw_button with self.action_label().

action_button_pattern = r'if self\s*\.draw_button\(\s*ui,\s*self\.action_label\(\),\s*vec2\(ui\.available_width\(\), 44\.0\),\s*ButtonKind::Primary,\s*!disabled,\s*\)\s*\.clicked\(\)\s*&& !disabled\s*\{\s*self\.execute\(ctx\);\s*\}'

new_button_logic = r'''let is_vx2 = self.selected_file.as_ref().is_some_and(|p| is_vx2_file(p));

        // Encrypt button
        let enc_mismatch = !self.password.is_empty() && !self.confirm_password.is_empty() && !constant_time_eq(&self.password, &self.confirm_password);
        let enc_disabled = self.password.chars().count() < crypto::MIN_PASSWORD_LEN
            || (self.mode == Mode::Encrypt && enc_mismatch)
            || (is_vx2 && !self.reencrypt_confirmed);

        if self.draw_button(
            ui,
            "ENCRYPT",
            vec2(ui.available_width(), 44.0),
            ButtonKind::Primary,
            !enc_disabled,
        ).clicked() && !enc_disabled {
            self.mode = Mode::Encrypt;
            self.execute(ctx);
        }

        ui.add_space(10.0);

        // Decrypt button
        let dec_disabled = self.password.is_empty() || !is_vx2;
        if self.draw_button(
            ui,
            "DECRYPT",
            vec2(ui.available_width(), 44.0),
            ButtonKind::Secondary,
            !dec_disabled,
        ).clicked() && !dec_disabled {
            self.mode = Mode::Decrypt;
            self.execute(ctx);
        }'''

content = re.sub(action_button_pattern, new_button_logic, content)

# Issue 4: File drop handling in draw_file_drop
# Ensure it captures dropped files and sets flow.
# The user wants "Add handling for ui.ctx().input(|i| i.raw.dropped_files.clone()) to actually capture dropped files"
# Wait, it's already in 'update'. But maybe they want it in 'draw_file_drop' too or instead?
# If I put it in 'draw_file_drop', it will only work when that screen is visible.
# But 'update' handles it globally.

# Let's check if 'set_selected_file' sets flow to Configure.
# Yes: self.flow = AppFlow::Configure;

# Let's check if 'pick_file' calls 'set_selected_file'.
# Yes: if let Some(path) = rfd::FileDialog::new().pick_file() { self.set_selected_file(path); }

# One thing: "confirm self.flow is set to AppFlow::Configure and self.selected_file is populated"
# I'll add a check in draw_file_drop just to be sure.

file_drop_input_pattern = r'let hover_drop = ui\.ctx\(\)\.input\(\|i\| !i\.raw\.hovered_files\.is_empty\(\)\);'
file_drop_input_replacement = r'''let dropped_files = ui.ctx().input(|i| i.raw.dropped_files.clone());
        if let Some(path) = dropped_files.first().and_then(|f| f.path.clone()) {
            self.set_selected_file(path);
        }
        let hover_drop = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());'''
content = re.sub(file_drop_input_pattern, file_drop_input_replacement, content)

with open('neuron-encrypt/src/gui.rs', 'w') as f:
    f.write(content)
