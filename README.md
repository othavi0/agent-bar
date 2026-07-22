<h1 align="center">Agent Bar</h1>

<p align="center">
  <img src="docs/assets/agent-bar-banner.png" alt="Banner do Agent Bar">
</p>

Monitor de quota de Claude Code, OpenAI Codex, Amp e Grok Build — nativo no Omarchy 4 (omarchy-shell), com Waybar suportada como tier legado. Quota de agente é aquele recurso que você só descobre que acabou quando acabou; aqui ela fica visível na barra o tempo todo. Um binário em Rust, sem daemon: o shell chama, ele imprime JSON e vai embora.

## O que ele mostra

Cada provider vira um módulo na barra com o percentual restante, colorido pelo estado: verde de 60% pra cima, amarelo entre 30 e 59, laranja entre 10 e 29, vermelho abaixo de 10. Passando o mouse, o tooltip abre as janelas de quota com horário de reset.

Na **Waybar**, clique esquerdo abre a TUI num terminal à parte; clique direito foca o provider (detail ou login se a sessão caiu). No **Omarchy 4** (omarchy-shell): esquerdo = popup de usage, direito = settings nativo no mesmo popup, meio = refresh. Dashboard completo (charts, histórico, login): `agent-bar menu` ou o link no rodapé do popup.

De onde vem o dado, por provider:

- **Claude Code** — token OAuth de `~/.claude/.credentials.json` + endpoint de usage da Anthropic. Janela de sessão (5 h), semanal (7 d), limites semanais por modelo e o gasto extra do plano, quando existe.
- **Codex** — fala JSON-RPC com `codex app-server`; se não der, cai pro parse do session log em `~/.codex/sessions`.
- **Amp** — roda `amp usage` e parseia o texto com regex, porque `--json` não existe. É o que tem.
- **Grok Build** — OAuth em `~/.grok/auth.json` + `signals.json` das sessões.
  O % da barra é **contexto restante da sessão recente**, não cota de plano xAI.

Tudo passa por um cache em disco (`~/.cache/agent-bar/`): 5 minutos pro Claude, 90 segundos pra Codex, Amp e Grok. A barra consulta a cada 2 minutos e ninguém martela API.

## Requisitos

- Linux x86_64. **Omarchy 4 (omarchy-shell)** é o alvo principal: `agent-bar setup` detecta o omarchy-shell e instala o bar-widget plugin nativo (chips + popup) automaticamente.
- **Waybar** — suportada como **tier legado**: funciona, recebe fix, não recebe feature nova. `agent-bar setup` integra com ela também, quando presente.
- `curl`, `tar` e `sha256sum` pro instalador.
- Um terminal que o helper reconheça: alacritty, kitty, foot, ghostty, wezterm, ou `xdg-terminal-exec`.
- As CLIs que você quer monitorar, instaladas. O login dá pra fazer pela própria TUI.
- `libnotify`, se quiser notificação de quota baixa. Opcional.

## Instalação

Ainda não tem pacote no AUR. O PKGBUILD já está pronto no repo e o `agent-bar-bin` sai em breve; por enquanto o caminho é o instalador:

```bash
curl -fsSL https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh | bash
```

O que ele faz, na ordem:

1. Resolve a última release no GitHub e baixa o tarball (binário estático, musl).
2. Confere o sha256. Não bateu, aborta.
3. Instala o binário em `~/.local/bin/agent-bar` e os assets (ícones, helper de terminal) em `~/.local/share/agent-bar`.
4. Rodando via pipe ele não consegue te perguntar nada, então para aí e te lembra do próximo passo.

O próximo passo é a integração com a Waybar:

```bash
agent-bar setup
```

O `setup` copia ícones e CSS pra `~/.config/waybar/agent-bar/`, insere um include no seu `config.jsonc`, um `@import` no seu `style.css` e recarrega a Waybar. Antes de tocar em qualquer arquivo seu, ele salva uma cópia `.agent-bar-backup` do lado. E o patch é feito em cima do texto, não round-trip de serializador: seus comentários no `config.jsonc` sobrevivem.

Pra fechar, logue nos providers: `agent-bar menu`, tela **Login**.

### cargo-binstall

O crate ainda não está no crates.io, mas o binstall resolve direto do repositório:

