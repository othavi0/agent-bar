# Design — `agent-bar update` com suporte a npm/Bun

Data: 2026-05-19
Status: aprovado, pronto para plano de implementação

## Problema

`agent-bar update` só atualiza o checkout git legado `~/.agent-bar`
(`git fetch` + `git reset --hard`). Para quem instalou via `bun add -g`
— o caminho de instalação primário — não existe comando de atualização.
O usuário precisa lembrar e digitar manualmente:

```bash
bun add -g @noctuacore/agent-bar && agent-bar setup
```

Esse contorno está documentado em 5 lugares (`README.md`, `docs/commands.md`,
`docs/runtime.md`, `docs/integration.md`, `docs/troubleshooting.md`), todos
repetindo que "`update` não é o atualizador do pacote npm". O comando chamado
`update` não atualiza a instalação real da maioria dos usuários.

## Objetivo

`agent-bar update` deve detectar o tipo de instalação e atualizar a
instalação correta: o pacote npm/Bun para installs globais, ou o checkout
git para o fluxo legado `~/.agent-bar`.

## Decisões

| # | Decisão | Escolha |
|---|---------|---------|
| 1 | Comportamento para installs npm | Auto-detectar os dois modos num único comando |
| 2 | Checagem de versão no registry | Não — sempre rodar `bun add -g`, o Bun resolve |
| 3 | Confirmação no fluxo npm | Sim — pedir confirmação, por consistência com o fluxo git |
| 4 | Checkout git de desenvolvimento | Recusar com instrução para usar `git pull` manual |

## Detecção do tipo de instalação

`update` classifica a instalação em 3 casos, checando a presença de `.git`
**diretamente em `REPO_ROOT`** (`existsSync(join(REPO_ROOT, '.git'))`), não
via `git rev-parse --git-dir` — este último sobe a árvore de diretórios e
poderia encontrar um repositório ancestral por engano.

| Caso (`InstallKind`) | Condição | Ação |
|----------------------|----------|------|
| `managed-git` | `.git` existe em `REPO_ROOT` **e** `REPO_ROOT === ~/.agent-bar` | Fluxo git atual, inalterado |
| `dev-git` | `.git` existe em `REPO_ROOT` mas `REPO_ROOT !== ~/.agent-bar` | Recusa, orienta `git pull` manual |
| `npm` | Sem `.git` em `REPO_ROOT` | Novo fluxo npm/Bun |

Quando instalado por `bun add -g`, o pacote vive no diretório global do Bun,
sem `.git` — daí o caso `npm`.

## Fluxo npm

1. Lê `name` e `version` do `package.json` local (em `REPO_ROOT`).
2. Mostra um resumo: nome do pacote, versão instalada, e o que será executado.
3. Pede confirmação ao usuário (mesmo padrão de UX do fluxo git).
4. Se confirmado, roda `bun add -g @noctuacore/agent-bar` — sempre, sem
   consultar o registry; o Bun resolve se há versão nova.
5. Roda `setup` com `confirm: false`, `clearScreen: false` para reaplicar a
   integração Waybar.
6. Emite outro de sucesso.

Se o usuário cancelar na confirmação, `update` encerra sem efeito.

## Fluxo dev-git

Substitui a mensagem atual de `wrong-root`. Quando `REPO_ROOT` é um checkout
git que não é `~/.agent-bar`, `update` recusa e instrui o usuário a atualizar
com `git pull` (ou os comandos git normais) — nunca roda `reset --hard` nem
`bun add -g` num repositório de trabalho.

## Estrutura de código (`src/update.ts`)

- Novo tipo `InstallKind = 'managed-git' | 'dev-git' | 'npm'`.
- Nova função de topo `runUpdate(...)` que detecta o `InstallKind` e roteia
  para o fluxo correspondente.
- `runManagedUpdate` permanece como está, atendendo o caso `managed-git`.
- Nova função `runNpmUpdate(...)` para o fluxo npm. Reusa os hooks já
  injetáveis (`runCommand`, `confirm`, `runSetup`, `onEvent`), mantendo o
  módulo testável sem tocar no sistema real.
- `InstallKind` injetável via options para os testes forçarem cada caso sem
  depender do filesystem.
- `main()` passa a tratar os status dos 3 casos. A mensagem do caso `dev-git`
  substitui a de `wrong-root`.

A confirmação do fluxo npm reusa a forma do callback `confirm` já existente,
adaptada para receber um resumo do tipo npm (nome + versão do pacote) em vez
de `UpdateSummary` do fluxo git. O contrato exato dos tipos fica a cargo do
plano de implementação.

## Arquivos afetados

| Arquivo | Mudança |
|---------|---------|
| `src/update.ts` | Detecção de `InstallKind`, `runUpdate`, `runNpmUpdate` |
| `tests/update.test.ts` | Testes do fluxo npm e da detecção dos 3 casos |
| `README.md` | Reescrever a seção de update (linhas ~29-34, ~57-59) |
| `docs/commands.md` | Atualizar tabela e seção "Update Behavior" |
| `docs/runtime.md` | Atualizar seção "Package Install" |
| `docs/integration.md` | Atualizar seção "Update" |
| `docs/troubleshooting.md` | Atualizar "Update Refuses To Run" |
| `AGENTS.md` | Linha ~121 — descrição de `src/update.ts` |
| `src/cli.ts` | Linha ~73 — texto de help do comando `update` |
| `CHANGELOG.md` | Entrada nova |

## Versão

Feature nova → release `4.1.0` separado. O `4.0.1` (já preparado, ainda não
publicado no npm por causa do 2FA) deve ser publicado primeiro. O bump de
versão é tarefa do passo de finalização, não desta implementação.

## Testes

- Detecção: `managed-git`, `dev-git` e `npm` retornam o roteamento correto.
- Fluxo npm: confirmação aceita roda `bun add -g` + `setup`; confirmação
  recusada encerra sem efeito.
- Fluxo dev-git: recusa com a mensagem de orientação.
- Fluxo `managed-git`: testes existentes continuam passando inalterados.

## Fora de escopo

- Checagem de versão contra o registry npm (decisão 2).
- Atualização automática agendada/em background.
- Auto-update do checkout `dev-git`.
