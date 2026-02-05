#!/bin/bash
# Returns icon path for a given LLM provider (for waybar image module)
# Usage: waybar-llm-icon.sh <claude|codex|ag>

PROVIDER="$1"
CACHE="/tmp/waybar-llm-${PROVIDER}.json"

if [ -f "$CACHE" ]; then
  AVAILABLE=$(jq -r '.available // false' "$CACHE" 2>/dev/null)
  ICON=$(jq -r '.icon // ""' "$CACHE" 2>/dev/null)
  if [ "$AVAILABLE" = "true" ] && [ -n "$ICON" ] && [ -f "$ICON" ]; then
    echo "$ICON"
  fi
fi
