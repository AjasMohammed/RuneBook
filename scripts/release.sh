#!/usr/bin/env bash
#
# release.sh — cut a Runebook release.
#
# Bumps the version in ALL of the version-bearing files so they stay in lockstep:
#   - package.json                  ("version": "...")
#   - src-tauri/tauri.conf.json     ("version": "...")
#   - src-tauri/Cargo.toml          ([package] version = "...")
#   - mcp-server/Cargo.toml         ([package] version = "...")
# then commits, creates an annotated tag `v<version>`, and (after you confirm)
# pushes the branch + tag. Pushing the tag is what triggers .github/workflows/
# release.yml, which builds and publishes the .deb + the runebook-mcp binary.
#
# Usage:
#   ./scripts/release.sh 0.2.0          # bump to 0.2.0, commit, tag, push (asks first)
#   ./scripts/release.sh 0.2.0 --yes    # same, no confirmation prompt
#   ./scripts/release.sh 0.2.0 --no-push  # bump + commit + tag locally; push later
#   ./scripts/release.sh 0.2.0 --dry-run  # show what would change; touch nothing
#
# Notes:
#   - Pass the version WITHOUT a leading 'v'. The tag gets the 'v' (v0.2.0).
#   - Only plain X.Y.Z releases are supported (no -rc / +build suffixes): a .deb
#     prerelease would order ABOVE the GA under dpkg and wedge users on the rc.
#   - Refuses to run on a dirty working tree, so the only change in the release
#     commit is the version bump.

set -euo pipefail

# ── Locate the repo root so the script works from anywhere ───────────────────
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." >/dev/null 2>&1 && pwd)"
cd "$REPO_ROOT"

# Files that carry the version, all kept in sync.
PKG_JSON="package.json"
TAURI_CONF="src-tauri/tauri.conf.json"
TAURI_CARGO="src-tauri/Cargo.toml"
MCP_CARGO="mcp-server/Cargo.toml"

# ── Pretty output ────────────────────────────────────────────────────────────
if [ -t 1 ]; then
  B=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YLW=$'\033[33m'; RST=$'\033[0m'
else
  B=""; RED=""; GRN=""; YLW=""; RST=""
fi
info() { printf '%s\n' "${B}==>${RST} $*"; }
ok()   { printf '%s\n' "${GRN}==>${RST} $*"; }
warn() { printf '%s\n' "${YLW}warning:${RST} $*" >&2; }
die()  { printf '%s\n' "${RED}error:${RST} $*" >&2; exit 1; }

# ── Args ─────────────────────────────────────────────────────────────────────
VERSION=""
ASSUME_YES=0
DO_PUSH=1
DRY_RUN=0
for arg in "$@"; do
  case "$arg" in
    -y|--yes)   ASSUME_YES=1 ;;
    --no-push)  DO_PUSH=0 ;;
    --dry-run)  DRY_RUN=1 ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \{0,1\}//' | sed '/^!/d'
      exit 0 ;;
    -* ) die "Unknown option: $arg (try --help)" ;;
    * )
      [ -z "$VERSION" ] || die "Version already given ('$VERSION'); unexpected extra arg '$arg'."
      VERSION="$arg" ;;
  esac
done

[ -n "$VERSION" ] || die "Usage: ./scripts/release.sh <version> [--yes] [--no-push] [--dry-run]  (e.g. 0.2.0)"
# Be forgiving if someone passes 'v0.2.0'.
VERSION="${VERSION#v}"
TAG="v${VERSION}"

