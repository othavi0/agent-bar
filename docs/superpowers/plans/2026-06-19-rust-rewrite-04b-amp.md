# Plano 04b — Amp provider (spawn + drain stderr + parse regex)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps usam checkbox (`- [ ]`).

**Goal:** Segundo provider real (Amp), estendendo o template `QuotaSource` do 04a: descoberta do binário, spawn de `amp usage` (drenando stderr, com timeout/kill), e parse por regex do stdout para `ProviderQuota` byte-exact com o TS.

**Architecture:** Amp roda um subprocesso (não HTTP). Estende `QuotaSource` (04a): `Raw = String` (stdout cru, cacheável). `fetch_raw` faz spawn via `tokio::process`. `build_quota` chama `parse_usage` (puro, regex). Duas extensões de fundação que o 1º `QuotaSource` concreto exige: `QuotaSource::build_quota` ganha `&Ctx` (o `fullAt` recalcula com o `now` atual a cada chamada, inclusive em cache hit) e `Ctx` ganha `home: PathBuf` (candidatos do binário ficam sob `$HOME`).

**Tech Stack:** tokio (`process`, `time`), regex, serde. Testes: strings de stdout mockadas (parse) + script-fake executável em tempdir (spawn) + `find_amp_bin_with` com seams injetáveis.

## Global Constraints

Toda task herda (copiadas verbatim do spec/CLAUDE.md/resume):

- **Contrato byte-exact com o TS é SAGRADO. A autoridade é a saída do TS** (`src/providers/amp.ts`, `src/amp-cli.ts` + os testes `tests/providers/amp.test.ts`, `tests/amp-cli.test.ts`). Rejeitar qualquer "fix" de review que divergiria do TS.
- **Sem `unwrap()`/`expect()` em produção** (deny lint em lib.rs/main.rs). Em `#[cfg(test)]` permitido. Nunca `!`. `unwrap_or`/`unwrap_or_else`/`.ok()`/`let _ =` são permitidos.
- **stdout limpo.** Logs só `log::{warn,error,debug}`. Nunca `eprintln!`/`println!`.
- **Strings de erro são CONTRATO verbatim** (já em `providers/error.rs`): `AmpError::{NotInstalled, NotLoggedIn, ParseFailed, Generic}`.
- `ProviderQuota` é serialize-only; o cache faz round-trip do **raw** (`String` p/ Amp).
- **`models` é `IndexMap`** (ordem de inserção = `Object.entries` do TS: "Free Tier" antes de "Credits"). **`meta` é `BTreeMap`** (acesso por chave via `meta_get`).
- **Formatação de número (CRÍTICO, do TS + amp.test.ts):** `freeRemaining`/`freeTotal`/`creditsBalance` usam `parseFloat`→número→string (strip de zeros: `"$3.50"`→`$3.5`, `"$5.00"`→`$5`, `"$10.00"`→`$10`). `replenishRate`/`bonus` usam a **string CRUA capturada** (`+$0.25/hr`, `+20% (5d)`). Em Rust: `format!("${}", f64)` casa com `Number.toString()` p/ esses valores (Display do f64 faz shortest-round-trip: `3.5f64`→`"3.5"`, `5.0f64`→`"5"`).
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. **RTK:** output é `cargo test: N passed`; NÃO existe `test result:` (ler bruto com `... 2>&1 | tail -8`). `cargo test` aceita só UM filtro posicional.
- **`cargo fmt --manifest-path rust/Cargo.toml` ANTES de `git add`.**
- **Read antes de Edit.** Edit falhou com `string not found`? re-Read antes — nunca de memória.
- Commits: Conventional Commits PT ≤50 chars. Identificadores/comentários em inglês.
- **NÃO tocar main.rs** (Plano 5). **NÃO tocar docs do projeto.**

---

## File Structure

- `rust/src/providers/mod.rs` (modificar T1) — `Ctx` ganha `home: PathBuf`; `test_support::ctx_for` seta `home`.
- `rust/src/providers/base.rs` (modificar T1) — `QuotaSource::build_quota` ganha `ctx: &Ctx<'_>`; `base_get_quota` passa `ctx`; fake de teste atualizado.
- `rust/src/providers/amp_cli.rs` (criar T2) — `AMP_INSTALL_COMMAND`, `amp_candidate_paths`, `find_amp_bin_with`, `find_amp_bin`, `which_in_path`.
- `rust/src/providers/amp.rs` (criar T3+T4) — `parse_usage` (T3) + `run_amp_usage`/`AmpProvider` (T4).
- `rust/src/providers/mod.rs` (modificar T2/T4) — `pub mod amp_cli;`/`pub mod amp;`; `registry()` ganha Amp (T4).

