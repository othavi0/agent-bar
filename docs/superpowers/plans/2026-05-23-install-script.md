# Install Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminar a chance de polução de `$HOME` no caminho recomendado de install, fornecendo `install.sh` hospedado no repo (curl|bash pattern).

**Architecture:** Bash script `install.sh` na raiz do repo, servido via raw.githubusercontent.com. Faz git clone em `~/.agent-bar` (reusa managed-git install kind existente em `src/update.ts`), instala deps com `bun install`, opcionalmente roda `agent-bar setup`. README promove curl|bash como caminho primário; `bun add -g` vira alternativa documentada.

**Tech Stack:** Bash 4+, git, bun. Sem novos arquivos TS, sem novos testes bun:test (smokes Bash).

**Spec:** `docs/superpowers/specs/2026-05-23-install-script-design.md`

---

## File Map

- **Create**
  - `install.sh` (raiz do repo) — entry point do install hospedado.
- **Modify**
  - `README.md` — seção Install promovendo curl|bash como primário.
  - `CHANGELOG.md` — `[Unreleased]` com Added (install script) + Changed (README).

`install.sh` é Bash auto-contido. Sem teste bun:test (lifecycle Bash é difícil de testar com a infra do projeto). Verificação via smokes manuais documentados no próprio plano.

`package.json` `files` field NÃO é modificado — `install.sh` é pra clone do repo, não pra publicação npm. Não vai no tarball npm.

---

## Task 1: Criar `install.sh`

**Files:**
- Create: `install.sh`

Script Bash auto-contido, ~120 linhas. Funções nomeadas pra cada responsabilidade. Smokes verificam cada caminho.

- [ ] **Step 1: Criar `install.sh` na raiz com conteúdo completo**

```bash
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

  # Dir exists. Decide: pull, abort, or force-overwrite.
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

# --- deps install ---------------------------------------------------------

install_deps() {
  log "Installing dependencies..."
  (cd "$INSTALL_DIR" && bun install) >&2
  ok "Dependencies ready"
}

# --- setup prompt ---------------------------------------------------------

maybe_setup() {
  if [[ "$NO_SETUP" -eq 1 ]]; then
    return
  fi

  local proceed=1
  if is_tty && [[ "$YES" -eq 0 ]]; then
    echo "" >&2
    read -r -p "Run 'agent-bar setup' now to wire Waybar? [Y/n] " ans
    case "${ans:-Y}" in
      [yY]|[yY][eE][sS]|"") proceed=1 ;;
      *) proceed=0 ;;
    esac
  fi

  if [[ "$proceed" -eq 1 ]]; then
    log "Running setup..."
    exec "$INSTALL_DIR/scripts/agent-bar" setup
  fi
}

# --- main -----------------------------------------------------------------

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
```

- [ ] **Step 2: Tornar o script executável**

Run: `chmod +x install.sh`
Expected: arquivo fica com flag +x. Validar com `ls -la install.sh`.

- [ ] **Step 3: Smoke 1 — Help flag**

Run: `bash install.sh --help`
Expected: exit 0, imprime as primeiras linhas de comentário do script (usage, flags, env).

- [ ] **Step 4: Smoke 2 — Plataforma não-Linux aborta**

Não dá pra mudar `uname -s` real. Editar momentaneamente a função `check_platform` é overkill. Vale apenas inspecionar visualmente o código: a função usa `uname -s` e aborta se != "Linux". Confiamos no código.

Alternativa de teste real (opcional, requer container Docker macOS-style — overkill): pular.

Marcar como verificado por inspeção.

- [ ] **Step 5: Smoke 3 — Flag inválida**

Run: `bash install.sh --foo`
Expected: exit 2, mensagem "unknown flag: --foo" em stderr.

- [ ] **Step 6: Smoke 4 — Install limpo em HOME temp**

```bash
TMP=$(mktemp -d)
HOME="$TMP" AGENT_BAR_HOME="$TMP/.agent-bar" bash install.sh --no-setup --yes
```

Expected:
- exit 0
- `$TMP/.agent-bar/.git` existe
- `$TMP/.agent-bar/scripts/agent-bar` existe
- `$TMP/.agent-bar/node_modules` existe (deps instaladas)
- `$TMP/package.json` NÃO existe (zero polução)
- `$TMP/bun.lock` NÃO existe

