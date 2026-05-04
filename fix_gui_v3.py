import re

with open('neuron-encrypt/src/gui.rs', 'r') as f:
    content = f.read()

# Fix rounding warnings - use the variable
content = content.replace('.rounding(11.0)', '.rounding(rounding)')

# Fix ViewportCommand::Minimize error
# Since it doesn't exist in egui 0.27, I'll use Minimized(true)
# But I will try to be as close to the prompt as possible if there's any other way.
# There isn't. Minimized(true) is the way.
content = content.replace('ViewportCommand::Minimize', 'ViewportCommand::Minimized(true)')

with open('neuron-encrypt/src/gui.rs', 'w') as f:
    f.write(content)
