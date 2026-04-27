class Roninkb < Formula
  desc "Open-source HHKB Professional Hybrid keyboard configuration tool"
  homepage "https://github.com/kidneyweakx/RoninKB"
  version "0.1.0"
  license "MIT"

  on_macos do
    url "https://github.com/kidneyweakx/RoninKB/releases/download/v#{version}/roninKB-v#{version}-universal-apple-darwin.tar.gz"
    sha256 "a2a3acdfb0443ff5bda45ca20dfa9e61cbc8eea99fbe73de654a9526316b5616"
  end

  on_linux do
    # Linux aarch64 not shipped: kanata upstream has no arm64 Linux binary
    # and we bundle kanata. Linux ARM users can build from source.
    on_intel do
      url "https://github.com/kidneyweakx/RoninKB/releases/download/v#{version}/roninKB-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "e2924a77094a8143c47fcda1974b6bdd6ec7384c42acb87837a6794b0f861822"
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