Verificação:
```bash
[[ -d "$TMP/.agent-bar/.git" ]] && echo "git ok"
[[ -x "$TMP/.agent-bar/scripts/agent-bar" ]] && echo "bin ok"
[[ -d "$TMP/.agent-bar/node_modules" ]] && echo "deps ok"
[[ ! -f "$TMP/package.json" ]] && echo "no pollution"
rm -rf "$TMP"
```

Esperado: 4 "ok" prints.

- [ ] **Step 7: Smoke 5 — Re-install é idempotente (git pull em vez de clone)**

```bash
TMP=$(mktemp -d)
HOME="$TMP" AGENT_BAR_HOME="$TMP/.agent-bar" bash install.sh --no-setup --yes
HOME="$TMP" AGENT_BAR_HOME="$TMP/.agent-bar" bash install.sh --no-setup --yes 2>&1 | tee /tmp/install-2nd.log
grep -q "Updating" /tmp/install-2nd.log && echo "pull path ok"
grep -q "Cloning" /tmp/install-2nd.log && echo "ERRO: clonou de novo" || echo "no re-clone ok"
rm -rf "$TMP" /tmp/install-2nd.log
```

Expected: ambos "ok" prints (atualizou em vez de clonar).

- [ ] **Step 8: Smoke 6 — Dir não-git existente aborta sem --force**

```bash
TMP=$(mktemp -d)
mkdir -p "$TMP/.agent-bar"
echo "junk" > "$TMP/.agent-bar/some-file"
HOME="$TMP" AGENT_BAR_HOME="$TMP/.agent-bar" bash install.sh --no-setup --yes 2>&1 | tee /tmp/install-junk.log || true
grep -q "not a git checkout" /tmp/install-junk.log && echo "abort ok"
[[ -f "$TMP/.agent-bar/some-file" ]] && echo "untouched ok"
rm -rf "$TMP" /tmp/install-junk.log
```

Expected: ambos "ok" prints (abortou + não tocou no dir).

- [ ] **Step 9: Smoke 7 — --force overrides dir existente não-git**

```bash
TMP=$(mktemp -d)
mkdir -p "$TMP/.agent-bar"
echo "junk" > "$TMP/.agent-bar/some-file"
HOME="$TMP" AGENT_BAR_HOME="$TMP/.agent-bar" bash install.sh --no-setup --yes --force
[[ ! -f "$TMP/.agent-bar/some-file" ]] && echo "overwritten ok"
[[ -d "$TMP/.agent-bar/.git" ]] && echo "cloned ok"
rm -rf "$TMP"
```

Expected: ambos "ok" prints.

- [ ] **Step 10: Lint do shell**

Run: `command -v shellcheck >/dev/null 2>&1 && shellcheck install.sh || echo "shellcheck not installed, skipping"`
Expected: zero warnings (ou skip se shellcheck não disponível).

Se houver warnings, corrigir antes de commitar. Padrão comum:
- Quotar variáveis: `"$var"` em vez de `$var`.
- `local x; x=$(...)` em vez de `local x=$(...)` sob `set -e`.

- [ ] **Step 11: Verificar projeto não regredindo**

Run: `bun test && bun run typecheck && bun run lint`
Expected: all green (install.sh não impacta TS, mas validar mesmo assim).

- [ ] **Step 12: Commit**

```bash
git add install.sh
git commit -m "feat: install.sh hospedado para zero poluição"
```

(PT, conventional, 48 chars.)

---

## Task 2: Atualizar `README.md`

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Substituir a seção `## Install`**

Localizar o bloco atual:

```markdown
## Install

Requires Bun.

\`\`\`bash
cd /tmp && bun add -g @noctuacore/agent-bar && agent-bar setup
\`\`\`

[... resto da seção ...]
```

Substituir por:

