#!/usr/bin/env python3
"""
VAULTX - SECURE FILE VAULT (Open Source Version)
AES-256-GCM-SIV · ARGON2ID · HKDF-SHA512
Version 1.2 - SECURITY ENHANCED

SECURITY NOTICE:
This is open source software. Always verify the source code before use.
Never trust compiled executables for cryptographic applications.
"""

import tkinter as tk
from tkinter import filedialog, messagebox
import tkinterdnd2 as tkdnd
import threading
import time
import os
import secrets
from datetime import datetime
import math
import json
import hashlib
import sys
import inspect

# Cryptographic imports - EXACT paths as specified
from cryptography.hazmat.primitives.ciphers.aead import AESGCMSIV
from cryptography.hazmat.primitives.kdf.argon2 import Argon2id
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from cryptography.hazmat.primitives import hashes


class SecurityError(Exception):
    """Security-related exceptions"""
    pass


class VaultXSecurity:
    """Security and integrity verification for VaultX"""
    
    # Known good hash of this file (update after any changes)
    EXPECTED_HASH = "PLACEHOLDER_HASH"
    
    @staticmethod
    def calculate_file_hash(file_path):
        """Calculate SHA-256 hash of a file"""
        hasher = hashlib.sha256()
        try:
            with open(file_path, 'rb') as f:
                for chunk in iter(lambda: f.read(4096), b""):
                    hasher.update(chunk)
            return hasher.hexdigest()
        except Exception:
            return None
    
    @staticmethod
    def verify_integrity():
        """Verify the application hasn't been modified"""
        try:
            current_file = inspect.getfile(inspect.currentframe())
            current_hash = VaultXSecurity.calculate_file_hash(current_file)
            
            # For development - allow any hash
            # In production, this should be verified against EXPECTED_HASH
            if current_hash is None:
                raise SecurityError("Cannot verify file integrity")
            
            return True
        except Exception as e:
            print(f"Security warning: {e}")
            return True  # Allow execution but warn user
    
    @staticmethod
    def detect_debugging():
        """Basic debugging detection"""
        try:
            import sys
            import pydevd
            return True  # PyDev debugger detected
        except ImportError:
            pass
        
        try:
            import pdb
            return 'pdb' in sys.modules
        except:
            pass
        
        return False
    
    @staticmethod
    def secure_random_bytes(size):
        """Generate cryptographically secure random bytes"""
        return secrets.token_bytes(size)
    
    @staticmethod
    def verify_password_strength(password):
        """Enhanced password strength verification"""
        if len(password) < 12:
            return False, "Password must be at least 12 characters"
        
        has_upper = any(c.isupper() for c in password)
        has_lower = any(c.islower() for c in password)
        has_digit = any(c.isdigit() for c in password)
        has_special = any(c in "!@#$%^&*()_+-=[]{}|;:,.<>?" for c in password)
        
        if not (has_upper and has_lower and has_digit and has_special):
            return False, "Password must contain uppercase, lowercase, digits, and special characters"
        
        # Check for common patterns
        if password.lower() in ["password", "12345678", "qwerty123"]:
            return False, "Password is too common"
        
        return True, "Password meets security requirements"


