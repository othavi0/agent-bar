# Reescrita Rust — Plano 01: Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Estabelecer o crate Rust e as 3 camadas puras/sync de fundação (config+Paths, cache atômico, settings) — totalmente testadas, sem providers nem async.

**Architecture:** Pacote único `agent-bar` no subdiretório `rust/` (coexiste com o TS na raiz durante a migração; promovido à raiz no cutover da Layer 8). Sem estado global: paths XDG resolvidos no `main` e injetados (`Paths`). Cache de arquivo atômico (tempfile + rename). Settings com split raw→typed (deserialize leniente, normalize coage valores inválidos pro default).

**Tech Stack:** Rust 2021, serde/serde_json, thiserror/anyhow, time 0.3, tempfile, log/env_logger; testes com assert_cmd/predicates/serial_test/temp-env.

## Global Constraints

- **Linux-only.** Sem suporte Windows/macOS.
- **stdout limpo:** só payload de máquina; todo log/diagnóstico vai pra **stderr** (`env_logger` com `Target::Stderr`).
- **Sem `unwrap()`/`expect()` em código de produção** (espelha o ban de `!` do projeto). Enforçar adicionando no topo do `rust/src/lib.rs`: `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]` (libera nos `#[cfg(test)]`). Em testes é permitido.
- **Sem estado global mutável** (sem `OnceLock`/`lazy_static` de config): `Paths` é resolvido no `main` e injetado. Tudo testável passando `Paths` de `tempdir`.
- **Crate vive em `rust/`** durante a migração. Todos os comandos `cargo` usam `--manifest-path rust/Cargo.toml`.
- **Guard de cache key:** `^[a-zA-Z0-9_-]+$` e não-vazio; key inválida → `Err(CacheError::InvalidKey)`, nunca path traversal.
- **Escrita atômica:** sempre tempfile no mesmo diretório + `rename` (cache e settings).
- **Conventional Commits em PT**, subject ≤ 50 chars.

---

### Task 1: Scaffold do crate

**Files:**
- Create: `rust/Cargo.toml`
- Create: `rust/build.rs`
- Create: `rust/src/main.rs`
- Create: `rust/src/lib.rs`
- Create: `rust/src/app_identity.rs`
- Create: `rust/src/logger.rs`
- Create: `rust/.gitignore`
- Create: `rust/tests/cli.rs`

**Interfaces:**
- Produces: `agent_bar::app_identity::{APP_NAME, WAYBAR_NAMESPACE, WAYBAR_MODULE_PREFIX, WAYBAR_SELECTOR_PREFIX, TERMINAL_HELPER_NAME, BACKUP_SUFFIX, APP_HIDDEN_CLASS, AMP_INSTALL_COMMAND, VERSION}` (todas `&'static str`); `agent_bar::logger::init(verbose: bool)`.

- [ ] **Step 1: Criar `rust/Cargo.toml`**

```toml
[package]
name = "agent-bar"
version = "6.0.0"
edition = "2021"
rust-version = "1.80"
license = "MIT"
description = "LLM quota monitor for Waybar - Claude, Codex, Amp"

[[bin]]
name = "agent-bar"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
time = { version = "0.3", features = ["serde-well-known", "local-offset", "formatting", "parsing"] }
tempfile = "3"
log = "0.4"
env_logger = "0.11"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
serial_test = "3"
temp-env = "0.3"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

- [ ] **Step 2: Criar `rust/.gitignore`**

```gitignore
/target
```

- [ ] **Step 3: Criar `rust/build.rs`** (placeholder p/ futura const de asset dir; hoje só garante rerun barato)

```rust
fn main() {
    // SYSTEM_ASSET_DIR será embutido aqui na Layer 7 (asset resolution).
    println!("cargo:rerun-if-changed=build.rs");
}
```

- [ ] **Step 4: Criar `rust/src/app_identity.rs`**

```rust
//! Constantes de identidade. Use estas em vez de strings hardcoded.

pub const APP_NAME: &str = "agent-bar";
pub const WAYBAR_NAMESPACE: &str = "agent-bar";
pub const WAYBAR_MODULE_PREFIX: &str = "custom/agent-bar-";
pub const WAYBAR_SELECTOR_PREFIX: &str = "#custom-agent-bar-";
pub const TERMINAL_HELPER_NAME: &str = "agent-bar-open-terminal";
pub const BACKUP_SUFFIX: &str = ".agent-bar-backup";
pub const APP_HIDDEN_CLASS: &str = "agent-bar-hidden";
pub const AMP_INSTALL_COMMAND: &str = "curl -fsSL https://ampcode.com/install.sh | bash";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