---

### Task 1: Extensões de fundação (`Ctx.home` + `build_quota(ctx)`)

**Files:**
- Modify: `rust/src/providers/mod.rs` (Ctx + test_support)
- Modify: `rust/src/providers/base.rs` (QuotaSource trait + base_get_quota + fake test)

**Interfaces:**
- Produces: `Ctx.home: PathBuf`; `QuotaSource::build_quota(&self, raw, base, ctx: &Ctx<'_>) -> ProviderQuota`.

- [ ] **Step 1: `Ctx` ganha `home`**

Em `rust/src/providers/mod.rs`, no struct `Ctx`, adicionar campo (após `version`):
```rust
    /// `$HOME` resolvido (candidatos de binário do Amp ficam sob ele).
    pub home: std::path::PathBuf,
```

Em `test_support::ctx_for`, no literal `Ctx { ... }`, adicionar:
```rust
            home: dir.to_path_buf(),
```
(o `dir` é o tempdir do teste — candidatos resolvem sob ele).

- [ ] **Step 2: `QuotaSource::build_quota` ganha `ctx`**

Em `rust/src/providers/base.rs`:
- No trait `QuotaSource`, trocar a assinatura:
```rust
    fn build_quota(&self, raw: Self::Raw, base: ProviderQuota, ctx: &Ctx<'_>) -> ProviderQuota;
```
- Em `base_get_quota`, na chamada `source.build_quota(raw, base)` → `source.build_quota(raw, base, ctx)`.
- No `#[cfg(test)] mod tests`, o `impl QuotaSource for Fake`: atualizar `fn build_quota(&self, raw: String, base: ProviderQuota, _ctx: &Ctx<'_>) -> ProviderQuota` (adicionar o param `_ctx`).

- [ ] **Step 3: Verificar (nada quebrou)**

Run: `cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -4` → **208 passed** (inalterado).
Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -3` → sem issues.

- [ ] **Step 4: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/providers/mod.rs rust/src/providers/base.rs
git commit -m "refactor(rust): Ctx.home + build_quota recebe ctx"
```

---

### Task 2: Locator do binário Amp (`amp_cli.rs`)

**Files:**
- Create: `rust/src/providers/amp_cli.rs`
- Modify: `rust/src/providers/mod.rs` (`pub mod amp_cli;`)

**Interfaces:**
- Produces: `amp_candidate_paths(home: &str) -> Vec<PathBuf>`; `find_amp_bin_with(home, which, exists) -> Option<PathBuf>`; `find_amp_bin(home: &str) -> Option<PathBuf>`; `AMP_INSTALL_COMMAND: &str`.

- [ ] **Step 1: Criar `rust/src/providers/amp_cli.rs`**

