//! Codex `app-server` JSON-RPC collection over bidirectional stdio.
//!
//! Spawns `codex app-server`, runs initialize → account/read →
//! account/rateLimits/read, and normalizes the camelCase payload into JSON
//! accepted by [`crate::providers::v2_map::codex_from_rate_limits_json`].

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::app_identity::APP_NAME;

/// Result of one app-server attempt. Distinguishes timeout so the adapter can
/// apply the catalog's single transient retry only on hard timeout.
#[derive(Debug)]
pub enum AppServerOutcome {
    Ok(Vec<u8>),
    TimedOut,
    Failed,
}

// ---- App-server wire types (camelCase) ----

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerWindow {
    used_percent: f64,
    #[serde(default)]
    window_duration_mins: Option<i64>,
    #[serde(default)]
    resets_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerIndividualLimit {
    #[serde(default)]
    remaining_percent: Option<f64>,
    #[serde(default)]
    resets_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerResetCredits {
    #[serde(default)]
    available_count: Option<u32>,
}

// credits{balance,...} is monetary and intentionally undeclared (JSON-022B).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerLimitBucket {
    #[serde(default)]
    primary: Option<CodexAppServerWindow>,
    #[serde(default)]
    secondary: Option<CodexAppServerWindow>,
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    individual_limit: Option<CodexAppServerIndividualLimit>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerRateLimitsReadResult {
    #[serde(default)]
    rate_limits: Option<CodexAppServerLimitBucket>,
    #[serde(default)]
    rate_limits_by_limit_id: Option<BTreeMap<String, CodexAppServerLimitBucket>>,
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit_reset_credits: Option<CodexAppServerResetCredits>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerAccount {
    #[serde(default)]
    plan_type: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CodexAppServerAccountReadResult {
    #[serde(default)]
    account: Option<CodexAppServerAccount>,
}

#[derive(Deserialize)]
struct AppServerResponse {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

// ---- Normalization → codex_from_rate_limits_json shape ----

fn window_to_json(raw: &CodexAppServerWindow, fallback_minutes: i64) -> serde_json::Value {
    serde_json::json!({
        "usedPercent": raw.used_percent,
        "windowDurationMins": raw.window_duration_mins.unwrap_or(fallback_minutes),
        "resetsAt": raw.resets_at.unwrap_or(0),
    })
}

/// True when a bucket carries at least one populated window (i.e. it is not
/// an empty placeholder like `premium` with `primary`/`secondary` both null).
fn has_window_data(b: &CodexAppServerLimitBucket) -> bool {
    b.primary.is_some() || b.secondary.is_some()
}

fn extra_bucket_to_json(limit_id: &str, bucket: &CodexAppServerLimitBucket) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert(
        "limitId".into(),
        serde_json::Value::String(limit_id.to_string()),
    );
    if let Some(p) = bucket.primary.as_ref() {
        m.insert("primary".into(), window_to_json(p, 300));
    }
    if let Some(s) = bucket.secondary.as_ref() {
        m.insert("secondary".into(), window_to_json(s, 10080));
    }
    serde_json::Value::Object(m)
}

/// Build JSON bytes with `primary` / `secondary` / `plan_type` for
/// [`crate::providers::v2_map::codex_from_rate_limits_json`].
fn normalize_to_rate_limits_json(
    raw: &CodexAppServerRateLimitsReadResult,
    account_plan_type: Option<&str>,
) -> Option<Vec<u8>> {
    let root = raw.rate_limits.as_ref();
    let mut primary = root.and_then(|r| r.primary.as_ref());
    let mut secondary = root.and_then(|r| r.secondary.as_ref());
    let mut individual_limit = root.and_then(|r| r.individual_limit.as_ref());

    // Preferred bucket: explicit `codex` key first (mirrors the upstream
    // backend's own preference), then any bucket that actually carries
    // windows. Every other data-carrying bucket is preserved as an extra
    // bucket instead of being silently dropped.
    let mut extra: Vec<serde_json::Value> = Vec::new();
    if primary.is_none() && secondary.is_none() {
        if let Some(by_id) = raw.rate_limits_by_limit_id.as_ref() {
            let preferred_key = if by_id.get("codex").is_some_and(has_window_data) {
                Some("codex".to_string())
            } else {
                by_id
                    .iter()
                    .find(|(_, b)| has_window_data(b))
                    .map(|(k, _)| k.clone())
            };
            if let Some(key) = preferred_key {
                if let Some(bucket) = by_id.get(key.as_str()) {
                    primary = bucket.primary.as_ref();
                    secondary = bucket.secondary.as_ref();
                    individual_limit = bucket.individual_limit.as_ref();
                }
                for (k, b) in by_id.iter() {
                    if *k != key && has_window_data(b) {
                        extra.push(extra_bucket_to_json(k, b));
                    }
                }
            }
        }
    }

    if primary.is_none() && secondary.is_none() {
        return None;
    }

    let plan_type = account_plan_type
        .map(str::to_string)
        .or_else(|| raw.plan_type.clone())
        .or_else(|| root.and_then(|r| r.plan_type.clone()));

    let mut doc = serde_json::Map::new();
    if let Some(p) = primary {
        doc.insert("primary".into(), window_to_json(p, 300));
    }
    if let Some(s) = secondary {
        doc.insert("secondary".into(), window_to_json(s, 10080));
    }
    if let Some(pt) = plan_type {
        doc.insert("plan_type".into(), serde_json::Value::String(pt));
    }
    if let Some(il) = individual_limit {
        if let Some(rem) = il.remaining_percent {
            doc.insert(
                "individualLimit".into(),
                serde_json::json!({
                    "remainingPercent": rem,
                    "resetsAt": il.resets_at.unwrap_or(0),
                }),
            );
        }
    }
    if let Some(n) = raw
        .rate_limit_reset_credits
        .as_ref()
        .and_then(|c| c.available_count)
    {
        doc.insert("rateLimitResetsAvailable".into(), serde_json::json!(n));
    }
    if !extra.is_empty() {
        doc.insert("extraBuckets".into(), serde_json::Value::Array(extra));
    }
    serde_json::to_vec(&serde_json::Value::Object(doc)).ok()
}

fn has_rate_limit_data(raw: &CodexAppServerRateLimitsReadResult) -> bool {
    raw.rate_limits.is_some() || raw.rate_limits_by_limit_id.is_some()
}

async fn write_json_line<W: AsyncWrite + Unpin>(
    w: &mut W,
    v: &serde_json::Value,
) -> std::io::Result<()> {
    let mut s = serde_json::to_string(v)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    s.push('\n');
    w.write_all(s.as_bytes()).await
}

/// Run the JSON-RPC handshake on generic streams. Returns normalized rate-limits
/// JSON bytes, or `None` on timeout / EOF / protocol error.
pub async fn run_appserver_protocol<R, W>(
    reader: R,
    writer: W,
    version: &str,
    timeout: Duration,
) -> Option<Vec<u8>>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    match run_appserver_protocol_outcome(reader, writer, version, timeout).await {
        AppServerOutcome::Ok(bytes) => Some(bytes),
        AppServerOutcome::TimedOut | AppServerOutcome::Failed => None,
    }
}

/// Same as [`run_appserver_protocol`] but preserves timeout vs hard failure.
pub async fn run_appserver_protocol_outcome<R, W>(
    reader: R,
    mut writer: W,
    version: &str,
    timeout: Duration,
) -> AppServerOutcome
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let init = serde_json::json!({
        "method": "initialize",
        "id": 0,
        "params": {
            "clientInfo": {
                "name": APP_NAME,
                "title": APP_NAME,
                "version": version
            }
        }
    });
    if write_json_line(&mut writer, &init).await.is_err() {
        return AppServerOutcome::Failed;
    }

    let mut lines = BufReader::new(reader).lines();
    // None = not yet; Some(None) = received without plan_type; Some(Some) = plan.
    let mut account_plan: Option<Option<String>> = None;
    let mut rate_limits: Option<CodexAppServerRateLimitsReadResult> = None;

    let hard = tokio::time::sleep(timeout);
    tokio::pin!(hard);
    // Grace starts far enough that it cannot fire before armed.
    let grace = tokio::time::sleep(timeout + Duration::from_secs(1));
    tokio::pin!(grace);
    let mut grace_armed = false;

    loop {
        tokio::select! {
            _ = &mut hard => {
                // Prefer already-parsed rate limits over classifying as timeout.
                return match rate_limits.as_ref().and_then(|r| {
                    let plan = account_plan.as_ref().and_then(|o| o.as_deref());
                    normalize_to_rate_limits_json(r, plan)
                }) {
                    Some(bytes) => AppServerOutcome::Ok(bytes),
                    None => AppServerOutcome::TimedOut,
                };
            }
            _ = &mut grace, if grace_armed => {
                return match rate_limits.as_ref().and_then(|r| {
                    let plan = account_plan.as_ref().and_then(|o| o.as_deref());
                    normalize_to_rate_limits_json(r, plan)
                }) {
                    Some(bytes) => AppServerOutcome::Ok(bytes),
                    None => AppServerOutcome::Failed,
                };
            }
            line = lines.next_line() => {
                let line = match line {
                    Ok(Some(l)) => l,
                    Ok(None) | Err(_) => return AppServerOutcome::Failed,
                };
                let msg: AppServerResponse = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                match msg.id {
                    Some(0) => {
                        // Initialize response: error or missing result → fail now.
                        if msg.error.is_some() || msg.result.is_none() {
                            log::debug!(
                                "Codex app-server initialize returned error or no result"
                            );
                            return AppServerOutcome::Failed;
                        }
                        if write_json_line(
                            &mut writer,
                            &serde_json::json!({"method": "initialized", "params": {}}),
                        )
                        .await
                        .is_err()
                        {
                            return AppServerOutcome::Failed;
                        }
                        if write_json_line(
                            &mut writer,
                            &serde_json::json!({
                                "method": "account/read",
                                "id": 1,
                                "params": {"refreshToken": false}
                            }),
                        )
                        .await
                        .is_err()
                        {
                            return AppServerOutcome::Failed;
                        }
                        if write_json_line(
                            &mut writer,
                            &serde_json::json!({
                                "method": "account/rateLimits/read",
                                "id": 2,
                                "params": {}
                            }),
                        )
                        .await
                        .is_err()
                        {
                            return AppServerOutcome::Failed;
                        }
                    }
                    Some(1) => {
                        let plan = msg
                            .result
                            .as_ref()
                            .and_then(|v| {
                                serde_json::from_value::<CodexAppServerAccountReadResult>(
                                    v.clone(),
                                )
                                .ok()
                            })
                            .and_then(|a| a.account)
                            .and_then(|a| a.plan_type);
                        account_plan = Some(plan);
                        if let Some(r) = rate_limits.as_ref() {
                            let plan_ref = account_plan.as_ref().and_then(|o| o.as_deref());
                            return match normalize_to_rate_limits_json(r, plan_ref) {
                                Some(bytes) => AppServerOutcome::Ok(bytes),
                                None => AppServerOutcome::Failed,
                            };
                        }
                    }
                    Some(2) => {
                        if msg.error.is_some() {
                            // Immediate failure (e.g. auth); do not wait for hard timeout.
                            log::debug!(
                                "Codex app-server account/rateLimits/read returned error"
                            );
                            return AppServerOutcome::Failed;
                        }
                        let parsed = msg.result.as_ref().and_then(|v| {
                            serde_json::from_value::<CodexAppServerRateLimitsReadResult>(
                                v.clone(),
                            )
                            .ok()
                        });
                        if let Some(r) = parsed {
                            if has_rate_limit_data(&r) {
                                rate_limits = Some(r);
                                if account_plan.is_some() {
                                    if let Some(rr) = rate_limits.as_ref() {
                                        let plan_ref = account_plan
                                            .as_ref()
                                            .and_then(|o| o.as_deref());
                                        return match normalize_to_rate_limits_json(
                                            rr, plan_ref,
                                        ) {
                                            Some(bytes) => AppServerOutcome::Ok(bytes),
                                            None => AppServerOutcome::Failed,
                                        };
                                    }
                                } else {
                                    grace.as_mut().reset(
                                        tokio::time::Instant::now()
                                            + Duration::from_millis(200),
                                    );
                                    grace_armed = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Spawn `exe app-server` with piped stdio, run the protocol, kill the child.
pub async fn fetch_rate_limits_via_appserver(
    exe: &Path,
    version: &str,
    timeout: Duration,
) -> AppServerOutcome {
    let mut child = match tokio::process::Command::new(exe)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return AppServerOutcome::Failed,
    };
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.start_kill();
            return AppServerOutcome::Failed;
        }
    };
    let stdin = match child.stdin.take() {
        Some(s) => s,
        None => {
            let _ = child.start_kill();
            return AppServerOutcome::Failed;
        }
    };
    let result = run_appserver_protocol_outcome(stdout, stdin, version, timeout).await;
    let _ = child.start_kill();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::v2_map::codex_from_rate_limits_json;
    use crate::status::schema::ProviderResult;
    use time::macros::datetime;
    use tokio::io::{duplex, AsyncBufReadExt, AsyncWriteExt, BufReader};

    async fn scripted_appserver_ok(server: tokio::io::DuplexStream) {
        let (read_half, mut write_half) = tokio::io::split(server);
        let mut lines = BufReader::new(read_half).lines();

        // initialize
        let init = lines.next_line().await.expect("init").expect("line");
        assert!(init.contains("initialize"), "{init}");
        write_half
            .write_all(br#"{"id":0,"result":{"protocolVersion":1}}"#)
            .await
            .expect("write init result");
        write_half.write_all(b"\n").await.expect("nl");

        // initialized + account/read + rateLimits/read (order may vary across lines)
        let mut saw_account = false;
        let mut saw_limits = false;
        while !(saw_account && saw_limits) {
            let line = lines.next_line().await.expect("req").expect("line");
            if line.contains("account/read") {
                saw_account = true;
                write_half
                    .write_all(br#"{"id":1,"result":{"account":{"planType":"plus"}}}"#)
                    .await
                    .expect("account");
                write_half.write_all(b"\n").await.expect("nl");
            } else if line.contains("account/rateLimits/read") {
                saw_limits = true;
                write_half
                    .write_all(
                        br#"{"id":2,"result":{"rateLimits":{"primary":{"usedPercent":12.5,"windowDurationMins":300,"resetsAt":1700000000},"secondary":{"usedPercent":40.0,"windowDurationMins":10080,"resetsAt":1700000000}}}}"#,
                    )
                    .await
                    .expect("limits");
                write_half.write_all(b"\n").await.expect("nl");
            }
            // ignore "initialized" notification
        }
        // Keep the stream open briefly so the client can finish.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[test]
    fn normalize_passes_individual_limit_and_reset_count_through() {
        let raw: CodexAppServerRateLimitsReadResult = serde_json::from_value(serde_json::json!({
            "rateLimits": {
                "limitId": "codex",
                "primary": {"usedPercent": 12.0, "windowDurationMins": 10080, "resetsAt": 1791000000},
                "secondary": null,
                "individualLimit": {"remainingPercent": 40.0, "resetsAt": 1791000000},
                "planType": "plus"
            },
            "rateLimitResetCredits": {"availableCount": 2, "credits": []}
        }))
        .expect("wire parse");
        let bytes = normalize_to_rate_limits_json(&raw, Some("plus")).expect("normalized");
        let doc: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(doc["individualLimit"]["remainingPercent"], 40.0);
        assert_eq!(doc["rateLimitResetsAvailable"], 2);
    }

    #[test]
    fn normalize_prefers_codex_bucket_over_alphabetical() {
        let raw: CodexAppServerRateLimitsReadResult = serde_json::from_value(serde_json::json!({
            "rateLimitsByLimitId": {
                "alpha": {"primary": {"usedPercent": 50.0, "windowDurationMins": 300, "resetsAt": 0}},
                "codex": {"primary": {"usedPercent": 12.0, "windowDurationMins": 10080, "resetsAt": 0}}
            }
        }))
        .expect("wire parse");
        let bytes = normalize_to_rate_limits_json(&raw, None).expect("normalized");
        let doc: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(doc["primary"]["usedPercent"], 12.0, "codex bucket must win");
        assert_eq!(doc["extraBuckets"][0]["limitId"], "alpha");
        assert_eq!(doc["extraBuckets"][0]["primary"]["usedPercent"], 50.0);
    }

    #[test]
    fn normalize_skips_null_window_buckets_like_premium() {
        let raw: CodexAppServerRateLimitsReadResult = serde_json::from_value(serde_json::json!({
            "rateLimitsByLimitId": {
                "codex": {"primary": {"usedPercent": 90.0, "windowDurationMins": 10080, "resetsAt": 0}},
                "premium": {"primary": null, "secondary": null}
            }
        }))
        .expect("wire parse");
        let bytes = normalize_to_rate_limits_json(&raw, None).expect("normalized");
        let doc: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(doc["primary"]["usedPercent"], 90.0);
        assert!(
            doc.get("extraBuckets").is_none(),
            "empty premium bucket must not appear"
        );
    }

    #[tokio::test]
    async fn appserver_protocol_reads_rate_limits() {
        let (client, server) = duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client);

        let server_task = tokio::spawn(async move {
            scripted_appserver_ok(server).await;
        });

        let out =
            run_appserver_protocol(client_read, client_write, "10.0.0", Duration::from_secs(2))
                .await
                .expect("limits json");
        let now = datetime!(2026-07-26 18:00:00 UTC);
        match codex_from_rate_limits_json(&out, now) {
            ProviderResult::Ready { windows, plan, .. } => {
                assert!(!windows.is_empty());
                assert_eq!(windows[0].id(), "session");
                assert_eq!(windows[1].id(), "weekly");
                assert_eq!(plan.as_ref().map(|p| p.id.as_str()), Some("plus"));
            }
            other => panic!("{other:?}"),
        }
        let _ = server_task.await;
    }

    #[tokio::test]
    async fn appserver_protocol_id2_error_returns_none_quickly() {
        let (client, server) = duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client);

        let server_task = tokio::spawn(async move {
            let (read_half, mut write_half) = tokio::io::split(server);
            let mut lines = BufReader::new(read_half).lines();
            let _ = lines.next_line().await; // initialize
            write_half
                .write_all(br#"{"id":0,"result":{}}"#)
                .await
                .expect("init");
            write_half.write_all(b"\n").await.expect("nl");

            // Drain client follow-ups; respond id=2 with error immediately.
            let mut saw_limits = false;
            while !saw_limits {
                let line = match lines.next_line().await {
                    Ok(Some(l)) => l,
                    _ => break,
                };
                if line.contains("account/read") {
                    write_half
                        .write_all(br#"{"id":1,"result":{"account":{}}}"#)
                        .await
                        .expect("account");
                    write_half.write_all(b"\n").await.expect("nl");
                } else if line.contains("account/rateLimits/read") {
                    saw_limits = true;
                    write_half
                        .write_all(br#"{"id":2,"error":{"code":401,"message":"unauthorized"}}"#)
                        .await
                        .expect("err");
                    write_half.write_all(b"\n").await.expect("nl");
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        });

        let started = std::time::Instant::now();
        let out =
            run_appserver_protocol(client_read, client_write, "10.0.0", Duration::from_secs(5))
                .await;
        let elapsed = started.elapsed();
        assert!(out.is_none(), "expected None on id=2 error");
        assert!(
            elapsed < Duration::from_secs(2),
            "must not wait hard timeout, elapsed={elapsed:?}"
        );
        let _ = server_task.await;
    }

    #[tokio::test]
    async fn appserver_protocol_timeout_is_timed_out_outcome() {
        let (client, _server) = duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client);
        // No server responses → hard timeout.
        let outcome = run_appserver_protocol_outcome(
            client_read,
            client_write,
            "10.0.0",
            Duration::from_millis(80),
        )
        .await;
        assert!(matches!(outcome, AppServerOutcome::TimedOut));
    }

    #[tokio::test]
    async fn appserver_hard_timeout_returns_ok_when_rate_limits_already_parsed() {
        let (client, server) = duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client);

        let server_task = tokio::spawn(async move {
            let (read_half, mut write_half) = tokio::io::split(server);
            let mut lines = BufReader::new(read_half).lines();
            let _ = lines.next_line().await; // initialize
            write_half
                .write_all(br#"{"id":0,"result":{}}"#)
                .await
                .expect("init");
            write_half.write_all(b"\n").await.expect("nl");

            // Send rate limits (id=2) but never account/read (id=1), so the
            // client arms grace and then hits hard timeout with limits held.
            let mut saw_limits = false;
            while !saw_limits {
                let line = match lines.next_line().await {
                    Ok(Some(l)) => l,
                    _ => break,
                };
                if line.contains("account/rateLimits/read") {
                    saw_limits = true;
                    write_half
                        .write_all(
                            br#"{"id":2,"result":{"rateLimits":{"primary":{"usedPercent":12.5,"windowDurationMins":300,"resetsAt":1700000000}}}}"#,
                        )
                        .await
                        .expect("limits");
                    write_half.write_all(b"\n").await.expect("nl");
                }
                // Deliberately ignore account/read so account_plan stays None.
            }
            // Hold the stream open past hard timeout.
            tokio::time::sleep(Duration::from_secs(2)).await;
        });

        let outcome = run_appserver_protocol_outcome(
            client_read,
            client_write,
            "10.0.0",
            Duration::from_millis(300),
        )
        .await;
        match outcome {
            AppServerOutcome::Ok(bytes) => {
                let now = datetime!(2026-07-26 18:00:00 UTC);
                match codex_from_rate_limits_json(&bytes, now) {
                    ProviderResult::Ready { windows, .. } => {
                        assert!(!windows.is_empty());
                        assert!((windows[0].used_percent() - 12.5).abs() < 0.01);
                    }
                    other => panic!("normalize failed: {other:?}"),
                }
            }
            other => panic!("expected Ok with parsed limits, got {other:?}"),
        }
        let _ = server_task.await;
    }

    #[tokio::test]
    async fn appserver_initialize_error_returns_failed_immediately() {
        let (client, server) = duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client);

        let server_task = tokio::spawn(async move {
            let (read_half, mut write_half) = tokio::io::split(server);
            let mut lines = BufReader::new(read_half).lines();
            let _ = lines.next_line().await; // initialize
            write_half
                .write_all(br#"{"id":0,"error":{"code":-32000,"message":"init failed"}}"#)
                .await
                .expect("err");
            write_half.write_all(b"\n").await.expect("nl");
            tokio::time::sleep(Duration::from_millis(50)).await;
        });

        let started = std::time::Instant::now();
        let outcome = run_appserver_protocol_outcome(
            client_read,
            client_write,
            "10.0.0",
            Duration::from_secs(5),
        )
        .await;
        let elapsed = started.elapsed();
        assert!(
            matches!(outcome, AppServerOutcome::Failed),
            "expected Failed on id=0 error, got {outcome:?}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "must not wait hard timeout, elapsed={elapsed:?}"
        );
        let _ = server_task.await;
    }

    #[tokio::test]
    async fn appserver_initialize_missing_result_returns_failed_immediately() {
        let (client, server) = duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client);

        let server_task = tokio::spawn(async move {
            let (read_half, mut write_half) = tokio::io::split(server);
            let mut lines = BufReader::new(read_half).lines();
            let _ = lines.next_line().await; // initialize
                                             // id=0 with neither result nor error → treat as failed.
            write_half.write_all(br#"{"id":0}"#).await.expect("empty");
            write_half.write_all(b"\n").await.expect("nl");
            tokio::time::sleep(Duration::from_millis(50)).await;
        });

        let started = std::time::Instant::now();
        let outcome = run_appserver_protocol_outcome(
            client_read,
            client_write,
            "10.0.0",
            Duration::from_secs(5),
        )
        .await;
        let elapsed = started.elapsed();
        assert!(
            matches!(outcome, AppServerOutcome::Failed),
            "expected Failed on id=0 without result, got {outcome:?}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "must not wait hard timeout, elapsed={elapsed:?}"
        );
        let _ = server_task.await;
    }
}
