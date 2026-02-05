#!/bin/bash
# LLM Usage Waybar Script — Claude + Codex + Antigravity

export PATH="$HOME/.cache/.bun/bin:$PATH"
export CODEX_BIN="$HOME/.cache/.bun/bin/codex"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
AG_FETCH="$SCRIPT_DIR/antigravity-waybar-usage-fetch"

# --- Helpers ---
color_for() {
  local pct="$1"
  if [ "$pct" = "?" ]; then
    echo "#cdd6f4"
  elif [ "$pct" -ge 60 ] 2>/dev/null; then
    echo "#a6e3a1"
  elif [ "$pct" -ge 30 ] 2>/dev/null; then
    echo "#f9e2af"
  elif [ "$pct" -ge 10 ] 2>/dev/null; then
    echo "#fab387"
  else
    echo "#f38ba8"
  fi
}

fmt_span() {
  local label="$1"; local pct="$2"; local color
  color=$(color_for "$pct")
  echo "<span foreground='${color}'>${label} ${pct}%</span>"
}

bar_5pct() {
  local pct="$1"
  if [ "$pct" = "?" ]; then
    echo "░░░░░░░░░░░░░░░░░░░░"
    return
  fi
  local filled=$((pct / 5))
  local empty=$((20 - filled))
  local color
  color=$(color_for "$pct")
  local filled_str="" empty_str=""
  if [ "$filled" -gt 0 ]; then
    filled_str=$(printf "%0.s█" $(seq 1 $filled))
  fi
  if [ "$empty" -gt 0 ]; then
    empty_str=$(printf "%0.s░" $(seq 1 $empty))
  fi
  echo "<span foreground='${color}'>${filled_str}</span><span foreground='#6c7086'>${empty_str}</span>"
}

human_eta() {
  local iso="$1"
  if [ -z "$iso" ] || [ "$iso" = "?" ]; then
    echo "?"; return
  fi
  local now=$(date +%s)
  local ts=$(date -d "$iso" +%s 2>/dev/null)
  if [ -z "$ts" ]; then echo "?"; return; fi
  local diff=$((ts - now))
  if [ $diff -lt 0 ]; then echo "0m"; return; fi
  local d=$((diff / 86400))
  local h=$(((diff % 86400) / 3600))
  local m=$(((diff % 3600) / 60))
  if [ $d -gt 0 ]; then
    printf "%dd %02dh" "$d" "$h"
  else
    printf "%dh %02dm" "$h" "$m"
  fi
}

# --- Claude (OAuth) ---
CLAUDE_CREDS="$HOME/.claude/.credentials.json"
C5_REM="?"; C5_RESET="?"; C5_RESET_ISO=""
C7_REM="?"; C7_RESET="?"; C7_RESET_ISO=""
C_PLAN="?"

if [ -f "$CLAUDE_CREDS" ]; then
  ACCESS_TOKEN=$(jq -r '.claudeAiOauth.accessToken // empty' "$CLAUDE_CREDS" 2>/dev/null)
  C_PLAN=$(jq -r '.claudeAiOauth.subscriptionType // "free"' "$CLAUDE_CREDS" 2>/dev/null)
  if [ -n "$ACCESS_TOKEN" ]; then
    USAGE=$(curl -s --max-time 5 "https://api.anthropic.com/api/oauth/usage" \
      -H "Authorization: Bearer $ACCESS_TOKEN" \
      -H "anthropic-beta: oauth-2025-04-20" 2>/dev/null)
    if echo "$USAGE" | jq -e '.error.error_code=="token_expired"' >/dev/null 2>&1; then
      C_PLAN="token expired"
    elif echo "$USAGE" | jq -e '.five_hour' &>/dev/null; then
      C5_USED=$(echo "$USAGE" | jq -r '.five_hour.utilization // 0' | xargs printf "%.0f")
      C7_USED=$(echo "$USAGE" | jq -r '.seven_day.utilization // 0' | xargs printf "%.0f")
      C5_REM=$((100 - C5_USED))
      C7_REM=$((100 - C7_USED))
      C5_RESET_ISO=$(echo "$USAGE" | jq -r '.five_hour.resets_at // ""')
      C7_RESET_ISO=$(echo "$USAGE" | jq -r '.seven_day.resets_at // ""')
      if [ -n "$C5_RESET_ISO" ] && [ "$C5_RESET_ISO" != "null" ]; then
        C5_RESET=$(date -d "$C5_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")
      else
        # rolling window: assume 5h from now for UX
        C5_RESET_ISO=$(date -d "+5 hours" -Iseconds)
        C5_RESET=$(date -d "$C5_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")
      fi
      if [ -n "$C7_RESET_ISO" ] && [ "$C7_RESET_ISO" != "null" ]; then
        C7_RESET=$(date -d "$C7_RESET_ISO" +"%d/%m" 2>/dev/null || echo "?")
      else
        C7_RESET="?"
        C7_RESET_ISO=""
      fi
    fi
  fi