```rust
//! Descoberta do binário `amp` (locator). Port de `src/amp-cli.ts` (só a metade
//! locator; o `ensure_amp_cli` interativo é Plano 6). Ordem: PATH (`which`),
//! depois caminhos conhecidos sob `$HOME`.

use std::path::{Path, PathBuf};

/// Comando oficial de instalação (usado pelo Plano 6; contrato de display).
pub const AMP_INSTALL_COMMAND: &str = "curl -fsSL https://ampcode.com/install.sh | bash";

/// Caminhos candidatos sob `$HOME`, na ordem de preferência. Vazio se `home` vazio.
pub fn amp_candidate_paths(home: &str) -> Vec<PathBuf> {
    if home.is_empty() {
        return Vec::new();
    }
    let h = Path::new(home);
    vec![
        h.join(".local").join("bin").join("amp"),
        h.join(".amp").join("bin").join("amp"),
        h.join(".cache").join(".bun").join("bin").join("amp"),
        h.join(".bun").join("bin").join("amp"),
    ]
}

/// Locator com seams injetáveis (`which`/`exists`) para teste. PATH primeiro;
/// depois o 1º candidato que existe; senão `None`.
pub fn find_amp_bin_with(
    home: &str,
    which: impl Fn(&str) -> Option<PathBuf>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(p) = which("amp") {
        return Some(p);
    }
    amp_candidate_paths(home).into_iter().find(|p| exists(p))
}

/// Procura um executável no `$PATH` (substitui `Bun.which`).
pub fn which_in_path(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(cmd))
        .find(|p| p.is_file())
}

/// Locator de produção: `which_in_path` + `Path::is_file`.
pub fn find_amp_bin(home: &str) -> Option<PathBuf> {
    find_amp_bin_with(home, |c| which_in_path(c), |p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_paths_under_home() {
        let paths = amp_candidate_paths("/tmp/agent-bar-home");
        let got: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
        assert_eq!(
            got,
            vec![
                "/tmp/agent-bar-home/.local/bin/amp",
                "/tmp/agent-bar-home/.amp/bin/amp",
                "/tmp/agent-bar-home/.cache/.bun/bin/amp",
                "/tmp/agent-bar-home/.bun/bin/amp",
            ]
        );
    }

    #[test]
    fn empty_home_yields_no_candidates() {
        assert!(amp_candidate_paths("").is_empty());
    }

    #[test]
    fn prefers_path_when_available() {
        let found = find_amp_bin_with(
            "/tmp/agent-bar-home",
            |_| Some(PathBuf::from("/usr/local/bin/amp")),
            |_| false,
        );
        assert_eq!(found, Some(PathBuf::from("/usr/local/bin/amp")));
    }

    #[test]
    fn falls_back_to_known_locations() {
        let found = find_amp_bin_with(
            "/tmp/agent-bar-home",
            |_| None,
            |p| p == Path::new("/tmp/agent-bar-home/.local/bin/amp"),
        );
        assert_eq!(found, Some(PathBuf::from("/tmp/agent-bar-home/.local/bin/amp")));
    }

    #[test]
    fn none_when_unavailable() {
        let found = find_amp_bin_with("/tmp/agent-bar-home", |_| None, |_| false);
        assert_eq!(found, None);
    }

    #[test]
    fn install_command_is_official() {
        assert_eq!(AMP_INSTALL_COMMAND, "curl -fsSL https://ampcode.com/install.sh | bash");
    }
}
```

- [ ] **Step 2: Registrar módulo**

Em `rust/src/providers/mod.rs`, adicionar `pub mod amp_cli;` (após `pub mod amp_cli;`... na verdade após `pub mod base;`, mantendo ordem: amp_cli antes de base alfabeticamente — colocar `pub mod amp_cli;` antes de `pub mod base;`).

- [ ] **Step 3: Verificar**

Run: `cargo test --manifest-path rust/Cargo.toml amp_cli 2>&1 | tail -5` → 6 testes passam.
Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -3` → sem issues.

- [ ] **Step 4: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/providers/amp_cli.rs rust/src/providers/mod.rs
git commit -m "feat(rust): locator do binário Amp"
```

---

### Task 3: `parse_usage` (parse regex puro)

**Files:**
- Create: `rust/src/providers/amp.rs` (só `parse_usage` + helpers nesta task)
- Modify: `rust/src/providers/mod.rs` (`pub mod amp;`)

**Interfaces:**
- Consumes: `iso_from_ms` (de `providers::mod`), `types::{ProviderQuota, QuotaWindow, ProviderExtra, AmpQuotaExtra}`, `IndexMap`, `BTreeMap`, `regex::Regex`.
- Produces: `parse_usage(stdout: &str, base: ProviderQuota, now_ms: u64) -> ProviderQuota`.

