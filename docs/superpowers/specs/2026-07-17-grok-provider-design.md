# Spec — Provider Grok (Grok Build CLI)

Data: 2026-07-17 · Status: aprovada pelo dono (brainstorming)  
Origem: pedido de provider Grok; decisão de produto = **uso local do Grok Build
CLI** (não API prepaid, não SuperGrok chat web).

## 0. Princípio de produto (não negociar)

O módulo **não** promete “cota de plano” estilo Claude (5h/semana). A xAI **não
expõe** endpoint estável de quota restante para o OAuth do Grok Build CLI.

O que o módulo responde, em um relance:

> Estou logado no Grok Build? Quanto do **contexto da sessão recente** ainda
> cabe? Qual modelo? Quanto mexi hoje (sessões/turns)?

Qualquer copy de UI (tooltip, builder, docs) deve deixar explícito que o `%` é
**contexto de sessão**, não billing nem limite de mensagens SuperGrok.

## 1. Decisões fechadas

| # | Decisão |
| --- | --- |
| D1 | Superfície: **Grok Build CLI** (`~/.grok`), não Management API / prepaid |
| D2 | Padrão: **`BaseProvider` / `QuotaSource`** (como Amp/Codex) |
| D3 | Rede: **zero** na v1 (só filesystem + existência de binário) |
| D4 | `primary` = **% restante de contexto** da sessão mais recente com signals |
| D5 | Settings **existentes** não ganham `grok` sozinhas; **defaults de install novo** incluem `grok` via `KNOWN_PROVIDER_IDS` |
| D6 | Login TUI: `grok login` |
| D7 | **Não** refresh de OAuth (mesmo princípio do Claude: não competir com o CLI no refresh token) |
| D8 | Cache TTL default: **90s** (local, como codex/amp) |
| D9 | ID estável: `grok` · display: `Grok` · cache key: `grok-usage` |

## 2. Fontes de verdade (paths)

Resolver home Grok:

```text
GROK_HOME env (não vazio) → PathBuf
senão → $HOME/.grok
```

| Path | Uso |
| --- | --- |
| `{grok_home}/auth.json` | login, expiry, account label |
| `{grok_home}/sessions/**/signals.json` | métricas de sessão |
| `{grok_home}/bin/grok` e `PATH` | detecção de CLI / login spawn |

Injetar em `Paths` (como claude/codex/amp):

- `grok_home: PathBuf`
- `grok_auth: PathBuf` (= `grok_home.join("auth.json")`)

Testes: temp dir + override de paths (não ler o `~/.grok` real do dev).

### 2.1 Shape de `auth.json` (observado 2026-07-17)

Objeto mapa:

```text
"https://auth.x.ai::<client_id>" → {
  auth_mode, key, refresh_token, expires_at,
  first_name, user_id, team_id, principal_id, …
}
```

- `expires_at`: RFC3339 com fração de segundo (ex. `2026-07-18T03:29:45.462038593Z`)
- `key`: access JWT (nunca logar, nunca cachear em disco do agent-bar)
- Pode haver **mais de uma** entrada; usar a de `expires_at` mais distante no futuro
  entre as que tenham `key` não vazio; se todas expiradas → not logged in

### 2.2 Shape de `signals.json` (observado 2026-07-17)

Campos relevantes (outros ignorados com `#[serde(default)]`):

| Campo | Tipo | Notas |
| --- | --- | --- |
| `contextTokensUsed` | number | tokens de contexto usados |
| `contextWindowTokens` | number | janela (ex. 500000) |
| `contextWindowUsage` | number | **% de uso** aproximado (arredondado; validar com used/total) |
| `primaryModelId` | string | ex. `grok-4.5` |
| `modelsUsed` | string[] | opcional |
| `turnCount` | number | |
| `sessionDurationSeconds` | number | opcional |

**Recência da sessão:** mtime do arquivo `signals.json` (filesystem). Se no
futuro aparecer `last_active_at` em `summary.json` irmão, preferir esse campo
quando presente (YAGNI na v1: só mtime).

**Prova de semântica de `contextWindowUsage`:** amostras reais mostraram
`usage ≈ round(100 * used / total)` (ex. used=63533/500000 → calc 12.7%, campo 12).
Portanto o campo é **% usado**, não restante.

## 3. Algoritmo `get_quota`

### 3.1 `is_available` (barato, sem rede)

`true` se **qualquer** de:

1. `grok_auth` existe e é arquivo legível (mesmo que JSON inválido — o parse
   detalhado fica no fetch), **ou**
2. `find_grok_bin(home)` retorna `Some` (PATH ou `{grok_home}/bin/grok` se
   `is_file`)

