# Design — Distribuição AUR (`agent-bar-bin`) via binário standalone

- **Data:** 2026-06-18
- **Status:** aprovado (design, pós-grill); aguardando revisão da spec
- **Escopo:** Tier 3 — distribuição. Pacote AUR `-bin` baixando um binário
  `bun build --compile` anexado ao GitHub Release.
- **Branch:** `master`
- **Revisão:** endurecida por um grill adversarial (Opus). Achados empíricos
  validados: o binário compila e roda sem Bun (`--version` = 5.2.0, 59 módulos),
  `import.meta.dir` num binário compilado = `/$bunfs/root`, tamanho ~**91 MB**.

## Contexto e objetivo

agent-bar instala hoje por `install.sh` (checkout em `~/.agent-bar`) ou Bun
global (`bun add -g`). O público — Waybar em Omarchy/Hyprland — é majoritariamente
Arch, onde o caminho idiomático é o **AUR**. Falta um pacote.

**Objetivo:** publicar `agent-bar-bin` no AUR, instalando um **binário
standalone** (sem exigir Bun no runtime do usuário) baixado do GitHub Release,
verificado por sha256, **sem build no PKGBUILD** (vetor do ataque "Atomic Arch"
de jun/2026). Para isso o release passa a produzir o binário, e o agent-bar ganha
quatro mudanças de código (abaixo).

**Não-objetivos (YAGNI / specs separadas):** OIDC trusted publishing +
`--provenance` (próxima spec; `NPM_TOKEN` segue válido); build `aarch64`
(follow-up; v1 é `x86_64`); embed de assets no binário (escolhemos `/usr/share`,
D2); automação do `git push` AUR (handoff manual, precisa da conta AUR).

## Decisões de design

- **D1 — Alvo `bun build --compile --target=bun-linux-x64`** (baseline, não
  `-modern`: compatibilidade ampla de CPU). **Compilar em `ubuntu-latest`** — o
  runner que o `publish.yml` já usa. O binário linka contra a glibc do runner;
  como o **alvo é só Arch** (rolling, sempre a glibc mais nova), o piso flutuante
  do `ubuntu-latest` **nunca morde** — pinar uma versão só valeria pra mirar
  distros não-rolling, que não são alvo. (CPU baseline e glibc são eixos
  ortogonais: baseline cobre a CPU; a glibc fica a cargo do runner e o Arch
  sempre satisfaz. `runner` ≠ `target` — Ubuntu é só o host de build.)
- **D2 — Assets em `/usr/share/agent-bar`, não embutidos.** Binário fica
  asset-free; ícones e helper vão para `/usr/share/agent-bar/` espelhando o
  layout do repo (`icons/`, `scripts/`). Distro-idiomático e mexe pouco no
  código.
- **D3 — Detecção de install compilado via `isCompiledBinary()`** =
  `import.meta.dir.startsWith('/$bunfs')` (VFS do `bun --compile`, validado
  empíricamente). É o **único sinal estrutural** — imune a rename do binário,
  symlink e jogos de PATH. **NÃO** usar basename de `process.execPath` (quebra se
  o binário for renomeado e travaria o nome do pacote pra sempre).
- **D4 — Asset único de release:** tarball `agent-bar-<ver>-x86_64.tar.gz`
  (binário + `icons/` + `scripts/agent-bar-open-terminal` + `LICENSE`). 1
  download, 1 sha256.
- **D5 — PKGBUILD + `.install` + `.SRCINFO` versionados** em `packaging/aur/`
  como fonte da verdade/template. Repo AUR separado; **push é do mantenedor**.
- **D6 — `pkgname=agent-bar-bin`**, `provides=('agent-bar')`,
  `conflicts=('agent-bar')` (futureproof contra um eventual pacote source
  `agent-bar` — comentar o porquê pra não removerem achando vestigial). Sem
  `depends` obrigatório (binário standalone); `optdepends` só com pacotes que
  existem de fato.

## Componentes

### 0. `isCompiledBinary()` — helper fundacional

Novo, num módulo compartilhado (ex.: `src/runtime.ts`):
```ts
export function isCompiledBinary(): boolean {
  return import.meta.dir.startsWith('/$bunfs');
}
```
Consumido por (2), (3) e (4). Pure + trivial de mockar (o plano confirma se vale
um override por env só pra teste, ex.: `AGENT_BAR_FORCE_COMPILED`).

### 1. Build do binário standalone

`bun build --compile --target=bun-linux-x64 --minify --outfile agent-bar src/index.ts`,
**em `ubuntu-22.04`**. Validado: roda sem Bun no PATH, `--version` lê a
`package.json` embutida, `await import('./…')` dinâmicos (string-literal) são
bundlados. ~91 MB (embute a engine JS — trade-off conhecido de não migrar pra
Rust). Smoke no plano: `--version`, `status`, `--provider claude`, `setup` em
temp dirs com Bun fora do PATH.

### 2. `resolveAssetSourceRoot()` — assets fora do repo

