//! Cache persistente de `UsageRecord` por arquivo (redb + postcard).
//! Chave: path do arquivo (string); valor: postcard de (size, mtime,
//! Vec<UsageRecord>). O cache é DERIVADO: qualquer erro (corrupção, versão
//! velha, IO) degrada pra re-parse — nunca panica, nunca é fonte de verdade.
//!
//! Duas camadas: L1 em memória (por processo, evita round-trip no redb
//! dentro da mesma chamada de `records()`) e L2 persistente (sobrevive entre
//! execuções — é o ganho real de performance).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[cfg(test)]
use redb::ReadableTableMetadata;
use redb::{Database, ReadableTable, TableDefinition};

use super::UsageRecord;

const FILES: TableDefinition<&str, &[u8]> = TableDefinition::new("files");
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");

/// Bump SEMPRE que `UsageRecord` ou a semântica do parse mudar (dedup, campos
/// novos) — força re-parse geral e corrige o histórico inteiro.
pub const CACHE_VERSION: u64 = 2;

/// Chave de cache por arquivo: (tamanho em bytes, mtime Unix segundos).
type FileKey = (u64, i64);

#[derive(serde::Serialize, serde::Deserialize)]
struct Entry {
    size: u64,
    mtime: i64,
    records: Vec<UsageRecord>,
}

/// Cache incremental (L1 memória) + persistente (L2 redb) de `UsageRecord`
/// por arquivo. Ver doc do módulo pra invariantes de degradação.
pub struct UsageCache {
    memory: HashMap<PathBuf, (FileKey, Vec<UsageRecord>)>,
    db: Option<Database>,
}

impl UsageCache {
    /// Abre (ou cria) o cache persistente em `db_path`. `None` = só memória
    /// (comportamento do cache anterior) — usado pelos testes que não
    /// precisam de persistência entre processos.
    pub fn open(db_path: Option<&Path>) -> Self {
        Self::open_with_version(db_path, CACHE_VERSION)
    }

    /// Como [`open`], mas com uma versão de cache explícita — usado em teste
    /// pra simular um bump de `CACHE_VERSION` sem recompilar.
    pub fn open_with_version(db_path: Option<&Path>, version: u64) -> Self {
        let db = db_path.and_then(open_db);
        let mut cache = Self {
            memory: HashMap::new(),
            db,
        };
        cache.enforce_version(version);
        cache
    }

    /// Compara a versão gravada em `META` com `expected`; se diferente (ou
    /// ausente, DB novo), dropa `FILES` inteira e grava a versão nova.
    fn enforce_version(&mut self, expected: u64) {
        let Some(db) = self.db.as_ref() else {
            return;
        };

        if read_meta_version(db) == Some(expected) {
            return;
        }

        let result = (|| -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                txn.delete_table(FILES)?;
                let mut meta = txn.open_table(META)?;
                meta.insert("version", expected)?;
            }
            txn.commit()?;
            Ok(())
        })();

        if let Err(e) = result {
            log::warn!("usage cache: falha ao aplicar versão nova ({e}); rodando só em memória");
            self.db = None;
        }
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

        if let Some((cached_key, records)) = self.memory.get(path) {
            if *cached_key == key {
                return records.clone();
            }
        }

        if let Some(records) = self.read_persisted(path, key) {
            self.memory
                .insert(path.to_path_buf(), (key, records.clone()));
            return records;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let records = parse(&content);
        self.persist(path, key, &records);
        self.memory
            .insert(path.to_path_buf(), (key, records.clone()));
        records
    }

    /// Lê o cache persistente para `path`. `None` = miss (chave ausente,
    /// (size, mtime) não bate, decode falhou, ou qualquer erro de redb —
    /// tudo degrada pra "reparseie", nunca panica).
    fn read_persisted(&self, path: &Path, key: FileKey) -> Option<Vec<UsageRecord>> {
        let db = self.db.as_ref()?;
        let map_key = path.to_string_lossy();

        let result = (|| -> anyhow::Result<Option<Vec<UsageRecord>>> {
            let txn = db.begin_read()?;
            let table = match txn.open_table(FILES) {
                Ok(t) => t,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
                Err(e) => return Err(e.into()),
            };
            let Some(guard) = table.get(map_key.as_ref())? else {
                return Ok(None);
            };
            let bytes = guard.value();
            let entry: Entry = match postcard::from_bytes(bytes) {
                Ok(e) => e,
                Err(_) => return Ok(None), // decode falho = miss silencioso
            };
            if entry.size == key.0 && entry.mtime == key.1 {
                Ok(Some(entry.records))
            } else {
                Ok(None)
            }
        })();

        match result {
            Ok(v) => v,
            Err(e) => {
                log::warn!("usage cache: leitura de {path:?} falhou ({e}); reparseando");
                None
            }
        }
    }

    /// Grava `records` no cache persistente sob `path`. Falha (encode ou
    /// redb) só loga — o cache é derivado, nunca é a fonte de verdade.
    fn persist(&self, path: &Path, key: FileKey, records: &[UsageRecord]) {
        let Some(db) = self.db.as_ref() else {
            return;
        };

        let entry = Entry {
            size: key.0,
            mtime: key.1,
            records: records.to_vec(),
        };
        let bytes = match postcard::to_allocvec(&entry) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("usage cache: encode de {path:?} falhou ({e}); não persistido");
                return;
            }
        };

        let map_key = path.to_string_lossy();
        let result = (|| -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(FILES)?;
                table.insert(map_key.as_ref(), bytes.as_slice())?;
            }
            txn.commit()?;
            Ok(())
        })();

        if let Err(e) = result {
            log::warn!("usage cache: escrita de {path:?} falhou ({e}); não persistido");
        }
    }

    /// Remove do cache (memória + redb) todo path que não esteja em `live`.
    /// Chamado ao final de `records()` (`usage/mod.rs`) pra não acumular
    /// entradas de arquivos deletados/rotacionados indefinidamente.
    pub fn gc(&mut self, live: &HashSet<PathBuf>) {
        self.memory.retain(|p, _| live.contains(p));

        let Some(db) = self.db.as_ref() else {
            return;
        };

        let result = (|| -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(FILES)?;
                let dead: Vec<String> = {
                    table
                        .iter()?
                        .filter_map(|entry| entry.ok())
                        .map(|(k, _v)| k.value().to_string())
                        .filter(|k| !live.contains(Path::new(k.as_str())))
                        .collect()
                };
                for k in dead {
                    table.remove(k.as_str())?;
                }
            }
            txn.commit()?;
            Ok(())
        })();

        if let Err(e) = result {
            log::warn!("usage cache: gc falhou ({e})");
        }
    }

    /// Conta as chaves persistidas em `FILES`. Só pra teste.
    #[cfg(test)]
    pub fn persisted_len(&self) -> usize {
        let Some(db) = self.db.as_ref() else {
            return 0;
        };

        let result = (|| -> anyhow::Result<usize> {
            let txn = db.begin_read()?;
            let table = match txn.open_table(FILES) {
                Ok(t) => t,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
                Err(e) => return Err(e.into()),
            };
            Ok(table.len()? as usize)
        })();

        result.unwrap_or(0)
    }
}

