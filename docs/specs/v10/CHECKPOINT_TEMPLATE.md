# Agent Bar v10 Checkpoint Template

Copy this file to `/tmp/agent-bar-v10-checkpoint-<number>.md`. Replace every
instructional sentence with evidence. Do not remove a section. Write `None`
when a section has no entries.

## Checkpoint identity

```text
Checkpoint:
Date:
Worker:
Repository:
Worktree:
Branch:
Base commit:
Head commit:
Remote branch:
Specification commit:
```

## Scope completed

List the exact implementation-plan tasks completed in this checkpoint.

```text
Task:
Requirement IDs:
Result:
```

## Commit inventory

List commits in oldest-to-newest order.

```text
<sha> <subject>
```

Confirm:

- [ ] No merge commit.
- [ ] No force-push.
- [ ] No skipped hook or signature.
- [ ] No unrelated change.
- [ ] No implementation commit on the specification branch.

## Changed files

### Created

```text
path — responsibility
```

### Modified

```text
path — behavioral change
```

### Deleted

```text
path — replacement that made deletion safe
```

## Requirement coverage

For every requirement touched:

| Requirement | Implementation path | Test/evidence | Result |
| --- | --- | --- | --- |
| `ID` | `path` | `test or screenshot` | Pass/Fail |

## Test evidence

Record the exact command, exit code, summary, and log path. “Tests pass” is not
evidence.

```text
Command:
Exit code:
Passed:
Failed:
Skipped:
Duration:
Log:
```

If a mandatory test was not run, state why. A skipped mandatory gate blocks
the next checkpoint.

## Visual and accessibility evidence

For QML checkpoints:

```text
Fixture:
Theme:
Monitor/viewport:
Screenshot:
Behavioral test:
Accessibility test:
```

List every required state. HTML mockups are not runtime evidence.

## Migration and transaction evidence

For checkpoint 3 or later:

```text
Fixture:
Fault injection point:
Expected rollback:
Observed rollback:
Before hash:
After/restore hash:
Journal:
```

## Release-candidate evidence

For checkpoint 4:

```text
Source commit:
Worktree clean:
Archive:
Checksum:
Metadata:
Release notes:
Receipt/metadata equality:
First-build hashes:
Second-build hashes:
```

## Deviations

Each deviation must name the affected requirement, reason, user impact, tests,
and proposed contract change.

```text
Requirement:
Requested behavior:
Implemented behavior:
Reason:
User impact:
Evidence:
Approval status:
```

An unapproved deviation is blocking. If none, write `None`.

## Known limitations

List reproducible limitations only. Do not hide failures under “future work”.
If none, write `None`.

## Security and privacy audit

Confirm:

- [ ] No credential, token, raw provider payload, or account identifier in
      logs, fixtures, screenshots, cache, or checkpoint.
- [ ] No `sh -c`, `bash -lc`, `eval`, or constructed shell string.
- [ ] No unsafe archive path or symlink acceptance.
- [ ] No unrelated live path was read or mutated beyond the approved scope.
- [ ] No global executable or package was installed.

## Live-environment audit

Before checkpoint 4 approval, the expected value is:

```text
Live mutations performed: None
```

If live QA was explicitly authorized and reached, list every exact path and
command, backup, result, and rollback.

## Remote audit

```text
git status --short:
git log --oneline <base>..<head>:
git diff --stat <base>..<head>:
Remote head:
Open PR:
```

## Reproduction

Give a fresh reviewer exact commands to reproduce this checkpoint from the
base commit, including required environment variables and fixture paths.

## Next proposed task

Name only the next implementation-plan task. Do not start it until Codex review
accepts the checkpoint.

## Stop declaration

```text
Implementation is stopped at this checkpoint.
No later task, live installation, merge, tag, or release has been performed.
```
