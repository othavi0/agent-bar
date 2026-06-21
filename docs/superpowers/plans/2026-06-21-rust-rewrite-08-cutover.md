# Plano 8 — Cutover Rust Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tornar o crate Rust a fonte da verdade publicável: promover `rust/`→raiz, remover o mundo TS/npm/bun, retargetar a distribuição pro binário musl estático, reescrever docs.

**Architecture:** Trabalho 100% na branch `rust-rewrite`. Promoção destrutiva primeiro (tag de preservação + git rm do TS + git mv do crate), depois correções de path que a promoção expõe, depois retarget de CI/packaging/install, depois docs. **NADA vai ao ar** (merge/tag/Release/AUR push/npm-drop publicado) sem smoke da TUI + OK explícito do usuário (gate de processo, fora deste plano).

**Tech Stack:** Rust 1.95 / cargo, target `x86_64-unknown-linux-musl`, reqwest+rustls (sem openssl), GitHub Actions, makepkg/AUR, bash.

**Spec de referência:** `docs/superpowers/specs/2026-06-20-rust-cutover-design.md`.

## Global Constraints

- **Branch:** `rust-rewrite`. Não tocar `master`.
- **Sem `unwrap()`/`expect()` em produção** (enforçado por `deny(clippy::unwrap_used, clippy::expect_used)` em `lib.rs` e `main.rs`; permitido em teste).
- **Versão única = `6.0.0` via `CARGO_PKG_VERSION`.** `Cargo.toml` já está em 6.0.0.
- **`cargo fmt` ANTES de cada `git add`.** Commits: Conventional Commits PT, subject ≤50 chars.
- **Gotcha RTK:** o RTK reformata o output do cargo — NÃO existe a string `test result:`. Para contar testes, ler o output bruto (`... 2>&1 | tail -6`) e somar os `passed`. Para clippy limpo: `cargo clippy: No issues found`. `cargo test` aceita só UM filtro posicional.
- **Read antes de Edit** (o harness exige; `cat`/`sed` não contam). Se Edit falhar com `string not found`, re-Read antes de re-tentar — nunca editar de memória.
- **Identidade:** usar constantes de `src/app-identity.ts`→ no Rust `app_identity` (`APP_NAME` etc.), nunca hardcode.
- **Lar canônico:** `github.com/othavioquiliao/agent-bar`. Asset de release: `agent-bar-{version}-x86_64.tar.gz`.
- **stdout limpo** (Waybar parseia JSON); logs → stderr.
- **A partir do T1a, o crate vive na RAIZ.** Verificação passa a ser `cargo test` / `cargo clippy --all-targets -- -D warnings` SEM `--manifest-path rust/`. Antes do T1a, usar `--manifest-path rust/Cargo.toml`.

---

### Task 1a: Promoção `rust/` → raiz (destrutivo, history-preserving)

Tag de preservação, remoção do mundo TS, e `git mv` do crate pra raiz. Task ISOLADA (só operações git destrutivas + .gitignore; nenhum edit de código fonte — isso é T1b).

**Files:**
- Tag: `v5.3.0-ts-final` (no HEAD atual, que ainda contém o TS)
- Delete: `src/` (TS), `tests/` (TS), `package.json`, `bun.lock`, `bunfig.toml`, `tsconfig.json`, `biome.json`, `dist/`, `scripts/agent-bar` (shim bun), `scripts/bun-publish-with-npm-token`
- Move: `rust/Cargo.toml`→`Cargo.toml`, `rust/Cargo.lock`→`Cargo.lock`, `rust/build.rs`→`build.rs`, `rust/src`→`src`, `rust/tests`→`tests`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: nada.
- Produces: crate Rust na raiz. `cargo` roda da raiz. `src/setup.rs`, `src/main.rs`, `src/waybar_contract.rs` agora nos paths de raiz (T1b edita).

- [ ] **Step 1: Confirmar branch e árvore limpa**

Run: `cd /home/othavio/Projects/agent-bar && git branch --show-current && git status --short`
Expected: `rust-rewrite` e árvore sem mudanças pendentes não-relacionadas (só os specs/plano novos já commitados).

