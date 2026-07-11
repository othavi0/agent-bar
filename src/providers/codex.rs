//! Codex provider. Estende `QuotaSource`. Duas fontes (app-server JSON-RPC +
//! fallback session-log) normalizadas para `CodexRateLimits`. Port fiel de
//! `src/providers/codex.ts`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use time::{Duration as TimeDuration, OffsetDateTime, UtcOffset};

use super::iso_from_ms;
use super::types::{
    CodexQuotaExtra, ExtraUsage, ModelWindows, ProviderExtra, ProviderQuota, QuotaWindow,
};
use crate::formatters::shared::{classify_window, normalize_plan, WindowKind};

// ---- Formato interno (snake_case = formato do session-log; é o Raw cacheável) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexWindowRaw {
    pub used_percent: f64,
    pub window_minutes: i64,
    pub resets_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexLimitBucket {
    pub limit_id: String,
    #[serde(default)]
    pub limit_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindowRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodexRateLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<CodexCredits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buckets: Option<IndexMap<String, CodexLimitBucket>>,
}

// ---- Helpers de conversão (puros) ----

/// Unix SEGUNDOS → ISO UTC; None se `<= 0`.
fn unix_to_iso(ts: i64) -> Option<String> {
    if ts <= 0 {
        None
    } else {
        Some(iso_from_ms((ts as u64) * 1000))
    }
}

/// CodexWindowRaw → QuotaWindow (remaining = 100 - round(used_percent)).
fn to_quota_window(raw: &CodexWindowRaw) -> QuotaWindow {
    QuotaWindow {
        remaining: 100.0 - raw.used_percent.round(),
        resets_at: unix_to_iso(raw.resets_at),
        window_minutes: Some(raw.window_minutes),
        used: None,
        severity: None,
    }
}

/// `limit_name` (não-vazio) ou `limit_id`; `[_-]+`→espaço; titlecase por palavra; vazio→"Codex".
fn format_bucket_label(bucket: &CodexLimitBucket) -> String {
    let raw = bucket
        .limit_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&bucket.limit_id);
    let normalized: String = raw
        .chars()
        .map(|c| if c == '_' || c == '-' { ' ' } else { c })
        .collect();
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return "Codex".to_string();
    }
    normalized
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Insere `qw` no kind certo de `windows` (fiveHour/sevenDay únicos; resto em other).
fn place_window(windows: &mut ModelWindows, raw: &CodexWindowRaw) {
    let qw = to_quota_window(raw);
    match classify_window(Some(raw.window_minutes)) {
        WindowKind::FiveHour if windows.five_hour.is_none() => windows.five_hour = Some(qw),
        WindowKind::SevenDay if windows.seven_day.is_none() => windows.seven_day = Some(qw),
        _ => windows.other.get_or_insert_with(Vec::new).push(qw),
    }
}

/// Constrói `modelsDetailed` a partir dos buckets (ou fallback legacy primary/secondary).
fn build_model_windows(limits: &CodexRateLimits) -> BTreeMap<String, ModelWindows> {
    let mut models: BTreeMap<String, ModelWindows> = BTreeMap::new();

    if let Some(buckets) = limits.buckets.as_ref().filter(|b| !b.is_empty()) {
        for bucket in buckets.values() {
            let mut windows = ModelWindows::default();
            for raw in [bucket.primary.as_ref(), bucket.secondary.as_ref()]
                .into_iter()
                .flatten()
            {
                place_window(&mut windows, raw);
            }
            // Fallback de mapeamento quando as durações não classificam limpo.
            if windows.five_hour.is_none() {
                if let Some(p) = bucket.primary.as_ref() {
                    windows.five_hour = Some(to_quota_window(p));
                }
            }
            if windows.seven_day.is_none() {
                if let Some(s) = bucket.secondary.as_ref() {
                    windows.seven_day = Some(to_quota_window(s));
                }
            }
            if windows.five_hour.is_none()
                && windows.seven_day.is_none()
                && windows.other.as_ref().map(Vec::is_empty).unwrap_or(true)
            {
                continue;
            }
            let base_name = format_bucket_label(bucket);
            let mut name = base_name.clone();
            let mut suffix = 2;
            while models.contains_key(&name) {
                name = format!("{base_name} ({suffix})");
                suffix += 1;
            }
            models.insert(name, windows);
        }
    }

    // Legacy: só primary/secondary, sem buckets.
    if models.is_empty() && (limits.primary.is_some() || limits.secondary.is_some()) {
        let mut windows = ModelWindows::default();
        for raw in [limits.primary.as_ref(), limits.secondary.as_ref()]
            .into_iter()
            .flatten()
        {
            place_window(&mut windows, raw);
        }
        if windows.five_hour.is_none() {
            if let Some(p) = limits.primary.as_ref() {
                windows.five_hour = Some(to_quota_window(p));
            }
        }
        if windows.seven_day.is_none() {
            if let Some(s) = limits.secondary.as_ref() {
                windows.seven_day = Some(to_quota_window(s));
            }
        }
        models.insert("Codex".to_string(), windows);
    }

    models
}

