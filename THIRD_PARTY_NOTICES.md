# Third-Party Notices

RoninKB is licensed under the MIT License (see [`LICENSE`](LICENSE)). Some
components shipped with RoninKB are covered by other open-source licenses.
This document records those components and points at their original sources.

If you redistribute RoninKB (binary or source) you must keep this notice
intact and ship the listed third-party license texts alongside it.

---

## kanata

- **Role**: Software-layer key remapping engine. RoninKB invokes `kanata` as
  a separate child process (see `crates/hhkb-daemon/src/kanata.rs`). When
  built with the `bundled-kanata` Cargo feature, RoninKB embeds an
  unmodified `kanata` release binary into the daemon and extracts it at
  runtime — it is never linked into the daemon as a library.
- **Source**: <https://github.com/jtroo/kanata>
- **Upstream release downloaded**: `v1.11.0` by default; pinned via
  `KANATA_VERSION` at build time.
- **License**: GNU Lesser General Public License v3.0 (LGPL-3.0).
- **License text**: [`THIRD_PARTY_LICENSES/kanata-LICENSE.txt`](THIRD_PARTY_LICENSES/kanata-LICENSE.txt)
- **Modifications**: None. RoninKB downloads the upstream release artifact
  unchanged.

### LGPL-3.0 obligations RoninKB satisfies

1. **Notice that the Library is used and covered by LGPL** — this file plus
   the README "Acknowledgements" section.
2. **Copy of the LGPL-3.0 license text** —
   [`THIRD_PARTY_LICENSES/kanata-LICENSE.txt`](THIRD_PARTY_LICENSES/kanata-LICENSE.txt).
   When the daemon is built with `--features bundled-kanata`, the same text
   is also extracted to disk next to the kanata binary so it travels with
   the executable.
3. **Source availability** — kanata is shipped unmodified, so the upstream
   repository at <https://github.com/jtroo/kanata> is the Corresponding
   Source. The release tag matching the bundled binary is identified by
   the `KANATA_VERSION` build-time variable.
4. **Replaceability** — `hhkb-daemon` invokes kanata as a separate process
   and accepts a custom path to the kanata binary (see
   `crates/hhkb-daemon/src/kanata.rs`). End users can replace the bundled
   binary with their own LGPL-compliant build of kanata.

### What this means for RoninKB's own license

RoninKB stays MIT-licensed. LGPL-3.0 §4 explicitly permits combining an
LGPL library with a work under any other license, provided the obligations
above are met. Because kanata is invoked as a separate executable rather
than linked, the combination is closer to "mere aggregation" (LGPL §0) than
to a Combined Work in the linking sense — but the obligations above are
written for the stricter interpretation, so the project is covered either
way.

If you fork RoninKB and **modify kanata's source**, those modifications
become subject to LGPL-3.0 and must be released under LGPL-3.0 (or a
compatible license). Don't do that inside this repository — keep any kanata
fork as a separate repository and adjust `KANATA_VERSION` / the build to
fetch from your fork.

---

## hhkb-app dependencies

The frontend (`apps/hhkb-app`) bundles standard npm dependencies. Run
`npm ls --prod` inside `apps/hhkb-app/` for the current snapshot. None of
those dependencies are copyleft as of this writing — Chakra UI, Framer
Motion, Zustand, lucide-react, and the Inter / JetBrains Mono fonts are
all permissive (MIT / OFL).

## Other workspace crates

All Rust crates pulled in via `Cargo.toml` are permissively licensed
(MIT / Apache-2.0 / BSD / Unlicense). Run `cargo deny check licenses` or
`cargo about generate` if you need an exhaustive license inventory for a
release.
