# Maintainer: Akayashuu <https://github.com/Akayashuu>
pkgname=cryptobar
pkgver=0.1.0
pkgrel=1
pkgdesc="Cours de cryptomonnaies pour Waybar, le terminal et le bureau"
arch=('x86_64' 'aarch64')
url="https://github.com/Akayashuu/cryptobar"
license=('MIT')
depends=('gtk4')
makedepends=('cargo')
optdepends=('waybar: module de barre de tâches'
            'hyprland: règles de fenêtre fournies')
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
}
