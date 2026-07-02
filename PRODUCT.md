# Product

## Register

product

## Users

Usuários de Waybar em Linux (Hyprland/Omarchy e afins) que trabalham com CLIs de
agentes LLM (Claude Code, Codex, Amp) o dia inteiro. Vivem no terminal, leem a
barra de relance dezenas de vezes por dia e abrem o menu TUI quando precisam de
mais contexto: "quanto me resta, quando reseta, quanto gastei". A janela do menu
é um terminal flutuante de tamanho variável — o layout precisa ser responsivo a
larguras de ~80 a 200+ colunas.

## Product Purpose

Monitor de quotas LLM para Waybar. O módulo da barra responde "posso usar
agora?" em um piscar; o menu TUI (`agent-bar menu`) é o dashboard completo:
quota por janela (sessão/semana/por-modelo), custo, histórico de uso, estado de
login por provider e configuração do módulo Waybar. Sucesso = o usuário confia
nos números (dado real, nunca placeholder), entende severidade sem ler texto, e
nunca vê a UI congelar.

## Brand Personality

Vibrante, hacker, expressivo. Densidade de informação estilo btop com
personalidade: cor semântica viva, gráficos braille densos, números hero com
peso visual. A energia vem dos dados reais se movendo — não de decoração.

## Anti-references

- A TUI atual (pré-redesign): 60–80% da tela vazia, barras binárias sem
  gradação, sparkline placeholder hardcoded, status de login contraditório.
- Dashboards SaaS genéricos: hero-metric + cards idênticos + neutralidade cinza.
- TUIs "form-like" (dialog/whiptail): telas que parecem instalador dos anos 90.

## Design Principles

1. **Dado real ou nada.** Nenhum elemento visual sem dado por trás; estado
   vazio/carregando é desenhado de propósito, nunca placeholder fingindo dado.
2. **Severidade pela cor, identidade pelo provider.** Verde→âmbar→vermelho
   comunica urgência; laranja/verde/magenta identificam Claude/Codex/Amp.
   Nunca cor arbitrária.
3. **Denso por padrão, responsivo por contrato.** Cada célula da janela
   trabalha; mais largura = mais resolução de dado (não bordas esticadas).
4. **A barra ensina, o menu aprofunda.** O menu nunca contradiz a barra
   (mesma fonte de dados, mesma severidade, mesmos números).
5. **Feedback em todo IO.** Fetch, login e save sempre têm estado visível
   (spinner/skeleton) — a TUI nunca congela sem explicação.

## Accessibility & Inclusion

- Contraste ≥4.5:1 dos textos sobre o fundo do terminal (One Dark evoluído).
- Severidade nunca só por cor: acompanha símbolo/percentual textual.
- `NO_COLOR` respeitado (contrato existente do render_ansi).
- Terminal do usuário controla a fonte; nada depende de nerd-font além dos
  glifos já usados pelo contrato Waybar.