- [ ] **Step 5: Criar `rust/src/logger.rs`**

```rust
//! Logging exclusivamente para stderr. stdout é reservado para payload de máquina.

/// Inicializa o logger global em stderr. `try_init` evita panic em re-init (testes).
pub fn init(verbose: bool) {
    let level = if verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    };
    let _ = env_logger::Builder::new()
        .filter_level(level)
        .target(env_logger::Target::Stderr)
        .try_init();
}
```

- [ ] **Step 6: Criar `rust/src/lib.rs`**

```rust
pub mod app_identity;
pub mod logger;
```

- [ ] **Step 7: Criar `rust/src/main.rs`** (mínimo; o CLI real é a Layer 6)

```rust
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", agent_bar::app_identity::VERSION);
        return;
    }
    eprintln!("agent-bar: CLI ainda não implementado (reescrita em andamento)");
    std::process::exit(1);
}
```

- [ ] **Step 8: Escrever o teste de integração `rust/tests/cli.rs`**

```rust
use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn prints_version() {
    Command::cargo_bin("agent-bar")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}
```

- [ ] **Step 9: Rodar o teste (deve passar após build)**

Run: `cargo test --manifest-path rust/Cargo.toml --test cli`
Expected: PASS (`prints_version`)

- [ ] **Step 10: Conferir lint/format**

Run: `cargo fmt --manifest-path rust/Cargo.toml && cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`
Expected: sem erros.

- [ ] **Step 11: Commit**

```bash
git add rust/Cargo.toml rust/build.rs rust/.gitignore rust/src/ rust/tests/cli.rs
git commit -m "feat(rust): scaffold do crate agent-bar"
```

---

### Task 2: `config` — Paths (DI) + constantes + status

