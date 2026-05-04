# Releasing RoninKB

This repo ships two flavors of binary distribution:

- A **Homebrew formula** at `Formula/roninkb.rb`, auto-updated on every tag.
- **Raw tarballs/zips** attached to a GitHub Release, consumed by
  `install.sh` / `install.ps1`.

## Cutting a release

1. Bump the version in:
   - `crates/hhkb-core/Cargo.toml`
   - `crates/hhkb-cli/Cargo.toml`
   - `crates/hhkb-daemon/Cargo.toml`
   - `crates/hhkb-macos-native/Cargo.toml` (v0.2.0+ workspace member)
   - `apps/hhkb-app/package.json`
   - `Formula/roninkb.rb` (`version "..."` — the release workflow will also
     rewrite this, but keeping it in sync locally avoids noisy diffs)

   v0.2.0+ ships a single daemon binary on every platform. `hhkb-macos-native`
   is a workspace member but does **not** produce a separate binary; it
   compiles into `hhkb-daemon` on macOS via a `cfg(target_os = "macos")`
   dependency. The macOS release archive is identical in shape to v0.1.x.

2. Commit and tag:

   ```bash
   git commit -am "release: v0.x.0"
   git tag v0.x.0
   git push origin main --tags
   ```

3. The `.github/workflows/release.yml` workflow runs on the tag push and
   will:

   - Build the React frontend (`apps/hhkb-app`).
   - Build `hhkb` and `hhkb-daemon` with
     `--features hhkb-core/hidapi-backend,hhkb-daemon/embedded-ui` for each
     target platform:
     - `x86_64-apple-darwin`
     - `aarch64-apple-darwin`
     - `universal-apple-darwin` (stitched via `lipo`)
     - `x86_64-unknown-linux-gnu`
     - `aarch64-unknown-linux-gnu` (built via `cross`)
     - `x86_64-pc-windows-msvc`
   - Stage each archive with `bin/`, `install/`, `README.md`, `LICENSE`.
   - Create a GitHub Release with all archives + `.sha256` files attached.
   - Update `Formula/roninkb.rb` with the new tag and the fresh SHA256
     digests for macOS universal + Linux x86_64 + Linux aarch64, then push
     a `chore(formula): release vX.Y.Z` commit back to `main`.

4. Verify end-user installation:

   ```bash
   brew tap kidneyweakx/roninkb https://github.com/kidneyweakx/RoninKB.git
   brew install roninkb
   brew services start roninkb
   curl http://127.0.0.1:7331/health
   ```

   ```bash
   curl -fsSL https://raw.githubusercontent.com/kidneyweakx/RoninKB/main/install.sh | sh
   ```

## Manually triggering a release

The workflow also supports `workflow_dispatch` with a `tag` input, handy
for re-running against an existing tag after fixing CI issues:

```
GitHub → Actions → Release → Run workflow → tag: v0.1.0
```

## Rolling back

Delete the tag, the GitHub Release, and revert the `chore(formula): ...`
commit on `main`. Homebrew users on the bad version can `brew update &&
brew upgrade roninkb` once a fixed release ships.
