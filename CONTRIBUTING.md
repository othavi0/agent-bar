# Contribuindo para o qbar

Obrigado pelo interesse em contribuir com o qbar! Este guia cobre o essencial para
configurar o ambiente, manter a consistencia do codigo e enviar suas alteracoes.

## Pre-requisitos

| Ferramenta | Versao minima |
|------------|---------------|
| [Bun](https://bun.sh) | >= 1.0 |
| Git | qualquer versao recente |

> **Nota:** Bun e o unico runtime suportado. Node/Deno nao sao compatoveis.

## Setup do dev environment

```bash
git clone <repo-url>
cd qbar
bun install
```

Pronto. Nao ha etapa de build -- o Bun executa TypeScript diretamente.

## Comandos uteis

```bash
bun run start          # Executar (equivale a ./scripts/qbar)
bun run dev            # Watch mode (reinicia ao salvar)
bun test               # Rodar testes (com coverage via bunfig.toml)
bun run typecheck      # tsc --noEmit -- validacao de tipos sem emitir arquivos
```

> **Atencao:** nao use `bun ./scripts/qbar`. O arquivo e um shim bash e o Bun vai
> tentar interpreta-lo como JavaScript. Use `./scripts/qbar` (shell) ou `bun run start`.

## Conventional Commits (em portugues)

Todas as mensagens de commit seguem o padrao [Conventional Commits](https://www.conventionalcommits.org/),
escritas em **portugues**:

| Prefixo     | Quando usar                                |
|-------------|--------------------------------------------|
| `feat:`     | Nova funcionalidade                        |
| `fix:`      | Correcao de bug                            |
| `refactor:` | Refatoracao sem mudanca de comportamento   |
| `test:`     | Adicao ou modificacao de testes            |
| `docs:`     | Documentacao                               |
| `chore:`    | Manutencao (deps, CI, configs)             |

Exemplos:

```
feat: adicionar provider para Gemini
fix: corrigir parsing de reset time no provider Amp
test: cobrir cenarios de cache expirado
```

## Code style

- **TypeScript strict** -- o `tsconfig.json` usa `"strict": true`. Evite `any` sempre que possivel.
- **Nomes de variaveis e funcoes em ingles**, usando `camelCase`.
- **Commits e comunicacao** em portugues.
- Path aliases: use `@/*` para importar de `src/*` (configurado no `tsconfig.json`).
- Sem build step: o Bun resolve TypeScript + path aliases em runtime.

## Estrutura do projeto

```
src/
  index.ts              # Entry point e command dispatcher
  cli.ts                # Parser de argumentos CLI
  config.ts             # Constantes (cache TTL, paths)
  settings.ts           # Leitura/escrita de ~/.config/qbar/settings.json
  cache.ts              # Cache em disco com TTL
  setup.ts              # Comando `qbar setup`
  waybar-contract.ts    # Contrato de modulos/CSS para Waybar
  waybar-integration.ts # Wiring automatico no config.jsonc + style.css
  providers/
    types.ts            # Interfaces Provider, ProviderQuota, QuotaWindow
    index.ts            # Registry de providers
    claude.ts           # Provider: Claude
    codex.ts            # Provider: Codex
    amp.ts              # Provider: Amp
  formatters/           # Formatacao de output (terminal e Waybar)
  tui/                  # Menus interativos e login flows (clack/prompts)
scripts/
  qbar                  # Bash shim (entry point do bin)
tests/                  # Testes (bun:test)
icons/                  # Icones dos providers para Waybar
docs/                   # Documentacao detalhada
```

## Testes

Os testes usam `bun:test` e ficam no diretorio `tests/`, espelhando a estrutura de `src/`.

### Rodando

```bash
bun test                       # Todos os testes (com coverage)
bun test tests/cache.test.ts   # Um arquivo especifico
```

### Escrevendo testes

```typescript
import { describe, expect, it, beforeEach, afterEach } from "bun:test";

describe("MinhaFeature", () => {
  it("deve fazer X quando Y", () => {
    const result = minhaFuncao();
    expect(result).toBe(valorEsperado);
  });
});
```

### Mocks

O `bun:test` oferece `mock()` e `spyOn()` nativamente:

```typescript
import { mock, spyOn } from "bun:test";

// Mock de funcao
const fn = mock(() => "valor mockado");

// Spy em metodo existente
const spy = spyOn(objeto, "metodo").mockReturnValue("fake");
```

### Boas praticas

- Use `beforeEach`/`afterEach` para setup e cleanup (especialmente arquivos temporarios).
- Testes de providers devem mockar chamadas HTTP -- nao dependa de credenciais reais.
- Nomeie os testes em portugues de forma descritiva: `"deve retornar erro quando token invalido"`.

## Como adicionar um novo provider

Consulte [`docs/new-provider.md`](docs/new-provider.md) para o guia completo.

Em resumo, um novo provider precisa:

1. Implementar a interface `Provider` de `src/providers/types.ts` (propriedades: `id`, `name`, `cacheKey`; metodos: `isAvailable()`, `getQuota()`).
2. Registrar o provider em `src/providers/index.ts`.
3. Adicionar testes em `tests/providers/<nome>.test.ts`.
4. Adicionar um icone em `icons/`.

## Links uteis

- [README principal](README.md) -- Quick Start e comandos
- [Docs index](docs/README.md) -- Documentacao detalhada
- [Waybar contract](docs/waybar-contract.md) -- Contrato de integracao com Waybar
- [Troubleshooting](docs/troubleshooting.md) -- Problemas comuns
