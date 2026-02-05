#!/bin/bash
# Returns colored percentage for a given LLM provider (for waybar custom module)
# Usage: waybar-llm-pct.sh <claude|codex|ag>

PROVIDER="$1"
CACHE="/tmp/waybar-llm-${PROVIDER}.json"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UPDATE_LOCK="/tmp/waybar-llm-update.lock"
REFERENCE_CACHE="/tmp/waybar-llm-claude.json"
STALE_SEC=55

# Trigger update if cache is stale (flock prevents concurrent runs)
needs_update=false
if [ ! -f "$CACHE" ]; then
  needs_update=true
elif [ ! -f "$REFERENCE_CACHE" ]; then
  needs_update=true
elif [ $(( $(date +%s) - $(stat -c %Y "$REFERENCE_CACHE" 2>/dev/null || echo 0) )) -gt $STALE_SEC ]; then
  needs_update=true
fi

if [ "$needs_update" = "true" ]; then
  (
    flock -n 200 || exit 0
    "$SCRIPT_DIR/waybar-llm-usage.sh" > /dev/null 2>&1
  ) 200>"$UPDATE_LOCK"
fi

# Read cache and output
if [ -f "$CACHE" ]; then
  AVAILABLE=$(jq -r '.available // false' "$CACHE" 2>/dev/null)
  if [ "$AVAILABLE" = "true" ]; then
    PCT=$(jq -r '.pct // "?"' "$CACHE" 2>/dev/null)
    COLOR=$(jq -r '.color // "#cdd6f4"' "$CACHE" 2>/dev/null)
    TOOLTIP=$(jq -r '.tooltip // ""' "$CACHE" 2>/dev/null)
    TEXT_JSON=$(printf "%s" "<span foreground='${COLOR}'>${PCT}%</span>" | jq -Rs .)
    TIP_JSON=$(printf "%s" "$TOOLTIP" | jq -Rs .)
    echo "{\"text\": ${TEXT_JSON}, \"tooltip\": ${TIP_JSON}}"
  else
    echo '{"text": ""}'
  fi
else
  echo '{"text": ""}'
fi
