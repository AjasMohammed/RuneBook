---
id: release-ci-and-self-update
label: Release CI + .deb Self-Update
type: decision
community: environment-build
edges:
  - target: production-readiness
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: mcp-server
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: db-rs-shared-with-mcp
    relation: references
    confidence: INFERRED
    confidence_score: 0.7
---

Added GitHub Actions + a CLI self-update (2026-06-18..21). Four files, repo
`AjasMohammed/RuneBook`:

- **`.github/workflows/release.yml`** — on push tag `v*`: verifies the tag matches
  all four version files, installs the Tauri apt deps (with retry), builds
  `runebook-mcp` first (fail fast), then `tauri-apps/tauri-action@v0.6.2` builds the
  `.deb` and publishes the GitHub Release; follow-up steps attach the MCP binary and
  a `SHA256SUMS`. `permissions: contents: write`.
- **`.github/workflows/ci.yml`** — on push/PR to main: version-consistency check,
  Vite build, then `fmt`/`clippy -D warnings`/`test`/`build --release` for BOTH crates,
  all `--locked` so a stale Cargo.lock fails loudly.
- **`scripts/update.sh`** — the self-update: curl GitHub API `releases/latest`, pick the
  `.deb` asset, `dpkg --compare-versions` vs the installed pkg, verify SHA256, then
  `sudo apt install ./pkg.deb`. `npm run update`.
- **`scripts/release.sh`** — bumps `package.json` + `src-tauri/tauri.conf.json` +
  `src-tauri/Cargo.toml` + `mcp-server/Cargo.toml` in lockstep, refreshes both
  Cargo.locks loudly, commits, tags `v<ver>`, pushes. `npm run release <ver>`.

**Why CLI self-update, not Tauri's in-app updater:** `tauri-plugin-updater` is
**AppImage-only on Linux** — a `.deb` lives in root-owned paths and the updater can't
self-elevate. So "auto-update on simple commands" = a script that asks for `sudo` once.

**Gotchas worth remembering (verified):**
- `tauri-action@v0.6.2` has **NO `includeFiles` input** (confirmed against its
  `action.yml`). The first draft used it; an unknown `with:` input is *silently
  ignored*, so the MCP binary would never attach. Extra assets MUST go through a
  separate `gh release upload "$TAG" … --clobber` step after tauri-action publishes.
- **dpkg orders `1.0.0-rc.1` ABOVE `1.0.0`** (treats `-rc.1` as a revision *greater*
  than none), so a prerelease `.deb` would wedge users above the GA and update.sh would
  say "already up to date". `release.sh` therefore rejects anything but plain `X.Y.Z`;
  `release.yml` also derives `prerelease:` from a `-` in the tag as defense-in-depth.
- **Casing:** GitHub path is `AjasMohammed/RuneBook` (capital B); product/binary/dpkg
  package is `runebook` (lowercase). `raw.githubusercontent.com` + the API are
  case-sensitive — do not "unify" them.
- `npm ci` + `cache: npm` + `cargo --locked` all require lockfiles — `package-lock.json`,
  `src-tauri/Cargo.lock`, `mcp-server/Cargo.lock` all exist and are tracked.

Built via a 4-phase reviewed Workflow (research → design → 3-lens adversarial review →
finalize). The finalize agent died twice on transient API errors, but the design agent
had already written review-hardened files to disk, so fixes were applied by hand from
the structured issue list (0 critical, 3 major, 9 minor, 5 nit). The one real bug the
review missed-but-web-check-caught was the `includeFiles` hallucination.

Not yet done: nothing is committed/pushed — the user must `git add` these and push for CI
to exist on GitHub. Versions are still 0.1.0; the first real use is `npm run release 0.2.0`.
