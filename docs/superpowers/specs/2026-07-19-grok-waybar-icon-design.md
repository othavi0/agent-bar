# Spec — Ícone oficial Grok na Waybar

**Data:** 2026-07-19

**Status:** aprovado (brainstorming)

**Origem:** placeholder `icons/grok-icon.svg` era um “G” genérico; trocar pelo logomark oficial xAI/Grok.

## 1. Problema

O provider Grok já existe na barra e na TUI. O asset Waybar em `icons/grok-icon.svg` é um SVG com `<text>G</text>`, não a marca do produto. Os demais providers usam logo de marca (PNG/SVG real).

## 2. Fonte canônica

| Item | Valor |
| --- | --- |
| Guidelines | https://x.ai/legal/brand-guidelines (14 Feb 2025) |
| Pack | https://data.x.ai/logos/SpaceXAI_Grok_Assets.zip (`SpaceXAI_Grok_Assets`) |
| Asset escolhido | `Grok_Logomark_Light.svg` (símbolo singularity G, fill branco) |
| Não usar | wordmark, full logomark, símbolos SpaceXAI (empresa ≠ produto Grok), `Grok_Logomark_Dark` (some em barra escura) |

Brand guidelines: usar logos **exatamente como fornecidos**, sem alteração de geometria ou cor.

## 3. Decisão de design

**Abordagem:** substituir o conteúdo de `icons/grok-icon.svg` pelo SVG oficial `Grok_Logomark_Light.svg`, mantendo o **nome de arquivo** do projeto.

| Decisão | Escolha | Motivo |
| --- | --- | --- |
| Qual marca | Logomark Grok light | Legível em 14px; fundo Waybar escuro |
| Formato no repo | SVG (mesmo path) | CSS/testes já referenciam `grok-icon.svg`; Amp também é SVG |
| Recolor tema cyan | Não | Viola “without any alteration” |
| Mudar CSS/Rust | Não | Zero mudança de contrato além do asset |

## 4. Escopo

### In

1. Sobrescrever `icons/grok-icon.svg` com o conteúdo de `Grok_Logomark_Light.svg` do pack oficial.
2. Documentar no CHANGELOG (quando houver release) ou nota de verificação: installs existentes precisam re-copiar icons.

### Out

- Alterar `waybar_contract.rs`, módulos JSON, ou nome do arquivo.
- Commitar o zip inteiro no repo.
- Ícones na TUI (Nerd Font / estado, não logomark de provider).
- Recolor, crop, path simplification “para performance”.

## 5. Contrato que permanece

- CSS: `#custom-agent-bar-grok { background-image: url("…/grok-icon.svg"); }`
- `background-size: 14px 14px` (já no export)
- Install: `icons/` → `~/.config/waybar/agent-bar/icons/` via `assets install` / setup

## 6. Deploy em máquina já instalada

O binário não embute o SVG em runtime para a barra; a Waybar lê o arquivo copiado no config. Após o merge/update:

1. Reinstalar assets (ex. fluxo `agent-bar assets install` com paths injetados, ou `setup`/`update` se o produto já republica icons — **sem** hand-edit de `~/.config/waybar` em testes).
2. Reload da Waybar se necessário.

## 7. Verificação

| Check | Como |
| --- | --- |
| Asset = oficial | Paths/fill do SVG = `Grok_Logomark_Light.svg` do zip |
| Contrato CSS | `cargo test waybar_contract` (asserta nome `grok-icon.svg`) |
| Sem regressão de wiring | Diff só em `icons/grok-icon.svg` (e spec/docs se aplicável) |

## 8. Critério de pronto

- [ ] `icons/grok-icon.svg` é o logomark light oficial (não “G” texto).
- [ ] Nenhuma alteração de cor/paths além do conteúdo oficial.
- [ ] `cargo test waybar_contract` passa.
- [ ] Spec deste doc revisado e ok para plano de implementação.
