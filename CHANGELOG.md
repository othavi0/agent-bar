# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [8.2.2] - 2026-07-19

### Changed
- **Standalone `agent-bar update`** re-copia icons e o terminal helper para os
  paths da Waybar (`~/.config/waybar/agent-bar/icons` e `…/scripts`) após
  baixar o release. Não re-patcha config/modules/CSS (use `setup` se a
  integração mudou).

## [8.2.1] - 2026-07-19

### Fixed
- **Ícone Waybar do Grok** — substituído o placeholder “G” pelo logomark
  oficial `Grok_Logomark_Light` do pack de brand xAI
  (`SpaceXAI_Grok_Assets.zip`). Sem mudança de CSS/contrato de arquivo
  (`grok-icon.svg`).

## [8.2.0] - 2026-07-17

Provider **Grok** (Grok Build CLI) na barra e na TUI.

### Added
- **Provider Grok (Grok Build CLI)** — OAuth em `~/.grok/auth.json` e
  `signals.json` das sessões; o % da barra é **contexto restante da sessão
  recente** (não cota de plano xAI). Login via `grok login` na TUI.
  Módulo Waybar, ícone, builder e fixtures de regressão.
  **Novos installs** listam `grok` por default; **settings existentes**
  precisam habilitar em Config (não há auto-inserção).

## [8.1.0] - 2026-07-17

Fundações de código, hardening de legibilidade e polish da TUI
(trilhas A/B/C pós-8.0.0).

### Added
- Fixtures de regressão do Amp (`tests/fixtures/amp/`) para formatos
  legado `$X/$Y` e free-tier em `% remaining`.
- Rótulo dual de tokens nos totais do Detail: principal = input+output,
  sufixo `(+N cache)` quando há cache.
- Legenda do chart com indicador `…+N` quando séries não cabem na largura.
- Discoverability de ajuda: chip `? ajuda` no Detail e hint na borda
  inferior da moldura.
- Specs: fundações/confiança, hardening de produto, polish TUI.

### Changed
- **Codex / TUI update / Detail** modularizados (sem mudança de contrato de
  produto nos splits).
- Config da TUI com rótulos humanos (`Provedores`, `Ordem`, `Exibição`,
  `Câmbio R$`…); chave técnica permanece na dica do campo.
- Sidebar colapsada usa `≡` / `→` / `⚙` em vez de H/L/C.
- Chart no Detail usa `Min(6)` em painel estreito (&lt; 72 cols); `Min(9)` no resto.
- Tokens de cor com contraste ≥4.5:1 sobre o fundo (`Comment`, `Red`,
  séries do chart).
- Pricing revalidado contra a tabela oficial Anthropic (2026-07-17).

### Fixed
- **Amp Free em `% remaining`** (CLI atual): parseava só o formato `$X/$Y`
  e deixava a janela primary vazia no free tier percentual.
- Serialização Waybar em falha de serde: nunca mais stdout vazio; payload
  degradado com `class: agent-bar disconnected`.
- Docs de arquitetura sem residual TypeScript (`main.rs` / `notify.rs`).

## [8.0.0] - 2026-07-11

Redesign completo da TUI (v8) + números confiáveis no usage.

> **Os números históricos exibidos MUDAM neste update**: o dedup de
> streaming corrige a contagem de tokens do Claude (a mesma request era
> somada N vezes — queda esperada de ~1/3 nos totais) e o pricing novo
> corrige o custo. É correção, não perda de dado.

### Added
- **Chart de colunas por modelo** no Detail e no Histórico (escala √,
  série mínima sempre visível, cores One Dark Turbo validadas CVD).
- **Histórico com dias expandíveis e sessões**: cada dia abre a lista de
  sessões (hora, projeto, modelo, tokens, custo), derivadas dos session
  logs.
- **Cache persistente de parse** (`usage.redb`): warm-start do histórico
  cai de ~8s para ~150ms; versão de cache invalida tudo quando a
  semântica do parse muda; degradação segura em corrupção/lock.
- **Right-click no módulo Waybar abre a TUI** focada no provider, com
  cache invalidado antes de rotear.
