# Publicação Automática no npm — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publicar `@noctuacore/agent-bar` no npm automaticamente quando uma GitHub Release é publicada, com testes e checagens como gate.

**Architecture:** Um único workflow do GitHub Actions disparado pelo evento `release: published`. O job confere que `package.json` bate com a tag da Release, roda `bun run release:check`, e publica via o script `scripts/bun-publish-with-npm-token` já existente (que consome a env var `NPM_CONFIG_TOKEN`). Mais uma seção "Release" no `AGENTS.md` documentando o fluxo.

**Tech Stack:** GitHub Actions, Bun, `oven-sh/setup-bun`, `actions/checkout`.

**Spec:** `docs/superpowers/specs/2026-05-19-publicacao-automatica-npm-design.md`

**Nota sobre testes:** um workflow de CI não tem teste automatizado. A verificação é por revisão estrutural e, no fim, pela próxima Release real. Os passos abaixo refletem isso — não há ciclo TDD.

---

### Task 1: Workflow de publicação

**Files:**
- Create: `.github/workflows/publish.yml`

- [ ] **Step 1: Criar o diretório e o arquivo do workflow**

Criar `.github/workflows/publish.yml` com exatamente este conteúdo:

```yaml
name: Publish to npm

on:
  release:
    types: [published]

permissions:
  contents: read

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Bun
        uses: oven-sh/setup-bun@v2

      - name: Install dependencies
        run: bun install --frozen-lockfile

      - name: Verify version matches release tag
        run: |
          PKG_VERSION="$(jq -r .version package.json)"
          EXPECTED_TAG="v${PKG_VERSION}"
          ACTUAL_TAG="${{ github.event.release.tag_name }}"
          if [ "$EXPECTED_TAG" != "$ACTUAL_TAG" ]; then
            echo "::error::Release tag '$ACTUAL_TAG' does not match package.json version (expected '$EXPECTED_TAG')"
            exit 1
          fi
          echo "Version $PKG_VERSION matches release tag $ACTUAL_TAG"

      - name: Release check
        run: bun run release:check

      - name: Publish to npm
        run: bun run publish:npm
        env:
          NPM_CONFIG_TOKEN: ${{ secrets.NPM_TOKEN }}
```

Justificativa de cada parte (não copiar para o arquivo):
- `on: release: types: [published]` — dispara só quando uma Release é publicada (não em draft/prerelease edit).
- `permissions: contents: read` — menor privilégio; o job só lê o repo, não escreve nada no GitHub.
- `actions/checkout@v4` — no evento `release`, faz checkout do commit que a tag aponta.
- `oven-sh/setup-bun@v2` — instala o Bun no runner.
- `bun install --frozen-lockfile` — instala deps respeitando o `bun.lock`.
- Guarda de versão — `jq` é pré-instalado no `ubuntu-latest`; compara `v<version>` com `github.event.release.tag_name` e falha cedo no mismatch.
- `bun run release:check` — script já existente: `bun test && bun run typecheck && bun run lint && bun run build && bun pm pack --dry-run --ignore-scripts`.
- `bun run publish:npm` — script já existente: `bash ./scripts/bun-publish-with-npm-token --access public`. O script lê `NPM_CONFIG_TOKEN` da env; quando presente, pula o fallback do `~/.npmrc` e roda `bun publish --access public`.
- `NPM_CONFIG_TOKEN` vem do secret `NPM_TOKEN` (criado manualmente pelo usuário — ver Task de documentação).

- [ ] **Step 2: Verificar a sintaxe YAML**

Se `actionlint` estiver instalado, rodar: `actionlint .github/workflows/publish.yml`
Expected: sem erros.

Se `actionlint` não estiver disponível (`command -v actionlint` vazio), validar o YAML com:

Run: `bun -e "const f=require('fs').readFileSync('.github/workflows/publish.yml','utf8'); if(!f.includes('release:')||!f.includes('publish:npm')) throw new Error('conteúdo inesperado'); console.log('arquivo presente e com as chaves esperadas');"`
Expected: imprime "arquivo presente e com as chaves esperadas".

Depois, revisar o arquivo visualmente contra o checklist:
- `on:` é `release` / `types: [published]`.
- Os 6 passos estão presentes e na ordem: Checkout, Setup Bun, Install, Verify version, Release check, Publish.
- O passo Publish tem o bloco `env:` com `NPM_CONFIG_TOKEN: ${{ secrets.NPM_TOKEN }}`.
- A indentação é consistente (2 espaços, sem tabs).

- [ ] **Step 3: Verificar que os scripts referenciados existem**

Run: `bun run --silent 2>&1 | grep -E "release:check|publish:npm" || jq -r '.scripts | keys[]' package.json | grep -E "release:check|publish:npm"`
Expected: lista `publish:npm` e `release:check` — confirma que o workflow chama scripts que existem no `package.json`.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/publish.yml
git commit -m "ci: publica no npm ao publicar uma Release"
```

---

### Task 2: Documentar o fluxo de release no AGENTS.md

**Files:**
- Modify: `AGENTS.md` (adicionar uma seção "Release")

- [ ] **Step 1: Encontrar o ponto de inserção**

Abrir `AGENTS.md` e localizar a seção sobre paths/runtime no final do arquivo (a tabela de owned paths que termina com `~/.config/waybar/scripts/agent-bar-open-terminal`). A nova seção "Release" deve ser adicionada logo após essa seção de paths, antes de qualquer seção de fechamento. Se a estrutura não casar com isso, inserir a seção "Release" como última seção do arquivo.

- [ ] **Step 2: Adicionar a seção "Release"**

Inserir esta seção (em inglês, para casar com o resto do `AGENTS.md`):

```markdown
## Release

Releases publish `@noctuacore/agent-bar` to npm automatically. The
`.github/workflows/publish.yml` workflow runs on the `release: published`
event: it verifies `package.json` matches the release tag, runs
`bun run release:check`, then publishes with `bun run publish:npm`.

To cut a release:

1. Bump `version` in `package.json`.
2. Update `CHANGELOG.md` — move or create the section for the new version.
3. Commit both changes.
4. Create a GitHub Release with tag `v<version>` (matching `package.json`) and
   notes. Publishing the Release triggers the workflow.

The workflow needs an `NPM_TOKEN` repository secret: an npm automation /
granular access token with publish permission. npm 2FA blocks interactive
publishing, and automation tokens bypass 2FA — so this secret is required for
CI publishing to work. Set it in GitHub → Settings → Secrets and variables →
Actions.
```

- [ ] **Step 3: Verificar lint e links**

Run: `bun run lint`
Expected: sem erros (o lint do biome não cobre `.md`, mas confirma que nada quebrou).

Revisar visualmente: a seção "Release" tem heading `##`, os code fences estão balanceados, e a numeração 1-4 está correta.

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md
git commit -m "docs: documenta o fluxo de release automático"
```

---

## Verification

Após as duas tasks:

1. `.github/workflows/publish.yml` existe, YAML válido, com o trigger `release: published` e os 6 passos.
2. `AGENTS.md` tem a seção "Release" descrevendo o fluxo e o pré-requisito do secret.
3. `bun run lint` passa.
4. A validação de ponta a ponta acontece na próxima Release real — não é testável localmente.

## Passos manuais do usuário (fora deste plano)

Estes não são tarefas de implementação — são pré-requisitos que só o usuário pode fazer, e devem ser explicados a ele ao final:

1. Criar um token de automação no npmjs.com com permissão de publish em `@noctuacore/agent-bar`.
2. Adicionar o token como secret `NPM_TOKEN` no repositório GitHub.

Sem o secret, o workflow falha no passo Publish.
