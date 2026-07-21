# Documentation

Operational docs for `agent-bar`.

## User Docs

- [Commands](commands.md) — public CLI surface and flags.
- [Runtime](runtime.md) — files, settings, cache, provider credentials.
- [Integration](integration.md) — setup, update, removal, and Waybar ownership.
- [Omarchy shell](omarchy-shell.md) — plugin `agent-bar.usage` (Omarchy 4+).
- [Troubleshooting](troubleshooting.md) — common runtime and Waybar failures.

## Developer Docs

- [Architecture](architecture.md) — data flow, provider layer, the two caches, formatters.
- [Waybar contract](waybar-contract.md) — generated modules, CSS, assets, and classes.
- [New provider guide](new-provider.md) — provider implementation checklist.
- [JSON output](json-output.md) — `--format json` / `--watch` contract for non-Waybar bars (Quickshell, Eww).
- [ADRs](adr/README.md) — decisões de arquitetura (settings Omarchy, dual-write, CLI).
- [`CONTEXT.md`](../CONTEXT.md) (raiz) — glossário do domínio.

## Superpowers (spec / plano / notas de entrega)

Não são contrato operacional do dia a dia; servem a agentes e a histórico de
design. A fonte de verdade do runtime continua em `src/` + docs acima.

- Spec 8.5.0: `superpowers/specs/2026-07-21-omarchy-settings-and-cli-simplify-design.md`
- Plano 8.5.0: `superpowers/plans/2026-07-21-omarchy-settings-and-cli-simplify.md`
- Nota de entrega: `superpowers/notes/2026-07-21-omarchy-settings-entrega.md`

## Source Of Truth

Runtime behavior lives in `src/`. Operational docs describe the current
contract. Specs/plans under `superpowers/` record how features were designed;
ADRs record why hard choices stuck.
