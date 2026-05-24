# Install script `install.sh` — distribuição zero-poluição

**Data:** 2026-05-23
**Status:** Design aprovado, aguardando implementação

## Contexto

Hoje o caminho recomendado de install é `bun add -g @noctuacore/agent-bar`,
que carrega o risco de poluir `$HOME` se o user esquecer `-g`. Construímos
defesa em camadas (detector no shim, doctor, setup hint), mas o risco
estrutural permanece — package managers tratam cwd como projeto por design.

A indústria (bun, deno, rustup, uv, starship) resolveu isso com install
scripts hospedados: `curl ... | bash`. Zero polução porque o script controla
o destino. Vamos adotar o mesmo padrão.

## Objetivo

Eliminar a chance de polução de `$HOME` no caminho recomendado de install,
adotando o padrão da indústria (install script hospedado). Manter `bun add -g`
como caminho alternativo pra quem prefere.

## Escopo aprovado

- Criar `install.sh` na raiz do repo. Servido via
  `https://raw.githubusercontent.com/othavioquiliao/agent-bar/master/install.sh`.
- Atualizar `README.md` pra promover o install script como caminho primário.
- Manter `bun add -g` documentado como alternativa avançada.
- Manter detector no shim e `doctor` (já implementados) como rede de segurança
  pra quem usar `bun add -g`.

## Decisões

### D1. Destino do checkout: `~/.agent-bar` (compat com managed-git existente)

O `update.ts` já detecta `~/.agent-bar` como `managed-git` install kind e
sabe fazer `git pull` + `bun install` + re-setup. Reusamos essa infra zero-cost.

Alternativa rejeitada: `~/.local/share/agent-bar` (XDG-compliant). Forçaria
re-trabalho em `update.ts`, `uninstall.ts`, e quebraria existing managed
installs. YAGNI — `~/.agent-bar` funciona.

### D2. Symlink: `~/.local/bin/agent-bar` (responsabilidade do `setup`)

Install script só garante que o checkout existe em `~/.agent-bar`. O
symlink em `~/.local/bin/agent-bar` continua sendo criado por `agent-bar setup`
(que já faz isso hoje em `createSymlink`). Install script chama `setup` ao
fim opcionalmente.

### D3. Update flow: re-rodar install script OU `agent-bar update`

Dois caminhos válidos:
- `curl ... | bash` — script detecta install existente em `~/.agent-bar`,
  faz `git pull` em vez de clone.
- `agent-bar update` — já funciona pra managed-git, sem mudança.

Ambos funcionam. README documenta `agent-bar update` como caminho idiomático.

### D4. Pré-requisitos validados no script: bun + git

- `bun` ausente: script aborta com mensagem instruindo install do bun
  (`curl -fsSL https://bun.sh/install | bash`).
- `git` ausente: aborta com mensagem.
- `curl`/`wget` não validado — se o script chegou aqui, um dos dois existe.

### D5. Plataforma: Linux only (warning explícito em macOS)

Agent-bar é Waybar-only (Wayland Linux). Script detecta plataforma e aborta
em macOS/Windows com mensagem clara. Linux x64/arm64 funcionam igualmente
(JS, sem nativos).

### D6. Diretório existente em `~/.agent-bar`

- Se é git checkout com remote do agent-bar: `git pull` + reinstall deps.
- Se é git checkout de outro remote: aborta com erro (não sobrescreve).
- Se não é git checkout: aborta com erro (peça pra rodar `rm -rf ~/.agent-bar` manualmente).
- Flag `--force`: pula esses checks (sobrescreve).

### D7. Setup automático ao fim

- TTY + sem `--no-setup`: pergunta "Run setup now? [Y/n]".
- Non-TTY ou `--yes`: roda setup automaticamente (sem prompt).
- `--no-setup`: pula.

## Design

### `install.sh` (raiz do repo)

Bash script (~100 linhas). Responsabilidades:

