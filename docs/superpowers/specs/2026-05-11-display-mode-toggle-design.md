# Display Mode Toggle — Remaining vs Used

Data: 2026-05-11
Status: Spec aprovada, aguardando plano de implementação.

## Contexto e motivação

Hoje todas as percentuais exibidas (terminal + Waybar) representam **quota restante**: 100% = não usei nada, 0% = esgotado. O usuário quer poder inverter para **quota usada**: 0% = não usei nada, 100% = esgotado. Mantém a opção atual como default para não quebrar a expectativa dos usuários existentes.

## Decisões

1. **Toggle, não substituição.** Novo setting `waybar.displayMode: 'remaining' | 'used'`. Default `'remaining'`.
2. **Domínio inalterado.** `QuotaWindow.remaining` continua sendo a fonte da verdade (0–100, restante). Conversão acontece **apenas no formatter**.
3. **Escopo do toggle:** afeta terminal output e Waybar igualmente (consistência visual entre CLI e bar).
4. **Cores invertidas quando `used`.** Thresholds atuais (`green`/`yellow`/`orange`) continuam baseados em "saúde" — implementação converte o valor de display de volta para `health` antes de consultar thresholds, mantendo `config.ts` single-source.
5. **Barra de progresso enche conforme usa.** `bar(v)` desenha `round(v/100 * width)` blocos cheios. Em `used`, naturalmente cresce com o consumo. Cor segue a regra de saúde.
6. **ETA label troca.** `"Full in <eta>"` → `"Resets in <eta>"` quando `mode = 'used'`. Timestamp e cálculo de diff são idênticos. Guards de "cota cheia" (`remaining === 100`) continuam operando sobre o valor literal, não display.
7. **UX:** toggle via menu TUI, padrão dos demais settings (`separators`, `layout`). Sem flag CLI dedicada nesta iteração.

## Schema

```ts
// src/settings.ts
export type DisplayMode = 'remaining' | 'used';

export interface Settings {
  version: number;
  waybar: {
    providers: string[];
    showPercentage: boolean;
    separators: SeparatorStyle;
    providerOrder: string[];
    displayMode: DisplayMode;       // novo, default 'remaining'
  };
  // resto inalterado
}
```

Migração: campo aditivo opcional. `normalizeSettings` injeta `'remaining'` se ausente. Sem bump de versão (v1 permanece).

Validação: `isValidDisplayMode(value)` análogo a `isValidSeparator`; valor inválido cai para `'remaining'`.

## Helper central

```ts
// src/formatters/shared.ts
import type { DisplayMode } from '../settings';

export function toDisplay(remaining: number | null, mode: DisplayMode): number | null {
  if (remaining === null) return null;
  return mode === 'used' ? 100 - remaining : remaining;
}

export function toHealth(displayValue: number | null, mode: DisplayMode): number | null {
  if (displayValue === null) return null;
  return mode === 'used' ? 100 - displayValue : displayValue;
}
```

`toHealth` é usado pelos seletores de cor para que `getColor` / `getColorForPercent` em `config.ts` permaneçam puramente baseados em "saúde restante".

## Ajustes nos formatters compartilhados

`formatEta` e `formatResetTime` em `src/formatters/shared.ts`:

- Aceitam `remaining: number | null` (valor literal, não display) — comportamento preservado.
- `formatEta` ganha parâmetro opcional `mode: DisplayMode`. Quando `mode === 'used'`, label retornado por callers passa a ser `"Resets in"` em vez de `"Full in"` (o helper devolve apenas o eta; o label fica nos formatters terminal/waybar).
- Guard `remaining === 100` (cota cheia, ETA irrelevante) inalterado.

## Propagação do `mode`

Fluxo:

1. `loadSettings()` retorna `settings.waybar.displayMode`.
2. `src/index.ts` passa o valor para os formatters de terminal e Waybar como parâmetro explícito.
3. Formatters propagam para `bar()`, `indicator()`, helpers internos.
4. Sem variável global mutável; sem singleton.

## Call sites a atualizar

`src/formatters/terminal.ts` (~10 sites onde `remaining` é exibido / colorido / barreado):

- Funções locais `bar(pct)`, `indicator(val)`, `getColor(pct)` ganham parâmetro `mode` ou recebem valor já convertido + valor literal para cor.
- Linhas afetadas (referência): 14, 22, 29, 44, 54, 102–109, 168–172, 203–219, 234, 246–248, 297–299.
- Label "Full in"/"Resets in" no bloco de free quota (linhas ~217–220) muda conforme `mode`.

`src/formatters/waybar.ts` (~8 sites análogos):

- `bar(val)` e `indicator(val)` recebem `mode`.
- `getColorForPercent` é importado de `config.ts`; caller converte `display → health` antes de chamar.
- Linhas afetadas (referência): 58, 61–64, 67, 76–79, 136–142, 154–160, 169+.
- Label "Full in"/"Resets in" análogo.

`src/config.ts`: `getColorForPercent` permanece como está — recebe valor de saúde. Sem mudança de assinatura.

## TUI

Novo item no menu de configuração:

- Arquivo: `src/tui/configure-layout.ts` (segue o padrão usado para `separators`, linhas 130 e 152).
- Pergunta: "Display mode" com opções `Remaining (default)` e `Used`.
- Persiste via `saveSettings(settings)`.

Decisão de localização: vai junto com `configure-layout.ts` (afinidade temática com `separators` e ordering), não em um menu novo.

## Testes

Novos casos cobrindo `mode = 'used'`:

- `tests/settings.test.ts`: default é `'remaining'`; valor inválido cai para `'remaining'`; campo persiste em round-trip de save/load.
- `tests/formatters.test.ts`: `toDisplay(0, 'used') === 100`, `toDisplay(100, 'used') === 0`, `toDisplay(null, *) === null`; cor de display 90 em `used` é vermelho/orange (= health 10), enquanto display 90 em `remaining` é verde.
- `tests/formatters-snapshot.test.ts`: snapshots adicionais `*-used.snap` para terminal e Waybar cobrindo (a) cota cheia (b) cota parcial (c) cota esgotada (d) provider com múltiplos windows.

Snapshots existentes (`mode='remaining'`) permanecem intactos.

## Verificação antes do merge

```bash
bun test tests/settings.test.ts tests/formatters.test.ts tests/formatters-snapshot.test.ts
bun run typecheck
bun run lint
```

Para handoff amplo: `bun test && bun run typecheck && bun run lint`.

## Não-objetivos

- Não renomear `remaining` no domínio (providers/types/cache).
- Sem toggle per-provider ou per-window nesta iteração.
- Sem flag CLI; toggle só via TUI / edição manual de `settings.json`.
- Sem mudança em `getColorForPercent` / thresholds em `config.ts`.