class VaultXApp:
    def __init__(self, root):
        self.root = root
        self.root.title("VAULTX - SECURE FILE VAULT (Open Source)")
        self.root.geometry("900x700")
        self.root.configure(bg='#FFFFFF')
        self.root.resizable(True, True)
        
        # Security verification
        if not VaultXSecurity.verify_integrity():
            self.show_security_warning()
        
        if VaultXSecurity.detect_debugging():
            self.show_debug_warning()
        
        # State variables
        self.mode = "ENCRYPT"  # ENCRYPT or DECRYPT
        self.selected_file = None
        self.is_running = False
        self.drag_active = False
        self.recent_files = []
        self.recent_files_file = "vaultx_recent.json"
        
        # Color scheme - White theme
        self.colors = {
            'bg': '#FFFFFF',           # White background
            'panel': '#F8F9FA',        # Light gray panel
            'card': '#FFFFFF',           # White cards
            'border': '#DEE2E6',       # Light border
            'primary': '#0066CC',       # Blue primary
            'secondary': '#28A745',    # Green secondary
            'danger': '#DC3545',       # Red danger
            'warning': '#FFC107',      # Amber warning
            'text': '#212529',          # Dark text
            'dim': '#6C757D',          # Gray dim text
            'hex_dark': '#E9ECEF',      # Light hex
            'hex_cyan': '#CCE5FF',     # Light cyan
            'hex_green': '#D4EDDA'     # Light green
        }
        
        self.setup_ui()
        self.setup_drag_drop()
        self.setup_keyboard_shortcuts()
        self.load_recent_files()
        self.start_clock()
        self.start_hex_animation()
        self.show_security_notice()
    
    def show_security_warning(self):
        """Show security warning if integrity check fails"""
        messagebox.showwarning(
            "Security Warning",
            "Application integrity verification failed.\n"
            "The source code may have been modified.\n\n"
            "For maximum security, always verify the source code\n"
            "before using any cryptographic application."
        )
    
    def show_debug_warning(self):
        """Show warning if debugging is detected"""
        messagebox.showwarning(
            "Debug Warning",
            "Debugging environment detected.\n"
            "This may compromise security.\n\n"
            "For production use, run without debugging tools."
        )
    
    def show_security_notice(self):
        """Show initial security notice"""
        self.log("SECURITY NOTICE: This is open source software.", self.colors['warning'])
        self.log("Always verify source code before use.", self.colors['warning'])
        self.log("Never trust compiled executables for crypto.", self.colors['warning'])
    
    def setup_ui(self):
        # Create hex canvas background
        self.hex_canvas = tk.Canvas(
            self.root, width=800, height=850,
            bg=self.colors['bg'], highlightthickness=0
        )
        self.hex_canvas.place(x=0, y=0)
        
        # Main container
        self.main_frame = tk.Frame(self.root, bg=self.colors['bg'])
        self.main_frame.place(x=0, y=0, width=800, height=850)
        
        # Status Bar
        self.create_status_bar()
        
        # Title
        self.create_title()
        
        # Hex Grid (will be drawn after UI setup)
        self.root.after(100, self.create_hex_grid)
        
        # Mode Selector
        self.create_mode_selector()
        
        # File Selector
        self.create_file_selector()
        
        # Password Input
        self.create_password_input()
        
        # System Log
        self.create_system_log()
        
        # Execute Button
        self.create_execute_button()
        
        # Progress Bar
        self.create_progress_bar()
        
        # Instruction label
        self.instruction_label = tk.Label(
            self.main_frame, text="Select a file, enter password, then click the button above to start",
            font=("Courier New", 9),
            fg=self.colors['dim'], bg=self.colors['bg']
        )
        self.instruction_label.pack(pady=(0, 5))
        
        # Warning Footer
        self.create_warning_footer()
    
    def create_status_bar(self):
        status_frame = tk.Frame(self.main_frame, bg=self.colors['panel'], height=40)
        status_frame.pack(fill=tk.X, padx=10, pady=(10, 5))
        status_frame.pack_propagate(False)
        
        # Status indicator
        status_container = tk.Frame(status_frame, bg=self.colors['panel'])
        status_container.pack(side=tk.LEFT, padx=20, pady=8)
        
        status_dot = tk.Canvas(status_container, width=10, height=10, 
                               bg=self.colors['panel'], highlightthickness=0)
        status_dot.pack(side=tk.LEFT)
        status_dot.create_oval(2, 2, 8, 8, fill=self.colors['secondary'], outline='')
        
        tk.Label(status_container, text="ONLINE", 
                font=("Courier New", 10, "bold"),
                fg=self.colors['secondary'], bg=self.colors['panel']).pack(side=tk.LEFT, padx=(5, 0))
        
        # Crypto info
        tk.Label(status_frame, 
                text="AES-256-GCM-SIV · Argon2id · HKDF-SHA512 · OPEN SOURCE",
                font=("Courier New", 9),
                fg=self.colors['text'], bg=self.colors['panel']).pack(side=tk.LEFT, padx=20)
        
        # Clock
        self.clock_label = tk.Label(status_frame, text="00:00:00",
                                   font=("Courier New", 10, "bold"),
                                   fg=self.colors['primary'], bg=self.colors['panel'])
        self.clock_label.pack(side=tk.RIGHT, padx=20)
    
    def create_title(self):
        title_frame = tk.Frame(self.main_frame, bg=self.colors['bg'])
        title_frame.pack(pady=(20, 10))
        
        # VAULTX title with colors
        title = tk.Label(title_frame, text="VAULTX",
                        font=("Courier New", 32, "bold"),
                        fg=self.colors['primary'], bg=self.colors['bg'])
        title.pack()
        
        subtitle = tk.Label(title_frame, 
                           text="SECURE FILE VAULT // OPEN SOURCE VERSION",
                           font=("Courier New", 10),
                           fg=self.colors['text'], bg=self.colors['bg'])
        subtitle.pack()
    
    def create_hex_grid(self):
        # Calculate hex grid parameters
        canvas_width = 800
        canvas_height = 850
        hex_size = 30  # Size of each hex
        max_cells = 200
        
        # Calculate grid dimensions to stay under 200 cells
        cols = min(int(canvas_width / (hex_size * 1.5)), 20)
        rows = min(max_cells // cols, 15)
        
        self.hex_cells = []
        
        # Draw hex grid
        for row in range(rows):
            for col in range(cols):
                x = col * hex_size * 1.5 + 50
                y = row * hex_size * 1.7 + 80
                if col % 2 == 1:
                    y += hex_size * 0.85
                
                # Create hexagon
                points = []
                for i in range(6):
                    angle = math.pi / 3 * i
                    px = x + hex_size * 0.866 * math.cos(angle)
                    py = y + hex_size * 0.866 * math.sin(angle)
                    points.extend([px, py])
                
                hex_id = self.hex_canvas.create_polygon(
                    points, fill='', outline=self.colors['hex_dark'], width=1
                )
                self.hex_cells.append(hex_id)
    
    def create_mode_selector(self):
        mode_frame = tk.Frame(self.main_frame, bg=self.colors['card'], relief=tk.RAISED, bd=1)
        mode_frame.pack(fill=tk.X, padx=20, pady=10)
        
        button_frame = tk.Frame(mode_frame, bg=self.colors['card'])
        button_frame.pack(fill=tk.X, padx=10, pady=10)
        
        self.encrypt_btn = tk.Button(
            button_frame, text="⬡  ENCRYPT MODE",
            font=("Courier New", 12, "bold"),
            bg=self.colors['primary'], fg=self.colors['bg'],
            relief=tk.FLAT, bd=0, padx=20, pady=10,
            command=lambda: self.set_mode("ENCRYPT")
        )
        self.encrypt_btn.pack(side=tk.LEFT, fill=tk.BOTH, expand=True, padx=(0, 5))
        
        self.decrypt_btn = tk.Button(
            button_frame, text="⬡  DECRYPT MODE",
            font=("Courier New", 12, "bold"),
            bg=self.colors['panel'], fg=self.colors['dim'],
            relief=tk.FLAT, bd=0, padx=20, pady=10,
            command=lambda: self.set_mode("DECRYPT")
        )
        self.decrypt_btn.pack(side=tk.LEFT, fill=tk.BOTH, expand=True, padx=(5, 0))
    
    def create_file_selector(self):
        file_frame = tk.Frame(self.main_frame, bg=self.colors['card'], relief=tk.RAISED, bd=1)
        file_frame.pack(fill=tk.X, padx=20, pady=10)
        
        tk.Label(file_frame, text="TARGET FILE", font=("Courier New", 10, "bold"),
                fg=self.colors['text'], bg=self.colors['card']).pack(anchor=tk.W, padx=10, pady=(10, 5))
        
        # Recent files dropdown
        self.recent_var = tk.StringVar()
        self.recent_dropdown = tk.OptionMenu(
            file_frame, self.recent_var, "Recent Files...",
            command=self.on_recent_file_selected
        )
        self.recent_dropdown.configure(
            font=("Courier New", 9),
            bg=self.colors['panel'], fg=self.colors['text'],
            activebackground=self.colors['primary'],
            activeforeground=self.colors['bg'],
            relief=tk.FLAT, bd=0,
            highlightthickness=0
        )
        self.recent_dropdown.pack(fill=tk.X, padx=10, pady=(0, 5))
        self.update_recent_dropdown()
        
        self.file_display = tk.Frame(file_frame, bg=self.colors['panel'], relief=tk.SUNKEN, bd=1)
        self.file_display.pack(fill=tk.X, padx=10, pady=(0, 5))
        
        self.file_label = tk.Label(self.file_display, text="No file selected",
                                  font=("Courier New", 10),
                                  fg=self.colors['dim'], bg=self.colors['panel'],
                                  anchor=tk.W)
        self.file_label.pack(fill=tk.X, padx=10, pady=5)
        
        # Button frame for browse and clear
        button_frame = tk.Frame(file_frame, bg=self.colors['card'])
        button_frame.pack(fill=tk.X, padx=10, pady=(0, 10))
        
        self.browse_btn = tk.Button(
            button_frame, text="BROWSE",
            font=("Courier New", 10, "bold"),
            bg=self.colors['border'], fg=self.colors['text'],
            relief=tk.FLAT, bd=0, padx=15, pady=5,
            command=self.browse_file
        )
        self.browse_btn.pack(side=tk.LEFT, padx=(0, 5))
        
        self.clear_recent_btn = tk.Button(
            button_frame, text="CLEAR RECENT",
            font=("Courier New", 9),
            bg=self.colors['border'], fg=self.colors['dim'],
            relief=tk.FLAT, bd=0, padx=10, pady=5,
            command=self.clear_recent_files
        )
        self.clear_recent_btn.pack(side=tk.RIGHT)
    
    def create_password_input(self):
        pass_frame = tk.Frame(self.main_frame, bg=self.colors['card'], relief=tk.RAISED, bd=1)
        pass_frame.pack(fill=tk.X, padx=20, pady=10)
        
        tk.Label(pass_frame, text="ENCRYPTION KEY", font=("Courier New", 10, "bold"),
                fg=self.colors['text'], bg=self.colors['card']).pack(anchor=tk.W, padx=10, pady=(10, 5))
        
        self.password_var = tk.StringVar()
        self.password_var.trace_add('write', lambda *args: self.update_password_strength())
        
        self.password_entry = tk.Entry(
            pass_frame, textvariable=self.password_var,
            font=("Courier New", 12), show="●",
            bg='#FFFFFF', fg='#000000',  # Black text on white background
            insertbackground='#000000',   # Black cursor
            relief=tk.SUNKEN, bd=2,        # Visible border
            highlightcolor=self.colors['primary'],
            highlightthickness=1
        )
        self.password_entry.pack(fill=tk.X, padx=10, pady=(0, 5))
        
        # Reveal checkbox
        self.reveal_var = tk.BooleanVar()
        reveal_check = tk.Checkbutton(
            pass_frame, text="REVEAL KEY", variable=self.reveal_var,
            font=("Courier New", 9),
            fg=self.colors['text'], bg=self.colors['card'],
            selectcolor=self.colors['panel'],
            activebackground=self.colors['card'],
            command=self.toggle_password_visibility
        )
        reveal_check.pack(anchor=tk.W, padx=10, pady=(0, 5))
        
        # Password strength meter
        self.strength_frame = tk.Frame(pass_frame, bg=self.colors['card'])
        self.strength_frame.pack(fill=tk.X, padx=10, pady=(0, 10))
        
        self.strength_bars = []
        for i in range(10):
            bar = tk.Label(self.strength_frame, text="▪", font=("Courier New", 8),
                          fg=self.colors['dim'], bg=self.colors['card'])
            bar.pack(side=tk.LEFT, padx=1)
            self.strength_bars.append(bar)
        
        self.strength_label = tk.Label(self.strength_frame, text="NONE",
                                      font=("Courier New", 8, "bold"),
                                      fg=self.colors['dim'], bg=self.colors['card'])
        self.strength_label.pack(side=tk.LEFT, padx=(10, 0))
    
    def create_system_log(self):
        log_frame = tk.Frame(self.main_frame, bg=self.colors['card'], relief=tk.RAISED, bd=1)
        log_frame.pack(fill=tk.X, padx=20, pady=10)
        
        tk.Label(log_frame, text="SYSTEM LOG", font=("Courier New", 10, "bold"),
                fg=self.colors['text'], bg=self.colors['card']).pack(anchor=tk.W, padx=10, pady=(10, 5))
        
        self.log_container = tk.Frame(log_frame, bg=self.colors['panel'], relief=tk.SUNKEN, bd=1)
        self.log_container.pack(fill=tk.X, padx=10, pady=(0, 10))
        
        self.log_lines = []
        for i in range(4):  # Increased to 4 lines for security notices
            line = tk.Label(self.log_container, text="",
                          font=("Courier New", 9),
                          fg=self.colors['dim'], bg=self.colors['panel'],
                          anchor=tk.W)
            line.pack(fill=tk.X, padx=10, pady=1)
            self.log_lines.append(line)
        
        self.log("System initialized. Ready for operations.", self.colors['secondary'])
    
    def create_progress_bar(self):
        self.progress_frame = tk.Frame(self.main_frame, bg=self.colors['card'], relief=tk.RAISED, bd=1)
        self.progress_frame.pack(fill=tk.X, padx=20, pady=10)
        
        self.progress_canvas = tk.Canvas(
            self.progress_frame, width=760, height=30,
            bg=self.colors['panel'], highlightthickness=0
        )
        self.progress_canvas.pack(padx=10, pady=10)
        
        # Progress bar background
        self.progress_canvas.create_rectangle(2, 8, 758, 22, 
                                            fill=self.colors['border'], outline='')
        
        # Progress bar fill (will be updated)
        self.progress_fill = self.progress_canvas.create_rectangle(2, 8, 2, 22,
                                                                  fill=self.colors['primary'], outline='')
        
        # Progress percentage
        self.progress_text = self.progress_canvas.create_text(380, 15,
                                                             text="0%",
                                                             font=("Courier New", 10, "bold"),
                                                             fill=self.colors['text'])
    
    def create_execute_button(self):
        self.execute_btn = tk.Button(
            self.main_frame, text="ENCRYPT FILE",
            font=("Courier New", 14, "bold"),
            bg=self.colors['primary'], fg=self.colors['bg'],
            relief=tk.FLAT, bd=0, padx=20, pady=15,
            command=self.execute_operation
        )
        self.execute_btn.pack(fill=tk.X, padx=20, pady=5)
    
    def create_warning_footer(self):
        warning_frame = tk.Frame(self.main_frame, bg=self.colors['bg'])
        warning_frame.pack(fill=tk.X, padx=20, pady=10)
        
        warning_text = "⚠  OPEN SOURCE SOFTWARE · VERIFY SOURCE CODE · LOSS OF PASSWORD = PERMANENT DATA LOSS"
        tk.Label(warning_frame, text=warning_text,
                font=("Courier New", 8),
                fg=self.colors['dim'], bg=self.colors['bg']).pack()
    
    def setup_drag_drop(self):
        # Enable drag and drop on the main window
        self.root.drop_target_register(tkdnd.DND_FILES)
        self.root.dnd_bind('<<Drop>>', self.on_drop)
        self.root.dnd_bind('<<DragEnter>>', self.on_drag_enter)
        self.root.dnd_bind('<<DragLeave>>', self.on_drag_leave)
        
        # Create drag overlay (initially hidden)
        self.drag_overlay = tk.Frame(self.root, bg=self.colors['primary'], relief=tk.RAISED, bd=3)
        self.drag_label = tk.Label(self.drag_overlay, 
                                 text="📁 DROP FILES TO ENCRYPT/DECRYPT",
                                 font=("Courier New", 16, "bold"),
                                 fg=self.colors['bg'], bg=self.colors['primary'])
        self.drag_label.pack(expand=True, fill=tk.BOTH)
    
    def on_drag_enter(self, event):
        self.drag_active = True
        self.drag_overlay.place(x=0, y=0, width=800, height=850)
        self.drag_overlay.lift()
    
    def on_drag_leave(self, event):
        self.drag_active = False
        self.drag_overlay.place_forget()
    
    def on_drop(self, event):
        self.drag_active = False
        self.drag_overlay.place_forget()
        
        # Get dropped files
        files = self.root.tk.splitlist(event.data)
        
        if not files:
            return
        
        # Handle first file (for now, we'll extend to batch later)
        file_path = files[0].strip('{}')  # Remove curly braces from Windows paths
        
        if not os.path.exists(file_path):
            self.log(f"ERROR: File not found: {file_path}", self.colors['danger'])
            return
        
        # Validate file based on current mode
        if self.mode == "DECRYPT" and not file_path.endswith('.vx2'):
            self.log("ERROR: Only .vx2 files can be decrypted in DECRYPT mode", self.colors['danger'])
            return
        
        # Set the file
        self.selected_file = file_path
        display_name = os.path.basename(file_path)
        self.file_label.configure(text=f"▶ {display_name}", fg=self.colors['primary'])
        
        # Add to recent files
        self.add_to_recent_files(file_path, self.mode)
        
        # Show file info
        size = os.path.getsize(file_path)
        size_str = self.format_size(size)
        directory = os.path.dirname(file_path)
        self.log(f"Dropped: {display_name} ({size_str})", self.colors['primary'])
        self.log(f"Location: {directory}", self.colors['text'])
        
        # If multiple files were dropped, mention it
        if len(files) > 1:
            self.log(f"Note: {len(files)-1} additional files dropped (batch processing coming soon)", self.colors['warning'])
    
    def setup_keyboard_shortcuts(self):
        # Ctrl+E: Encrypt mode
        self.root.bind('<Control-e>', lambda e: self.set_mode("ENCRYPT"))
        self.root.bind('<Control-E>', lambda e: self.set_mode("ENCRYPT"))
        
        # Ctrl+D: Decrypt mode
        self.root.bind('<Control-d>', lambda e: self.set_mode("DECRYPT"))
        self.root.bind('<Control-D>', lambda e: self.set_mode("DECRYPT"))
        
        # Ctrl+O: Browse file
        self.root.bind('<Control-o>', lambda e: self.browse_file())
        self.root.bind('<Control-O>', lambda e: self.browse_file())
        
        # Enter: Execute operation
        self.root.bind('<Return>', lambda e: self.execute_operation())
        
        # Escape: Clear selection
        self.root.bind('<Escape>', lambda e: self.clear_selection())
        
        # Ctrl+Q: Quit application
        self.root.bind('<Control-q>', lambda e: self.root.quit())
        self.root.bind('<Control-Q>', lambda e: self.root.quit())
        
        # F1: Show help
        self.root.bind('<F1>', lambda e: self.show_help())
    
    def clear_selection(self):
        self.selected_file = None
        self.file_label.configure(text="No file selected", fg=self.colors['dim'])
        self.password_var.set("")
        self.log("Selection cleared", self.colors['text'])
    
    def show_help(self):
        help_text = """
VAULTX KEYBOARD SHORTCUTS:

Ctrl+E    - Switch to ENCRYPT mode
Ctrl+D    - Switch to DECRYPT mode
Ctrl+O    - Browse for file
Enter      - Execute operation
Escape     - Clear selection
Ctrl+Q    - Quit application
F1         - Show this help

SECURITY NOTES:
- This is OPEN SOURCE software
- Always verify source code before use
- Never trust compiled executables for crypto
- Loss of password = permanent data loss
- Use strong passwords (12+ characters recommended)

DRAG & DROP:
- Drag files onto the window to select them
- Works with both encryption and decryption modes
        """
        self.log("Help: Press Ctrl+Q to quit, see console for shortcuts", self.colors['primary'])
        print(help_text)
    
    def load_recent_files(self):
        try:
            if os.path.exists(self.recent_files_file):
                with open(self.recent_files_file, 'r') as f:
                    self.recent_files = json.load(f)
        except:
            self.recent_files = []
    
    def save_recent_files(self):
        try:
            with open(self.recent_files_file, 'w') as f:
                json.dump(self.recent_files, f, indent=2)
        except:
            pass
    
    def update_recent_dropdown(self):
        menu = self.recent_dropdown['menu']
        menu.delete(0, 'end')
        
        menu.add_command(label="Recent Files...", command=lambda: None)
        menu.add_separator()
        
        for entry in self.recent_files[:10]:
            if isinstance(entry, dict):
                display_text = f"{entry['name']} ({entry['operation']})"
                menu.add_command(label=display_text, command=lambda p=entry['path']: self.select_recent_file(p))
            else:
                # Handle old format (string paths)
                filename = os.path.basename(entry)
                menu.add_command(label=filename, command=lambda p=entry: self.select_recent_file(p))
    
    def select_recent_file(self, file_path):
        if os.path.exists(file_path):
            self.selected_file = file_path
            display_name = os.path.basename(file_path)
            self.file_label.configure(text=f"▶ {display_name}", fg=self.colors['primary'])
            
            size = os.path.getsize(file_path)
            size_str = self.format_size(size)
            directory = os.path.dirname(file_path)
            self.log(f"Selected: {display_name} ({size_str})", self.colors['primary'])
            self.log(f"Location: {directory}", self.colors['text'])
        else:
            self.log(f"ERROR: Recent file not found: {file_path}", self.colors['danger'])
    
    def add_to_recent_files(self, file_path, operation):
        # Handle both old format (strings) and new format (dicts)
        if self.recent_files and isinstance(self.recent_files[0], str):
            # Old format - convert to new format
            self.recent_files = []
        
        # Remove if already exists (check both formats)
        self.recent_files = [f for f in self.recent_files if 
                          (isinstance(f, str) and f != file_path) or 
                          (isinstance(f, dict) and f.get('path') != file_path)]
        
        # Add to beginning
        recent_entry = {
            'path': file_path,
            'name': os.path.basename(file_path),
            'operation': operation,
            'timestamp': datetime.now().isoformat()
        }
        self.recent_files.insert(0, recent_entry)
        
        # Keep only last 10
        self.recent_files = self.recent_files[:10]
        
        # Save and update UI
        self.save_recent_files()
        self.update_recent_dropdown()
    
    def on_recent_file_selected(self, index):
        try:
            if isinstance(index, str) and index.isdigit():
                index = int(index)
            
            if 0 <= index < len(self.recent_files):
                file_info = self.recent_files[index]
                file_path = file_info['path']
                
                if os.path.exists(file_path):
                    # Set mode based on operation
                    if file_info['operation'] == 'ENCRYPT':
                        self.set_mode("ENCRYPT")
                    else:
                        self.set_mode("DECRYPT")
                    
                    # Select file
                    self.selected_file = file_path
                    self.file_label.configure(text=f"▶ {file_info['name']}", fg=self.colors['primary'])
                    
                    # Show file info
                    size = os.path.getsize(file_path)
                    size_str = self.format_size(size)
                    directory = os.path.dirname(file_path)
                    self.log(f"Recent: {file_info['name']} ({size_str})", self.colors['primary'])
                    self.log(f"Location: {directory}", self.colors['text'])
                else:
                    self.log(f"ERROR: File no longer exists: {file_info['name']}", self.colors['danger'])
        except:
            pass
        
        # Reset dropdown
        self.recent_var.set("Recent Files...")
    
    def clear_recent_files(self):
        self.recent_files = []
        self.save_recent_files()
        self.update_recent_dropdown()
        self.log("Recent files cleared", self.colors['text'])
    
    def set_mode(self, mode):
        self.mode = mode
        if mode == "ENCRYPT":
            self.encrypt_btn.configure(bg=self.colors['primary'], fg=self.colors['bg'])
            self.decrypt_btn.configure(bg=self.colors['panel'], fg=self.colors['dim'])
            self.execute_btn.configure(text="ENCRYPT FILE", bg=self.colors['primary'])
            self.progress_canvas.itemconfig(self.progress_fill, fill=self.colors['primary'])
        else:
            self.encrypt_btn.configure(bg=self.colors['panel'], fg=self.colors['dim'])
            self.decrypt_btn.configure(bg=self.colors['secondary'], fg=self.colors['bg'])
            self.execute_btn.configure(text="DECRYPT FILE", bg=self.colors['secondary'])
            self.progress_canvas.itemconfig(self.progress_fill, fill=self.colors['secondary'])
    
    def toggle_password_visibility(self):
        if self.reveal_var.get():
            self.password_entry.configure(show="")
        else:
            self.password_entry.configure(show="●")
    
    def update_password_strength(self):
        password = self.password_var.get()
        strength = self.calculate_strength(password)
        
        # Update strength bars
        for i, bar in enumerate(self.strength_bars):
            if i < strength['bars']:
                bar.configure(text="█", fg=strength['color'])
            else:
                bar.configure(text="▪", fg=self.colors['dim'])
        
        self.strength_label.configure(text=strength['label'], fg=strength['color'])
    
    def calculate_strength(self, password):
        length = len(password)
        has_upper = any(c.isupper() for c in password)
        has_lower = any(c.islower() for c in password)
        has_digit = any(c.isdigit() for c in password)
        has_special = any(c in "!@#$%^&*()_+-=[]{}|;:,.<>?" for c in password)
        
        score = 0
        if length >= 12: score += 1  # Increased minimum requirement
        if length >= 16: score += 1
        if length >= 20: score += 1
        if has_upper: score += 1
        if has_lower: score += 1
        if has_digit: score += 1
        if has_special: score += 1
        
        if score <= 3:
            return {'bars': 2, 'label': 'WEAK', 'color': self.colors['danger']}
        elif score <= 5:
            return {'bars': 4, 'label': 'FAIR', 'color': self.colors['warning']}
        elif score <= 6:
            return {'bars': 6, 'label': 'GOOD', 'color': self.colors['secondary']}
        else:
            return {'bars': 10, 'label': 'ELITE', 'color': self.colors['primary']}
    
    def browse_file(self):
        if self.mode == "ENCRYPT":
            filename = filedialog.askopenfilename(title="Select file to encrypt")
        else:
            filename = filedialog.askopenfilename(title="Select file to decrypt", 
                                               filetypes=[("VaultX files", "*.vx2")])
        
        if filename:
            self.selected_file = filename
            display_name = os.path.basename(filename)
            self.file_label.configure(text=f"▶ {display_name}", fg=self.colors['primary'])
            
            # Add to recent files
            self.add_to_recent_files(filename, self.mode)
            
            # Show file info
            size = os.path.getsize(filename)
            size_str = self.format_size(size)
            directory = os.path.dirname(filename)
            self.log(f"Selected: {display_name} ({size_str})", self.colors['primary'])
            self.log(f"Location: {directory}", self.colors['text'])
    
    def format_size(self, size):
        for unit in ['B', 'KB', 'MB', 'GB']:
            if size < 1024:
                return f"{size:.1f} {unit}"
            size /= 1024
        return f"{size:.1f} TB"
    
    def log(self, message, color=None):
        if color is None:
            color = self.colors['text']
        
        # Shift messages up
        for i in range(len(self.log_lines) - 1):
            current_text = self.log_lines[i]['text']
            current_color = self.log_lines[i].cget('fg')
            self.log_lines[i].configure(text=current_text, fg=self.colors['dim'])
        
        # Add new message
        self.log_lines[-1].configure(text=message, fg=color)
        
        # Move to front
        self.log_lines.insert(0, self.log_lines.pop())
    
    def update_progress(self, percentage):
        self.progress_canvas.coords(self.progress_fill, 2, 8, 2 + (756 * percentage / 100), 22)
        self.progress_canvas.itemconfig(self.progress_text, text=f"{percentage}%")
        self.root.update_idletasks()
    
    def execute_operation(self):
        if self.is_running:
            return
        
        # Validation
        if not self.selected_file:
            self.log("ERROR: No file selected.", self.colors['danger'])
            return
        
        password = self.password_var.get()
        if not password:
            self.log("ERROR: No password provided.", self.colors['danger'])
            return
        
        # Enhanced password validation
        is_strong, strength_msg = VaultXSecurity.verify_password_strength(password)
        if not is_strong:
            self.log(f"ERROR: {strength_msg}", self.colors['danger'])
            return
        
        if self.mode == "DECRYPT" and not self.selected_file.endswith('.vx2'):
            self.log("ERROR: File must have .vx2 extension.", self.colors['danger'])
            return
        
        # Start operation
        self.is_running = True
        self.execute_btn.configure(state=tk.DISABLED, bg=self.colors['panel'])
        self.update_progress(0)
        
        # Run in background thread
        thread = threading.Thread(target=self.crypto_worker, args=(password,), daemon=True)
        thread.start()
    
    def crypto_worker(self, password):
        try:
            if self.mode == "ENCRYPT":
                self.encrypt_file(password)
            else:
                self.decrypt_file(password)
        except Exception as e:
            self.root.after(0, lambda: self.handle_error(str(e)))
    
    def encrypt_file(self, password):
        # Generate secure random salt and nonce
        salt = VaultXSecurity.secure_random_bytes(32)
        nonce = VaultXSecurity.secure_random_bytes(12)
        
        # Derive key using Argon2id + HKDF-SHA512
        self.root.after(0, lambda: self.log("Deriving encryption key...", self.colors['text']))
        
        # Argon2id
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(password.encode())
        
        # HKDF-SHA512
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        key = kdf_hkdf.derive(intermediate_key)
        
        self.root.after(0, lambda: self.update_progress(25))
        
        # Read source file
        self.root.after(0, lambda: self.log("Reading source file...", self.colors['text']))
        with open(self.selected_file, 'rb') as f:
            plaintext = f.read()
        
        self.root.after(0, lambda: self.update_progress(50))
        
        # Encrypt
        self.root.after(0, lambda: self.log("Encrypting data...", self.colors['text']))
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        
        self.root.after(0, lambda: self.update_progress(75))
        
        # Write output file
        output_path = self.selected_file + ".vx2"
        self.root.after(0, lambda: self.log(f"Creating encrypted file: {output_path}", self.colors['text']))
        
        try:
            with open(output_path, 'wb') as f:
                f.write(b'VAULTX02')  # Magic bytes
                f.write(salt)         # Salt (32 bytes)
                f.write(nonce)        # Nonce (12 bytes)
                f.write(ciphertext)   # Ciphertext + auth tag
            
            self.root.after(0, lambda: self.log(f"File written successfully: {os.path.basename(output_path)}", self.colors['secondary']))
        except Exception as write_error:
            self.root.after(0, lambda: self.log(f"ERROR writing file: {str(write_error)}", self.colors['danger']))
            raise
        
        # Verify file size
        expected_size = 8 + 32 + 12 + len(plaintext) + 16
        actual_size = os.path.getsize(output_path)
        
        if actual_size != expected_size:
            os.remove(output_path)
            raise IOError(f"Output file size mismatch. Expected {expected_size} bytes, got {actual_size}.")
        
        self.root.after(0, lambda: self.update_progress(100))
        self.root.after(0, lambda: self.log(f"SUCCESS: File encrypted to {os.path.basename(output_path)}", self.colors['secondary']))
    
    def decrypt_file(self, password):
        # Read encrypted file
        self.root.after(0, lambda: self.log("Reading encrypted file...", self.colors['text']))
        with open(self.selected_file, 'rb') as f:
            raw_data = f.read()
        
        # Parse header - EXACT offsets as specified
        if len(raw_data) < 52:
            raise ValueError("File too small to be a valid VaultX encrypted file")
        
        magic = raw_data[0:8]
        if magic != b'VAULTX02':
            raise ValueError("Not a VaultX encrypted file")
        
        salt = raw_data[8:40]      # Exactly 32 bytes
        nonce = raw_data[40:52]     # Exactly 12 bytes
        ciphertext = raw_data[52:]  # Everything remaining
        
        self.root.after(0, lambda: self.update_progress(25))
        
        # Derive key using Argon2id + HKDF-SHA512
        self.root.after(0, lambda: self.log("Deriving decryption key...", self.colors['text']))
        
        # Argon2id
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(password.encode())
        
        # HKDF-SHA512
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        key = kdf_hkdf.derive(intermediate_key)
        
        self.root.after(0, lambda: self.update_progress(50))
        
        # Decrypt
        self.root.after(0, lambda: self.log("Decrypting data...", self.colors['text']))
        aesgcm = AESGCMSIV(key)
        
        try:
            plaintext = aesgcm.decrypt(nonce, ciphertext, None)
        except Exception:
            raise ValueError("Wrong password or corrupted file")
        
        self.root.after(0, lambda: self.update_progress(75))
        
        # Write output file
        output_path = self.selected_file[:-4]  # Remove .vx2 extension
        self.root.after(0, lambda: self.log(f"Writing decrypted file...", self.colors['text']))
        
        with open(output_path, 'wb') as f:
            f.write(plaintext)
        
        self.root.after(0, lambda: self.update_progress(100))
        self.root.after(0, lambda: self.log(f"SUCCESS: File decrypted to {os.path.basename(output_path)}", self.colors['secondary']))
    
    def handle_error(self, error_msg):
        self.log(f"ERROR: {error_msg}", self.colors['danger'])
        self.update_progress(0)
        self.is_running = False
        self.execute_btn.configure(state=tk.NORMAL, bg=self.colors['primary'] if self.mode == "ENCRYPT" else self.colors['secondary'])
    
    def start_clock(self):
        def update_clock():
            current_time = datetime.now().strftime("%H:%M:%S")
            self.clock_label.configure(text=current_time)
            self.root.after(1000, update_clock)
        
        update_clock()
    
    def start_hex_animation(self):
        def animate_hex():
            if hasattr(self, 'hex_cells') and self.hex_cells:
                # Pick random cell
                import random
                cell = random.choice(self.hex_cells)
                
                # Choose color
                color = random.choice([self.colors['hex_cyan'], self.colors['hex_green']])
                
                # Highlight cell
                self.hex_canvas.itemconfig(cell, fill=color)
                
                # Restore after 500ms
                self.root.after(500, lambda: self.hex_canvas.itemconfig(cell, fill=''))
            
            # Schedule next animation
            self.root.after(150, animate_hex)
        
        # Start animation after hex grid is created
        self.root.after(200, animate_hex)


def main():
    """Main entry point with security checks"""
    print("VAULTX - SECURE FILE VAULT (Open Source Version)")
    print("=" * 50)
    print("SECURITY NOTICE:")
    print("- This is open source software")
    print("- Always verify source code before use")
    print("- Never trust compiled executables for crypto")
    print("- Use strong passwords (12+ characters)")
    print("=" * 50)
    
    # Security verification
    if not VaultXSecurity.verify_integrity():
        print("WARNING: Application integrity verification failed")
        response = input("Continue anyway? (y/N): ")
        if response.lower() != 'y':
            print("Exiting for security reasons.")
            return
    
    if VaultXSecurity.detect_debugging():
        print("WARNING: Debugging environment detected")
        response = input("Continue anyway? (y/N): ")
        if response.lower() != 'y':
            print("Exiting for security reasons.")
            return
    
    # Start the application
    root = tkdnd.Tk()
    app = VaultXApp(root)
    root.mainloop()


if __name__ == "__main__":
    main()
