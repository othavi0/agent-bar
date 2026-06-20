# Plano 8 — Cutover Rust (dist + promoção + docs)

> **Status:** design aprovado (farol verde do usuário 2026-06-20). Spec de registro do
> ÚLTIMO plano da reescrita TS→Rust. Os Planos 1–7 estão completos e fechados
> (531 testes, clippy clean) — ver `docs/superpowers/rust-rewrite-resume.md` §1 e o
> ledger `.superpowers/sdd/progress.md`. Este plano remove o mundo TS/npm/bun e torna
> o crate Rust a fonte da verdade publicável.

## 1. Objetivo

Tornar o crate Rust (hoje em `rust/`) a fonte da verdade publicável do agent-bar:

1. Promover `rust/` para a raiz do repo (preservando history via `git mv`).
2. Remover o mundo TS/npm/bun (código, deps, lockfiles, scripts de publish bun).
3. Retargetar a distribuição do binário standalone (bun-compiled) para um **binário
   musl estático Rust**: CI, PKGBUILD/AUR, `install.sh`.
4. Reescrever a documentação (CLAUDE.md, README, CONTRIBUTING, `docs/*`) para o Rust.

**Restrição de segurança (inviolável):** todo o trabalho acontece na branch
`rust-rewrite`. **NADA vai ao ar** — merge para `master`, criação de tag/Release,
push para o AUR, drop do pacote npm publicado — até a TUI passar num smoke do usuário
**e** o usuário dar OK explícito. Ver §5 (Release gate).

## 2. Decisões travadas (do usuário, não relitigar)

- **Distribuição:** AUR + cargo-binstall + `install.sh`; **DROPAR npm**.
- **Target:** `x86_64-unknown-linux-musl` (binário estático, roda em qualquer distro).
- **Perfil release:** `opt-level=z`, `lto`, `codegen-units=1`, `strip` — **SEM**
  `panic=abort`, **SEM** mimalloc. (Já presente em `rust/Cargo.toml`.)
- **Fate do TS:** deletar limpo na branch + tag `v5.3.0-ts-final` antes da deleção
  (git preserva tudo, recuperável).
- **Ferramenta de dist:** hand-rolled — retargetar `publish.yml`/PKGBUILD/`install.sh`
  existentes (não cargo-dist; YAGNI para 1 target + AUR + curl|sh).

## 3. Decisões desta sessão (convention-clear)

- **crates.io:** NÃO publicar agora. cargo-binstall puxa o binário direto do GitHub
  Release via `[package.metadata.binstall]`; `cargo install` from-source = YAGNI.
- **Install dir do `install.sh`:** `~/.local/bin` (user install, sem sudo,
  zero-pollution — coerente com o ethos do install.sh atual).
- **Lar canônico:** `github.com/othavioquiliao/agent-bar` (onde Releases, `install.sh`
  e PKGBUILD já apontam). O escopo npm `@noctuacore/agent-bar` é abandonado.
- **Versão:** `6.0.0`, fonte única `CARGO_PKG_VERSION`. Resolve o drift atual
  (npm 5.3.0 / Cargo 6.0.0 / PKGBUILD 5.3.0).

## 4. Tasks

Cada task tem um gate de verificação próprio (mais estreito possível).

### T1 — Promoção `rust/` → raiz (mecânica git, history-preserving)

1. **Tag de preservação ANTES de deletar:** `git tag v5.3.0-ts-final <commit do TS>`
   (TS recuperável para sempre via git).
2. **Deletar o mundo TS:** `src/` (22 arquivos TS), `tests/` (TS), `package.json`,
   `bun.lock`, `bunfig.toml`, `tsconfig.json`, `biome.json`, `dist/`, `node_modules/`,
   `scripts/agent-bar` (shim bash que roda bun), `scripts/bun-publish-with-npm-token`.
3. **Promover o crate:** `git mv rust/Cargo.toml .`, `git mv rust/Cargo.lock .`,
   `git mv rust/build.rs .`, `git mv rust/src src`, `git mv rust/tests tests`. O
   `rust/.gitignore` (só `/target`) é absorvido pelo `.gitignore` raiz.
