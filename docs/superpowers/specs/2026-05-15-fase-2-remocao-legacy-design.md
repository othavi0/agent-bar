# Fase 2 — Remoção total da camada legacy

**Data:** 2026-05-15
**Status:** aprovado, pronto para planejamento
**Projeto:** agent-bar — mutirão de limpeza (Fase 2 de 3)

## Contexto

O `agent-bar` teve dois nomes anteriores: `qbar` (mais antigo) e `agent-bar-omarchy`
(intermediário). O código carrega uma camada de compatibilidade para ambos:
constantes `LEGACY_*`/`QBAR_LEGACY_*`, migração de paths de settings/cache, limpeza
de assets Waybar antigos, símlink e `bin` alias de compatibilidade.

A Fase 1 (correção e endurecimento) já está concluída e no `master`. Esta fase
remove **toda** a camada de compatibilidade. As migrações são one-shot e já foram
executadas pelos usuários; a máquina do próprio autor já está 100% no nome novo
(`~/.config/agent-bar/`, `~/.cache/agent-bar/`, símlink `~/.local/bin/agent-bar`),
sem nenhum diretório `qbar`/`agent-bar-omarchy`.

Isto é uma **breaking change**: o comando `agent-bar-omarchy` e o `bin` alias
deixam de existir.

## Decisões tomadas no design

- Remover **toda** a camada legacy (`qbar` + `agent-bar-omarchy`) — hard-cut, sem
  período de transição.
- Deletar a pasta `snippets/` inteira (exemplos manuais redundantes com o contrato
  Waybar gerado).
- Bump de versão para `4.0.0` (breaking change).
- Os 3 artefatos legacy na máquina do autor (símlink `~/.local/bin/agent-bar-omarchy`
  e os backups `*.agent-bar-omarchy-backup` em `~/.config/waybar/`) **já foram
  removidos** antes desta fase — não são item de spec.

## Abordagem

Remoção mecânica e coesa. A ordem importa: editar os **consumidores** das constantes
legacy antes de deletar as constantes em si, senão o build (`tsc`) quebra. O plano de
implementação sequencia por módulo (leaf-first: waybar/uninstall/setup/cache/settings
→ depois `config.ts` → depois `app-identity.ts`) para manter o build verde a cada
commit. O `bun run typecheck` é a rede de segurança principal: qualquer import legacy
órfão falha a compilação.

## Escopo

### Arquivos deletados por completo

- `scripts/agent-bar-omarchy`
- `scripts/agent-bar-omarchy-open-terminal`
- `snippets/` (diretório inteiro: `waybar-config.jsonc`, `waybar-modules.jsonc`,
  `waybar-style.css`)

### Arquivos editados — remover a superfície legacy

| Arquivo | Remover |
| --- | --- |
| `src/app-identity.ts` | As 16 constantes `LEGACY_*` e `QBAR_LEGACY_*`. Mantêm-se `APP_NAME`, `APP_WINDOW_TITLE`, `APP_BASE_CLASS`, `APP_HIDDEN_CLASS`, `WAYBAR_NAMESPACE`, `WAYBAR_MODULE_PREFIX`, `WAYBAR_SELECTOR_PREFIX`, `TERMINAL_HELPER_NAME`, `BACKUP_SUFFIX`. |
| `src/config.ts` | Os 6 paths legacy em `CONFIG.paths`: `legacyCache`, `qbarLegacyCache`, `waybarLegacyCache`, `waybarQbarLegacyCache`, `legacyConfig`, `qbarLegacyConfig`. |
| `src/cache.ts` | `migrateLegacyCache()` e o array `legacyCacheDirs` do construtor; a chamada de migração em `ensureDir()`. O construtor passa a receber só `cacheDir`. |
| `src/settings.ts` | `migrateLegacySettingsSync()` e as 4 chamadas a ela; simplificar `getSettingsPaths()` para devolver só `settingsDir`/`settingsFile`. **Manter** `migrateSettings()` (schema v1→v2). |
| `src/setup.ts` | Variáveis e criação/remoção dos símlinks legacy (`legacyLink`, `legacyTarget`, `qbarLegacyLink`) em `createSymlink()`. Cria apenas o símlink `agent-bar`. |
| `src/uninstall.ts` | `legacyDefaults`, `qbarLegacyDefaults`, `LEGACY_SETTINGS_DIR`, `QBAR_LEGACY_SETTINGS_DIR`, `LEGACY_SYMLINK`, `QBAR_LEGACY_SYMLINK`, as referências legacy em `p.note()`, e as ~8 chamadas `removePathIfExists` de paths legacy. `removePathIfExists` (genérica) fica. |
| `src/waybar-integration.ts` | `LEGACY_STYLE_IMPORT`, `QBAR_LEGACY_STYLE_IMPORT`; os prefixos legacy em `MANAGED_MODULE_PREFIXES` (fica só `[WAYBAR_MODULE_PREFIX]`); `getLegacyWaybarIntegrationPaths()`, `getQbarLegacyWaybarIntegrationPaths()`; simplificar `stripManagedStyleImports()` para 1 prefixo; remover o cleanup legacy em `applyWaybarIntegration()` e `removeWaybarIntegration()`. As funções `getLegacyModuleIDs()`/`getQbarLegacyModuleIDs()` ficam órfãs após a limpeza e são removidas — `tsc`/Biome confirmam que não há mais consumidor. |
| `src/waybar-contract.ts` | `getLegacyWaybarAssetPaths()`, `getQbarLegacyWaybarAssetPaths()`, `cleanupLegacyWaybarAssets()`, `getLegacyModuleIDs()`, `getQbarLegacyModuleIDs()`; imports de constantes legacy. |
| `package.json` | O `bin` alias `"agent-bar-omarchy"`; bump `version` para `4.0.0`. |
| `src/tui/index.ts` | O comentário "inspired by Omarchy branding" — deixar só "Block-style logo". |

