# GitHub Actions Workflows

This directory contains CI/CD automation for RoninKB. Each workflow file is
scoped to a specific concern so jobs stay fast and independently cacheable.

## Workflows

### `ci.yml`

Primary continuous integration pipeline. Runs on pushes to `main`, pull
requests targeting `main`, and manual `workflow_dispatch`. PRs use a
concurrency group so newer pushes cancel in-flight runs.

Jobs:

- **`rust-test`** — matrix build across `ubuntu-latest`, `macos-latest`, and
  `windows-latest`. Installs the Rust stable toolchain with `rustfmt` and
  `clippy`, caches the Cargo registry/target via `Swatinem/rust-cache`, and
  runs `cargo fmt --check`, `cargo build`, `cargo test --workspace`, and
  `cargo clippy -- -D warnings` against the `hhkb-core/hidapi-backend`
  feature. Linux additionally installs `libudev-dev`, `libxdo-dev`, and
  `pkg-config` (hidapi + tray system dependencies).
- **`rust-features`** — Linux-only feature-flag smoke check. Verifies that
  `hhkb-core` with `firmware-write`, and `hhkb-daemon` with each combination
  of `tray` / `clipboard`, compile cleanly via `cargo check`.
- **`frontend`** — builds and tests `apps/hhkb-app`. Sets up Node.js 20 with
  npm caching keyed on `apps/hhkb-app/package-lock.json`, runs `npm ci`,
  `npx tsc --noEmit`, `npm run test`, `npm run build`, then uploads
  `apps/hhkb-app/dist` as the `hhkb-app-dist` artifact so downstream
  workflows (e.g. release) can reuse it.
- **`summary`** — depends on all three jobs above and prints
  `All checks passed`. Wire this up in branch protection so a single
  required check can gate merges.

## Conventions

- `permissions: contents: read` is set at the workflow level; jobs should
  request additional scopes explicitly when needed.
- Long-running or platform-specific steps live behind `if:` guards on the
  matrix `os` so the workflow stays a single file.
- System dependencies are installed with `sudo apt-get` on Linux only;
  macOS and Windows runners need no extra packages today.

Future release / publish workflows will be documented here as they land.
