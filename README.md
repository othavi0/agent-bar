<h1 align="center">Agent Bar</h1>

<p align="center">
  <img src="docs/assets/agent-bar-banner.png" alt="Banner do Agent Bar">
</p>

Monitor de quota de Claude Code, OpenAI Codex e Amp na Waybar. Quota de agente é aquele recurso que você só descobre que acabou quando acabou; aqui ela fica visível na barra o tempo todo. Um binário em Rust, sem daemon: a Waybar chama, ele imprime JSON e vai embora.

## O que ele mostra

Cada provider vira um módulo na barra com o percentual restante, colorido pelo estado: verde de 60% pra cima, amarelo entre 30 e 59, laranja entre 10 e 29, vermelho abaixo de 10. Passando o mouse, o tooltip abre as janelas de quota com horário de reset. Clique esquerdo abre a TUI num terminal à parte; clique direito força um refresh, ou o login se a sessão caiu.

De onde vem o dado, por provider:

- **Claude Code** — token OAuth de `~/.claude/.credentials.json` + endpoint de usage da Anthropic. Janela de sessão (5 h), semanal (7 d), limites semanais por modelo e o gasto extra do plano, quando existe.
- **Codex** — fala JSON-RPC com `codex app-server`; se não der, cai pro parse do session log em `~/.codex/sessions`.
- **Amp** — roda `amp usage` e parseia o texto com regex, porque `--json` não existe. É o que tem.

Tudo passa por um cache em disco (`~/.cache/agent-bar/`): 5 minutos pro Claude, 90 segundos pra Codex e Amp. A barra consulta a cada 2 minutos e ninguém martela API.

## Requisitos

- Linux x86_64 com Waybar. Uso no Hyprland, mas não tem nada específico dele aqui.
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
| `agent-bar status` | Quotas de todos os providers no terminal, sem TUI. Com `-r`, ignora o cache. |
| `agent-bar menu` | A TUI: overview com gauges e sparklines, detalhe por provider, histórico de uso (24 h/7 d), login e config da Waybar. Mouse funciona. |
| `agent-bar setup` | (Re)aplica a integração: assets, patches na config da Waybar, symlink, reload. Pergunta antes. |
| `agent-bar update` | Atualiza a instalação (detalhes acima). |
| `agent-bar uninstall` | Remove binário, assets da Waybar, settings e cache, e reverte os patches na config. Pede confirmação. |
| `agent-bar remove` | O mesmo, sem perguntar. |
| `agent-bar doctor` | Caça sobras da era npm do projeto no `$HOME` (`package.json`, `node_modules`, lockfiles) e limpa. `--dry-run` só lista, `--yes` não pergunta. |
| `agent-bar assets install` | Só copia ícones e helper, sem tocar na config. Aceita `--waybar-dir` e `--scripts-dir`. |
| `agent-bar export waybar-modules` | Imprime o contrato JSON dos módulos, pra quem prefere fiação manual. |
| `agent-bar export waybar-css` | O mesmo pro CSS. |
| `agent-bar help` | Todos os comandos e flags. |
| `agent-bar --version` | Versão instalada. |

Flags úteis no dia a dia: `--provider <id>` (`-p`) limita a um provider, `--refresh` (`-r`) invalida o cache antes de buscar, `--verbose` (`-v`) liga o debug no stderr sem sujar o stdout que a Waybar parseia.

## Outras barras (Quickshell, Eww, Ironbar)

A Waybar é o alvo, mas o dado sai em JSON puro pra qualquer consumidor:

```bash
agent-bar --format json    # snapshot de todos os providers, sem markup Pango
agent-bar --watch          # stream NDJSON, um objeto por linha (piso de 60 s; ajuste com --interval)
```

O schema é versionado (`schemaVersion: 1`) e está descrito, com exemplo de Quickshell, em [docs/json-output.md](docs/json-output.md).

## Desinstalação

`agent-bar uninstall` mostra a lista do que vai remover e pede confirmação; `agent-bar remove` faz o mesmo sem perguntar. Os dois revertem os patches na config da Waybar. Os backups `.agent-bar-backup` ficam, caso você queira comparar depois.

## Stack

Rust 2021 (MSRV 1.88), tokio + reqwest/rustls no fetch, ratatui na TUI, serde no resto. Release compilada com `opt-level = "z"`, LTO e strip: o binário musl sai pequeno e sem dependência de sistema. Testes com insta (snapshots), wiremock (HTTP) e assert_cmd (CLI).

## Docs

- [Índice](docs/README.md)
- [Arquitetura](docs/architecture.md) — como um poll vira módulo renderizado
- [Comandos](docs/commands.md) — a versão longa da tabela acima
- [Runtime](docs/runtime.md) — quais paths o agent-bar considera dele
- [Integração com a Waybar](docs/integration.md)
- [Contrato Waybar](docs/waybar-contract.md) — module IDs, CSS, refresh por signal
- [Saída JSON](docs/json-output.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Novo provider](docs/new-provider.md)
- [CHANGELOG](CHANGELOG.md)

## Licença

[MIT](LICENSE).