4. **Manter:** `scripts/agent-bar-open-terminal` (helper de terminal pro left-click
   da Waybar; é o `terminal_script` do `waybar_contract`), `icons/`, `LICENSE`,
   `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `docs/`, `AGENTS.md`, `packaging/`,
   `install.sh`, `.github/`.
5. **`.gitignore` raiz:** adicionar `/target`; remover `node_modules`/`dist`.

**⚠️ Correções de path pós-promoção (CONFIRMADAS por leitura do código — sem isto, o
dev/checkout quebra silenciosamente; testes NÃO pegam porque injetam paths via seam):**

1. **`repo_root = parent(CARGO_MANIFEST_DIR)`** em 3 sítios: `setup.rs:37`,
   `main.rs:284`, `waybar_contract.rs:457` (+ docstring `waybar_contract.rs:430`).
   Hoje o crate está em `rust/`, então o pai do manifest = raiz do repo. **Depois da
   promoção, `CARGO_MANIFEST_DIR` JÁ É a raiz** → `.parent()` sobe um nível DEMAIS
   (acima do repo). Fix: usar `Path::new(env!("CARGO_MANIFEST_DIR"))` direto (sem
   `.parent()`). Afeta só runs dev/checkout (instalação de sistema usa `/usr/share`).
2. **`create_symlink` (`setup.rs:42`)** aponta o symlink dev `~/.local/bin/agent-bar`
   para `<repo_root>/scripts/agent-bar` — o **shim bun que vamos deletar**. Fix:
   apontar para o binário Rust compilado. `std::env::current_exe()` (o próprio binário
   que está rodando `setup`) é o alvo mais correto para instalação dev.

**Gate:** `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
rodam a partir da raiz (sem `--manifest-path rust/`). Suíte verde (531 testes),
clippy clean. `git status` limpo (sem arquivos órfãos do `rust/`). **Dev-smoke manual**
(os 3 sítios de path acima não são cobertos por teste): rodar `agent-bar` do checkout
promovido + `agent-bar setup --help`/dry-run confirma que `repo_root` resolve para a
raiz correta.

### T2 — Perfil release + target musl

1. Confirmar `[profile.release]` (já presente, decisão travada). Sem alteração esperada.
2. Adicionar suporte ao target `x86_64-unknown-linux-musl`. reqwest já usa
   `default-features=false, features=["rustls-tls"]` (sem openssl/native-tls) → musl
   viável. **Risco técnico:** `ring` (dep do rustls) no musl. Fallback se musl-gcc
   embirrar: `cargo-zigbuild` (zig como linker, lida com musl robustamente).
3. `[package.metadata.binstall]` apontando para o asset do GitHub Release
   (`agent-bar-{version}-x86_64.tar.gz`), `bin-dir` para o layout do tarball.

**Gate:** build local `cargo build --release --target x86_64-unknown-linux-musl`
sucede; `ldd target/.../agent-bar` reporta "not a dynamic executable" (estático);
o binário roda (`--version`, `--format json`).

### T3 — Retarget `publish.yml` (CI release)

1. Trigger `release: published` mantido.
2. **Remover:** setup-bun, `bun install`, `bun run release:check`, `bun run publish:npm`,
   o secret NPM_TOKEN (não mais necessário).
3. **Adicionar:** instalar toolchain Rust + target musl (+ musl-tools ou zigbuild);
   `cargo build --release --target x86_64-unknown-linux-musl`.
4. **Reusar (adaptado):** empacotar tarball (`agent-bar` + `scripts/agent-bar-open-terminal`
   + `icons/` + `LICENSE`), gerar `.sha256`, `gh release upload`.
5. **Version check:** comparar a version do `Cargo.toml` com a tag do release
   (substitui o `jq` de `package.json`).

**Gate:** `act` ou dry-run mental do workflow; YAML válido; passos coerentes com o
tarball que o PKGBUILD espera (mesmo nome de asset, mesmo layout interno).

### T4 — PKGBUILD + .SRCINFO + .install

1. `pkgver=6.0.0`.
2. Remover `options=('!strip' '!debug')` (era para a VFS embutida do bun; o binário
   Rust strippa limpo — e já vem stripado do perfil release).
3. `sha256sums` = placeholder até o Release real produzir o `.sha256` (nunca `SKIP`).
4. Regenerar `.SRCINFO` (deve casar com o PKGBUILD).
5. Revisar `packaging/aur/agent-bar-bin.install` (mensagem pós-install — provedores
   CLI detectados em runtime).

**AUR push = gated** (depois do Release real existir + OK do usuário).

**Gate:** `namcap PKGBUILD` se disponível; `.SRCINFO` consistente; layout de `package()`
casa com o tarball do T3.

### T5 — `install.sh` rewrite

Reescrever o curl|bash atual (git clone + `bun install`) para:

1. Detectar arquitetura (x86_64; erro claro em outras por ora).
2. Baixar `agent-bar-{version}-x86_64.tar.gz` do GitHub Release de othavioquiliao/agent-bar.
3. Extrair o binário para `~/.local/bin/agent-bar` + assets (icons/terminal helper)
   para um dir de dados (ex: `~/.local/share/agent-bar`). O `agent-bar setup` resolve
   a origem dos assets via `resolve_asset_source_root` (3 vias: `AGENT_BAR_ASSET_DIR` /
   `/usr/share` / dev-checkout) — o `install.sh` exporta `AGENT_BAR_ASSET_DIR` apontando
   pro dir extraído antes de chamar `setup` (não é checkout nem system install).
