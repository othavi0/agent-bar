# Spec — Fundações e confiança (trilha A)

Data: 2026-07-17 · Status: rascunho para review do dono  
Origem: brainstorming pós-auditoria profunda + rodadas impeccable
(critique / audit / distill / harden / layout / clarify) sobre o agent-bar v8.

## Contexto

O agent-bar 8.0.0 já entrega o produto: monitor de quota Waybar + TUI densa
(estilo btop), com redesign v8 (Overview removida, chart por modelo, gauges
sólidos, right-click → TUI). O código funciona e tem golden/insta fortes, mas
a manutenção concentra risco em monólitos e em parsers de terceiros.

Auditoria (sessão 2026-07-17):

| Arquivo | LOC | Problema |
| --- | --- | --- |
| `src/providers/codex.rs` | ~2058 | RPC + session log + normalize misturados |
| `src/tui/update.rs` | ~2040 | state machine monolítica |
| `src/tui/render/detail.rs` | ~1469 | layout + seções + formatação |
| `src/cli.rs` | ~1228 | parse + help (prioridade menor) |
| `src/update.rs` | ~1338 | lifecycle (prioridade menor) |
| Amp `parse_usage` | regex | formato CLI muda sem aviso |
| Waybar stdout | `unwrap_or_default` | serialize fail → string vazia silenciosa |
| Docs | residual TS | `architecture.md` ainda cita `*.ts` |

Rodadas impeccable na TUI (contexto, não escopo de implementação desta
trilha) apontaram contraste `Comment`, labels de Config, chart em 80 cols e
discoverability de help. Esses itens ficam no **backlog B/C** (ver §6).

## Decisões do dono (fechadas)

1. **Trilha A primeiro** (fundações + confiança), não polish visual.
2. **Abordagem strangler modular**: extrair módulos sem mudar comportamento;
   vários PRs pequenos; golden/insta como rede de segurança.
3. **Não-objetivos desta trilha:** redesign visual, novos providers, i18n,
   AUR/release, reabrir Overview, mudar thresholds de cor, reabrir pricing/
   câmbio, GlyphMode.
4. **Claude continua fora de `BaseProvider`.**
5. **Error strings de provider são contrato** (não reescrever).
6. **XML-escape só em `render_pango`.**
7. **Sem round-trip JSONC via serde** em `waybar_integration`.

## Objetivo

Tornar o código navegável e os parsers/contratos à prova de regressão **sem
mudar** o contrato de produto (stdout Waybar em sucesso, error strings,
snapshots golden/insta), exceto o comportamento intencional de falha de
serialização Waybar (§3.4).

## Não-objetivos

- Trilha B (hardening de produto visual/contraste/empty states).
- Trilha C (distill/polish TUI, labels humanos, help).
- Otimização de CPU/IO além do que já existe (redb usage).
- Lock multi-process no cache de quota (só se virar bug real).

---

## §1 Critério de PR verde

Cada PR desta trilha deve:

1. `cargo clippy --all-targets -- -D warnings` limpo.
2. Testes da fatia tocada (ver matriz em `CLAUDE.md` §2); se contrato
   compartilhado se moveu, ampliar para `cargo test && cargo clippy
   --all-targets -- -D warnings`.
3. **Diff de comportamento zero** nos PRs de split (A1–A3): só moves,
   `pub(crate)`, reexports. Snapshots insta/golden idênticos.
4. Conventional Commits em PT, subject ≤ 50 chars.
5. Nenhum `unwrap`/`expect` novo em produção
   (`deny(clippy::unwrap_used)` já está no crate).
6. Sem mutar `~/.config/waybar` / `~/.config/agent-bar` em testes (temp dirs
   + `XDG_*` + flags).

---

## §2 PR A1 — Split do Codex provider

### Problema

`src/providers/codex.rs` concentra JSON-RPC do app-server, fallback de
session log, tipos raw, normalização para `ProviderQuota` e o impl de
`QuotaSource`/`Provider`. Qualquer fix de parse arrisca o RPC e vice-versa.

### Design

Quebrar em submódulo, facade no path atual se necessário para imports:

```text
src/providers/codex.rs                 → reexport fino OU removido
src/providers/codex/
  mod.rs                               → impl QuotaSource + Provider (Codex)
  types.rs                             → CodexRateLimits, CodexWindowRaw, credits
  normalize.rs                         → to_quota_window, labels, build_quota
  app_server.rs                        → JSON-RPC / app-server
  session_log.rs                       → fallback de session logs
```

Regras:

