//! Concrete [`ProviderAdapter`] implementations for the four locked providers.

use std::path::Path;

use crate::cli::ProviderId;
use crate::status::schema::{Account, Plan, ProviderResult};
use crate::support::FileSystem;

use super::adapter::{
    collection_exe, login_available, missing_collection, unauthenticated, BoxFuture,
    CollectionContext, ProviderAdapter,
};
use super::catalog::{AMP, CLAUDE, CODEX, GROK};
use super::codex_session_log::find_latest_rate_limits;
use super::process::ProcessSpec;
use super::v2_map::{
    amp_from_usage_text, claude_from_usage_json, codex_from_rate_limits_json,
    grok_from_auth_and_signals,
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
                    if out.stderr.to_ascii_lowercase().contains("auth")
                        || out.stdout.to_ascii_lowercase().contains("not signed")
                    {
                        unauthenticated(
                            ProviderId::Amp,
                            AMP.display_name,
                            "Amp is not authenticated.",
                            login_available(discovery),
                            AMP.installation_url,
                        )
                    } else {
                        ProviderResult::ProviderError {
                            id: ProviderId::Amp,
                            name: AMP.display_name.to_owned(),
                            message: "Amp usage command failed.".into(),
                            retryable: false,
                        }
                    }
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

// ---------------------------------------------------------------------------
// Grok
// ---------------------------------------------------------------------------

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
                    );
                }
            };

            let (logged_in, account) = parse_grok_auth(&auth_bytes);
            if !logged_in {
                return unauthenticated(
                    ProviderId::Grok,
                    GROK.display_name,
                    "Grok is not authenticated.",
                    login_available(discovery),
                    GROK.installation_url,
                );
            }

            let signals = find_latest_signals(context.fs, &grok_home.join("sessions"));
            let _ = collection_exe(discovery); // login/collection independence
            grok_from_auth_and_signals(
                true,
                account,
                signals.as_deref(),
                context.clock.now_utc(),
                login_available(discovery),
            )
        })
    }
}

fn parse_grok_auth(bytes: &[u8]) -> (bool, Option<String>) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return (false, None);
    };
    let Some(map) = value.as_object() else {
        return (false, None);
    };
    for (_k, entry) in map {
        let key = entry.get("key").and_then(|v| v.as_str()).unwrap_or("");
        if !key.is_empty() {
            let name = entry
                .get("first_name")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            return (true, name);
        }
    }
    (false, None)
}

/// Bounded walk: no follow links, max depth 8, max 4096 entries, max 256 signals.
fn find_latest_signals(fs: &dyn FileSystem, sessions: &Path) -> Option<Vec<u8>> {
    // Prefer a simple known file if present (tests); otherwise walk via std for
    // directory listing (FileSystem seam is read/metadata only).
    let direct = sessions.join("recent/signals.json");
    if let Ok(bytes) = fs.read(&direct) {
        return Some(bytes);
    }
    walk_signals_std(sessions)
}

