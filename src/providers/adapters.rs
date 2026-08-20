//! Concrete [`ProviderAdapter`] implementations for the four locked providers.

use crate::cli::ProviderId;
use crate::status::schema::{Account, Plan, ProviderResult};

use super::adapter::{
    collection_exe, login_available, missing_collection, unauthenticated, BoxFuture,
    CollectionContext, ProviderAdapter,
};
use super::catalog::{AMP, CLAUDE, CODEX, GROK, ANTIGRAVITY};
use super::codex_app_server::{fetch_rate_limits_via_appserver, AppServerOutcome};
use super::codex_session_log::find_latest_rate_limits;
use super::process::{ProcessOutput, ProcessSpec};
use super::v2_map::{
    amp_from_usage_text, claude_from_usage_json, codex_from_rate_limits_json,
    grok_from_billing_json, antigravity_from_usage_text,
};
use super::{Discovery, ProviderDescriptor};

// ---------------------------------------------------------------------------
// Amp
// ---------------------------------------------------------------------------

pub struct AmpAdapter;

pub static AMP_ADAPTER: AmpAdapter = AmpAdapter;

impl ProviderAdapter for AmpAdapter {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &AMP
    }

    fn collect<'a>(
        &'a self,
        context: &'a CollectionContext<'a>,
        discovery: &'a Discovery,
    ) -> BoxFuture<'a, ProviderResult> {
        Box::pin(async move {
            let Some(exe) = collection_exe(discovery) else {
                return missing_collection(ProviderId::Amp, AMP.display_name, AMP.installation_url);
            };
            let spec = ProcessSpec::new(exe, ["usage"])
                .with_timeout(AMP.timeout)
                .with_max_output(AMP.max_output_bytes)
                .with_env("NO_COLOR", "1")
                .with_env("TERM", "dumb");
            match context.process.run(&spec).await {
                Ok(out) if out.timed_out => ProviderResult::NetworkError {
                    id: ProviderId::Amp,
                    name: AMP.display_name.to_owned(),
                    message: "Amp usage timed out.".into(),
                },
                Ok(out) if out.exit_code != Some(0) => {
                    classify_amp_failure(&out, login_available(discovery))
                }
                Ok(out) => amp_from_usage_text(&out.stdout, context.clock.now_utc()),
                Err(_) => ProviderResult::NetworkError {
                    id: ProviderId::Amp,
                    name: AMP.display_name.to_owned(),
                    message: "Failed to run Amp usage.".into(),
                },
            }
        })
    }
}

/// Classify a non-zero `amp usage` exit. Unauthenticated requires an explicit
/// marker; a bare "auth" substring (e.g. "authorization server unavailable")
/// is an operational failure, not a login problem.
fn classify_amp_failure(out: &ProcessOutput, login_available: bool) -> ProviderResult {
    let stdout = out.stdout.to_ascii_lowercase();
    let stderr = out.stderr.to_ascii_lowercase();
    let explicit = ["not signed", "sign in", "unauthorized", "please log in"];
    if explicit
        .iter()
        .any(|m| stdout.contains(m) || stderr.contains(m))
    {
        return unauthenticated(
            ProviderId::Amp,
            AMP.display_name,
            "Amp is not authenticated.",
            login_available,
            AMP.installation_url,
            false,
        );
    }
    ProviderResult::ProviderError {
        id: ProviderId::Amp,
        name: AMP.display_name.to_owned(),
        message: "Amp usage command failed.".into(),
        retryable: false,
    }
}

// ---------------------------------------------------------------------------
// Grok
// ---------------------------------------------------------------------------

/// Authenticated billing endpoint (literal; equality-tested).
pub const GROK_BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";

pub struct GrokAdapter;

pub static GROK_ADAPTER: GrokAdapter = GrokAdapter;