- API pública do crate inalterada: `CodexProvider` e tipos que outros
  módulos importam continuam acessíveis via `providers::codex::…`.
- Testes unitários do arquivo atual migram para o submódulo que os
  exerce (ou ficam em `mod.rs` se cross-cutting).
- Nenhuma mudança de string de erro, cache key, ou shape de `ProviderQuota`.

### Verificação

```bash
cargo test providers::codex
cargo test providers::base
cargo test --test golden
cargo clippy --all-targets -- -D warnings
```

### Meta de tamanho

Nenhum arquivo em `providers/codex/` acima de ~600 LOC (orientação, não
hard gate se um subdomínio legítimo for maior).

---

## §3 PR A2 — Split de `tui/update`

### Problema

`src/tui/update.rs` (~2040 LOC) concentra navegação, config, login, fetch,
history e help num único match de `Action`.

### Design

```text
src/tui/update.rs                      → removido ou vira reexport
src/tui/update/
  mod.rs                               → pub fn update(state, action) dispatcher
  navigation.rs                        → sidebar, screen switches, help, quit
  config.rs                            → Config* actions + SaveConfig
  login.rs                             → Login*
  fetch.rs                             → ProviderFetched / refresh / status
  history.rs                           → History* / range / expand day
```

Regras:

- `Action` permanece em `src/tui/action.rs` (já existe).
- `update(state, action) -> …` (assinatura atual) é a única porta pública.
- Cada submódulo exporta handlers que o dispatcher chama; sem nova camada
  de framework.
- Testes em `update.rs` migram para o submódulo ou para `update/mod.rs`
  com `#[cfg(test)]` — preferir manter os mesmos nomes de teste.

### Verificação

```bash
cargo test tui::update
cargo test tui::
cargo clippy --all-targets -- -D warnings
```

Snapshots de render não devem mudar (update puro de estado).

---

## §4 PR A3 — Split de `tui/render/detail`

### Problema

`detail.rs` (~1469 LOC) mistura layout/colapso, seções, formatação e
estados logged-out/erro.

### Design

```text
src/tui/render/detail.rs               → removido ou reexport
src/tui/render/detail/
  mod.rs                               → render_detail / render_full
  layout.rs                            → constraints + colapso progressivo
  windows.rs                           → seção JANELAS
  chart.rs                             → TOKENS/HORA
  models.rs                            → MODELOS HOJE
  extra.rs                             → EXTRA USAGE
  totals.rs                            → linha hoje / 7 dias
  states.rs                            → logged_out / provider error
  format.rs                            → truncate_name, fmt_reset,
                                         derive_bar_width, LABEL_W, suffixes
```

Regras:

- Contrato visual v8 preservado: coluna de gauge alinhada (`LABEL_W = 12`),
  chart com mínimo efetivo, colapso EXTRA → MODELOS → chart.
- Snapshots em `src/tui/render/snapshots/agent_bar__tui__render__detail__*`
  **byte-idênticos** após o split.
- Helpers usados só pelo detail ficam em `format.rs`; se outro render
  precisar depois, extrair num PR separado (YAGNI).

### Verificação

```bash
cargo test tui::render::detail
cargo test tui::render
cargo clippy --all-targets -- -D warnings
```

---

## §5 PR A4 — Confiança e contratos

Este PR **pode** mudar comportamento em um ponto documentado (serialize
Waybar). O resto é aditivo (fixtures/docs).

### A4.1 Fixtures Amp

- Diretório: `tests/fixtures/amp/` (ou `src/providers/fixtures/amp/` se
  preferir unit-test only — preferir `tests/fixtures/amp/` para reuso).
- No mínimo dois arquivos de stdout real/sanitizado:
  - `usage-legacy-dollars.txt` — formato `$X/$Y remaining`
  - `usage-free-pct.txt` — formato `Amp Free: N% remaining today…`
- Testes em `providers::amp` (ou integration) que leem o fixture e
  assertam `primary.remaining`, account e meta relevantes.
- Objetivo: mudança de wording da CLI quebra o CI de forma óbvia.

### A4.2 Falha barulhenta na serialização Waybar

Hoje (`main.rs`):

```rust
println!("{}", serde_json::to_string(o).unwrap_or_default());
```

Isso emite string vazia se a serialização falhar — Waybar some sem
diagnóstico.

**Comportamento novo (decisão):**

