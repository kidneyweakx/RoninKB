class Roninkb < Formula
  desc "Open-source HHKB Professional Hybrid keyboard configuration tool"
  homepage "https://github.com/kidneyweakx/RoninKB"
  version "0.1.1"
  license "MIT"

  on_macos do
    url "https://github.com/kidneyweakx/RoninKB/releases/download/v#{version}/roninKB-v#{version}-universal-apple-darwin.tar.gz"
    sha256 "f2530061c42f1b7d44b9af43d032590af6b754e2f05aa1925920acae2d7da511"

    # v0.2.0+ default macOS path uses the native CGEventTap backend; no
    # third-party driver required. Power users who want DriverKit-grade
    # tap-hold can opt into the Kanata backend via the UI, which depends on
    # Karabiner-DriverKit-VirtualHIDDevice (shipped with Karabiner-Elements).
    # We can't `depends_on cask: ...`
    # from a third-party tap (Homebrew rejects it as an unsupported special
    # dependency), so the caveats below walk users through the one-line cask
    # install. The daemon's /kanata/status endpoint surfaces driver_activated
    # so the web UI also surfaces the missing-driver state interactively.
  end

  on_linux do
    # Linux aarch64 not shipped: kanata upstream has no arm64 Linux binary
    # and we bundle kanata. Linux ARM users can build from source.
    on_intel do
      url "https://github.com/kidneyweakx/RoninKB/releases/download/v#{version}/roninKB-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "6c244237f2ca39e2bd96ec9a6762dc111826f44b49ff0a72f4ef88dc26e13071"
    end
  end

  def install
    bin.install "bin/hhkb"
    bin.install "bin/hhkb-daemon"
    pkgshare.install "install"
    doc.install "README.md" if File.exist?("README.md")
  end

  service do
    run [opt_bin/"hhkb-daemon"]
    keep_alive true
    log_path var/"log/roninKB-daemon.log"
    error_log_path var/"log/roninKB-daemon.err.log"
    environment_variables RUST_LOG: "info"
  end

  def caveats
    <<~EOS
      RoninKB Daemon serves the web UI at:
        http://127.0.0.1:7331/

      To start the daemon now and at login:
        brew services start roninkb

      To start it just for this session:
        hhkb-daemon

      To use the CLI:
        hhkb list
        hhkb info
        hhkb dump

      macOS first-run setup:
        1. Open the web UI. Settings → "Software backend" lists every backend
           the daemon detects. The native backend (default in v0.2.0) needs
           only Input Monitoring + Accessibility — both prompted in-app.
        2. Power users wanting sub-100 ms tap-hold can install Karabiner-Elements
           and switch the active backend to "Kanata" in the same panel:
             brew install --cask karabiner-elements
           The Karabiner sysext setup is fully driven from the UI's
           "Activate driver" / "Open System Settings" buttons — no manual
           hunting through Settings panes required.

      Linux users: install the udev rule first so the daemon can talk to hidraw:
        sudo cp #{pkgshare}/install/linux/99-roninKB.rules /etc/udev/rules.d/
        sudo udevadm control --reload-rules && sudo udevadm trigger
    EOS
  end

  test do
    assert_match "hhkb", shell_output("#{bin}/hhkb --help")
    assert_match "hhkb-daemon", shell_output("#{bin}/hhkb-daemon --help 2>&1", 0)
  end
end
