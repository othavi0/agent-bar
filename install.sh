#!/usr/bin/env bash
#
# agent-bar installer — zero-toolchain binary install.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh | bash
#
# Flags:
#   --force      Reinstall even if already up to date (no prompting).
#   --no-setup   Skip the `agent-bar setup` step at the end.
#   --yes, -y    Assume yes for prompts (non-interactive).
#
# Env:
#   AGENT_BAR_VERSION  Version to install (default: latest release tag).
#   AGENT_BAR_DATA     Data dir for icons/scripts (default: $HOME/.local/share/agent-bar).

set -euo pipefail

# --- config ----------------------------------------------------------------

GITHUB_REPO="othavi0/agent-bar"
BIN_DIR="$HOME/.local/bin"
DATA_DIR="${AGENT_BAR_DATA:-$HOME/.local/share/agent-bar}"

FORCE=0
NO_SETUP=0
YES=0

for arg in "$@"; do
  case "$arg" in
    --force)    FORCE=1 ;;
    --no-setup) NO_SETUP=1 ;;
    --yes|-y)   YES=1 ;;
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

# --- helpers ---------------------------------------------------------------

is_tty() { [[ -t 1 ]]; }
is_interactive() { [[ -t 0 ]]; }

if is_tty; then
  C_RED='\033[31m'
  C_GREEN='\033[32m'
  C_YELLOW='\033[33m'
  C_BLUE='\033[34m'
  C_BOLD='\033[1m'
  C_RESET='\033[0m'
else
  C_RED='' C_GREEN='' C_YELLOW='' C_BLUE='' C_BOLD='' C_RESET=''
fi

log()  { echo -e "${C_BLUE}==>${C_RESET} $*" >&2; }
ok()   { echo -e "${C_GREEN}✓${C_RESET} $*" >&2; }
warn() { echo -e "${C_YELLOW}!${C_RESET} $*" >&2; }
die()  { echo -e "${C_RED}✗${C_RESET} $*" >&2; exit 1; }

# --- pre-flight checks -----------------------------------------------------

check_platform() {
  local uname_s
  uname_s=$(uname -s 2>/dev/null || echo unknown)
  if [[ "$uname_s" != "Linux" ]]; then
    die "agent-bar requires Linux (Waybar is Wayland-only). Detected: $uname_s"
  fi

  local arch
  arch=$(uname -m 2>/dev/null || echo unknown)
  if [[ "$arch" != "x86_64" ]]; then
    die "Only x86_64 prebuilt binaries are available. Detected: $arch"
  fi
}

check_deps() {
  command -v curl     >/dev/null 2>&1 || die "curl not found. Install via your distro's package manager."
  command -v sha256sum >/dev/null 2>&1 || die "sha256sum not found. Install coreutils via your distro's package manager."
  command -v tar      >/dev/null 2>&1 || die "tar not found. Install via your distro's package manager."
}

# --- version resolution ----------------------------------------------------

