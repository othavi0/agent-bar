# Troubleshooting

Resolve the private helper:

```bash
PLUGIN="$HOME/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar"
```

## Start with doctor

```bash
"$PLUGIN" doctor scan
```

Doctor is read-only. `doctor scan` checks a fixed list of legacy artifact
paths from previous Agent Bar generations, classifies each through the
ownership rules, and reports the evidence. It does not check bundle
integrity, settings or cache validity, shell entry placement, or
transaction journals, and it never prints credentials or account
identifiers.

## One provider is unavailable

```bash
"$PLUGIN" status provider claude format human cache bypass
```

Interpret the typed state:

| State | Action |
| --- | --- |
| `cli_missing` | Use `Install guide`; Agent Bar does not install it |
| `unauthenticated` | Use `Sign in` when login is available; otherwise use `Install guide` |
| `rate_limited` | Wait for the provider or reset; do not relogin blindly |
| `network_error` | Check network, then `Check again` |
| `provider_error` | Inspect safe stderr diagnostics with `RUST_LOG` |
| `stale` | Last good data is visible; refresh failed temporarily |

## Chip shows `—`

The provider is connected but exposes no normalized percentage quota for that
account. Agent Bar intentionally does not show spend, balance, or credits as a
substitute percentage.

## Codex Retry loops with “rate limits not available”

Ensure the Codex CLI is logged in. Agent Bar collects Codex rate limits in
three ordered tiers: `codex app-server` JSON-RPC `account/rateLimits/read`
first, then an explicit `~/.codex/rate-limits.json` read when the file
exists, and finally the newest valid rate-limit events under
`~/.codex/sessions`.

## Grok shows `—` or missing Weekly

When billing returns no usable percentage, Grok is connected with empty windows
and the chip shows `—`. Weekly reset comes from the billing period end.
Context is no longer a product window.

## Popup does not appear

Check:

```bash
omarchy plugin validate "$HOME/.config/omarchy/plugins/agent-bar.usage"
omarchy-shell shell rescanPlugins
```

Then inspect:

- manifest ID and version;
- `service` and `bar-widget` entry points;
- one exact `agent-bar.usage` entry in `shell.json`;
- helper/manifest version equality.

Do not run `omarchy bar plugin add` over an existing entry; it can reset
placement.

## Settings do not save

```bash
"$PLUGIN" config show
```

Confirm the settings file is valid and user-owned. Save errors leave the
previous file intact. Use `RUST_LOG=debug` only with sanitized output.

While a maintenance operation (update or uninstall) holds the exclusive
maintenance lock, `config apply` waits for the lock after validating; it
completes once maintenance finishes, and the settings file is untouched
until then.

## Update failed

`update apply` only hands off to `omarchy plugin update agent-bar.usage
--yes`; that command owns the actual result. Its failure modes:

- **Non-fast-forward**: `omarchy plugin update` refuses to update a plugin
  directory with local modifications or diverged history. It never force-
  pushes or overwrites; resolve or discard the local change in the plugin
  directory, then retry.
- **Validation failure after fetch**: the update fetches, fast-forwards, and
  re-validates with `omarchy-plugin-validate`. A failing validation runs
  `git reset --hard ORIG_HEAD` automatically, restoring the previous
  version; nothing is left half-installed.
- **Not a git checkout**: a plugin directory installed before the git-based
  distribution has no `.git`. `omarchy plugin update` silently skips it in
  a bulk run and refuses it outright when targeted by ID. `update check`
  detects this and reports `reinstallRequired: true`; the Settings UI shows
  the one-time migration instruction:

  ```bash
  omarchy plugin remove agent-bar.usage
  omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git
  ```

  Settings, cache, and backups live outside the plugin directory and
  survive the reinstall.

Confirm the outcome with:

```bash
"$PLUGIN" doctor scan
"$PLUGIN" version
```

## Modified or ambiguous legacy files

`doctor clean` removes only confirmed ownership and creates a backup:

```bash
"$PLUGIN" doctor clean
```

Modified or ambiguous paths remain for manual review.

## Collect diagnostics

Provide:

- Agent Bar version.
- Omarchy and Quickshell versions.
- Sanitized `doctor scan`.
- Exact status command and exit code.
- Typed provider state.
- Relevant sanitized transaction journal.

Never include credential files, raw provider payloads, tokens, account labels,
or live `shell.json` contents that expose unrelated user configuration.
