#!/usr/bin/env bash
#
# update.sh — self-update Runebook from its latest GitHub Release (.deb).
#
# Tauri's built-in updater (tauri-plugin-updater) is NOT viable for a .deb
# install: a .deb lives in root-owned system paths (/usr/bin, /usr/share), and
# the updater "cannot request admin privileges by itself", so it only works for
# AppImage / user-wide installs. This script is the supported update path for
# the .deb: it asks for sudo once, at install time.
#
# What it does:
#   1. Query the GitHub REST API for the latest (non-prerelease) release.
#   2. Find the release asset whose name ends in `.deb`.
#   3. Compare the release version to the installed version (dpkg).
#   4. Verify the .deb's SHA256 against the release's SHA256SUMS asset (if any).
#   5. If newer: download to a temp dir and install with
#        `sudo apt install ./pkg.deb`  (preferred — resolves runtime deps).
#      Only if apt/apt-get are absent does it fall back to `sudo dpkg -i`; a
#      genuine apt install failure aborts loudly rather than retrying dpkg over a
#      half-applied state.
#
# Usage:
#   ./scripts/update.sh              # check + update if newer (prompts before install)
#   ./scripts/update.sh --check      # report only, never install (exit 10 if update available)
#   ./scripts/update.sh --yes        # non-interactive (assume "yes" to prompts)
#
# Recommended one-liner (inspect before running — never blind-pipe to bash):
#   curl -fsSL https://raw.githubusercontent.com/AjasMohammed/RuneBook/main/scripts/update.sh -o update.sh
#   less update.sh && bash update.sh
#
# If piped straight to bash with no TTY and no --yes, this script will NOT
# auto-install: it prints the plan and exits, so you re-run with intent.

set -euo pipefail

# ── Config ───────────────────────────────────────────────────────────────────
# NOTE: the repo path is RuneBook (capital B); the product/binary/dpkg package is
# runebook (lowercase b). raw.githubusercontent.com and the GitHub API are
# CASE-SENSITIVE on the owner/repo path — do NOT "unify" the casing to match the
# product name, or every fetch 404s silently.
OWNER="AjasMohammed"
REPO="RuneBook"
PKG="runebook"                       # dpkg package name
BIN="runebook"                       # process / executable name
API="https://api.github.com/repos/${OWNER}/${REPO}/releases/latest"

# ── Flags ────────────────────────────────────────────────────────────────────
CHECK_ONLY=0
ASSUME_YES=0
for arg in "$@"; do
  case "$arg" in
    --check)        CHECK_ONLY=1 ;;
    -y|--yes)       ASSUME_YES=1 ;;
    -h|--help)
      # When piped (curl ... | bash), $0 is "bash", not this file, so the header
      # grep would fail. Print the embedded header only if $0 is a real file.
      if [ -f "$0" ]; then
        grep '^#' "$0" | sed 's/^# \{0,1\}//' | sed '/^!/d'
      else
        printf '%s\n' \
          "update.sh — self-update Runebook from its latest GitHub Release (.deb)." \
          "" \
          "Flags: --check (report only, exit 10 if update available), -y/--yes (non-interactive)." \
          "Docs:  https://github.com/AjasMohammed/RuneBook"
      fi
      exit 0 ;;
    *) echo "Unknown option: $arg (try --help)" >&2; exit 2 ;;
  esac
done

# ── Pretty output ────────────────────────────────────────────────────────────
if [ -t 1 ]; then
  B=$'\033[1m'; DIM=$'\033[2m'; RED=$'\033[31m'; GRN=$'\033[32m'; YLW=$'\033[33m'; RST=$'\033[0m'
else
  B=""; DIM=""; RED=""; GRN=""; YLW=""; RST=""
fi
info()  { printf '%s\n' "${B}==>${RST} $*"; }
ok()    { printf '%s\n' "${GRN}==>${RST} $*"; }
warn()  { printf '%s\n' "${YLW}warning:${RST} $*" >&2; }
die()   { printf '%s\n' "${RED}error:${RST} $*" >&2; exit 1; }

# ── Preflight ────────────────────────────────────────────────────────────────
command -v curl    >/dev/null 2>&1 || die "curl is required but not installed."
command -v dpkg    >/dev/null 2>&1 || die "dpkg is required (this script targets Debian/Ubuntu/Pop!_OS)."

HAVE_JQ=0
command -v jq >/dev/null 2>&1 && HAVE_JQ=1

# ── Installed version (empty if not installed) ───────────────────────────────
INSTALLED=""
if dpkg-query -W -f='${Status}' "$PKG" 2>/dev/null | grep -q "install ok installed"; then
  INSTALLED="$(dpkg-query -W -f='${Version}' "$PKG" 2>/dev/null || true)"
fi

# ── Fetch latest release JSON ────────────────────────────────────────────────
# -f makes curl fail (non-zero) on HTTP >=400 (e.g. 403 rate-limit, 404 no release).
info "Checking ${OWNER}/${REPO} for the latest release..."
HDR=()
# An optional token raises the unauthenticated 60 req/hr limit to 5000/hr.
if [ -n "${GITHUB_TOKEN:-}" ]; then
  HDR=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
