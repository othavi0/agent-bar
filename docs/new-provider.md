# Adicionando um novo Provider

## Overview

Um **provider** em qbar representa uma fonte de dados de quota de LLM. Cada provider implementa a interface `Provider` definida em `src/providers/types.ts`, que exige:

| Campo/Metodo     | Tipo                          | Descricao                                     |
|------------------|-------------------------------|-----------------------------------------------|
| `id`             | `readonly string`             | Identificador unico (ex: `"claude"`, `"amp"`) |
| `name`           | `readonly string`             | Nome para exibicao no TUI e Waybar            |
| `cacheKey`       | `readonly string`             | Chave de cache (formato `<name>-quota`)        |
| `isAvailable()`  | `() => Promise<boolean>`      | Verifica se credenciais existem (rapido, sem fetch) |
| `getQuota()`     | `() => Promise<ProviderQuota>`| Busca e retorna os dados de quota              |

O retorno de `getQuota()` segue o contrato `ProviderQuota`. Os campos obrigatorios sao `provider`, `displayName` e `available`. O campo `primary` e o valor principal exibido no modulo Waybar.

---

## Template de codigo

```typescript
// src/providers/<name>.ts
import { CONFIG } from '../config';
import { logger } from '../logger';
import { cache } from '../cache';
import type { Provider, ProviderQuota } from './types';

export class MyProvider implements Provider {
  readonly id = 'my-provider';
  readonly name = 'My Provider';
  readonly cacheKey = 'my-provider-quota';

  async isAvailable(): Promise<boolean> {
    // Verificar se credenciais/binario existem no filesystem.
    // NAO fazer fetch — deve ser rapido.
    const file = Bun.file(CONFIG.paths.myProvider.credentials);
    return await file.exists();
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    if (!await this.isAvailable()) {
      return { ...base, error: 'Not logged in. Run `qbar login my-provider` to authenticate.' };
    }

    try {
      return await cache.getOrFetch<ProviderQuota>(
        this.cacheKey,
        async () => {
          // --- Buscar dados de quota aqui ---
          // Use AbortController para timeouts:
          const controller = new AbortController();
          const timeout = setTimeout(() => controller.abort(), CONFIG.api.timeoutMs);

          try {
            // fetch / spawn / parse — depende do provider
            const remaining = 75; // exemplo
            return {
              ...base,
              available: true,
              primary: {
                remaining,
                resetsAt: new Date(Date.now() + 3_600_000).toISOString(),
              },
            };
          } finally {
            clearTimeout(timeout);
          }
        },
        CONFIG.cache.ttlMs,
      );
    } catch (error) {
      logger.error('MyProvider quota fetch error', { error });
      return { ...base, error: 'Failed to fetch usage' };
    }
  }
}
```

---

## Checklist de integracao

1. **Criar `src/providers/<name>.ts`** implementando a interface `Provider`
2. **Registrar em `src/providers/index.ts`** — adicionar import e incluir no array `providers`:
   ```typescript
   import { MyProvider } from './my-provider';
   // ...
   export const providers: Provider[] = [
     new ClaudeProvider(),
     new CodexProvider(),
     new AmpProvider(),
     new MyProvider(),  // <-- novo
   ];
   ```