fi

# --- Codex (codex-quota) ---
X5_REM="?"; X5_RESET="?"; X5_RESET_ISO=""
X7_REM="?"; X7_RESET="?"; X7_RESET_ISO=""
CODEX_CACHE="/tmp/codex-quota.json"
CODEX_REFRESH_SEC=120

if [ ! -f "$CODEX_CACHE" ] || [ $(( $(date +%s) - $(stat -c %Y "$CODEX_CACHE" 2>/dev/null || echo 0) )) -gt $CODEX_REFRESH_SEC ]; then
  CODEX_JSON=$(python "$SCRIPT_DIR/codex-quota.py" --json --fresh 2>/dev/null)
  if echo "$CODEX_JSON" | jq -e '.primary' >/dev/null 2>&1; then
    echo "$CODEX_JSON" > "$CODEX_CACHE"
  else
    CODEX_JSON=""
  fi
fi

if [ -f "$HOME/.codex/auth.json" ] && [ -f "$CODEX_CACHE" ]; then
  CODEX_JSON=$(cat "$CODEX_CACHE")
fi

if [ -n "$CODEX_JSON" ]; then
  HAS_PRIMARY=$(echo "$CODEX_JSON" | jq -r 'has("primary")')
  HAS_SECONDARY=$(echo "$CODEX_JSON" | jq -r 'has("secondary")')
  if [ "$HAS_PRIMARY" = "true" ] && [ "$HAS_SECONDARY" = "true" ]; then
    P_USED=$(echo "$CODEX_JSON" | jq -r '.primary.used_percent // 0' 2>/dev/null | xargs printf "%.0f")
    S_USED=$(echo "$CODEX_JSON" | jq -r '.secondary.used_percent // 0' 2>/dev/null | xargs printf "%.0f")
    X5_REM=$((100 - P_USED))
    X7_REM=$((100 - S_USED))
    X5_RESET_ISO=$(echo "$CODEX_JSON" | jq -r '.primary.resets_at // ""' 2>/dev/null)
    X7_RESET_ISO=$(echo "$CODEX_JSON" | jq -r '.secondary.resets_at // ""' 2>/dev/null)
    [ -n "$X5_RESET_ISO" ] && X5_RESET=$(date -d "$X5_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")
    [ -n "$X7_RESET_ISO" ] && X7_RESET=$(date -d "$X7_RESET_ISO" +"%d/%m" 2>/dev/null || echo "?")
  else
    X5_REM="?"; X7_REM="?"; X5_RESET="?"; X7_RESET="?"; X5_RESET_ISO=""; X7_RESET_ISO=""
  fi
fi