impl ProviderAdapter for GrokAdapter {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &GROK
    }

    fn collect<'a>(
        &'a self,
        context: &'a CollectionContext<'a>,
        discovery: &'a Discovery,
    ) -> BoxFuture<'a, ProviderResult> {
        Box::pin(async move {
            let grok_home = match context.env.resolve_grok_home() {
                Ok(path) => path,
                Err(_) => {
                    return ProviderResult::ProviderError {
                        id: ProviderId::Grok,
                        name: GROK.display_name.to_owned(),
                        message: "GROK_HOME is invalid.".into(),
                        retryable: false,
                    };
                }
            };
            if !grok_home.is_absolute() {
                return ProviderResult::ProviderError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Grok home must be absolute.".into(),
                    retryable: false,
                };
            }

            let auth_path = grok_home.join("auth.json");
            let auth_bytes = match context.fs.read(&auth_path) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return unauthenticated(
                        ProviderId::Grok,
                        GROK.display_name,
                        "Grok is not authenticated.",
                        login_available(discovery),
                        GROK.installation_url,
                        false,
                    );
                }
            };

            // Token is used only for the Authorization header — never stored in
            // ProviderResult, logs, or error messages.
            let (token, account) = match parse_grok_auth_token(&auth_bytes) {
                Some(v) => v,
                None => {
                    return unauthenticated(
                        ProviderId::Grok,
                        GROK.display_name,
                        "Grok is not authenticated.",
                        login_available(discovery),
                        GROK.installation_url,
                        false,
                    );
                }
            };

            let bearer = format!("Bearer {token}");
            let headers = [
                ("Authorization", bearer.as_str()),
                ("x-grok-client-mode", "cli"),
            ];
            let max_body = GROK.max_output_bytes;
            let login = login_available(discovery);
            let now = context.clock.now_utc();

            match super::retry::http_get_with_retry(
                context.http,
                &GROK,
                GROK_BILLING_URL,
                &headers,
                max_body,
            )
            .await
            {
                Ok(resp) if resp.status == 401 || resp.status == 403 => unauthenticated(
                    ProviderId::Grok,
                    GROK.display_name,
                    "Grok authentication was rejected.",
                    login,
                    GROK.installation_url,
                    false,
                ),
                Ok(resp) if (200..300).contains(&resp.status) => {
                    let _ = resp.final_url;
                    grok_from_billing_json(&resp.body, account, now, login)
                }
                Ok(resp) if resp.status >= 500 => ProviderResult::ProviderError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Grok billing request failed.".into(),
                    retryable: true,
                },
                Ok(_) => ProviderResult::ProviderError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Grok billing request failed.".into(),
                    retryable: false,
                },
                Err(super::adapter::HttpError::Network(_)) => ProviderResult::NetworkError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Network error while contacting Grok.".into(),
                },
                Err(super::adapter::HttpError::RedirectRefused(_)) => {
                    ProviderResult::ProviderError {
                        id: ProviderId::Grok,
                        name: GROK.display_name.to_owned(),
                        message: "Grok billing redirect refused.".into(),
                        retryable: false,
                    }
                }
                Err(super::adapter::HttpError::BodyTooLarge) => ProviderResult::ProviderError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Grok billing response exceeded size limit.".into(),
                    retryable: false,
                },
                Err(super::adapter::HttpError::InvalidResponse(_)) => {
                    ProviderResult::ProviderError {
                        id: ProviderId::Grok,
                        name: GROK.display_name.to_owned(),
                        message: "Invalid Grok billing response.".into(),
                        retryable: false,
                    }
                }
            }
        })
    }
}

/// Extract API key + optional first_name label from Grok `auth.json`.
///
/// Returns `None` when no non-empty `key` is present. The token must only be
/// used for the HTTP Authorization header and must never enter `ProviderResult`.
fn parse_grok_auth_token(bytes: &[u8]) -> Option<(String, Option<String>)> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let map = value.as_object()?;
    for (_k, entry) in map {
        let key = entry.get("key").and_then(|v| v.as_str()).unwrap_or("");
        if key.is_empty() {
            continue;
        }
        let name = entry
            .get("first_name")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        return Some((key.to_owned(), name));
    }
    None
}

// ---------------------------------------------------------------------------
// Codex
// ---------------------------------------------------------------------------

pub struct CodexAdapter;

pub static CODEX_ADAPTER: CodexAdapter = CodexAdapter;