- [ ] **Step 2: Tag de preservação do TS (no HEAD atual, que tem o TS)**

```bash
git tag v5.3.0-ts-final
git tag --list 'v5.3.0*'
```
Expected: `v5.3.0-ts-final` listado. (O TS fica recuperável: `git checkout v5.3.0-ts-final -- src`.)

- [ ] **Step 3: Remover o mundo TS (git rm)**

```bash
git rm -r --quiet src tests dist
git rm --quiet package.json bun.lock bunfig.toml tsconfig.json biome.json
git rm --quiet scripts/agent-bar scripts/bun-publish-with-npm-token
```
Expected: sem erro. (Se algum path não existir, removê-lo do comando — não inventar.)

- [ ] **Step 4: Promover o crate Rust pra raiz (git mv)**

```bash
git mv rust/Cargo.toml Cargo.toml
git mv rust/Cargo.lock Cargo.lock
git mv rust/build.rs build.rs
git mv rust/src src
git mv rust/tests tests
git rm --quiet rust/.gitignore   # só `/target`; absorvido pelo .gitignore raiz no Step 5
rmdir rust 2>/dev/null || true
```
Expected: `rust/` vazio e removido. `ls src/main.rs Cargo.toml build.rs` resolve.

- [ ] **Step 5: Atualizar `.gitignore` raiz**

Read `.gitignore`, então remover as linhas `node_modules/` e `dist/`, e adicionar `/target` (absorve o `rust/.gitignore`). Resultado deve conter `/target` e NÃO conter `node_modules`/`dist`.

Run: `grep -E '/target|node_modules|dist' .gitignore`
Expected: só `/target`.

- [ ] **Step 6: Verificar que o crate builda da raiz**

Run: `cargo build 2>&1 | tail -3`
Expected: `Finished` sem erro (compila da raiz; `node_modules/` ainda existe no disco mas não é tracked — pode `rm -rf node_modules` depois, fora do git).

- [ ] **Step 7: Rodar a suíte e clippy da raiz**

