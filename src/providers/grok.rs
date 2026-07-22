//! Grok Build CLI provider. Zero rede: auth.json + sessions/**/signals.json.
//! Estende `QuotaSource` / `base_get_quota` (como Amp/Codex). Sem OAuth refresh.

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::{Date, OffsetDateTime, UtcOffset};

use super::base::{base_get_quota, QuotaSource};
use super::error::{GrokError, ProviderError};
use super::grok_cli::find_grok_bin;
use super::types::{GrokQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow};
use super::{Ctx, Provider};

const MAX_WALK_DEPTH: u32 = 16;
const MAX_WALK_VISITS: u32 = 2000;

/// Snapshot de sessão (cacheável; sem tokens JWT).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSnap {
    pub mtime_ms: u64,
    pub context_tokens_used: Option<u64>,
    pub context_window_tokens: Option<u64>,
    pub primary_model_id: Option<String>,
    pub turn_count: u32,
}

/// Raw cacheável do Grok — **sem** access/refresh tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrokRaw {
    pub account: Option<String>,
    pub logged_in: bool,
    pub sessions: Vec<SessionSnap>,
}

/// Visão sanitizada do auth (sem JWT).
#[derive(Debug, Clone, PartialEq)]
pub struct AuthView {
    pub account: Option<String>,
    pub logged_in: bool,
}

#[derive(Debug, Deserialize)]
struct AuthEntry {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    first_name: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalsFile {
    #[serde(default)]
    context_tokens_used: Option<u64>,
    #[serde(default)]
    context_window_tokens: Option<u64>,
    #[serde(default)]
    primary_model_id: Option<String>,
    #[serde(default)]
    turn_count: Option<u32>,
}

/// % restante de contexto: `(100 - 100*used/window).clamp(0, 100)`.
/// `None` se `window == 0`.
pub(crate) fn context_remaining_pct(used: u64, window: u64) -> Option<f64> {
    if window == 0 {
        return None;
    }
    let used_pct = 100.0 * (used as f64) / (window as f64);
    Some((100.0 - used_pct).clamp(0.0, 100.0))
}

/// Parse de `auth.json`. JSON inválido → `InvalidCredentials`.
/// Logado = existe entry com `key` não vazio. `expires_at` NÃO desloga:
/// o access token (6h) é renovado pelo Grok CLI via refresh_token, e este
/// provider é zero-rede — nem usa o token, só lê signals.json.
pub(crate) fn parse_auth_entries(
    bytes: &[u8],
    _now: OffsetDateTime,
) -> Result<AuthView, GrokError> {
    let map: HashMap<String, AuthEntry> =
        serde_json::from_slice(bytes).map_err(|_| GrokError::InvalidCredentials)?;

    // Entre entries com key não vazio, preferir a de expires_at mais distante
    // (desempate estável quando o CLI mantém múltiplas entradas).
    let mut best: Option<(OffsetDateTime, AuthEntry)> = None;
    for (_k, entry) in map {
        let key = entry.key.as_deref().unwrap_or("").trim();
        if key.is_empty() {
            continue;
        }
        let exp = entry
            .expires_at
            .as_deref()
            .and_then(parse_expires_at)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        match &best {
            Some((prev_exp, _)) if exp <= *prev_exp => {}
            _ => best = Some((exp, entry)),
        }
    }

    let Some((_exp, entry)) = best else {
        return Ok(AuthView {
            account: None,
            logged_in: false,
        });
    };

    Ok(AuthView {
        account: Some(account_label(&entry)),
        logged_in: true,
    })
}

fn account_label(entry: &AuthEntry) -> String {
    if let Some(name) = entry
        .first_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return name.to_string();
    }
    if let Some(uid) = entry
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let short: String = uid.chars().take(8).collect();
        return short;
    }
    "Grok".to_string()
}

/// RFC3339; tolerar fração de segundo com mais de 9 dígitos truncando.
fn parse_expires_at(s: &str) -> Option<OffsetDateTime> {
    if let Ok(dt) = OffsetDateTime::parse(s, &Rfc3339) {
        return Some(dt);
    }
    // Truncar nanos excedentes: "2026-07-18T03:29:45.462038593Z" ok;
    // se >9 dígitos fracionais, cortar.
    let truncated = truncate_rfc3339_fraction(s)?;
    OffsetDateTime::parse(&truncated, &Rfc3339).ok()
}