- Config da TUI com seções waybar/tui e hint do signal de reload.
- Preços 2026-07: Fable/Mythos, Sonnet 5 com virada automática do preço
  introdutório em 2026-09-01, Opus legado (≤4.1) separado do 4.5+, tiers
  de cache 5m/1h, fast mode (Opus 4.7/4.8) e `inference_geo`.
- Nomes de modelo humanizados nas telas ("Fable 5", "Opus 4.8"…).

### Changed
- **Overview removida — a TUI boota direto no provider** (navegação pela
  sidebar; sem abas).
- **`waybar.interval` das settings agora chega ao config do Waybar**;
  default efetivo 60s — quem nunca configurou o valor verá o intervalo
  mudar de 120s para 60s.
- Paleta One Dark Turbo em toda a TUI; gauge sólido com precisão de
  oitavos.
- README reescrito em pt-BR; URLs migradas para `othavi0/agent-bar`.

### Fixed
- **Dedup de streaming no parser do Claude**: 1 record por request
  (a última entrada vence) — antes cada entrada parcial do streaming
  contava de novo.
- **Codex logado com token expirado**: resposta de erro JSON-RPC do
  app-server agora falha rápido em vez de girar até o timeout de 4s.
- Semana downsampled do chart cobre os 7 dias; empty-state do chart
  centralizado; colapso limpo em terminal baixo.
- Flake raro de testes que tocam PATH (serializados via lock comum).

## [7.1.0] - 2026-07-02

Leva de ajustes visuais da TUI pós-teste real do redesign.

### Added
- **Painel "Hoje (24h)" no Overview**: quando sobra altura abaixo dos cards,
  o espaço vira o chart braille das últimas 24h (mesma visualização do
  Histórico) com totais de hoje/7d no rodapé. Em terminal baixo o layout
  antigo permanece intacto.

### Changed
- **Animação de transição de tela (coalesce) removida** — reprovada em uso
  real; a navegação agora troca de tela instantaneamente. Sweep no fetch,
  pulse crítico e count-up de custo permanecem.
- Popup de ajuda (`?`) se dimensiona pelo conteúdo (antes era 60%x70% do
  frame e cortava as seções finais em terminais menores) e escurece a tela
  por baixo enquanto aberto.
- Copy da TUI revisada: acentos corrigidos ("instruções", "vírgula",
  "inválido", "configuração"…) e "Waybar Config" → "Config do Waybar".

### Fixed
- **"hoje 0 tok" / "sem uso de tokens" durante o carregamento**: enquanto o
  parse dos session logs não terminou, Histórico, Detail e cards agora dizem
  "coletando…" em vez de afirmar zero sobre dado que só não chegou.
- Parse dos session logs roda **uma vez** por refresh (rodava duas — uma pra
  janela de hoje, outra pra de 7 dias), cortando o tempo de carregamento do
  histórico pela metade.
- Rodapé do painel novo usa o mesmo vocabulário de tokens da tabela do
  Histórico (input+output), evitando totais contraditórios entre telas.

## [7.0.1] - 2026-07-02

Hotfix de distribuição: `agent-bar update` e `install.sh`.

### Fixed
- **`agent-bar update` estava quebrado em todo install standalone** desde a
  6.0.0: a detecção usava um path de compile-time (`CARGO_MANIFEST_DIR`, o
  diretório do runner do CI) e caía num modo npm legado tentando ler
  `package.json` inexistente. A detecção agora parte do binário real
  (`current_exe`): checkout de dev → fluxo git; instalação de sistema/AUR →
  orienta o gerenciador de pacotes; standalone → **self-update real** (baixa
  a última release, verifica sha256 obrigatoriamente, substitui o binário de
  forma atômica e espelha os assets).
- **`agent-bar setup` avulso falhava em install standalone** — a resolução
  de assets ganhou o candidato `~/.local/share/agent-bar` (respeitando
  `AGENT_BAR_DATA`/`XDG_DATA_HOME`), unificada entre update e setup.