Hoje `installWaybarAssets`/`getDefaultWaybarAssetPaths` (`src/waybar-contract.ts`)
leem `icons/`/`scripts/` de `DEFAULT_REPO_ROOT = join(import.meta.dir, '..')`.
Num binário compilado isso vira `/$bunfs/root/..` — inexistente no fs real.

Nova `resolveAssetSourceRoot(): string` com prioridade:
1. `process.env.AGENT_BAR_ASSET_DIR` (override; **exigir caminho absoluto**), se
   contém `icons/`;
2. `/usr/share/agent-bar`, se contém `icons/` (install de sistema);
3. **só quando `!isCompiledBinary()`**: `DEFAULT_REPO_ROOT` (checkout/npm).

No binário compilado o fallback (3) é pulado: se (1)/(2) falham, erro **claro**
— `Asset directory not found. Run \`agent-bar setup\` after installing, or set AGENT_BAR_ASSET_DIR.`
— em vez de vazar um path `/$bunfs/..`. O layout em `/usr/share/agent-bar`
espelha o repo (`icons/`, `scripts/`), então a lógica relativa de
`installWaybarAssets` funciona sem mudança — só o root difere. (Nota: o env
override é controlado pelo próprio usuário e o Waybar roda como ele — não há
fronteira de privilégio cruzada; exigir absoluto é higiene, não mitigação de
escalonamento.)

### 3. `appBin` correto no install de sistema (4ª mudança — pega do grill)

`getDefaultWaybarAssetPaths()` (`src/waybar-contract.ts:160`) hoje retorna
`appBin: \`$HOME/.local/bin/${APP_NAME}\``. Num install AUR o binário está em
`/usr/bin/agent-bar` (no PATH) e **não existe** `~/.local/bin/agent-bar` → o
`exec` do módulo gerado apontaria pra um path inexistente e o **Waybar quebraria
silenciosamente** (módulo não aparece, sem erro visível).

Fix: quando `isCompiledBinary()`, `appBin` = `'agent-bar'` (nome puro, resolvido
via PATH — funciona em `/usr/bin` ou onde o pacote pôr). Caso contrário mantém
`$HOME/.local/bin/agent-bar` (managed/npm criam esse symlink no setup). Sem o
fix, `setup` num AUR escreve config Waybar quebrada.

### 4. `update` ciente de install de sistema

`detectInstallKind` (`src/update.ts`) hoje retorna `'npm'` sem `.git` → um
binário AUR cairia no caminho `bun add -g` (errado: não há Bun nem npm).
Adicionar o kind `'system'`: quando `isCompiledBinary()`, `main()` imprime
orientação para o gerenciador de pacotes (ex.: `paru -Syu agent-bar-bin`) e
aborta — **sem** `bun add -g`, **sem** `git`.

### 5. Workflow de release (`.github/workflows/publish.yml`)

**Sequencial, no mesmo job, APÓS `publish:npm`** (não job paralelo — evita
release com binário publicado mas npm falho, sem rollback). Roda no mesmo
`ubuntu-latest` do job atual. Ordem: (1) verify version, (2) `release:check`,
(3) `publish:npm`, (4) compila binário, (5) monta `pkg/` (binário + `icons/` +
`scripts/agent-bar-open-terminal` + `LICENSE`), (6)
`tar czf agent-bar-${VERSION}-x86_64.tar.gz`, (7) `sha256sum` (logado), (8)
`gh release upload v${VERSION} …` (usa `GITHUB_TOKEN`, sem segredo novo).

**Failure mode documentado:** se (4)–(8) falham *após* o npm já ter publicado, o
release fica sem o binário — correção é rerodar `gh release upload` manualmente
(não retry automático).

**Guard de drift de versão** (no `release:check` ou no verify do CI):
```bash
grep -q "^pkgver=$(jq -r .version package.json)$" packaging/aur/PKGBUILD \
  || { echo "PKGBUILD pkgver != package.json version"; exit 1; }
```
Elimina a classe de bug "esqueci de bumpar o pkgver" (3 fontes de versão:
package.json embutida, PKGBUILD pkgver, tag git).

### 6. PKGBUILD (`packaging/aur/PKGBUILD`)

```bash
# Maintainer: Othavio <obsidianlab3d@gmail.com>
pkgname=agent-bar-bin
pkgver=5.2.0          # mantido em sincronia com package.json (guard no CI)
pkgrel=1
pkgdesc="LLM quota monitor for Waybar (Claude, Codex, Amp) — standalone binary"
arch=('x86_64')
url="https://github.com/othavioquiliao/agent-bar"
license=('MIT')
provides=('agent-bar')   # futureproof p/ um eventual pacote source agent-bar
conflicts=('agent-bar')
optdepends=('waybar: status bar integration'
            'libnotify: desktop low/critical quota notifications')
# NOTA: CLIs de provider (Claude/Codex/Amp) NÃO entram em optdepends —
# não há nomes de pacote AUR/pacman canônicos verificados; orientação vai no
# .install. Verificar nomes reais antes de submeter se quiser incluí-los.
install="${pkgname}.install"
source=("agent-bar-${pkgver}-x86_64.tar.gz::${url}/releases/download/v${pkgver}/agent-bar-${pkgver}-x86_64.tar.gz")
sha256sums=('<preenchido por release a partir do sha256sum do CI>')

package() {
  install -Dm755 "${srcdir}/agent-bar" "${pkgdir}/usr/bin/agent-bar"
  install -Dm755 "${srcdir}/scripts/agent-bar-open-terminal" \
    "${pkgdir}/usr/share/agent-bar/scripts/agent-bar-open-terminal"
  # ícones como dados (644), não herdar bits de exec do tarball (namcap)
  for icon in "${srcdir}"/icons/*; do
    install -Dm644 "$icon" "${pkgdir}/usr/share/agent-bar/icons/$(basename "$icon")"
  done
  install -Dm644 "${srcdir}/LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
```

