# Prevenção de poluição em `$HOME` na instalação

**Data:** 2026-05-23
**Status:** Design aprovado, aguardando implementação

## Contexto

Usuário relatou que ao instalar `@noctuacore/agent-bar` na máquina de outra
pessoa, sobraram `package.json` e lockfile em `$HOME`. Investigação mostrou:

- `package.json` do projeto não tem `preinstall`/`postinstall`.
- `scripts/agent-bar` (bin) é shim Bash que só faz `exec bun src/index.ts`.
- `src/update.ts` só roda `bun install` dentro de `~/.agent-bar` ou `bun add -g`.
- Nenhum código do agent-bar escreve em `$HOME`.

**Causa raiz:** comando de install rodado sem `-g` a partir de `~`:
- `bun add @noctuacore/agent-bar` → cria `package.json` + `bun.lock` + `node_modules/` em `$HOME`
- Variantes com `npm i` (sem `-g`) ou `bun install <pkg>` têm o mesmo efeito.

O projeto não tem bug — tem zero defesa contra erro humano.

## Objetivo

Reduzir probabilidade do erro acontecer + dar caminho de cleanup quando
acontecer. Sem mudar a UX do install correto (`bun add -g`).

## Escopo aprovado

Três frentes, ordenadas da mais barata para a mais cara:

1. **README defensivo** — snippet imune a esquecimento de `-g`.
2. **`preinstall` guard** — aborta install local em `$HOME` antes de poluir.
3. **`agent-bar doctor`** — detecta lixo órfão em `$HOME` e oferece cleanup.
   `agent-bar setup` ganha um check leve que sugere `doctor` se detectar lixo.

## Decisões

### D1. Preinstall sempre aborta quando `INIT_CWD == $HOME`

`INIT_CWD` é setado por npm e bun para o diretório de onde o usuário invocou
o comando, independente do `cwd` que o lifecycle script vê. É a sinalização
correta para "onde o usuário estava quando rodou install".

Trade-off: também aborta `bun add -g @noctuacore/agent-bar` rodado a partir
de `~`. Mitigação: mensagem de erro orienta `cd /tmp && bun add -g ...`, que
é o mesmo snippet do README. Usuário não fica preso, só é forçado a usar a
forma defensiva.

**Por que não tentar detectar `-g`:** `npm_config_global` e o path do pacote
em `~/.bun/install/global/` funcionam, mas adicionam complexidade e edge
cases entre versões de bun/npm. Estrito + mensagem clara é simpler e seguro.

### D2. Doctor é manual, com hint no setup

`agent-bar doctor` só roda quando invocado. `agent-bar setup` ganha um check
de detecção (não-destrutivo) que imprime sugestão se achar lixo:

> Detected leftover install in $HOME. Run `agent-bar doctor` to clean up.

Sem check no Waybar JSON path (overhead em loop apertado + contrato de stdout
limpo no CLAUDE.md §1).

### D3. Doctor é conservador em `$HOME/package.json`

Remove `package.json` **só** se as deps forem exclusivamente
`@noctuacore/agent-bar` (deps + devDeps unidas == `{@noctuacore/agent-bar}`).
Se tiver qualquer outra dep, warna mas não toca — assume projeto legítimo do
usuário. Lockfile só é removido se o package.json foi removido ou estava
ausente mas há `node_modules/@noctuacore/`.

## Design

### Frente A — README

**Mudança em `README.md`:** seção "Install" passa a usar:

```bash
cd /tmp && bun add -g @noctuacore/agent-bar && agent-bar setup
```

Nota curta logo abaixo:

> Don't drop the `-g`. If you ever see `package.json` appear in `$HOME` after
> installing, run `agent-bar doctor` to clean it up.

### Frente B — preinstall guard

**Mudança em `package.json`:** adicionar campo `scripts.preinstall`:

```json
"preinstall": "node -e \"const os=require('os');const c=process.env.INIT_CWD||process.cwd();if(c===os.homedir()){console.error('\\n[agent-bar] Install must be global. Local install in $HOME pollutes the home directory.\\n  Run from a safe dir:  cd /tmp && bun add -g @noctuacore/agent-bar\\n');process.exit(1)}\""
```

Notas:
- Inline (não vira arquivo em `scripts/` — CLAUDE.md §1 proíbe TS lá).
- Usa `node -e`: bun/npm garantem node disponível durante lifecycle.
- Mensagem inclui o comando exato pra desbloquear.

**Teste:** `tests/package.test.ts` valida que o script existe e referencia
`INIT_CWD` + `homedir`.

### Frente C — `agent-bar doctor`

**Novo arquivo:** `src/doctor.ts`

Responsabilidades:
- Detectar artefatos em `$HOME`:
  - `package.json` órfão (regra D3)
  - `node_modules/@noctuacore/agent-bar/`
  - `bun.lock`, `bun.lockb`, `package-lock.json` (regra D3)
