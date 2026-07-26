# New Provider Guide

> Target v10 provider adapter contract; not yet implemented on the
> specification branch.

Adding a provider is a cross-contract change. It requires Rust metadata and
normalization, deterministic fixtures, an approved icon, official installation
and login behavior, QML rendering compatibility, settings migration, and
documentation.

## Adapter

Implement:

```rust
pub trait ProviderAdapter: Send + Sync {
    fn descriptor(&self) -> &'static ProviderDescriptor;
    fn discover(&self, env: &ExecutionEnvironment) -> Discovery;
    fn login_command(&self, discovery: &Discovery) -> Result<ProcessSpec>;
    fn collect<'a>(
        &'a self,
        context: &'a CollectionContext,
        discovery: &'a Discovery,
    ) -> BoxFuture<'a, ProviderResult>;
}
```

`CollectionContext` provides narrow process, HTTP, filesystem, clock, and
redaction capabilities. Do not force HTTP/filesystem/composite providers into a
fake command abstraction.

## Catalog entry

Add exactly one descriptor containing:

- stable lowercase ID;
- English display name;
- approved icon key;
- official HTTPS documentation page for installation;
- executable and login discovery metadata;
- login argv;
- TTL and timeout;
- output cap and retry policy.

Use the typed `ProviderDescriptor` and path-template shape in
`docs/specs/v10/02-target-architecture.md`. The catalog is the only provider
order and metadata source. Never store an installer script URL as the
`view_installation` target, and never concatenate login argv into a shell
string.

The adapter, not the descriptor, owns collection source precedence, stable
window IDs/labels, bounded filesystem traversal, parser fixtures, and raw-error
classification. Add those values to the locked collection-policy table before
implementing a new provider.

## Discovery

Collection availability and login availability are separate. A provider may
collect from existing HTTP credentials or local data without an installed
interactive login CLI.

CLI discovery verifies executable permission, not only file existence.

## Normalization

Return typed percentage windows:

- stable ID and English label;
- finite used/remaining values in `0..=100`;
- sum within 0.01 of 100;
- UTC reset or `null`;
- typed provider state and safe error/action.

Do not return:

- raw provider output;
- HTML/ANSI;
- credentials or account identifiers;
- `-1` or textual sentinels;
- spend, balance, credits, currency, cost, or arbitrary extras.

A connected provider with no percentage quota returns an empty windows array.
The UI renders `—`.

## Error mapping

Map raw failures to:

```text
cli_missing
unauthenticated
rate_limited
network_error
provider_error
```

Temporary failure with last good data becomes `stale` at the coordinator.
Messages are safe English copy. Control flow never uses regex over the message.

## Required fixtures

For the new provider add:

- ready percentage data;
- connected with no percentage window;
- missing collection source;
- login unavailable;
- unauthenticated;
- rate-limited;
- network failure;
- malformed output;
- timeout/termination;
- sanitization probe;
- single-provider/all-provider equality.

Tests use fake process/HTTP/filesystem data only.

## UI and assets

- Add the approved provider icon.
- Verify chip, rail, tooltip, provider header, every state, keyboard order, and
  accessibility label.
- Add light/dark deterministic screenshots.
- Do not add provider-specific parsing or arbitrary fields to QML.

## Completion checklist

1. Descriptor and adapter.
2. Fixtures and contract tests.
3. Settings schema/default/migration update.
4. Bundle receipt and icon.
5. QML state/accessibility tests.
6. Active documentation.
7. Full Rust, QML, manifest, bundle, and legacy gates.
