# Redesign da TUI v8 — spec

**Data:** 2026-07-10 · **Status:** aprovada pelo usuário (brainstorming com mockups)

## Contexto

A TUI atual (pós-redesign v7) mantém cinco problemas apontados pelo usuário e
confirmados por auditoria de código + snapshots + inspeção ao vivo:

1. Boot sempre na tela Visão Geral (`AppState::new` → `Screen::Overview`,
   `src/tui/state.rs:241`); right-click do Waybar cai em `action-right`, que
   imprime relatório estático e espera Enter — beco sem saída.
2. Detail é um `Paragraph` único top-anchored sem layout por seção
   (`src/tui/render/detail.rs:471-554`): 13–19 linhas em branco em terminal de
   32 linhas; overflow corta conteúdo em silêncio.
3. Gráfico tokens/h do Detail é 1 linha de sparkline agregada numa cor; não
   existe série por modelo no pipeline (`bucket_by_hour` agrega por provider).
   Com escala linear, Opus (9,8M/dia) desapareceria ao lado de Fable (264M/dia).
4. Nomes de modelo aparecem como id raw truncado (`claude-opus…`);
   `pricing.rs` não conhece `claude-fable-5` (modelo dominante do usuário) —
   custo silenciosamente ausente.
5. Config Waybar: campo `interval` é órfão (`module_definition` hardcoda 120 —
   provado ao vivo: settings.json=60, modules.jsonc=120); `fxRate` mora na aba
   Waybar mas só afeta a TUI. Gauges usam `lerp_rgb` gradiente + pulso
   (percepção "AI slop").

Decisões tomadas com o usuário (AskUserQuestion + mockup HTML com dados reais):
**(a)** matar a Visão Geral e abrir direto no provider; **(b)** Histórico com
dias expandíveis contendo sessões e gastos; **(c)** direção visual
**One Dark Turbo** (base One Dark, gauges sólidos, séries recalibradas).

## §1 Navegação e boot

- **Removem-se** `Screen::Overview`, `SidebarItem::Overview` e
  `src/tui/render/dashboard.rs`. Sidebar passa a ter duas seções:
  `PROVEDORES` (um item por provider habilitado, com % da janela de sessão à
  direita quando disponível) e `MAIS` (Histórico, Login, Waybar).
- `run_tui(ctx, initial_provider: Option<&str>)` propaga o alvo até
  `event_loop::run`, que o guarda em `AppState.pending_focus: Option<String>`.
  Resolução é **lazy e por id**: no braço `Action::ProviderFetched`, se
  `quota.provider == pending_focus`, setar `screen = Detail`,
  `selected = índice recém-inserido`, limpar `pending_focus`. Nunca resolver
  por índice fixo no boot — `state.providers` é populado por ordem de chegada
  de fetch, não pela ordem do registry.
- Boot do `menu` (e fallback interativo): `initial_provider` = primeiro
  provider da ordem do registry filtrado por `settings.waybar.providers`.
  Enquanto o fetch do alvo não chega, renderizar o Detail dele em skeleton
  (moldura + placeholders), nunca tela vazia nem outra tela.
- `action-right <provider>` **sempre abre a TUI** com
  `pending_focus = provider`:
  - provider logado → Detail do provider;
  - indisponível/desconectado (`looks_disconnected` mantém a semântica atual)
    → TUI abre na tela **Login** com o provider pré-selecionado (o fluxo
    `login_spawn` existente cuida do suspend/resume do terminal).
  - O caminho antigo (print estático + `wait_enter`) morre; relatório estático
    continua disponível via `agent-bar terminal`/`status`.
- Teclas: `Esc` em Histórico/Login/Waybar volta ao provider focado (último
  Detail visitado; fallback = primeiro habilitado). `Esc` num Detail não faz
  nada; `q` sai. Demais atalhos (`h/g/w`, `j/k`, Enter, `r`, `?`) inalterados.
- Mouse: semântica atual preservada (click em item da sidebar ativa).

## §2 Tela Detail

- `render_full` deixa de empilhar `Vec<Line>` num `Paragraph` único e passa a
  usar `Layout::vertical` com um `Rect` por seção:
  1. JANELAS — `Length(n_janelas)`;
  2. GRÁFICO tokens/h — **`Fill(1)` com mínimo efetivo de 6 linhas** (absorve
     a altura excedente do terminal — é o fix estrutural do espaço em branco);
  3. MODELOS HOJE — `Length(n_modelos)`;
  4. EXTRA USAGE (só Claude, quando houver) — `Length(n)`;
  5. TOTAIS — `Length(1)`;
  6. chips — `Length(1)`.
