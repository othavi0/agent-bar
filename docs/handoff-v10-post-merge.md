# Handoff: Agent Bar v10 after PR #25 merge

**Date:** 2026-07-27  
**Audience:** Next implementer / release owner (Grok, Codex, or human)  
**Merge:** [PR #25](https://github.com/othavi0/agent-bar/pull/25) → `master`  
**Merge commit:** `005879d72d513d500520d4da3bebce4948a370b4`  
**Feature tip included:** `986c4c5` (`fix: polish popup rail and usage bars`)

Default branch is **`master`** (not `main`).

---

## 1. What is done (do not redo)

### Product / code (v10 on master)

- Product is **only** the Omarchy Quattro plugin `agent-bar.usage` + private helper `bin/agent-bar`.
- Rust/Cargo + QML only; no Node product surface.
- Schema-v2 status; settings-v1; no schema-v1 / TUI / Waybar / history / money.
- Shared `Service.qml` + per-monitor `BarWidget.qml`.
- Consolidated popup: rail, provider view, settings, maintenance.
- Live-proven (on user machine, post-polish commits):
  - Chips with real percents/states on dual monitors.
  - Popup opens on left/right click.
  - Rail option-A stack, bordered strip, neutral selection, settings slot.
  - Usage progress tracks (Amp Daily, Grok Context, etc.).
  - Content-fit height; Flickable only when overflowing.
  - Foreign-monitor dismiss layer + `Service.dismissPopup()`.
  - Same-monitor outside-click via KeyboardPanel (platform pattern).

### Process already completed

| Step | Status |
| --- | --- |
| Implementation Tasks through live QA polish | Done on `feat/quickshell-native-v10` |
| CI-style gates (`fmt` / `test` / `clippy` / `diff --check`) | Passed at polish commit |
| PR opened and body updated | #25 |
| **Merge to master** | **Done by user** |
| Dual RC at `b9475bf` + tag/publish `v10.0.0` | **Done** (assets on GitHub Release) |
| `publish.yml` checksum CWD fix | **Done** (PR #26) |
| Live re-evidence TEST-035…042 post-release | **Done** — [report](qa/v10.0.0-live-qa-2026-07-27.md) |

### Canonical docs for this slice

| Doc | Use |
| --- | --- |
| `docs/specs/v10/README.md` | Reading order for full v10 contract |
| `docs/specs/v10/08-plugin-bundle-and-release.md` | Bundle, install, update, release |
| `docs/specs/v10/07-testing-and-acceptance.md` | TEST-035…042 live matrix |
| `docs/specs/v10/10-grok-execution-runbook.md` | Permissions (no tag/publish without auth) |
| `docs/superpowers/specs/2026-07-26-popup-dismiss-rail-scroll-design.md` | Popup/rail/scroll design as implemented |
| `docs/architecture.md` | Runtime ownership after polish |
| `docs/specs/v10/04-quickshell-ux-and-accessibility.md` | UX-015/020A/020B, A11Y-018A |

---

## 2. What is NOT done (you own this)

### A. Release publish — **done**

| Item | Result |
| --- | --- |
| Dual RC at `b9475bf` | Byte-identical archive/sidecar/metadata |
| Archive SHA-256 | `102035312ef33c98498f645877ab0857a9eff39790d3a6a295c70276d786c6d6` |
| Tag + GitHub Release | [v10.0.0](https://github.com/othavi0/agent-bar/releases/tag/v10.0.0) |
| `install.sh` against published | Exercised with `--force --yes`; existing entry used rescan only |
| CI attach fix | PR #26 (`sha256sum` from output dir) |

### B. Live acceptance re-evidence — **done**

Canonical post-release report:

- [docs/qa/v10.0.0-live-qa-2026-07-27.md](qa/v10.0.0-live-qa-2026-07-27.md)
- Screenshots: [docs/qa/v10.0.0-2026-07-27/](qa/v10.0.0-2026-07-27/)

| ID | Result (2026-07-27) |
| --- | --- |
| TEST-035–036 | Pass — backup + published install; shell.json unchanged |
| TEST-037 chips | Pass — dual monitor real % / state cues |
| TEST-037 popup | Pass — open, transfer, outside-click, settings, usage track |
| TEST-037 theme | Skip (optional) |
| TEST-038 update | Fail (env) — unauth GitHub API HTTP 403 under rate limit |
| TEST-038 uninstall + purge | Pass — non-TTY JSON confirmation |
| TEST-040–042 | Pass — baseline hash restore; no unrelated plugin mutation |

Earlier Task 22 report (`/tmp/agent-bar-v10-live-qa.md`) remains historical/stale.

### C. Product / engineering residuals (non-blocking; still open)

| Residual | Severity | Suggested action |
| --- | --- | --- |
| `appliedSettings` not loaded until Settings open | Medium | Optional: `config show` on service start so chip order/metric match settings without opening Settings |
| IpcHandler `health` / `refresh` typed-arg warnings | Low | Use IPC-safe types or drop unused IPC surface |
| Cross-monitor dismiss vs native Omarchy | Info | Foreign dismiss is **stricter** than stock KeyboardPanel panels; keep or drop by product choice |
| Codex/Claude live states (error / unauth) | Env | Not product bugs if provider CLIs/auth are missing |
| Doctor “ambiguous paths” (waybar leftovers, debug symlink) | Low | Clean only if policy allows; do not expand doctor scope without auth |
| QML test runner flaky/silent in some agent environments | Low | Run `qmltestrunner` on a real desktop session as part of release gate |

### D. Repo hygiene after merge

1. Fast-forward / delete local feature branches if desired:
   - `feat/quickshell-native-v10` (merged)
   - Worktrees: `/home/othavio/Projects/agent-bar-v10` may still point at the feature branch — retarget to `master`.
2. Spec branch `spec/quickshell-native-v10` remains historical docs target unless product wants it closed.
3. Do **not** force-push master. Do **not** bypass hooks.

---

## 3. Boot checklist for the next agent

```text
1. git fetch origin && git checkout master && git pull --ff-only
2. git log -1 --oneline   # expect merge 005879d or later master tip
3. Read CLAUDE.md + docs/specs/v10/README.md + 08 + 07 + 10
4. Read docs/handoff-v10-post-merge.md (this file)
5. git status --short     # preserve unrelated work
6. Do NOT implement features until release/live gates below are green
   unless the user expands scope.
```

Hard rules (still in force):

- Rust/QML only; plugin-only product.
- No `unwrap`/`expect` in production Rust.
- No live mutation of Omarchy/Hyprland outside the authorized QA gate.
- No tag/publish/merge without explicit user authorization (merge already done).
- English Conventional Commits ≤ 50 characters.
- Zero AI attribution in commits/PRs.

---

## 4. Suggested execution order

```text
Phase 1 — Release engineering
  [x] Clean master worktree
  [x] Dual assemble+release at merge HEAD
  [x] Validate archive inventory/mode/arch/version/traversal/rollback
  [x] Prepare notes docs/releases/10.0.0.md
  [x] Tag v10.0.0 + GitHub publish (authorized)

Phase 2 — Live acceptance on Omarchy host
  [x] Install published bundle
  [x] Full TEST-035…042 with screenshots (docs/qa/v10.0.0-*)
  [x] Popup tour, purge uninstall, dual-monitor transfer
  [x] Restore baseline; write honest report

Phase 3 — Close residuals (optional / follow-up PR)
  [ ] appliedSettings on cold start
  [ ] IPC typed args
  [ ] update check GitHub unauth 403 mitigation (if product wants token/cache)

Phase 4 — Announce
  [x] README/install curl pin to v10.0.0
  [x] User-facing release notes (docs/releases/10.0.0.md + GitHub body)
```

---

## 5. Commands cheat sheet

```bash
# Gates (every code change)
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check

# QML (when available)
find assets/omarchy -type f -name '*.qml' -exec \
  qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen qmltestrunner \
  -input tests/qml \
  -import /usr/share/omarchy/shell \
  -import assets/omarchy \
  -o -,txt

# Hyprland 0.56 cursor (automation)
hyprctl dispatch 'hl.dsp.cursor.move({x=100,y=100})'

# Install plugin from local RC helper (QA only)
# ~/.config/omarchy/plugins is parent of agent-bar.usage
./target/.../agent-bar setup plugins-dir "$HOME/.config/omarchy/plugins"
omarchy plugin rescan
omarchy restart shell
```

Release builder shape (from BUNDLE-012 family — confirm exact binary name in tree):

```text
agent-bar-bundle release bundle <plugin-dir> output <output-dir>
  source-commit <40-hex> release-notes <path>
```

---

## 6. Out of scope unless user expands

- Reintroducing TUI, Waybar, monetary metrics, schema-v1.
- Global install / AUR / cargo-binstall product.
- Editing `/usr/share/omarchy`.
- Provider CLI install or credential handling.
- Merging further stacks without review.

---

## 7. One-line status for standup

> **v10.0.0 is published and live-accepted (dual monitor popup + purge + restore). Residuals: appliedSettings cold start, IPC typed args, update-check GitHub 403. Optional hygiene: close feat branch / retarget worktrees.**
