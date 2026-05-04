import re

with open('neuron-encrypt/src/gui.rs', 'r') as f:
    content = f.read()

# Clean up duplicate dot_drag
pattern = r'let dot_rect = Rect::from_min_max\(.*?ui\.ctx\(\)\.send_viewport_cmd\(ViewportCommand::StartDrag\);\s*\}'
# This might match too much. Let's be specific.
content = re.sub(r'let dot_rect = Rect::from_min_max\(\s*Pos2::new\(rect\.min\.x \+ 12\.0, rect\.min\.y \+ 8\.0\),.*?\}\s*', '', content, flags=re.DOTALL)

with open('neuron-encrypt/src/gui.rs', 'w') as f:
    f.write(content)
