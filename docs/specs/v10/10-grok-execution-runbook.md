# Grok Execution Runbook

## Objective

Implement Agent Bar v10 exactly as specified in this directory. The canonical
specification commit is the commit containing this runbook on
`origin/spec/quickshell-native-v10`.

Grok is the implementer. Codex is the independent checkpoint reviewer. The user
controls scope changes, live QA authorization sequencing, merge, and release.

## Execution requirements

- `EXEC-001`: Start from the exact specification commit.
- `EXEC-002`: Use an isolated worktree and the approved feature branch.
- `EXEC-003`: Execute implementation-plan tasks in order with test-first
  evidence.
- `EXEC-004`: Use small English Conventional Commits.
- `EXEC-005`: Push only stable checkpoint commits.
- `EXEC-006`: Stop at every mandatory checkpoint.
- `EXEC-007`: Document and obtain approval for every deviation.
- `EXEC-008`: Keep live mutation behind checkpoint 4 acceptance.
- `EXEC-009`: Open but never merge the final ready PR.
- `EXEC-010`: Never tag, publish, distribute, force-push, or bypass hooks.

## Required preparation

1. Fetch the specification branch.
2. Resolve its exact head SHA.
3. Create an isolated worktree and feature branch from that SHA.
4. Read repository instructions and every specification file in canonical
   order.
5. Confirm the worktree is clean before editing.

```bash
git fetch origin
SPEC_SHA="$(git rev-parse origin/spec/quickshell-native-v10)"
git worktree add ../agent-bar-v10 \
  -b feat/quickshell-native-v10 "$SPEC_SHA"
cd ../agent-bar-v10
git status --short
```

Do not reuse the user's live plugin checkout as an implementation worktree.

## Required reading

Read in this order:

```text
AGENTS.md
CLAUDE.md
docs/specs/v10/README.md
docs/specs/v10/01-product-contract.md
docs/specs/v10/02-target-architecture.md
docs/specs/v10/03-cli-and-json-contract.md
docs/specs/v10/04-quickshell-ux-and-accessibility.md
docs/specs/v10/05-settings-cache-and-notifications.md
docs/specs/v10/06-migration-and-legacy-removal.md
docs/specs/v10/07-testing-and-acceptance.md
docs/specs/v10/08-plugin-bundle-and-release.md
docs/specs/v10/09-implementation-plan.md
docs/specs/v10/REQUIREMENTS_MATRIX.md
docs/specs/v10/CHECKPOINT_TEMPLATE.md
```

The v10 specification overrides v9 active behavior in the implementation
worktree. Historical records do not override it.

## Permissions

Grok may:

- edit implementation, tests, active docs, workflows, and packaging in the
  isolated worktree;
- delete files explicitly superseded by the plan;
- create small commits on `feat/quickshell-native-v10`;
- push stable checkpoint commits to that branch;
- write checkpoint files under `/tmp`;
- open one final ready pull request after all approved gates.

Grok may not:

- edit or rewrite the canonical specification without approval;
- work directly on `master` or `spec/quickshell-native-v10`;
- mutate the live Omarchy desktop before checkpoint 4 Codex approval;
- touch unrelated plugins, shell layout, Hyprland, themes, terminals, or system
  packages;
- merge, tag, publish, distribute, force-push, or bypass hooks;
- install provider CLIs or handle credentials;
- hide a failing or skipped gate;
- continue past a mandatory checkpoint.

## Execution method

Follow `09-implementation-plan.md` in order.

For every task:

1. Read each target file completely before editing.
2. Write the failing test.
3. Run it and record the expected failure.
4. Implement only the behavior required to pass.
5. Run focused verification.
6. Review the diff for unrelated changes, public copy, secrets, and legacy
   leakage.
7. Commit with an English Conventional Commit subject of at most 50 characters.

Do not batch unrelated tasks into one commit. Do not defer required tests,
cleanup, or documentation to an unspecified later step.

## Checkpoint protocol

At the end of Tasks 7, 14, 20, and 21:

1. Run that checkpoint's complete gate.
2. Push the stable branch.
3. Copy `CHECKPOINT_TEMPLATE.md` to the required `/tmp` path.
4. Fill every section with exact evidence.
5. Stop all implementation.
6. Give the user only the checkpoint path and a concise status.

The user sends the path to Codex. Grok resumes only after the user communicates
Codex acceptance and any required corrections.

## Handling a necessary deviation

Stop before implementing it. Write:

- affected requirement ID;
- why the specified behavior cannot be implemented;
- evidence from code/current Quattro;
- recommended replacement;
- product, migration, and test impact.

Place it in the checkpoint or a dedicated
`/tmp/agent-bar-v10-deviation-<id>.md`. Do not silently change the design.

## Live QA boundary

Checkpoint 4 is isolated. Even after its tests pass, Grok must stop.

Only after Codex accepts checkpoint 4 may Grok perform Task 22 using the exact
authorized path scope. Backups and rollback are mandatory. A live failure
reopens checkpoint 4: reproduce it with an isolated test, fix it on the feature
branch, rerun the gate, and request review again.

## Pull request boundary

After passing live QA and its review:

- push the final feature head;
- open a ready PR against `master`;
- include requirements, commits, gates, screenshots, migration evidence,
  rollback evidence, and remaining limitations;
- include no AI attribution;
- do not merge.

## Ready-to-paste Grok prompt

```text
Implement Agent Bar v10 from the canonical specification branch.

Repository: /home/othavio/Projects/agent-bar
Specification branch: origin/spec/quickshell-native-v10
Implementation branch: feat/quickshell-native-v10

First fetch the specification branch, resolve its exact SHA, and create an
isolated worktree from that SHA. Read AGENTS.md, CLAUDE.md, and every file in
docs/specs/v10 in the reading order defined by README.md.

The product is only the Omarchy Quattro Quickshell plugin
agent-bar.usage. The Rust binary is a private helper bundled under
bin/agent-bar. Do not create a standalone application, global executable, AUR
package, cargo-binstall product, TUI, Waybar compatibility, history, monetary
data, charts, or schema-v1 status compatibility.

Execute docs/specs/v10/09-implementation-plan.md task by task using TDD and
small English Conventional Commits. Do not rewrite the canonical spec or make
silent deviations. If a deviation is necessary, stop and document it for
approval.

Stop after each mandatory checkpoint. Write the checkpoint using
docs/specs/v10/CHECKPOINT_TEMPLATE.md at the exact /tmp path required by the
plan, push the stable feature branch, and wait for independent Codex review.

You may implement, commit, push the feature branch, and eventually open the
final ready PR. You may not merge, tag, publish, force-push, bypass hooks, or
mutate the live desktop before checkpoint 4 is accepted. Live QA is limited to
the exact backup/test/rollback scope in the specification.

Begin with Task 1 only.
```