**Contrato (de `amp.ts` `parseUsage` + `amp.test.ts`):**
- `Signed in as (\S+)` → `account`.
- `Amp Free:\s*\$([0-9.]+)/\$([0-9.]+)\s*remaining` → free (remaining/total floats); `pct = round(remaining/total*100)` (0 se total≤0); `primary`/`models["Free Tier"]` = `{remaining: pct, resetsAt: fullAt}`.
- `replenishes \+\$([0-9.]+)/hour` → replenishRate `+${raw}/hr` (string CRUA).
- `\+(\d+)%\s*bonus\s*for\s*(\d+)\s*more\s*days` → bonus `+${raw1}% (${raw2}d)`.
- **fullAt:** só se replenish presente E `remaining < total`: `rate = parseFloat(replenish_raw)`; `eff = bonus ? rate*(1+bonus1/100) : rate`; `hours = (total-remaining)/eff`; se `eff > 0 && hours.is_finite()` → `iso_from_ms((now_ms as f64 + hours*3_600_000.0) as u64)`; senão `None`.
- `Individual credits:\s*\$([0-9.]+)\s*remaining` → `models["Credits"] = {remaining: balance>0 ? 100 : 0, resetsAt: None}`; `meta.creditsBalance = $${balance}`.
- `meta`: `freeRemaining=$${remaining}`, `freeTotal=$${total}` (sempre que free), `replenishRate` (se presente), `bonus` (se presente), `creditsBalance` (se credits). `extra.meta` só se `meta` não-vazio.
- Retorno: `{...base, provider:"amp", available:true, account, primary, models: Some(models) (sempre, mesmo vazio), extra (se meta não-vazio)}`.

- [ ] **Step 1: Criar `rust/src/providers/amp.rs` com `parse_usage`**

```rust
//! Amp provider. Estende `QuotaSource` (04a): spawn de `amp usage` + parse regex.
//! Port fiel de `src/providers/amp.ts`. NÃO há `amp usage --json` → regex no texto.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;

use super::iso_from_ms;
use super::types::{AmpQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow};

fn re(cell: &'static OnceLock<Option<Regex>>, pattern: &str) -> Option<&'static Regex> {
    cell.get_or_init(|| Regex::new(pattern).ok()).as_ref()
}

macro_rules! lazy_re {
    ($name:ident, $pat:expr) => {{
        static CELL: OnceLock<Option<Regex>> = OnceLock::new();
        re(&CELL, $pat)
    }};
}

/// `$` + número estilo `Number.toString()` (Display do f64 = shortest round-trip).
fn dollars(n: f64) -> String {
    format!("${n}")
}

/// Parse do stdout de `amp usage` para `ProviderQuota`. `now_ms` é o relógio
/// atual (o `fullAt` recalcula a cada chamada, inclusive em cache hit).
pub fn parse_usage(stdout: &str, base: ProviderQuota, now_ms: u64) -> ProviderQuota {
    let cap1 = |re: Option<&Regex>, i: usize| -> Option<String> {
        re.and_then(|r| r.captures(stdout))
            .and_then(|c| c.get(i).map(|m| m.as_str().to_string()))
    };

    let account = cap1(lazy_re!(RE_SIGNED, r"Signed in as (\S+)"), 1);

    let free_re = lazy_re!(RE_FREE, r"Amp Free:\s*\$([0-9.]+)/\$([0-9.]+)\s*remaining");
    let free_caps = free_re.and_then(|r| r.captures(stdout));

    let replenish = cap1(lazy_re!(RE_REPLENISH, r"replenishes \+\$([0-9.]+)/hour"), 1);
    let bonus_re = lazy_re!(RE_BONUS, r"\+(\d+)%\s*bonus\s*for\s*(\d+)\s*more\s*days");
    let bonus_caps = bonus_re.and_then(|r| r.captures(stdout));
    let bonus_pct = bonus_caps.as_ref().and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
    let bonus_days = bonus_caps.as_ref().and_then(|c| c.get(2)).map(|m| m.as_str().to_string());

    let credits = cap1(
        lazy_re!(RE_CREDITS, r"Individual credits:\s*\$([0-9.]+)\s*remaining"),
        1,
    );

    let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
    let mut meta: BTreeMap<String, String> = BTreeMap::new();
    let mut primary: Option<QuotaWindow> = None;

    if let Some(fc) = free_caps {
        let remaining: f64 = fc.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0.0);
        let total: f64 = fc.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0.0);
        let pct = if total > 0.0 {
            (remaining / total * 100.0).round()
        } else {
            0.0
        };

        // fullAt: só com replenish e não-cheio.
        let full_at: Option<String> = if let Some(rep) = replenish.as_deref() {
            if remaining < total {
                let rate: f64 = rep.parse().unwrap_or(0.0);
                let eff = match bonus_pct.as_deref() {
                    Some(b) => rate * (1.0 + b.parse::<f64>().unwrap_or(0.0) / 100.0),
                    None => rate,
                };
                let hours = (total - remaining) / eff;
                if eff > 0.0 && hours.is_finite() {
                    Some(iso_from_ms((now_ms as f64 + hours * 3_600_000.0) as u64))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let window = QuotaWindow {
            remaining: pct,
            resets_at: full_at.clone(),
            window_minutes: None,
            used: None,
        };
        primary = Some(window.clone());
        models.insert("Free Tier".to_string(), window);
        meta.insert("freeRemaining".to_string(), dollars(remaining));
        meta.insert("freeTotal".to_string(), dollars(total));
        if let Some(rep) = replenish.as_deref() {
            meta.insert("replenishRate".to_string(), format!("+${rep}/hr"));
        }
        if let (Some(p), Some(d)) = (bonus_pct.as_deref(), bonus_days.as_deref()) {
            meta.insert("bonus".to_string(), format!("+{p}% ({d}d)"));
        }
    }

    if let Some(bal_str) = credits.as_deref() {
        let balance: f64 = bal_str.parse().unwrap_or(0.0);
        models.insert(
            "Credits".to_string(),
            QuotaWindow {
                remaining: if balance > 0.0 { 100.0 } else { 0.0 },
                resets_at: None,
                window_minutes: None,
                used: None,
            },
        );
        meta.insert("creditsBalance".to_string(), dollars(balance));
    }

    let extra = if meta.is_empty() {
        None
    } else {
        Some(ProviderExtra::Amp(AmpQuotaExtra { meta: Some(meta) }))
    };

    ProviderQuota {
        provider: "amp".to_string(),
        available: true,
        account,
        primary,
        models: Some(models),
        extra,
        ..base
    }
}
```

