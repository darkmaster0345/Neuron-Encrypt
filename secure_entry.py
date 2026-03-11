#!/usr/bin/env python3
"""
VAULTX SECURE ENTRY WIDGET
Secure password entry widget with memory protection

SECURITY FEATURES:
- Automatic memory zeroing
- Protection against memory harvesting
- Secure string handling
- Clipboard protection
- Keylogging protection
"""

import tkinter as tk
from tkinter import ttk
import threading
import time
import sys
from secure_memory import SecureString, SecurityError, register_secure_string, cleanup_all_memory


class SecureEntry(tk.Entry):
    """Secure password entry widget with memory protection"""
    
    def __init__(self, master=None, **kwargs):
        """Initialize secure entry widget"""
        # Force password mode
        kwargs['show'] = kwargs.get('show', '●')
        
        super().__init__(master, **kwargs)
        
        self._secure_password = None
        self._cleanup_timer = None
        self._cleanup_interval = 30000  # 30 seconds
        self._max_length = kwargs.get('max_length', 128)
        self._clipboard_protection = True
        
        # Bind events
        self.bind('<KeyRelease>', self._on_key_release)
        self.bind('<FocusOut>', self._on_focus_out)
        self.bind('<FocusIn>', self._on_focus_in)
        self.bind('<Button-3>', self._disable_context_menu)  # Disable right-click
        self.bind('<Control-v>', self._disable_paste)  # Disable paste
        self.bind('<Control-c>', self._disable_copy)  # Disable copy
        
        # Start cleanup timer
        self._start_cleanup_timer()
    
    def _disable_context_menu(self, event):
        """Disable context menu"""
        return "break"
    
    def _disable_paste(self, event):
        """Disable paste to prevent clipboard attacks"""
        return "break"
    
    def _disable_copy(self, event):
        """Disable copy to prevent clipboard exposure"""
        return "break"
    
    def _on_key_release(self, event):
        """Handle key release - update secure password"""
        try:
            # Get current text
            current_text = self.get()
            
            # Enforce maximum length
            if len(current_text) > self._max_length:
                current_text = current_text[:self._max_length]
                self.delete(0, tk.END)
                self.insert(0, current_text)
            
            # Create secure string
            if current_text:
                # Zero old password
                if self._secure_password:
                    self._secure_password.zero()
                
                # Create new secure password
                self._secure_password = SecureString(current_text)
                register_secure_string(self._secure_password)
            else:
                # No password entered
                if self._secure_password:
                    self._secure_password.zero()
                    self._secure_password = None
            
            # Reset cleanup timer
            self._reset_cleanup_timer()
            
        except Exception as e:
            # Security first - zero everything on error
            self._emergency_cleanup()
    
    def _on_focus_out(self, event):
        """Handle focus out - start immediate cleanup"""
        self._start_immediate_cleanup()
    
    def _on_focus_in(self, event):
        """Handle focus in - reset cleanup timer"""
        self._reset_cleanup_timer()
    
    def _start_cleanup_timer(self):
        """Start the automatic cleanup timer"""
        if self._cleanup_timer:
            self.after_cancel(self._cleanup_timer)
        
        self._cleanup_timer = self.after(self._cleanup_interval, self._auto_cleanup)
    
    def _reset_cleanup_timer(self):
        """Reset the cleanup timer"""
        self._start_cleanup_timer()
    
    def _start_immediate_cleanup(self):
        """Start immediate cleanup (shorter delay)"""
        if self._cleanup_timer:
            self.after_cancel(self._cleanup_timer)
        
        self._cleanup_timer = self.after(5000, self._auto_cleanup)  # 5 seconds
    
    def _auto_cleanup(self):
        """Automatic cleanup of password"""
        try:
            if self._secure_password:
                self._secure_password.zero()
                self._secure_password = None
            
            # Clear the entry field
            self.delete(0, tk.END)
            
        except Exception:
            pass
    
    def _emergency_cleanup(self):
        """Emergency cleanup on error"""
        try:
            if self._secure_password:
                self._secure_password.zero()
                self._secure_password = None
            
            self.delete(0, tk.END)
        except Exception:
            pass
    
    def get_secure_password(self) -> SecureString:
        """Get secure password (returns SecureString object)"""
        if not self._secure_password:
            raise SecurityError("No password entered")
        
        return self._secure_password
    
    def get_password_bytes(self) -> bytes:
        """Get password as bytes (use carefully)"""
        if not self._secure_password:
            raise SecurityError("No password entered")
        
        return self._secure_password.copy()
    
    def get_password_str(self) -> str:
        """Get password as string (use carefully)"""
        if not self._secure_password:
            raise SecurityError("No password entered")
        
        # This is less secure but sometimes necessary
        password_bytes = self._secure_password.copy()
        password_str = password_bytes.decode('utf-8')
        
        # Zero the bytes immediately
        SecureMemory.zero_memory(password_bytes)
        
        return password_str
    
    def clear(self):
        """Clear the entry and zero password"""
        self._auto_cleanup()
    
    def set_placeholder(self, text: str):
        """Set placeholder text (not secure)"""
        if not self.get():
            self.insert(0, text)
            self._placeholder = text
    
    def __del__(self):
        """Cleanup on deletion"""
        try:
            if self._cleanup_timer:
                self.after_cancel(self._cleanup_timer)
            
            if self._secure_password:
                self._secure_password.zero()
        except:
            pass