- **Gráfico de colunas empilhadas por modelo** (novo widget, substitui o
  sparkline de 1 linha):
  - 24 colunas (1/h), cada coluna empilha os modelos do provider de baixo pra
    cima na ordem fixa de slots (ver §6); resolução vertical de ⅛ de célula
    (block chars `▁▂▃▄▅▆▇█`), coluna com 2 células de largura + 1 de gap
    quando couber, degradando pra 1+1 em terminais estreitos.
  - **Escala √** no eixo Y (rótulos calculados pela própria escala); uso > 0
    garante no mínimo ⅛ de bloco (nenhum modelo com uso vira invisível).
  - Eixo X com horas locais; legenda com `● Nome tratado  total` por série.
  - O mesmo widget serve o Histórico (§4) — um único componente de chart de
    colunas no crate.
- Nomes de modelo: novo helper `display_model_name(id: &str) -> String` em
  módulo compartilhado (ex. `src/usage/model_names.rs`):
  - `claude-fable-5` → `Fable 5`; `claude-opus-4-8` → `Opus 4.8`;
    `claude-sonnet-5` → `Sonnet 5`; `claude-haiku-4-5` → `Haiku 4.5`;
    padrão geral: remover prefixo `claude-`, capitalizar família, versões com
    `.`; ids codex tipo `gpt-5.5-codex` → `GPT-5.5 Codex`.
  - Fallback: id original truncado com `…` (comportamento atual).
  - Usado em: MODELOS HOJE, legendas de gráfico, sessões do Histórico.
- MODELOS HOJE: largura do gauge derivada da largura real (reusar
  `derive_bar_width`; **`FIXED_GAUGE_W` morre**), coluna de custo por modelo
  (pricing §3), guard de overflow horizontal igual ao das janelas.
- Overflow vertical: se o conteúdo mínimo não couber, colapsar seções na ordem
  EXTRA USAGE → MODELOS HOJE (vira linha-resumo "N modelos · $X") → gráfico
  encolhe até 6 → nunca corte silencioso; indicador `…` quando algo colapsou.
- Estados: fetch pendente → skeleton da seção; erro → mensagem na seção com a
  string de erro do provider (contrato de strings preservado).

## §3 Pipeline de dados de uso

- `UsageRecord` ganha `session_id: Option<String>` e `project: Option<String>`:
  - Claude: derivados do path `~/.claude/projects/<projeto>/<sessão>.jsonl`
    (arquivo = sessão; dir = projeto, exibido sem o prefixo de path escapado).
  - Codex: `~/.codex/sessions/...` → session id do arquivo; project ausente.
  - Amp permanece sem records locais (inalterado).
- Novos agregadores em `src/usage/buckets.rs`:
  - `bucket_by_model_hour(records, provider, horas)` → séries do gráfico
    (mapa modelo → vec de tokens/h, com merge dos modelos além dos 6 slots em
    "outros");
  - `sessions_by_day(records)` → `Vec<DaySessions { date, tokens, cost_usd,
    sessions: Vec<SessionAgg { start, project, dominant_model, tokens,
    cost_usd }> }>`, ordenado desc por data e hora de início.
- `pricing.rs` reescrito com a tabela oficial
  ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing),
  verificada em 10/07/2026), USD por MTok (input / output / cache write 5m /
  cache read):
  - `fable` e `mythos`: 10 / 50 / 12.50 / 1
  - `opus` 4.5–4.8: 5 / 25 / 6.25 / 0.50 · `opus` ≤ 4.1: 15 / 75 / 18.75 / 1.50
  - `sonnet-5`: 2 / 10 / 2.50 / 0.20 (introdutório até 2026-08-31; nota no
    código) · `sonnet` ≤ 4.6: 3 / 15 / 3.75 / 0.30
  - `haiku-4-5`: 1 / 5 / 1.25 / 0.10
  - codex/gpt: manter tabela atual.
  - Modelo sem entrada: `cost_usd_of` → `None` **+ log::warn de modelo
    desconhecido (uma vez por modelo por execução)**; UI mostra `—`.
- Janela do Detail continua "hoje" (meia-noite local); Histórico continua 7d.

## §4 Histórico

- Substitui a tabela dia×provider por **dias expandíveis**:
  - Linha-dia: `▸/▾ dd/mm · rótulo (hoje/dia-da-semana) · tokens · custo ·
    N sessões`; Enter/click alterna expandido.
  - Expandido: uma linha por sessão — `hh:mm · projeto · modelo dominante
    (nome tratado) · tokens · custo`, ordenadas desc por início; sessões além
    do que couber roláveis com o scroll existente (indicadores de overflow
    mantidos).
  - Amp: linha própria por dia com `AmpDollars` (sem sessões), como hoje.