**Files:**
- Create: `rust/src/config.rs`
- Modify: `rust/src/lib.rs` (adicionar `pub mod config;`)
- Test: dentro de `config.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: nada de tasks anteriores.
- Produces:
  - `agent_bar::config::Paths { cache_dir, config_dir, claude_credentials, codex_auth, codex_sessions, amp_settings, amp_threads: PathBuf }`
  - `Paths::from_env() -> anyhow::Result<Paths>`
  - `agent_bar::config::now_ms() -> u64`
  - `agent_bar::config::status_for_percent(pct: Option<f64>) -> HealthStatus`
  - `enum HealthStatus { Ok, Low, Warn, Critical }`
  - consts: `CLAUDE_USAGE_URL`, `CLAUDE_USER_AGENT`, `CLAUDE_BETA_HEADER` (`&str`); `HTTP_TIMEOUT_SECS`, `PROVIDER_TIMEOUT_SECS` (`u64`); `DEFAULT_INTERVAL_SECS` (`u32`); `KNOWN_PROVIDER_IDS: [&str; 3]`; `fn default_ttl_secs(provider: &str) -> u32`.

- [ ] **Step 1: Escrever os testes falhando em `rust/src/config.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    #[serial_test::serial]
    fn paths_from_env_uses_xdg_when_set() {
        temp_env::with_vars(
            [
                ("HOME", Some("/home/u")),
                ("XDG_CACHE_HOME", Some("/x/cache")),
                ("XDG_CONFIG_HOME", Some("/x/config")),
            ],
            || {
                let p = Paths::from_env().unwrap();
                assert_eq!(p.cache_dir, PathBuf::from("/x/cache/agent-bar"));
                assert_eq!(p.config_dir, PathBuf::from("/x/config/agent-bar"));
                assert_eq!(p.claude_credentials, PathBuf::from("/home/u/.claude/.credentials.json"));
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn paths_from_env_falls_back_to_home_when_xdg_unset() {
        temp_env::with_vars(
            [
                ("HOME", Some("/home/u")),
                ("XDG_CACHE_HOME", None::<&str>),
                ("XDG_CONFIG_HOME", None::<&str>),
            ],
            || {
                let p = Paths::from_env().unwrap();
                assert_eq!(p.cache_dir, PathBuf::from("/home/u/.cache/agent-bar"));
                assert_eq!(p.config_dir, PathBuf::from("/home/u/.config/agent-bar"));
            },
        );
    }

    #[test]
    fn status_thresholds() {
        assert_eq!(status_for_percent(None), HealthStatus::Ok);
        assert_eq!(status_for_percent(Some(75.0)), HealthStatus::Ok);
        assert_eq!(status_for_percent(Some(59.9)), HealthStatus::Low);
        assert_eq!(status_for_percent(Some(29.9)), HealthStatus::Warn);
        assert_eq!(status_for_percent(Some(9.9)), HealthStatus::Critical);
        assert_eq!(status_for_percent(Some(0.0)), HealthStatus::Critical);
    }

    #[test]
    fn ttl_defaults_per_provider() {
        assert_eq!(default_ttl_secs("claude"), 300);
        assert_eq!(default_ttl_secs("codex"), 90);
        assert_eq!(default_ttl_secs("amp"), 90);
        assert_eq!(default_ttl_secs("unknown"), 300);
    }
}
```

- [ ] **Step 2: Rodar (deve falhar — `config` não existe)**

Run: `cargo test --manifest-path rust/Cargo.toml config 2>&1 | head`
Expected: FAIL de compilação (`Paths`/`status_for_percent` not found).

- [ ] **Step 3: Implementar `rust/src/config.rs`** (acima dos testes)

```rust
//! Configuração estática + resolução de paths (injetada, sem estado global).

use std::env;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// API Claude (contrato §3.10 do design — UA hardcoded, não a versão do agent-bar).
pub const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
pub const CLAUDE_USER_AGENT: &str = "claude-code/2.1.179";
pub const CLAUDE_BETA_HEADER: &str = "oauth-2025-04-20";

pub const HTTP_TIMEOUT_SECS: u64 = 5;
pub const PROVIDER_TIMEOUT_SECS: u64 = 10;

/// Poll interval default do módulo Waybar (era 120s hardcoded; ver design §1).
pub const DEFAULT_INTERVAL_SECS: u32 = 60;

pub const KNOWN_PROVIDER_IDS: [&str; 3] = ["claude", "codex", "amp"];

/// TTL default de cache por provider (segundos). Claude conservador (rate-limit);
/// Codex/Amp são locais e podem ser mais frescos.
pub fn default_ttl_secs(provider: &str) -> u32 {
    match provider {
        "codex" | "amp" => 90,
        _ => 300,
    }
}

// Thresholds de saúde (% restante).
pub const THRESHOLD_GREEN: f64 = 60.0;
pub const THRESHOLD_YELLOW: f64 = 30.0;
pub const THRESHOLD_ORANGE: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Ok,
    Low,
    Warn,
    Critical,
}

/// Bucket de saúde a partir do % restante cru. `None` → Ok (desconhecido).
pub fn status_for_percent(pct: Option<f64>) -> HealthStatus {
    match pct {
        None => HealthStatus::Ok,
        Some(p) if p < THRESHOLD_ORANGE => HealthStatus::Critical,
        Some(p) if p < THRESHOLD_YELLOW => HealthStatus::Warn,
        Some(p) if p < THRESHOLD_GREEN => HealthStatus::Low,
        Some(_) => HealthStatus::Ok,
    }
}

/// Epoch em milissegundos. Os módulos puros recebem `now_ms` por parâmetro
/// (testáveis); só os call sites de produção chamam esta função.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Paths resolvidos de XDG/HOME. Injetado pela call-chain (sem singleton global).
#[derive(Debug, Clone)]
pub struct Paths {
    pub cache_dir: PathBuf,
    pub config_dir: PathBuf,
    pub claude_credentials: PathBuf,
    pub codex_auth: PathBuf,
    pub codex_sessions: PathBuf,
    pub amp_settings: PathBuf,
    pub amp_threads: PathBuf,
}

impl Paths {
    pub fn from_env() -> anyhow::Result<Self> {
        let home = env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("HOME não está definido"))?;

        let xdg_dir = |var: &str, fallback: &str| -> PathBuf {
            env::var_os(var)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(fallback))
        };

        let xdg_cache = xdg_dir("XDG_CACHE_HOME", ".cache");
        let xdg_config = xdg_dir("XDG_CONFIG_HOME", ".config");

        Ok(Self {
            cache_dir: xdg_cache.join("agent-bar"),
            config_dir: xdg_config.join("agent-bar"),
            claude_credentials: home.join(".claude").join(".credentials.json"),
            codex_auth: home.join(".codex").join("auth.json"),
            codex_sessions: home.join(".codex").join("sessions"),
            amp_settings: xdg_config.join("amp").join("settings.json"),
            amp_threads: home.join(".local").join("share").join("amp").join("threads"),
        })
    }

    /// Caminho do settings.json sob o config dir.
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
}
```

- [ ] **Step 4: Adicionar o módulo em `rust/src/lib.rs`**

```rust
pub mod app_identity;
pub mod config;
pub mod logger;
```

- [ ] **Step 5: Rodar (deve passar)**

Run: `cargo test --manifest-path rust/Cargo.toml config`
Expected: PASS (4 testes).

- [ ] **Step 6: Lint + Commit**

```bash
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/config.rs rust/src/lib.rs
git commit -m "feat(rust): config Paths (DI) + thresholds + TTL"
```

---

### Task 3: `cache` — cache de arquivo atômico

**Files:**
- Create: `rust/src/cache.rs`
- Modify: `rust/src/lib.rs` (adicionar `pub mod cache;`)
- Test: dentro de `cache.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: nada.
- Produces:
  - `agent_bar::cache::cache_path(cache_dir: &Path, key: &str) -> Result<PathBuf, CacheError>`
  - `agent_bar::cache::get<T: DeserializeOwned>(cache_dir: &Path, key: &str, now_ms: u64) -> Option<T>`
  - `agent_bar::cache::set<T: Serialize>(cache_dir: &Path, key: &str, data: &T, ttl_ms: u64, now_ms: u64) -> anyhow::Result<()>`
  - `agent_bar::cache::invalidate(cache_dir: &Path, key: &str)`
  - `enum CacheError { InvalidKey(String) }` (Display = `Invalid cache key: "<key>"`)