- **`install.sh` agora migra instalações antigas sozinho**: upgrade
  automático quando a versão difere (sem exigir `--force`), detecção sã de
  binários da era TypeScript (que respondiam `--version` com o JSON do
  módulo), remoção best-effort do pacote npm legado
  (`@noctuacore/agent-bar`) e de symlinks antigos. `--force` fica só para
  reinstalar a mesma versão.

### Changed
- Caminho de update npm/bun removido por completo (legado morto).
- Diretório temporário do self-update via `tempfile` (0700, criação
  atômica) em vez de path manual em `/tmp`.

## [7.0.0] - 2026-07-02

Redesign completo da TUI do `agent-bar menu` (spec e plano em
`docs/superpowers/`). O contrato Waybar (módulos, JSON, CSS, tooltips) está
**intacto** — a mudança é toda no menu interativo.

### Added
- **Navegação por sidebar única** (Geral / providers / Histórico / Login /
  Waybar) com drill-down por provider — as 4 abas antigas foram removidas.
- **Suporte completo a mouse**: click seleciona/ativa (sidebar, cards, chips),
  hover destaca, wheel rola (cards e tabela do histórico).
- **Cobertura completa do `/usage` do Claude**: provider migrado pros blocos
  novos `limits[]` + `spend` da API OAuth — sessão, semana, limite semanal
  por-modelo (nome vindo da API) e extra usage/créditos, com severidade
  oficial da API e fallback transparente pros campos legados.
- **Tela Overview** com um card denso por provider: gauges com gradiente por
  célula, sparkline real de tokens/h (24h), custo do dia e estado de login
  confiável; estados desenhados pra deslogado/carregando/vazio.
- **Tela Histórico** com chart braille de área (24h/7d via tecla `t`) e
  tabela dia × provider × tokens × custo com scroll; linha do Amp com saldo
  real e nota de ausência de logs locais.
- **Bucketing horário** do histórico (`usage::buckets`) — gráficos com dado
  real por hora em vez de 7 pontos diários esticados.
- **Motion** gated por `menu.animations`: sweep no fetch iniciado pelo
  usuário, coalesce na troca de tela, pulse/blink em quota crítica (<10%) e
  count-up no custo (tachyonfx).
- **Ícones Nerd Font** com fallback Unicode via `glyphMode`.
- **Fonte configurável do menu**: `menu.fontFamily` (default "IBM Plex Mono")
  e `menu.fontSize`, aplicadas pelo helper via flags do terminal
  (alacritty/kitty/foot/ghostty); comando interno `agent-bar menu-font`.
- Settings novas: `menu.animations`, `menu.fontFamily`, `menu.fontSize`.

### Changed
- **Fetch, login e save saíram do event loop**: a TUI nunca mais congela —
  spinner e progresso por provider aparecem de verdade, teclas respondem
  durante o fetch, e o refetch pós-login é automático.
- **Estado de login derivado do fetch real** (5 estados, incluindo `erro`
  distinto de `deslogado`) — fim do `[ok]` baseado em existência de arquivo.
- **Cascata do helper de terminal** (`agent-bar-open-terminal`): honra
  `$TERMINAL` (lança o terminal preferido com flags de fonte quando
  suportado; desconhecido → caminho xdg); alacritty direto vem antes do
  caminho uwsm/xdg pra aplicar fonte preservando o float do Hyprland.
- **MSRV corrigida para 1.88** (piso real das dependências); dependências
  novas: `tachyonfx`, `tui-scrollview`; `tui-popup` removida.
- "pico HHh" e labels de eixo do histórico em hora local (antes UTC).

### Fixed
- **Tecla `r` (refresh) nunca funcionou** — setava "Loading" sem disparar
  fetch; agora dispara refetch real (com guard contra fetch duplicado).
- **Comando de login do Codex era inválido** (`codex auth login` não existe
  na CLI; corrigido para `codex login`).
- **Tela corrompida ao voltar do login** — repaint completo ressincroniza o
  buffer do terminal.
- **Help overlay corrompia a tabela por baixo** (texto vazando "pr"/"sto") —
  área limpa com `Clear` antes do popup.
- Sparkline "tokens/h" do detalhe era um placeholder hardcoded idêntico para
  todos os providers — substituído por dado real por provider.
