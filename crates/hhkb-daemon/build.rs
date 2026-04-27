//! Build-time prerequisite checks.
//!
//! ## `embedded-ui` feature
//! When active, `rust-embed` reads the compiled hhkb-app bundle from
//! `apps/hhkb-app/dist`. Fails with a friendly message if it's missing.
//!
//! ## `bundled-kanata` feature
//! Downloads the appropriate kanata binary from GitHub Releases and embeds it
//! at `OUT_DIR/kanata-bundle` so `kanata.rs` can `include_bytes!` it. The
//! download is skipped when a cached copy for the same version already exists.
//! Override the version via the `KANATA_VERSION` environment variable.

fn main() {
    println!("cargo:rerun-if-changed=../../apps/hhkb-app/dist");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KANATA_VERSION");

    check_embedded_ui();
    bundle_kanata();
}

fn check_embedded_ui() {
    if std::env::var("CARGO_FEATURE_EMBEDDED_UI").is_err() {
        return;
    }
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dist = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR should have at least two ancestors")
        .join("apps/hhkb-app/dist");

    if !dist.exists() || !dist.join("index.html").exists() {
        panic!(
            "embedded-ui feature requires the hhkb-app frontend to be built first.\n\n\
             Run:\n  cd apps/hhkb-app && npm install && npm run build\n\n\
             Expected file: {}\n",
            dist.join("index.html").display()
        );
    }
}

fn bundle_kanata() {
    if std::env::var("CARGO_FEATURE_BUNDLED_KANATA").is_err() {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let bundle_path = std::path::Path::new(&out_dir).join("kanata-bundle");
    let version_file = std::path::Path::new(&out_dir).join("kanata-bundle.version");
    let version = std::env::var("KANATA_VERSION").unwrap_or_else(|_| "v1.11.0".to_string());

    // Skip download if we already have the right version cached in OUT_DIR.
    if bundle_path.exists() {
        if let Ok(v) = std::fs::read_to_string(&version_file) {
            if v.trim() == version {
                println!(
                    "cargo:warning=bundled-kanata: cached {} found, skipping download",
                    version
                );
                return;
            }
        }
    }

    let target = std::env::var("TARGET").unwrap();
    let (zip_name, bin_name): (&str, &str) = if target.contains("aarch64-apple-darwin") {
        ("macos-binaries-arm64.zip", "kanata_macos_arm64")
    } else if target.contains("x86_64-apple-darwin") {
        ("macos-binaries-x64.zip", "kanata_macos_x64")
    } else if target.contains("x86_64-unknown-linux") {
        ("linux-binaries-x64.zip", "kanata_linux_x64")
    } else if target.contains("x86_64-pc-windows") {
        // kanata's Windows zip ships 8 variants. We want TTY (no GUI tray)
        // because the daemon spawns kanata as a child and pipes stderr;
        // winIOv2 (Windows IOCTL v2) is the user-mode hook that doesn't
        // require installing the Wintercept kernel driver, so users get a
        // working install with no extra steps. Skip the `_cmd_allowed_`
        // build — kanata's `cmd` action is a shell-exec primitive we don't
        // want enabled by default.
        (
            "windows-binaries-x64.zip",
            "kanata_windows_tty_winIOv2_x64.exe",
        )
    } else {
        panic!(
            "bundled-kanata: target {target} is not yet supported.\n\
             Supported: aarch64-apple-darwin, x86_64-apple-darwin, \
             x86_64-unknown-linux-gnu, x86_64-pc-windows-msvc"
        );
    };

    let url = format!("https://github.com/jtroo/kanata/releases/download/{version}/{zip_name}");
    let zip_path = std::path::Path::new(&out_dir).join("kanata-download.zip");

    println!("cargo:warning=bundled-kanata: downloading {version} for {target}…");

    let status = std::process::Command::new("curl")
        .args(["-fsSL", "-o", zip_path.to_str().unwrap(), &url])
        .status()
        .expect(
            "bundled-kanata: `curl` not found — \
             install curl or build without the bundled-kanata feature",
        );
    assert!(
        status.success(),
        "bundled-kanata: failed to download kanata from {url}"
    );

    // Extract the binary from the zip archive.
    let zip_data = std::fs::read(&zip_path).expect("bundled-kanata: could not read downloaded zip");
    let cursor = std::io::Cursor::new(zip_data);
    let mut archive =
        zip::ZipArchive::new(cursor).expect("bundled-kanata: downloaded file is not a valid zip");
    let mut entry = archive.by_name(bin_name).unwrap_or_else(|_| {
        panic!("bundled-kanata: '{bin_name}' not found in zip — check kanata release assets")
    });
    let mut out_file =
        std::fs::File::create(&bundle_path).expect("bundled-kanata: cannot create bundle file");
    std::io::copy(&mut entry, &mut out_file)
        .expect("bundled-kanata: failed to extract kanata binary");
    drop(out_file);

    // Mark as executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bundle_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bundle_path, perms).unwrap();
    }

    // Cache the version so subsequent builds don't re-download.
    std::fs::write(&version_file, &version).unwrap();

    // Clean up the zip.
    let _ = std::fs::remove_file(&zip_path);

    let size = std::fs::metadata(&bundle_path).unwrap().len();
    println!(
        "cargo:warning=bundled-kanata: ready ({} bytes) → {}",
        size,
        bundle_path.display()
    );
}