- [ ] **Step 1: Escrever os testes falhando em `rust/src/cache.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_invalid_keys() {
        let dir = tempdir().unwrap();
        for bad in ["", "a/b", "../x", "a.b", "a b", "claude!"] {
            assert!(cache_path(dir.path(), bad).is_err(), "key {bad:?} deveria falhar");
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
```

- [ ] **Step 2: Rodar (deve falhar)**

Run: `cargo test --manifest-path rust/Cargo.toml cache 2>&1 | head`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/cache.rs`**

```rust
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
```

- [ ] **Step 4: Adicionar o módulo em `rust/src/lib.rs`**

```rust
pub mod app_identity;
pub mod cache;
pub mod config;
pub mod logger;
```

- [ ] **Step 5: Rodar (deve passar)**

Run: `cargo test --manifest-path rust/Cargo.toml cache`
Expected: PASS (5 testes).

- [ ] **Step 6: Lint + Commit**

```bash
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/cache.rs rust/src/lib.rs
git commit -m "feat(rust): cache de arquivo atômico com TTL"
```

---

### Task 4: `settings` — schema + normalize + load/save

**Files:**
- Create: `rust/src/settings.rs`
- Modify: `rust/src/lib.rs` (adicionar `pub mod settings;`)
- Test: dentro de `settings.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `agent_bar::config::Paths`.
- Produces:
  - `agent_bar::settings::Settings { version: u32, waybar: Waybar, tooltip: Tooltip, models: BTreeMap<String, Vec<String>>, window_policy: BTreeMap<String, WindowPolicy>, notify: Notify, cache: CacheSettings }`
  - `Waybar { providers: Vec<String>, show_percentage: bool, separators: SeparatorStyle, provider_order: Vec<String>, display_mode: DisplayMode, signal: Option<u8>, interval: u32 }`
  - enums `SeparatorStyle`, `DisplayMode`, `WindowPolicy`; `Notify { enabled: bool }`; `CacheSettings { ttl: BTreeMap<String, u32> }`
  - `settings::load(paths: &Paths) -> Settings`
  - `settings::save(paths: &Paths, settings: &Settings) -> anyhow::Result<()>`
  - `settings::normalize_provider_selection(providers: &[String], provider_order: &[String]) -> (Vec<String>, Vec<String>)`