- Nomes truncados com corte seco ("Free Tie") — truncagem com `…`.
- Race de ondas de fetch sobrepostas corrompendo spinner/`last_update`.
- `abbrev` de tokens estourava a fronteira de unidade ("1000.0K" → "1.0M").
- Extra usage habilitado sem limite configurado renderizava gauge
  autocontraditório ("$X de $0.00") — agora "usado · sem limite".

## [6.0.1] - 2026-06-21

### Fixed
- **`install.sh` corrompia o binário no `setup`.** `create_symlink` criava um symlink
  `~/.local/bin/agent-bar` apontando pra si mesmo (dangling) quando o binário já
  estava nesse caminho — caso do `install.sh` —, destruindo o executável. Agora
  detecta (via `canonicalize`) que o binário já está no destino e pula o symlink.
  `cargo install` / `cargo binstall` (instalam em `~/.cargo/bin`) não eram afetados.

## [6.0.0] - 2026-06-21

Reescrita completa de TypeScript/Bun para **Rust** (binário único), preservando
paridade byte-exact do contrato Waybar/Pango e da saída `--format json`.

### Changed
- **Runtime Rust.** O monitor agora é um binário Rust único (tokio + reqwest/rustls)
  no lugar do runtime TypeScript/Bun. Comportamento de Waybar/CLI inalterado —
  paridade byte-exact travada por golden snapshots vs a saída do TS.
- **TUI full-screen** reescrita em ratatui (abas Dashboard / Waybar / History /
  Login) com engine de custo via session logs locais (US$/R$). O event loop faz o
  parse de logs em background, mantendo a UI responsiva desde o boot.
- **Distribuição via binário musl estático.** `install.sh` baixa o tarball prebuilt
  do GitHub Release (verificado por sha256); o AUR (`agent-bar-bin`) e
  `cargo binstall` também instalam o binário. Build de release via `cargo-zigbuild`.

### Removed
- **Bun / Node / npm no runtime.** Sem dependência de runtime JS. O pacote npm
  `@noctuacore/agent-bar` foi descontinuado — a última versão TypeScript está
  preservada na tag `v5.3.0-ts-final`.

### Migração
- Instalações npm antigas: `agent-bar doctor` detecta e limpa os resíduos;
  reinstale via `install.sh`, AUR ou `cargo binstall`.

## [5.3.0] - 2026-06-18

### Added
- **Pacote AUR `agent-bar-bin`** (Arch). Instala um binário standalone
  (`bun build --compile`) baixado do GitHub Release e verificado por sha256 — sem
  exigir Bun no runtime do usuário e **sem build no PKGBUILD** (mitigação do vetor
  de supply-chain "Atomic Arch"). Uso: `paru -S agent-bar-bin && agent-bar setup`.
  O workflow de release agora compila e anexa o tarball
  `agent-bar-<ver>-x86_64.tar.gz` (+ `.sha256`) ao GitHub Release. PKGBUILD,
  `.install` e `.SRCINFO` versionados em `packaging/aur/`.

### Changed
- **Install de sistema reconhecido em todo o app.** Um binário compilado é
  detectado por `isCompiledBinary()` (marcador `/$bunfs` do `bun --compile`):
  `agent-bar setup` lê os assets de `/usr/share/agent-bar`, gera o módulo Waybar
  com `exec: agent-bar` (resolvido via PATH) e **pula** o symlink `~/.local/bin`;
  `agent-bar update` orienta o gerenciador de pacotes (ex.: `paru -Syu`) em vez de
  tentar `bun add -g`. Os installs existentes (managed/npm/dev) ficam inalterados.

## [5.2.0] - 2026-06-18

### Added
- **Saída Waybar single-provider expõe `percentage` e `alt`** para desbloquear
  `format-icons`. `alt` carrega o health state (`ok` / `low` / `warn` /
  `critical`, ou `disconnected`) para `format-icons` keyed por estado;
  `percentage` é o valor displayMode-aware (o mesmo número do `text`), clampado
  a `0..100`, para `{percentage}` no `format` ou `format-icons` em array. Ambos
  são **omitidos** quando o provider está conectado mas sem dados de quota — um
  window ausente nunca reporta `ok`. O módulo agregado e o contrato
  `--format json` permanecem inalterados.
