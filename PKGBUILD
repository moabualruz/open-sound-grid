# Maintainer: Open Sound Grid contributors
pkgname=open-sound-grid
pkgver=0.1.0
pkgrel=1
pkgdesc='Professional audio matrix mixer for Linux — route apps across multiple output mixes'
arch=('x86_64')
url='https://github.com/moabualruz/open-sound-grid'
license=('CC-BY-NC-SA-4.0')
depends=('libpulse' 'dbus')
makedepends=('cargo' 'pkg-config')
optdepends=('pipewire-pulse: PipeWire compatibility layer')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
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
  cargo build --frozen --release
}

check() {
  cd "$pkgname-$pkgver"
  export RUSTUP_TOOLCHAIN=stable
  cargo test --frozen
}

package() {
  cd "$pkgname-$pkgver"
  install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
  install -Dm644 "assets/open-sound-grid.desktop" "$pkgdir/usr/share/applications/$pkgname.desktop"
  install -Dm644 "assets/icon.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/$pkgname.svg"
  install -Dm644 LICENSE.md "$pkgdir/usr/share/licenses/$pkgname/LICENSE.md"
}