impl ProviderAdapter for CodexAdapter {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &CODEX
    }

    fn collect<'a>(
        &'a self,
        context: &'a CollectionContext<'a>,
        discovery: &'a Discovery,
    ) -> BoxFuture<'a, ProviderResult> {
        Box::pin(async move {
            let home = &context.env.home;
            if !home.is_absolute() {
                return ProviderResult::ProviderError {
                    id: ProviderId::Codex,
                    name: CODEX.display_name.to_owned(),
                    message: "Codex home must be absolute.".into(),
                    retryable: false,
                };
            }

            // 1. App-server when collection exe is present (one timeout retry).
            if let Some(exe) = collection_exe(discovery) {
                let version = crate::app_identity::VERSION;
                let timeout = CODEX.timeout;
                let mut outcome = fetch_rate_limits_via_appserver(exe, version, timeout).await;
                if matches!(outcome, AppServerOutcome::TimedOut) {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    outcome = fetch_rate_limits_via_appserver(exe, version, timeout).await;
                }
                if let AppServerOutcome::Ok(bytes) = outcome {
                    return codex_from_rate_limits_json(&bytes, context.clock.now_utc());
                }
            }

            // 2. Bounded session-log fallback (~/.codex/sessions/**/*.jsonl).
            // The log's own event timestamp — not collection time — becomes
            // last_success_at, since this data may be hours or days old.
            if let Some((bytes, log_timestamp)) =
                find_latest_rate_limits(&home.join(".codex/sessions"))
            {
                let now = log_timestamp.unwrap_or_else(|| context.clock.now_utc());
                return codex_from_rate_limits_json(&bytes, now);
            }

            // 3. Typed miss / cli_missing
            if collection_exe(discovery).is_none() {
                return missing_collection(
                    ProviderId::Codex,
                    CODEX.display_name,
                    CODEX.installation_url,
                );
            }
            ProviderResult::ProviderError {
                id: ProviderId::Codex,
                name: CODEX.display_name.to_owned(),
                message: "Codex rate limits were not available.".into(),
                retryable: true,
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Claude
// ---------------------------------------------------------------------------

pub const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

pub struct ClaudeAdapter;

pub static CLAUDE_ADAPTER: ClaudeAdapter = ClaudeAdapter;

impl ProviderAdapter for ClaudeAdapter {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &CLAUDE
    }

    fn collect<'a>(
        &'a self,
        context: &'a CollectionContext<'a>,
        discovery: &'a Discovery,
    ) -> BoxFuture<'a, ProviderResult> {
        Box::pin(async move {
            let cred_path = context.env.home.join(".claude/.credentials.json");
            let cred_bytes = match context.fs.read(&cred_path) {
                Ok(b) => b,
                Err(_) => {
                    return unauthenticated(
                        ProviderId::Claude,
                        CLAUDE.display_name,
                        "Claude is not authenticated.",
                        login_available(discovery),
                        CLAUDE.installation_url,
                        false,
                    );
                }
            };
            let creds = match parse_claude_credentials(&cred_bytes) {
                Some(v) => v,
                None => {
                    return unauthenticated(
                        ProviderId::Claude,
                        CLAUDE.display_name,
                        "Claude is not authenticated.",
                        login_available(discovery),
                        CLAUDE.installation_url,
                        false,
                    );
                }
            };

            // An expired session self-heals when Claude Code refreshes the
            // token; report it as retryable so prior data is retained as stale.
            let now_ms = context
                .clock
                .now_utc()
                .unix_timestamp()
                .saturating_mul(1000);
            if creds.expires_at_ms.is_some_and(|exp| exp <= now_ms) {
                return unauthenticated(
                    ProviderId::Claude,
                    CLAUDE.display_name,
                    "Claude session expired. Open Claude Code to refresh it.",
                    login_available(discovery),
                    CLAUDE.installation_url,
                    true,
                );
            }

            // Never log the token. Pass only as Authorization header value.
            let bearer = format!("Bearer {}", creds.token);
            let headers = [
                ("Authorization", bearer.as_str()),
                ("anthropic-beta", "oauth-2025-04-20"),
            ];
            match super::retry::http_get_with_retry(
                context.http,
                &CLAUDE,
                CLAUDE_USAGE_URL,
                &headers,
                CLAUDE.max_output_bytes,
            )
            .await
            {
                Ok(resp) if resp.status == 401 || resp.status == 403 => unauthenticated(
                    ProviderId::Claude,
                    CLAUDE.display_name,
                    "Claude authentication was rejected.",
                    login_available(discovery),
                    CLAUDE.installation_url,
                    false,
                ),
                Ok(resp) if resp.status == 429 => ProviderResult::RateLimited {
                    id: ProviderId::Claude,
                    name: CLAUDE.display_name.to_owned(),
                    message: "Claude rate limited the request.".into(),
                },
                Ok(resp) if !(200..300).contains(&resp.status) => ProviderResult::ProviderError {
                    id: ProviderId::Claude,
                    name: CLAUDE.display_name.to_owned(),
                    message: "Claude usage request failed.".into(),
                    retryable: false,
                },
                Ok(resp) => {
                    // Redact: never store Authorization values in the domain result.
                    let _ = resp.final_url;
                    claude_from_usage_json(
                        &resp.body,
                        context.clock.now_utc(),
                        creds.plan,
                        creds.account,
                        login_available(discovery),
                    )
                }
                Err(super::adapter::HttpError::RedirectRefused(_)) => {
                    ProviderResult::ProviderError {
                        id: ProviderId::Claude,
                        name: CLAUDE.display_name.to_owned(),
                        message: "Claude usage redirect refused.".into(),
                        retryable: false,
                    }
                }
                Err(super::adapter::HttpError::BodyTooLarge) => ProviderResult::ProviderError {
                    id: ProviderId::Claude,
                    name: CLAUDE.display_name.to_owned(),
                    message: "Claude usage response exceeded size limit.".into(),
                    retryable: false,
                },
                Err(super::adapter::HttpError::Network(_)) => ProviderResult::NetworkError {
                    id: ProviderId::Claude,
                    name: CLAUDE.display_name.to_owned(),
                    message: "Network error while contacting Claude.".into(),
                },
                Err(super::adapter::HttpError::InvalidResponse(_)) => {
                    ProviderResult::ProviderError {
                        id: ProviderId::Claude,
                        name: CLAUDE.display_name.to_owned(),
                        message: "Invalid Claude usage response.".into(),
                        retryable: false,
                    }
                }
            }
        })
    }
}