fn flatten_models(
    models_detailed: &BTreeMap<String, ModelWindows>,
) -> IndexMap<String, QuotaWindow> {
    let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
    for (name, w) in models_detailed {
        let selected = w
            .five_hour
            .clone()
            .or_else(|| w.seven_day.clone())
            .or_else(|| w.other.as_ref().and_then(|o| o.first().cloned()));
        if let Some(qw) = selected {
            models.insert(name.clone(), qw);
        }
    }
    models
}

fn pick_primary(
    limits: &CodexRateLimits,
    models_detailed: &BTreeMap<String, ModelWindows>,
) -> Option<QuotaWindow> {
    if let Some(p) = limits.primary.as_ref() {
        return Some(to_quota_window(p));
    }
    for m in models_detailed.values() {
        if let Some(fh) = m.five_hour.as_ref() {
            return Some(fh.clone());
        }
    }
    for m in models_detailed.values() {
        if let Some(sd) = m.seven_day.as_ref() {
            return Some(sd.clone());
        }
    }
    None
}

fn pick_secondary(
    limits: &CodexRateLimits,
    models_detailed: &BTreeMap<String, ModelWindows>,
) -> Option<QuotaWindow> {
    if let Some(s) = limits.secondary.as_ref() {
        return Some(to_quota_window(s));
    }
    for m in models_detailed.values() {
        if let Some(sd) = m.seven_day.as_ref() {
            return Some(sd.clone());
        }
    }
    None
}

/// CodexRateLimits → ProviderQuota. `error` embutido se sem janelas usáveis.
pub fn build_codex_quota(limits: &CodexRateLimits, base: ProviderQuota) -> ProviderQuota {
    let models_detailed = build_model_windows(limits);
    let models = flatten_models(&models_detailed);
    let primary = pick_primary(limits, &models_detailed);
    let secondary = pick_secondary(limits, &models_detailed);

    if primary.is_none() && secondary.is_none() && models_detailed.is_empty() {
        return ProviderQuota {
            error: Some(crate::providers::error::CodexError::NoQuotaWindows.to_string()),
            ..base
        };
    }

    // Credits → extraUsage.
    let credits_extra: Option<ExtraUsage> = limits.credits.as_ref().and_then(|c| {
        let balance: f64 = c.balance.parse().unwrap_or(0.0);
        if c.has_credits || balance > 0.0 {
            Some(ExtraUsage {
                enabled: true,
                remaining: if c.unlimited {
                    100.0
                } else {
                    100.0_f64.min(balance.round())
                },
                limit: if c.unlimited { -1.0 } else { 0.0 },
                used: 0.0,
            })
        } else {
            None
        }
    });

    let extra = if !models_detailed.is_empty() || credits_extra.is_some() {
        Some(ProviderExtra::Codex(CodexQuotaExtra {
            models_detailed: if models_detailed.is_empty() {
                None
            } else {
                Some(models_detailed)
            },
            extra_usage: credits_extra,
        }))
    } else {
        None
    };

    let plan = normalize_plan(limits.plan_type.as_deref());

    ProviderQuota {
        available: true,
        primary,
        secondary,
        models: if models.is_empty() {
            None
        } else {
            Some(models)
        },
        plan_type: limits.plan_type.clone().filter(|s| !s.is_empty()),
        plan,
        extra,
        ..base
    }
}

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

use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

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

// ---- Fallback session-log ----

/// Acha o `.jsonl` mais recente em `sessions_dir/YYYY/MM/DD` (hoje ou ontem, hora local).
pub fn find_latest_session_file(
    sessions_dir: &Path,
    now_ms: u64,
    offset: UtcOffset,
) -> Option<PathBuf> {
    let now = OffsetDateTime::from_unix_timestamp_nanos((now_ms as i128) * 1_000_000)
        .ok()?
        .to_offset(offset);
    for day_offset in 0..2i64 {
        let date = now - TimeDuration::days(day_offset);
        let dir = sessions_dir
            .join(format!("{:04}", date.year()))
            .join(format!("{:02}", date.month() as u8))
            .join(format!("{:02}", date.day()));
        let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let mtime = match entry.metadata().and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                newest = Some((mtime, path));
            }
        }
        if let Some((_, path)) = newest {
            return Some(path);
        }
    }
    None
}

#[derive(Deserialize)]
struct SessionEvent {
    #[serde(default)]
    payload: Option<SessionPayload>,
}

#[derive(Deserialize)]
struct SessionPayload {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    rate_limits: Option<CodexRateLimits>,
}

/// Lê o arquivo de sessão, faz scan reverso e retorna o primeiro `token_count` com `rate_limits`.
pub fn extract_rate_limits(path: &Path) -> Option<CodexRateLimits> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.trim().lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(ev) = serde_json::from_str::<SessionEvent>(line) {
            if let Some(p) = ev.payload {
                if p.kind.as_deref() == Some("token_count") {
                    if let Some(rl) = p.rate_limits {
                        return Some(rl);
                    }
                }
            }
        }
    }
    None
}

// ---- Provider (QuotaSource + Provider impls) ----

use std::process::Stdio;

use super::base::{base_get_quota, QuotaSource};
use super::error::{CodexError, ProviderError};
use super::{Ctx, Provider};
use async_trait::async_trait;

