//! Cache de arquivo atômico, cross-process, com TTL.
//! - Escrita atômica: tempfile no mesmo diretório + rename.
//! - Erros NUNCA são cacheados (o caller só chama `set` no sucesso).
//! - `now_ms` é injetado para testabilidade.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("Invalid cache key: \"{0}\"")]
    InvalidKey(String),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheEntryRef<'a, T> {
    data: &'a T,
    fetched_at: u64,
    expires_at: u64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheEntryOwned<T> {
    data: T,
    #[allow(dead_code)]
    fetched_at: u64,
    expires_at: u64,
}

fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn cache_path(cache_dir: &Path, key: &str) -> Result<PathBuf, CacheError> {
    if !is_valid_key(key) {
        return Err(CacheError::InvalidKey(key.to_string()));
    }
    Ok(cache_dir.join(format!("{key}.json")))
}

/// Lê o cache se válido; `None` em miss, expirado, corrompido ou key inválida.
pub fn get<T: DeserializeOwned>(cache_dir: &Path, key: &str, now_ms: u64) -> Option<T> {
    let path = cache_path(cache_dir, key).ok()?;
    let bytes = std::fs::read(&path).ok()?;
    let entry: CacheEntryOwned<T> = serde_json::from_slice(&bytes).ok()?;
    if now_ms > entry.expires_at {
        return None;
    }
    Some(entry.data)
}

/// Escreve atomicamente (tempfile + rename). `mkdir -p` no primeiro write.
pub fn set<T: Serialize>(
    cache_dir: &Path,
    key: &str,
    data: &T,
    ttl_ms: u64,
    now_ms: u64,
) -> anyhow::Result<()> {
    let path = cache_path(cache_dir, key)?;
    std::fs::create_dir_all(cache_dir)?;

    let entry = CacheEntryRef {
        data,
        fetched_at: now_ms,
        expires_at: now_ms.saturating_add(ttl_ms),
    };
    let json = serde_json::to_string_pretty(&entry)?;

    let mut tmp = tempfile::NamedTempFile::new_in(cache_dir)?;
    tmp.write_all(json.as_bytes())?;
    tmp.persist(&path)?;
    Ok(())
}

/// Remove a entrada (no-op se ausente ou key inválida).
pub fn invalidate(cache_dir: &Path, key: &str) {
    if let Ok(path) = cache_path(cache_dir, key) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_invalid_keys() {
        let dir = tempdir().unwrap();
        for bad in ["", "a/b", "../x", "a.b", "a b", "claude!"] {
            assert!(
                cache_path(dir.path(), bad).is_err(),
                "key {bad:?} deveria falhar"
            );
        }
        assert!(cache_path(dir.path(), "claude-usage").is_ok());
        assert!(cache_path(dir.path(), "codex_quota1").is_ok());
    }

    #[test]
    fn set_then_get_roundtrips_within_ttl() {
        let dir = tempdir().unwrap();
        set(dir.path(), "k", &vec![1u32, 2, 3], 60_000, 1_000).unwrap();
        let got: Option<Vec<u32>> = get(dir.path(), "k", 30_000);
        assert_eq!(got, Some(vec![1, 2, 3]));
    }

    #[test]
    fn get_returns_none_after_expiry() {
        let dir = tempdir().unwrap();
        set(dir.path(), "k", &"v".to_string(), 5_000, 1_000).unwrap();
        // now (10_000) > expires_at (1_000 + 5_000 = 6_000)
        let got: Option<String> = get(dir.path(), "k", 10_000);
        assert_eq!(got, None);
    }

    #[test]
    fn get_returns_none_on_missing_or_corrupt() {
        let dir = tempdir().unwrap();
        let missing: Option<String> = get(dir.path(), "nope", 0);
        assert_eq!(missing, None);

        std::fs::write(dir.path().join("bad.json"), b"{ not json").unwrap();
        let corrupt: Option<String> = get(dir.path(), "bad", 0);
        assert_eq!(corrupt, None);
    }

    #[test]
    fn invalidate_removes_entry() {
        let dir = tempdir().unwrap();
        set(dir.path(), "k", &1u32, 60_000, 0).unwrap();
        invalidate(dir.path(), "k");
        let got: Option<u32> = get(dir.path(), "k", 0);
        assert_eq!(got, None);
    }
}
