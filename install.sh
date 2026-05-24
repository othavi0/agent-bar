#!/usr/bin/env bash
#
# agent-bar installer — zero-pollution distribution.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/othavioquiliao/agent-bar/master/install.sh | bash
#
# Flags:
#   --force      Overwrite existing ~/.agent-bar (non-git or wrong remote).
#   --no-setup   Skip the `agent-bar setup` prompt at the end.
#   --yes, -y    Assume yes for prompts (non-interactive).
#
# Env:
#   AGENT_BAR_HOME    Install dir (default: $HOME/.agent-bar).
#   AGENT_BAR_REPO    Git repo URL (default: github.com/othavioquiliao/agent-bar.git).
#   AGENT_BAR_BRANCH  Branch to clone (default: master).

set -euo pipefail

REPO_URL="${AGENT_BAR_REPO:-https://github.com/othavioquiliao/agent-bar.git}"
BRANCH="${AGENT_BAR_BRANCH:-master}"
INSTALL_DIR="${AGENT_BAR_HOME:-$HOME/.agent-bar}"

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

# --- pre-flight checks ----------------------------------------------------

check_platform() {
  local uname_s
  uname_s=$(uname -s 2>/dev/null || echo unknown)
  if [[ "$uname_s" != "Linux" ]]; then
    die "agent-bar requires Linux (Waybar is Wayland-only). Detected: $uname_s"
  fi
}

check_deps() {
  command -v bun >/dev/null 2>&1 || die "bun not found. Install: curl -fsSL https://bun.sh/install | bash"
  command -v git >/dev/null 2>&1 || die "git not found. Install via your distro's package manager."
}

# --- install dir resolution -----------------------------------------------

clone_or_update() {
  if [[ ! -d "$INSTALL_DIR" ]]; then
    log "Cloning $REPO_URL into $INSTALL_DIR"
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$INSTALL_DIR" >&2
    ok "Cloned"
    return
  fi

  if [[ ! -d "$INSTALL_DIR/.git" ]]; then
    if [[ "$FORCE" -eq 1 ]]; then
      warn "Overwriting non-git $INSTALL_DIR (--force)"
      rm -rf "$INSTALL_DIR"
      git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$INSTALL_DIR" >&2
      ok "Cloned"
      return
    fi
    die "$INSTALL_DIR exists but is not a git checkout. Use --force to overwrite or remove it manually."
  fi

  local current_remote
  current_remote=$(git -C "$INSTALL_DIR" remote get-url origin 2>/dev/null || echo "")
  if [[ "$current_remote" != "$REPO_URL" ]]; then
    if [[ "$FORCE" -eq 1 ]]; then
      warn "Overwriting checkout with different remote (--force): $current_remote"
      rm -rf "$INSTALL_DIR"
      git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$INSTALL_DIR" >&2
      ok "Cloned"
      return
    fi
    die "$INSTALL_DIR points to a different remote ($current_remote). Use --force to overwrite."
  fi

  log "Existing checkout found. Updating..."
  git -C "$INSTALL_DIR" fetch --depth 1 origin "$BRANCH" >&2
  git -C "$INSTALL_DIR" reset --hard "origin/$BRANCH" >&2
  ok "Updated"
}

install_deps() {
  log "Installing dependencies..."
  (cd "$INSTALL_DIR" && bun install) >&2
  ok "Dependencies ready"
}

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
    exec "$INSTALL_DIR/scripts/agent-bar" setup
  fi
}

main() {
  echo "" >&2
  log "${C_BOLD}agent-bar installer${C_RESET}"
  check_platform
  check_deps
  clone_or_update
  install_deps

  ok "Installed at $INSTALL_DIR"
  warn "Add ~/.local/bin to your PATH if it isn't already (setup will symlink the binary there)."

  maybe_setup
}

main