Run: `cargo test 2>&1 | tail -6` e `cargo clippy --all-targets -- -D warnings 2>&1 | tail -3`
Expected: 531 `passed` somados, clippy `No issues found`. (Os testes passam mesmo com o bug de path do T1b porque os seams injetam paths.)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: promove crate Rust para a raiz"
```

---

### Task 1b: Correções de path pós-promoção

A promoção muda a semântica de `CARGO_MANIFEST_DIR` (era `rust/`; agora é a raiz) e deleta o shim `scripts/agent-bar`. Corrigir os 3 sítios de `repo_root` e o alvo do symlink dev. NÃO coberto por teste (seams injetam paths) → verificação por build + grep + dev-smoke manual.

**Files:**
- Modify: `src/setup.rs` (repo_root ~linha 37; create_symlink target ~linha 42)
- Modify: `src/main.rs` (repo_root ~linha 284)
- Modify: `src/waybar_contract.rs` (repo_root ~linha 457; docstring ~linha 430)

**Interfaces:**
- Consumes: crate na raiz (T1a).
- Produces: `repo_root` resolve para a raiz do repo em dev/checkout; symlink dev aponta pro binário compilado.

- [ ] **Step 1: Ler os 3 sítios**

Read `src/setup.rs` (linhas 28-50), `src/main.rs` (linhas 278-295), `src/waybar_contract.rs` (linhas 424-465). Confirmar o padrão `Path::new(env!("CARGO_MANIFEST_DIR")).parent()...`.

- [ ] **Step 2: Corrigir `repo_root` em `waybar_contract.rs`**

Em `resolve_asset_source_root` (~457): trocar
```rust
let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_else(|| PathBuf::from("."));
```
por
```rust
// Pós-cutover: o crate É a raiz do repo; CARGO_MANIFEST_DIR já aponta pra raiz.
let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
```
E atualizar a docstring (~430) de "diretório pai do `CARGO_MANIFEST_DIR`" para "o `CARGO_MANIFEST_DIR` (raiz do repo)".

- [ ] **Step 3: Corrigir `repo_root` em `main.rs`**

Em ~284: aplicar a mesma troca (remover `.parent()`, usar `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`) e atualizar o comentário ~283.

- [ ] **Step 4: Corrigir `repo_root` E o alvo do symlink em `setup.rs`**

Em `create_symlink` (~30-48): substituir o bloco `repo_root` + `target` por:
```rust
// Pós-cutover: o symlink dev aponta pro binário compilado que está rodando
// `setup` (antes apontava pro shim bun `scripts/agent-bar`, removido no cutover).
let target = std::env::current_exe()?;
```
Remover a derivação de `repo_root` deste fn (não é mais usada aqui) e atualizar a docstring (~28) de "→ `<repo_root>/scripts/agent-bar`" para "→ binário compilado (`current_exe`)".

- [ ] **Step 5: fmt + build + clippy**

Run: `cargo fmt && cargo build 2>&1 | tail -3 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -3`
Expected: compila, clippy `No issues found`.

- [ ] **Step 6: Suíte verde**

Run: `cargo test 2>&1 | tail -6`
Expected: 531 `passed` (nenhum teste deve quebrar — a mudança afeta defaults env-based, não os seams).

- [ ] **Step 7: Confirmar que nenhum `.parent()` de CARGO_MANIFEST_DIR sobrou**

Run: `grep -rn 'CARGO_MANIFEST_DIR' src/ | grep -i parent`
Expected: nada (zero matches).

- [ ] **Step 8: Dev-smoke manual (não-coberto por teste)**

Run: `cargo build && ./target/debug/agent-bar export waybar-modules 2>&1 | head -5`
Expected: JSON de módulos (o export usa `resolve_asset_source_root` no caminho dev → confirma que repo_root resolve sem panic e acha `icons/`/`scripts/`). Comparar mentalmente: o `app_bin`/`terminal_script` apontam pra paths sob a raiz, não um nível acima.

- [ ] **Step 9: Commit**

```bash
cargo fmt
git add src/setup.rs src/main.rs src/waybar_contract.rs
git commit -m "fix: resolve repo_root e symlink pos-promocao"
```

---

### Task 2: Perfil release + target musl + binstall metadata

Confirmar o perfil release (já presente), validar o build musl estático, e adicionar metadata do cargo-binstall.

**Files:**
- Modify: `Cargo.toml` (adicionar `[package.metadata.binstall]`)

**Interfaces:**
- Consumes: crate na raiz.
- Produces: binário musl estático buildável; metadata que o cargo-binstall usa pra puxar o asset do GitHub Release.

- [ ] **Step 1: Confirmar o perfil release**

Read `Cargo.toml` (`[profile.release]`). Confirmar `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, e AUSÊNCIA de `panic = "abort"`.
Expected: tudo conforme; nenhuma edição necessária no perfil.

- [ ] **Step 2: Instalar o target musl**

Run: `rustup target add x86_64-unknown-linux-musl 2>&1 | tail -2`
Expected: `installed` ou `up to date`.

- [ ] **Step 3: Build release musl (valida o risco `ring`)**

Run: `cargo build --release --target x86_64-unknown-linux-musl 2>&1 | tail -5`
Expected: `Finished`. **Se falhar no link do `ring`/openssl:** instalar `musl-tools` (`sudo pacman -S musl` ou equivalente) e re-tentar; se ainda falhar, usar `cargo zigbuild --release --target x86_64-unknown-linux-musl` (fallback documentado no spec). Registrar no ledger qual caminho funcionou.

- [ ] **Step 4: Confirmar binário estático**

Run: `ldd target/x86_64-unknown-linux-musl/release/agent-bar 2>&1; file target/x86_64-unknown-linux-musl/release/agent-bar`
Expected: `not a dynamic executable` (ldd) e `statically linked` (file).

- [ ] **Step 5: Smoke do binário musl**

Run: `./target/x86_64-unknown-linux-musl/release/agent-bar --version && ./target/x86_64-unknown-linux-musl/release/agent-bar --format json 2>&1 | head -c 200`
Expected: `6.0.0` e JSON válido (providers buscam dados).