**Nota p/ o implementer:** se o clippy reclamar de `now_ms as f64` ou `... as u64` (cast precision/truncation), manter — é a conversão intencional que espelha `new Date(Date.now() + hours*3.6e6)` do JS (truncamento p/ ms inteiro). Se reclamar do `lazy_re!` macro (ex.: `unused`), ajustar mas manter o cache de regex. O `dollars()` depende do Display do f64 dar shortest round-trip (`5.0`→`"5"`); isso é garantido pelo Rust.

- [ ] **Step 2: Registrar módulo**

Em `rust/src/providers/mod.rs`, adicionar `pub mod amp;` (após `pub mod amp_cli;`).

- [ ] **Step 3: Testes de `parse_usage`** (append `#[cfg(test)] mod tests` em amp.rs)

Porte os casos de `amp.test.ts`. Use um `base()` helper e um `now` fixo. Asserções-chave (verbatim do TS):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_700_000_000_000; // relógio fixo

    fn base() -> ProviderQuota {
        ProviderQuota {
            provider: "amp".into(),
            display_name: "Amp".into(),
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

    fn meta_of(q: &ProviderQuota) -> &BTreeMap<String, String> {
        match q.extra.as_ref() {
            Some(ProviderExtra::Amp(a)) => a.meta.as_ref().expect("meta"),
            _ => panic!("expected Amp extra"),
        }
    }

    const FULL: &str = "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining\nreplenishes +$0.25/hour\n+20% bonus for 5 more days\nIndividual credits: $10.00 remaining";

    #[test]
    fn parses_full_output() {
        let q = parse_usage(FULL, base(), NOW);
        assert!(q.available);
        assert_eq!(q.account.as_deref(), Some("user@email.com"));
        assert_eq!(q.primary.as_ref().unwrap().remaining, 70.0); // 3.5/5 = 70%
        let models = q.models.as_ref().unwrap();
        assert_eq!(models["Free Tier"].remaining, 70.0);
        assert_eq!(models["Credits"].remaining, 100.0);
        // ordem de inserção: Free Tier antes de Credits (IndexMap)
        let keys: Vec<&str> = models.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["Free Tier", "Credits"]);
        let m = meta_of(&q);
        assert_eq!(m.get("freeRemaining").map(String::as_str), Some("$3.5"));
        assert_eq!(m.get("freeTotal").map(String::as_str), Some("$5"));
        assert_eq!(m.get("replenishRate").map(String::as_str), Some("+$0.25/hr"));
        assert_eq!(m.get("bonus").map(String::as_str), Some("+20% (5d)"));
        assert_eq!(m.get("creditsBalance").map(String::as_str), Some("$10"));
        // fullAt presente e no futuro
        let resets = q.primary.as_ref().unwrap().resets_at.as_deref().unwrap();
        assert!(resets.ends_with('Z'));
    }

    #[test]
    fn eta_with_bonus_is_about_5h() {
        // eff = 0.25 * 1.20 = 0.30; hours = 1.5/0.30 = 5.0
        let q = parse_usage(FULL, base(), NOW);
        let resets = q.primary.as_ref().unwrap().resets_at.as_deref().unwrap();
        // 5h após NOW
        let expected = iso_from_ms(NOW + 5 * 3_600_000);
        assert_eq!(resets, expected);
    }

    #[test]
    fn no_bonus_eta_is_6h_and_meta_omits_bonus() {
        let out = "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining\nreplenishes +$0.25/hour";
        let q = parse_usage(out, base(), NOW);
        let m = meta_of(&q);
        assert!(m.get("bonus").is_none());
        assert_eq!(m.get("replenishRate").map(String::as_str), Some("+$0.25/hr"));
        // hours = 1.5/0.25 = 6.0
        let resets = q.primary.as_ref().unwrap().resets_at.as_deref().unwrap();
        assert_eq!(resets, iso_from_ms(NOW + 6 * 3_600_000));
        // sem credits
        assert!(q.models.as_ref().unwrap().get("Credits").is_none());
    }

    #[test]
    fn no_replenish_means_null_resets_and_no_meta_rate() {
        let out = "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining";
        let q = parse_usage(out, base(), NOW);
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
        let m = meta_of(&q);
        assert!(m.get("replenishRate").is_none());
        assert!(m.get("bonus").is_none());
    }

    #[test]
    fn full_quota_has_null_resets() {
        let out = "Signed in as user@email.com\nAmp Free: $5.00/$5.00 remaining\nreplenishes +$0.25/hour";
        let q = parse_usage(out, base(), NOW);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 100.0);
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
    }

    #[test]
    fn zero_replenish_stays_available_null_resets() {
        let out = "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining\nreplenishes +$0/hour";
        let q = parse_usage(out, base(), NOW);
        assert!(q.available);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 70.0);
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
    }

    #[test]
    fn zero_credits_balance_means_remaining_zero() {
        let out = "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining\nreplenishes +$0.25/hour\nIndividual credits: $0.00 remaining";
        let q = parse_usage(out, base(), NOW);
        assert_eq!(q.models.as_ref().unwrap()["Credits"].remaining, 0.0);
        assert_eq!(meta_of(&q).get("creditsBalance").map(String::as_str), Some("$0"));
    }
}
```

- [ ] **Step 4: Verificar**

Run: `cargo test --manifest-path rust/Cargo.toml amp 2>&1 | tail -6` → testes de `amp` (parse) + `amp_cli` passam.
Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -3` → sem issues.

