//! Fallback de session-log do Codex.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use time::{Duration as TimeDuration, OffsetDateTime, UtcOffset};

use super::types::CodexRateLimits;

// ---- Fallback session-log ----

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