- [ ] **Step 6: Adicionar `repository` ao `[package]`**

O template `{ repo }` do cargo-binstall lê o campo `repository` do `[package]` — que hoje NÃO existe no Cargo.toml. Adicionar (após `description`):
```toml
repository = "https://github.com/othavioquiliao/agent-bar"
```

- [ ] **Step 7: Adicionar metadata do cargo-binstall**

Adicionar ao `Cargo.toml` (no fim, ou após `[package]` antes de `[[bin]]`, seguindo o estilo do arquivo):
```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/agent-bar-{ version }-x86_64.tar.gz"
pkg-fmt = "tgz"
bin-dir = "agent-bar"
```
(O tarball tem o binário `agent-bar` na raiz do arquivo — ver T3 Step de empacotamento.)

- [ ] **Step 8: Validar o Cargo.toml**

Run: `cargo metadata --no-deps --format-version 1 >/dev/null 2>&1 && echo OK`
Expected: `OK` (TOML válido, metadata parseável).

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml
git commit -m "build: target musl + metadata cargo-binstall"
```

---

### Task 3: Retarget `publish.yml` (CI release)

Trocar o pipeline npm+bun-compile por build musl Rust, mantendo o empacotamento de tarball + sha256 + upload ao Release.

**Files:**
- Modify: `.github/workflows/publish.yml`

**Interfaces:**
- Consumes: Cargo.toml na raiz (version), o asset name padrão.
- Produces: workflow que builda o binário musl e anexa `agent-bar-{version}-x86_64.tar.gz` + `.sha256` ao Release. O PKGBUILD (T4) consome esse asset.

- [ ] **Step 1: Ler o workflow atual**

Read `.github/workflows/publish.yml` inteiro (entender os steps reusáveis de empacotamento/upload).

- [ ] **Step 2: Reescrever o workflow**

Substituir o conteúdo por (nome do job `publish`→`release`; trigger e permissions mantidos):
```yaml
name: Release binary

on:
  release:
    types: [published]

permissions:
  contents: write # gh release upload

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust + musl target
        run: |
          rustup toolchain install stable --profile minimal
          rustup target add x86_64-unknown-linux-musl
          sudo apt-get update && sudo apt-get install -y musl-tools

      - name: Verify version matches release tag
        env:
          RELEASE_TAG: ${{ github.event.release.tag_name }}
        run: |
          PKG_VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
          EXPECTED_TAG="v${PKG_VERSION}"
          if [ "$EXPECTED_TAG" != "$RELEASE_TAG" ]; then
            echo "::error::Release tag '$RELEASE_TAG' != Cargo.toml version (expected '$EXPECTED_TAG')"
            exit 1
          fi
          echo "Version $PKG_VERSION matches release tag $RELEASE_TAG"

      - name: Build static musl binary
        run: cargo build --release --target x86_64-unknown-linux-musl

      - name: Package tarball + sha256
        run: |
          VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
          mkdir -p pkg/scripts pkg/icons
          cp target/x86_64-unknown-linux-musl/release/agent-bar pkg/
          cp -r icons/. pkg/icons/
          cp scripts/agent-bar-open-terminal pkg/scripts/
          cp LICENSE pkg/
          tar czf "agent-bar-${VERSION}-x86_64.tar.gz" -C pkg .
          sha256sum "agent-bar-${VERSION}-x86_64.tar.gz" | tee "agent-bar-${VERSION}-x86_64.tar.gz.sha256"

      - name: Attach to release
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
          gh release upload "v${VERSION}" \
            "agent-bar-${VERSION}-x86_64.tar.gz" \
            "agent-bar-${VERSION}-x86_64.tar.gz.sha256" \
            --clobber
```
Nota: o tarball tem `agent-bar` na raiz do arquivo (não em `pkg/`), porque `tar -C pkg .` empacota o conteúdo de `pkg/`. Isso casa com o `bin-dir = "agent-bar"` do binstall (T2) e o `package()` do PKGBUILD (T4).

- [ ] **Step 3: Validar o YAML**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/publish.yml')); print('YAML OK')"`
Expected: `YAML OK`.