3. **Adicionar icon** em `icons/<name>-icon.svg` (ou `.png`) — 16x16 ou 24x24. Referenciado automaticamente pelo CSS do Waybar
4. **Adicionar ao Waybar contract** em `src/waybar-contract.ts`:
   - Incluir o id na const `WAYBAR_PROVIDERS`:
     ```typescript
     export const WAYBAR_PROVIDERS = ["claude", "codex", "amp", "my-provider"] as const;
     ```
   - Adicionar a regra CSS de icone na funcao `exportWaybarCss`:
     ```typescript
     `#custom-qbar-my-provider { background-image: url("${iconRef("my-provider-icon.svg")}"); }`
     ```
5. **Adicionar ao TUI login flow** em `src/tui/login.ts` — adicionar um `case` no `switch(choice)` com as instrucoes de login do provider
6. **Adicionar paths de credenciais** a `CONFIG.paths` em `src/config.ts`:
   ```typescript
   myProvider: {
     credentials: join(homedir(), '.my-provider', 'auth.json'),
   },
   ```
7. **Criar testes** em `tests/providers/<name>.test.ts` cobrindo:
   - `isAvailable()` com e sem credenciais
   - `getQuota()` com resposta valida
   - `getQuota()` com erros (timeout, credenciais invalidas, API indisponivel)
   - Validacao dos campos de `ProviderQuota` retornados

---

## Convencoes

### Cache

- O `cacheKey` **deve ser unico** entre todos os providers. Usar o formato `<name>-quota` (ex: `claude-usage`, `codex-quota`, `amp-quota`)
- Chaves de cache so aceitam `[a-zA-Z0-9_-]` — a classe `Cache` rejeita caracteres fora desse range
- Usar `cache.getOrFetch()` para o padrao get-or-fetch com TTL automatico. Para controle manual (como Codex faz), usar `cache.get()` + `cache.set()` separadamente

### Disponibilidade

- `isAvailable()` deve ser **rapido**: verificar existencia de arquivo ou binario, nunca fazer HTTP request ou spawn de processo
- Exemplos reais: Claude verifica se `~/.claude/.credentials.json` existe e tem `accessToken`; Codex verifica se `~/.codex/auth.json` existe; Amp verifica se o binario `amp` esta no PATH

### Tratamento de erros

- `getQuota()` **nunca deve lançar excecao** para o chamador. Erros devem ser retornados no campo `error`:
  ```typescript
  return { ...base, available: false, error: 'Descricao do problema' };
  ```
- A camada de orquestracao em `src/providers/index.ts` ja faz catch de excecoes nao tratadas, mas o provider deve lidar com seus proprios erros para dar mensagens uteis

### Timeouts

- Usar `AbortController` para HTTP requests (como Claude faz)
- Usar timeout explicito para CLI spawns (como Codex faz com `setTimeout`)
- O timeout padrao da API esta em `CONFIG.api.timeoutMs` (5s). A orquestracao aplica um timeout global de 10s por provider

### Dados de quota

- `primary` e o valor exibido no modulo Waybar (barra principal). Use-o para a janela mais relevante
- `secondary` e a janela secundaria (exibida no tooltip)
- `models` e `modelsDetailed` sao para providers com multiplos limites por modelo
- `meta` aceita key-values arbitrarios para dados extras especificos do provider (ex: Amp usa para `freeRemaining`, `replenishRate`)
- Todos os valores de `remaining` sao **percentuais de 0 a 100**
- `resetsAt` deve ser um **ISO 8601 timestamp** ou `null`

---

## Exemplos reais

Os tres providers existentes ilustram padroes diferentes:

### Claude (`src/providers/claude.ts`) — API fetch

- Busca quota via HTTP (`fetch`) na API da Anthropic
- Usa `cache.getOrFetch()` com TTL padrao
- Timeout via `AbortController` no request
- Retorna `primary` (5h), `secondary` (7d), `weeklyModels`, e `extraUsage`

### Codex (`src/providers/codex.ts`) — CLI spawn + file parsing

- Estrategia em duas camadas: tenta `codex app-server` (stdio JSON-RPC) primeiro, faz fallback para parse de session logs (`.jsonl`)
- Usa `cache.get()` + `cache.set()` manualmente para controle fino
- Dados ricos em `modelsDetailed` com multiplos buckets de limites
- Exemplo de como lidar com protocolos mais complexos

### Amp (`src/providers/amp.ts`) — CLI spawn + stdout parsing

- Executa `amp usage` e faz parse do texto de saida com regex
- Usa `cache.getOrFetch()` encapsulando todo o fetch
- Calcula `resetsAt` estimado a partir da taxa de reposicao
- Popula `models` e `meta` com dados extras (free tier, credits)