async fn fetch_via_appserver(version: &str) -> Option<CodexRateLimits> {
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

pub struct CodexProvider;

#[async_trait(?Send)]
impl QuotaSource for CodexProvider {
    type Raw = CodexRateLimits;

    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn cache_key(&self) -> &'static str {
        "codex-quota"
    }

    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        ctx.paths.codex_auth.exists()
    }

    async fn fetch_raw(&self, ctx: &Ctx<'_>) -> Result<CodexRateLimits, ProviderError> {
        if let Some(limits) = fetch_via_appserver(ctx.version).await {
            return Ok(limits);
        }
        log::warn!("Codex app-server unavailable, falling back to session log");
        let session =
            find_latest_session_file(&ctx.paths.codex_sessions, ctx.now_ms, ctx.local_offset)
                .ok_or(CodexError::NoSessionData)?;
        extract_rate_limits(&session).ok_or(ProviderError::Codex(CodexError::NoRateLimitData))
    }

    fn build_quota(
        &self,
        raw: CodexRateLimits,
        base: ProviderQuota,
        _ctx: &Ctx<'_>,
    ) -> ProviderQuota {
        build_codex_quota(&raw, base)
    }

    fn unavailable_error(&self) -> String {
        CodexError::NotLoggedIn.to_string()
    }

    fn to_user_facing_error(&self, error: &ProviderError) -> String {
        match error {
            ProviderError::Codex(e) => e.to_string(),
            _ => CodexError::Generic.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl Provider for CodexProvider {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn cache_key(&self) -> &'static str {
        "codex-quota"
    }

    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        QuotaSource::is_available(self, ctx).await
    }

    async fn get_quota(&self, ctx: &Ctx<'_>) -> ProviderQuota {
        base_get_quota(self, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ProviderQuota {
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: false,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    fn win(used: f64, mins: i64, resets: i64) -> CodexWindowRaw {
        CodexWindowRaw {
            used_percent: used,
            window_minutes: mins,
            resets_at: resets,
        }
    }

    fn codex_extra(q: &ProviderQuota) -> &CodexQuotaExtra {
        match q.extra.as_ref() {
            Some(ProviderExtra::Codex(c)) => c,
            _ => panic!("expected Codex extra"),
        }
    }

    // Future timestamp for tests that need a non-zero resets_at
    fn future_unix() -> i64 {
        // 2030-01-01T00:00:00Z in unix seconds
        1893456000
    }

    // -----------------------------------------------------------------------
    // primary/secondary basics
    // -----------------------------------------------------------------------

    #[test]
    fn primary_used_40_remaining_60_with_window() {
        let limits = CodexRateLimits {
            primary: Some(win(40.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.available);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 60.0);
        assert_eq!(q.primary.as_ref().unwrap().window_minutes, Some(300));
        assert!(q.primary.as_ref().unwrap().resets_at.is_none()); // resets 0
    }

    #[test]
    fn primary_used_0_remaining_100() {
        let limits = CodexRateLimits {
            primary: Some(win(0.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 100.0);
    }

    #[test]
    fn primary_used_100_remaining_0() {
        let limits = CodexRateLimits {
            primary: Some(win(100.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 0.0);
    }

    #[test]
    fn secondary_used_20_remaining_80() {
        let limits = CodexRateLimits {
            primary: Some(win(40.0, 300, future_unix())),
            secondary: Some(win(20.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 80.0);
        assert_eq!(q.secondary.as_ref().unwrap().window_minutes, Some(10080));
    }

    #[test]
    fn resets_at_iso_string_from_unix_timestamp() {
        let ts: i64 = 1711540800; // fixed known timestamp
        let limits = CodexRateLimits {
            primary: Some(win(0.0, 300, ts)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // 1711540800 seconds → 1711540800000 ms → iso
        let expected = iso_from_ms((ts as u64) * 1000);
        assert_eq!(
            q.primary.as_ref().unwrap().resets_at.as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn resets_at_null_when_zero() {
        let limits = CodexRateLimits {
            primary: Some(win(50.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
    }

    // -----------------------------------------------------------------------
    // No usable data → error
    // -----------------------------------------------------------------------

    #[test]
    fn no_usable_data_errors() {
        let limits = CodexRateLimits::default();
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.error.as_deref(), Some("No quota windows found"));
        assert!(!q.available);
    }

    #[test]
    fn plan_type_only_no_windows_errors() {
        let limits = CodexRateLimits {
            plan_type: Some("pro".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.error.as_deref(), Some("No quota windows found"));
        assert!(!q.available);
    }

    // -----------------------------------------------------------------------
    // Plan type mapping
    // -----------------------------------------------------------------------

    #[test]
    fn plan_type_enterprise_maps() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("enterprise".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.plan.as_deref(), Some("Enterprise"));
        assert_eq!(q.plan_type.as_deref(), Some("enterprise"));
    }

    #[test]
    fn plan_type_null_omitted() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: None,
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.plan.is_none());
        assert!(q.plan_type.is_none());
    }

    #[test]
    fn plan_type_various_cases() {
        let cases = [
            ("free", "Free"),
            ("pro", "Pro"),
            ("team", "Business"),
            ("business", "Business"),
            ("enterprise", "Enterprise"),
            ("edu", "Edu"),
            ("education", "Edu"),
            ("go", "Go"),
            ("plus", "Plus"),
            ("apikey", "API Key"),
            ("api_key", "API Key"),
        ];
        for (input, expected) in cases {
            let limits = CodexRateLimits {
                primary: Some(win(10.0, 300, 0)),
                plan_type: Some(input.into()),
                ..Default::default()
            };
            let q = build_codex_quota(&limits, base());
            assert_eq!(q.plan.as_deref(), Some(expected), "plan_type '{input}'");
            assert_eq!(q.plan_type.as_deref(), Some(input));
        }
    }

    #[test]
    fn unknown_plan_type_titlecased() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("custom_plan_xyz".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.plan.as_deref(), Some("Custom Plan Xyz"));
    }

    // -----------------------------------------------------------------------
    // Credits / extraUsage
    // -----------------------------------------------------------------------

    #[test]
    fn credits_has_credits_true_sets_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: false,
                balance: "10.50".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert!(eu.enabled);
        assert_eq!(eu.remaining, 11.0); // min(100, round(10.50)) = 11
        assert_eq!(eu.limit, 0.0);
        assert_eq!(eu.used, 0.0);
    }

    #[test]
    fn credits_capped_and_unlimited() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: false,
                balance: "250".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert_eq!(eu.remaining, 100.0); // min(100, 250)
        assert_eq!(eu.limit, 0.0);

        // TS test uses has_credits: true here (brief had has_credits: false — mismatch vs codex.test.ts:569)
        let limits2 = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: true,
                balance: "0".into(),
            }),
            ..Default::default()
        };
        let eu2 = codex_extra(&build_codex_quota(&limits2, base()))
            .extra_usage
            .clone()
            .unwrap();
        assert_eq!(eu2.remaining, 100.0);
        assert_eq!(eu2.limit, -1.0);
    }

    #[test]
    fn credits_balance_gt_zero_without_has_credits() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: false,
                unlimited: false,
                balance: "5.00".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert!(eu.enabled);
        assert_eq!(eu.remaining, 5.0);
    }

    #[test]
    fn credits_no_data_omits_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // extra is present (modelsDetailed), but extra_usage is None
        let ce = codex_extra(&q);
        assert!(ce.extra_usage.is_none());
    }

    #[test]
    fn credits_false_and_balance_zero_omits_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: false,
                unlimited: false,
                balance: "0".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let ce = codex_extra(&q);
        assert!(ce.extra_usage.is_none());
    }

    // -----------------------------------------------------------------------
    // Window classification
    // -----------------------------------------------------------------------

    #[test]
    fn window_300_classifies_as_five_hour() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        assert!(model.five_hour.is_some());
        assert_eq!(model.five_hour.as_ref().unwrap().remaining, 90.0);
    }

    #[test]
    fn window_10080_classifies_as_seven_day() {
        let limits = CodexRateLimits {
            secondary: Some(win(25.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        assert!(model.seven_day.is_some());
        assert_eq!(model.seven_day.as_ref().unwrap().remaining, 75.0);
    }

    #[test]
    fn tolerates_five_hour_within_90min() {
        // 210 = 300 - 90 (boundary), 390 = 300 + 90 (boundary)
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: Some(win(10.0, 210, future_unix())),
                secondary: None,
            },
        );
        buckets.insert(
            "b2".to_string(),
            CodexLimitBucket {
                limit_id: "b2".into(),
                limit_name: None,
                primary: Some(win(20.0, 390, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        for windows in md.values() {
            assert!(
                windows.five_hour.is_some(),
                "expected fiveHour for boundary minutes"
            );
        }
    }

    #[test]
    fn tolerates_seven_day_within_1440min() {
        // 8640 = 10080 - 1440, 11520 = 10080 + 1440
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: None,
                secondary: Some(win(30.0, 8640, future_unix())),
            },
        );
        buckets.insert(
            "b2".to_string(),
            CodexLimitBucket {
                limit_id: "b2".into(),
                limit_name: None,
                primary: None,
                secondary: Some(win(40.0, 11520, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        for windows in md.values() {
            assert!(
                windows.seven_day.is_some(),
                "expected sevenDay for boundary minutes"
            );
        }
    }

    #[test]
    fn unrecognized_window_uses_fallback_mapping() {
        // 60 min = "other" from classify_window, but fallback: primary→fiveHour, secondary→sevenDay
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: Some(win(10.0, 60, future_unix())),
                secondary: Some(win(20.0, 60, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        // primary (60 min → other → fallback fiveHour)
        // secondary (60 min → other initially, but place_window already put it in other;
        //   then fallback seven_day fills since five_hour was already filled by place_window fallback)
        // Actually: place_window(primary, 60) → other; place_window(secondary, 60) → other
        // then fallback: five_hour is None → fill from primary; seven_day is None → fill from secondary
        assert!(model.five_hour.is_some());
        assert!(model.seven_day.is_some());
    }

    // -----------------------------------------------------------------------
    // Multiple buckets
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_buckets_create_models_detailed_entries() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex-mini".to_string(),
            CodexLimitBucket {
                limit_id: "codex-mini".into(),
                limit_name: Some("Codex Mini".into()),
                primary: Some(win(30.0, 300, future_unix())),
                secondary: Some(win(15.0, 10080, future_unix())),
            },
        );
        buckets.insert(
            "codex-standard".to_string(),
            CodexLimitBucket {
                limit_id: "codex-standard".into(),
                limit_name: Some("Codex Standard".into()),
                primary: Some(win(60.0, 300, future_unix())),
                secondary: Some(win(45.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.available);
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 2);
        assert!(md.contains_key("Codex Mini"));
        assert!(md.contains_key("Codex Standard"));
        assert_eq!(md["Codex Mini"].five_hour.as_ref().unwrap().remaining, 70.0);
        assert_eq!(md["Codex Mini"].seven_day.as_ref().unwrap().remaining, 85.0);
        assert_eq!(
            md["Codex Standard"].five_hour.as_ref().unwrap().remaining,
            40.0
        );
        assert_eq!(
            md["Codex Standard"].seven_day.as_ref().unwrap().remaining,
            55.0
        );
    }

    #[test]
    fn limit_id_used_when_limit_name_null() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "my_custom_limit".to_string(),
            CodexLimitBucket {
                limit_id: "my_custom_limit".into(),
                limit_name: None,
                primary: Some(win(10.0, 300, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 1);
        assert!(md.contains_key("My Custom Limit"));
    }

    #[test]
    fn flatten_picks_five_hour_first() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(25.0, 300, future_unix())),
                secondary: Some(win(50.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.models.is_some());
        let models = q.models.as_ref().unwrap();
        assert_eq!(models["Codex"].remaining, 75.0); // fiveHour preferred
    }

    #[test]
    fn dedup_bucket_names_with_suffix() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "a".to_string(),
            CodexLimitBucket {
                limit_id: "a".into(),
                limit_name: Some("gpt".into()),
                primary: Some(win(20.0, 300, 0)),
                secondary: None,
            },
        );
        buckets.insert(
            "b".to_string(),
            CodexLimitBucket {
                limit_id: "b".into(),
                limit_name: Some("gpt".into()),
                primary: Some(win(30.0, 300, 0)),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert!(md.contains_key("Gpt"));
        assert!(md.contains_key("Gpt (2)"));
    }

    #[test]
    fn dedup_with_codex_label_name_collision() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "a".to_string(),
            CodexLimitBucket {
                limit_id: "a".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(10.0, 300, 0)),
                secondary: None,
            },
        );
        buckets.insert(
            "b".to_string(),
            CodexLimitBucket {
                limit_id: "b".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(20.0, 300, 0)),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 2);
        assert!(md.contains_key("Codex"));
        assert!(md.contains_key("Codex (2)"));
    }

    // -----------------------------------------------------------------------
    // Legacy fallback (no buckets, only primary/secondary)
    // -----------------------------------------------------------------------

    #[test]
    fn legacy_single_codex_entry() {
        let limits = CodexRateLimits {
            primary: Some(win(35.0, 300, future_unix())),
            secondary: Some(win(55.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 1);
        assert!(md.contains_key("Codex"));
        assert_eq!(md["Codex"].five_hour.as_ref().unwrap().remaining, 65.0);
        assert_eq!(md["Codex"].seven_day.as_ref().unwrap().remaining, 45.0);
    }

    // -----------------------------------------------------------------------
    // Primary/secondary selection
    // -----------------------------------------------------------------------

    #[test]
    fn explicit_primary_secondary_preferred_over_buckets() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: None,
                primary: Some(win(99.0, 300, future_unix())),
                secondary: Some(win(99.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            primary: Some(win(30.0, 300, future_unix())),
            secondary: Some(win(50.0, 10080, future_unix())),
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 70.0);
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 50.0);
    }

    #[test]
    fn falls_back_to_bucket_five_hour_seven_day() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: None,
                primary: Some(win(40.0, 300, future_unix())),
                secondary: Some(win(60.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // pickPrimary → first model's fiveHour
        assert_eq!(q.primary.as_ref().unwrap().remaining, 60.0);
        // pickSecondary → first model's sevenDay
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 40.0);
    }

    // -----------------------------------------------------------------------
    // Edge: empty buckets / skipped
    // -----------------------------------------------------------------------

    #[test]
    fn empty_plan_type_is_omitted() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(
            q.plan_type.is_none(),
            "plan_type vazio deve ser omitido (casa o TS)"
        );
        assert!(q.plan.is_none());
    }

    #[test]
    fn skips_bucket_with_no_primary_or_secondary() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "empty".to_string(),
            CodexLimitBucket {
                limit_id: "empty".into(),
                limit_name: None,
                primary: None,
                secondary: None,
            },
        );
        buckets.insert(
            "valid".to_string(),
            CodexLimitBucket {
                limit_id: "valid".into(),
                limit_name: Some("Valid Bucket".into()),
                primary: Some(win(20.0, 300, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, future_unix())),
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert!(md.contains_key("Valid Bucket"));
        assert!(!md.contains_key("empty") && !md.contains_key("Empty"));
    }

    // -----------------------------------------------------------------------
    // normalize_appserver_rate_limits — Task 2
    // -----------------------------------------------------------------------

    fn app_win(
        used_percent: f64,
        window_duration_mins: Option<i64>,
        resets_at: Option<i64>,
    ) -> CodexAppServerWindow {
        CodexAppServerWindow {
            used_percent,
            window_duration_mins,
            resets_at,
        }
    }

    #[test]
    fn normalize_root_rate_limits_simple() {
        // rateLimits simples (usedPercent 30, windowDurationMins 300)
        // → CodexRateLimits com primary{used_percent 30, window 300}
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(30.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: None,
            credits: None,
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("deve retornar Some");
        let primary = result.primary.expect("primary deve existir");
        assert_eq!(primary.used_percent, 30.0);
        assert_eq!(primary.window_minutes, 300);
        assert_eq!(primary.resets_at, 0);
    }

    #[test]
    fn normalize_rate_limits_by_limit_id_single_bucket() {
        // rateLimitsByLimitId com 1 bucket
        let mut by_id = IndexMap::new();
        by_id.insert(
            "codex-mini".to_string(),
            CodexAppServerLimitBucket {
                limit_id: Some("codex-mini".into()),
                limit_name: Some("Codex Mini".into()),
                primary: Some(app_win(50.0, Some(300), Some(1893456000))),
                secondary: Some(app_win(20.0, Some(10080), Some(1893456000))),
                plan_type: None,
            },
        );
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: None,
            rate_limits_by_limit_id: Some(by_id),
            credits: None,
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("deve retornar Some");
        let buckets = result.buckets.expect("buckets deve existir");
        assert_eq!(buckets.len(), 1);
        let bucket = &buckets["codex-mini"];
        assert_eq!(bucket.limit_id, "codex-mini");
        assert_eq!(bucket.limit_name.as_deref(), Some("Codex Mini"));
        let p = bucket.primary.as_ref().expect("primary do bucket");
        assert_eq!(p.used_percent, 50.0);
        assert_eq!(p.window_minutes, 300);
    }

    #[test]
    fn normalize_credits_camelcase_to_snake() {
        // credits camelCase → snake_case; balance None → "0"
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(10.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: None,
            credits: Some(CodexAppServerCredits {
                has_credits: true,
                unlimited: false,
                balance: Some("42.5".into()),
            }),
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("deve retornar Some");
        let credits = result.credits.expect("credits deve existir");
        assert!(credits.has_credits);
        assert!(!credits.unlimited);
        assert_eq!(credits.balance, "42.5");
    }

    #[test]
    fn normalize_credits_balance_none_defaults_to_zero_string() {
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(0.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: None,
            credits: Some(CodexAppServerCredits {
                has_credits: false,
                unlimited: false,
                balance: None,
            }),
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("deve retornar Some");
        let credits = result.credits.expect("credits deve existir");
        assert_eq!(credits.balance, "0");
    }

    #[test]
    fn normalize_plan_type_priority_account_over_raw_over_root() {
        // account_plan_type > raw.planType > root.planType
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(10.0, Some(300), None)),
                secondary: None,
                plan_type: Some("root-plan".into()),
            }),
            rate_limits_by_limit_id: None,
            credits: None,
            plan_type: Some("raw-plan".into()),
        };

        // account_plan_type vence
        let r1 = normalize_appserver_rate_limits(&raw, Some("account-plan")).expect("Some");
        assert_eq!(r1.plan_type.as_deref(), Some("account-plan"));

        // sem account → raw.planType
        let r2 = normalize_appserver_rate_limits(&raw, None).expect("Some");
        assert_eq!(r2.plan_type.as_deref(), Some("raw-plan"));

        // sem account, sem raw.planType → root.planType
        let raw_no_raw_plan = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(10.0, Some(300), None)),
                secondary: None,
                plan_type: Some("root-plan".into()),
            }),
            rate_limits_by_limit_id: None,
            credits: None,
            plan_type: None,
        };
        let r3 = normalize_appserver_rate_limits(&raw_no_raw_plan, None).expect("Some");
        assert_eq!(r3.plan_type.as_deref(), Some("root-plan"));
    }

    #[test]
    fn normalize_returns_none_when_everything_empty() {
        let raw = CodexAppServerRateLimitsReadResult::default();
        assert!(normalize_appserver_rate_limits(&raw, None).is_none());
    }

    #[test]
    fn normalize_root_inserted_only_if_key_absent() {
        // root com limitId "codex" e by_id já tem "codex" → root NÃO sobrescreve
        let mut by_id = IndexMap::new();
        by_id.insert(
            "codex".to_string(),
            CodexAppServerLimitBucket {
                limit_id: Some("codex".into()),
                limit_name: None,
                primary: Some(app_win(70.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            },
        );
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: Some("codex".into()),
                limit_name: None,
                primary: Some(app_win(10.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: Some(by_id),
            credits: None,
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("Some");
        let buckets = result.buckets.expect("buckets");
        assert_eq!(buckets.len(), 1);
        // O bucket do by_id (70%) vence, root (10%) não sobrescreve
        assert_eq!(
            buckets["codex"].primary.as_ref().unwrap().used_percent,
            70.0
        );
    }

    #[test]
    fn normalize_fallback_id_codex_when_root_limit_id_absent() {
        // root sem limitId → fallback "codex"
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(55.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: None,
            credits: None,
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("Some");
        let buckets = result.buckets.expect("buckets");
        assert!(buckets.contains_key("codex"));
    }

    // -----------------------------------------------------------------------
    // session-log fallback — Task 3
    // -----------------------------------------------------------------------

    fn make_session_dir(
        base: &std::path::Path,
        now_ms: u64,
        offset: UtcOffset,
    ) -> std::path::PathBuf {
        use time::OffsetDateTime;
        let dt = OffsetDateTime::from_unix_timestamp_nanos((now_ms as i128) * 1_000_000)
            .unwrap()
            .to_offset(offset);
        base.join(format!("{:04}", dt.year()))
            .join(format!("{:02}", dt.month() as u8))
            .join(format!("{:02}", dt.day()))
    }

    const TOKEN_COUNT_LINE: &str = r#"{"payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":40,"window_minutes":300,"resets_at":0}}}}"#;

    #[test]
    fn find_and_extract_session_log_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let now_ms: u64 = 1_750_000_000_000; // um timestamp fixo qualquer
        let offset = UtcOffset::UTC;
        let dir = make_session_dir(tmp.path(), now_ms, offset);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("session.jsonl");
        std::fs::write(&file, TOKEN_COUNT_LINE).unwrap();

        let found = find_latest_session_file(tmp.path(), now_ms, offset);
        assert_eq!(found.as_deref(), Some(file.as_path()));

        let rl = extract_rate_limits(found.as_ref().unwrap()).unwrap();
        assert_eq!(rl.primary.as_ref().unwrap().used_percent, 40.0);
        assert_eq!(rl.primary.as_ref().unwrap().window_minutes, 300);
    }

    #[test]
    fn find_session_multiple_files_picks_newest_mtime() {
        let tmp = tempfile::tempdir().unwrap();
        let now_ms: u64 = 1_750_000_000_000;
        let offset = UtcOffset::UTC;
        let dir = make_session_dir(tmp.path(), now_ms, offset);
        std::fs::create_dir_all(&dir).unwrap();

        let old_file = dir.join("old.jsonl");
        let new_file = dir.join("new.jsonl");
        std::fs::write(&old_file, TOKEN_COUNT_LINE).unwrap();
        std::fs::write(&new_file, TOKEN_COUNT_LINE).unwrap();

        // set old_file mtime to past
        let past = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(past)).unwrap();
        let future = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000);
        filetime::set_file_mtime(&new_file, filetime::FileTime::from_system_time(future)).unwrap();

        let found = find_latest_session_file(tmp.path(), now_ms, offset).unwrap();
        assert_eq!(found, new_file);
    }

    #[test]
    fn extract_rate_limits_scan_reverse_skips_non_token_count() {
        let tmp = tempfile::tempdir().unwrap();
        let now_ms: u64 = 1_750_000_000_000;
        let offset = UtcOffset::UTC;
        let dir = make_session_dir(tmp.path(), now_ms, offset);
        std::fs::create_dir_all(&dir).unwrap();

        let content = [
            r#"{"payload":{"type":"other_event"}}"#,
            TOKEN_COUNT_LINE,
            r#"{"payload":{"type":"something_else"}}"#,
        ]
        .join("\n");
        let file = dir.join("session.jsonl");
        std::fs::write(&file, &content).unwrap();

        let rl = extract_rate_limits(&file).unwrap();
        assert_eq!(rl.primary.as_ref().unwrap().used_percent, 40.0);
    }

    #[test]
    fn find_session_falls_back_to_yesterday() {
        let tmp = tempfile::tempdir().unwrap();
        // Usar now_ms tal que hoje não tem dir, mas ontem tem
        let now_ms: u64 = 1_750_000_000_000;
        let offset = UtcOffset::UTC;

        // Cria dir de ontem
        use time::{Duration as TimeDuration, OffsetDateTime};
        let now = OffsetDateTime::from_unix_timestamp_nanos((now_ms as i128) * 1_000_000)
            .unwrap()
            .to_offset(offset);
        let yesterday = now - TimeDuration::days(1);
        let yesterday_dir = tmp
            .path()
            .join(format!("{:04}", yesterday.year()))
            .join(format!("{:02}", yesterday.month() as u8))
            .join(format!("{:02}", yesterday.day()));
        std::fs::create_dir_all(&yesterday_dir).unwrap();
        let file = yesterday_dir.join("session.jsonl");
        std::fs::write(&file, TOKEN_COUNT_LINE).unwrap();

        // Não cria dir de hoje → deve cair no fallback
        let found = find_latest_session_file(tmp.path(), now_ms, offset);
        assert_eq!(found.as_deref(), Some(file.as_path()));
    }

    #[test]
    fn find_session_returns_none_when_no_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let now_ms: u64 = 1_750_000_000_000;
        let found = find_latest_session_file(tmp.path(), now_ms, UtcOffset::UTC);
        assert!(found.is_none());
    }

    // -----------------------------------------------------------------------
    // run_appserver_protocol — Task 4
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn appserver_happy_path() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        tokio::spawn(async move {
            let (sr, mut sw) = tokio::io::split(server);
            let mut lines = BufReader::new(sr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let v: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("id").and_then(|i| i.as_i64()) {
                    Some(0) => {
                        let _ = sw
                            .write_all(b"{\"id\":0,\"result\":{\"capabilities\":{}}}\n")
                            .await;
                    }
                    Some(1) => {
                        let _ = sw
                            .write_all(
                                b"{\"id\":1,\"result\":{\"account\":{\"planType\":\"pro\"}}}\n",
                            )
                            .await;
                    }
                    Some(2) => {
                        let _ = sw
                            .write_all(b"{\"id\":2,\"result\":{\"rateLimits\":{\"limitId\":\"codex-default\",\"primary\":{\"usedPercent\":30,\"windowDurationMins\":300,\"resetsAt\":1700000000}}}}\n")
                            .await;
                    }
                    _ => {}
                }
            }
        });
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(4)).await;
        let limits = out.expect("should resolve");
        assert_eq!(limits.primary.as_ref().unwrap().used_percent, 30.0);
        assert_eq!(limits.plan_type.as_deref(), Some("pro"));
    }

    #[tokio::test]
    async fn appserver_timeout_returns_none() {
        let (client, _server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        // _server nunca responde
        let out =
            run_appserver_protocol(cr, cw, "test", std::time::Duration::from_millis(100)).await;
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn appserver_eof_returns_none() {
        // Cria um duplex e dropa o server imediatamente → client lê EOF
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        drop(server);
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(4)).await;
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn appserver_rate_limits_error_returns_none_without_waiting_hard_timeout() {
        // Reproduz o caso real: token de auth expirado/revogado → app-server
        // responde `account/rateLimits/read` (id=2) com um erro JSON-RPC em vez
        // de `result`. Antes do fix, isso ficava sem tratamento e o loop
        // esperava o hard timeout inteiro (aqui, 4s) antes de retornar None.
        // O fix deve retornar None assim que o erro chega, bem antes do timeout.
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        tokio::spawn(async move {
            let (sr, mut sw) = tokio::io::split(server);
            let mut lines = BufReader::new(sr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let v: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("id").and_then(|i| i.as_i64()) {
                    Some(0) => {
                        let _ = sw
                            .write_all(b"{\"id\":0,\"result\":{\"capabilities\":{}}}\n")
                            .await;
                    }
                    Some(1) => {
                        let _ = sw
                            .write_all(
                                b"{\"id\":1,\"result\":{\"account\":{\"planType\":\"plus\"}}}\n",
                            )
                            .await;
                    }
                    Some(2) => {
                        let _ = sw
                            .write_all(
                                b"{\"id\":2,\"error\":{\"code\":-32603,\"message\":\"failed to fetch codex rate limits: GET https://chatgpt.com/backend-api/wham/usage failed: 401 Unauthorized; token_expired\"}}\n",
                            )
                            .await;
                    }
                    _ => {}
                }
            }
        });
        let start = std::time::Instant::now();
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(4)).await;
        let elapsed = start.elapsed();
        assert!(out.is_none());
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "esperou {elapsed:?}; deveria retornar assim que o erro de id=2 chega, não aguardar o hard timeout de 4s"
        );
    }

    #[tokio::test]
    async fn appserver_grace_resolves_without_account() {
        // Responde id0 (capabilities) e id2 (rateLimits) mas NÃO id1 (account).
        // Após grace 200ms deve resolver com Some (plan_type None).
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        tokio::spawn(async move {
            let (sr, mut sw) = tokio::io::split(server);
            let mut lines = BufReader::new(sr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let v: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("id").and_then(|i| i.as_i64()) {
                    Some(0) => {
                        let _ = sw
                            .write_all(b"{\"id\":0,\"result\":{\"capabilities\":{}}}\n")
                            .await;
                    }
                    // id1 (account/read) intencionalmente ignorado
                    Some(2) => {
                        let _ = sw
                            .write_all(b"{\"id\":2,\"result\":{\"rateLimits\":{\"limitId\":\"codex-default\",\"primary\":{\"usedPercent\":30,\"windowDurationMins\":300,\"resetsAt\":1700000000}}}}\n")
                            .await;
                    }
                    _ => {}
                }
            }
        });
        // timeout de 2s; grace de 200ms deve disparar antes
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(2)).await;
        let limits = out.expect("grace deve resolver");
        assert_eq!(limits.primary.as_ref().unwrap().used_percent, 30.0);
        // plan_type None porque account não foi recebido
        assert!(limits.plan_type.is_none());
    }

    // -----------------------------------------------------------------------
    // CodexProvider — Task 5
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn codex_provider_is_available_when_auth_exists() {
        use crate::providers::test_support::{ctx_for, settings};
        let tmp = tempfile::tempdir().unwrap();
        let settings = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(tmp.path(), &settings, &client, 0);
        // codex_auth does not exist yet → false
        assert!(!QuotaSource::is_available(&CodexProvider, &ctx).await);
        // create the file → true
        std::fs::write(&ctx.paths.codex_auth, b"{}").unwrap();
        assert!(QuotaSource::is_available(&CodexProvider, &ctx).await);
    }

    #[test]
    fn codex_provider_to_user_facing_error_codex_variant() {
        let e = ProviderError::Codex(CodexError::NoSessionData);
        assert_eq!(
            CodexProvider.to_user_facing_error(&e),
            "No session data found"
        );
    }

    #[test]
    fn codex_provider_to_user_facing_error_non_codex_variant() {
        use crate::providers::error::AmpError;
        let e = ProviderError::Amp(AmpError::Generic);
        assert_eq!(
            CodexProvider.to_user_facing_error(&e),
            "Failed to fetch Codex usage"
        );
    }

    #[test]
    fn appserver_credits_tolerates_missing_bool_fields() {
        let c: CodexAppServerCredits = serde_json::from_str(r#"{"balance":"5"}"#).unwrap();
        assert!(!c.has_credits);
        assert!(!c.unlimited);
        assert_eq!(c.balance.as_deref(), Some("5"));
    }
}
