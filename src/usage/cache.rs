//! Cache incremental de `UsageRecord` por arquivo: re-parseia apenas quando
//! size ou mtime mudaram. Índice em memória por chamada de `aggregate`.
//!
//! TODO (polish): persistir o índice em `cache_dir/usage-index.json` entre
//! chamadas (serialização com serde_json, escrita atômica via tempfile).
//! O ganho de não re-parsear sessões antigas já vem do índice em memória.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::UsageRecord;

/// Chave de cache por arquivo: (tamanho em bytes, mtime Unix segundos).
type FileKey = (u64, i64);

/// Índice em memória: `path → (key, records)`.
#[derive(Debug, Default)]
pub struct UsageCache {
    index: HashMap<PathBuf, (FileKey, Vec<UsageRecord>)>,
}

impl UsageCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Devolve os `UsageRecord` do `path`, re-parseando somente se size ou
    /// mtime mudaram. Erros de IO (arquivo inexistente, permissão) → `vec![]`.
    pub fn cached_or_parse(
        &mut self,
        path: &Path,
        parse: impl FnOnce(&str) -> Vec<UsageRecord>,
    ) -> Vec<UsageRecord> {
        let key = match file_key(path) {
            Some(k) => k,
            None => return vec![],
        };

        if let Some((cached_key, records)) = self.index.get(path) {
            if *cached_key == key {
                return records.clone();
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let records = parse(&content);
        self.index
            .insert(path.to_path_buf(), (key, records.clone()));
        records
    }
}

/// Lê `(size, mtime_unix_secs)` do arquivo via `std::fs::metadata`.
/// Retorna `None` em erro de IO.
fn file_key(path: &Path) -> Option<FileKey> {
    let meta = std::fs::metadata(path).ok()?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
        })
        .unwrap_or(0);
    Some((size, mtime))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use time::macros::datetime;

    fn dummy_record() -> UsageRecord {
        UsageRecord {
            provider: "test".into(),
            model: None,
            input: 1,
            output: 1,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: 0,
            fast: false,
            geo_us: false,
            ts: datetime!(2026-06-19 12:00 UTC),
        }
    }

    fn parse_once(content: &str) -> Vec<UsageRecord> {
        if content.trim().is_empty() {
            vec![]
        } else {
            vec![dummy_record()]
        }
    }

    #[test]
    fn cache_hit_does_not_reparse() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        std::fs::write(&path, "some content").unwrap();

        let mut cache = UsageCache::new();

        // Primeira chamada: parseia (retorna 1 record).
        let r1 = cache.cached_or_parse(&path, parse_once);
        assert_eq!(r1.len(), 1);

        // Segunda chamada com mesmo arquivo: deve devolver do cache sem chamar o parser.
        let _ = cache.cached_or_parse(&path, |_| {
            panic!("não deveria reparsear");
        });
    }

    #[test]
    fn cache_miss_on_changed_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        std::fs::write(&path, "v1").unwrap();

        let mut cache = UsageCache::new();
        let r1 = cache.cached_or_parse(&path, parse_once);
        assert_eq!(r1.len(), 1);

        // Muda o arquivo (size diferente → key muda).
        std::fs::write(&path, "").unwrap();
        let r2 = cache.cached_or_parse(&path, parse_once);
        assert_eq!(r2.len(), 0);
    }

    #[test]
    fn nonexistent_file_returns_empty() {
        let mut cache = UsageCache::new();
        let r = cache.cached_or_parse(Path::new("/nonexistent/path/file.jsonl"), parse_once);
        assert!(r.is_empty());
    }
}