- **Refresh sob demanda via `signal`** (opt-in). O novo setting `waybar.signal`
  (`1..30`, default off) injeta `signal: N` em cada módulo gerado; o Waybar
  re-executa o módulo ao receber `SIGRTMIN+N`. Como o `exec` do módulo lê o cache
  de 5 min, um signal puro só re-renderiza dados cacheados — para forçar um fetch
  **fresco** use o recipe documentado:
  `agent-bar -p <provider> -r && pkill -RTMIN+<N> waybar` (com exemplo de Stop
  hook do Claude Code em `docs/waybar-contract.md`).

### Docs
- `docs/waybar-contract.md`: campos de saída `percentage`/`alt`, exemplos de
  `format-icons` (por estado via `alt` e por percentual via array), e a
  configuração + recipe de refresh do `signal`.

## [5.1.0] - 2026-06-17

### Added
- **Claude: tier do plano no tooltip** (`Max 5x` / `Max 20x`). O cabeçalho do
  tooltip do Claude agora lê `rateLimitTier` do `~/.claude/.credentials.json`
  (ex.: `default_claude_max_5x`) e surfa o multiplicador que o `subscriptionType`
  ("max") descartava — antes mostrava só "Max". Planos sem multiplicador (Pro,
  Free) ficam inalterados. Lógica isolada em `deriveClaudePlan()`.
- **`docs/architecture.md`**: data-flow completo (poll do Waybar → provider →
  cache → formatter → saída JSON/Pango), a distinção entre os dois caches
  (quota 5 min cross-process em `cache.ts` vs settings 5 s in-process em
  `formatters/waybar.ts`), e `BaseProvider` vs `ClaudeProvider` direto. Passa a
  ser publicado no pacote npm.
- Documentação do comando interno `action-right` (right-click do Waybar) em
  `docs/commands.md` e `docs/waybar-contract.md`, com a lógica de
  refresh-ou-login e os campos do módulo gerado.

### Changed
- **Claude: short-circuit de token expirado.** Quando o `expiresAt` (epoch-ms
  das credenciais) já passou, o provider devolve o erro de token expirado **sem**
  chamar a API da Anthropic — a chamada falharia de qualquer forma e o agent-bar
  nunca renova o token (o refresh single-use corre com o Claude Code). Mesma
  string de erro e mesmo roteamento de login do `action-right`; apenas mais
  rápido e funcional offline.

## [5.0.0] - 2026-06-17

### Removed
- **Provider Copilot removido por inteiro** (breaking). A interface
  `--headless --stdio` do Copilot CLI é oculta/frágil (some sem aviso em
  auto-updates) e não estava em uso. Saíram: provider, CLI locator, builder,
  ícone, tipos `CopilotQuota*`, paths de config, registries (tooltip/terminal/
  TUI), entrada em `WAYBAR_PROVIDERS`, e a migração de settings v1→v2 que só
  existia para injetar Copilot. Providers suportados agora: Claude, Codex, Amp.

### Added
- **Notificações desktop de quota baixa/crítica** via `notify-send`: alerta
  quando qualquer janela de quota cruza 90% usado (low) ou 95% (critical),
  incluindo as semanais por-modelo do Claude. Piggyback no poll do Waybar com
  dedup por estado (`~/.cache/agent-bar/notify-<provider>.json`), re-arma ao
  recuperar, escala low→critical. Best-effort: não faz nada se `notify-send`
  estiver ausente e só dispara quando a saída é consumida pelo Waybar.
  Controlado por `notify.enabled` no settings.

### Breaking
- O provider `copilot` não existe mais. Settings que listavam `copilot` têm a
  entrada removida automaticamente na carga; nenhuma ação necessária.
- **Notificações vêm ligadas por padrão** (`notify.enabled: true`). Após
  atualizar, alertas de quota baixa passam a aparecer sem opt-in — desligue com
  `"notify": { "enabled": false }` em `~/.config/agent-bar/settings.json`.

## [4.2.0] - 2026-06-17

