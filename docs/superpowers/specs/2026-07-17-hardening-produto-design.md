# Spec — Hardening de produto (trilha B)

Data: 2026-07-17 · Base: `origin/master` pós-merge PR #11  
Origem: backlog §10 da spec fundações-confiança + rodadas impeccable.

## Objetivo

Endurecer legibilidade (contraste), consistência dos rótulos de tokens,
overflow de legenda do chart, revalidação de pricing, e fallback Waybar
visível em falha de serialize.

## Escopo

| Item | Mudança |
| --- | --- |
| B1 Contraste | `Comment`, `Red`, `Series1..6` ≥ 4.5:1 sobre `Bg` #282c34 |
| B2 Tokens dual | Rótulos "N tok": principal = input+output; sufixo cache se >0 |
| B3 Legenda chart | Séries omitidas por largura → indicador `…+N` |
| B4 Pricing | Revalidar tabela oficial (2026-07-17); bump data no módulo |
| B5 Waybar err | `waybar_error_payload.class` = `agent-bar disconnected` (visível) |

## Não-objetivos

Trilha C (labels Config humanos, chart Min height, help discoverability).
Split A5. Novos providers.

## Invariantes

CLAUDE.md hard rules. Charts/gauges de intensidade continuam cache-inclusive
no *plot*; só os *rótulos textuais* de totais usam dual. Snapshots TUI podem
mudar se o texto dos totais/legendas mudar — atualizar com intenção.

## Decisões de valor

- Comment: `#8b95a5` (~4.63:1)
- Red: `#e88b93` (~5.70:1)
- Series: levantar cada slot abaixo de 4.5 para o menor bump que passe AA
- Dual: `fmt_tokens_dual(io, cache)` → `"9,9M"` ou `"9,9M (+1,4B cache)"`
- Waybar: `class: "agent-bar disconnected"`, `alt: disconnected`, `text: err`
