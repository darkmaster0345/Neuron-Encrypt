import sys

file_path = 'neuron-encrypt/src/gui.rs'
with open(file_path, 'r') as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if 'let can_proceed = self.password.len() >= crypto::MIN_PASSWORD_LEN && (self.mode == Mode::Decrypt || *self.password == *self.confirm_password);' in line:
        indent = line[:line.find('let')]
        new_lines.append(f"{indent}// Disable button if validation fails\n")
        new_lines.append(f"{indent}let is_short = self.password.chars().count() < crypto::MIN_PASSWORD_LEN;\n")
        new_lines.append(f"{indent}let password_match = self.mode == Mode::Decrypt || *self.password == *self.confirm_password;\n")
        new_lines.append(f"{indent}let reencrypt_gate = if self.mode == Mode::Encrypt && self.selected_file.as_ref().map(|p| is_vx2_file(p)).unwrap_or(false) {{\n")
        new_lines.append(f"{indent}    self.reencrypt_confirmed\n")
        new_lines.append(f"{indent}}} else {{\n")
        new_lines.append(f"{indent}    true\n")
        new_lines.append(f"{indent}}};\n")
        new_lines.append(f"\n")
        new_lines.append(f"{indent}let can_proceed = !is_short && password_match && reencrypt_gate;\n")
    elif '// Disable button if validation fails' in line:
        continue # Handled above
    else:
        new_lines.append(line)

with open(file_path, 'w') as f:
    f.writelines(new_lines)
