# Spec — Números confiáveis, cache persistente e estados de fetch

Data: 2026-07-03 · Origem: triagem dos problemas detectados na sessão de
ajustes visuais (pós-v7.1.0). Decisões tomadas com o dono via brainstorming.

## Contexto e problema

A leva visual da 7.1.0 expôs quatro grupos de problemas:

1. **Números não confiáveis.** (a) O parser do Claude soma toda linha
   `assistant` do JSONL, mas o Claude Code escreve várias entradas por
   request durante o streaming (mesmo `requestId`, `output_tokens`
   crescendo) — contamos a mesma request várias vezes
   (ref: claude-devtools#74). (b) O preço de cache write usa multiplicador
   único, mas 5m custa 1.25× e 1h custa 2× o input base (ref: ccusage#899);
   o JSONL real já traz o breakdown `usage.cache_creation.ephemeral_5m/1h`.
   (c) Detail soma cache nos rótulos "N tok" (1.4B) enquanto History/painel
   somam input+output (~10M) — telas se contradizem. (d) "Hoje" corta na
   meia-noite local, mas a tabela do History bucketiza por data UTC —
   painel 7.6M vs tabela 10.0M pro mesmo dia.
2. **Cold load lento.** Parse dos ~2100 JSONL leva 10-20s por processo do
   menu (já foi 2×, reduzido a 1× na 7.1.0). O TODO de persistir o índice
   está documentado em `usage/cache.rs` desde a criação.
3. **Estados de fetch instáveis.** Sidebar/Overview crescem conforme
   `ProviderFetched` chega (itens pulam de posição); header degrada pra
   `⠽ · -` durante refresh; Codex mostra `! erro` na máquina do dono
   (causa não diagnosticada).
4. **Miudezas.** Help popup corta em <30 linhas; History carregado-vazio
   deixa ~24 linhas de buraco; flake de teste por mutação de PATH; actions
   do CI com deprecation de Node 20; URLs `othavioquiliao` pós-rename.

## Decisões do dono (fechadas, não reabrir)

- **Rótulo duplo de tokens**: principal = input+output; sufixo de cache
  onde couber. Ex.: `9.9M (+1.4B cache)`. Vale pra TODOS os rótulos "N tok"
  (Detail, History, painel Overview). Charts/sparklines continuam
  cache-inclusive (intensidade visual, sem número).
- **Fronteira de dia**: meia-noite **local** em tudo ("hoje" e buckets da
  tabela/rodapé do History).
- **Cache persistente**: `redb` (KV embedded puro Rust, estável) + valores
  `postcard` (serde, compacto, seguro). Rejeitados: JSON único (reescrita
  integral a cada save), rkyv (unsafe/acoplado a layout, velocidade
  desnecessária), sled (alpha estagnado), fjall (dev encerrando).
- **Dedup de streaming + preço 5m/1h entram no escopo** — sem eles a
  unificação só deixa os números consistentemente errados.
- **Preços de modelos atualizados** com valores oficiais vigentes,
  verificados online na implementação (nunca de memória).

## Design

### S1 — Números confiáveis

**Dedup de streaming (`usage/claude.rs`).** `parse_claude_lines` agrupa
por `requestId` (fallback: `message.id`; sem ambos, a linha vale sozinha)
e emite UM `UsageRecord` por request: a última entrada vista (ordem do
arquivo; empate resolvido por maior `output_tokens`). Dedup é por arquivo —
requests não cruzam arquivos de sessão. Teste com fixture de streaming
real (2+ entradas do mesmo requestId, tokens crescendo) assertando que só
a final conta.

**Preço de cache write (`usage/pricing.rs`).** O parser passa a extrair o
breakdown `usage.cache_creation.ephemeral_5m_input_tokens` /
`ephemeral_1h_input_tokens` quando presente (novo(s) campo(s) em
`UsageRecord`); o custo usa 1.25× pro tier 5m e 2× pro 1h. Sem breakdown
(logs antigos): fallback documentado = tratar `cache_creation_input_tokens`
inteiro como 5m (comportamento atual, conservador). Preços por modelo
atualizados da tabela oficial da Anthropic/OpenAI no momento da
implementação, com comentário de fonte+data na tabela.

**Rótulo duplo (`tui/render/shared.rs`).** Helper único
`fmt_tokens_dual(io: u64, cache: u64) -> String` → `"9.9M (+1.4B cache)"`;
cache 0 → só `"9.9M"`. Aplicado em: totais do Detail (hoje/7 dias e
por-modelo), tabela + rodapé "Total 7d" do History, rodapé do painel
"Hoje (24h)". Em larguras apertadas onde o sufixo não cabe (tabela do
History em terminal estreito), o sufixo é dropado — o número principal
nunca é.

**Dia local (`usage/buckets.rs`).** `bucket_by_day` e
`bucket_by_provider_day` ganham parâmetro `local_offset: time::UtcOffset`
e bucketizam por `ts.to_offset(local_offset).date()`. Callers passam
`state.local_offset`. Testes cobrindo record às 02:00 UTC com offset -3
caindo no dia anterior.

### S2 — Cache persistente de parse

`usage/cache.rs` evolui de índice em memória para redb:

- Arquivo: `<cache_dir>/usage.redb` (mesma resolução de cache dir já usada
  pelo app; testes usam temp dir via paths injetados).
- Tabela `files`: chave = path canônico (str); valor = postcard de
  `(size: u64, mtime: i64, records: Vec<UsageRecord>)`.
- Chave meta `version`: const no código; formato mudou → bump → tabela é
  dropada e reconstruída (a 1ª run pós-dedup re-parseia tudo e corrige o
  histórico inteiro).
- Fluxo por arquivo: stat → `(size, mtime)` bate com o armazenado →
  decode; não bate/ausente → parse + upsert daquele path só.
- GC ao fim do refresh: remove chaves cujo path não existe mais.
- Corrupção/erro de abertura do redb: deletar o arquivo e reconstruir
  (cache é derivado, nunca fonte de verdade). Nunca panicar por causa dele.
- A camada em memória atual permanece como L1 dentro de um mesmo processo.

Deps novas: `redb`, `postcard` (+`serde` features) — ambas puro Rust,
compatíveis com o build musl estático do publish.yml.

### S3 — Estados de fetch/erro

**Sidebar estável.** No boot, `state.providers` é semeado com um slot por
provider habilitado em `settings.waybar.providers`, na ordem configurada,
em estado skeleton/pending. `ProviderFetched` substitui o slot in-place
(mesmo índice). Nenhum item muda de posição durante o fetch inicial.
Cursor/hit-testing continuam válidos (índices estáveis por construção).

**Header persistente.** `header_status` mantém o último custo e horário
conhecidos durante refresh (spinner aparece ao lado); `-` só quando nunca
houve load no processo. (O custo já se comporta assim via `display_cost`;
o fix é principalmente o relógio/estado inicial.)

**Codex `! erro`.** Task de diagnóstico ANTES de qualquer fix: reproduzir
na máquina do dono, capturar o erro verbatim do provider e tratar a causa
real (hipótese inicial, não confirmada: CLI do codex mudou de interface —
já aconteceu na sessão do redesign). Se for mudança de contrato do
provider, a mensagem de erro do provider é contrato de teste — atualizar
junto.

### S4 — Miudezas

- **Help popup em terminal baixo**: se o conteúdo não cabe, remover as
  linhas em branco entre seções; se ainda não couber, truncar com
  indicador final `… (+N atalhos)` — nunca corte mudo.
- **History carregado-vazio**: mensagem "sem uso…" centralizada
  verticalmente na área do chart (sem o buraco de ~24 linhas até a linha
  do Amp).
- **Flake de PATH**: identificar os testes que mutam `PATH`/env e
  serializá-los com mutex estático compartilhado (padrão já usado no repo
  para env; sem dependência nova).
- **CI**: bump `actions/checkout` e `mlugg/setup-zig` pras major atuais
  (verificar latest na implementação) — mata o warning de Node 20.
- **URLs**: `url=`/source do PKGBUILD e `.SRCINFO` → `othavi0/agent-bar`.

## Testes e verificação

- Dedup: fixture JSONL real de streaming (mesmo requestId ×3) → 1 record.
- Pricing: casos 5m-only, 1h-only, misto, sem-breakdown (fallback), modelo
  desconhecido (custo None) — asserts de valor exato.
- Buckets: offset -3 movendo record de dia; offset 0 preserva snapshots.
- Cache: round-trip; invalidação por (size,mtime); bump de versão dropa;
  arquivo corrompido reconstrói sem panic.
- Render: snapshots atualizados de propósito (rótulo duplo muda vários);
  smoke tmux 110x32 + 78x24 antes de qualquer "pronto".
- Regra RTK do repo: 1 filtro posicional por `cargo test`, confiar no exit.

## Fora de escopo

- AUR push (manual do dono, precisa da chave SSH dele).
- Mudar semântica dos charts (continuam cache-inclusive).
- Qualquer coisa de Copilot/legado (proibido; varredura já feita na 7.1.x).
- Release: esta leva NÃO corta release automaticamente; decidir ao final.

## Riscos

- Dedup + preços novos mudam números históricos exibidos (vão CAIR em
  tokens e mudar em custo) — esperado e desejado; comunicar no CHANGELOG.
- redb novo no stack: mitigado por cache-é-descartável + testes de
  corrupção.
- Snapshots em massa: revisar diffs um a um antes de aceitar (contrato).
