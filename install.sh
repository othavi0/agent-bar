#!/usr/bin/env bash
#
# Installs the Agent Bar plugin for Omarchy Quattro. Nothing else.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh | bash
#
# Flags:
#   --force      Reinstall even if this version is already installed.
#   --yes, -y    Answer yes to every prompt.
#
# Env:
#   AGENT_BAR_VERSION  Version to install. Defaults to the latest release.
#
# Installs to: $HOME/.config/omarchy/plugins/agent-bar.usage
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
      echo "Unknown flag: $arg" >&2
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
  [[ "$uname_s" == "Linux" ]] || die "Agent Bar needs Linux. This is $uname_s."
  arch=$(uname -m 2>/dev/null || echo unknown)
  [[ "$arch" == "x86_64" ]] || die "Agent Bar ships for x86_64 only. This is $arch."
}

check_deps() {
  command -v curl >/dev/null 2>&1 || die "curl is missing. Install it and run this again."
  command -v sha256sum >/dev/null 2>&1 || die "sha256sum is missing. Install coreutils and run this again."
  command -v tar >/dev/null 2>&1 || die "tar is missing. Install it and run this again."
  command -v zstd >/dev/null 2>&1 || die "zstd is missing. Install it and run this again."
}

resolve_version() {
  if [[ -n "${AGENT_BAR_VERSION:-}" ]]; then
    local v="${AGENT_BAR_VERSION}"
    v="${v#v}"
    echo "$v"
    return
  fi
  log "Finding the latest release..."
  local tag
  tag=$(curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" \
    | grep '"tag_name"' \
    | head -1 \
    | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
  [[ -n "$tag" ]] || die "Could not find the release list. Set AGENT_BAR_VERSION and run this again."
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

  [[ -f "$receipt" ]] || die "The download is incomplete: bundle.json is missing."
  [[ -f "$manifest" ]] || die "The download is incomplete: manifest.json is missing."
  [[ -f "${stage}/Service.qml" ]] || die "The download is incomplete: Service.qml is missing."
  [[ -f "${stage}/BarWidget.qml" ]] || die "The download is incomplete: BarWidget.qml is missing."
  [[ -x "${stage}/bin/agent-bar" ]] \
    || die "The download is incomplete: bin/agent-bar is missing or not executable."
  [[ -x "${stage}/scripts/agent-bar-open-terminal" ]] \
    || die "The download is incomplete: the terminal helper is missing or not executable."

  # Receipt pluginId + version must match expected install target.
  grep -q '"pluginId"[[:space:]]*:[[:space:]]*"'"${PLUGIN_ID}"'"' "$receipt" \
    || die "This download is for another plugin, not ${PLUGIN_ID}."
  local receipt_version
  receipt_version=$(grep -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "$receipt" \
    | head -1 \
    | sed 's/.*"\([^"]*\)"$/\1/')
  [[ -n "$receipt_version" ]] || die "The download is incomplete: bundle.json has no version."
  [[ "$receipt_version" == "$expected_version" ]] \
    || die "The download is version ${receipt_version}, not ${expected_version}."

  local man_version
  man_version=$(grep -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "$manifest" \
    | head -1 \
    | sed 's/.*"\([^"]*\)"$/\1/')
  [[ "$man_version" == "$expected_version" ]] \
    || die "The manifest is version ${man_version}, not ${expected_version}."

  # Inventory paths listed in the receipt must exist on disk.
  # bundle.json is not listed in its own files array.
  local missing=0
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    if [[ ! -e "${stage}/${path}" ]]; then
      warn "The download is missing a file it lists: ${path}"
      missing=1
    fi
  done < <(grep -o '"path"[[:space:]]*:[[:space:]]*"[^"]*"' "$receipt" \
    | sed 's/.*"\([^"]*\)"$/\1/')
  [[ "$missing" -eq 0 ]] || die "The download does not match its own receipt."

  # Helper version must match receipt (BUNDLE-006).
  local helper_version
  helper_version=$("${stage}/bin/agent-bar" version 2>/dev/null | head -1 | tr -d '[:space:]' || true)
  [[ -n "$helper_version" ]] || die "The helper in this download does not run here."
  [[ "$helper_version" == "$expected_version" ]] \
    || die "The helper is version ${helper_version}, not ${expected_version}."

  ok "Download verified (${expected_version})"
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

  log "Verifying the download..."
  (cd "$tmpdir" && sha256sum -c "${asset}.sha256") >&2 \
    || die "The download is corrupted. Run this again."
  ok "Checksum matches"

  log "Extracting..."
  mkdir -p "${tmpdir}/extract"
  tar --use-compress-program=zstd -xf "${tmpdir}/${asset}" -C "${tmpdir}/extract"

  [[ -d "${tmpdir}/extract/${PLUGIN_ID}" ]] \
    || die "The archive has no ${PLUGIN_ID} directory."

  # Refuse archive paths that escape the extract root (defense in depth).
  if find "${tmpdir}/extract" -name '..' -o -type l | grep -q .; then
    die "The archive contains unsafe paths. Nothing was installed."
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
      die "Could not write to ${PLUGINS_DIR}."
    fi
    rm -rf "${bak}"
  else
    mv "${stage}" "${PLUGIN_ROOT}"
  fi

  ok "Installed at ${PLUGIN_ROOT}"

  rm -rf "$tmpdir"
  trap - EXIT
}

activate_plugin() {
  if ! command -v omarchy >/dev/null 2>&1; then
    warn "omarchy was not found. Enable it yourself: omarchy plugin enable ${PLUGIN_ID}"
    return
  fi

  if [[ -f "${HOME}/.config/omarchy/shell.json" ]] \
    && grep -q "${PLUGIN_ID}" "${HOME}/.config/omarchy/shell.json" 2>/dev/null; then
    log "Already in your shell. Rescanning..."
    omarchy plugin rescan || warn "Rescan failed. Run it yourself: omarchy plugin rescan"
  else
    local proceed=0
    if [[ "$YES" -eq 1 ]]; then
      proceed=1
    elif [[ -t 0 ]]; then
      echo "" >&2
      read -r -p "Enable ${PLUGIN_ID} now? [Y/n] " ans
      case "${ans:-Y}" in
        [yY]|[yY][eE][sS]|"") proceed=1 ;;
        *) proceed=0 ;;
      esac
    else
      warn "Nothing to answer here. Run: omarchy plugin enable ${PLUGIN_ID}"
      return
    fi
    if [[ "$proceed" -eq 1 ]]; then
      log "Enabling ${PLUGIN_ID}..."
      omarchy plugin enable "${PLUGIN_ID}" \
        || warn "Enable failed. Run it yourself: omarchy plugin enable ${PLUGIN_ID}"
    fi
  fi
}

main() {
  echo "" >&2
  log "Agent Bar installer"
  check_platform
  check_deps

  local version
  version=$(resolve_version)
  version="${version#v}"

  local existing
  existing=$(existing_version || true)
  if [[ -n "$existing" && "$existing" == "$version" && "$FORCE" -eq 0 ]]; then
    ok "Already at ${version}. Use --force to reinstall."
    exit 0
  fi

  if [[ -n "$existing" ]]; then
    log "Updating ${existing} to ${version}..."
  else
    log "Installing ${version}..."
  fi

  install_plugin "$version"
  activate_plugin
  ok "Agent Bar ${version} is ready"
  log "Helper: ${PLUGIN_ROOT}/bin/agent-bar"
  log "No global executable was installed."
}

main