fi

JSON="$(curl -fsSL "${HDR[@]}" -H "Accept: application/vnd.github+json" "$API" 2>/dev/null)" || {
  die "Could not reach the GitHub API. If this is a rate limit (HTTP 403), wait an hour or set GITHUB_TOKEN. URL: $API"
}
[ -n "$JSON" ] || die "Empty response from GitHub API."

# ── Parse tag + .deb asset URL (jq preferred, grep/sed fallback) ─────────────
if [ "$HAVE_JQ" -eq 1 ]; then
  TAG="$(printf '%s' "$JSON"      | jq -r '.tag_name // empty')"
  PRERELEASE="$(printf '%s' "$JSON" | jq -r '.prerelease // false')"
  DEB_URL="$(printf '%s' "$JSON"  | jq -r '.assets[]? | select(.name|endswith(".deb")) | .browser_download_url' | head -n1)"
  DEB_NAME="$(printf '%s' "$JSON" | jq -r '.assets[]? | select(.name|endswith(".deb")) | .name' | head -n1)"
  SUMS_URL="$(printf '%s' "$JSON" | jq -r '.assets[]? | select(.name=="SHA256SUMS") | .browser_download_url' | head -n1)"
else
  warn "jq not found — using a best-effort text parser. Install jq for robustness: sudo apt install jq"
  TAG="$(printf '%s' "$JSON"        | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"'   | head -n1 | sed -E 's/.*"([^"]*)"$/\1/')"
  PRERELEASE="$(printf '%s' "$JSON" | grep -o '"prerelease"[[:space:]]*:[[:space:]]*[a-z]*'  | head -n1 | grep -o '[a-z]*$')"
  DEB_URL="$(printf '%s' "$JSON"    | grep -o '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*\.deb"' | head -n1 | grep -o 'https[^"]*')"
  DEB_NAME="$(basename "${DEB_URL:-}")"
  SUMS_URL="$(printf '%s' "$JSON"   | grep -o '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*/SHA256SUMS"' | head -n1 | grep -o 'https[^"]*')"
fi

[ -n "${TAG:-}" ]     || die "Could not determine the release tag from the API response."
[ -n "${DEB_URL:-}" ] || die "The latest release ($TAG) has no .deb asset. Nothing to install."

# /releases/latest already excludes prereleases & drafts, but double-check.
if [ "${PRERELEASE:-false}" = "true" ]; then
  die "Latest release ($TAG) is a prerelease; refusing to install. (Use the GitHub UI to install prereleases manually.)"
fi

# Strip a leading 'v' (v0.2.0 -> 0.2.0) so it matches dpkg's version string.
LATEST="${TAG#v}"

info "Installed: ${INSTALLED:-<none>}    Latest: ${LATEST} (${TAG})"

# ── Decide whether to update (dpkg --compare-versions handles semver) ────────
if [ -n "$INSTALLED" ] && ! dpkg --compare-versions "$LATEST" gt "$INSTALLED"; then
  ok "Already up to date (${INSTALLED})."
  exit 0
fi

if [ "$CHECK_ONLY" -eq 1 ]; then
  info "Update available: ${INSTALLED:-<none>} -> ${LATEST}. (run without --check to install)"
  exit 10
fi

if [ -z "$INSTALLED" ]; then
  info "Runebook is not currently installed — this will perform a fresh install of ${LATEST}."
else
  info "Update available: ${INSTALLED} -> ${LATEST}."
fi

# ── Confirm ──────────────────────────────────────────────────────────────────
# Never auto-run `sudo apt install` without intent. With an interactive TTY we
# prompt. When piped (curl ... | bash, no TTY on stdin) and no --yes was given,
# we DO NOT silently install — we show the plan and exit, so the user re-runs
# deliberately. This avoids a one-liner installing as soon as sudo happens to be
# non-interactive (cached timestamp / NOPASSWD).
if [ "$ASSUME_YES" -ne 1 ]; then
  if [ -t 0 ]; then
    printf '%s' "Download and install ${DEB_NAME}? [Y/n] "
    read -r reply
    case "$reply" in [nN]*) info "Aborted."; exit 0 ;; esac
  else
    info  "Update available: ${INSTALLED:-<none>} -> ${LATEST} (${DEB_NAME})."
    warn  "Non-interactive stdin (piped): not auto-installing without confirmation."
    info  "To install, re-run with intent, e.g.:"
    info  "    curl -fsSL https://raw.githubusercontent.com/${OWNER}/${REPO}/main/scripts/update.sh -o update.sh"
    info  "    less update.sh && bash update.sh --yes"
    info  "  (or, if you have the repo checked out:  ./scripts/update.sh --yes )"
    exit 0
  fi
fi

# ── Stop the app if it's running (a busy binary can't be replaced cleanly) ───
if pgrep -x "$BIN" >/dev/null 2>&1; then
  warn "Runebook appears to be running."
  STOP=1
  if [ "$ASSUME_YES" -ne 1 ] && [ -t 0 ]; then
    printf '%s' "Stop it before updating? [Y/n] "
    read -r r2
    case "$r2" in [nN]*) STOP=0 ;; esac
  fi
  if [ "$STOP" -eq 1 ]; then
    info "Stopping Runebook..."
    pkill -TERM -x "$BIN" 2>/dev/null || true
    for _ in 1 2 3 4 5; do
      pgrep -x "$BIN" >/dev/null 2>&1 || break
      sleep 1
    done
    pgrep -x "$BIN" >/dev/null 2>&1 && pkill -KILL -x "$BIN" 2>/dev/null || true
  else
    warn "Continuing with Runebook running; you may need to restart it after updating."
  fi
