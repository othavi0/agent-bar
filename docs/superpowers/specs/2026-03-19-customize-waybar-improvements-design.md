# Customize Waybar — Improvements Design

**Date:** 2026-03-19
**Scope:** `src/tui/configure-layout.ts`, `src/waybar-integration.ts`

## Context

The unified "Customize Waybar" menu was created by merging the old "Configure Waybar" and "Customize Layout" menus. Three issues remain:

1. Users are forced to go through all 3 steps even when they only want to change one thing.
2. Provider order changes are not applied to the Waybar config (`modules-right` array).
3. After applying, the Waybar bar takes a moment to reload but the spinner stops immediately.

## Changes

### 1. Skip per step — "Keep current order" option

**File:** `src/tui/configure-layout.ts`

- Steps 1 (Providers) and 3 (Style) already support "keep current" naturally — pressing Enter without changing the multiselect/select preserves the current value.
- Step 2 (Order) needs an explicit skip. Add a `p.confirm` before the positional selects:
  - Message: `"Change display order? (currently: Claude → Codex → Amp)"`
  - Default: `false` (pressing Enter skips)
  - If declined, keep `settings.waybar.providerOrder` as-is (filtered to selected providers).
  - If accepted, proceed with the positional select flow.
- This replaces the current `p.note` + automatic select flow that always runs.

### 2. Fix: Provider order not applied to Waybar config

**File:** `src/waybar-integration.ts`

**Root cause:** `ensureModulesRight()` (line 190) merges modules into the `modules-right` array using an append-if-missing strategy. It never reorders existing entries. When a user changes order from `[claude, codex, amp]` to `[amp, claude, codex]`, all IDs already exist so nothing changes.

**Fix:** After the merge step, reorder the qbar module IDs within the array to match the desired `providerOrder`. Non-qbar entries in `modules-right` remain untouched in their original positions. The qbar entries are extracted, reordered to match `moduleIDs`, and placed back at the positions where qbar entries originally were (or appended at the end if new).

Specifically, update `ensureModulesRight` to:
1. Merge missing IDs (existing behavior).
2. Extract the indices where qbar modules appear in the merged array.
3. Place the `moduleIDs` (in the correct order) at those indices.
4. Compare result with input — mark as `changed` if different.

### 3. Waybar reload delay with spinner

**Files:** `src/tui/configure-layout.ts`, `src/tui/configure-waybar.ts` (deleted, N/A)

After sending `pkill -SIGUSR2 waybar`, keep the spinner running for **2 seconds** with text `"Waiting for Waybar to reload..."` before stopping. This gives Waybar time to process the signal and re-render modules.

**Implementation:** Add `await new Promise(r => setTimeout(r, 2000))` after the `Bun.spawn` call, before `s.stop()`.

This delay only applies in the TUI interactive flows (`configure-layout.ts`). The CLI `apply-local` and `setup` commands keep the current instant-stop behavior since they are non-interactive.

## Files Changed

| File | Change |
|------|--------|
| `src/tui/configure-layout.ts` | Add skip for step 2, add 2s delay after reload |
| `src/waybar-integration.ts` | Fix `ensureModulesRight` to reorder existing qbar modules |

## Testing

- `bun run typecheck` — must pass
- `bun test` — all 74 tests must pass
- Manual: run `qbar menu`, enter Customize Waybar, skip steps, verify order is applied