**Notas de comportamento (do TS `src/settings.ts`):**
- Enums inválidos (separators/displayMode/windowPolicy) **coagem pro default** (não erro) → deserialize como String no raw, converte no normalize.
- `signal` válido = inteiro 1..=30; fora → `None` (feature off).
- `notify.enabled` = `true` salvo se explicitamente `false`.
- `provider_order` deriva via `normalize_provider_selection` (filtra a known ids, dedup, reconcilia ordem).
- `window_policy` default `{codex: Both}`; `cache.ttl` default `{claude:300, codex:90, amp:90}`; `interval` default `60` (campos novos, não existem no TS).
- **Auto-repair:** após load+normalize, comparar `to_value(normalized)` vs o `Value` do arquivo bruto; se diferem, regravar (`save`). Idempotente.

- [ ] **Step 1: Escrever os testes falhando em `rust/src/settings.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn paths_in(dir: &std::path::Path) -> Paths {
        Paths {
            cache_dir: dir.join("cache"),
            config_dir: dir.join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
        }
    }

    #[test]
    fn defaults_when_no_file() {
        let dir = tempdir().unwrap();
        let s = load(&paths_in(dir.path()));
        assert_eq!(s.version, 2);
        assert_eq!(s.waybar.providers, vec!["claude", "codex", "amp"]);
        assert_eq!(s.waybar.separators, SeparatorStyle::Gap);
        assert_eq!(s.waybar.display_mode, DisplayMode::Remaining);
        assert_eq!(s.waybar.interval, 60);
        assert!(s.waybar.signal.is_none());
        assert!(s.notify.enabled);
        assert_eq!(s.cache.ttl.get("claude"), Some(&300));
        assert_eq!(s.cache.ttl.get("codex"), Some(&90));
        assert_eq!(s.window_policy.get("codex"), Some(&WindowPolicy::Both));
    }

    #[test]
    fn coerces_invalid_enums_to_default() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(
            p.settings_file(),
            r#"{"waybar":{"separators":"bogus","displayMode":"weird","signal":99},
                "windowPolicy":{"codex":"nope"}}"#,
        )
        .unwrap();
        let s = load(&p);
        assert_eq!(s.waybar.separators, SeparatorStyle::Gap);
        assert_eq!(s.waybar.display_mode, DisplayMode::Remaining);
        assert!(s.waybar.signal.is_none()); // 99 fora de 1..=30
        assert_eq!(s.window_policy.get("codex"), Some(&WindowPolicy::Both));
    }

    #[test]
    fn keeps_valid_signal_and_separator() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(
            p.settings_file(),
            r#"{"waybar":{"separators":"glass","signal":8}}"#,
        )
        .unwrap();
        let s = load(&p);
        assert_eq!(s.waybar.separators, SeparatorStyle::Glass);
        assert_eq!(s.waybar.signal, Some(8));
    }

    #[test]
    fn provider_selection_filters_dedups_and_orders() {
        let (providers, order) = normalize_provider_selection(
            &["amp".into(), "claude".into(), "amp".into(), "ghost".into()],
            &["claude".into()],
        );
        assert_eq!(providers, vec!["amp", "claude"]); // dedup, known-only, ordem de `providers`
        assert_eq!(order, vec!["claude", "amp"]); // order válido + faltantes ao fim
    }

    #[test]
    fn notify_disabled_only_when_explicit_false() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(p.settings_file(), r#"{"notify":{"enabled":false}}"#).unwrap();
        assert!(!load(&p).notify.enabled);
    }

    #[test]
    fn save_then_load_is_stable() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let s1 = load(&p);
        save(&p, &s1).unwrap();
        let s2 = load(&p);
        assert_eq!(s1, s2);
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)**

Run: `cargo test --manifest-path rust/Cargo.toml settings 2>&1 | head`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/settings.rs`**

