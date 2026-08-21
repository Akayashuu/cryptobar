# Maintainer: Akayashuu <https://github.com/Akayashuu>
pkgname=cryptobar
pkgver=0.2.0
pkgrel=1
pkgdesc="Cours de cryptomonnaies pour Quickshell/Waybar, le terminal et le bureau"
arch=('x86_64' 'aarch64')
url="https://github.com/Akayashuu/cryptobar"
license=('MIT')
depends=('gtk4')
makedepends=('cargo')
optdepends=('quickshell: widgets de barre fournis'
            'waybar: module de barre de tâches'
            'hyprland: règles de fenêtre fournies'
            'libnotify: notifications des alertes de prix')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
  cd "$pkgname-$pkgver"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd "$pkgname-$pkgver"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  cargo build --frozen --release --all-features
}

check() {
  cd "$pkgname-$pkgver"
  export RUSTUP_TOOLCHAIN=stable
  cargo test --frozen --all-features
}

package() {
  cd "$pkgname-$pkgver"
  install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
  install -Dm644 waybar/crypto.jsonc "$pkgdir/usr/share/doc/$pkgname/waybar/crypto.jsonc"
  install -Dm644 waybar/style.css "$pkgdir/usr/share/doc/$pkgname/waybar/style.css"
  install -Dm644 hypr/cryptobar.conf "$pkgdir/usr/share/doc/$pkgname/hypr/cryptobar.conf"
  install -Dm644 CHANGELOG.md "$pkgdir/usr/share/doc/$pkgname/CHANGELOG.md"
  # Les widgets Quickshell sont copiés tels quels : l'utilisateur les branche
  # dans sa configuration avec quickshell/install.sh.
  cp -r quickshell "$pkgdir/usr/share/doc/$pkgname/quickshell"
  find "$pkgdir/usr/share/doc/$pkgname/quickshell" -type f -exec chmod 644 {} +
  chmod 755 "$pkgdir/usr/share/doc/$pkgname/quickshell/install.sh"
}