- [ ] **Step 4: Confirmar ausência de npm/bun**

Run: `grep -nE 'bun|npm|NPM_TOKEN|setup-bun' .github/workflows/publish.yml`
Expected: nada.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/publish.yml
git commit -m "ci: release builda binario musl Rust"
```

---

### Task 4: PKGBUILD + .SRCINFO + .install

Retargetar o pacote AUR `agent-bar-bin` pro binário Rust. AUR push fica gated (pós-Release real).

**Files:**
- Modify: `packaging/aur/PKGBUILD`
- Modify: `packaging/aur/.SRCINFO`
- Read/Modify: `packaging/aur/agent-bar-bin.install`

**Interfaces:**
- Consumes: o tarball do Release (T3).
- Produces: PKGBUILD que instala o binário Rust + assets.

- [ ] **Step 1: Ler os 3 arquivos**

Read `packaging/aur/PKGBUILD`, `packaging/aur/.SRCINFO`, `packaging/aur/agent-bar-bin.install`.

- [ ] **Step 2: Editar o PKGBUILD**

- `pkgver=6.0.0`
- Remover a linha `options=('!strip' '!debug')` E o comentário acima dela sobre a VFS do bun (o binário Rust strippa limpo e já vem stripado do perfil release).
- `sha256sums=('SKIP_PLACEHOLDER_PREENCHER_NO_RELEASE')` — colocar o literal `'0000000000000000000000000000000000000000000000000000000000000000'` como placeholder visível (NUNCA `SKIP`; preenchido a partir do `.sha256` do Release no momento do AUR push).
- Manter `source=(...)`, `provides`/`conflicts`/`optdepends`/`install` e o `package()` (o layout do tarball é idêntico: `agent-bar` + `scripts/agent-bar-open-terminal` + `icons/*` + `LICENSE`).

- [ ] **Step 3: Atualizar o comentário do sha256**

Garantir um comentário acima de `sha256sums` explicando: "Preenchido por release a partir do .sha256 produzido pelo CI. Nunca 'SKIP'."

- [ ] **Step 4: Regenerar o .SRCINFO**

Editar `.SRCINFO` manualmente pra casar com o PKGBUILD: `pkgver = 6.0.0`, `sha256sums = <mesmo placeholder>`, e remover qualquer linha `options` se existir. (Não rodar `makepkg --printsrcinfo` se makepkg não estiver disponível; editar à mão e conferir campo-a-campo vs o PKGBUILD.)

- [ ] **Step 5: Conferir consistência PKGBUILD ↔ .SRCINFO**

Run: `grep -E 'pkgver|sha256' packaging/aur/PKGBUILD packaging/aur/.SRCINFO`
Expected: `pkgver` 6.0.0 e mesmo sha256 placeholder em ambos.

- [ ] **Step 6: Revisar o .install**

Confirmar que `agent-bar-bin.install` só tem mensagem pós-install (sem referência a bun/npm). Editar se mencionar runtime obsoleto.

- [ ] **Step 7: Commit**

```bash
git add packaging/aur/
git commit -m "packaging: AUR aponta pro binario Rust 6.0.0"
```

---

### Task 5: `install.sh` rewrite

Reescrever o curl|bash (git clone + bun) pra baixar o tarball musl prebuilt.

**Files:**
- Rewrite: `install.sh`

**Interfaces:**
- Consumes: o asset do Release (T3); `agent-bar setup` (`AGENT_BAR_ASSET_DIR` seam).
- Produces: instalador zero-toolchain.

- [ ] **Step 1: Ler o install.sh atual**

Read `install.sh` inteiro (preservar a estrutura de flags/help/`die`/cores que serve bem).

- [ ] **Step 2: Reescrever a lógica de instalação**

Manter o cabeçalho de doc, `set -euo pipefail`, parsing de flags (`--force`/`--no-setup`/`--yes`), e substituir a lógica de clone+bun por:
1. Detectar arch: `uname -m` deve ser `x86_64`; senão `die "Only x86_64 prebuilt binaries are available."`.
2. Resolver a versão: `AGENT_BAR_VERSION` (default: resolver a tag `latest` via `https://api.github.com/repos/othavioquiliao/agent-bar/releases/latest`, campo `tag_name`).
3. Baixar `agent-bar-${VERSION#v}-x86_64.tar.gz` + `.sha256` de `https://github.com/othavioquiliao/agent-bar/releases/download/${VERSION}/` para um tempdir (`mktemp -d`).
4. Verificar o sha256 (`sha256sum -c`).
5. Extrair: binário → `~/.local/bin/agent-bar` (`install -Dm755`); `icons/` + `scripts/agent-bar-open-terminal` → `${AGENT_BAR_DATA:-$HOME/.local/share/agent-bar}/`.
6. Se `~/.local/bin` não estiver no `$PATH`, avisar (não falhar).
7. Salvo `--no-setup`: `AGENT_BAR_ASSET_DIR="$DATA_DIR" ~/.local/bin/agent-bar setup` (passa `--yes` se setado).
8. Envs: `AGENT_BAR_VERSION`, `AGENT_BAR_DATA`. Remover `AGENT_BAR_REPO`/`AGENT_BAR_BRANCH`/`AGENT_BAR_HOME` (não há mais clone) — ou manter `AGENT_BAR_REPO` só pra compor a URL de download (opcional).
9. Adicionar, no fim, uma nota: "Tem cargo? `cargo binstall agent-bar` também funciona."

- [ ] **Step 3: shellcheck**

Run: `shellcheck install.sh 2>&1 | tail -20 || echo "shellcheck ausente — pular"`
Expected: sem warnings acionáveis (ou shellcheck ausente).

- [ ] **Step 4: Dry-run da sintaxe (sem tocar o sistema)**

Run: `bash -n install.sh && echo "syntax OK"`
Expected: `syntax OK`. (NÃO executar o install.sh — mutaria o desktop. Validação é syntax + shellcheck + leitura.)

- [ ] **Step 5: Confirmar ausência de bun/clone**

Run: `grep -nE 'bun|git clone|bun install' install.sh`
Expected: nada.

- [ ] **Step 6: Commit**

```bash
git add install.sh
git commit -m "install: baixa binario musl prebuilt (sem bun)"
```

---

### Task 6a: Reescrever docs agent-facing (CLAUDE.md + AGENTS.md)

`CLAUDE.md` é a instrução de agente mais crítica. Reescrever pro Rust mantendo as hard rules vivas.

**Files:**
- Modify: `CLAUDE.md`
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: a realidade pós-cutover (cargo, raiz, comandos).
- Produces: instruções de agente corretas.

- [ ] **Step 1: Ler o CLAUDE.md e AGENTS.md atuais**

Read `CLAUDE.md` inteiro e `AGENTS.md`.

- [ ] **Step 2: Reescrever a §1 Hard Rules do CLAUDE.md**

- Trocar "Bun only. Sem Node, npm..." por "Rust/cargo only. Toolchain via rustup."
- Remover as regras sobre `bun ./scripts/agent-bar` e "nunca converter shims em scripts/ para TypeScript" (o shim bun foi deletado; `scripts/agent-bar-open-terminal` permanece como helper bash e segue sem virar Rust — manter essa regra reformulada).
- **Manter** (ainda vivas): não mutar desktop ao vivo; stdout limpo; legacy morto (qbar/antigravity/llm-usage); XML-escape só em `render-pango`→`render_pango.rs`; constantes de identidade; provider error strings = contrato.

- [ ] **Step 3: Reescrever a §2 Verification Matrix do CLAUDE.md**

Trocar cada `bun test tests/X.test.ts` pelo equivalente cargo. Exemplos:
| Área | Comando |
| --- | --- |
| Um provider | `cargo test providers::<provider>` |
| Formatters/tooltips | `cargo test formatters golden` |
| Contratos TypeScript→Rust | `cargo clippy --all-targets -- -D warnings` |
| Amplo antes de handoff | `cargo test && cargo clippy --all-targets -- -D warnings` |
Incluir o gotcha RTK (sem `test result:`; um filtro posicional só).

- [ ] **Step 4: Reescrever §§3-9 do CLAUDE.md**

- §3 Project-Specific: trocar referências a `src/app-identity.ts` por `src/app_identity.rs`; manter as regras de `ClaudeProvider` (implementa `Provider` direto), XML-escape, no-round-trip-JSONC, cache 5s.
- §4 Testing: `cargo test` (bun:test→bun:test removido); mock via seams; `insta` pra snapshots; `#[tokio::test]`.
- §6 Conventions: TypeScript strict→Rust strict + clippy `-D warnings`; biome→rustfmt (`cargo fmt`). Remover "ESM only".
- §7 Adicionar provider: apontar pro `docs/new-provider.md` (Rust); "Estenda `BaseProvider`" (Rust).
- §8 Release: o workflow `publish.yml` builda binário musl em `release: published`; versão via `Cargo.toml`. Remover NPM_TOKEN; mencionar AUR + install.sh.
- §9 Pointers: paths permanecem.
- Atualizar a linha de abertura ("Bun only"→Rust) e remover menção a `bun run start`.

- [ ] **Step 5: Atualizar AGENTS.md (shim Codex)**

Confirmar que aponta à verdade Rust (cargo, `src/` na raiz). Se for shim mínimo, ajustar a 1-2 frases relevantes.

- [ ] **Step 6: Grep de resíduo**

Run: `grep -niE '\bbun\b|npm|ts-node|bun:test|\.test\.ts|biome|ESM' CLAUDE.md AGENTS.md`
Expected: nada (exceto menções históricas intencionais, se houver — justificar cada uma).

- [ ] **Step 7: Commit**

```bash
git add CLAUDE.md AGENTS.md
git commit -m "docs: CLAUDE.md/AGENTS.md para o runtime Rust"
```

---

### Task 6b: Reescrever docs user-facing (README + CONTRIBUTING + docs/)

**Files:**
- Modify: `README.md`, `CONTRIBUTING.md`
- Modify: `docs/README.md`, `docs/architecture.md`, `docs/runtime.md`, `docs/commands.md`, `docs/integration.md`, `docs/waybar-contract.md`, `docs/new-provider.md`, `docs/troubleshooting.md`, `docs/json-output.md`

**Interfaces:**
- Consumes: realidade pós-cutover.
- Produces: docs de usuário/contribuidor corretos.

- [ ] **Step 1: Ler README.md e CONTRIBUTING.md**

Read ambos.

- [ ] **Step 2: Reescrever instalação no README**

Trocar `npm i -g @noctuacore/agent-bar` (ou similar) por: `install.sh` (curl|bash), AUR (`yay -S agent-bar-bin`), `cargo binstall agent-bar`. Atualizar quick-start (rodar `agent-bar`, não `bun run start`). Manter o banner/feature list.

- [ ] **Step 3: Reescrever CONTRIBUTING.md**

Build/test/lint via cargo (`cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`). Remover bun/biome/tsconfig. Estrutura do projeto: `src/` (Rust), `tests/`.

- [ ] **Step 4: Atualizar docs/*.md**

Para cada arquivo em `docs/`, Read e atualizar referências de runtime/comando/arquitetura do TS pro Rust:
- `architecture.md`: módulos Rust (`providers/`, `formatters/`, `tui/`, `usage/`) em vez de TS.
- `runtime.md`: cargo/binário em vez de bun.
- `commands.md`: o CLI é o mesmo contrato; conferir que os exemplos batem com `agent-bar help` (Rust).
- `new-provider.md`: estender `BaseProvider` (Rust); `ClaudeProvider` é a exceção.
- `integration.md`/`waybar-contract.md`/`json-output.md`/`troubleshooting.md`: trocar referências de bun/npm; o contrato Waybar/JSON é idêntico (byte-exact) — só o runtime muda.
- `docs/README.md`: índice.

- [ ] **Step 5: Grep de resíduo (exceto CHANGELOG histórico)**

Run: `grep -rniE '\bbun\b|@noctuacore|npm i|bun run|\.test\.ts|biome' README.md CONTRIBUTING.md docs/ | grep -v CHANGELOG`
Expected: nada acionável (justificar exceções).

- [ ] **Step 6: `git diff --check` (whitespace)**

Run: `git diff --check`
Expected: sem erros.

- [ ] **Step 7: Commit**

```bash
git add README.md CONTRIBUTING.md docs/
git commit -m "docs: README/CONTRIBUTING/docs para o Rust"
```

---

### Task 7: check de versão (substitui check:pkgver/release:check)

Substituir os scripts npm de release por uma checagem Cargo-based: `Cargo.toml` version == PKGBUILD pkgver == tag.

**Files:**
- Create: `scripts/check-version` (bash, executável)

**Interfaces:**
- Consumes: `Cargo.toml`, `packaging/aur/PKGBUILD`.
- Produces: checagem de consistência de versão (uso local + referenciável no CI/release).

- [ ] **Step 1: Escrever o script**

Create `scripts/check-version`:
```bash
#!/usr/bin/env bash
# Confere que Cargo.toml, PKGBUILD e (opcional) a tag passada batem.
set -euo pipefail

CARGO_VER="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
PKG_VER="$(grep -m1 '^pkgver=' packaging/aur/PKGBUILD | cut -d= -f2)"

fail=0
if [ "$CARGO_VER" != "$PKG_VER" ]; then
  echo "MISMATCH: Cargo.toml=$CARGO_VER PKGBUILD=$PKG_VER" >&2
  fail=1
fi

if [ "${1:-}" != "" ]; then
  TAG="${1#v}"
  if [ "$CARGO_VER" != "$TAG" ]; then
    echo "MISMATCH: Cargo.toml=$CARGO_VER tag=$TAG" >&2
    fail=1
  fi
fi

[ "$fail" -eq 0 ] && echo "version OK: $CARGO_VER"
exit "$fail"
```

- [ ] **Step 2: Tornar executável**

Run: `chmod +x scripts/check-version`

- [ ] **Step 3: Testar caminho feliz (versões batem)**

Primeiro garantir que PKGBUILD está em 6.0.0 (T4 fez). Run: `./scripts/check-version && ./scripts/check-version v6.0.0`
Expected: `version OK: 6.0.0` em ambos, exit 0.

- [ ] **Step 4: Testar caminho de falha (tag errada)**

Run: `./scripts/check-version v9.9.9; echo "exit=$?"`
Expected: `MISMATCH` no stderr e `exit=1`.

- [ ] **Step 5: shellcheck**

Run: `shellcheck scripts/check-version 2>&1 | tail -10 || echo "shellcheck ausente"`
Expected: sem warnings acionáveis.

- [ ] **Step 6: Commit**

```bash
git add scripts/check-version
git commit -m "build: check de versao Cargo/PKGBUILD/tag"
```

---

## Verificação final do plano (antes do handoff de release gate)

Após T1a-T7, rodar o gate amplo:

- [ ] `cargo test 2>&1 | tail -6` → 531 `passed` (ou mais, se T1b/T7 adicionarem testes).
- [ ] `cargo clippy --all-targets -- -D warnings 2>&1 | tail -3` → `No issues found`.
- [ ] `cargo build --release --target x86_64-unknown-linux-musl` → `Finished` + `ldd` estático.
- [ ] `git grep -niE '\bbun\b|@noctuacore|\.test\.ts' -- ':!CHANGELOG.md' ':!docs/superpowers/'` → nada acionável.
- [ ] `git status` limpo; `rust/` não existe; `src/Cargo.toml` na raiz.

**Release gate (PROCESSO, fora deste plano — exige OK explícito do usuário a cada passo):**
1. Smoke da TUI pelo usuário + fix da TUI (gated, fora deste plano — ver ledger).
2. Merge `rust-rewrite` → `master`.
3. `git tag v6.0.0` + GitHub Release (adicionar entrada no CHANGELOG.md aqui).
4. CI builda o binário + anexa ao Release.
5. AUR push (preencher `sha256sums` do PKGBUILD a partir do `.sha256` do Release).