```rust
//! Settings: schema tipado + normalização leniente (raw→typed) + load/save atômico.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::Paths;

pub const CURRENT_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SeparatorStyle {
    Pill,
    Gap,
    Bare,
    Glass,
    Shadow,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    Remaining,
    Used,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowPolicy {
    Both,
    FiveHour,
    SevenDay,
}

fn separator_from_str(s: &str) -> Option<SeparatorStyle> {
    Some(match s {
        "pill" => SeparatorStyle::Pill,
        "gap" => SeparatorStyle::Gap,
        "bare" => SeparatorStyle::Bare,
        "glass" => SeparatorStyle::Glass,
        "shadow" => SeparatorStyle::Shadow,
        "none" => SeparatorStyle::None,
        _ => return None,
    })
}

fn display_mode_from_str(s: &str) -> Option<DisplayMode> {
    Some(match s {
        "remaining" => DisplayMode::Remaining,
        "used" => DisplayMode::Used,
        _ => return None,
    })
}

fn window_policy_from_str(s: &str) -> Option<WindowPolicy> {
    Some(match s {
        "both" => WindowPolicy::Both,
        "five_hour" => WindowPolicy::FiveHour,
        "seven_day" => WindowPolicy::SevenDay,
        _ => return None,
    })
}

// ---- Schema tipado (serialize) ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Tooltip {}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Waybar {
    pub providers: Vec<String>,
    pub show_percentage: bool,
    pub separators: SeparatorStyle,
    pub provider_order: Vec<String>,
    pub display_mode: DisplayMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u8>,
    pub interval: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notify {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheSettings {
    pub ttl: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub waybar: Waybar,
    pub tooltip: Tooltip,
    pub models: BTreeMap<String, Vec<String>>,
    pub window_policy: BTreeMap<String, WindowPolicy>,
    pub notify: Notify,
    pub cache: CacheSettings,
}

// ---- Schema bruto (deserialize leniente) ----

#[derive(Debug, Default, Deserialize)]
struct RawSettings {
    waybar: Option<RawWaybar>,
    models: Option<BTreeMap<String, Vec<String>>>,
    #[serde(rename = "windowPolicy")]
    window_policy: Option<BTreeMap<String, String>>,
    notify: Option<RawNotify>,
    cache: Option<RawCache>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawWaybar {
    providers: Option<Vec<String>>,
    show_percentage: Option<bool>,
    separators: Option<String>,
    provider_order: Option<Vec<String>>,
    display_mode: Option<String>,
    signal: Option<i64>,
    interval: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct RawNotify {
    enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCache {
    ttl: Option<BTreeMap<String, u32>>,
}

fn default_providers() -> Vec<String> {
    crate::config::KNOWN_PROVIDER_IDS.iter().map(|s| s.to_string()).collect()
}

fn default_ttl_map() -> BTreeMap<String, u32> {
    crate::config::KNOWN_PROVIDER_IDS
        .iter()
        .map(|p| (p.to_string(), crate::config::default_ttl_secs(p)))
        .collect()
}

/// Filtra a known ids + dedup; reconcilia `provider_order` (válidos + faltantes ao fim).
/// Espelha `normalizeProviderSelection` do TS.
pub fn normalize_provider_selection(
    providers: &[String],
    provider_order: &[String],
) -> (Vec<String>, Vec<String>) {
    let known = |p: &str| crate::config::KNOWN_PROVIDER_IDS.contains(&p);

    let mut deduped: Vec<String> = Vec::new();
    for p in providers {
        if known(p) && !deduped.contains(p) {
            deduped.push(p.clone());
        }
    }

    let mut order: Vec<String> = provider_order
        .iter()
        .filter(|p| deduped.contains(*p))
        .cloned()
        .collect();
    for p in &deduped {
        if !order.contains(p) {
            order.push(p.clone());
        }
    }

    (deduped, order)
}

fn normalize(raw: RawSettings) -> Settings {
    let rw = raw.waybar.unwrap_or_default();

    let providers = rw.providers.unwrap_or_else(default_providers);
    let provider_order = rw.provider_order.unwrap_or_else(default_providers);
    let (providers, provider_order) = normalize_provider_selection(&providers, &provider_order);

    let separators = rw
        .separators
        .as_deref()
        .and_then(separator_from_str)
        .unwrap_or(SeparatorStyle::Gap);

    let display_mode = rw
        .display_mode
        .as_deref()
        .and_then(display_mode_from_str)
        .unwrap_or(DisplayMode::Remaining);

    let signal = rw
        .signal
        .filter(|n| (1..=30).contains(n))
        .map(|n| n as u8);

    // window_policy: default {codex: Both}, mesclado com o raw (inválido → Both).
    let mut window_policy: BTreeMap<String, WindowPolicy> = BTreeMap::new();
    window_policy.insert("codex".to_string(), WindowPolicy::Both);
    if let Some(raw_wp) = raw.window_policy {
        for (k, v) in raw_wp {
            window_policy.insert(k, window_policy_from_str(&v).unwrap_or(WindowPolicy::Both));
        }
    }

    // cache.ttl: defaults mesclados com overrides do raw.
    let mut ttl = default_ttl_map();
    if let Some(rc) = raw.cache {
        if let Some(raw_ttl) = rc.ttl {
            ttl.extend(raw_ttl);
        }
    }

    Settings {
        version: CURRENT_VERSION,
        waybar: Waybar {
            providers,
            show_percentage: rw.show_percentage.unwrap_or(true),
            separators,
            provider_order,
            display_mode,
            signal,
            interval: rw.interval.unwrap_or(crate::config::DEFAULT_INTERVAL_SECS),
        },
        tooltip: Tooltip {},
        models: raw.models.unwrap_or_default(),
        window_policy,
        notify: Notify {
            enabled: raw.notify.and_then(|n| n.enabled) != Some(false),
        },
        cache: CacheSettings { ttl },
    }
}

/// Carrega + normaliza. Defaults em ausência/erro. Auto-repair se o conteúdo
/// normalizado difere do arquivo bruto.
pub fn load(paths: &Paths) -> Settings {
    let file = paths.settings_file();
    let bytes = match std::fs::read(&file) {
        Ok(b) => b,
        Err(_) => return normalize(RawSettings::default()),
    };

    let raw_value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[agent-bar] Settings parse error (using defaults): {e}");
            return normalize(RawSettings::default());
        }
    };

    let raw: RawSettings = serde_json::from_value(raw_value.clone()).unwrap_or_default();
    let normalized = normalize(raw);

    let norm_value = serde_json::to_value(&normalized).unwrap_or(serde_json::Value::Null);
    if norm_value != raw_value {
        let _ = save(paths, &normalized);
    }

    normalized
}

/// Grava atomicamente (tempfile + rename), pretty 2-espaços, sempre normalizado.
pub fn save(paths: &Paths, settings: &Settings) -> anyhow::Result<()> {
    use std::io::Write;

    std::fs::create_dir_all(&paths.config_dir)?;
    let json = serde_json::to_string_pretty(settings)?;

    let mut tmp = tempfile::NamedTempFile::new_in(&paths.config_dir)?;
    tmp.write_all(json.as_bytes())?;
    tmp.persist(paths.settings_file())?;
    Ok(())
}
```

