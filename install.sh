#!/usr/bin/env bash
#
# agent-bar plugin bootstrap — installs the Omarchy Quattro plugin bundle only.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh | bash
#
# Flags:
#   --force      Reinstall even if already at the target version.
#   --yes, -y    Assume yes for prompts (non-interactive enable).
#
# Env:
#   AGENT_BAR_VERSION  Version to install (default: latest release tag, without v).
#
# Product: $HOME/.config/omarchy/plugins/agent-bar.usage
# No global executable is installed.

set -euo pipefail

GITHUB_REPO="othavi0/agent-bar"
TARGET="x86_64-unknown-linux-gnu"
PLUGIN_ID="agent-bar.usage"
PLUGINS_DIR="${HOME}/.config/omarchy/plugins"
PLUGIN_ROOT="${PLUGINS_DIR}/${PLUGIN_ID}"

FORCE=0
YES=0

for arg in "$@"; do
  case "$arg" in
    --force) FORCE=1 ;;
    --yes|-y) YES=1 ;;
    --help|-h)
      sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    *)
      echo "agent-bar install: unknown flag: $arg" >&2
      exit 2
      ;;
  esac
done

log()  { echo "==> $*" >&2; }
ok()   { echo "OK  $*" >&2; }
warn() { echo "!   $*" >&2; }
die()  { echo "ERR $*" >&2; exit 1; }

check_platform() {
  local uname_s arch
  uname_s=$(uname -s 2>/dev/null || echo unknown)
  [[ "$uname_s" == "Linux" ]] || die "agent-bar requires Linux. Detected: $uname_s"
  arch=$(uname -m 2>/dev/null || echo unknown)
  [[ "$arch" == "x86_64" ]] || die "Only x86_64 plugin bundles are published. Detected: $arch"
}

check_deps() {
  command -v curl >/dev/null 2>&1 || die "curl not found"
  command -v sha256sum >/dev/null 2>&1 || die "sha256sum not found"
  command -v tar >/dev/null 2>&1 || die "tar not found"
  command -v zstd >/dev/null 2>&1 || die "zstd not found (required for .tar.zst)"
}

