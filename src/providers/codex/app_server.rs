//! App-server JSON-RPC types, normalização e protocolo.

use std::process::Stdio;

use indexmap::IndexMap;
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use super::types::{CodexCredits, CodexLimitBucket, CodexRateLimits, CodexWindowRaw};

// ---- Tipos do app-server (camelCase, Deserialize) ----

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerWindow {
    pub used_percent: f64,
    #[serde(default)]
    pub window_duration_mins: Option<i64>,
    #[serde(default)]
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerLimitBucket {
    #[serde(default)]
    pub limit_id: Option<String>,
    #[serde(default)]
    pub limit_name: Option<String>,
    #[serde(default)]
    pub primary: Option<CodexAppServerWindow>,
    #[serde(default)]
    pub secondary: Option<CodexAppServerWindow>,
    #[serde(default)]
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerCredits {
    #[serde(default)]
    pub has_credits: bool,
    #[serde(default)]
    pub unlimited: bool,
    #[serde(default)]
    pub balance: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerRateLimitsReadResult {
    #[serde(default)]
    pub rate_limits: Option<CodexAppServerLimitBucket>,
    #[serde(default)]
    pub rate_limits_by_limit_id: Option<IndexMap<String, CodexAppServerLimitBucket>>,
    #[serde(default)]
    pub credits: Option<CodexAppServerCredits>,
    #[serde(default)]
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerAccount {
    #[serde(default)]
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CodexAppServerAccountReadResult {
    #[serde(default)]
    pub account: Option<CodexAppServerAccount>,
}

// ---- Normalização app-server → CodexRateLimits ----

fn to_raw_window(raw: &CodexAppServerWindow, fallback_minutes: i64) -> CodexWindowRaw {
    CodexWindowRaw {
        used_percent: raw.used_percent,
        window_minutes: raw.window_duration_mins.unwrap_or(fallback_minutes),
        resets_at: raw.resets_at.unwrap_or(0),
    }
}

fn normalize_bucket(
    raw: &CodexAppServerLimitBucket,
    fallback_id: Option<&str>,
) -> Option<CodexLimitBucket> {
    let limit_id = raw
        .limit_id
        .clone()
        .or_else(|| fallback_id.map(str::to_string))?;
    let primary = raw.primary.as_ref().map(|w| to_raw_window(w, 300));
    let secondary = raw.secondary.as_ref().map(|w| to_raw_window(w, 10080));
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(CodexLimitBucket {
        limit_id,
        limit_name: raw.limit_name.clone(),
        primary,
        secondary,
    })
}

pub fn normalize_appserver_rate_limits(
    raw: &CodexAppServerRateLimitsReadResult,
    account_plan_type: Option<&str>,
) -> Option<CodexRateLimits> {
    let mut buckets: IndexMap<String, CodexLimitBucket> = IndexMap::new();
    if let Some(by_id) = raw.rate_limits_by_limit_id.as_ref() {
        for (limit_id, bucket) in by_id {
            if let Some(n) = normalize_bucket(bucket, Some(limit_id)) {
                buckets.insert(n.limit_id.clone(), n);
            }
        }
    }
    let root = raw.rate_limits.as_ref();
    let root_bucket = root.and_then(|r| {
        let fallback = r.limit_id.as_deref().unwrap_or("codex");
        normalize_bucket(r, Some(fallback))
    });
    if let Some(rb) = root_bucket.as_ref() {
        if !buckets.contains_key(&rb.limit_id) {
            buckets.insert(rb.limit_id.clone(), rb.clone());
        }
    }

    let first = buckets.values().next();
    let primary = root_bucket
        .as_ref()
        .and_then(|b| b.primary.clone())
        .or_else(|| first.and_then(|b| b.primary.clone()));
    let secondary = root_bucket
        .as_ref()
        .and_then(|b| b.secondary.clone())
        .or_else(|| first.and_then(|b| b.secondary.clone()));

    if primary.is_none() && secondary.is_none() && buckets.is_empty() {
        return None;
    }

    let credits = raw.credits.as_ref().map(|c| CodexCredits {
        has_credits: c.has_credits,
        unlimited: c.unlimited,
        balance: c.balance.clone().unwrap_or_else(|| "0".to_string()),
    });

    let plan_type = account_plan_type
        .map(str::to_string)
        .or_else(|| raw.plan_type.clone())
        .or_else(|| root.and_then(|r| r.plan_type.clone()));

    Some(CodexRateLimits {
        primary,
        secondary,
        credits,
        plan_type,
        buckets: if buckets.is_empty() {
            None
        } else {
            Some(buckets)
        },
    })
}

// ---- Protocolo app-server (JSON-RPC sobre AsyncRead/AsyncWrite) ----

#[derive(serde::Deserialize)]
struct AppServerResponse {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

async fn write_json<W: AsyncWrite + Unpin>(
    w: &mut W,
    v: &serde_json::Value,
) -> std::io::Result<()> {
    let mut s = serde_json::to_string(v).unwrap_or_default();
    s.push('\n');
    w.write_all(s.as_bytes()).await
}

/// Roda o handshake JSON-RPC sobre `reader`/`writer` genéricos e retorna
/// `Some(CodexRateLimits)` em caso de sucesso ou `None` em timeout/EOF/erro.
/// Port de `fetchRateLimitsViaAppServer` (codex.ts ~359-453).
pub async fn run_appserver_protocol<R, W>(
    reader: R,
    mut writer: W,
    version: &str,
    timeout: std::time::Duration,
) -> Option<CodexRateLimits>
where
    R: tokio::io::AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    use crate::app_identity::APP_NAME;

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
    write_json(&mut writer, &init).await.ok()?;

    let mut lines = BufReader::new(reader).lines();
    // None = não recebido ainda; Some(None) = recebido mas sem plan_type
    let mut account_plan: Option<Option<String>> = None;
    let mut rate_limits: Option<CodexAppServerRateLimitsReadResult> = None;

    let hard = tokio::time::sleep(timeout);
    tokio::pin!(hard);
    // grace começa longe no futuro (timeout + 1s) para nunca disparar antes de ser armado.
    let grace = tokio::time::sleep(timeout + std::time::Duration::from_secs(1));
    tokio::pin!(grace);
    let mut grace_armed = false;

    loop {
        tokio::select! {
            _ = &mut hard => return None,
            _ = &mut grace, if grace_armed => {
                return rate_limits.as_ref().and_then(|r| {
                    let plan = account_plan.as_ref().and_then(|o| o.as_deref());
                    normalize_appserver_rate_limits(r, plan)
                });
            }
            line = lines.next_line() => {
                let line = match line {
                    Ok(Some(l)) => l,
                    _ => return None,
                };
                let msg: AppServerResponse = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                match msg.id {
                    Some(0) if msg.result.is_some() => {
                        let _ = write_json(
                            &mut writer,
                            &serde_json::json!({"method": "initialized", "params": {}}),
                        )
                        .await;
                        let _ = write_json(
                            &mut writer,
                            &serde_json::json!({
                                "method": "account/read",
                                "id": 1,
                                "params": {"refreshToken": false}
                            }),
                        )
                        .await;
                        let _ = write_json(
                            &mut writer,
                            &serde_json::json!({
                                "method": "account/rateLimits/read",
                                "id": 2,
                                "params": {}
                            }),
                        )
                        .await;
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
                            let plan_ref =
                                account_plan.as_ref().and_then(|o| o.as_deref());
                            return normalize_appserver_rate_limits(r, plan_ref);
                        }
                    }
                    Some(2) => {
                        if let Some(err) = msg.error.as_ref() {
                            // app-server respondeu com erro pro rate-limits (ex.: token
                            // expirado/revogado → 401 no backend). Não virá outra
                            // resposta pro id=2; sair já em vez de esperar o hard
                            // timeout ocioso.
                            log::debug!("Codex app-server rateLimits/read error: {err}");
                            return None;
                        }
                        let parsed = msg.result.as_ref().and_then(|v| {
                            serde_json::from_value::<CodexAppServerRateLimitsReadResult>(
                                v.clone(),
                            )
                            .ok()
                        });
                        if let Some(r) = parsed {
                            let has_data =
                                r.rate_limits.is_some() || r.rate_limits_by_limit_id.is_some();
                            if has_data {
                                rate_limits = Some(r);
                                if account_plan.is_some() {
                                    if let Some(rr) = rate_limits.as_ref() {
                                        let plan_ref = account_plan
                                            .as_ref()
                                            .and_then(|o| o.as_deref());
                                        return normalize_appserver_rate_limits(rr, plan_ref);
                                    }
                                } else {
                                    grace.as_mut().reset(
                                        tokio::time::Instant::now()
                                            + std::time::Duration::from_millis(200),
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

pub(crate) async fn fetch_via_appserver(version: &str) -> Option<CodexRateLimits> {
    let mut child = tokio::process::Command::new("codex")
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let stdin = child.stdin.take()?;
    let result =
        run_appserver_protocol(stdout, stdin, version, std::time::Duration::from_secs(4)).await;
    let _ = child.start_kill();
    result
}