- [ ] **Step 4: Adicionar o módulo em `rust/src/lib.rs`**

```rust
pub mod app_identity;
pub mod cache;
pub mod config;
pub mod logger;
pub mod settings;
```

- [ ] **Step 5: Rodar (deve passar)**

Run: `cargo test --manifest-path rust/Cargo.toml settings`
Expected: PASS (6 testes).

- [ ] **Step 6: Rodar a suíte inteira + lint**

Run: `cargo test --manifest-path rust/Cargo.toml && cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`
Expected: tudo PASS, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add rust/src/settings.rs rust/src/lib.rs
git commit -m "feat(rust): settings schema + normalize + load/save"
```

---

## Próximos planos (roadmap — autorados just-in-time)

Cada um produz software testável e é escrito quando a camada anterior fecha (as interfaces firmam com o código real):

- **02 — formatters puros (Layer 4a):** golden snapshots do TS (passo 0) → `theme` (ColorToken/One Dark) → `segments` → `render_pango` (boundary de escape) → `render_ansi` → `builders` → `json`. insta byte-exact.
- **03 — formatters c/ settings (Layer 4b):** `waybar.rs` (+ cache 5s aqui) + `view_model`.
- **04 — providers (Layer 5):** `http` (Client compartilhado), `types`/`error`/trait/registry/`base`, Claude, Codex (app-server + fallback walkdir), Amp; `notify`. tokio entra aqui.
- **05 — CLI (Layer 6):** clap, dispatch, hidden-module, `--watch`, `action_right`, `reload_waybar`.
- **06 — install (Layer 7a-7d):** `waybar_integration` (scanner), `waybar_contract` (+ asset resolution), setup/uninstall/remove, update + doctor.
- **07 — distribuição (Layer 8):** musl + cargo-dist + tarball + PKGBUILD; remover npm; CI; reescrever CLAUDE.md.
