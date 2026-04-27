class Roninkb < Formula
  desc "Open-source HHKB Professional Hybrid keyboard configuration tool"
  homepage "https://github.com/kidneyweakx/RoninKB"
  version "0.1.0"
  license "MIT"

  on_macos do
    url "https://github.com/kidneyweakx/RoninKB/releases/download/v#{version}/roninKB-v#{version}-universal-apple-darwin.tar.gz"
    sha256 "71d5e8f0bafccd9eef3ab40d3555a9528352296b576ede9aba4b8e302ebe1266"
  end

  on_linux do
    # Linux aarch64 not shipped: kanata upstream has no arm64 Linux binary
    # and we bundle kanata. Linux ARM users can build from source.
    on_intel do
      url "https://github.com/kidneyweakx/RoninKB/releases/download/v#{version}/roninKB-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "59779a136352bce209b9ceecbbeaa1b9ea6dc5e7a7bf4ac505374af81ff9d29e"
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
