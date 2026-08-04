# Troubleshooting

Resolve the private helper:

```bash
PLUGIN="$HOME/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar"
```

## Start with doctor

```bash
"$PLUGIN" doctor scan
```

Doctor is read-only. It reports bundle/version integrity, settings/cache
validity, shell entry placement, provider discovery, legacy ownership, and
incomplete transactions without printing credentials or account identifiers.

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

Ensure the Codex CLI is logged in. Agent Bar collects Codex rate limits through
`codex app-server` JSON-RPC `rateLimits/read`, then newest valid rate-limit
events under `~/.codex/sessions`. It does not rely on `rate-limits.json` alone.

## Grok shows `—` or missing Weekly

When billing returns no usable percentage, Grok is connected with empty windows
and the chip shows `—`. Weekly reset comes from the billing period end.
Context is no longer a product window.

## Popup does not appear

Check:

```bash
omarchy plugin validate "$HOME/.config/omarchy/plugins/agent-bar.usage"
omarchy plugin rescan
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

## Update failed

```bash
"$PLUGIN" doctor scan
```

Inspect the latest transaction journal under
`$XDG_STATE_HOME/agent-bar/transactions`. A failed update must restore the
previous complete bundle. Do not delete staging/quarantine manually before
doctor identifies it.

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