```bash
cargo binstall --git https://github.com/othavi0/agent-bar agent-bar
agent-bar setup
```

### Pelo código

`git clone`, `cargo build`, `./target/debug/agent-bar setup`. O symlink passa a apontar pro binário de debug e cada rebuild aparece no próximo tick da barra. Detalhes no [CONTRIBUTING.md](CONTRIBUTING.md).

## Atualização

```bash
agent-bar update
```

Ele descobre como você instalou e age de acordo. Instalação pelo instalador: self-update, baixando a release nova, conferindo o sha256 e trocando o binário de forma atômica. Checkout de desenvolvimento: recusa e manda usar `git pull`. Pacote de sistema: te aponta pro gerenciador de pacotes.

## Comandos

| Comando | O que faz |
| --- | --- |
| `agent-bar` | Chamado pela Waybar (sem TTY), imprime o JSON do módulo. Num terminal, abre a TUI. |
| `agent-bar status` | Quotas de todos os providers no terminal, sem TUI. Com `-r`, ignora o cache. `-t` / `--terminal` é alias. |
| `agent-bar menu` | A TUI (dashboard): detalhe do primeiro provider habilitado, histórico, login e Config. |
| `agent-bar config show` | Imprime o subset editável de settings (JSON). |
| `agent-bar config apply` | Aplica patch JSON em settings (`--json` / `--file`). Não recarrega a Waybar. |
| `agent-bar setup` | (Re)aplica a integração: assets, Waybar e/ou plugin Omarchy, symlink, reload. Pergunta antes. |
| `agent-bar update` | Atualiza a instalação (detalhes acima). |
| `agent-bar uninstall` | Remove binário, assets, settings e cache, e reverte patches. Pede confirmação. |
| `agent-bar doctor` | Caça sobras da era npm no `$HOME` e limpa. `--dry-run` só lista, `--yes` não pergunta. |
| `agent-bar help` | Comandos e flags públicos (internos de packager/Waybar ficam ocultos). |
| `agent-bar --version` | Versão instalada. |

Flags úteis no dia a dia: `--provider <id>` (`-p`) limita a um provider, `--refresh` (`-r`) invalida o cache antes de buscar, `--verbose` (`-v`) liga o debug no stderr sem sujar o stdout que a Waybar parseia. Superfície completa em [docs/commands.md](docs/commands.md).

## Outras barras (Quickshell, Eww, Ironbar)

A Waybar é o alvo, mas o dado sai em JSON puro pra qualquer consumidor:

```bash
agent-bar --format json    # snapshot de todos os providers, sem markup Pango
agent-bar --watch          # stream NDJSON, um objeto por linha (piso de 60 s; ajuste com --interval)
```

O schema é versionado (`schemaVersion: 1`) e está descrito, com exemplo de Quickshell, em [docs/json-output.md](docs/json-output.md).

## Desinstalação

`agent-bar uninstall` mostra a lista do que vai remover e pede confirmação; `agent-bar remove` é alias forçado (`uninstall --yes`). Os dois revertem os patches na config da Waybar e desregistram o plugin Omarchy quando existir. Os backups `.agent-bar-backup` ficam, caso você queira comparar depois.

## Stack

Rust 2021 (MSRV 1.88), tokio + reqwest/rustls no fetch, ratatui na TUI, serde no resto. Release compilada com `opt-level = "z"`, LTO e strip: o binário musl sai pequeno e sem dependência de sistema. Testes com insta (snapshots), wiremock (HTTP) e assert_cmd (CLI).

## Docs

- [Índice](docs/README.md)
- [Arquitetura](docs/architecture.md) — como um poll vira módulo renderizado
- [Comandos](docs/commands.md) — a versão longa da tabela acima
- [Runtime](docs/runtime.md) — quais paths o agent-bar considera dele
- [Integração com a Waybar](docs/integration.md)
- [Omarchy shell](docs/omarchy-shell.md) — plugin, clicks, settings nativo
- [Contrato Waybar](docs/waybar-contract.md) — module IDs, CSS, refresh por signal
- [Saída JSON](docs/json-output.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Novo provider](docs/new-provider.md)
- [CHANGELOG](CHANGELOG.md)

## Licença

[MIT](LICENSE).