resolve_version() {
  if [[ -n "${AGENT_BAR_VERSION:-}" ]]; then
    echo "$AGENT_BAR_VERSION"
    return
  fi
  log "Resolving latest release..."
  local tag
  tag=$(curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" \
    | grep '"tag_name"' \
    | head -1 \
    | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
  if [[ -z "$tag" ]]; then
    die "Could not resolve latest release tag. Set AGENT_BAR_VERSION to install a specific version."
  fi
  echo "$tag"
}

# --- existing-install detection ---------------------------------------------

# Captura `--version` do binário instalado, mantém só a primeira linha, e
# valida contra um padrão semver simples. Qualquer outra coisa (JSON de
# binário da era TS pré-5.0.0, saída vazia, erro) vira "legacy".
detect_existing_version() {
  local bin_path="$1"
  local raw
  raw=$("$bin_path" --version 2>/dev/null | head -n1) || true
  if [[ "$raw" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+ ]]; then
    echo "$raw"
  else
    echo "legacy"
  fi
}

# --- legacy npm/bun cleanup --------------------------------------------------

# Best-effort: uma instalação global via npm/bun (era pré-5.0.0) pode deixar
# o pacote registrado e/ou um symlink em $BIN_DIR apontando pra node_modules.
# Nunca fatal — cada passo segue mesmo se falhar.
cleanup_legacy_npm() {
  if command -v npm >/dev/null 2>&1; then
    if npm ls -g "@noctuacore/agent-bar" >/dev/null 2>&1; then
      log "Removing legacy npm package..."
      npm rm -g "@noctuacore/agent-bar" >/dev/null 2>&1 \
        || warn "Failed to remove legacy npm package (continuing)."
    fi
  fi

  if command -v bun >/dev/null 2>&1; then
    if bun pm ls -g 2>/dev/null | grep -q "@noctuacore/agent-bar"; then
      log "Removing legacy bun package..."
      bun remove -g "@noctuacore/agent-bar" >/dev/null 2>&1 \
        || warn "Failed to remove legacy bun package (continuing)."
    fi
  fi

  if [[ -L "${BIN_DIR}/agent-bar" ]]; then
    log "Removing stale symlink at ${BIN_DIR}/agent-bar..."
    rm -f "${BIN_DIR}/agent-bar" || warn "Failed to remove stale symlink (continuing)."
  fi
}

# --- download + verify + extract -------------------------------------------

install_binary() {
  local version="$1"
  # Normaliza pra ter sempre o prefixo 'v' (a tag do Release é vX.Y.Z; AGENT_BAR_VERSION
  # pode vir sem). base_url usa a tag (com 'v'); o asset usa a versão SEM 'v'.
  [[ "$version" == v* ]] || version="v${version}"
  local ver_bare="${version#v}"
  local asset="agent-bar-${ver_bare}-x86_64.tar.gz"
  local base_url="https://github.com/${GITHUB_REPO}/releases/download/${version}"

  local tmpdir
  tmpdir=$(mktemp -d)
  # shellcheck disable=SC2064
  trap "rm -rf '$tmpdir'" EXIT

  log "Downloading ${asset}..."
  curl -fL  --progress-bar "${base_url}/${asset}"        -o "${tmpdir}/${asset}"
  curl -fsSL               "${base_url}/${asset}.sha256"  -o "${tmpdir}/${asset}.sha256"

  log "Verifying checksum..."
  (cd "$tmpdir" && sha256sum -c "${asset}.sha256") >&2 \
    || die "Checksum mismatch — download may be corrupted. Try again."
  ok "Checksum OK"

  log "Extracting..."
  tar xzf "${tmpdir}/${asset}" -C "$tmpdir"

  # Install binary
  mkdir -p "$BIN_DIR"
  install -Dm755 "${tmpdir}/agent-bar" "${BIN_DIR}/agent-bar"
  ok "Binary installed at ${BIN_DIR}/agent-bar"

  # Install data assets (icons + helper script)
  mkdir -p "${DATA_DIR}/icons" "${DATA_DIR}/scripts"
  if [[ -d "${tmpdir}/icons" ]]; then
    cp -r "${tmpdir}/icons/." "${DATA_DIR}/icons/"
  fi
  if [[ -f "${tmpdir}/scripts/agent-bar-open-terminal" ]]; then
    install -Dm755 "${tmpdir}/scripts/agent-bar-open-terminal" \
      "${DATA_DIR}/scripts/agent-bar-open-terminal"
  fi
  ok "Assets installed at ${DATA_DIR}"

  # Limpa o tmpdir explicitamente: o `exec agent-bar setup` (caminho default) pula
  # a EXIT trap, então a limpeza precisa acontecer aqui.
  rm -rf "$tmpdir"
  trap - EXIT
}

# --- PATH check ------------------------------------------------------------

check_path() {
  case ":${PATH}:" in
    *":${BIN_DIR}:"*) : ;;  # already in PATH
    *)
      warn "${BIN_DIR} is not in your \$PATH."
      warn "Add the following to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
      warn "  export PATH=\"\$HOME/.local/bin:\$PATH\""
      ;;
  esac
}

# --- optional setup --------------------------------------------------------

maybe_setup() {
  if [[ "$NO_SETUP" -eq 1 ]]; then
    return
  fi

  local proceed=0
  if [[ "$YES" -eq 1 ]]; then
    proceed=1
  elif is_interactive; then
    echo "" >&2
    read -r -p "Run 'agent-bar setup' now to wire Waybar? [Y/n] " ans
    case "${ans:-Y}" in
      [yY]|[yY][eE][sS]|"") proceed=1 ;;
      *) proceed=0 ;;
    esac
  else
    warn "Non-interactive install. Run 'agent-bar setup' manually to wire Waybar."
  fi

  if [[ "$proceed" -eq 1 ]]; then
    log "Running setup..."
    AGENT_BAR_ASSET_DIR="$DATA_DIR" exec "${BIN_DIR}/agent-bar" setup
  fi
}

# --- main ------------------------------------------------------------------

main() {
  echo "" >&2
  log "${C_BOLD}agent-bar installer${C_RESET}"
  check_platform
  check_deps

  local version
  version=$(resolve_version)
  [[ "$version" == v* ]] || version="v${version}"
  local ver_bare="${version#v}"

  if [[ -x "${BIN_DIR}/agent-bar" ]]; then
    local existing_ver existing_bare
    existing_ver=$(detect_existing_version "${BIN_DIR}/agent-bar")
    existing_bare="${existing_ver#v}"

    if [[ "$existing_ver" != "legacy" && "$existing_bare" == "$ver_bare" ]]; then
      if [[ "$FORCE" -eq 0 ]]; then
        ok "agent-bar is already up to date (v${ver_bare})."
        exit 0
      fi
      log "Reinstalling agent-bar ${version} (--force)..."
    else
      log "Updating agent-bar (${existing_ver} -> ${version})..."
    fi
  else
    log "Installing agent-bar ${version}..."
  fi

  cleanup_legacy_npm
  install_binary "$version"
  check_path

  ok "agent-bar ${version} installed!"

  maybe_setup

  echo "" >&2
  log "Tip: have cargo? ${C_BOLD}cargo binstall agent-bar${C_RESET} also works."
}

main