```markdown
## Install

Recommended (zero pollution, runs setup automatically):

\`\`\`bash
curl -fsSL https://raw.githubusercontent.com/othavioquiliao/agent-bar/master/install.sh | bash
\`\`\`

Requires `bun` and `git`. Installs to `~/.agent-bar` and runs `agent-bar setup`.

`setup` installs the Waybar modules, CSS, provider icons, terminal helper, and
`~/.local/bin/agent-bar` symlink.

To update later, run:

\`\`\`bash
agent-bar update
\`\`\`

### Alternative: Bun global

If you already use Bun globally and prefer that workflow:

\`\`\`bash
bun add -g @noctuacore/agent-bar && agent-bar setup
\`\`\`

> ⚠ Don't drop the `-g`. Without it, `bun add` writes `package.json` + `bun.lock`
> to your current directory. If that happens, run `agent-bar doctor` to clean up.

For development, use a normal checkout:

\`\`\`bash
git clone git@github.com:othavioquiliao/agent-bar.git
cd agent-bar
bun install
bun run start status
\`\`\`
```

(Inspecionar o README atual antes pra confirmar o que está lá e fazer a
substituição cirúrgica — outras seções permanecem intactas.)

- [ ] **Step 2: Verificar render mental**

Re-ler a seção pra garantir: install primário é o curl|bash; alternativa Bun
ainda documentada mas com warning sobre `-g`; dev checkout permanece.

- [ ] **Step 3: Lint**

Run: `bun run lint`
Expected: limpo (Biome ignora markdown, mas validar).

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: README promove install.sh como primário"
```

(PT, conventional, 49 chars.)

---

## Task 3: Atualizar `CHANGELOG.md`

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Adicionar entries sob `[Unreleased]`**

Estado atual de `[Unreleased]` (depois dos commits anteriores):
- `### Added` — doctor command, setup hint, **detector no shim Bash**.
- `### Changed` — README snippet defensivo.
- `### Removed` — preinstall script.

Adicionar:

Em `### Added`:

```markdown
- `install.sh` hosted installer: zero-pollution install path via
  `curl -fsSL .../install.sh | bash`. Clones to `~/.agent-bar`, installs deps,
  and optionally runs `agent-bar setup`. Adopts the curl|bash pattern used by
  bun, deno, rustup, uv, and other serious CLI tools.
```

Em `### Changed`, substituir a entry sobre README:

```markdown
- README now promotes the hosted install script as the primary install path.
  `bun add -g` remains documented as an alternative with explicit warning about
  the `-g` flag.
```

- [ ] **Step 2: Verificação final**

Run:
```bash
bun test && bun run typecheck && bun run lint
```
Expected: all green.

Run smoke quick (clean install em temp):
```bash
TMP=$(mktemp -d)
HOME="$TMP" AGENT_BAR_HOME="$TMP/.agent-bar" bash install.sh --no-setup --yes
[[ -d "$TMP/.agent-bar/.git" ]] && [[ ! -f "$TMP/package.json" ]] && echo "all good"
rm -rf "$TMP"
```
Expected: "all good".

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: CHANGELOG do install.sh"
```

(PT, conventional, 31 chars.)

---

## Self-Review

**Spec coverage:**
- D1 (destino `~/.agent-bar`) → Task 1 (`INSTALL_DIR="${AGENT_BAR_HOME:-$HOME/.agent-bar}"`).
- D2 (symlink via setup) → Task 1 (`maybe_setup` chama `agent-bar setup` que cuida disso).
- D3 (update via re-run ou `agent-bar update`) → Task 1 (clone_or_update faz pull) + README menciona `agent-bar update`.
- D4 (pré-requisitos bun + git) → Task 1 `check_deps`.
- D5 (Linux only) → Task 1 `check_platform`.
- D6 (dir existente: pull/abort/force) → Task 1 `clone_or_update` com 3 branches.
- D7 (setup prompt: TTY confirma, --yes pula prompt, --no-setup pula tudo) → Task 1 `maybe_setup`.
- Flags `--force`/`--no-setup`/`--yes`/`-y` → Task 1 flag parsing.
- Env `AGENT_BAR_HOME`/`AGENT_BAR_REPO`/`AGENT_BAR_BRANCH` → Task 1 topo do script.
- README atualizado → Task 2.
- CHANGELOG → Task 3.

**Placeholders:** nenhum. Todo step tem código ou comando concreto.

**Consistency:**
- `INSTALL_DIR` usado em todas as funções.
- `is_tty`/`log`/`ok`/`warn`/`die` definidos antes do uso.
- `maybe_setup` faz `exec` — substitui processo, então não há retorno (consistente com pattern de install scripts que terminam dando controle pra setup).
- README links pro raw.githubusercontent.com com path correto da branch `master`.