# ── Validate version: STRICT X.Y.Z only ──────────────────────────────────────
# Prerelease/build metadata (e.g. -rc.1, +build) is deliberately REJECTED:
#   - release.yml would have to special-case it, and
#   - dpkg orders semver prereleases BACKWARDS — `dpkg --compare-versions
#     1.0.0-rc.1 gt 1.0.0` is TRUE (Debian reads -rc.1 as a revision GREATER
#     than none), so an rc .deb would wedge users above the eventual GA and
#     update.sh would report "already up to date". Cut only plain releases here.
# (If prereleases are ever wanted, use the Debian-correct `~rc.1` separator for
#  the package version and set release.yml's `prerelease` from the tag.)
SEMVER_RE='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'
[[ "$VERSION" =~ $SEMVER_RE ]] || die "Not a valid release version: '$VERSION' (expected plain X.Y.Z, e.g. 0.2.0; prereleases like 1.0.0-rc.1 are not supported)."

# ── Preflight: tools, repo, files ────────────────────────────────────────────
command -v git >/dev/null 2>&1 || die "git is required but not installed."
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "Not inside a git work tree."

for f in "$PKG_JSON" "$TAURI_CONF" "$TAURI_CARGO" "$MCP_CARGO"; do
  [ -f "$f" ] || die "Expected file not found: $f (run from the Runebook repo)."
done

# ── Refuse on a dirty tree (the release commit must be the bump alone) ────────
# Skipped for --dry-run, which writes nothing and is meant to preview safely.
if [ "$DRY_RUN" -ne 1 ] && [ -n "$(git status --porcelain)" ]; then
  die "Working tree is not clean. Commit or stash your changes first, then re-run."
fi

# ── Refuse if the tag already exists ─────────────────────────────────────────
if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  die "Tag ${TAG} already exists. Bump to a new version or delete the tag first."
fi

# ── Read the current version (from package.json) for a friendly summary ──────
CURRENT="$(grep -E '"version"[[:space:]]*:' "$PKG_JSON" | head -n1 | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
info "Current version: ${CURRENT:-<unknown>}    ->    New version: ${VERSION}  (tag ${TAG})"

if [ "$CURRENT" = "$VERSION" ]; then
  warn "Files already report version ${VERSION}; will re-tag/commit only if there are changes."
fi

# ── Edit helpers ─────────────────────────────────────────────────────────────
# Each bumps exactly one occurrence (the package version) and verifies the
# result, so a format change in any file fails loudly instead of silently no-op.

bump_json() { # file
  local file="$1"
  if [ "$DRY_RUN" -eq 1 ]; then
    info "[dry-run] would set \"version\": \"${VERSION}\" in ${file}"
    return
  fi
  # Replace only the first top-level "version": "..." line.
  perl -0pi -e 'BEGIN{$done=0} s/("version"\s*:\s*")[^"]*(")/$done++ ? "$1$2" : "${1}'"$VERSION"'$2"/e unless $done' "$file"
  grep -Eq "\"version\"[[:space:]]*:[[:space:]]*\"${VERSION//./\\.}\"" "$file" \
    || die "Failed to set version in ${file} (pattern not found?)."
  info "Updated ${file}"
}

bump_cargo() { # file
  local file="$1"
  if [ "$DRY_RUN" -eq 1 ]; then
    info "[dry-run] would set version = \"${VERSION}\" under [package] in ${file}"
    return
  fi
  # Replace the first `version = "..."` line only (the [package] version, which
  # appears before any [dependencies] in these crates).
  perl -0pi -e 'BEGIN{$done=0} s/^(version\s*=\s*")[^"]*(")/$done++ ? "$1$2" : "${1}'"$VERSION"'$2"/me unless $done' "$file"
  grep -Eq "^version[[:space:]]*=[[:space:]]*\"${VERSION//./\\.}\"" "$file" \
    || die "Failed to set version in ${file} (pattern not found?)."
  info "Updated ${file}"
}

info "Bumping version to ${VERSION} across all four files..."
bump_json  "$PKG_JSON"
bump_json  "$TAURI_CONF"
bump_cargo "$TAURI_CARGO"
bump_cargo "$MCP_CARGO"