resolve_version() {
  if [[ -n "${AGENT_BAR_VERSION:-}" ]]; then
    local v="${AGENT_BAR_VERSION}"
    v="${v#v}"
    echo "$v"
    return
  fi
  log "Resolving latest release..."
  local tag
  tag=$(curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" \
    | grep '"tag_name"' \
    | head -1 \
    | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
  [[ -n "$tag" ]] || die "Could not resolve latest release. Set AGENT_BAR_VERSION."
  echo "${tag#v}"
}

existing_version() {
  if [[ -f "${PLUGIN_ROOT}/manifest.json" ]]; then
    grep -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "${PLUGIN_ROOT}/manifest.json" \
      | head -1 \
      | sed 's/.*"\([^"]*\)"$/\1/' || true
  fi
}

# Validate staged plugin tree inventory/receipt before live swap (BUNDLE-013).
validate_staged_bundle() {
  local stage="$1"
  local expected_version="$2"
  local receipt="${stage}/bundle.json"
  local manifest="${stage}/manifest.json"

  [[ -f "$receipt" ]] || die "Staged bundle missing bundle.json receipt"
  [[ -f "$manifest" ]] || die "Staged bundle missing manifest.json"
  [[ -f "${stage}/Service.qml" ]] || die "Staged bundle missing Service.qml"
  [[ -f "${stage}/BarWidget.qml" ]] || die "Staged bundle missing BarWidget.qml"
  [[ -x "${stage}/bin/agent-bar" ]] || die "Staged helper bin/agent-bar missing or not executable"
  [[ -x "${stage}/scripts/agent-bar-open-terminal" ]] \
    || die "Staged terminal helper missing or not executable"

  # Receipt pluginId + version must match expected install target.
  grep -q '"pluginId"[[:space:]]*:[[:space:]]*"'"${PLUGIN_ID}"'"' "$receipt" \
    || die "bundle.json pluginId is not ${PLUGIN_ID}"
  local receipt_version
  receipt_version=$(grep -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "$receipt" \
    | head -1 \
    | sed 's/.*"\([^"]*\)"$/\1/')
  [[ -n "$receipt_version" ]] || die "bundle.json missing version"
  [[ "$receipt_version" == "$expected_version" ]] \
    || die "bundle.json version ${receipt_version} != expected ${expected_version}"

  local man_version
  man_version=$(grep -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "$manifest" \
    | head -1 \
    | sed 's/.*"\([^"]*\)"$/\1/')
  [[ "$man_version" == "$expected_version" ]] \
    || die "manifest version ${man_version} != expected ${expected_version}"

  # Inventory paths listed in the receipt must exist on disk.
  # bundle.json is not listed in its own files array.
  local missing=0
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    if [[ ! -e "${stage}/${path}" ]]; then
      warn "Receipt path missing on disk: ${path}"
      missing=1
    fi
  done < <(grep -o '"path"[[:space:]]*:[[:space:]]*"[^"]*"' "$receipt" \
    | sed 's/.*"\([^"]*\)"$/\1/')
  [[ "$missing" -eq 0 ]] || die "Staged inventory does not match bundle.json"

  # Helper version must match receipt (BUNDLE-006).
  local helper_version
  helper_version=$("${stage}/bin/agent-bar" version 2>/dev/null | head -1 | tr -d '[:space:]' || true)
  [[ -n "$helper_version" ]] || die "Staged helper did not print a version"
  [[ "$helper_version" == "$expected_version" ]] \
    || die "Helper version ${helper_version} != expected ${expected_version}"

  ok "Staged bundle inventory/receipt validated (${expected_version})"
}

install_plugin() {
  local version="$1"
  local asset="${PLUGIN_ID}-${version}-${TARGET}.tar.zst"
  local base_url="https://github.com/${GITHUB_REPO}/releases/download/v${version}"
  local tmpdir
  tmpdir=$(mktemp -d)
  # shellcheck disable=SC2064
  trap "rm -rf '$tmpdir'" EXIT

  log "Downloading ${asset}..."
  curl -fL --progress-bar "${base_url}/${asset}" -o "${tmpdir}/${asset}"
  curl -fsSL "${base_url}/${asset}.sha256" -o "${tmpdir}/${asset}.sha256"

  log "Verifying checksum..."
  (cd "$tmpdir" && sha256sum -c "${asset}.sha256") >&2 \
    || die "Checksum mismatch — download may be corrupted."
  ok "Checksum OK"

  log "Extracting plugin bundle..."
  mkdir -p "${tmpdir}/extract"
  tar --use-compress-program=zstd -xf "${tmpdir}/${asset}" -C "${tmpdir}/extract"

  [[ -d "${tmpdir}/extract/${PLUGIN_ID}" ]] \
    || die "Archive missing top-level ${PLUGIN_ID}/"

  # Refuse archive paths that escape the extract root (defense in depth).
  if find "${tmpdir}/extract" -name '..' -o -type l | grep -q .; then
    die "Archive contains links or traversal components"
  fi

  mkdir -p "${PLUGINS_DIR}"
  local stage="${PLUGINS_DIR}/.${PLUGIN_ID}.stage-install"
  rm -rf "${stage}"
  cp -a "${tmpdir}/extract/${PLUGIN_ID}" "${stage}"

  # BUNDLE-013: validate staged inventory/receipt before install swap.
  validate_staged_bundle "${stage}" "${version}"

  # Atomic-ish swap: move old aside, move stage into place.
  if [[ -e "${PLUGIN_ROOT}" ]]; then
    local bak="${PLUGINS_DIR}/.${PLUGIN_ID}.prev-install"
    rm -rf "${bak}"
    mv "${PLUGIN_ROOT}" "${bak}"
    if ! mv "${stage}" "${PLUGIN_ROOT}"; then
      mv "${bak}" "${PLUGIN_ROOT}" || true
      die "Failed to install plugin root"
    fi
    rm -rf "${bak}"
  else
    mv "${stage}" "${PLUGIN_ROOT}"
  fi

  ok "Plugin installed at ${PLUGIN_ROOT}"

  rm -rf "$tmpdir"
  trap - EXIT
}

activate_plugin() {
  if ! command -v omarchy >/dev/null 2>&1; then
    warn "omarchy CLI not found. Enable manually: omarchy plugin enable ${PLUGIN_ID}"
    return
  fi

  if [[ -f "${HOME}/.config/omarchy/shell.json" ]] \
    && grep -q "${PLUGIN_ID}" "${HOME}/.config/omarchy/shell.json" 2>/dev/null; then
    log "Existing shell entry — running omarchy plugin rescan"
    omarchy plugin rescan || warn "rescan failed; run: omarchy plugin rescan"
  else
    local proceed=0
    if [[ "$YES" -eq 1 ]]; then
      proceed=1
    elif [[ -t 0 ]]; then
      echo "" >&2
      read -r -p "Enable ${PLUGIN_ID} via omarchy plugin enable? [Y/n] " ans
      case "${ans:-Y}" in
        [yY]|[yY][eE][sS]|"") proceed=1 ;;
        *) proceed=0 ;;
      esac
    else
      warn "Non-interactive install. Run: omarchy plugin enable ${PLUGIN_ID}"
      return
    fi
    if [[ "$proceed" -eq 1 ]]; then
      log "Running omarchy plugin enable ${PLUGIN_ID}"
      omarchy plugin enable "${PLUGIN_ID}" \
        || warn "enable failed; run: omarchy plugin enable ${PLUGIN_ID}"
    fi
  fi
}

main() {
  echo "" >&2
  log "agent-bar plugin installer"
  check_platform
  check_deps

  local version
  version=$(resolve_version)
  version="${version#v}"

  local existing
  existing=$(existing_version || true)
  if [[ -n "$existing" && "$existing" == "$version" && "$FORCE" -eq 0 ]]; then
    ok "agent-bar.usage is already at ${version}"
    exit 0
  fi

  if [[ -n "$existing" ]]; then
    log "Updating agent-bar.usage (${existing} -> ${version})..."
  else
    log "Installing agent-bar.usage ${version}..."
  fi

  install_plugin "$version"
  activate_plugin
  ok "agent-bar.usage ${version} ready"
  log "Private helper: ${PLUGIN_ROOT}/bin/agent-bar"
  log "No global executable was installed."
}

main