struct ClaudeCredentials {
    token: String,
    plan: Option<Plan>,
    account: Option<Account>,
    expires_at_ms: Option<i64>,
}

fn parse_claude_credentials(bytes: &[u8]) -> Option<ClaudeCredentials> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let oauth = value.get("claudeAiOauth")?;
    let token = oauth.get("accessToken")?.as_str()?.to_owned();
    if token.is_empty() {
        return None;
    }
    let plan = claude_plan(
        oauth.get("subscriptionType").and_then(|v| v.as_str()),
        oauth.get("rateLimitTier").and_then(|v| v.as_str()),
    );
    let expires_at_ms = oauth.get("expiresAt").and_then(|v| v.as_i64());
    Some(ClaudeCredentials {
        token,
        plan,
        account: None,
        expires_at_ms,
    })
}

/// Prefer the granular rate-limit tier ("max_20x" → "Max 20x"); fall back to
/// the capitalized subscription type. Mirrors the native widget's formatTier.
fn claude_plan(subscription_type: Option<&str>, rate_limit_tier: Option<&str>) -> Option<Plan> {
    if let Some(tier) = rate_limit_tier.filter(|t| !t.is_empty()) {
        if let Some(pos) = tier.find("max_") {
            let suffix = &tier[pos + 4..];
            let digits = suffix.strip_suffix('x').unwrap_or("");
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                return Some(Plan {
                    id: tier.to_owned(),
                    label: format!("Max {suffix}"),
                });
            }
        }
        return Some(Plan {
            id: tier.to_owned(),
            label: capitalize_ascii(tier),
        });
    }
    let sub = subscription_type.filter(|s| !s.is_empty())?;
    Some(Plan {
        id: sub.to_owned(),
        label: capitalize_ascii(sub),
    })
}

fn capitalize_ascii(raw: &str) -> String {
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// Test-only fixed clock.
#[cfg(test)]
pub struct FixedClock(pub time::OffsetDateTime);

#[cfg(test)]
impl crate::support::Clock for FixedClock {
    fn now_utc(&self) -> time::OffsetDateTime {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Antigravity
// ---------------------------------------------------------------------------

pub struct AntigravityAdapter;

pub static ANTIGRAVITY_ADAPTER: AntigravityAdapter = AntigravityAdapter;

impl ProviderAdapter for AntigravityAdapter {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &ANTIGRAVITY
    }

    fn collect<'a>(
        &'a self,
        context: &'a CollectionContext<'a>,
        discovery: &'a Discovery,
    ) -> BoxFuture<'a, ProviderResult> {
        Box::pin(async move {
            let Some(exe) = collection_exe(discovery) else {
                return missing_collection(ProviderId::Antigravity, ANTIGRAVITY.display_name, ANTIGRAVITY.installation_url);
            };

            let mut cmd = tokio::process::Command::new(exe);
            cmd.stdin(std::process::Stdio::piped())
               .stdout(std::process::Stdio::piped())
               .stderr(std::process::Stdio::piped())
               .kill_on_drop(true)
               .env("NO_COLOR", "1")
               .env("TERM", "dumb");

            let spawn_res = cmd.spawn();
            let mut child = match spawn_res {
                Ok(c) => c,
                Err(e) => return ProviderResult::NetworkError {
                    id: ProviderId::Antigravity,
                    name: ANTIGRAVITY.display_name.to_owned(),
                    message: format!("Failed to spawn Antigravity CLI: {e}").into(),
                },
            };

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(b"/usage\n").await;
            }

            let wait_fut = child.wait_with_output();
            match tokio::time::timeout(ANTIGRAVITY.timeout, wait_fut).await {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    if output.status.success() {
                        let account_email = read_email_from_creds(&context.env.home);
                        let plan = read_plan_from_settings(&context.env.home);
                        antigravity_from_usage_text(&stdout, context.clock.now_utc(), account_email, plan)
                    } else {
                        classify_antigravity_failure(output.status.code(), &stdout, &stderr)
                    }
                }
                _ => {
                    ProviderResult::NetworkError {
                        id: ProviderId::Antigravity,
                        name: ANTIGRAVITY.display_name.to_owned(),
                        message: "Antigravity usage command timed out.".into(),
                    }
                }
            }
        })
    }
}

