#!/usr/bin/env bash
set -e

# RoninKB installer for macOS and Linux.
# Usage: curl -fsSL https://raw.githubusercontent.com/kidneyweakx/RoninKB/main/install.sh | sh

REPO="kidneyweakx/RoninKB"
INSTALL_DIR="${RONINKB_INSTALL_DIR:-/usr/local/bin}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$OS" in
  darwin) target="universal-apple-darwin" ;;
  linux)
    case "$ARCH" in
      x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
      aarch64|arm64)
        echo "Linux aarch64 release binaries are not published — kanata"
        echo "upstream does not ship that arch and our daemon bundles it."
        echo "Build from source instead:"
        echo "  git clone https://github.com/kidneyweakx/RoninKB"
        echo "  cd RoninKB && cargo build --release -p hhkb-cli -p hhkb-daemon \\"
        echo "    --features hhkb-core/hidapi-backend"
        exit 1
        ;;
      *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

# Get latest release tag.
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep tag_name | cut -d'"' -f4)
if [ -z "$LATEST" ]; then
  echo "Failed to fetch latest release. Falling back to v0.1.0"
  LATEST="v0.1.0"
fi

echo "Installing RoninKB $LATEST for $target"

URL="https://github.com/$REPO/releases/download/$LATEST/roninKB-$LATEST-$target.tar.gz"
TMP=$(mktemp -d)

curl -fsSL "$URL" | tar -xz -C "$TMP"
EXTRACTED="$TMP/roninKB-$LATEST-$target"

if [ ! -d "$EXTRACTED/bin" ]; then
  echo "Unexpected archive layout"
  exit 1
fi

mkdir -p "$INSTALL_DIR"
if [ -w "$INSTALL_DIR" ]; then
  install -m 755 "$EXTRACTED/bin/hhkb" "$INSTALL_DIR/hhkb"
  install -m 755 "$EXTRACTED/bin/hhkb-daemon" "$INSTALL_DIR/hhkb-daemon"
else
  sudo install -m 755 "$EXTRACTED/bin/hhkb" "$INSTALL_DIR/hhkb"
  sudo install -m 755 "$EXTRACTED/bin/hhkb-daemon" "$INSTALL_DIR/hhkb-daemon"
fi

echo "Installed to $INSTALL_DIR"
echo
echo "Next steps:"
echo "  1. Start the daemon:  hhkb-daemon &"
echo "  2. Open the UI:       http://127.0.0.1:7331/"
echo "  3. Try the CLI:       hhkb list"
echo
echo "Linux users: copy $EXTRACTED/install/linux/99-roninKB.rules to /etc/udev/rules.d/"
echo "then run: sudo udevadm control --reload-rules && sudo udevadm trigger"