Se ambos falham → unavailable com `GrokError::NotInstalled`.

### 3.2 Auth gate (dentro de `fetch_raw` / build)

1. Ler e desserializar `auth.json`.
2. Se arquivo ausente ou JSON inválido ou nenhuma entry com `key` →
   `NotLoggedIn`.
3. Parsear `expires_at` com `time` (RFC3339; tolerar nanos truncando para
   precisão suportada). Se **todas** as entries com key estão com
   `expires_at < now` → `NotLoggedIn` (**não** tentar refresh).
4. Account label: `first_name` trim não vazio, senão `user_id` curto (8 chars),
   senão `"Grok"`.

### 3.3 Scan de signals

1. Walk recursivo sob `{grok_home}/sessions` por arquivos nomeados
   exatamente `signals.json` (não seguir symlinks para fora de `sessions`
   se o walk permitir; `std::fs::read_dir` recursivo manual ou `walkdir` —
   **preferir std only** se o crate ainda não tem walkdir: implementação
   recursiva simples com profundidade máxima **16** e limite de **2000**
   arquivos visitados para não travar o poll da barra).
2. Para cada arquivo: `metadata.modified()` + parse JSON tolerante.
3. Ordenar por mtime desc; **sessão mais recente** = first.
4. Se lista vazia: `available=true`, `primary=None`, meta
   `noSessions=true` (ou omitir primary — builder mostra “sem sessões”).

### 3.4 Cálculo de `primary` (contexto restante)

Da sessão mais recente com `contextWindowTokens > 0` e
`contextTokensUsed` presente:

```text
used_pct = 100.0 * (contextTokensUsed as f64) / (contextWindowTokens as f64)
remaining = (100.0 - used_pct).clamp(0.0, 100.0)
// arredondar remaining para inteiro-ish como outros providers:
// remaining.round() em f64 no QuotaWindow.remaining
```

**Não** usar só `contextWindowUsage` como fonte primária (é arredondado);
usar a razão used/total. Se `contextWindowTokens == 0` ou missing → sem
primary.

`QuotaWindow`:

```text
remaining: <calculado>
resets_at: None          // contexto não “reseta” com wall-clock
window_minutes: None
used: Some(used_pct.round())
severity: None
```

### 3.5 Secondary e extras (v1 mínima)

- **secondary:** `None` na v1 (evitar inventar segunda janela).  
  Opcional no tooltip via **meta** / models, não via secondary, para não
  poluir severidade da barra.
- **models:** se `primaryModelId` presente, um entry
  `IndexMap { model_display => mesma QuotaWindow do primary }` (nome
  tratado: `grok-4.5` → `Grok 4.5` com helper local simples).
- **plan:** `primaryModelId` tratado ou `"Grok Build"`.
- **extra:** `ProviderExtra` — **não** criar variante nova se o generic
  builder bastar. Preferir `extra: None` e meta no builder via campos
  account/plan/models. Se precisar de meta string-only, usar padrão Amp
  (`ProviderExtra::Amp` é errado semanticamente) — **melhor:** não usar
  untagged Amp; colocar contagens só no **builder** lendo campos que
  passamos via…  

**Problema:** `ProviderQuota` não tem `meta` genérico fora de `ProviderExtra`.

**Decisão minuciosa:**

1. Adicionar `ProviderExtra::Grok(GrokQuotaExtra)` com:

```text
GrokQuotaExtra {
  sessions_today: Option<u32>,
  turns_today: Option<u32>,
  context_tokens_used: Option<u64>,
  context_window_tokens: Option<u64>,
  recent_model: Option<String>,
}
```

2. Serialização untagged como as outras variantes (serialize-only no quota).
3. Builder `formatters/builders/grok.rs` consome `get_grok_extra`.
4. Contagem “hoje”: mtime do signals em **data local** (`ctx.local_offset`)
   igual a “hoje”; somar `turnCount` e contar arquivos.

### 3.6 Cache

- key: `grok-usage`
- TTL: `ctx.ttl_ms("grok")` default 90s
- Raw: struct com dados **já sanitizados** (sem access/refresh tokens):

```text
GrokRaw {
  account: Option<String>,
  logged_in: bool,
  sessions: Vec<GrokSessionSnap>, // mtime_ms, used, window, usage_field, model, turns
}
```

Auth revalidada a cada fetch_raw (barato); se raw veio do cache, o
`build_quota` ainda confia no raw (TTL curto). **Não** colocar JWT no raw.

## 4. Erros (contrato verbatim)