# --- Antigravity (LSP) ---
AG_CLAUDE="?"; AG_GEM_PRO="?"; AG_GEM_FLASH="?"
AG_CLAUDE_RESET="?"; AG_GEM_PRO_RESET="?"; AG_GEM_FLASH_RESET="?"
AG_CLAUDE_RESET_ISO=""; AG_GEM_PRO_RESET_ISO=""; AG_GEM_FLASH_RESET_ISO=""
AG_ACCOUNT="?"
AG_PROCESS=$(ps aux | grep "language_server_linux" | grep -v grep | head -1)
if [ -n "$AG_PROCESS" ]; then
  AG_CSRF=$(echo "$AG_PROCESS" | grep -oP '(?<=--csrf_token )[a-f0-9-]+')
  if [ -n "$AG_CSRF" ]; then
    for port in $(lsof -nP -iTCP -sTCP:LISTEN 2>/dev/null | grep "language_" | awk '{print $9}' | cut -d: -f2 | head -3); do
      AG_JSON=$(curl -sk --max-time 3 "https://127.0.0.1:$port/exa.language_server_pb.LanguageServerService/GetUserStatus" \
        -H "X-Codeium-Csrf-Token: $AG_CSRF" \
        -H "Connect-Protocol-Version: 1" \
        -H "Content-Type: application/json" \
        -d '{"metadata":{"ideName":"antigravity"}}' 2>/dev/null)
      if echo "$AG_JSON" | jq -e '.userStatus.email' &>/dev/null; then
        AG_ACCOUNT=$(echo "$AG_JSON" | jq -r '.userStatus.name // .userStatus.email' | cut -d' ' -f1)
        AG_CLAUDE=$(echo "$AG_JSON" | jq -r '[.userStatus.cascadeModelConfigData.clientModelConfigs[] | select(.label | test("Claude Opus.*Thinking"; "i")) | .quotaInfo.remainingFraction // 1] | .[0] * 100 | floor')
        AG_GEM_PRO=$(echo "$AG_JSON" | jq -r '[.userStatus.cascadeModelConfigData.clientModelConfigs[] | select(.label | test("Gemini 3 Pro.*High"; "i")) | .quotaInfo.remainingFraction // 1] | .[0] * 100 | floor')
        AG_GEM_FLASH=$(echo "$AG_JSON" | jq -r '[.userStatus.cascadeModelConfigData.clientModelConfigs[] | select(.label | test("Gemini 3 Flash"; "i")) | .quotaInfo.remainingFraction // 1] | .[0] * 100 | floor')
        AG_CLAUDE_RESET_ISO=$(echo "$AG_JSON" | jq -r '.userStatus.cascadeModelConfigData.clientModelConfigs[] | select(.label | test("Claude Opus.*Thinking"; "i")) | .quotaInfo.resetTime // ""' | head -1)
        AG_GEM_PRO_RESET_ISO=$(echo "$AG_JSON" | jq -r '.userStatus.cascadeModelConfigData.clientModelConfigs[] | select(.label | test("Gemini 3 Pro.*High"; "i")) | .quotaInfo.resetTime // ""' | head -1)
        AG_GEM_FLASH_RESET_ISO=$(echo "$AG_JSON" | jq -r '.userStatus.cascadeModelConfigData.clientModelConfigs[] | select(.label | test("Gemini 3 Flash"; "i")) | .quotaInfo.resetTime // ""' | head -1)
        [ -n "$AG_CLAUDE_RESET_ISO" ] && AG_CLAUDE_RESET=$(date -d "$AG_CLAUDE_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")
        [ -n "$AG_GEM_PRO_RESET_ISO" ] && AG_GEM_PRO_RESET=$(date -d "$AG_GEM_PRO_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")
        [ -n "$AG_GEM_FLASH_RESET_ISO" ] && AG_GEM_FLASH_RESET=$(date -d "$AG_GEM_FLASH_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")
        break
      fi
    done
  fi
fi

# --- Bar Text ---
Cld=$(fmt_span "Cld" "$C5_REM")
Cdx=$(fmt_span "Cdx" "$X5_REM")
SEP="<span weight='bold'>·</span>"

# --- Tooltip ---
line_fmt() {
  local label="$1"; local pct="$2"; local reset_iso="$3"; local reset_hm="$4"
  local bar eta
  bar=$(bar_5pct "$pct")
  eta=$(human_eta "$reset_iso")
  printf "%-14.14s %s - %3s%% - %-7s (%s)" "$label" "$bar" "$pct" "$eta" "$reset_hm"
}

T=""

if [ "$C5_REM" != "?" ] || [ "$C7_REM" != "?" ]; then
  T+="━━━ Claude (${C_PLAN}) ━━━\\n"
  if [ "$C_PLAN" = "token expired" ]; then
    T+="⚠️ Token expirou. Faça login no Claude CLI.\\n"
  fi
  T+="$(line_fmt "Claude 4.5 Opus" "$C5_REM" "$C5_RESET_ISO" "$C5_RESET")\\n"
  T+="$(line_fmt "Weekly" "$C7_REM" "$C7_RESET_ISO" "$C7_RESET")\\n"
fi