1. Tentar `serde_json::to_string`.
2. Em `Ok`, imprimir em stdout como hoje.
3. Em `Err`:
   - logar o erro em **stderr** (`log::error!` ou `eprintln!` se logger
     ainda não estiver no path);
   - imprimir em stdout um **payload mínimo válido** de módulo Waybar com
     texto de erro e class de estado degradado (ex. incluir token
     `disconnected` / padrão já usado para erro de provider), **nunca**
     string vazia;
   - **não** `exit(1)` no path Waybar (Waybar trata non-zero de forma
     inconsistente entre versões; stdout estruturado é o contrato).

Teste: unitário no helper de print (extrair função pura se necessário)
cobrindo o ramo de erro com um tipo que não serializa, ou mock da
serialização se for mais simples.

### A4.3 Docs residual TS

- Atualizar `docs/architecture.md` (e qualquer doc operacional que ainda
  cite `index.ts`, `notify.ts`, `src/providers/*.ts` como fonte viva).
- Menções históricas em `CHANGELOG.md` podem ficar.
- Verificação: `git diff --check` + grep por `\.ts` em `docs/*.md`
  (exceto superpowers histórico e CHANGELOG).

### A4.4 Fora deste PR (não expandir)

- Fixtures completas de Claude API response (bom, mas opcional depois).
- Split de `cli.rs` / `update.rs` lifecycle (PR A5 opcional).

---

## §6 PR A5 (opcional)

Só se A1–A4 estiverem verdes e houver fôlego:

- `cli.rs` → `cli/{mod,parse,help}.rs`
- `update.rs` (self-update) → `update/{mod,detect,download,apply}.rs`

Mesmas regras de comportamento zero. Não bloqueia o fechamento da trilha A.

---

## §7 Ordem e dependências

```text
A1 (codex)  ──┐
A2 (tui/update) ─┼──→ A4 (confiança)  ──→ [A5 opcional]
A3 (detail) ──┘
```

A1, A2 e A3 são **independentes** entre si (podem ser PRs paralelos ou
sequenciais na ordem de maior dor: A1 → A2 → A3).  
A4 pode começar em paralelo nos itens de fixture/docs; o item Waybar
print só toca `main.rs` (e helper extraído) e não depende dos splits.

Recomendação de execução sequencial se um dev só: **A1 → A2 → A3 → A4**.

---

## §8 Invariantes do projeto (não negociar)

- Rust/cargo only; `scripts/agent-bar-open-terminal` permanece Bash.
- stdout limpo no path Waybar (só o JSON do módulo; logs em stderr).
- Legacy morto: sem `qbar`, `antigravity`, theme-repo Omarchy, etc.
- Identidade via `app_identity` constants.
- Paths injetáveis / `XDG_*` em testes.

---

## §9 Métricas de sucesso

| Métrica | Antes | Meta |
| --- | --- | --- |
| Maior arquivo Codex | ~2058 LOC monólito | submódulos ≤ ~600 LOC cada |
| `tui/update` | ~2040 LOC | dispatcher + handlers ≤ ~400 LOC/arquivo (orientação) |
| `render/detail` | ~1469 LOC | seções isoladas; snapshots idênticos |
| Amp format break | regressão silenciosa | fixture falha no CI |
| Waybar serialize fail | `""` | payload de erro + log stderr |
| Docs operacionais | residual TS | paths Rust apenas |

---

## §10 Backlog explícito (trilhas B e C — não implementar aqui)

**Trilha B — Hardening de produto**

- Contraste `Comment` (#6a7485 ≈ 2.97:1 em #282c34) e revisão de `Red`/séries.
- Consistência de rótulos de tokens se ainda houver divergência real.
- Edge cases de overflow em legendas do chart.
- Pricing fresher (tabela oficial com data).

**Trilha C — Distill / polish TUI**

- Labels humanos na Config (`providerOrder` → linguagem de UI).
- Chart em ~80 colunas (progressive disclosure).
- Discoverability do help (`?`).
- Sidebar estreita (`H/L/C`) mais legível.

Origem: rodadas impeccable 2026-07-17.

---

## §11 Riscos

| Risco | Mitigação |
| --- | --- |
| Split move teste e “some” cobertura | Rodar fatia + greps de `mod tests` após move |
| Reexport quebra path de `cargo test providers::codex` | Manter módulo `providers::codex` como raiz |
| A4.2 muda golden Waybar | Atualizar só se o payload de erro for exercitado; path feliz idêntico |
| PR grande demais | Abortar e fatiar; um monólito por PR |

---

## §12 Entregáveis

1. Esta spec (review do dono).
2. Plano de implementação (`writing-plans`) com tasks atômicas por PR.
3. PRs A1–A4 (A5 opcional) mergeados com critérios §1.

Não há mudança de versão/release nesta trilha (refactor interno +
hardening de contrato de falha).