```text
Grok CLI not installed. Install from https://x.ai/cli or ensure ~/.grok/bin/grok is on PATH.
Not logged in. Open `agent-bar menu` and choose Provider login.
Failed to read Grok credentials.
Failed to parse Grok session data.
```

- `NotInstalled` → `unavailable_error` quando `!is_available`  
- `NotLoggedIn` → gate de auth / expiry  
- `Failed to read…` → auth.json ilegível/JSON quebrado com arquivo presente  
- `Failed to parse…` → walk ok mas **todas** as sessions falharam parse **e**
  auth ok — raro; se auth ok e zero sessions válidas por parse, preferir
  available sem primary + log debug, **não** error (degradação).  
  `Failed to parse` só se auth ok e o walk estourou limite de segurança
  sem conseguir ler nada útil **e** havia signals no caminho — na prática
  YAGNI: **omitir** essa string na v1 se não houver caso claro; manter no
  enum para futuro.

**v1 strings que testes devem cobrir:** as três primeiras. A quarta só se
implementada com caso de teste.

## 5. Integração no crate (checklist)

Espelhar `docs/new-provider.md` + esta lista (mais precisa):

| # | Arquivo / local | Ação |
| --- | --- | --- |
| 1 | `src/providers/grok.rs` | Provider + parse + scan |
| 2 | `src/providers/grok_cli.rs` | `find_grok_bin` (espelho amp_cli) |
| 3 | `src/providers/mod.rs` | `mod grok`; `registry()` push `GrokProvider` |
| 4 | `src/providers/error.rs` | `GrokError` + `From` em `ProviderError` |
| 5 | `src/providers/types.rs` | `GrokQuotaExtra` + `ProviderExtra::Grok` |
| 6 | `src/providers/extras.rs` | `get_grok_extra` |
| 7 | `src/config.rs` | `KNOWN_PROVIDER_IDS` **4** ids; `default_ttl_secs("grok")→90`; `Paths` |
| 8 | `src/theme.rs` | `provider_hex("grok")` → cor dedicada (ver §6) |
| 9 | `src/waybar_contract.rs` | `WAYBAR_PROVIDERS` inclui `grok`; CSS/ícone |
| 10 | `icons/grok-icon.svg` (ou png) | asset |
| 11 | `src/formatters/builders/grok.rs` | tooltip |
| 12 | `src/formatters/builders/mod.rs` | `pub mod grok` |
| 13 | `src/formatters/waybar.rs` | match provider id |
| 14 | `src/formatters/terminal.rs` | match provider id |
| 15 | `src/tui/login_spawn.rs` | `"grok" => grok login` |
| 16 | TUI login list / ids | incluir `grok` na ordem de providers de login |
| 17 | `src/usage/model_names.rs` | opcional: `grok-*` display (se fácil) |
| 18 | testes unitários + fixtures | `tests/fixtures/grok/` |
| 19 | golden / waybar_contract tests | atualizar contagens de 3→4 providers onde fixo |
| 20 | `docs/new-provider.md` | linha na tabela de padrões |
| 21 | `README.md` | mencionar Grok Build na lista de providers |

**Ordem no registry (display default):**  
`claude`, `codex`, `amp`, `grok` — Grok por último para não reordenar a barra
de quem já tem os três.

**Settings existentes:** `normalize_provider_selection` só mantém ids
presentes na lista do user; **não** injeta `grok`.  
**Settings novas / default:** `default_providers()` itera
`KNOWN_PROVIDER_IDS` → inclui `grok`. Documentar no CHANGELOG: “novos
installs passam a listar grok; quem já tem settings precisa habilitar em
Config”.

## 6. Identidade visual

| Token | Valor |
| --- | --- |
| Cor marca | `#56b6c2` (Cyan One Dark — distinto de Claude orange, Codex green, Amp magenta) |
| Ícone Waybar | `icons/grok-icon.svg` — glifo simples “G” ou marca abstrata monocromática legível em 16–22px |
| CSS | mesmo pipeline `export_waybar_css` / seletor `#custom-agent-bar-grok` |

Severidade da barra: **só** pelo `primary` (contexto restante) via
`status_for_percent` — contexto cheio (remaining baixo) = warn/critical,
o que comunica “sessão inchada”, não “plano acabando”. Aceitável e
documentado no tooltip.

## 7. Builder / tooltip (conteúdo)

Linhas sugeridas (Pango via segments, escape só no render_pango):

```text
Grok · <model>
contexto  ████░░░░  87% restante
39k / 500k tokens · sessão recente
hoje  3 sessões · 12 turns
account: <label>
```

Se deslogado: mensagem de erro + hint de login.  
Se logado sem sessões: “sem sessões locais ainda”.