- [ ] **Step 5: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/providers/amp.rs rust/src/providers/mod.rs
git commit -m "feat(rust): parse_usage do Amp (regex)"
```

---

### Task 4: `AmpProvider` (spawn + QuotaSource + registry)

**Files:**
- Modify: `rust/src/providers/amp.rs` (adicionar `run_amp_usage` + `AmpProvider`)
- Modify: `rust/src/providers/mod.rs` (`registry()` ganha Amp)

**Interfaces:**
- Consumes: `amp_cli::find_amp_bin`, `error::{AmpError, ProviderError}`, `base::{base_get_quota, quota_base, QuotaSource}`, `super::{Ctx, Provider}`, `config::HTTP_TIMEOUT_SECS`.
- Produces: `AmpProvider` (unit struct) impl `Provider` + `QuotaSource` (Raw=String); `run_amp_usage`.

**Contrato (de `amp.ts`):**
- `is_available` = `find_amp_bin(home).is_some()`.
- `unavailable_error` = `AmpError::NotInstalled`.
- `fetch_raw`: bin não achado → `Err(AmpError::Generic)` (TS 'Amp CLI not found' → não-sentinel → Generic); spawn `amp usage` (env `NO_COLOR=1`/`TERM=dumb`, stdout/stderr pipe, `kill_on_drop`); timeout 5s → `Err(AmpError::NotLoggedIn)` (TS: kill→exit≠0→sentinel); `wait_with_output` drena stderr concorrente; `exit≠0 || !"Signed in as"` → `Err(AmpError::NotLoggedIn)`; senão `Ok(stdout)`.
- `build_quota` = `parse_usage(&raw, base, ctx.now_ms)` (sempre sucesso).
- `to_user_facing_error`: `Amp(NotLoggedIn)` → string NotLoggedIn; senão → Generic.

- [ ] **Step 1: Adicionar `run_amp_usage` + imports em `amp.rs`**

No topo de `amp.rs`, adicionar imports:
```rust
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;

