//! CoreService.js hand-copies closed enums from the Rust schema. This test
//! keeps them in lock-step: a new state/action/provider added on one side
//! fails here instead of silently freezing the popup at runtime.

use std::collections::BTreeSet;

fn extract_keys(source: &str, var_name: &str) -> BTreeSet<String> {
    let start = source
        .find(&format!("var {var_name} = {{"))
        .unwrap_or_else(|| panic!("{var_name} not found in CoreService.js"));
    let rest = &source[start..];
    let end = rest.find('}').expect("unterminated const object");
    let body = &rest[..end];
    let mut keys = BTreeSet::new();
    for cap in body.split('"').skip(1).step_by(2) {
        // split('"') alternates outside/inside quotes; skip(1).step_by(2)
        // yields the quoted keys only ("ready", "stale", ...).
        if !cap.is_empty() && cap.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            keys.insert(cap.to_owned());
        }
    }
    keys
}

#[test]
fn servicecore_enums_match_schema() {
    let js = std::fs::read_to_string("assets/omarchy/CoreService.js").expect("read CoreService.js");

    let states = extract_keys(&js, "PROVIDER_STATES");
    let expected_states: BTreeSet<String> = [
        "ready",
        "stale",
        "cli_missing",
        "unauthenticated",
        "rate_limited",
        "network_error",
        "provider_error",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        states, expected_states,
        "PROVIDER_STATES drifted from ProviderState"
    );

    let kinds = extract_keys(&js, "ACTION_KINDS");
    let expected_kinds: BTreeSet<String> = ["retry", "login", "view_installation"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        kinds, expected_kinds,
        "ACTION_KINDS drifted from ActionKind"
    );

    let providers = extract_keys(&js, "CLOSED_PROVIDERS");
    let expected_providers: BTreeSet<String> = ["claude", "codex", "amp", "grok"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        providers, expected_providers,
        "CLOSED_PROVIDERS drifted from ProviderId"
    );
}