if [ "$X5_REM" != "?" ] || [ "$X7_REM" != "?" ]; then
  if [ -n "$T" ]; then T+="\\n"; fi
  T+="━━━ Codex ━━━\\n"
  T+="$(line_fmt "Codex 5h" "$X5_REM" "$X5_RESET_ISO" "$X5_RESET")\\n"
  T+="$(line_fmt "Codex 7d" "$X7_REM" "$X7_RESET_ISO" "$X7_RESET")\\n"
fi

# Fallback to cloud if local failed
if [ "$AG_ACCOUNT" = "?" ]; then
  AG_CLOUD=$($AG_FETCH 2>/dev/null)
  if echo "$AG_CLOUD" | jq -e '.models' >/dev/null 2>&1; then
    AG_ACCOUNT=$(echo "$AG_CLOUD" | jq -r '.email // .accountEmail // "?"')
    AG_CLAUDE=$(echo "$AG_CLOUD" | jq -r '.models[]? | select(.label | test("Claude"; "i")) | .remainingPercentage // empty' | head -1)
    AG_GEM_PRO=$(echo "$AG_CLOUD" | jq -r '.models[]? | select(.label | test("Gemini.*Pro"; "i")) | .remainingPercentage // empty' | head -1)
    AG_GEM_FLASH=$(echo "$AG_CLOUD" | jq -r '.models[]? | select(.label | test("Gemini.*Flash"; "i")) | .remainingPercentage // empty' | head -1)
    AG_CLAUDE_RESET_ISO=$(echo "$AG_CLOUD" | jq -r '.models[]? | select(.label | test("Claude"; "i")) | .resetTime // ""' | head -1)
    AG_GEM_PRO_RESET_ISO=$(echo "$AG_CLOUD" | jq -r '.models[]? | select(.label | test("Gemini.*Pro"; "i")) | .resetTime // ""' | head -1)
    AG_GEM_FLASH_RESET_ISO=$(echo "$AG_CLOUD" | jq -r '.models[]? | select(.label | test("Gemini.*Flash"; "i")) | .resetTime // ""' | head -1)
  elif echo "$AG_CLOUD" | jq -e '.snapshot' >/dev/null 2>&1; then
    AG_ACCOUNT=$(echo "$AG_CLOUD" | jq -r '.email // .accountEmail // "?"')
    # Map snapshot models if present
    AG_CLAUDE=$(echo "$AG_CLOUD" | jq -r '.snapshot.models[]? | select(.label | test("Claude"; "i")) | .remainingPercentage // empty' | head -1)
    AG_GEM_PRO=$(echo "$AG_CLOUD" | jq -r '.snapshot.models[]? | select(.label | test("Gemini.*Pro"; "i")) | .remainingPercentage // empty' | head -1)
    AG_GEM_FLASH=$(echo "$AG_CLOUD" | jq -r '.snapshot.models[]? | select(.label | test("Gemini.*Flash"; "i")) | .remainingPercentage // empty' | head -1)
    AG_CLAUDE_RESET_ISO=$(echo "$AG_CLOUD" | jq -r '.snapshot.models[]? | select(.label | test("Claude"; "i")) | .resetTime // ""' | head -1)
    AG_GEM_PRO_RESET_ISO=$(echo "$AG_CLOUD" | jq -r '.snapshot.models[]? | select(.label | test("Gemini.*Pro"; "i")) | .resetTime // ""' | head -1)
    AG_GEM_FLASH_RESET_ISO=$(echo "$AG_CLOUD" | jq -r '.snapshot.models[]? | select(.label | test("Gemini.*Flash"; "i")) | .resetTime // ""' | head -1)
  fi
fi

# normalize cloud percentages (0-1 -> 0-100)
normalize_pct() {
  local v="$1"
  if [ -z "$v" ] || [ "$v" = "?" ]; then
    echo "$v"; return
  fi
  if awk "BEGIN{exit !($v<=1)}" 2>/dev/null; then
    awk "BEGIN{printf \"%.0f\", $v*100}"
  else
    awk "BEGIN{printf \"%.0f\", $v}"
  fi
}
AG_CLAUDE=$(normalize_pct "$AG_CLAUDE")
AG_GEM_PRO=$(normalize_pct "$AG_GEM_PRO")
AG_GEM_FLASH=$(normalize_pct "$AG_GEM_FLASH")
[ -n "$AG_CLAUDE_RESET_ISO" ] && AG_CLAUDE_RESET=$(date -d "$AG_CLAUDE_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")
[ -n "$AG_GEM_PRO_RESET_ISO" ] && AG_GEM_PRO_RESET=$(date -d "$AG_GEM_PRO_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")
[ -n "$AG_GEM_FLASH_RESET_ISO" ] && AG_GEM_FLASH_RESET=$(date -d "$AG_GEM_FLASH_RESET_ISO" +"%H:%M" 2>/dev/null || echo "?")