### 7. `packaging/aur/agent-bar-bin.install`

agent-bar exige config pós-install (diferente de um app standalone) — sem isto o
Waybar fica sem módulos e o usuário não sabe por quê.
```bash
post_install() {
  echo "==> Run 'agent-bar setup' to integrate the modules into Waybar."
  echo "==> Provider CLIs (claude/codex/amp) are detected at runtime if installed."
}
post_upgrade() {
  echo "==> Updated. Restart Waybar if modules look stale."
}
```

`.SRCINFO` gerado de `makepkg --printsrcinfo` e versionado.

## Layout instalado

| Caminho | Conteúdo |
| --- | --- |
| `/usr/bin/agent-bar` | binário standalone (755) |
| `/usr/share/agent-bar/icons/` | ícones de provider (644) |
| `/usr/share/agent-bar/scripts/agent-bar-open-terminal` | helper de terminal (755) |
| `/usr/share/licenses/agent-bar-bin/LICENSE` | licença MIT |

Pós-install o usuário roda `agent-bar setup` → integra no Waybar lendo assets de
`/usr/share/agent-bar` (componente 2) e gravando `exec: agent-bar` (componente 3).

## Testes

- **`isCompiledBinary`**: true sob o marcador `/$bunfs` (mock/override de teste),
  false caso contrário. (`tests/runtime.test.ts`.)
- **`resolveAssetSourceRoot`**: env override absoluto vence; `/usr/share/agent-bar`
  quando existe; fallback ao repo só se `!isCompiledBinary()`; sob compilado sem
  assets → erro claro (não path `$bunfs`). (mock fs.)
- **`getDefaultWaybarAssetPaths().appBin`**: `agent-bar` sob compilado;
  `$HOME/.local/bin/agent-bar` caso contrário. (`tests/waybar-contract.test.ts`.)
- **`detectInstallKind` → `'system'`** sob compilado; managed/dev com `.git`;
  `'npm'` sem `.git` rodando via bun. **`update` system** não invoca `bun add -g`
  e imprime orientação. (`tests/update.test.ts`, mock do runner.)
- **Smoke do binário** (manual, no plano): `--version`/`status`/`--provider`/`setup`
  com Bun fora do PATH.

## Checklist por release

1. CI já valida `package.json` version == tag git; **guard novo**: `pkgver` do
   PKGBUILD == `package.json` version (componente 5).
2. Após o release publicar o tarball + sha256: atualizar `pkgver` e
   `sha256sums` em `packaging/aur/PKGBUILD`, **regenerar `.SRCINFO`**
   (`makepkg --printsrcinfo > .SRCINFO`), commitar no repo.
3. **Handoff (mantenedor):** copiar `PKGBUILD`/`.install`/`.SRCINFO` pro repo AUR
   (`ssh://aur@aur.archlinux.org/agent-bar-bin.git`) e `git push`. Requer conta
   AUR + chave SSH — **passo seu**. (Automação CI→AUR = follow-up.)

## Boundaries e risco

- **Supply-chain**: zero build no PKGBUILD — só download + sha256. Sem
  `bun`/`npm` em `makedepends`/`depends`.
- **Aditivo**: installs existentes (managed/npm) inalterados — `isCompiledBinary()`
  é false neles, então `resolveAssetSourceRoot`/`appBin`/`update` mantêm o
  comportamento atual.
- **glibc**: binário linkado à glibc do runner `ubuntu-latest`; roda em Arch
  (rolling, sempre mais novo) — direção segura, e por ser Arch-only o piso
  flutuante nunca morde. Musl/Alpine **não** são alvo (limitação documentada).
- **`uninstall`/`doctor` num AUR**: `uninstall` mira `~/.local/bin/agent-bar`
  (inexistente no AUR) e a integração Waybar — **não** toca `/usr/bin/agent-bar`
  (owned pelo pacman). `doctor` (limpa lixo de `bun add -g`) é inócuo num AUR
  limpo e correto pra quem migrou de global; sem mudança de código (YAGNI).
- **Tamanho ~91 MB**: real, aceitável p/ `-bin` (cold-start ~100-200ms,
  imperceptível num interval de 120s). É o trade-off de `bun --compile` vs
  binário nativo.
- **Burden por release**: bump manual de `pkgver`/`sha256`/`.SRCINFO` + push AUR.
  O guard de CI pega o esquecimento do `pkgver`; o resto é checklist até a
  automação CI→AUR (follow-up).