/// Abre (ou cria) o redb em `path`. Corrupção/erro de abertura → apaga e
/// tenta uma vez mais; falha de novo → `None` (cache degrada pra memória).
/// Nunca panica.
fn open_db(path: &Path) -> Option<Database> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match Database::create(path) {
        Ok(db) => Some(db),
        Err(e) => {
            log::warn!("usage cache: abrir {path:?} falhou ({e}); tentando recriar");
            let _ = std::fs::remove_file(path);
            match Database::create(path) {
                Ok(db) => Some(db),
                Err(e2) => {
                    log::warn!(
                        "usage cache: recriação de {path:?} falhou ({e2}); rodando só em memória"
                    );
                    None
                }
            }
        }
    }
}

/// Lê a versão gravada em `META["version"]`. `None` = tabela ainda não
/// existe (DB novo) ou qualquer erro de leitura (degrada pra "versão nova").
fn read_meta_version(db: &Database) -> Option<u64> {
    let txn = db.begin_read().ok()?;
    let table = match txn.open_table(META) {
        Ok(t) => t,
        Err(_) => return None,
    };
    table.get("version").ok()?.map(|g| g.value())
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

    #[test]
    fn persistent_cache_survives_reopen() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("usage.redb");
        let f = dir.path().join("s.jsonl");
        std::fs::write(&f, "conteudo").unwrap();

        let mut c1 = UsageCache::open(Some(&db));
        let r1 = c1.cached_or_parse(&f, |_| vec![dummy_record()]);
        assert_eq!(r1.len(), 1);
        drop(c1);

        // Reabrir: mesmo (size, mtime) → NÃO re-parseia.
        let mut c2 = UsageCache::open(Some(&db));
        let r2 = c2.cached_or_parse(&f, |_| panic!("não deveria reparsear"));
        assert_eq!(r2, r1);
    }

    #[test]
    fn changed_file_reparses_and_version_bump_drops_everything() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("usage.redb");
        let f = dir.path().join("s.jsonl");
        std::fs::write(&f, "v1").unwrap();
        let mut c = UsageCache::open(Some(&db));
        let _ = c.cached_or_parse(&f, |_| vec![dummy_record()]);
        std::fs::write(&f, "v2 maior").unwrap(); // size muda → key muda
        let r = c.cached_or_parse(&f, |_| vec![]);
        assert!(r.is_empty(), "arquivo mudado re-parseia");
        drop(c);
        // Simular bump de versão: gravar meta antiga e reabrir.
        let mut c = UsageCache::open_with_version(Some(&db), CACHE_VERSION + 1);
        let r = c.cached_or_parse(&f, |_| vec![dummy_record(), dummy_record()]);
        assert_eq!(r.len(), 2, "versão nova invalida tudo");
    }

    #[test]
    fn corrupted_db_is_rebuilt_not_fatal() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("usage.redb");
        std::fs::write(&db, b"isto nao e um redb").unwrap();
        let mut c = UsageCache::open(Some(&db)); // não pode panicar
        let f = dir.path().join("s.jsonl");
        std::fs::write(&f, "x").unwrap();
        assert_eq!(c.cached_or_parse(&f, |_| vec![dummy_record()]).len(), 1);
    }

    #[test]
    fn gc_removes_dead_paths() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("usage.redb");
        let f = dir.path().join("s.jsonl");
        std::fs::write(&f, "x").unwrap();
        let mut c = UsageCache::open(Some(&db));
        let _ = c.cached_or_parse(&f, |_| vec![dummy_record()]);
        c.gc(&std::collections::HashSet::new()); // nenhum path vivo
        assert_eq!(c.persisted_len(), 0);
    }
}