if [ "$AG_ACCOUNT" != "?" ]; then
  T+="\\n━━━ Antigravity (${AG_ACCOUNT}) ━━━\\n"
  T+="$(line_fmt "Claude Opus" "$AG_CLAUDE" "$AG_CLAUDE_RESET_ISO" "$AG_CLAUDE_RESET")\\n"
  T+="$(line_fmt "Gemini Pro" "$AG_GEM_PRO" "$AG_GEM_PRO_RESET_ISO" "$AG_GEM_PRO_RESET")\\n"
  T+="$(line_fmt "Gemini Flash" "$AG_GEM_FLASH" "$AG_GEM_FLASH_RESET_ISO" "$AG_GEM_FLASH_RESET")"
fi

# finalize bar text after AG cloud fallback
HAS_CLAUDE=""
HAS_CODEX=""
HAS_AG=""
[ "$C5_REM" != "?" ] && HAS_CLAUDE=1
[ "$X5_REM" != "?" ] && HAS_CODEX=1
[ "$AG_ACCOUNT" != "?" ] && HAS_AG=1

TEXT=""
if [ -n "$HAS_CLAUDE" ]; then
  TEXT="$Cld"
fi
if [ -n "$HAS_CODEX" ]; then
  if [ -n "$TEXT" ]; then TEXT+=" ${SEP} "; fi
  TEXT+="$Cdx"
fi
if [ -n "$HAS_AG" ]; then
  AG=$(fmt_span "AG" "$AG_CLAUDE")
  if [ -n "$TEXT" ]; then TEXT+=" ${SEP} "; fi
  TEXT+="$AG"
fi

if [ -n "$TEXT" ]; then
  TEXT="| ${TEXT} |"
else
  TEXT="| <span foreground='#cdd6f4'>Connect to Provider</span> |"
fi

# convert literal \n to real newlines for tooltip rendering
T=${T//\\n/$'\n'}

# --- Per-provider cache files (for icon-based waybar modules) ---
ICON_DIR="$HOME/.config/waybar/icons"
FULL_TIP_JSON=$(printf "%s" "$T" | jq -Rs .)

write_provider_cache() {
  local file="$1" pct="$2" color="$3" available="$4" icon="$5"
  jq -n \
    --arg pct "$pct" \
    --arg color "$color" \
    --argjson available "$available" \
    --arg icon "$icon" \
    --argjson tooltip "$FULL_TIP_JSON" \
    '{pct:$pct, color:$color, available:$available, icon:$icon, tooltip:$tooltip}' > "$file"
}

if [ -n "$HAS_CLAUDE" ]; then
  write_provider_cache "/tmp/waybar-llm-claude.json" "$C5_REM" "$(color_for "$C5_REM")" "true" "$ICON_DIR/claude-code-icon.png"
else
  write_provider_cache "/tmp/waybar-llm-claude.json" "?" "#cdd6f4" "false" ""
fi

if [ -n "$HAS_CODEX" ]; then
  write_provider_cache "/tmp/waybar-llm-codex.json" "$X5_REM" "$(color_for "$X5_REM")" "true" "$ICON_DIR/codex-icon.png"
else
  write_provider_cache "/tmp/waybar-llm-codex.json" "?" "#cdd6f4" "false" ""
fi

if [ -n "$HAS_AG" ]; then
  write_provider_cache "/tmp/waybar-llm-ag.json" "$AG_CLAUDE" "$(color_for "$AG_CLAUDE")" "true" "$ICON_DIR/antigravity-icon.png"
else
  write_provider_cache "/tmp/waybar-llm-ag.json" "?" "#cdd6f4" "false" ""
fi

# --- Legacy single-module output (backward compatible) ---
TEXT_JSON=$(printf "%s" "$TEXT" | jq -Rs .)
TIP_JSON=$FULL_TIP_JSON
echo "{\"text\": $TEXT_JSON, \"tooltip\": $TIP_JSON, \"class\": \"codexbar\"}"