fn read_plan_from_settings(home: &std::path::Path) -> Option<Plan> {
    let settings_path = home.join(".gemini/settings.json");
    let content = std::fs::read_to_string(settings_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let selected_type = val.get("security")?
        .get("auth")?
        .get("selectedType")?
        .as_str()?;
        
    match selected_type {
        "oauth-personal" => Some(Plan {
            id: "pro".to_owned(),
            label: "Pro".to_owned(),
        }),
        "oauth-enterprise" => Some(Plan {
            id: "enterprise".to_owned(),
            label: "Enterprise".to_owned(),
        }),
        _ => Some(Plan {
            id: selected_type.to_owned(),
            label: capitalize_ascii(selected_type),
        }),
    }
}

fn read_email_from_creds(home: &std::path::Path) -> Option<String> {
    let creds_path = home.join(".gemini/oauth_creds.json");
    let content = std::fs::read_to_string(creds_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let id_token = val.get("id_token")?.as_str()?;
    
    // Parse the payload of id_token
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let payload = parts[1];
    let decoded = base64_decode_jwt_segment(payload)?;
    let payload_val: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    payload_val.get("email").and_then(|v| v.as_str()).map(|s| s.to_owned())
}

fn base64_decode_jwt_segment(input: &str) -> Option<Vec<u8>> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    
    // JWT base64url encoding uses - and _ instead of + and /
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let clean_byte = match byte {
            b'-' => b'+',
            b'_' => b'/',
            b => b,
        };
        let val = CHARSET.iter().position(|&c| c == clean_byte)? as u32;
        buffer = (buffer << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}

fn classify_antigravity_failure(exit_code: Option<i32>, stdout: &str, stderr: &str) -> ProviderResult {
    let out = format!("{}\n{}", stdout, stderr).to_ascii_lowercase();
    let explicit = ["not signed", "sign in", "unauthorized", "please log in", "authentication", "unauthenticated", "login", "expired"];
    if explicit.iter().any(|m| out.contains(m)) {
        return unauthenticated(
            ProviderId::Antigravity,
            ANTIGRAVITY.display_name,
            "Antigravity is not authenticated.",
            true,
            ANTIGRAVITY.installation_url,
            false,
        );
    }
    ProviderResult::ProviderError {
        id: ProviderId::Antigravity,
        name: ANTIGRAVITY.display_name.to_owned(),
        message: format!("Antigravity usage command failed (exit code {:?}).", exit_code).into(),
        retryable: false,
    }
}

/// In-memory filesystem for adapter tests.
#[cfg(test)]
#[derive(Default)]
pub struct MapFileSystem {
    pub files: std::collections::HashMap<std::path::PathBuf, Vec<u8>>,
}

#[cfg(test)]
impl crate::support::FileSystem for MapFileSystem {
    fn read(&self, path: &std::path::Path) -> std::io::Result<Vec<u8>> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "missing"))
    }

    fn metadata(&self, path: &std::path::Path) -> std::io::Result<crate::support::FileMetadata> {
        let bytes = self.read(path)?;
        Ok(crate::support::FileMetadata {
            len: bytes.len() as u64,
            modified: None,
            is_dir: false,
            is_symlink: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::adapter::{CollectionContext, HttpResponse};
    use crate::providers::catalog::{
        CollectionAvailability, ExecutionEnvironment, LoginAvailability,
    };
    use crate::providers::http::ScriptedHttpClient;
    use crate::providers::process::{ProcessError, ProcessOutput, ProcessRunner, ProcessSpec};
    use crate::providers::v2_map::assert_no_money;
    use std::path::Path;
    use std::sync::Mutex;
    use time::macros::datetime;

    struct ScriptedProcess {
        outputs: Mutex<Vec<Result<ProcessOutput, ProcessError>>>,
        pub last_spec: Mutex<Option<ProcessSpec>>,
    }

    impl ScriptedProcess {
        fn one(out: ProcessOutput) -> Self {
            Self {
                outputs: Mutex::new(vec![Ok(out)]),
                last_spec: Mutex::new(None),
            }
        }
    }

    impl ProcessRunner for ScriptedProcess {
        fn run<'a>(
            &'a self,
            spec: &'a ProcessSpec,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ProcessOutput, ProcessError>> + Send + 'a>,
        > {
            *self.last_spec.lock().unwrap() = Some(spec.clone());
            let next = self
                .outputs
                .lock()
                .unwrap()
                .pop()
                .unwrap_or_else(|| Err(ProcessError::Spawn("empty".into())));
            Box::pin(async move { next })
        }
    }

    fn discovery_with_exe(path: &Path) -> Discovery {
        Discovery {
            collection: CollectionAvailability::Available {
                executable: path.to_path_buf(),
            },
            login: LoginAvailability::Available {
                executable: path.to_path_buf(),
            },
        }
    }

    #[tokio::test]
    async fn amp_collect_ready_from_fixture() {
        let fixture = include_str!("../../tests/fixtures/amp/usage-free-pct.txt");
        let process = ScriptedProcess::one(ProcessOutput {
            exit_code: Some(0),
            stdout: fixture.to_owned(),
            stderr: String::new(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        });
        let http = ScriptedHttpClient::default();
        let fs = MapFileSystem::default();
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/tmp/home"),
            path_dirs: vec![],
            grok_home: None,
        };
        let clock = FixedClock(datetime!(2026-07-26 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = discovery_with_exe(Path::new("/usr/bin/amp"));
        let result = AMP_ADAPTER.collect(&ctx, &discovery).await;
        assert_no_money(&result);
        assert!(matches!(result, ProviderResult::Ready { .. }));
        let spec = process.last_spec.lock().unwrap().clone().unwrap();
        assert_eq!(spec.args, vec!["usage".to_owned()]);
        assert!(spec.env.iter().any(|(k, v)| k == "NO_COLOR" && v == "1"));
    }

    fn fake_process_output(exit_code: i32, stdout: &str, stderr: &str) -> ProcessOutput {
        ProcessOutput {
            exit_code: Some(exit_code),
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn amp_network_flavored_auth_substring_is_not_unauthenticated() {
        // "authorization server unavailable" contains "auth" but is operational.
        let out = fake_process_output(1, "", "authorization server unavailable");
        let result = classify_amp_failure(&out, true);
        assert!(matches!(result, ProviderResult::ProviderError { .. }));
    }

    #[test]
    fn amp_not_signed_in_is_unauthenticated() {
        let out = fake_process_output(1, "You are not signed in. Run amp login.", "");
        let result = classify_amp_failure(&out, true);
        assert!(matches!(result, ProviderResult::Unauthenticated { .. }));
    }

    #[tokio::test]
    async fn amp_missing_collection_source() {
        let process = ScriptedProcess::one(ProcessOutput {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        });
        let http = ScriptedHttpClient::default();
        let fs = MapFileSystem::default();
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/tmp/home"),
            path_dirs: vec![],
            grok_home: None,
        };
        let clock = FixedClock(datetime!(2026-07-26 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = Discovery {
            collection: CollectionAvailability::Missing,
            login: LoginAvailability::Missing,
        };
        let result = AMP_ADAPTER.collect(&ctx, &discovery).await;
        assert!(matches!(result, ProviderResult::CliMissing { .. }));
    }

    #[tokio::test]
    async fn claude_http_exact_url_and_redacts_auth_from_domain() {
        let body = br#"{"five_hour":{"utilization":10.0,"resets_at":"2026-07-26T22:00:00Z"}}"#;
        let http = ScriptedHttpClient::single(Ok(HttpResponse {
            status: 200,
            final_url: CLAUDE_USAGE_URL.into(),
            body: body.to_vec(),
        }));
        let process = ScriptedProcess::one(ProcessOutput {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        });
        let mut fs = MapFileSystem::default();
        let cred_path = std::path::PathBuf::from("/home/u/.claude/.credentials.json");
        fs.files.insert(
            cred_path.clone(),
            br#"{"claudeAiOauth":{"accessToken":"SECRET_TOKEN_VALUE","subscriptionType":"pro"}}"#
                .to_vec(),
        );
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/home/u"),
            path_dirs: vec![],
            grok_home: None,
        };
        let clock = FixedClock(datetime!(2026-07-26 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = discovery_with_exe(Path::new("/usr/bin/claude"));
        let result = CLAUDE_ADAPTER.collect(&ctx, &discovery).await;
        let dbg = format!("{result:?}");
        assert!(!dbg.contains("SECRET_TOKEN_VALUE"));
        assert_eq!(
            http.last_url.lock().unwrap().as_deref(),
            Some(CLAUDE_USAGE_URL)
        );
        let headers = http.last_headers.lock().unwrap().clone();
        assert!(
            headers.iter().any(|(k, v)| {
                k == "Authorization" && v.starts_with("Bearer ") && v.contains("SECRET_TOKEN_VALUE")
            }),
            "Authorization Bearer header missing: got keys {:?}",
            headers.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()
        );
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "anthropic-beta" && v == "oauth-2025-04-20"),
            "anthropic-beta header missing"
        );
        assert!(matches!(result, ProviderResult::Ready { .. }));
        assert_no_money(&result);
    }

    #[tokio::test]
    async fn claude_redirect_refused() {
        let http = ScriptedHttpClient::single(Err(
            crate::providers::adapter::HttpError::RedirectRefused("https://evil.example/".into()),
        ));
        let process = ScriptedProcess::one(ProcessOutput {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        });
        let mut fs = MapFileSystem::default();
        fs.files.insert(
            std::path::PathBuf::from("/home/u/.claude/.credentials.json"),
            br#"{"claudeAiOauth":{"accessToken":"tok"}}"#.to_vec(),
        );
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/home/u"),
            path_dirs: vec![],
            grok_home: None,
        };
        let clock = FixedClock(datetime!(2026-07-26 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = discovery_with_exe(Path::new("/usr/bin/claude"));
        let result = CLAUDE_ADAPTER.collect(&ctx, &discovery).await;
        assert!(matches!(result, ProviderResult::ProviderError { .. }));
    }

    #[tokio::test]
    async fn claude_retries_once_on_network_error() {
        let body = br#"{"five_hour":{"utilization":10.0,"resets_at":"2026-07-28T22:00:00Z"}}"#;
        let http = ScriptedHttpClient {
            responses: Mutex::new(vec![
                Ok(HttpResponse {
                    status: 200,
                    final_url: CLAUDE_USAGE_URL.into(),
                    body: body.to_vec(),
                }),
                Err(crate::providers::adapter::HttpError::Network("blip".into())),
            ]),
            last_url: Mutex::new(None),
            last_headers: Mutex::new(Vec::new()),
        };
        let process = empty_process();
        let mut fs = MapFileSystem::default();
        fs.files.insert(
            std::path::PathBuf::from("/home/u/.claude/.credentials.json"),
            br#"{"claudeAiOauth":{"accessToken":"tok"}}"#.to_vec(),
        );
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/home/u"),
            path_dirs: vec![],
            grok_home: None,
        };
        let clock = FixedClock(datetime!(2026-07-28 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = discovery_with_exe(Path::new("/usr/bin/claude"));
        let result = CLAUDE_ADAPTER.collect(&ctx, &discovery).await;
        assert!(
            matches!(result, ProviderResult::Ready { .. }),
            "one transient network error must be retried: {result:?}"
        );
    }

    #[tokio::test]
    async fn claude_expired_token_skips_http_and_is_retryable() {
        let http = ScriptedHttpClient::default(); // any HTTP call would error
        let process = empty_process();
        let mut fs = MapFileSystem::default();
        fs.files.insert(
            std::path::PathBuf::from("/home/u/.claude/.credentials.json"),
            // expiresAt in the past relative to the fixed clock below.
            br#"{"claudeAiOauth":{"accessToken":"tok","expiresAt":1690000000000}}"#.to_vec(),
        );
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/home/u"),
            path_dirs: vec![],
            grok_home: None,
        };
        let clock = FixedClock(datetime!(2026-07-28 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = discovery_with_exe(Path::new("/usr/bin/claude"));
        let result = CLAUDE_ADAPTER.collect(&ctx, &discovery).await;
        assert!(
            http.last_url.lock().unwrap().is_none(),
            "expired token must not trigger an HTTP request"
        );
        match result {
            ProviderResult::Unauthenticated {
                message, retryable, ..
            } => {
                assert!(message.contains("expired"), "message: {message}");
                assert!(retryable, "expired session must be retryable");
            }
            other => panic!("expected unauthenticated, got {other:?}"),
        }
    }

    #[test]
    fn claude_plan_formats_rate_limit_tier() {
        let plan = claude_plan(Some("max"), Some("max_20x"));
        assert_eq!(plan.as_ref().map(|p| p.label.as_str()), Some("Max 20x"));
        assert_eq!(plan.as_ref().map(|p| p.id.as_str()), Some("max_20x"));

        // Real-world tier shape observed live: prefix before max_.
        let real = claude_plan(Some("max"), Some("default_claude_max_20x"));
        assert_eq!(real.as_ref().map(|p| p.label.as_str()), Some("Max 20x"));
        assert_eq!(
            real.as_ref().map(|p| p.id.as_str()),
            Some("default_claude_max_20x")
        );

        let fallback = claude_plan(Some("pro"), None);
        assert_eq!(fallback.as_ref().map(|p| p.label.as_str()), Some("Pro"));
        assert_eq!(fallback.as_ref().map(|p| p.id.as_str()), Some("pro"));

        assert!(claude_plan(None, None).is_none());
    }

    fn grok_test_env_and_auth(fs: &mut MapFileSystem) -> ExecutionEnvironment {
        let home = std::path::PathBuf::from("/home/u");
        let grok_home = home.join(".grok");
        fs.files.insert(
            grok_home.join("auth.json"),
            // Synthetic key — not a real JWT or credential.
            br#"{"acct":{"key":"SYNTH_GROK_KEY_NOT_REAL","first_name":"Ada"}}"#.to_vec(),
        );
        ExecutionEnvironment {
            home,
            path_dirs: vec![],
            grok_home: None,
        }
    }

    fn empty_process() -> ScriptedProcess {
        ScriptedProcess::one(ProcessOutput {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        })
    }

    #[tokio::test]
    async fn grok_ready_from_billing_http() {
        let body = include_bytes!("../../tests/fixtures/providers/grok/billing-weekly.json");
        let http = ScriptedHttpClient::single(Ok(HttpResponse {
            status: 200,
            final_url: GROK_BILLING_URL.into(),
            body: body.to_vec(),
        }));
        let process = empty_process();
        let mut fs = MapFileSystem::default();
        let env = grok_test_env_and_auth(&mut fs);
        let clock = FixedClock(datetime!(2026-07-26 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = Discovery {
            collection: CollectionAvailability::Missing,
            login: LoginAvailability::Available {
                executable: std::path::PathBuf::from("/usr/bin/grok"),
            },
        };
        let result = GROK_ADAPTER.collect(&ctx, &discovery).await;
        assert_no_money(&result);
        let dbg = format!("{result:?}");
        assert!(
            !dbg.contains("SYNTH_GROK_KEY_NOT_REAL"),
            "token must not appear in domain result"
        );
        assert_eq!(
            http.last_url.lock().unwrap().as_deref(),
            Some(GROK_BILLING_URL)
        );
        let headers = http.last_headers.lock().unwrap().clone();
        assert!(
            headers.iter().any(|(k, v)| {
                k == "Authorization" && v.starts_with("Bearer ") && v.contains("SYNTH_GROK_KEY")
            }),
            "Authorization Bearer header missing: {headers:?}"
        );
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "x-grok-client-mode" && v == "cli"),
            "x-grok-client-mode header missing: {headers:?}"
        );
        match result {
            ProviderResult::Ready {
                windows, account, ..
            } => {
                assert_eq!(windows.len(), 1);
                assert_eq!(windows[0].id(), "weekly");
                assert_eq!(windows[0].label(), "Weekly (7d)");
                assert!(windows.iter().all(|w| w.id() != "context"));
                assert_eq!(account.as_ref().map(|a| a.label.as_str()), Some("Ada"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn grok_billing_401_unauthenticated() {
        let http = ScriptedHttpClient::single(Ok(HttpResponse {
            status: 401,
            final_url: GROK_BILLING_URL.into(),
            body: br#"{"error":"unauthorized"}"#.to_vec(),
        }));
        let process = empty_process();
        let mut fs = MapFileSystem::default();
        let env = grok_test_env_and_auth(&mut fs);
        let clock = FixedClock(datetime!(2026-07-26 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = Discovery {
            collection: CollectionAvailability::Missing,
            login: LoginAvailability::Available {
                executable: std::path::PathBuf::from("/usr/bin/grok"),
            },
        };
        let result = GROK_ADAPTER.collect(&ctx, &discovery).await;
        let dbg = format!("{result:?}");
        assert!(!dbg.contains("SYNTH_GROK_KEY_NOT_REAL"));
        assert!(matches!(result, ProviderResult::Unauthenticated { .. }));
    }

    #[tokio::test]
    async fn grok_billing_timeout_network_error() {
        let http = ScriptedHttpClient::single(Err(crate::providers::adapter::HttpError::Network(
            "timeout".into(),
        )));
        let process = empty_process();
        let mut fs = MapFileSystem::default();
        let env = grok_test_env_and_auth(&mut fs);
        let clock = FixedClock(datetime!(2026-07-26 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = discovery_with_exe(Path::new("/usr/bin/grok"));
        let result = GROK_ADAPTER.collect(&ctx, &discovery).await;
        let dbg = format!("{result:?}");
        assert!(!dbg.contains("SYNTH_GROK_KEY_NOT_REAL"));
        assert!(matches!(result, ProviderResult::NetworkError { .. }));
    }

    #[tokio::test]
    async fn codex_session_log_last_success_at_uses_log_timestamp_not_clock() {
        let process = empty_process();
        let http = ScriptedHttpClient::default();
        let fs = MapFileSystem::default();

        // Real tempdir: find_latest_rate_limits walks std::fs directly, not
        // the injected fs seam.
        let home_dir = tempfile::tempdir().expect("tempdir");
        let home = home_dir.path().to_path_buf();
        let sessions = home.join(".codex/sessions/2026/07/28");
        std::fs::create_dir_all(&sessions).expect("mkdir sessions");
        let jsonl = concat!(
            r#"{"timestamp":"2026-07-28T10:00:00Z","type":"event","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":12.5,"window_minutes":10080}}}}"#,
            "\n",
        );
        std::fs::write(sessions.join("rollout.jsonl"), jsonl).expect("write jsonl");

        let env = ExecutionEnvironment {
            home,
            path_dirs: vec![],
            grok_home: None,
        };
        // Fake clock's "now" is far after the log's own timestamp — asserts
        // last_success_at reflects data generation, not collection time.
        let clock = FixedClock(datetime!(2026-08-06 12:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        // Non-existent exe so app-server spawn fails immediately and
        // collection falls through to the session-log fallback (no live
        // Codex dependency, no rate-limits.json — that stage is gone).
        let discovery = discovery_with_exe(Path::new("/nonexistent/codex"));
        let result = CODEX_ADAPTER.collect(&ctx, &discovery).await;
        assert_no_money(&result);
        match result {
            ProviderResult::Ready {
                windows,
                last_success_at,
                ..
            } => {
                assert_eq!(windows[0].id(), "weekly");
                assert_eq!(last_success_at, datetime!(2026-07-28 10:00:00 UTC));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn discover_delegates_to_catalog() {
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/tmp"),
            path_dirs: vec![],
            grok_home: None,
        };
        let d = AMP_ADAPTER.discover(&env).unwrap();
        assert!(matches!(d.collection, CollectionAvailability::Missing));
    }
}
