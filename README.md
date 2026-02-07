# qbar

Monitor de quota de LLMs para Waybar.

Mostra o uso restante de **Claude**, **Codex** e **Antigravity** direto na sua barra.

## Instalação

```bash
# Clona o repositório
git clone https://github.com/othavioquiliao/qbar.git
cd qbar

# Instala as dependências do projeto
bun install

# Configura tudo automaticamente (copia ícones, edita waybar config/css, cria symlink)
bun src/setup.ts
```

Pronto. Os módulos aparecem na Waybar.

## Uso

| Ação | Descrição |
|------|-----------|
| **Hover** | Mostra tooltip com detalhes de quota |
| **Click esquerdo** | Abre menu interativo |
| **Click direito** | Refresh (ou login se desconectado) |

### Comandos

```bash
qbar              # Output JSON para Waybar
qbar status       # Mostra quotas no terminal
qbar menu         # Menu interativo
qbar setup        # (Re)configura Waybar automaticamente
```

## Login dos Providers

Use `qbar menu` → **Provider login**. O qbar instala as CLIs automaticamente via `yay`:

| Provider | O que faz |
|----------|-----------|
| Claude | Usa sua conta do Claude.ai (claude-code CLI) |
| Codex | Usa sua conta do OpenAI Codex (codex CLI) |
| Antigravity | Usa Google OAuth (antigravity-usage) |

## Cores

| Quota restante | Cor |
|----------------|-----|
| ≥60% | 🟢 Verde |
| ≥30% | 🟡 Amarelo |
| ≥10% | 🟠 Laranja |
| <10% | 🔴 Vermelho |

## Troubleshooting

**Waybar não inicia após setup?**
```bash
# Restaura backup (criado automaticamente)
ls ~/.config/waybar/*.qbar-backup-*
cp ~/.config/waybar/config.jsonc.qbar-backup-XXXXX ~/.config/waybar/config.jsonc
```

**Provider mostra ícone de desconectado (󱘖)?**
- Click direito no módulo para iniciar o login

**Refresh não atualiza valor?**
- O cache dura 2 minutos. Click direito força refresh imediato.

## Arquitetura

```
~/.config/waybar/
├── config.jsonc          # Módulos qbar-claude, qbar-codex, qbar-antigravity
├── style.css             # Estilos e cores dos módulos
├── qbar/icons/           # Ícones PNG dos providers
└── scripts/
    └── qbar-open-terminal  # Helper para abrir terminal flutuante

~/.config/qbar/
└── settings.json         # Preferências do usuário

~/.config/waybar/qbar/cache/
└── *.json                # Cache de quotas (TTL 2min)
```

## License

MIT
