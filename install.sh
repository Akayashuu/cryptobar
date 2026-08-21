#!/usr/bin/env bash
# Installation de cryptobar : binaire, configuration et fichiers d'intégration.
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"

info() { printf '\033[1;34m::\033[0m %s\n' "$1"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$1" >&2; }

command -v cargo >/dev/null || { warn "cargo est requis"; exit 1; }

info "compilation en mode release"
cargo build --release --locked

info "installation du binaire dans $PREFIX/bin"
install -Dm755 target/release/cryptobar "$PREFIX/bin/cryptobar"

info "écriture de la configuration par défaut si elle est absente"
"$PREFIX/bin/cryptobar" config init 2>/dev/null || info "configuration déjà présente, conservée"

# Les fichiers d'intégration ne sont jamais fusionnés automatiquement : ils
# sont déposés à côté pour que l'utilisateur les copie dans sa propre config.
info "dépôt des exemples d'intégration"
install -Dm644 waybar/crypto.jsonc "$CONFIG_HOME/waybar/cryptobar.jsonc"
install -Dm644 waybar/style.css "$CONFIG_HOME/waybar/cryptobar.css"
install -Dm644 hypr/cryptobar.conf "$CONFIG_HOME/hypr/cryptobar.conf"

cat <<'MSG'

Installation terminée. Il reste à brancher, selon votre environnement :

  0. Quickshell (dots-hyprland « ii ») — si c'est votre barre :
       ./quickshell/install.sh && qs -c ii kill && qs -c ii -d

  1. Waybar — dans ~/.config/waybar/config.jsonc, copiez le bloc
     "custom/crypto" de ~/.config/waybar/cryptobar.jsonc et ajoutez
     "custom/crypto" à modules-right (ou left/center).
     Puis dans ~/.config/waybar/style.css :
       @import "cryptobar.css";

  2. Hyprland — dans ~/.config/hypr/hyprland.conf :
       source = ~/.config/hypr/cryptobar.conf

Assurez-vous que ~/.local/bin est dans votre PATH, puis :
  cryptobar bar

Pour suivre un portefeuille, ajoutez une ADRESSE PUBLIQUE depuis la fenêtre
(cryptobar popup, section « Adresses suivies »). cryptobar n'accepte jamais
une clé privée ni une phrase de récupération.
MSG
