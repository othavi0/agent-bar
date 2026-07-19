# Grok Waybar Official Icon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Substituir o placeholder “G” em `icons/grok-icon.svg` pelo logomark oficial Grok Light do pack xAI, sem alterar CSS, Rust ou o nome do arquivo.

**Architecture:** Asset-only change. A Waybar carrega o ícone via CSS `background-image` gerado em `export_waybar_css()` apontando para `icons_dir/grok-icon.svg`. O setup/`assets install` copia o diretório `icons/` do repo para o destino Waybar. Basta sobrescrever o SVG no repo com o conteúdo oficial; installs existentes precisam re-rodar assets install (fora do escopo de código).

**Tech Stack:** SVG estático; Rust/cargo só para verificação de contrato (`cargo test waybar_contract`); fonte oficial `https://data.x.ai/logos/SpaceXAI_Grok_Assets.zip`.

## Global Constraints

- Fonte canônica: pack `SpaceXAI_Grok_Assets.zip` das [xAI Brand Guidelines](https://x.ai/legal/brand-guidelines).
- Asset: **exatamente** `Grok_Logomark_Light.svg` (sem recolor, sem editar paths).
- Nome no repo permanece `icons/grok-icon.svg` (contrato CSS/testes).
- Não mutar desktop ao vivo: não rodar `agent-bar setup`/`update`/`uninstall` sem aprovação; assets install só com paths injetados/temp.
- Sem Node/npm; Rust/cargo only para testes.
- Conventional Commits em PT, subject ≤50 chars; zero atribuição de AI em commits.
- Spec: `docs/superpowers/specs/2026-07-19-grok-waybar-icon-design.md`.

---

## File map

| Path | Responsibility |
| --- | --- |
| `icons/grok-icon.svg` | **Modify** — único asset Waybar do provider Grok |
| `src/waybar_contract.rs` | **Read only** — CSS e teste assertam `grok-icon.svg` |
| `docs/superpowers/specs/2026-07-19-grok-waybar-icon-design.md` | Spec já commitada |

Nenhum arquivo Rust, CSS gerado em runtime, ou teste golden de bytes do SVG precisa mudar.

---

### Task 1: Substituir `icons/grok-icon.svg` pelo logomark oficial Light

**Files:**
- Modify: `icons/grok-icon.svg` (substituir conteúdo inteiro)
- Test: `src/waybar_contract.rs` (já existente — `css_has_base_styles_icons_states`)
- Source of truth: download oficial (não usar lobe/uxwing/wiki)

**Interfaces:**
- Consumes: zip oficial `SpaceXAI_Grok_Assets/Grok_Logomark_Light.svg`
- Produces: `icons/grok-icon.svg` byte-equivalente ao logomark light oficial (paths + fills)

- [ ] **Step 1: Baixar o pack oficial e extrair o logomark Light**

```bash
mkdir -p /tmp/xai-brand && cd /tmp/xai-brand
curl -sL \
  -A "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36" \
  -H "Accept: application/zip,*/*" \
  -H "Referer: https://x.ai/legal/brand-guidelines" \
  -o SpaceXAI_Grok_Assets.zip \
  "https://data.x.ai/logos/SpaceXAI_Grok_Assets.zip"
file SpaceXAI_Grok_Assets.zip
# Expected: Zip archive data, …
unzip -o SpaceXAI_Grok_Assets.zip -d extracted
test -f "extracted/SpaceXAI_Grok_Assets/Grok_Logomark_Light.svg"
```

Expected: zip válido; arquivo `Grok_Logomark_Light.svg` existe.

- [ ] **Step 2: Verificar o conteúdo oficial (baseline)**

```bash
cat "/tmp/xai-brand/extracted/SpaceXAI_Grok_Assets/Grok_Logomark_Light.svg"
```

Expected: SVG com `viewBox="0 0 1024 1024"`, dois `<path … fill="white"/>` (logomark singularity). **Não** deve conter `<text>` nem a letra “G” genérica.

Conteúdo de referência (deve ser idêntico ao baixado — se o pack mudar no futuro, preferir o arquivo do zip sobre este snapshot):

```svg
<svg width="1024" height="1024" viewBox="0 0 1024 1024" fill="none" xmlns="http://www.w3.org/2000/svg">
<path d="M395.479 633.828L735.91 381.105C752.599 368.715 776.454 373.548 784.406 392.792C826.26 494.285 807.561 616.253 724.288 699.996C641.016 783.739 525.151 802.104 419.247 760.277L303.556 814.143C469.49 928.202 670.987 899.995 796.901 773.282C896.776 672.843 927.708 535.937 898.785 412.476L899.047 412.739C857.105 231.37 909.358 158.874 1016.4 10.6326C1018.93 7.11771 1021.47 3.60279 1024 0L883.144 141.651V141.212L395.392 633.916" fill="white"/>
<path d="M325.226 695.251C206.128 580.84 226.662 403.776 328.285 301.668C403.431 226.097 526.549 195.254 634.026 240.596L749.454 186.994C728.657 171.88 702.007 155.623 671.424 144.2C533.19 86.9942 367.693 115.465 255.323 228.382C147.234 337.081 113.244 504.215 171.613 646.833C215.216 753.423 143.739 828.818 71.7385 904.916C46.2237 931.893 20.6216 958.87 0 987.429L325.139 695.339" fill="white"/>
</svg>
```

- [ ] **Step 3: Confirmar o placeholder atual (antes da troca)**

```bash
cat icons/grok-icon.svg
```

Expected (estado antigo): contém algo como `<text …>G</text>` e fill `#c0c9d4`.

- [ ] **Step 4: Substituir o asset no repo**

```bash
cp "/tmp/xai-brand/extracted/SpaceXAI_Grok_Assets/Grok_Logomark_Light.svg" \
   icons/grok-icon.svg
# Byte-identical ao oficial:
cmp -s \
  "/tmp/xai-brand/extracted/SpaceXAI_Grok_Assets/Grok_Logomark_Light.svg" \
  icons/grok-icon.svg && echo IDENTICAL
```

Expected: `IDENTICAL`. **Não** editar fill, viewBox ou paths depois do `cp`.

- [ ] **Step 5: Verificar que o placeholder sumiu**

```bash
rg -n '<text|font-family|>G<' icons/grok-icon.svg && echo "FAIL still placeholder" || echo "OK no text G"
rg -n 'fill="white"' icons/grok-icon.svg
```

Expected: `OK no text G`; pelo menos duas ocorrências de `fill="white"`.

- [ ] **Step 6: Rodar contrato Waybar**

```bash
cargo test waybar_contract
```

Expected: todos os testes do filtro passam, incluindo assert de que o CSS exportado contém `grok-icon.svg`.

- [ ] **Step 7: Commit**

```bash
git add icons/grok-icon.svg
git commit -m "$(cat <<'EOF'
fix: ícone oficial Grok na Waybar

EOF
)"
```

---

### Task 2: Plano de reinstall de assets (doc no commit message / handoff)

**Files:**
- Nenhuma mudança de código adicional se o commit da Task 1 bastar.
- Handoff textual para o usuário (não hand-edit `~/.config/waybar`).

**Interfaces:**
- Consumes: `icons/grok-icon.svg` da Task 1
- Produces: instrução de deploy local

- [ ] **Step 1: Confirmar como o produto instala icons**

```bash
rg -n "install_assets|icons_dir|assets install" src/waybar_contract.rs src/cli.rs docs/commands.md | head -40
```

Expected: `assets install` / setup copiam `icons/` para o waybar dir.

- [ ] **Step 2: Handoff ao usuário (não executar live setup sem pedido)**

Texto a reportar ao usuário após a Task 1:

```
Para ver o logo na barra já instalada, re-copie os icons (ex. via
`agent-bar assets install --waybar-dir …` ou o fluxo de update/setup
que o produto usa) e recarregue a Waybar. Não edite
~/.config/waybar/agent-bar/icons à mão se preferir o fluxo oficial.
```

- [ ] **Step 3: Sem commit extra** se só handoff — a Task 2 é verificação + mensagem, não novo arquivo.

---

## Self-review (plan vs spec)

| Spec requirement | Task |
| --- | --- |
| Substituir por `Grok_Logomark_Light.svg` oficial | Task 1 steps 1–4 |
| Sem alteração de cor/paths | Task 1 step 4 `cmp` + step 5 |
| Nome `grok-icon.svg` / sem mudar CSS-Rust | File map + test waybar_contract |
| Installs existentes re-copiam icons | Task 2 handoff |
| Verificação waybar_contract | Task 1 step 6 |

Placeholder scan: nenhum TBD/TODO. Type consistency: N/A (asset only).