- Imprimir relatório usando `tui/terminal-ui` (`printCommandHeader`,
  `printKeyValues`).
- Confirmar remoção via `@clack/prompts` `confirm`.
- Remover artefatos selecionados.
- Suportar `--dry-run` e `--yes` (não-interativo, útil em testes).

**API exportada para testes:**

```ts
export interface DoctorFindings {
  packageJsonOrphan: boolean;
  packageJsonMixed: boolean;  // tem agent-bar + outras deps
  nodeModulesDir: string | null;
  lockfiles: string[];
}

export interface DoctorOptions {
  home?: string;       // injeção pra testes
  dryRun?: boolean;
  yes?: boolean;
  confirm?: (findings: DoctorFindings) => Promise<boolean>;
}

export async function scan(home: string): Promise<DoctorFindings>;
export async function runDoctor(options: DoctorOptions): Promise<{
  status: 'clean' | 'cancelled' | 'cleaned' | 'mixed-only';
  removed: string[];
}>;
export async function main(): Promise<void>;
```

**Mudança em `src/index.ts`:** dispatcher ganha case `doctor` que chama
`runDoctor` na forma interativa.

**Mudança em `src/setup.ts`:** após sucesso, chamar `scan(homedir())` e se
detectar lixo, imprimir hint apontando pra `agent-bar doctor`. Sem prompt,
sem remoção — só print. Falha silenciosa se o scan errar (não bloqueia setup).

**Não-objetivos:**
- Não remove `~/.agent-bar` (managed install — fora de escopo).
- Não toca em `~/.bun/install/global/` (instalação global correta).
- Não verifica versão do bun/node.

### Testes

| Arquivo | Cenários |
| --- | --- |
| `tests/package.test.ts` (existente) | `preinstall` presente, contém `INIT_CWD`, contém `homedir` |
| `tests/doctor.test.ts` (novo) | (a) `$HOME` limpo → `status: 'clean'`. (b) `package.json` órfão puro + lockfile → remove ambos. (c) `package.json` misto (agent-bar + outras deps) → `status: 'mixed-only'`, não remove package.json mas remove `node_modules/@noctuacore/`. (d) só `node_modules/@noctuacore/` → remove. (e) só lockfile sem package.json → não remove (regra D3). (f) `--dry-run` → não remove nada, retorna lista. (g) `confirm: false` → `status: 'cancelled'`. |
| `tests/setup.test.ts` (existente, se houver) ou novo `tests/setup-doctor-hint.test.ts` | Hint impresso quando lixo presente; ausente quando limpo. |
| `tests/cli.test.ts` (existente) | `agent-bar doctor` é reconhecido pelo dispatcher. |

Mocks via `fs` (já é padrão do projeto, vide §4 do CLAUDE.md). `home` é
injetado por opção pra evitar precisar mock global de `os.homedir`.

### Documentação

- `docs/commands.md` — entry para `doctor` (descrição, flags, exemplos).
- `README.md` — seção "Install" atualizada (A) e referência ao `doctor` no troubleshooting.
- `CHANGELOG.md` — `[Unreleased]` com `feat: add doctor command` e
  `feat: refuse local install in $HOME via preinstall guard`.

## Riscos

| Risco | Severidade | Mitigação |
| --- | --- | --- |
| Preinstall aborta `bun add -g` legítimo rodado de `~` | Médio | Mensagem clara orienta `cd /tmp && bun add -g`. README usa essa forma. |
| Doctor remove `package.json` de projeto legítimo do usuário | Alto | Regra D3: só remove se deps == `{@noctuacore/agent-bar}`. Em caso misto, status `mixed-only` + warn. Confirm interativo obrigatório fora de `--yes`. |
| `INIT_CWD` não disponível em algum runner | Baixo | Fallback `process.cwd()`. Se nenhum bater, guard não dispara (fail-open OK — pior caso é o lixo aparecer, que é o estado atual). |
| Mensagens de erro do provider mudam contrato (CLAUDE.md §3) | N/A | Mudanças não tocam providers. |

## Out of scope

- Detecção de instalações via outros gerenciadores (yarn, pnpm) — agent-bar é
  Bun-only por contrato.
- Cleanup de `~/.agent-bar` managed install.
- Telemetria de quantos usuários acionam o preinstall guard.
- Migração automática (mover deps do `$HOME/package.json` órfão pra global).

## Verificação

Após implementação, rodar (CLAUDE.md §2):

```
bun test tests/package.test.ts tests/doctor.test.ts tests/cli.test.ts
bun run typecheck
bun run lint
```

Smoke manual:
- `bun run start doctor` em `$HOME` limpo → "clean".
- Criar `$HOME/package.json` mock com agent-bar dep, rodar `bun run start doctor`, confirmar remoção.
- `INIT_CWD=$HOME node -e "$(jq -r .scripts.preinstall package.json)"` → exit 1.
- `INIT_CWD=/tmp node -e "$(jq -r .scripts.preinstall package.json)"` → exit 0.
