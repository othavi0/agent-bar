# Omarchy-shell plugin (Omarchy 4+)

O Omarchy 4 substituiu a Waybar pelo `omarchy-shell` (Quickshell). O
agent-bar se integra como bar-widget plugin de terceiro.

## O que o setup instala

`agent-bar setup` detecta o omarchy-shell (CLI `omarchy` no PATH +
`/usr/share/omarchy/shell/`) e escreve o drop-in:

```
~/.config/omarchy/plugins/agent-bar.usage/
  manifest.json          # id agent-bar.usage, version = versão do binário
  Widget.qml             # chips + popup (consome `agent-bar --format json`)
  icons/                 # ícones dos providers
  scripts/agent-bar-open-terminal
```

Depois roda `omarchy plugin rescan`, `omarchy plugin enable agent-bar.usage`
e `omarchy bar plugin add agent-bar.usage` (best-effort: falhas viram aviso
e os comandos podem ser rodados manualmente).

Se a Waybar também estiver instalada, o fluxo Waybar clássico roda junto.

## Widget

- Um chip por provider (ícone + % restante do limite primário), cores do
  tema do shell, severidade espelhando o TUI (≥60 ok / 30-59 / 10-29 / <10).
- Clique esquerdo: popup nativo (janelas primária/secundária, breakdown por
  modelo, reset, plan/conta).
- Clique direito: abre o TUI (`agent-bar menu`) em terminal flutuante.
- Clique do meio: refresh forçado (`--refresh`).
- Setting `refreshIntervalSec` (default 60, min 30) via
  `omarchy bar plugin set` ou `shell.json`.

## Dados

O QML roda `agent-bar --format json` (contrato em
[`json-output.md`](json-output.md)). Os arquivos QML são embutidos no
binário — version-locked com o schema. Após `agent-bar update`, re-rode
`agent-bar setup` para atualizar o drop-in (o update imprime esse hint).

## Remoção

`agent-bar uninstall`/`remove` desregistra o widget
(`omarchy bar plugin remove` + `omarchy plugin remove`, best-effort) e
apaga o diretório do plugin.

## Testes

Fluxo coberto por `cargo test omarchy_integration` e `cargo test setup`
com temp dirs (`--omarchy-plugins-dir`). O QML não tem harness
automatizado: mudanças visuais exigem verificação manual no desktop.