fn truncate_rfc3339_fraction(s: &str) -> Option<String> {
    let dot = s.find('.')?;
    let (head, frac_and_tz) = s.split_at(dot + 1);
    // frac_and_tz = "462038593Z" ou "462038593+00:00"
    let mut digits = 0usize;
    for (i, c) in frac_and_tz.char_indices() {
        if c.is_ascii_digit() {
            digits += 1;
            if digits > 9 {
                // cortar a partir daqui; manter sufixo TZ
                let tz = &frac_and_tz[i..];
                let kept = &frac_and_tz[..i];
                // kept tem 9 dígitos; se o original tinha mais, kept é os 9 primeiros
                // na verdade i aponta pro 10º dígito, kept = 9 dígitos
                return Some(format!("{head}{kept}{tz}"));
            }
        } else {
            // TZ começa
            if digits == 0 {
                return None;
            }
            return Some(s.to_string());
        }
    }
    Some(s.to_string())
}

/// Walk recursivo sob `sessions_dir` por arquivos `signals.json`.
/// Depth max 16, max 2000 visits; não segue symlinks.
/// `now_local_date` / `offset` reservados p/ contagem de “hoje” no build.
pub(crate) fn collect_signals(
    sessions_dir: &Path,
    _now_local_date: Date,
    _offset: UtcOffset,
) -> Vec<SessionSnap> {
    if !sessions_dir.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut visits = 0u32;
    walk_signals(sessions_dir, 0, &mut visits, &mut out);
    out.sort_by_key(|s| std::cmp::Reverse(s.mtime_ms));
    out
}

fn walk_signals(dir: &Path, depth: u32, visits: &mut u32, out: &mut Vec<SessionSnap>) {
    if depth > MAX_WALK_DEPTH || *visits >= MAX_WALK_VISITS {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if *visits >= MAX_WALK_VISITS {
            return;
        }
        *visits += 1;

        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        // Não seguir symlinks (evita sair de sessions).
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            walk_signals(&path, depth + 1, visits, out);
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        if entry.file_name() != "signals.json" {
            continue;
        }
        if let Some(snap) = parse_signals_file(&path) {
            out.push(snap);
        }
    }
}

fn parse_signals_file(path: &Path) -> Option<SessionSnap> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let mtime_ms = system_time_to_ms(mtime);
    let bytes = std::fs::read(path).ok()?;
    let file: SignalsFile = serde_json::from_slice(&bytes).ok()?;
    Some(SessionSnap {
        mtime_ms,
        context_tokens_used: file.context_tokens_used,
        context_window_tokens: file.context_window_tokens,
        primary_model_id: file.primary_model_id,
        turn_count: file.turn_count.unwrap_or(0),
    })
}

fn system_time_to_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn ms_to_local_date(mtime_ms: u64, offset: UtcOffset) -> Option<Date> {
    let dt = OffsetDateTime::from_unix_timestamp_nanos(i128::from(mtime_ms) * 1_000_000).ok()?;
    Some(dt.to_offset(offset).date())
}

/// `grok-4.5` → `Grok 4.5`; ids sem prefixo ficam como estão.
fn display_grok_model(id: &str) -> String {
    if let Some(rest) = id.strip_prefix("grok-") {
        if rest.is_empty() {
            return "Grok".to_string();
        }
        return format!("Grok {rest}");
    }
    id.to_string()
}