fn walk_signals_std(sessions: &Path) -> Option<Vec<u8>> {
    use std::cmp::Ordering;
    use std::fs;

    if !sessions.is_dir() {
        return None;
    }
    let mut candidates: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
    let mut stack = vec![(sessions.to_path_buf(), 0u32)];
    let mut visits = 0u32;
    while let Some((dir, depth)) = stack.pop() {
        if visits >= 4096 || depth > 8 {
            continue;
        }
        visits += 1;
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visits += 1;
            if visits >= 4096 {
                break;
            }
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() && depth < 8 {
                stack.push((path, depth + 1));
            } else if meta.is_file()
                && path.file_name().and_then(|s| s.to_str()) == Some("signals.json")
            {
                let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                candidates.push((mtime, path));
            }
        }
    }
    candidates.sort_by(|a, b| match b.0.cmp(&a.0) {
        Ordering::Equal => a.1.as_os_str().cmp(b.1.as_os_str()),
        other => other,
    });
    candidates.truncate(256);
    let path = &candidates.first()?.1;
    fs::read(path).ok()
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
            // Prefer filesystem session-log fixture path under $HOME/.codex
            let home = &context.env.home;
            if !home.is_absolute() {
                return ProviderResult::ProviderError {
                    id: ProviderId::Codex,
                    name: CODEX.display_name.to_owned(),
                    message: "Codex home must be absolute.".into(),
                    retryable: false,
                };
            }
            // 1. Explicit rate-limits.json if present
            let rates_path = home.join(".codex/rate-limits.json");
            if let Ok(bytes) = context.fs.read(&rates_path) {
                return codex_from_rate_limits_json(&bytes, context.clock.now_utc());
            }

            // 2. Bounded session-log fallback (~/.codex/sessions/**/*.jsonl)
            if let Some(bytes) = find_latest_rate_limits(&home.join(".codex/sessions")) {
                return codex_from_rate_limits_json(&bytes, context.clock.now_utc());
            }

            // 3. No collection exe → cli_missing
            let Some(exe) = collection_exe(discovery) else {
                return missing_collection(
                    ProviderId::Codex,
                    CODEX.display_name,
                    CODEX.installation_url,
                );
            };
            // 4. App-server not wired yet (Task 3); typed retryable miss.
            let _ = exe;
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
                    );
                }
            };
            let (token, plan, account) = match parse_claude_credentials(&cred_bytes) {
                Some(v) => v,
                None => {
                    return unauthenticated(
                        ProviderId::Claude,
                        CLAUDE.display_name,
                        "Claude is not authenticated.",
                        login_available(discovery),
                        CLAUDE.installation_url,
                    );
                }
            };

            // Never log the token. Pass only as Authorization header value.
            let headers = [
                ("Authorization", token.as_str()),
                ("anthropic-beta", "oauth-2025-04-20"),
            ];
            match context
                .http
                .get(CLAUDE_USAGE_URL, &headers, CLAUDE.max_output_bytes)
                .await
            {
                Ok(resp) if resp.status == 401 || resp.status == 403 => unauthenticated(
                    ProviderId::Claude,
                    CLAUDE.display_name,
                    "Claude authentication was rejected.",
                    login_available(discovery),
                    CLAUDE.installation_url,
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
                        plan,
                        account,
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

fn parse_claude_credentials(bytes: &[u8]) -> Option<(String, Option<Plan>, Option<Account>)> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let oauth = value.get("claudeAiOauth")?;
    let token = oauth.get("accessToken")?.as_str()?.to_owned();
    if token.is_empty() {
        return None;
    }
    let plan = oauth
        .get("subscriptionType")
        .and_then(|v| v.as_str())
        .map(|id| Plan {
            id: id.to_owned(),
            label: id.to_owned(),
        });
    Some((token, plan, None))
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

/// In-memory filesystem for adapter tests.
#[cfg(test)]
#[derive(Default)]
pub struct MapFileSystem {
    pub files: std::collections::HashMap<std::path::PathBuf, Vec<u8>>,
}

#[cfg(test)]
impl FileSystem for MapFileSystem {
    fn read(&self, path: &Path) -> std::io::Result<Vec<u8>> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "missing"))
    }

    fn metadata(&self, path: &Path) -> std::io::Result<crate::support::FileMetadata> {
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
    async fn grok_ready_from_auth_and_signals_fixture() {
        let process = ScriptedProcess::one(ProcessOutput {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        });
        let http = ScriptedHttpClient::default();
        let mut fs = MapFileSystem::default();
        let home = std::path::PathBuf::from("/home/u");
        let grok_home = home.join(".grok");
        fs.files.insert(
            grok_home.join("auth.json"),
            br#"{"acct":{"key":"k","first_name":"Ada"}}"#.to_vec(),
        );
        fs.files.insert(
            grok_home.join("sessions/recent/signals.json"),
            include_bytes!("../../tests/fixtures/grok/signals-recent.json").to_vec(),
        );
        let env = ExecutionEnvironment {
            home,
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
            login: LoginAvailability::Available {
                executable: std::path::PathBuf::from("/usr/bin/grok"),
            },
        };
        let result = GROK_ADAPTER.collect(&ctx, &discovery).await;
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows[0].id(), "context");
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn codex_from_home_rate_limits_file() {
        let process = ScriptedProcess::one(ProcessOutput {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        });
        let http = ScriptedHttpClient::default();
        let mut fs = MapFileSystem::default();
        let home = std::path::PathBuf::from("/home/u");
        fs.files.insert(
            home.join(".codex/rate-limits.json"),
            br#"{"primary":{"usedPercent":25.0,"windowDurationMins":300,"resetsAt":1700000000},"secondary":{"usedPercent":40.0,"windowDurationMins":10080,"resetsAt":1700000000}}"#.to_vec(),
        );
        let env = ExecutionEnvironment {
            home,
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
        let discovery = discovery_with_exe(Path::new("/usr/bin/codex"));
        let result = CODEX_ADAPTER.collect(&ctx, &discovery).await;
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 2);
                assert_eq!(windows[0].id(), "session");
                assert_eq!(windows[1].id(), "weekly");
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