- Gráfico do topo: mesmo widget de colunas por modelo do §2, com toggle
  `[t]` 24h/7d (comportamento atual do range preservado: tabela sempre 7d).
- Rodapé: total 7d em tokens + custo + nº de sessões.
- Legend box do gráfico herda o `BorderType` heavy do painel (fix da
  inconsistência de borda).

## §5 Config (Waybar + TUI)

- **Fix do bug do `interval`**: `module_definition`/`export_waybar_modules`/
  `apply_waybar_integration` recebem `settings.waybar.interval` e o gravam no
  modules.jsonc. Teste de regressão cobre settings=60 → jsonc=60.
- A tela `Waybar` vira `Config`, com duas seções visuais:
  - `WAYBAR` — providers, providerOrder, separators, displayMode, signal,
    interval (afetam a barra; save mantém settings::save +
    apply_waybar_integration + reload SIGUSR2);
  - `TUI` — fxRate (e futuros settings de exibição do menu), rotulada
    "afeta só este menu".
- Campo `signal` ganha descrição: "sinal para refresh externo
  (`pkill -SIGRTMIN+<n> waybar`); o agent-bar não o dispara sozinho".
- `CLAUDE.md` do projeto: **remover** a regra do "cache de 5s em
  waybar_contract.rs" (não existe no código Rust; resíduo do port TS).

## §6 Tema — One Dark Turbo

- Base One Dark preservada (`ColorToken` continua a fonte única). Novos tokens:
  - Séries de gráfico `Series1..Series6` (ordem fixa por entidade — Claude:
    Fable→slot1, Opus→2, Sonnet→3, Haiku→4; codex→5; outros→6):
    `#3f8fd6 #cb7e30 #b562d6 #55a34a #2ba3b4 #af8f2c`
    (validadas: banda OKLCH L 0.48–0.67, chroma ≥ 0.1, CVD ΔE adjacente ≥ 12.6,
    contraste ≥ 3:1 sobre `#282c34`). Cor segue a entidade, nunca o rank —
    filtrar séries não repinta as sobreviventes.
  - `Accent = #61afef` (seleção, teclas de chip, borda focada, título ativo).
- **`gauge_spans` perde o `lerp_rgb`**: fill sólido na cor de severidade (ou
  de série, em MODELOS HOJE) com precisão de ⅛ (`▏▎▍▌▋▊▉█`), trilho
  `EmptyTrack`. **`pulse_color` morre**; estado crítico (<10%) sinaliza com
  ícone `●` na cor Red + percentual em bold — sem animação de brilho.
- Os 3 `Color::Rgb` literais (`render/mod.rs:164`, `login.rs:47`,
  `config.rs:52`) viram tokens (`Bg`, `SelBgAlt` ou reuso de `SelBg`).
- `theme_bridge::provider_color` delega para `theme::provider_hex` (mata a
  duplicação). Severidade e cores de provider inalteradas.
- `NO_COLOR` e glyph fallback: contratos existentes preservados.

## §7 Testes e verificação

- **Regressão do interval**: settings.waybar.interval=60 →
  export_waybar_modules grava 60 (goldens do contrato Waybar atualizam **de
  propósito**: interval variável).
- **Unit**: `display_model_name` (ids atuais + fallback), pricing (fable,
  opus 4.8 vs 4.1, sonnet 5, haiku, desconhecido → None + warn),
  `bucket_by_model_hour` (merge "outros", 24/168h), `sessions_by_day`
  (derivação de session/project do path, dominant_model, custo),
  `pending_focus` (fetch tardio, provider inexistente → permanece no
  fallback, fetch de outro provider não rouba o foco).
- **Snapshots insta** novos: detail 80×24 / 100×32 / 160×44 (verificar que o
  gráfico absorve altura e nada fica em branco), detail com colapso de seção,
  history colapsado + expandido, config com seções WAYBAR/TUI, sidebar sem
  Overview (larga + colapsada), login via pending_focus.
- Matriz do CLAUDE.md §2: `cargo test providers::<p>`, `cache`, `settings`,
  `formatters`, `--test golden`, `waybar_contract`, `waybar_integration` +
  `cargo clippy --all-targets -- -D warnings` antes do handoff.
- Verificação perceptual: screenshot da TUI real lado a lado com o mockup
  aprovado (mesmos dados) antes de declarar pronto.

## Fora de escopo

- Série por modelo pro Amp (sem logs locais de token).
- Persistência do índice de usage em disco (TODO existente em
  `usage/cache.rs` permanece).
- Tema configurável pelo usuário (paleta segue compile-time).
- Mudanças no formato do módulo da barra (texto/tooltip) além do interval.