### Added
- `--format json`: versioned, Pango-free JSON contract that mirrors the internal
  quota model, for non-Waybar bars (Quickshell, Eww, Ironbar). Emits all
  registered providers (`--provider <id>` for a single one); independent of the
  `waybar.providers` setting. Schema, stability policy, and a Quickshell QML
  example in [`docs/json-output.md`](docs/json-output.md).
- `--watch [--interval <seconds>]`: long-running NDJSON stream (one envelope per
  line), backpressure-aware scheduling, EPIPE-safe, fails fast on unknown
  provider.
- `--version` / `-V` flag.
- `engines.bun` in `package.json`.

### Changed
- Copilot and Amp providers now follow the `BaseProvider` `fetchRaw`/`buildQuota`
  contract — the cache stores raw provider data instead of a pre-built quota.
- Copilot "used" percentage is computed at the provider layer
  (`QuotaWindow.used`); the Waybar renderer reuses `render-pango`'s span/escape
  boundary instead of a divergent local copy.

### Fixed
- Claude: send `User-Agent: claude-code/<version>` to avoid the aggressive
  rate-limit bucket (persistent 429s) on the OAuth usage endpoint; keep the
  request abort timer armed through the response-body read.
- Waybar config patcher: bracket-aware array matching that respects strings and
  JSONC comments. The previous non-greedy regex could corrupt `config.jsonc`
  when `modules-right`/`include` contained nested brackets, and could rewrite
  commented-out lines. `removeWaybarIntegration` now backs up before mutating.
- Amp: the `amp usage` subprocess now has a timeout and is killed on hang (no
  more zombie processes per Waybar poll); auth failures are no longer cached.
- Cache writes are atomic (temp file + rename).
- CLI: explicit error on `assets`/`export` without a valid subcommand.

### Removed
- The CI-only `bun-publish-with-npm-token` helper is no longer shipped in the
  npm `files` allowlist.

## [4.1.0] - 2026-05-23

### Added
- `agent-bar doctor` command: detects and cleans `@noctuacore/agent-bar`
  leftovers (`package.json`, lockfiles, `node_modules/@noctuacore/`) in `$HOME`
  caused by `bun add` / `npm i` without `-g`.
- `setup` now warns when `$HOME` has leftover install artifacts and points to
  `agent-bar doctor`.
- Bin shim (`scripts/agent-bar`) now detects install pollution in `$HOME` on
  every invocation and prints a warning suggesting `agent-bar doctor`. Warns at
  most once per hour per UID (cached in `$XDG_RUNTIME_DIR`) so Waybar logs stay
  clean.
- `install.sh` hosted installer: zero-pollution install path via
  `curl -fsSL .../install.sh | bash`. Clones to `~/.agent-bar`, installs deps,
  and optionally runs `agent-bar setup`. Adopts the curl|bash pattern used by
  bun, deno, rustup, uv, and other serious CLI tools.

### Changed
- README now promotes the hosted install script as the primary install path.
  `bun add -g` remains documented as an alternative with explicit warning about
  the `-g` flag.
- Documentation refresh: `CONTRIBUTING.md` rewritten in English and trimmed,
  with a new "Dev install" section explaining how to wire a local checkout
  straight into Waybar. `docs/runtime.md`, `docs/integration.md`,
  `docs/commands.md`, and `docs/troubleshooting.md` updated to drop the
  outdated "legacy" label on `~/.agent-bar`, reflect `install.sh` as the
  primary install path, and document `$HOME` pollution handling.

### Removed
- `preinstall` script from `package.json` — Bun does not execute lifecycle
  scripts of dependencies by default, so the guard was silent theater. Replaced
  by a Bash-level detector in the bin shim that runs on every invocation
  regardless of package manager.

## [4.0.2] - 2026-05-19

### Changed

- `agent-bar update` agora detecta instalações npm/Bun e atualiza o pacote
  global com `bun add -g`, em vez de tratar apenas o checkout legado
  `~/.agent-bar`.

### Fixed

- Logo da TUI exibia `QBAR` (nome antigo do projeto) ao abrir o menu. Substituído pela block-art `AGENT BAR`.

