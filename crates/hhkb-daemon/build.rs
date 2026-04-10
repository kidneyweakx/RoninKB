//! Build-time prerequisite check for the `embedded-ui` feature.
//!
//! When `--features embedded-ui` is active, `rust-embed` will read the
//! compiled hhkb-app bundle from `apps/hhkb-app/dist` at compile time. If
//! that directory is missing we want to fail the build with a friendly
//! message rather than a confusing rust-embed error.
//!
//! `#[cfg(feature = ...)]` is not available inside `build.rs`, but Cargo
//! sets `CARGO_FEATURE_<NAME>` env vars for every enabled feature, so we
//! detect the flag via `std::env` instead.

fn main() {
    println!("cargo:rerun-if-changed=../../apps/hhkb-app/dist");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_FEATURE_EMBEDDED_UI").is_ok() {
        let manifest_dir =
            std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
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
}