4. Rodar `agent-bar setup` (salvo `--no-setup`).
5. Mencionar `cargo binstall agent-bar` como path alternativo (para quem tem cargo).
6. Manter flags `--force`/`--no-setup`/`--yes` e envs `AGENT_BAR_HOME`/`AGENT_BAR_REPO`/
   `AGENT_BAR_BRANCH` (adaptadas — `AGENT_BAR_VERSION` ou "latest" via API do GitHub).

**Gate:** `shellcheck install.sh` clean; dry-run da lógica de download/extract com um
tarball local fake (sem tocar o desktop do usuário).

### T6 — Reescrever docs

1. **`CLAUDE.md`:** trocar "Bun only" → toolchain Cargo/Rust; reescrever a Verification
   Matrix (§2) para `cargo test <area>` / `cargo clippy` por área; remover regras
   TS-específicas (ESM, biome, `bun:test`); **manter** as hard rules ainda vivas
   (stdout limpo, byte-exact Waybar/Pango, constantes de identidade, legacy morto,
   sem `unwrap!`/`expect!` em produção, XML-escape só em render_pango).
2. **`README.md` + `CONTRIBUTING.md`:** instalação via `install.sh`/AUR/binstall (não
   npm); runtime Rust; build via cargo; contributor workflow (cargo test/clippy/fmt).
3. **`docs/*.md`** (architecture, runtime, commands, integration, waybar-contract,
   new-provider, troubleshooting, json-output): atualizar para o Rust. `new-provider.md`
   → estender `BaseProvider` em Rust (Claude implementa `Provider` direto).
4. **`AGENTS.md`** (shim de compat Codex): atualizar conteúdo para apontar à verdade Rust.

**Gate:** `git diff --check` (whitespace); revisão de coerência (nenhuma menção a bun/npm
como caminho vigente; comandos citados existem no CLI Rust). Não há teste automatizado
de docs — revisão manual.

### T7 — `check:pkgver` / release helpers

Substituir os scripts npm (`check:pkgver`, `release:check`, `prepack`, `publish:*`,
`lint`/`lint:fix` do package.json — que some no T1) por uma checagem Cargo-based:
version do `Cargo.toml` == `pkgver` do PKGBUILD == tag do release. Pode ser um pequeno
shell em `scripts/` ou um teste de integração no crate.

**Gate:** rodar a checagem com versões casando (pass) e desencontradas (fail).

## 5. Release gate (processo, não código)

Ordem inviolável após o cutover na branch:

1. Branch cutover completo (T1–T7), suíte verde, clippy clean.
2. **Smoke da TUI pelo usuário** (comando fornecido). Bug da TUI (provável wiring do
   event loop — investigação em curso) corrigido e re-smokado.
3. Merge `rust-rewrite` → `master` (com OK do usuário).
4. `git tag v6.0.0` + GitHub Release (com OK do usuário).
5. CI builda o binário musl + anexa ao Release.
6. AUR push (com OK do usuário; atualizar `sha256sums` do PKGBUILD a partir do `.sha256`
   do Release).

Cada passo 3–6 exige OK explícito do usuário. Nenhum é automático.

## 6. Riscos

- **musl + ring:** maior risco técnico. Mitigação: `cargo-zigbuild`. Endereçado no T2
  (gate de build local antes do CI).
- **TUI quebrada:** o cutover NÃO depende da TUI para a funcionalidade Waybar/CLI (que
  está 100%). Mas a TUI é a feature-headline; o release está gated nela (§5). Bug sendo
  investigado em paralelo.
- **Drift de docs:** docs grandes; risco de menção residual a bun/npm. Mitigação: grep
  por `bun`/`npm`/`@noctuacore`/`bun:test` após T6 (exceto menções históricas no
  CHANGELOG, que ficam).
- **Paths sensíveis à promoção (CONFIRMADO):** `repo_root = parent(CARGO_MANIFEST_DIR)`
  em 3 sítios + symlink dev → shim bun deletado. Endereçado explicitamente no T1.
  Risco residual: não há teste cobrindo (seams injetam paths) → exige dev-smoke manual.

## 7. Fora de escopo

- Suporte multi-arch (aarch64, Mac) — futuro; hoje só x86_64 musl.
- Publicação em crates.io.
- Reescrita da TUI (bug fix é pré-release gate, não parte do cutover).
- Mudanças funcionais no comportamento do CLI/Waybar (paridade já travada nos Planos 1–7).