## 8. Login

```text
find_grok_bin → Command::new(bin).arg("login")
```

stdio inherited; mesmo restore/reinit de terminal que Amp/Codex.

## 9. Testes (mínimo obrigatório)

| Teste | Assert |
| --- | --- |
| `not_installed` | sem home/bin → error string NotInstalled |
| `not_logged_in_missing_auth` | home sem auth → NotLoggedIn |
| `not_logged_in_expired` | expires_at no passado → NotLoggedIn |
| `context_remaining_from_signals` | used=50k window=500k → remaining ≈ 90 |
| `picks_most_recent_session` | dois signals, mtimes diferentes → usa o novo |
| `ignores_corrupt_signals` | um JSON lixo + um ok → ainda available |
| `find_grok_bin_prefers_path` | espelho amp_cli |
| `builder_mentions_context_not_plan_quota` | tooltip contém “contexto” (ou string fixa da copy) |
| golden/waybar_contract | contagens e listas de 4 providers onde hardcodado |

Fixtures: **nunca** copiar JWT real do dev; auth de teste com
`key: "test-token"`, `expires_at` futuro fixo, `first_name: "Test"`.

## 10. Fora da v1 (backlog explícito)

- Management API / prepaid balance  
- Rate-limit tier da API  
- Usage engine USD a partir de events (events atuais não trazem
  `input_tokens` de billing de forma confiável)  
- Refresh OAuth  
- SuperGrok web message limits  
- secondary window artificial  
- Auto-inserir `grok` em settings.json já existente  

## 11. Riscos e mitigações

| Risco | Mitigação |
| --- | --- |
| `signals.json` muda de schema | serde defaults; testes de fixture; ignore unknown |
| Walk lento com milhares de sessões | limite 2000 visits + depth 16; TTL 90s |
| Confundir % contexto com cota | copy no tooltip + README |
| `WAYBAR_PROVIDERS` length 3 hardcoded em testes | atualizar todos os `3` → `4` / slices |
| Cor Cyan colide com UI accent | aceitável: marca de provider vs accent de foco; documentar |
| docs/new-provider.md cita arquivos TUI antigos | ao editar, apontar paths reais (`login_spawn`, não `list_all`) |

### 11.1 Inventário de hardcodes 3-providers (obrigatório no PR)

Locais conhecidos em 2026-07-17 (grep ao implementar; pode haver mais):

| Local | O que mudar |
| --- | --- |
| `src/config.rs` `KNOWN_PROVIDER_IDS: [&str; 3]` | `4` + `"grok"` |
| `src/waybar_contract.rs` `WAYBAR_PROVIDERS: [&str; 3]` | `4` + `"grok"` |
| `src/tui/render/login.rs` `PROVIDERS: [(&str,&str); 3]` | entrada `("grok","Grok")` |
| `src/tui/login_spawn.rs` match login | braço `"grok"` |
| `src/tui/theme_bridge.rs` loop ids | incluir `"grok"` |
| `src/settings.rs` testes default providers | expect 4 ids se assertar lista completa |
| `src/waybar_integration.rs` testes com `["claude","codex","amp"]` | incluir grok onde o teste for “todos” |
| `src/waybar_contract.rs` testes `s(&["claude","codex","amp"])` | idem |
| `src/formatters/waybar.rs` / `terminal.rs` match | braço `grok` → builder |
| `src/tui/update` / `login_selected_id` se lista fixa | alinhar com PROVIDERS |

Qualquer `assert_eq!(….len(), 3)` ligado a providers deve ser reavaliado.

## 12. Critério de pronto (DoD)

1. `cargo test providers::grok` verde  
2. `cargo test waybar_contract` e `cargo test --test golden` verdes (ou
   atualizados com intenção)  
3. `cargo clippy --all-targets -- -D warnings` limpo  
4. `agent-bar status -p grok` (com `GROK_HOME` de fixture ou real) mostra
   % ou estado deslogado sem panic  
5. README + new-provider.md atualizados  
6. **Não** mutar `~/.config/waybar` ao vivo nos testes  

## 13. Ordem de implementação sugerida (para o plano)

1. Paths + KNOWN_PROVIDER_IDS + theme + error  
2. `grok_cli` + parse auth/signals + provider + testes unitários  
3. extras + builder + waybar/terminal dispatch  
4. waybar_contract + icon + CSS  
5. login_spawn + TUI login list  
6. docs + golden adjustments + gate  

## 14. Não-objetivos de escopo (repetir)

Não é redesign de TUI, não é trilha A5, não é release acoplado (pode ir
em minor 8.2.0 quando pronto).
