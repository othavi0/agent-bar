# Design — publicação automática no npm

Data: 2026-05-19
Status: aprovado, pronto para plano de implementação

## Problema

Publicar o pacote `@noctuacore/agent-bar` no npm é hoje totalmente manual: bump
de versão, `bun run publish:npm`, autenticação 2FA via navegador, criação de tag
git. O passo de 2FA não pode rodar em CI, então não há automação possível sem
antes resolver a autenticação.

## Objetivo

Publicar o pacote no npm automaticamente quando uma GitHub Release é publicada,
com os testes e checagens rodando como gate antes da publicação.

## Decisões

| # | Decisão | Escolha |
|---|---------|---------|
| 1 | Gatilho da publicação | GitHub Release publicada (`on: release: types: [published]`) |
| 2 | Autenticação npm em CI | Token de automação npm, guardado como secret `NPM_TOKEN` |
| 3 | Gate antes de publicar | `bun run release:check` (testes, typecheck, lint, build, pack) |
| 4 | Consistência versão/tag | Workflow falha se `package.json` não bater com a tag da Release |
| 5 | CI de testes em PRs | Fora de escopo (YAGNI) — `release:check` já roda os testes |
| 6 | Onde documentar o processo | `AGENTS.md` (doc canônica; `CLAUDE.md` é só shim) |

## Pré-requisito manual (uma vez, feito pelo usuário)

O 2FA da conta npm impede publicação não-interativa. A solução é um **token de
automação**:

1. Em npmjs.com, criar um Granular Access Token (ou classic Automation token)
   com permissão de publish no pacote `@noctuacore/agent-bar`. Tokens de
   automação ignoram o 2FA na publicação.
2. Adicionar o token como secret do repositório em GitHub → Settings → Secrets
   and variables → Actions, com o nome `NPM_TOKEN`.

Sem esse secret, o workflow falha no passo de publish com mensagem clara.

## Workflow `.github/workflows/publish.yml`

**Trigger:**

```yaml
on:
  release:
    types: [published]
```

**Job único, em `ubuntu-latest`. Passos:**

1. **Checkout** — `actions/checkout`. No evento `release`, isso pega o commit
   que a tag da Release aponta, então publica exatamente o estado da Release.
2. **Setup Bun** — `oven-sh/setup-bun`.
3. **Instalar dependências** — `bun install --frozen-lockfile`.
4. **Guarda de versão** — lê `version` do `package.json` e compara com
   `github.event.release.tag_name`. A tag esperada é `v<version>` (ex.: versão
   `4.0.3` ↔ tag `v4.0.3`). Se não baterem, o passo falha com mensagem
   explicando o mismatch — antes de qualquer interação com o npm.
5. **Release check** — `bun run release:check`. Roda testes, typecheck, lint,
   build e pack dry-run. Qualquer falha aborta o job e nada é publicado.
6. **Publish** — `bun run publish:npm`, com a env var `NPM_CONFIG_TOKEN`
   definida a partir do secret `NPM_TOKEN`. O script
   `scripts/bun-publish-with-npm-token` já consome `NPM_CONFIG_TOKEN` — não
   precisa de alteração.

**Tratamento de falhas:** os passos rodam em sequência; qualquer falha aborta o
job e nada é publicado. O mismatch de versão/tag falha no passo 4, antes do
`release:check` e do publish.

## Documentação em `AGENTS.md`

Adicionar uma seção "Release" descrevendo o fluxo de release a partir de agora:

1. Bump da versão no `package.json`.
2. Atualizar o `CHANGELOG.md` (mover/criar a seção da versão).
3. Commit das duas mudanças.
4. Criar uma GitHub Release com tag `v<versão>` e notas.
5. O workflow `publish.yml` publica no npm automaticamente.

A seção também registra o pré-requisito do secret `NPM_TOKEN` e o porquê (2FA).

## Arquivos afetados

| Arquivo | Mudança |
|---------|---------|
| `.github/workflows/publish.yml` | Criar — o workflow de publicação |
| `AGENTS.md` | Adicionar a seção "Release" |

## Verificação

Não há testes automatizados para um workflow de CI. A verificação é:

- O YAML é válido e o workflow aparece na aba Actions do GitHub após o merge.
- O passo da guarda de versão é exercitado conceitualmente: tag `v<version>`
  igual ao `package.json` passa; diferente falha.
- A validação real de ponta a ponta acontece na próxima Release de verdade.
  Até lá, o workflow pode ser revisado por leitura.

## Fora de escopo

- CI de testes em pull requests e pushes (decisão 5).
- Criação automática de tag ou de GitHub Release — a Release é criada
  manualmente pelo usuário, e isso é o gatilho.
- Geração automática de release notes / changelog.
- Publicação em outros registries além do npm.