fn now_from_ctx(ctx: &Ctx<'_>) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(ctx.now_ms) * 1_000_000)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

fn local_today(ctx: &Ctx<'_>) -> Date {
    now_from_ctx(ctx).to_offset(ctx.local_offset).date()
}

fn pick_primary_session(sessions: &[SessionSnap]) -> Option<&SessionSnap> {
    // Já ordenados por mtime desc; primeira com window>0 e used presente.
    sessions
        .iter()
        .find(|s| s.context_tokens_used.is_some() && s.context_window_tokens.is_some_and(|w| w > 0))
}

fn build_primary_window(session: &SessionSnap) -> Option<QuotaWindow> {
    let used = session.context_tokens_used?;
    let window = session.context_window_tokens.filter(|w| *w > 0)?;
    let remaining = context_remaining_pct(used, window)?;
    let used_pct = 100.0 * (used as f64) / (window as f64);
    Some(QuotaWindow {
        remaining: remaining.round(),
        resets_at: None,
        window_minutes: None,
        used: Some(used_pct.round()),
        severity: None,
        window_kind: None,
    })
}

fn count_today(sessions: &[SessionSnap], today: Date, offset: UtcOffset) -> (u32, u32) {
    let mut sessions_today = 0u32;
    let mut turns_today = 0u32;
    for s in sessions {
        let Some(d) = ms_to_local_date(s.mtime_ms, offset) else {
            continue;
        };
        if d == today {
            sessions_today = sessions_today.saturating_add(1);
            turns_today = turns_today.saturating_add(s.turn_count);
        }
    }
    (sessions_today, turns_today)
}

pub struct GrokProvider;

#[async_trait(?Send)]
impl QuotaSource for GrokProvider {
    type Raw = GrokRaw;

    fn id(&self) -> &'static str {
        "grok"
    }

    fn name(&self) -> &'static str {
        "Grok"
    }

    fn cache_key(&self) -> &'static str {
        "grok-usage"
    }

    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        ctx.paths.grok_auth.is_file()
            || find_grok_bin(
                &ctx.home.to_string_lossy(),
                Some(ctx.paths.grok_home.as_path()),
            )
            .is_some()
    }

    async fn fetch_raw(&self, ctx: &Ctx<'_>) -> Result<GrokRaw, ProviderError> {
        let auth_path = &ctx.paths.grok_auth;
        let bytes = match std::fs::read(auth_path) {
            Ok(b) => b,
            Err(_) => return Err(GrokError::NotLoggedIn.into()),
        };
        let auth = parse_auth_entries(&bytes, now_from_ctx(ctx))?;
        let sessions_dir = ctx.paths.grok_home.join("sessions");
        let sessions = collect_signals(&sessions_dir, local_today(ctx), ctx.local_offset);
        Ok(GrokRaw {
            account: auth.account,
            logged_in: auth.logged_in,
            sessions,
        })
    }

    fn build_quota(&self, raw: GrokRaw, base: ProviderQuota, ctx: &Ctx<'_>) -> ProviderQuota {
        if !raw.logged_in {
            return ProviderQuota {
                error: Some(GrokError::NotLoggedIn.to_string()),
                account: raw.account,
                ..base
            };
        }

        let today = local_today(ctx);
        let (sessions_today, turns_today) = count_today(&raw.sessions, today, ctx.local_offset);

        let primary_session = pick_primary_session(&raw.sessions);
        let primary = primary_session.and_then(build_primary_window);

        let recent_model = primary_session
            .and_then(|s| s.primary_model_id.clone())
            .filter(|m| !m.is_empty());

        let plan = recent_model
            .as_deref()
            .map(display_grok_model)
            .or_else(|| Some("Grok Build".to_string()));

        let models = match (primary.as_ref(), recent_model.as_deref()) {
            (Some(w), Some(id)) => {
                let mut m = IndexMap::new();
                m.insert(display_grok_model(id), w.clone());
                Some(m)
            }
            _ => None,
        };

        let extra = ProviderExtra::Grok(GrokQuotaExtra {
            sessions_today: Some(sessions_today),
            turns_today: Some(turns_today),
            context_tokens_used: primary_session.and_then(|s| s.context_tokens_used),
            context_window_tokens: primary_session.and_then(|s| s.context_window_tokens),
            recent_model,
        });

        ProviderQuota {
            available: true,
            account: raw.account,
            plan,
            primary,
            models,
            extra: Some(extra),
            ..base
        }
    }

    fn unavailable_error(&self) -> String {
        GrokError::NotInstalled.to_string()
    }

    fn to_user_facing_error(&self, error: &ProviderError) -> String {
        match error {
            ProviderError::Grok(e) => e.to_string(),
            _ => GrokError::NotLoggedIn.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl Provider for GrokProvider {
    fn id(&self) -> &'static str {
        "grok"
    }

    fn name(&self) -> &'static str {
        "Grok"
    }

    fn cache_key(&self) -> &'static str {
        "grok-usage"
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
    use crate::providers::test_support::{ctx_for, settings};
    use std::path::PathBuf;
    use tempfile::tempdir;
    use time::macros::{date, datetime};

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/grok");

    fn fixture_bytes(name: &str) -> Vec<u8> {
        std::fs::read(PathBuf::from(FIXTURES).join(name))
            .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
    }

    fn fixture_str(name: &str) -> String {
        String::from_utf8(fixture_bytes(name)).expect("utf8 fixture")
    }

    fn write_auth(dir: &Path, name: &str) {
        let auth = dir.join("grok").join("auth.json");
        if let Some(parent) = auth.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&auth, fixture_bytes(name)).unwrap();
    }

    fn write_signals(dir: &Path, rel: &str, body: &str) -> PathBuf {
        let path = dir.join("grok").join("sessions").join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, body).unwrap();
        path
    }

    fn touch_bin(home: &Path) {
        let p = home.join(".grok").join("bin").join("grok");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::File::create(&p).unwrap();
    }

    #[test]
    fn context_remaining_pct_happy() {
        assert_eq!(context_remaining_pct(50_000, 500_000), Some(90.0));
    }

    #[test]
    fn context_remaining_pct_none_on_zero_window() {
        assert_eq!(context_remaining_pct(0, 0), None);
        assert_eq!(context_remaining_pct(10, 0), None);
    }

    #[test]
    fn context_remaining_pct_full_and_clamp() {
        assert_eq!(context_remaining_pct(500_000, 500_000), Some(0.0));
        // used > window → clamp a 0
        assert_eq!(context_remaining_pct(600_000, 500_000), Some(0.0));
    }

    #[test]
    fn parse_auth_valid() {
        let now = datetime!(2026-07-17 12:00:00 UTC);
        let view = parse_auth_entries(&fixture_bytes("auth-valid.json"), now).unwrap();
        assert!(view.logged_in);
        assert_eq!(view.account.as_deref(), Some("Test User"));
    }

    #[test]
    fn parse_auth_expired_token_still_logged_in() {
        let now = datetime!(2026-07-17 12:00:00 UTC);
        let view = parse_auth_entries(&fixture_bytes("auth-expired.json"), now).unwrap();
        // Access token vencido ≠ logout: o Grok CLI renova via refresh_token.
        assert!(view.logged_in);
        assert_eq!(view.account.as_deref(), Some("Test User"));
    }

    #[test]
    fn parse_auth_empty_key_not_logged_in() {
        let now = datetime!(2026-07-17 12:00:00 UTC);
        let json = br#"{"https://auth.x.ai::c": {"key": "", "first_name": "X"}}"#;
        let view = parse_auth_entries(json, now).unwrap();
        assert!(!view.logged_in);
        assert!(view.account.is_none());
    }

    #[test]
    fn parse_auth_invalid_json() {
        let now = datetime!(2026-07-17 12:00:00 UTC);
        let err = parse_auth_entries(b"not-json", now).unwrap_err();
        assert_eq!(err.to_string(), "Failed to read Grok credentials.");
    }

    #[test]
    fn parse_auth_nanos_fraction() {
        let now = datetime!(2026-07-17 12:00:00 UTC);
        let json = br#"{
          "https://auth.x.ai::c": {
            "key": "tok",
            "expires_at": "2099-07-18T03:29:45.462038593Z",
            "first_name": "Nano"
          }
        }"#;
        let view = parse_auth_entries(json, now).unwrap();
        assert!(view.logged_in);
        assert_eq!(view.account.as_deref(), Some("Nano"));
    }

    #[test]
    fn display_grok_model_formats() {
        assert_eq!(display_grok_model("grok-4.5"), "Grok 4.5");
        assert_eq!(display_grok_model("other"), "other");
    }

    /// Isola `PATH` (host pode ter `grok` real). Mutex evita corrida entre testes.
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct PathGuard {
        old: Option<std::ffi::OsString>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl PathGuard {
        fn clear() -> Self {
            let lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let old = std::env::var_os("PATH");
            // PATH vazio: `which` não encontra binários do host.
            std::env::set_var("PATH", "");
            Self { old, _lock: lock }
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(p) => std::env::set_var("PATH", p),
                None => std::env::remove_var("PATH"),
            }
        }
    }

    #[tokio::test]
    async fn not_installed() {
        let _path = PathGuard::clear();
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        // now_ms irrelevante; sem auth e sem bin sob home de teste.
        let ctx = ctx_for(dir.path(), &s, &client, 1_720_000_000_000);
        let q = GrokProvider.get_quota(&ctx).await;
        assert!(!q.available);
        assert_eq!(
            q.error.as_deref(),
            Some(
                "Grok CLI not installed. Install from https://x.ai/cli or ensure ~/.grok/bin/grok is on PATH."
            )
        );
    }

    #[tokio::test]
    async fn not_logged_in_missing_auth() {
        let _path = PathGuard::clear();
        let dir = tempdir().unwrap();
        // bin sob home de teste → available, mas sem auth → NotLoggedIn
        touch_bin(dir.path());
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 1_720_000_000_000);
        let q = GrokProvider.get_quota(&ctx).await;
        assert!(!q.available);
        assert_eq!(
            q.error.as_deref(),
            Some("Not logged in. Open `agent-bar menu` and choose Provider login.")
        );
    }

    #[tokio::test]
    async fn expired_access_token_still_serves_quota() {
        let dir = tempdir().unwrap();
        write_auth(dir.path(), "auth-expired.json");
        write_signals(
            dir.path(),
            "proj/sid/signals.json",
            &fixture_str("signals-recent.json"),
        );
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 1_720_000_000_000);
        let q = GrokProvider.get_quota(&ctx).await;
        assert!(q.available, "err={:?}", q.error);
        assert_eq!(q.account.as_deref(), Some("Test User"));
        assert_eq!(q.primary.as_ref().unwrap().remaining, 90.0);
    }

    #[tokio::test]
    async fn happy_path_remaining_90() {
        let dir = tempdir().unwrap();
        write_auth(dir.path(), "auth-valid.json");
        write_signals(
            dir.path(),
            "proj/sid/signals.json",
            &fixture_str("signals-recent.json"),
        );
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 1_720_000_000_000);
        let q = GrokProvider.get_quota(&ctx).await;
        assert!(q.available, "err={:?}", q.error);
        assert_eq!(q.account.as_deref(), Some("Test User"));
        let primary = q.primary.as_ref().expect("primary");
        assert_eq!(primary.remaining, 90.0);
        assert!(primary.resets_at.is_none());
        assert_eq!(primary.used, Some(10.0));
        assert_eq!(q.plan.as_deref(), Some("Grok 4.5"));
        match q.extra.as_ref() {
            Some(ProviderExtra::Grok(e)) => {
                assert_eq!(e.context_tokens_used, Some(50_000));
                assert_eq!(e.context_window_tokens, Some(500_000));
                assert_eq!(e.recent_model.as_deref(), Some("grok-4.5"));
            }
            other => panic!("expected Grok extra, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn picks_most_recent_session() {
        let dir = tempdir().unwrap();
        write_auth(dir.path(), "auth-valid.json");
        let old = write_signals(
            dir.path(),
            "proj/old/signals.json",
            &fixture_str("signals-full.json"),
        );
        let new = write_signals(
            dir.path(),
            "proj/new/signals.json",
            &fixture_str("signals-recent.json"),
        );
        // mtimes: old no passado, new no futuro.
        let past = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let future = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000);
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(past)).unwrap();
        filetime::set_file_mtime(&new, filetime::FileTime::from_system_time(future)).unwrap();

        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 1_720_000_000_000);
        let q = GrokProvider.get_quota(&ctx).await;
        assert!(q.available, "err={:?}", q.error);
        // recent = 90%, full = 0%
        assert_eq!(q.primary.as_ref().unwrap().remaining, 90.0);
        assert_eq!(
            q.extra.as_ref().and_then(|e| match e {
                ProviderExtra::Grok(g) => g.context_tokens_used,
                _ => None,
            }),
            Some(50_000)
        );
    }

    #[tokio::test]
    async fn ignores_corrupt_signals() {
        let dir = tempdir().unwrap();
        write_auth(dir.path(), "auth-valid.json");
        write_signals(dir.path(), "proj/bad/signals.json", "NOT JSON{{{");
        write_signals(
            dir.path(),
            "proj/good/signals.json",
            &fixture_str("signals-recent.json"),
        );
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 1_720_000_000_000);
        let q = GrokProvider.get_quota(&ctx).await;
        assert!(q.available, "err={:?}", q.error);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 90.0);
    }

    #[test]
    fn collect_signals_sorts_and_parses() {
        let dir = tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let a = sessions.join("a/signals.json");
        let b = sessions.join("b/signals.json");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, fixture_str("signals-recent.json")).unwrap();
        std::fs::write(&b, fixture_str("signals-full.json")).unwrap();
        let past = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let future = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(200);
        filetime::set_file_mtime(&a, filetime::FileTime::from_system_time(past)).unwrap();
        filetime::set_file_mtime(&b, filetime::FileTime::from_system_time(future)).unwrap();

        let snaps = collect_signals(&sessions, date!(2026 - 07 - 17), UtcOffset::UTC);
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[0].context_tokens_used, Some(500_000)); // b first (newer)
        assert_eq!(snaps[1].context_tokens_used, Some(50_000));
    }

    #[test]
    fn grok_raw_does_not_serialize_jwt_fields() {
        let raw = GrokRaw {
            account: Some("Test".into()),
            logged_in: true,
            sessions: vec![],
        };
        let j = serde_json::to_value(&raw).unwrap();
        let s = j.to_string();
        assert!(!s.contains("refresh"));
        // snake_case default; sem campos de JWT
        assert_eq!(j["logged_in"], true);
        assert_eq!(j["account"], "Test");
        assert!(j.get("key").is_none());
        assert!(j.get("refresh_token").is_none());
        let _ = s;
    }
}