## [4.0.0] - 2026-05-15

### Added

- Setting `waybar.displayMode` (`remaining` | `used`) com toggle via TUI Configure Layout. Quando `used`, percentuais e barra refletem quota consumida (0% = nada usado, 100% = esgotado); cores e classes CSS continuam baseadas em saúde. Default: `remaining` (comportamento anterior preservado).

### Changed

- Renamed the project to `agent-bar` (previously `qbar`, then `agent-bar-omarchy`). Runtime state now lives under `~/.config/agent-bar` and `~/.cache/agent-bar`; Waybar module IDs use the `agent-bar` namespace.

### Removed

- Removed the `qbar` and `agent-bar-omarchy` compatibility layer entirely:
  legacy identity constants, settings/cache path migration, Waybar legacy-asset
  cleanup, the `agent-bar-omarchy` CLI symlink and `bin` alias, and the `snippets/`
  manual examples.

### Breaking

- The `agent-bar-omarchy` command no longer exists. Installations still using the
  old name must reinstall as `agent-bar`; old settings/cache under the previous
  names are not migrated.

## [3.0.0] - 2026-03-27

### Added

- Amp provider with free/credits monitoring and SVG icon
- Interactive Waybar layout configuration via `qbar setup`
- Per-provider model selection with `Configure Models`
- Window policies for quota display (both, five_hour, seven_day)
- Settings schema versioning with validation and atomic writes
- Bun dependency check at startup
- Cache management improvements with configurable TTL (5 min default)
- Codex app-server integration with dynamic window labels
- Auto-activate provider in Waybar after login
- Right-click action shows full provider info

### Changed

- Removed Antigravity provider in favor of direct Claude/Codex/Amp integration
- Streamlined cache invalidation across providers
- Updated Waybar integration to flat-onedark theme
- Improved CLI help output with better formatting
- Simplified provider integration architecture

### Fixed

- Waybar module rendering and provider toggle behavior
- Amp icon display and tooltip tree connectors
- Cache invalidation now properly deletes stale entries
- Action-right routing for provider-specific actions

## [2.0.0] - 2026-02-09

### Added

- Complete TypeScript rewrite with Bun runtime
- Interactive TUI menu with clack/prompts
- Provider architecture: Claude, Codex, Antigravity as pluggable modules
- `qbar setup` for automated Waybar configuration (config.jsonc + style.css)
- `qbar uninstall` to cleanly remove all integration files
- `qbar update` command for self-update
- Beautiful `--help` UI matching hover/status style
- Smart context detection: shows help in interactive terminal, JSON in Waybar
- Extra Usage support with timeline visualization
- Separate Waybar modules per provider with PNG icons via CSS
- Rich Catppuccin-themed tooltips with model grouping
- Provider login/logout flows with automatic Waybar refresh
- Antigravity native OAuth login and token auto-refresh
- Per-module visual separators (pill, gap, bare, glass, shadow, none)
- Ora spinner for refresh actions
- Disconnected state indicator with red icon

### Changed

- Renamed project from llm-usage to qbar
- Cache directory moved to `~/.cache/qbar/`
- Tooltip layout redesigned with box drawing characters
- Terminal output now matches hover/tooltip style
- Waybar interval set to 2 minutes

### Fixed

- Tooltip newline handling and JSON escaping
- Cache invalidation deletes file instead of writing empty object
- Null remainingFraction treated as 0% (exhausted)
- Login terminal stays open during OAuth flows
- Antigravity percentages normalization and tier grouping
- Bar rendering when filled/empty segments are zero
- Bun PATH resolution in Waybar environment

## [1.0.0] - 2026-02-04

### Added

- Initial release as Waybar LLM usage monitor
- Claude and Codex quota monitoring via shell scripts
- Antigravity cloud fallback helper scripts
- Right-click menu for login and refresh actions
- Waybar tooltip with usage bars and reset times
- Provider visibility toggling (hide when logged out)
- Logout submenu with per-provider cache cleanup
- Auto-refresh Waybar after login/logout actions
- Monospace tooltip formatting with Pango markup
- Documentation in English and PT-BR