fi

# ── Download to a self-cleaning temp dir ─────────────────────────────────────
TMP="$(mktemp -d "${TMPDIR:-/tmp}/runebook-update.XXXXXX")"
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT INT TERM

DEB_PATH="${TMP}/${DEB_NAME}"
info "Downloading ${DEB_NAME} ..."
curl -fL --progress-bar -o "$DEB_PATH" "$DEB_URL" || die "Download failed: $DEB_URL"
[ -s "$DEB_PATH" ] || die "Downloaded file is empty: $DEB_PATH"

# Sanity: a .deb is an `ar` archive starting with the magic "!<arch>".
if command -v file >/dev/null 2>&1; then
  file "$DEB_PATH" | grep -qi "debian binary package" || warn "Downloaded file doesn't look like a .deb; continuing anyway."
fi

# ── Verify SHA256 against the release's SHA256SUMS, if published ──────────────
# release.yml attaches a SHA256SUMS asset. If present, the .deb's hash MUST match
# before we install — a tampered/corrupt asset is fatal. If the release predates
# checksums (no SHA256SUMS) we warn and continue (back-compat).
if [ -n "${SUMS_URL:-}" ] && command -v sha256sum >/dev/null 2>&1; then
  info "Verifying checksum against SHA256SUMS ..."
  SUMS_PATH="${TMP}/SHA256SUMS"
  curl -fsSL "${HDR[@]}" -o "$SUMS_PATH" "$SUMS_URL" || die "Could not download SHA256SUMS: $SUMS_URL"
  EXPECTED="$(grep -E "[[:space:]][*]?${DEB_NAME}\$" "$SUMS_PATH" | awk '{print $1}' | head -n1)"
  [ -n "$EXPECTED" ] || die "SHA256SUMS has no entry for ${DEB_NAME}; refusing to install an unverifiable download."
  ACTUAL="$(sha256sum "$DEB_PATH" | awk '{print $1}')"
  if [ "$EXPECTED" != "$ACTUAL" ]; then
    die "Checksum mismatch for ${DEB_NAME}! expected ${EXPECTED}, got ${ACTUAL}. Aborting (download may be corrupt or tampered)."
  fi
  ok "Checksum verified (${ACTUAL})."
else
  warn "No SHA256SUMS asset (or sha256sum unavailable) — skipping checksum verification."
fi

# ── Install (needs root). apt resolves deps; dpkg is the fallback. ────────────
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
  command -v sudo >/dev/null 2>&1 || die "Root privileges are required to install. Re-run as root or install sudo."
  SUDO="sudo"
  info "Installing requires root — you may be prompted for your password."
fi

info "Installing ${LATEST} ..."
# Prefer apt (resolves runtime deps). Fall back to dpkg+apt-get -f ONLY when the
# apt/apt-get binaries are absent — not when an install ATTEMPT fails. Retrying a
# different installer over a half-applied dpkg state can wedge it; a genuine
# install failure should surface with the apt output, not be papered over.
if command -v apt >/dev/null 2>&1; then
  $SUDO apt install -y "$DEB_PATH" \
    || die "apt install failed. See the output above; resolve the issue and re-run."
elif command -v apt-get >/dev/null 2>&1; then
  $SUDO apt-get install -y "$DEB_PATH" \
    || die "apt-get install failed. See the output above; resolve the issue and re-run."
else
  warn "apt/apt-get not found; falling back to dpkg -i (+ apt-get install -f is unavailable here)."
  $SUDO dpkg -i "$DEB_PATH" \
    || die "dpkg -i failed and apt is unavailable to resolve dependencies. Install deps manually and re-run."
fi

# ── Verify ───────────────────────────────────────────────────────────────────
NOW="$(dpkg-query -W -f='${Version}' "$PKG" 2>/dev/null || true)"
if [ "$NOW" = "$LATEST" ]; then
  ok "Runebook updated to ${NOW}. Launch it from your app menu or run \`${BIN}\`."
else
  die "Install finished but version is '${NOW:-<unknown>}', expected '${LATEST}'. Check the apt/dpkg output above."
fi