use super::base::{base_get_quota, quota_base, QuotaSource};
use super::error::{AmpError, ProviderError};
use super::{Ctx, Provider};
use crate::config::HTTP_TIMEOUT_SECS;
use crate::providers::amp_cli::find_amp_bin;
```

Adicionar a função de spawn:
```rust
/// Roda `amp usage` e devolve o stdout cru. Lança (sem cachear) em auth-fail.
/// `wait_with_output` drena stdout E stderr concorrentemente (sem deadlock de
/// pipe); `kill_on_drop` garante kill no timeout.
async fn run_amp_usage(bin: &Path) -> Result<String, ProviderError> {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.arg("usage")
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn().map_err(|_| AmpError::Generic)?;
    let output = match tokio::time::timeout(
        Duration::from_secs(HTTP_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    {
        Ok(Ok(o)) => o,
        Ok(Err(_)) => return Err(AmpError::Generic.into()),
        // timeout: kill_on_drop mata o filho; espelha o TS (kill→exit≠0→não-logado).
        Err(_) => return Err(AmpError::NotLoggedIn.into()),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let signed = lazy_re!(RE_SIGNED, r"Signed in as (\S+)")
        .map(|r| r.is_match(&stdout))
        .unwrap_or(false);
    if !output.status.success() || !signed {
        return Err(AmpError::NotLoggedIn.into());
    }
    Ok(stdout)
}
```

- [ ] **Step 2: Adicionar `AmpProvider` + impls**

```rust
pub struct AmpProvider;

#[async_trait(?Send)]
impl QuotaSource for AmpProvider {
    type Raw = String;
    fn id(&self) -> &'static str { "amp" }
    fn name(&self) -> &'static str { "Amp" }
    fn cache_key(&self) -> &'static str { "amp-quota" }

    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        find_amp_bin(&ctx.home.to_string_lossy()).is_some()
    }

    async fn fetch_raw(&self, ctx: &Ctx<'_>) -> Result<String, ProviderError> {
        let bin = find_amp_bin(&ctx.home.to_string_lossy()).ok_or(AmpError::Generic)?;
        run_amp_usage(&bin).await
    }

    fn build_quota(&self, raw: String, base: ProviderQuota, ctx: &Ctx<'_>) -> ProviderQuota {
        parse_usage(&raw, base, ctx.now_ms)
    }

    fn unavailable_error(&self) -> String {
        AmpError::NotInstalled.to_string()
    }

    fn to_user_facing_error(&self, error: &ProviderError) -> String {
        match error {
            ProviderError::Amp(AmpError::NotLoggedIn) => AmpError::NotLoggedIn.to_string(),
            _ => AmpError::Generic.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl Provider for AmpProvider {
    fn id(&self) -> &'static str { "amp" }
    fn name(&self) -> &'static str { "Amp" }
    fn cache_key(&self) -> &'static str { "amp-quota" }
    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        QuotaSource::is_available(self, ctx).await
    }
    async fn get_quota(&self, ctx: &Ctx<'_>) -> ProviderQuota {
        base_get_quota(self, ctx).await
    }
}
```

**Nota:** `quota_base` é importado mas pode não ser usado diretamente aqui (o `base_get_quota` o usa internamente). Se o compilador acusar import não-usado, remover `quota_base` do `use`.

- [ ] **Step 3: Registrar no `registry()`**

Em `rust/src/providers/mod.rs`, trocar:
```rust
pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![Box::new(claude::ClaudeProvider), Box::new(amp::AmpProvider)]
}
```
E atualizar o teste `registry_has_claude` (ou adicionar `registry_has_claude_and_amp`): assert `len() == 2`, ids `["claude", "amp"]`.

- [ ] **Step 4: Testes de spawn + orquestração** (append em `amp.rs` tests)

Use um script-fake executável num tempdir p/ exercitar `run_amp_usage` (spawn real, sem mock):

```rust
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    /// Escreve um script `amp` fake executável que imprime `body` e sai com `code`.
    fn fake_amp(dir: &Path, body: &str, code: i32) -> std::path::PathBuf {
        let p = dir.join("amp");
        let mut f = std::fs::File::create(&p).unwrap();
        write!(f, "#!/bin/sh\ncat <<'EOF'\n{body}\nEOF\nexit {code}\n").unwrap();
        let mut perms = std::fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&p, perms).unwrap();
        p
    }

    #[tokio::test]
    async fn run_amp_usage_ok_on_signed_in() {
        let dir = tempdir().unwrap();
        let bin = fake_amp(dir.path(), "Signed in as me@x.com\nAmp Free: $5.00/$5.00 remaining", 0);
        let out = run_amp_usage(&bin).await.unwrap();
        assert!(out.contains("Signed in as me@x.com"));
    }

    #[tokio::test]
    async fn run_amp_usage_errs_on_nonzero_exit() {
        let dir = tempdir().unwrap();
        let bin = fake_amp(dir.path(), "boom", 1);
        let err = run_amp_usage(&bin).await.unwrap_err();
        assert_eq!(err.to_string(), "Not logged in. Open `agent-bar menu` and choose Provider login.");
    }

    #[tokio::test]
    async fn run_amp_usage_errs_when_no_signed_in_line() {
        let dir = tempdir().unwrap();
        let bin = fake_amp(dir.path(), "some unexpected output", 0);
        let err = run_amp_usage(&bin).await.unwrap_err();
        assert_eq!(err.to_string(), "Not logged in. Open `agent-bar menu` and choose Provider login.");
    }

    #[tokio::test]
    async fn unavailable_when_no_bin() {
        use crate::providers::test_support::{ctx_for, settings};
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        // ctx.home aponta p/ um tempdir vazio (sem amp); PATH pode ter amp real,
        // então este teste verifica build_quota/orquestração via fake, não is_available.
        let ctx = ctx_for(dir.path(), &s, &client, NOW);
        // build_quota direto (sem spawn) com stdout mockado.
        let q = AmpProvider.build_quota(
            "Signed in as me@x.com\nAmp Free: $5.00/$5.00 remaining".to_string(),
            quota_base("amp", "Amp"),
            &ctx,
        );
        assert!(q.available);
        assert_eq!(q.account.as_deref(), Some("me@x.com"));
    }
```

**Nota p/ o implementer:** `quota_base` precisa estar importado no escopo de teste (já está no `use super::base::...` do módulo, mas confirme que está acessível nos testes; se não, importe `use super::base::quota_base;` no `mod tests`). O teste de `is_available` real é frágil (PATH pode ter `amp`), então cobrimos a orquestração via `build_quota` direto + os 3 testes de `run_amp_usage` com script-fake (que exercitam o spawn real).

- [ ] **Step 5: Verificar**

Run: `cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -8` → todos passam (208 + amp_cli + amp parse + amp spawn).
Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -3` → sem issues.

- [ ] **Step 6: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/providers/amp.rs rust/src/providers/mod.rs
git commit -m "feat(rust): AmpProvider (spawn usage + registry)"
```

---

## Self-Review (autor)

- **Cobertura do spec:** §3.5 (descoberta de binário ordem; spawn NO_COLOR/TERM; drain stderr; kill; auth fail→Err antes do cache) → T2/T4. Parse byte-exact (dollar strings via parseFloat; pct round; fullAt com/sem bonus; zero-replenish; full-quota; zero-credits) → T3 (porta `amp.test.ts`).
- **Extensões de fundação:** `Ctx.home` (candidatos do binário) + `build_quota(ctx)` (fullAt usa now atual) → T1, com a suíte 208 re-verificada.
- **Fidelidade de ordem:** `models` IndexMap (Free Tier → Credits); `meta` BTreeMap (acesso por chave).
- **Sem placeholders:** todo passo tem código real + testes concretos.
- **Consistência de tipos:** `QuotaSource::Raw=String`; `build_quota(ctx)` casa T1↔T4; `parse_usage(stdout, base, now_ms)` casa T3↔T4.
- **DEFERIDO:** Codex (04c), notify (04d). `registry()` cresce no 04c.
