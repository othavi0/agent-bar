# Design: descoberta de executável não deve canonicalizar shims (mise)

Data: 2026-08-11
Status: aprovado (opção A)

## Problema

Com o Amp CLI instalado via mise, o único `amp` no PATH do Quickshell é o shim
`~/.local/share/mise/shims/amp`, um symlink para `/usr/bin/mise`. A descoberta
(`resolve_executable` em `src/providers/catalog.rs`) canonicaliza o candidato
encontrado, então o helper executa `/usr/bin/mise usage` em vez do shim. O mise
vê `argv[0]` = `mise`, não entra em modo shim e roda o subcomando próprio
`mise usage` (usage-spec, ~213 KB, exit 0, ~15 ms). O parser não encontra
nenhuma linha do Amp, o resultado vira `Ready` com `windows: []` e a UI mostra
"Amp reports no quota / This account is billed another way."

Reproduzido na máquina do usuário em 2026-08-11 com o ambiente exato do
Quickshell. Testes decisivos:

- `amp` → symlink para `/usr/bin/mise` no PATH: coleta falha (`ready`, 0
  janelas, 33 ms).
- `amp` → symlink para o binário Node real: coleta funciona (`ready`, 3
  janelas, ~1,2 s).

## Alcance do defeito

- Coleta do Amp (`amp usage`): quebrada quando instalado via mise.
- Codex app-server: mesma falha; o adapter cai no fallback de session log e
  serve dados velhos.
- `login` de qualquer provider via mise: executaria `/usr/bin/mise login`.
- Claude não é afetado na coleta (HTTP/OAuth). Grok lê JSON do disco na coleta,
  mas o `login` seria afetado.

## Correção (opção A, aprovada)

`resolve_executable` retorna o caminho encontrado **sem** seguir symlinks, nos
dois ramos (PATH e fallbacks). Executar o path que o PATH resolveu preserva o
`argv[0]` do shim e ativa o modo shim do mise — comportamento POSIX padrão.
`canonicalize_best_effort` deixa de ser usado na resolução (remover se ficar
sem uso).

Alternativas rejeitadas:

- Canonicalizar apenas quando o basename do alvo coincide: branch extra sem
  benefício, já que nada exige paths canônicos na execução.
- Forçar `argv[0]` via `CommandExt::arg0` mantendo o path canônico: frágil,
  depende de detalhe interno do mise e exige estender `ProcessSpec`.

## Testes

- Regressão em `catalog.rs`: num diretório temporário no PATH, `amp` como
  symlink para outro binário de basename diferente (simulando o shim); a
  descoberta deve retornar o path do symlink, não o alvo.
- Atualizar o teste existente que espera o path canonicalizado
  (`fs::canonicalize` em `catalog.rs`, ~linha 517).
- Gates de contrato: `cargo fmt --check`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings`.

## Fora de escopo

- Investigar por que o Grok reporta `windows: []` (fluxo distinto, lê billing
  JSON do disco).
- Atualizar o bundle instalado em `~/.config/omarchy/plugins` — segue o gate
  de QA final definido em `CLAUDE.md`.