# Keep each Cargo.lock in step so the bump commit is self-consistent and CI is
# happy. `cargo update -p <name>` rewrites that crate's own [[package]] version
# entry in the lock from the just-edited Cargo.toml (without touching dependency
# versions or hitting the network with --offline).
#
# This MUST NOT be swallowed: if the offline registry cache is incomplete the
# update fails, and a silent `|| true` would commit Cargo.toml at the new version
# while Cargo.lock still pins the old one. CI then runs --locked (release.yml and
# ci.yml both pass --locked), so a stale lock would fail the build. Refresh it
# loudly here and verify the result, or abort the release.
refresh_lock() { # dir crate-name
  local dir="$1" crate="$2" lock="$1/Cargo.lock"
  [ -f "$lock" ] || { info "No ${lock} yet — skipping lock refresh for ${crate}."; return 0; }
  if ! ( cd "$dir" && cargo update --offline -p "$crate" ); then
    die "Could not refresh ${lock} offline. Run 'cargo update -p ${crate}' (online) in ${dir}/ first, then re-run."
  fi
  # Verify the lock now records the new version for the crate's own entry.
  awk -v c="$crate" -v v="$VERSION" '
    $0=="name = \"" c "\"" {f=1; next}
    f && $0=="version = \"" v "\"" {print "ok"; exit}
    f && /^name = / {exit}
  ' "$lock" | grep -q ok \
    || die "Refreshed ${lock} but it does not pin ${crate} at ${VERSION}. Refusing to commit a stale lockfile."
  info "Refreshed ${lock}"
}

if [ "$DRY_RUN" -ne 1 ]; then
  command -v cargo >/dev/null 2>&1 \
    || die "cargo not found — needed to refresh Cargo.lock so the release commit is self-consistent (CI runs --locked). Install Rust or run on a machine with cargo."
  info "Refreshing Cargo.lock files..."
  refresh_lock src-tauri  runebook
  refresh_lock mcp-server runebook-mcp
fi

if [ "$DRY_RUN" -eq 1 ]; then
  ok "[dry-run] No files were changed. Re-run without --dry-run to apply."
  exit 0
fi

# ── Show the diff and confirm ────────────────────────────────────────────────
echo
info "Pending changes:"
git --no-pager diff --stat
echo

if [ -z "$(git status --porcelain)" ]; then
  warn "No file changes were produced (already at ${VERSION}?)."
  die  "Nothing to commit. Aborting before tagging to avoid an empty release commit."
fi

if [ "$ASSUME_YES" -ne 1 ]; then
  printf '%s' "Commit these changes, create tag ${TAG}$([ "$DO_PUSH" -eq 1 ] && printf '%s' ', and push')? [y/N] "
  read -r reply
  case "$reply" in
    [yY]*) : ;;
    *) warn "Aborted. Your working tree still has the version bump; 'git checkout -- .' to discard."; exit 0 ;;
  esac
fi

# ── Commit + annotated tag ───────────────────────────────────────────────────
info "Committing..."
git add "$PKG_JSON" "$TAURI_CONF" "$TAURI_CARGO" "$MCP_CARGO" \
        src-tauri/Cargo.lock mcp-server/Cargo.lock 2>/dev/null || \
  git add "$PKG_JSON" "$TAURI_CONF" "$TAURI_CARGO" "$MCP_CARGO"
git commit -m "chore(release): ${VERSION}"

info "Tagging ${TAG}..."
git tag -a "$TAG" -m "Runebook ${VERSION}"

# ── Push (this is what fires the release workflow) ───────────────────────────
if [ "$DO_PUSH" -eq 0 ]; then
  ok "Committed and tagged ${TAG} locally (not pushed)."
  info "When ready:  git push origin HEAD && git push origin ${TAG}"
  exit 0
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
info "Pushing ${BRANCH} and tag ${TAG} to origin..."
git push origin "$BRANCH"
git push origin "$TAG"

ok "Released ${TAG}. GitHub Actions (release.yml) is now building the .deb + MCP server."
info "Watch it:  https://github.com/AjasMohammed/RuneBook/actions"
