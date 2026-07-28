# Agent Bar v11 — Recuperação + paridade Quattro (design)

Data: 2026-07-28 · Status: aprovado pelo mantenedor · Base: `master`/`claude-ajustes` @ `4cb7407` (v10.0.0)

## Contexto

A v10.0.0 (PR #25) e o PR #28 entregaram o plugin Quickshell-nativo, mas a auditoria
de 2026-07-28 (6 áreas, workflow multi-agente, achados reverificados por leitura
direta) encontrou bugs empilhados que impedem o objetivo central do produto:
monitorar as janelas de 5h/semanal e os horários de reset dos providers.
O Omarchy Quattro (4.0.0.alpha, PR basecamp/omarchy#6231) agora traz um widget
nativo `model-usage` (Claude+Codex) que serve de implementação de referência —
funciona nesta máquina contra o mesmo endpoint com a mesma credencial.

Sintomas confirmados pelo usuário: Claude sem dados (401), Codex com dados
errados, Grok/Amp quebrados, popup ruim (denso, timestamps ilegíveis,
hierarquia confusa, chip da barra fraco), refresh/notificações não confiáveis.

Decisão de estratégia (usuário): **manter o Agent Bar e buscar paridade total
com o widget nativo**, corrigindo com fix + refactor juntos, em fases verticais
por provider — Claude primeiro.

## Objetivo

1. Monitoramento funcional dos 4 providers (Claude, Codex, Amp, Grok).
2. Eliminar as classes de bug da auditoria (auth, parsing, freshness, retry).
3. Desmontar o god-module `ServiceCore.js` e sincronizar enums com o schema.
4. Redesign do popup/chip orientado por mockups aprovados visualmente.
5. Paridade funcional com o widget nativo, com emenda formal do contrato.

## Fora de escopo (decisões explícitas)

- Refactor de `src/plugin/maintenance.rs` (4.070 linhas): dívida registrada;
  alto risco, nenhum benefício para os sintomas atuais.
- Qualquer dado monetário (spend/balance/credits/currency): continua banido.
- Reintroduzir TUI/Waybar/Pango/schema-v1: continua banido.

## Arquitetura

**Intacto**: topologia Quattro — helper Rust privado (`bin/agent-bar`) emite um
único objeto JSON schema-v2 no stdout; `Service.qml` é o único dono de
polling/processos; `BarWidget.qml` resolve o serviço via
`bar.shell.serviceFor(moduleName)`. A auditoria confirmou zero drift de
contrato com o alpha instalado (r1429) — mecânica de manifest, enable/rescan e
registro de widget corretos.

**Muda**:

- **Retry compartilhado**: extrair o loop hoje exclusivo do `GrokAdapter`
  (`adapters.rs:199`) para um helper usado pelos 4 adapters. O spec v10 já
  promete "one transient retry" para todos (`catalog.rs` declara
  `RetryPolicy::OneTransient` uniforme); o código nunca cumpriu para
  Claude/Amp.
- **Parsing tolerante de `resets_at`** em helper único no `v2_map`: RFC3339 +
  epoch em segundos e millis. Evidência de necessidade: o Codex já trata epoch
  `i64` (`v2_map.rs:277-278`) e o widget nativo trata epoch explicitamente
  (`Claude.qml:327-345`) — o endpoint real é menos uniforme que o contrato
  documentado (JSON-014).
- **Schema v2 aditivo, não v3**: campos novos da Fase 5 entram como opcionais.
  QML antigo ignora; QML novo renderiza. Nenhuma quebra para consumidores.
- **`ServiceCore.js` fatiado** por responsabilidade: envelope/estado, chip,
  popup, settings, maintenance, a11y. Enums (`PROVIDER_STATES`/`ACTION_KINDS`)
  deixam de ser cópias manuais: gerados ou verificados por teste contra
  `status-v2.schema.json`, eliminando o congelamento silencioso do popup
  quando o Rust ganha um state novo.

## Fase 1 — Claude completo

Critério de pronto: a barra real do mantenedor mostrando 5h + semanal +
resets do Claude com dado vivo (prova funcional + perceptual + de dados).

TDD por item; o teste de header nasce antes do fix (é o teste que teria
impedido o bug de shippar — o irmão do Grok já o faz em `adapters.rs:785`):

1. **Header `Bearer`** — `adapters.rs:382` envia o token OAuth cru; a API
   responde 401 e a UI mostra "unauthenticated" com credencial válida.
   Fix: prefixar `"Bearer "` (padrão do Grok, `adapters.rs:155`) + assert de
   `last_headers` no teste do Claude.
2. **Bucket `seven_day_oauth_apps`** — `ClaudeUsageDoc` (`v2_map.rs:378-397`)
   não declara o campo; serde descarta em silêncio. Fix: adicionar campo e
   preferi-lo sobre `seven_day` (precedência do widget nativo,
   `Claude.qml:406`).
3. **Dedup de IDs de janela** — `limits[]` dinâmico e campos legados
   `seven_day_opus`/`seven_day_sonnet` geram o mesmo id
   (`weekly_model_id`, `v2_map.rs:512` vs `:532`);
   `ensure_unique_window_ids` (`status/schema.rs:996-1007`) rejeita e o
   coordinator (`status/coordinator.rs:240`) rebaixa a linha inteira do Claude
   para `provider_error`, apagando 5h/semanal válidos. Fix: deduplicar por id
   na montagem (`limits[]` vence; legado só preenche ausências).
4. **`resets_at` tolerante** — helper compartilhado (ver Arquitetura),
   aplicado ao Claude nesta fase.
5. **Retry compartilhado** — helper extraído do Grok, aplicado ao Claude
   nesta fase (Amp adota na Fase 4).
6. **Expiry client-side** — token vencido vira estado próprio ("expirado",
   com último dado em cache exibido e hint de login), não o genérico
   "authentication was rejected". Sem refresh de token: o contrato "não
   manuseia credenciais" permanece.
7. **Tier real** — `parse_claude_credentials` passa a ler `rateLimitTier`
   (`max_20x` → "Max 20x"), com fallback para `subscriptionType`.
8. **Código morto** — remover o falso "double-division guard"
   (`v2_map.rs:554-563`, ramos idênticos) e cobrir com teste o caso
   percentual real (1.0 renderiza 1%, não 100%).

## Fase 2 — UI transversal + redesign do popup

O usuário confirmou os quatro problemas visuais: denso/poluído, timestamps
ilegíveis, hierarquia confusa, chip da barra fraco.

1. **Humanização de tempo** — helper único (`formatWhen`): resets como
   "em 2h 14m", atualização como "há 3 min". Atualizar o teste que hoje
   **fixa** a string ISO crua (`tst_BarWidget.qml:257-263`).
2. **Redesign do popup e do chip** — GATE OBRIGATÓRIO: mockups visuais
   (HTML/companion) comparando opções de hierarquia e densidade, aprovados
   pelo mantenedor ANTES de tocar QML de layout. O redesign ataca os quatro
   problemas confirmados; o chip passa a comunicar estado de relance
   (pior janela/percentual mais crítico entre providers habilitados).
3. **`error.retryable` de verdade** — QML passa a ler o boolean do schema em
   vez da lista hardcoded paralela de states.
4. **Split do `ServiceCore.js`** + enum sync por teste/geração (ver
   Arquitetura). Funções test-only (`requiredScreenshotNames`,
   `themePalette`) saem do bundle shipado.
5. **Limpeza rust-core** — remover os 4 `let _ =` vestigiais
   (`status/coordinator.rs:221`, `cache/store.rs:121`, `cli/mod.rs:651`,
   `notifications/mod.rs:193`), o subsistema forced-targets nunca invocado
   (`cache/coordinator.rs:87`) e a dependência dormante apontada pela
   auditoria; endurecer o teste anti-dormant para checar uso real, não
   comentário.

## Fase 3 — Codex

1. **Freshness honesta** — dado de session-log (potencialmente de horas
   atrás) deixa de ser reportado como `DataSource::Live` com
   `last_success_at: now`; passa a carregar fonte e timestamp reais.
2. **Sandbox no spawn** — `codex app-server` ganha `-s read-only -a
   untrusted` (paridade com o scanner nativo; princípio do menor
   privilégio).
3. **3º estágio legalizado** — a leitura de `~/.codex/rate-limits.json`
   (`adapters.rs:304-308`) ganha cap de 1 MiB + rejeição de symlink (mesmas
   proteções do session-log) e entra no spec (hoje o contrato documenta só
   2 estágios).
4. **Dedup de schemas** — eliminar os tipos paralelos de
   `codex_app_server.rs:28-58` e o round-trip parse→build→serialize→reparse
   (`normalize_to_rate_limits_json` → `codex_from_rate_limits_json`);
   deserializar uma vez nos tipos do `v2_map`.
5. **Remover `rate_limits_by_limit_id`** — caminho especulativo sem fixture,
   sem teste e sem contraparte no spec ou na referência.
6. **Retry no Codex** — adotar o helper compartilhado nos estágios
   transitórios (RPC/timeout), honrando o `RetryPolicy::OneTransient` que o
   catalog já declara para o provider.

## Fase 4 — Amp/Grok

1. **Amp: classificador de auth específico** — fim do substring `"auth"`
   (`adapters.rs:54-56`); matching por frases/padrões conhecidos do CLI.
2. **Amp: parse-miss visível** — saída do `amp usage` que não casa nenhum
   padrão conhecido vira `provider_error` "formato não reconhecido"
   (retryable), distinto do estado válido "conectado sem janela" (`—`).
3. **Grok: `currentPeriod.type` respeitado** — rótulo da janela deixa de ser
   "weekly" hardcoded (`v2_map.rs:166`).
4. **Grok: expiry local** — checar `expires_at` do `auth.json` antes do HTTP;
   vencido → estado "expirado" acionável (mesmo padrão da Fase 1 item 6).
5. **Dead code** — remover `grok_from_auth_and_signals`/`GrokSignals`
   (implementação pré-#28) e as 4 fixtures órfãs.
6. **Retry no Amp** — adotar o helper compartilhado (fecha a promessa do
   spec para os 4 providers).

## Fase 5 — Paridade com o nativo + emenda de contrato

A emenda vem ANTES do código desta fase:

1. **Emenda v11 do contrato** (`CLAUDE.md` + spec): permitir histórico e
   stats locais não-monetários (o v10 os proíbe: "v10 removes: session
   history and charts"); documentar os headers do Claude na linha do spec
   (a assimetria de documentação — Grok com headers explícitos, Claude sem —
   plausivelmente deixou o bug do Bearer passar); atualizar a menção à
   branch de implementação.
2. **Coexistência com o widget nativo documentada** — ambos consultam
   `api.anthropic.com/api/oauth/usage`; rodar os dois duplica polling no
   mesmo endpoint rate-limitado (risco de 429). Documentar em
   `docs/runtime.md` com recomendação de habilitar um só.
3. **Stats locais via Rust** — subcomando do helper (sem Python, contrato
   Rust/QML) lendo `~/.claude/stats-cache.json`, `history.jsonl` e
   `~/.claude/projects` com os mesmos caps/anti-symlink de arquivo;
   equivalente ao scanner Python do nativo. Campos aditivos no schema v2.
4. **UI de paridade** — tokens/dia por modelo, prompts/sessões de hoje,
   atividade recente no popup — com mockup aprovado antes (mesmo gate da
   Fase 2).
5. **Settings schema declarativo do Quattro** — decidir na fase: adotar
   (scriptável via `omarchy bar plugin set`) ou registrar em
   `docs/runtime.md` por que a surface própria permanece. Critério: se o
   redesign da Fase 2 reduzir a settings surface a tipos flat, adotar.

## Tratamento de erros

Inalterado no princípio: falha operacional de provider é **dado tipado**,
nunca falha de processo. As mudanças refinam a taxonomia: "expirado" distinto
de "rejeitado" (Fases 1/4), parse-miss distinto de "sem janela" (Fase 4),
freshness real na fonte (Fase 3). Nenhum item relaxa redação de
segredos/tokens em logs, cache, screenshots ou UI.

## Testes e verificação

- TDD por item: teste falhando → fix mínimo → verde (regra do repo).
- Gate de cada fase: `cargo fmt --check` + `cargo test` + `cargo clippy
  --all-targets -- -D warnings` + `git diff --check`; mudanças QML rodam
  `qmllint` + `qmltestrunner` offscreen (a auditoria constatou que a suíte
  QML nunca foi executada de fato — ela roda como baseline já na Fase 1);
  ShellCheck para scripts.
- Fim de fase = QA ao vivo autorizado na barra real do mantenedor: prova
  funcional (testes), perceptual (screenshot lado a lado) e de dados (valores
  reais conferidos contra `claude.ai`/fontes).
- Fixtures novas por provider cobrindo os shapes reais observados
  (percent-scale 1.0, epoch vs ISO, buckets OAuth, period types do Grok).

## Riscos e mitigação

- **API da Anthropic sem contrato público** — mitigado seguindo o widget
  nativo como referência viva e com parsing tolerante; divergência futura
  cai em `provider_error` tipado, nunca crash.
- **Refactor QML grande (Fase 2)** — mitigado pelo split incremental com a
  suíte `qmltestrunner` rodando de verdade e screenshots comparativos.
- **Alpha do Quattro em movimento** (r1429, atualiza com frequência) —
  mitigado pela checagem de drift no início de cada fase (barata: a
  auditoria já mapeou os pontos de contato).

## Referências

- Auditoria 2026-07-28 (workflow `agent-bar-deep-audit`, 6 áreas + síntese).
- Widget nativo: `/usr/share/omarchy/shell/plugins/model-usage/` (referência
  de endpoint, precedência de buckets e sandbox flags).
- PR basecamp/omarchy#6231 (Quattro), PRs locais #25 e #28.
- `docs/specs/v10/` — contrato v10 vigente até a emenda da Fase 5.
