#!/bin/bash
# Xpander Installation Script
# This script installs xpander and sets up the required dependencies

set -e

echo "=== Xpander Installation ==="
echo

# Check if running as root
if [ "$EUID" -eq 0 ]; then
    echo "Please do not run this script as root."
    echo "It will ask for sudo when needed."
    exit 1
fi

# Detect distribution
if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO=$ID
else
    echo "Could not detect Linux distribution"
    exit 1
fi

echo "Detected distribution: $DISTRO"
echo

# Install ydotool
echo "Step 1: Installing ydotool..."
case $DISTRO in
    ubuntu|debian|pop|linuxmint|zorin)
        #  sudo apt update
        sudo apt install -y ydotool
        ;;
    fedora)
        sudo dnf install -y ydotool
        ;;
    arch|manjaro|endeavouros)
        sudo pacman -S --noconfirm ydotool
        ;;
    opensuse*)
        sudo zypper install -y ydotool
        ;;
    *)
        echo "Unknown distribution. Please install ydotool manually."
        echo "Then run this script again."
        exit 1
        ;;
esac

echo "ydotool installed successfully"
echo

# Enable and start ydotoold service (if available)
# Step 2: Enabling ydotoold service
echo "Step 2: Configuring ydotoold service..."

# Stop and disable any existing ydotool (standard) service to avoid conflicts
if systemctl list-unit-files | grep -q "ydotool.service"; then
    echo "Disabling conflicting standard ydotool.service..."
    sudo systemctl stop ydotool.service || true
    sudo systemctl disable ydotool.service || true
fi

if command -v ydotoold &> /dev/null; then
    # Create our custom service file for ydotoold
    sudo tee /etc/systemd/system/ydotoold.service > /dev/null << 'SVCEOF'
[Unit]
Description=ydotool daemon
After=multi-user.target

[Service]
Type=simple
ExecStart=/usr/bin/ydotoold --socket-path=/tmp/.ydotool_socket --socket-perm=0666
Restart=on-failure

[Install]
WantedBy=multi-user.target
SVCEOF

    sudo systemctl daemon-reload
    sudo systemctl enable ydotoold
    sudo systemctl restart ydotoold
    echo "Created and started ydotoold.service"
else
    echo "ydotool 0.1.x detected - configuring uinput permissions"
    
    # Create udev rule to allow input group to write to /dev/uinput
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/80-uinput.rules > /dev/null
    
    # Reload rules
    sudo udevadm control --reload-rules
    sudo udevadm trigger --sysname-match=uinput
    
    # Also verify/fix permissions immediately just in case
    if [ -e /dev/uinput ]; then
        sudo chgrp input /dev/uinput
        sudo chmod 0660 /dev/uinput
    fi
    
    echo "Configured /dev/uinput permissions for input group"
fi
echo

# Add user to input group
echo "Step 3: Adding user to input group..."
if groups $USER | grep -q '\binput\b'; then
    echo "User is already in the input group"
else
    sudo usermod -aG input $USER
    echo "Added $USER to input group"
    echo "NOTE: You need to log out and back in for this to take effect!"
fi
echo

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo "Step 4: Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "Step 4: Rust is already installed"
fi
echo

# Install GTK4 development libraries
echo "Step 5: Installing GTK4 development libraries..."
case $DISTRO in
    ubuntu|debian|pop|linuxmint|zorin)
        sudo apt install -y libgtk-4-dev libdbus-1-dev pkg-config
        ;;
    fedora)
        sudo dnf install -y gtk4-devel dbus-devel
        ;;
    arch|manjaro|endeavouros)
        sudo pacman -S --noconfirm gtk4 dbus
        ;;
    opensuse*)
        sudo zypper install -y gtk4-devel dbus-1-devel
        ;;
esac
echo

# Build xpander
echo "Step 6: Building xpander..."
cargo build --release

echo "Build complete!"
echo

# Install binary
echo "Step 7: Installing xpander to system..."
INSTALL_DIR="/usr/local/bin"

# Stop existing service/process if running
echo "Stopping existing xpander processes..."
systemctl --user stop xpander.service 2>/dev/null || true
pkill -u "$USER" -x xpander 2>/dev/null || true

# Copy new binary
echo "Copying binary to $INSTALL_DIR..."
sudo cp -f target/release/xpander "$INSTALL_DIR/"
sudo chmod +x "$INSTALL_DIR/xpander"

echo "Installed to $INSTALL_DIR/xpander"
echo

# Create default config
echo "Step 8: Creating default configuration..."
CONFIG_DIR="$HOME/.config/xpander"
mkdir -p "$CONFIG_DIR"

if [ ! -f "$CONFIG_DIR/config.yaml" ]; then
    cp config.example.yaml "$CONFIG_DIR/config.yaml"
    echo "Created $CONFIG_DIR/config.yaml"
else
    echo "Config file already exists, not overwriting"
fi
echo

# Create systemd user service
echo "Step 9: Creating systemd user service..."
SYSTEMD_DIR="$HOME/.config/systemd/user"
mkdir -p "$SYSTEMD_DIR"

cat > "$SYSTEMD_DIR/xpander.service" << EOF
[Unit]
Description=Xpander Text Expansion Daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/xpander
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
echo "Created systemd user service"
echo

# Create desktop file
echo "Step 10: Creating desktop entry..."
APPS_DIR="$HOME/.local/share/applications"
mkdir -p "$APPS_DIR"

cat > "$APPS_DIR/xpander.desktop" << EOF
[Desktop Entry]
Type=Application
Name=Xpander
Comment=Text Expansion for Linux
Exec=$INSTALL_DIR/xpander
Icon=input-keyboard
Terminal=false
Categories=Utility;
StartupNotify=false
X-GNOME-Autostart-enabled=true
EOF

echo "Created desktop entry"
echo

echo "=== Installation Complete ==="
echo
echo "To start xpander now:"
echo "  xpander"
echo
echo "To enable autostart on login:"
echo "  systemctl --user enable xpander"
echo "  systemctl --user start xpander"
echo
echo "Configuration file: $CONFIG_DIR/config.yaml"
echo
echo "IMPORTANT: If you were just added to the 'input' group,"
echo "you need to log out and back in for keyboard monitoring to work."
echo