### O que NÃO sai

- **`migrateSettings()`** (`src/settings.ts`) — migração de **schema** v1→v2.
  Verificado independente de identidade de nome legacy: depende apenas do array
  `LEGACY_DEFAULT_PROVIDERS` (`['claude', 'codex', 'amp']`), que é dado puro de
  schema. Permanece intacta.
- Helpers genéricos: `removePathIfExists`, o contrato Waybar core, `getSettingsPaths`
  (simplificada, não removida).

## Testes

- **Deletar** os testes de *migração de path* (tornam-se obsoletos):
  - `tests/app-identity.test.ts` — bloco "keeps agent-bar-omarchy as the compatibility namespace".
  - `tests/cache.test.ts` — teste "moves legacy cache files into the new directory".
  - `tests/settings.test.ts` — testes "moves agent-bar-omarchy settings into the new namespace", "still moves old qbar settings into the new namespace", e "does not overwrite existing new settings when a legacy directory still exists".
  - `tests/waybar-integration.test.ts` — teste "migrates legacy agent-bar-omarchy wiring to agent-bar and removes it cleanly".
- **Manter** o teste de *migração de schema* v1→v2 em `tests/settings.test.ts`
  ("adds Copilot to legacy default provider settings"). Renomear para deixar
  explícito que valida schema v1→v2, não path migration.
- Qualquer outro teste que importe constantes legacy removidas deve ser ajustado ou
  deletado conforme exercite só comportamento legacy.

## Documentação

- `README.md` — título "Agent Bar Omarchy" → "Agent Bar".
- `CLAUDE.md` — corrigir nome no título, comando (`./scripts/agent-bar-omarchy` →
  `./scripts/agent-bar`), paths (`~/.config/agent-bar-omarchy` → `~/.config/agent-bar`),
  remover a linha sobre compatibilidade `qbar`.
- `docs/integration.md` — remover/reescrever o parágrafo sobre a renomeação a partir
  de `agent-bar-omarchy`.
- `AGENTS.md` — **alvo mínimo nesta fase:** remover a seção "Legacy Policy" (agora
  factualmente falsa), tirar `~/.local/bin/agent-bar-omarchy` da tabela de owned
  paths, e ajustar menções a `qbar`/`agent-bar-omarchy` que deixaram de ser verdade.
  A refatoração **completa** do AGENTS.md fica para a Fase 3 — aqui só se corrige o
  que ficou incorreto.
- `CHANGELOG.md` — nova seção `[4.0.0]` documentando a remoção da camada de
  compatibilidade. O histórico antigo (`qbar`, `agent-bar-omarchy`, `[3.0.0]`) é
  preservado — changelog histórico é legítimo.
- Verificar e corrigir qualquer referência a `snippets/` em `docs/**`, `README.md`
  ou `AGENTS.md` (o diretório deixa de existir).

## Plano de verificação

- `bun run typecheck` — rede de segurança principal; um import legacy órfão falha aqui.
- `bun test` — suíte verde após as deleções de teste sincronizadas (351 testes hoje;
  o número cai com a remoção dos testes de migração legacy).
- `bun run lint` — Biome limpo (imports não usados são erro).
- Verificação de resíduo: `grep -ri "qbar\|agent-bar-omarchy\|omarchy" src/ tests/`
  não deve retornar nada além de, no máximo, entradas históricas intencionais. O
  alvo é zero ocorrências em `src/` e `tests/`.
- Antes do handoff: `bun test && bun run typecheck && bun run lint`.

## Fora de escopo

- Limpeza dos artefatos legacy na máquina do autor — **já feita** antes desta fase.
- Refatoração estrutural (builders duplicados, arquivos grandes) e rewrite completo
  de `AGENTS.md`/`CLAUDE.md` — Fase 3.

## Risco conhecido e aceito

Removida a limpeza de prefixos Waybar antigos (`custom/agent-bar-omarchy-*`,
`custom/qbar-*`) e a migração de path, uma instalação hipotética ainda no nome
antigo, ao atualizar para a v4.0.0:

- perderia o vínculo com suas settings/cache antigas (recriadas como padrão);
- manteria módulos/imports Waybar órfãos do nome antigo, sem limpeza automática.

Aceito: não há instalações reais com o nome antigo, e a máquina do autor já está
inteiramente no nome novo. Documentar a quebra na seção `[4.0.0]` do `CHANGELOG.md`
é a mitigação suficiente.
