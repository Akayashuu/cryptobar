#!/usr/bin/env bash
# cryptobar installation: binary, configuration and integration files.
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"

info() { printf '\033[1;34m::\033[0m %s\n' "$1"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$1" >&2; }

command -v cargo >/dev/null || { warn "cargo is required"; exit 1; }

info "building in release mode"
cargo build --release --locked

info "installing the binary into $PREFIX/bin"
install -Dm755 target/release/cryptobar "$PREFIX/bin/cryptobar"

info "writing the default configuration when missing"
"$PREFIX/bin/cryptobar" config init 2>/dev/null || info "configuration already present, kept"

# The integration files are never merged automatically: they are dropped
# alongside so the user copies them into their own configuration.
info "dropping the integration samples"
install -Dm644 waybar/crypto.jsonc "$CONFIG_HOME/waybar/cryptobar.jsonc"
install -Dm644 waybar/style.css "$CONFIG_HOME/waybar/cryptobar.css"
install -Dm644 hypr/cryptobar.conf "$CONFIG_HOME/hypr/cryptobar.conf"

cat <<'MSG'

Installation complete. What is left to wire up, depending on your setup:

  0. Quickshell (dots-hyprland "ii") — if that is your bar:
       ./quickshell/install.sh && qs -c ii kill && qs -c ii -d

  1. Waybar — in ~/.config/waybar/config.jsonc, copy the "custom/crypto"
     block from ~/.config/waybar/cryptobar.jsonc and add "custom/crypto" to
     modules-right (or left/center).
     Then in ~/.config/waybar/style.css:
       @import "cryptobar.css";

  2. Hyprland — in ~/.config/hypr/hyprland.conf:
       source = ~/.config/hypr/cryptobar.conf

Make sure ~/.local/bin is in your PATH, then:
  cryptobar bar

To watch a wallet, add a PUBLIC ADDRESS from the window (cryptobar popup,
"Watched addresses" section). cryptobar never accepts a private key nor a
recovery phrase.
MSG