1. Detectar plataforma. Aborta se não-Linux.
2. Detectar `bun` e `git`. Aborta com instruções se faltarem.
3. Decidir install dir (`AGENT_BAR_HOME` env override, default `$HOME/.agent-bar`).
4. Resolver estado do install dir:
   - Não existe → `git clone --depth 1 <repo> <dir>`.
   - Existe + git válido + remote correto → `git pull --ff-only`.
   - Existe + inválido → erro (a menos que `--force`).
5. `cd $INSTALL_DIR && bun install --production` (deps).
6. Mensagem de sucesso com path do install.
7. Se TTY + sem `--no-setup`: prompt pra rodar setup.
8. Roda setup se confirmado: `exec "$INSTALL_DIR/scripts/agent-bar" setup`.

Flags:
- `--force`: sobrescreve install dir existente.
- `--no-setup`: skip setup prompt.
- `--yes` / `-y`: assume yes pra prompts (non-interactive).

Variáveis de ambiente:
- `AGENT_BAR_HOME`: override do install dir (default `$HOME/.agent-bar`).
- `AGENT_BAR_REPO`: override do repo (default
  `https://github.com/othavioquiliao/agent-bar.git`).
- `AGENT_BAR_BRANCH`: override da branch (default `master`).

Saída:
- Mensagens estruturadas em stderr (não pollui stdout — boa prática
  pra quem encadeia `| sh`).
- Cores apenas se TTY.

### `README.md` — seção Install reescrita

```markdown
## Install

Recommended:

\`\`\`bash
curl -fsSL https://raw.githubusercontent.com/othavioquiliao/agent-bar/master/install.sh | bash
\`\`\`

Requires `bun` and `git`. Installs to `~/.agent-bar` and runs setup.

### Alternative: Bun global

If you already use Bun globally:

\`\`\`bash
bun add -g @noctuacore/agent-bar && agent-bar setup
\`\`\`

⚠ If you accidentally drop the `-g`, run `agent-bar doctor` to clean up.
```

Update section permanece igual (`agent-bar update`).

### Não-objetivos

- Hosting de domínio dedicado (agent-bar.dev). Usa raw.githubusercontent.com.
- Suporte a macOS/Windows. Waybar é Wayland-Linux-only.
- Suporte a outros package managers (deb, rpm). YAGNI.
- AUR. Pode vir depois como caminho alternativo, fora deste spec.
- Binário compilado via `bun build --compile`. Mais complexo, fora deste spec.

## Riscos

| Risco | Mitigação |
|---|---|
| `curl \| bash` é controverso por segurança | Padrão da indústria, OSS, código visível no repo, GitHub URL canônica e versionada. |
| Install script bugado vira problema crítico (todo install passa por ele) | Script simples (~100 linhas), testado em CI antes de cada release. |
| `~/.agent-bar` existe de install antigo (managed-git pré-this-spec) | Detect git + remote — se for o nosso repo, git pull. Funciona transparente. |
| Bun não instalado: user fica frustrado | Mensagem clara com comando de install do bun. |
| Falha de rede no meio do clone | `git clone` falha visível, user re-roda. Sem state corrompido (clone é atômico). |

## Verificação

Após implementação:

```bash
# Smoke 1: install limpo
HOME=$(mktemp -d) bash install.sh --no-setup --yes
# Verifica ~/.agent-bar/scripts/agent-bar existe, ~/package.json NÃO existe

# Smoke 2: re-install (idempotente)
HOME=$(mktemp -d) bash install.sh --no-setup --yes
HOME=$(mktemp -d) bash install.sh --no-setup --yes  # 2nd run = git pull

# Smoke 3: plataforma não-Linux
OSTYPE=darwin bash install.sh  # aborta com mensagem

# Smoke 4: $HOME limpo após install
HOME=$(mktemp -d) bash install.sh --no-setup --yes
ls $HOME  # só ".agent-bar/"
```

## Out of scope (futuros)

- AUR package (`agent-bar-bin` ou `agent-bar-git`).
- `bun build --compile` binary release no GitHub.
- Telemetria opcional de install count.
- `install.sh` em mirror próprio (agent-bar.dev).
