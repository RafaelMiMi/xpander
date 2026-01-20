# Xpander

A text expansion daemon for Linux (Wayland/X11) written in Rust. Type trigger phrases and have them automatically expanded to full text snippets.

## Features

- **Text Expansion**: Define triggers that expand to longer text snippets
- **Variables**: Use dynamic variables like `{{date}}`, `{{time}}`, `{{clipboard}}`, `{{env:VAR}}`, `{{shell:cmd}}`, `{{uuid}}`, `{{random:N}}`
- **Cursor Positioning**: Place cursor at specific position with `$|$` marker
- **System Tray**: Easy access to enable/disable, reload config, and open settings
- **GTK4 GUI**: Visual snippet editor for managing your expansions
- **Hot Reload**: Config file changes are automatically detected
- **Case Propagation**: Match the case of your trigger in the replacement

## Requirements

- Linux with Wayland or X11
- ydotool (for simulating keyboard input)
- GTK4 libraries
- Rust toolchain (for building)

## Installation

```bash
# Clone the repository
git clone https://github.com/RafaelMiMi/xpander.git
cd xpander

# Run the install script
./install.sh
```

The install script will:
1. Install ydotool
2. Add your user to the `input` group
3. Install GTK4 development libraries
4. Build and install xpander to `~/.local/bin`
5. Create default config at `~/.config/xpander/config.yaml`
6. Set up systemd user service

**Note**: You need to log out and back in after installation for the `input` group membership to take effect.

## Usage

### Start the daemon
```bash
xpander
```

### Enable autostart
```bash
systemctl --user enable xpander
systemctl --user start xpander
```

### Open the GUI
```bash
xpander --gui
```

Or right-click the system tray icon and select "Open Configuration..."

## Configuration

Edit `~/.config/xpander/config.yaml`:

```yaml
settings:
  enabled: true
  keystroke_delay_ms: 12

snippets:
  - trigger: ";email"
    replace: "myemail@example.com"
    label: "Email address"

  - trigger: ";date"
    replace: "{{date}}"
    label: "Current date"

  - trigger: ";sig"
    replace: |
      Best regards,
      Your Name
    label: "Email signature"
```

### Available Variables

| Variable | Description |
|----------|-------------|
| `{{date}}` | Current date (YYYY-MM-DD) |
| `{{date:FORMAT}}` | Date with custom strftime format |
| `{{time}}` | Current time (HH:MM:SS) |
| `{{datetime}}` | Date and time |
| `{{clipboard}}` | Clipboard contents |
| `{{env:VAR}}` | Environment variable |
| `{{shell:cmd}}` | Shell command output |
| `{{uuid}}` | Random UUID |
| `{{random:N}}` | Random N-digit number |

### Snippet Options

| Option | Description |
|--------|-------------|
| `trigger` | The text that triggers expansion |
| `replace` | The replacement text |
| `label` | Optional description |
| `enabled` | Enable/disable this snippet |
| `propagate_case` | Match trigger case in replacement |
| `word_boundary` | Only match at word boundaries |
| `cursor_position` | Move cursor to `$|$` marker |

## License

MIT
