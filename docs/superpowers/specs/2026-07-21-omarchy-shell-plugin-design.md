# Suporte ao omarchy-shell (Omarchy 4) — plugin nativo

Data: 2026-07-21 · Status: aprovado em brainstorming

## Contexto

O Omarchy 4 (`4.0.0.alpha`) substituiu a Waybar pelo **omarchy-shell**, um
shell Quickshell (QML/Qt) próprio, com bar plugin-based:

- Layout do bar vive em `~/.config/omarchy/shell.json`.
- Widgets são plugins com `manifest.json`; terceiros vão em
  `~/.config/omarchy/plugins/<id>/` (drop-in manual ou `omarchy plugin add
  <git-url>`), gerenciados por `omarchy plugin rescan/enable` e
  `omarchy bar plugin add`.
- Existe um widget first-party `omarchy.model-usage` (Claude + Codex, popup
  nativo, scanners Python). Não cobre Amp nem Grok, nem o TUI/notify/doctor
  do agent-bar.
- O bar aceita módulos `type: "command"` com JSON estilo Waybar, mas
  degradado: só `text`/`tooltip`/`class=="active"`, sem Pango, sem cores por
  classe. Não serve como integração de produto.

O agent-bar já expõe `agent-bar --format json` (envelope `schemaVersion: 1`,
documentado em `docs/json-output.md`), feito exatamente para bars que
renderizam nativo. A camada de dados está pronta; o trabalho é apresentação
(QML) + integração (setup/update/uninstall).

## Decisões (aprovadas com o usuário)

1. **Plugin QML nativo de terceiro** — não ponte `command`, não PR upstream
   no `model-usage`. O Rust continua fonte única de dados; QML é casca fina.
2. **Widget único com chips** — um slot no bar com um chip por provider
   (glifo + % primary), popup único. Idioma igual ao `model-usage`.
3. **Popup = paridade com o tooltip rico da Waybar** — barras
   primary/secondary, breakdown por modelo, reset times, plan/conta,
   severidade. Análise funda (charts, sessões/dia) continua no TUI.

## Arquitetura

### Plugin

- Id: `agent-bar.usage` (namespace obrigatório; prefixo `omarchy.*` é
  reservado). Diretório de instalação: `~/.config/omarchy/plugins/agent-bar.usage/`.
- Arquivos: `manifest.json` + `Widget.qml` (popup incluso; componentes extras
  só se o arquivo passar de ~800 linhas).
- Manifest: `schemaVersion: 1`, `kinds: ["bar-widget"]`,
  `entryPoints.barWidget: "Widget.qml"`, `barWidget.allowMultiple: false`,
  `version` = `CARGO_PKG_VERSION` do binário gerador.
- Schema de settings do manifest: apenas `refreshIntervalSec` (integer,
  default 60, min 30, max 3600). Quais providers aparecem continua sendo
  decisão do `settings.json` do agent-bar (fonte única, sem duplicação).

### Distribuição: drop-in embutido no binário

Os arquivos do plugin são embutidos no binário Rust via `include_str!`
(mesmo padrão dos assets Waybar em `waybar_integration.rs`), e o `setup`
os escreve como drop-in. Razões contra o caminho `omarchy plugin add
<git-url>`:

- Distribuição única — AUR/install.sh/binstall já entregam o binário.
- QML version-locked com o schema JSON do binário (sem skew).
- Funciona offline; `agent-bar update` reescreve o plugin junto do binário.

### Setup / update / uninstall

- `agent-bar setup` detecta o bar disponível:
  - Waybar presente → fluxo atual, inalterado.
  - omarchy-shell presente (CLI `omarchy` no PATH **e**
    `/usr/share/omarchy/shell/` existente) → instala o drop-in e roda
    `omarchy plugin rescan`, `omarchy plugin enable agent-bar.usage`,
    `omarchy bar plugin add agent-bar.usage` (modo não-interativo `--yes`).
  - Ambos presentes → perguntar (ou ambos com `--yes`).
- Flag nova `--omarchy-plugins-dir <path>` (par do `--waybar-dir`): injeta o
  path de instalação para testes e dry-run. `--dry-run` imprime o que faria.
- `agent-bar update`: se o drop-in existir, reescreve `manifest.json` +
  `Widget.qml` com a versão nova.