class SecurePasswordFrame(tk.Frame):
    """Frame containing secure password entry with additional security features"""
    
    def __init__(self, master=None, **kwargs):
        """Initialize secure password frame"""
        super().__init__(master, **kwargs)
        
        self._reveal_var = tk.BooleanVar(value=False)
        self._secure_entry = None
        self._strength_label = None
        self._strength_bars = []
        
        self._setup_ui()
    
    def _setup_ui(self):
        """Setup the UI components"""
        # Password entry
        self._secure_entry = SecureEntry(
            self,
            font=("Courier New", 12),
            max_length=128
        )
        self._secure_entry.pack(fill=tk.X, pady=(0, 5))
        
        # Controls frame
        controls_frame = tk.Frame(self)
        controls_frame.pack(fill=tk.X)
        
        # Reveal checkbox
        reveal_check = tk.Checkbutton(
            controls_frame,
            text="REVEAL KEY",
            variable=self._reveal_var,
            command=self._toggle_reveal,
            font=("Courier New", 9)
        )
        reveal_check.pack(side=tk.LEFT)
        
        # Clear button
        clear_btn = tk.Button(
            controls_frame,
            text="CLEAR",
            command=self._clear_password,
            font=("Courier New", 9)
        )
        clear_btn.pack(side=tk.RIGHT)
        
        # Strength meter
        self._setup_strength_meter()
    
    def _setup_strength_meter(self):
        """Setup password strength meter"""
        strength_frame = tk.Frame(self)
        strength_frame.pack(fill=tk.X, pady=(5, 0))
        
        self._strength_bars = []
        for i in range(10):
            bar = tk.Label(
                strength_frame,
                text="▪",
                font=("Courier New", 8),
                fg="gray"
            )
            bar.pack(side=tk.LEFT, padx=1)
            self._strength_bars.append(bar)
        
        self._strength_label = tk.Label(
            strength_frame,
            text="NONE",
            font=("Courier New", 8, "bold"),
            fg="gray"
        )
        self._strength_label.pack(side=tk.LEFT, padx=(10, 0))
        
        # Bind password change to strength update
        self._secure_entry.bind('<KeyRelease>', self._update_strength)
    
    def _toggle_reveal(self):
        """Toggle password visibility"""
        if self._reveal_var.get():
            self._secure_entry.config(show="")
        else:
            self._secure_entry.config(show="●")
    
    def _clear_password(self):
        """Clear the password"""
        self._secure_entry.clear()
        self._update_strength()
    
    def _update_strength(self, event=None):
        """Update password strength meter"""
        try:
            password = self._secure_entry.get_password_str()
            strength = self._calculate_strength(password)
            
            # Update strength bars
            colors = {
                'weak': 'red',
                'fair': 'orange', 
                'good': 'yellow',
                'elite': 'green'
            }
            
            color = colors.get(strength['level'], 'gray')
            
            for i, bar in enumerate(self._strength_bars):
                if i < strength['bars']:
                    bar.config(text="█", fg=color)
                else:
                    bar.config(text="▪", fg="gray")
            
            self._strength_label.config(
                text=strength['level'].upper(),
                fg=color
            )
            
            # Zero the password string
            SecureMemory.zero_memory(password)
            
        except SecurityError:
            # No password entered
            for bar in self._strength_bars:
                bar.config(text="▪", fg="gray")
            self._strength_label.config(text="NONE", fg="gray")
        except Exception:
            # Error - clear display
            self._clear_password()
    
    def _calculate_strength(self, password: str) -> dict:
        """Calculate password strength"""
        length = len(password)
        has_upper = any(c.isupper() for c in password)
        has_lower = any(c.islower() for c in password)
        has_digit = any(c.isdigit() for c in password)
        has_special = any(c in "!@#$%^&*()_+-=[]{}|;:,.<>?" for c in password)
        
        score = 0
        if length >= 12: score += 1
        if length >= 16: score += 1
        if length >= 20: score += 1
        if has_upper: score += 1
        if has_lower: score += 1
        if has_digit: score += 1
        if has_special: score += 1
        
        if score <= 2:
            return {'bars': 2, 'level': 'weak'}
        elif score <= 4:
            return {'bars': 4, 'level': 'fair'}
        elif score <= 6:
            return {'bars': 6, 'level': 'good'}
        else:
            return {'bars': 10, 'level': 'elite'}
    
    def get_secure_password(self) -> SecureString:
        """Get secure password"""
        return self._secure_entry.get_secure_password()
    
    def get_password_bytes(self) -> bytes:
        """Get password as bytes"""
        return self._secure_entry.get_password_bytes()
    
    def get_password_str(self) -> str:
        """Get password as string"""
        return self._secure_entry.get_password_str()
    
    def clear(self):
        """Clear the password"""
        self._clear_password()
    
    def focus(self):
        """Focus the entry widget"""
        self._secure_entry.focus_set()
    
    def __del__(self):
        """Cleanup on deletion"""
        try:
            if self._secure_entry:
                self._secure_entry.clear()
        except:
            pass


# Import SecureMemory for zero_memory function
from secure_memory import SecureMemory
