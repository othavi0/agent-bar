# Codex Model Windows + Configure Models UX Design

Date: 2026-02-19
Status: Approved (brainstorming)
Owner: qbar

## 1. Context

Current qbar behavior:
- `codex` provider returns shared `primary` and `secondary` windows.
- `Configure Models` depends on `quota.models`, so Codex is effectively excluded today.
- UI currently focuses on `% remaining`, without strong generic handling of `5h` + `7d` per model/bucket.

Goal from user:
- Generic quota UX that always surfaces `5h` and `7d` timers.
- If one window is missing for a model/bucket, show `N/A` (never hide by inference).
- Improve UX/UI in `Configure Models` and integrate cleanly into main menu flow.
- Include plan-awareness beyond just `Pro`/`Non-Pro`.

## 2. Validated Requirements

- Always expose `5h` and `7d` in display layer (with fallback `N/A` when absent).
- Use a generic, plan-aware model; do not hardcode behavior to only two plans.
- Recommended product direction chosen: Approach 2 (balanced UX + maintainable complexity).
- `Configure Models` should support:
- model visibility selection
- window policy per provider (`both`, `5h`, `7d`)

## 3. Codex Plan Findings (Research Summary)

Based on OpenAI docs as of 2026-02-19:
- Public Codex plan surfaces include: `Plus`, `Pro`, `Business`, `Enterprise/Edu`, `API Key`.
- There is mention of limited-time access for `Free` and `Go`.
- App Server exposes `account.planType` and quota payloads from `account/rateLimits/read`.
- App Server may provide both:
- `rateLimits` (single-bucket backward-compatible view)
- `rateLimitsByLimitId` (multi-bucket view keyed by `limitId`)

Implementation implication:
- Plan handling must be dynamic and normalized, not binary.
- Quota parsing should prefer multi-bucket payload when present.

## 4. Options Considered

1. Basic:
- Improve hints only.
- Fast but low UX value.

2. Recommended:
- Two-step Configure Models flow:
- model visibility
- window policy
- Plan badge + preview.
- Good UX with low-to-medium complexity.

3. Advanced:
- Per-model policy overrides.
- Powerful but over-complex for current scope.

Decision: Option 2.

## 5. Architecture Design

## 5.1 Provider Layer (`src/providers/codex.ts`)

- Extend app-server parsing to include:
- `account.planType`
- `rateLimitsByLimitId` when available
- fallback to existing `rateLimits`
- Build internal per-bucket window shape from quota payload:
- `5h` candidate: ~300 min windows
- `7d` candidate: ~10080 min windows
- preserve unknown durations under `other` bucket data (do not discard)
- Keep legacy `primary`/`secondary` fields for compatibility with existing renderers.

## 5.2 Types (`src/providers/types.ts`)

- Keep existing `ProviderQuota.primary` and `ProviderQuota.secondary`.
- Add optional provider-agnostic model window shape, for example:
- `modelsDetailed?: Record<string, { fiveHour?: QuotaWindow; sevenDay?: QuotaWindow; other?: QuotaWindow[] }>`
- Add optional `planType?: string` at `ProviderQuota` level for cross-UI display.

## 5.3 Settings (`src/settings.ts`)

- Keep `models` behavior unchanged (empty array means show all).
- Add per-provider window policy:
- `windowPolicy?: Record<string, "both" | "five_hour" | "seven_day">`
- Default policy for Codex: `"both"`.

## 6. UX/UI Design (Configure Models)

Target file: `src/tui/configure-models.ts`

Flow:
1. Provider select:
- Include Codex with plan badge/hint.
2. Window policy select (new):
- `Always show 5h + 7d (N/A fallback)` (default)
- `Show only 5h`
- `Show only 7d`
3. Model multiselect (improved hints):
- Hint format: `5h: 72% (2h 10m) | 7d: N/A`
- Sort by criticality first (lowest remaining), then name.
4. Save summary:
- Show chosen policy + number of selected models + detected plan.

UX principles:
- Stable, predictable display for every plan.
- No hidden limits due to absent fields.
- Preserve existing keyboard and clack prompt patterns used by qbar.

## 7. Rendering Rules

Targets:
- `src/formatters/waybar.ts`
- `src/formatters/terminal.ts`
- `src/tui/list-all.ts`

Rules:
- Apply `windowPolicy` at render time.
- If policy is `both`, render `5h` + `7d` columns/labels always.
- Missing window => explicit `N/A` for eta/reset.
- Keep current color thresholds and style language.

## 8. Error Handling

- App-server read failure => fallback to session JSONL parser.
- Missing `planType` => `Unknown`.
- Missing `resetsAt` => `N/A`.
- Unknown window durations remain available in data (`other`) for diagnostics/future UX.

## 9. Testing Strategy

Unit:
- normalize/alias `planType` values (including legacy names if present).
- map `windowDurationMins` to `5h`, `7d`, `other`.
- apply `windowPolicy` correctly.

Integration:
- `Configure Models` persists both model selection and window policy.
- Render output shows `N/A` where window absent.

Regression:
- Non-Codex providers remain unchanged.
- Legacy settings file without `windowPolicy` still works.

## 10. Rollout Plan

1. Add data model/types + backward compatibility.
2. Add Codex provider parsing for multi-bucket + plan type.
3. Add Configure Models policy step and improved hints.
4. Apply policy in all renderers.
5. Verify with local manual flow and command output.

## 11. Local Application Runbook (Post-Implementation)

1. Update code in provider/types/settings/tui/formatters.
2. Run project checks/tests (`bun test` or repository test command).
3. Open menu: `bun src/cli.ts menu` and validate `Configure Models`.
4. Validate terminal output: `qbar status`.
5. Validate Waybar tooltip: run `qbar` and inspect hover details.
6. Commit and deploy locally via normal qbar update/install flow.

## 12. References

- https://developers.openai.com/codex/pricing
- https://developers.openai.com/codex/app-server
- https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan
- https://github.com/openai/codex