- `uninstall`/`remove`: remove o diretório do plugin e a entrada do layout
  (`omarchy bar plugin remove` + `omarchy plugin remove`, com fallback de
  apagar o diretório se o CLI não estiver disponível).
- Constantes novas em `src/app_identity.rs`: `OMARCHY_PLUGIN_ID`,
  `OMARCHY_PLUGINS_SUBDIR` (e o que mais o código precisar) — sem strings
  hardcoded espalhadas.

## Dados (Widget.qml → binário)

- `Process` do Quickshell rodando `agent-bar --format json` one-shot,
  disparado por `Timer` com `refreshIntervalSec` (default 60s,
  `triggeredOnStart: true`). Mesmo padrão do `model-usage` first-party.
- Sem `--watch`/NDJSON no v1 — processo persistente dentro do quickshell é
  complexidade sem retorno agora; fica como evolução possível.
- Parse do envelope `schemaVersion: 1`. Campos usados: `providers[]` com
  `provider`, `displayName`, `available`, `error`, `plan`, `account`,
  `primary`/`secondary` (`used`, `remaining`, `resetsAt`, `severity`),
  `models`, `staleReason`.
- Localização do binário: `agent-bar` resolvido pelo PATH do login shell
  (o bar roda comandos via `bash -lc`, mesmo mecanismo dos módulos command).
- Resiliência: JSON inválido/processo falho → mantém último estado bom e
  marca stale visualmente (opacidade reduzida); nunca crashar o shell.

## Widget: chips + popup

### Chips no bar

- Um chip por provider retornado no envelope: glifo do provider + percentual
  do `primary.remaining` (inteiro).
- Cores do tema do shell — `bar.foreground`, `Color.accent`, `bar.urgent` —
  nada de paleta própria; o widget acompanha o tema Omarchy ao vivo.
- Severidade: `normal` → foreground; `warning`/`critical` → cor de alerta
  (mapeada de `Color`/`bar.urgent`).
- `available: false` → chip discreto (glifo esmaecido, sem %); `staleReason`
  presente → opacidade reduzida.
- Barra vertical (`bar.vertical`): forma compacta só-glifo, como os widgets
  first-party fazem.

### Popup (clique esquerdo)

Paridade com o tooltip rico da Waybar, por provider:

- Header: displayName + plan/conta.
- Barras de progresso primary/secondary com % e reset time formatado.
- Breakdown por modelo (`models`), com severidade por cor.
- Extras relevantes do envelope quando existirem (ex.: extra usage do
  Claude, créditos do Amp) — texto simples, sem chart.
- Rodapé com hint das ações (TUI / refresh).
- Coordenação de popup via `bar.requestPopout`/`bar.releasePopout` (regra
  one-popup-at-a-time do shell).

### Interações

| Ação | Comportamento |
|---|---|
| Esquerdo | popup nativo |
| Direito | abre o TUI (`agent-bar menu`) via terminal helper existente (`scripts/agent-bar-open-terminal`), com `bar.run(...)` |
| Meio | refresh forçado (`agent-bar --format json --refresh` no próximo tick imediato) |

## Testes e verificação

- **Golden**: novos golden tests dos arquivos gerados (`manifest.json`
  byte-a-byte; `Widget.qml` presença/estrutura) no padrão da suite golden
  existente.
- **Setup/integração**: temp dirs + `--omarchy-plugins-dir` + `XDG_*`;
  mock do CLI `omarchy` via seam (fn pointer/trait, padrão existente);
  nunca mutar desktop vivo em teste (hard rule).
- **Contrato Waybar**: intocado — suite atual é a regressão.
- **QML**: sem harness automatizado (risco declarado). Verificação
  perceptual manual no desktop do usuário, com aprovação explícita, ao
  final: chips renderizados + popup + screenshot lado a lado com o
  `model-usage` (as 3 provas de "pronto": funcional, perceptual, dados).

## Fora de escopo (v1)

- `--watch`/NDJSON como fonte do widget.
- Port do TUI (charts, sessões/dia) para QML.
- Repo git separado para `omarchy plugin add`.
- Settings UI própria além do schema do manifest.
- Notificações via shell (o `notify` atual continua como está).

## Docs a atualizar na implementação

- `docs/integration.md` (novo fluxo de detecção de bar).
- Novo `docs/omarchy-shell.md` ou seção equivalente (contrato do plugin).
- `README.md` (Omarchy 4 suportado).
- `CHANGELOG.md` apenas no corte de release.
