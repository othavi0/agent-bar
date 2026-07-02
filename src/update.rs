//! Detecção de tipo de instalação e lógica de update.
//!
//! `ManagedGit`/`DevGit`: fluxo git, port original de `src/update.ts:113-252`
//! (mantido — o `run_managed_update` continua fazendo `fetch`/`reset --hard`/
//! `clean -fd`/`run_setup`).
//!
//! `Standalone`: self-update via GitHub Releases. Substitui o antigo caminho
//! `Npm` (hotfix 7.0.1) — `detect_install_kind` usava `CARGO_MANIFEST_DIR`
//! (path de COMPILE TIME, aponta pro runner do CI) pra achar o repo_root; em
//! qualquer instalação standalone (curl|bash) isso caía sempre no ramo `Npm`
//! tentando ler um `package.json` inexistente. A detecção agora parte do
//! binário real (`std::env::current_exe()`), subindo diretórios em runtime.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::bail;
use serde_json::Value;

use crate::app_identity::TERMINAL_HELPER_NAME;
use crate::runtime::is_system_install;

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// `owner/repo` do GitHub — mesmo valor usado por `install.sh` (`GITHUB_REPO`).
pub const GITHUB_REPO: &str = "othavioquiliao/agent-bar";

/// User-Agent dedicado pro self-update (GitHub exige um; o cliente HTTP
/// compartilhado em `http::client()` é específico do Claude — timeout curto
/// de 5s e UA `claude-code/...` não servem pra baixar um tarball de release).
pub const SELFUPDATE_USER_AGENT: &str = concat!("agent-bar-selfupdate/", env!("CARGO_PKG_VERSION"));

const RELEASE_ASSET_ARCH: &str = "x86_64";

// ---------------------------------------------------------------------------
// InstallKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstallKind {
    ManagedGit,
    DevGit,
    Standalone,
    System,
}

// ---------------------------------------------------------------------------
// CommandResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub ok: bool,
    pub output: String,
}

// ---------------------------------------------------------------------------
// is_managed_install_root
// ---------------------------------------------------------------------------

/// Retorna `true` se `repo_root` e `install_root` apontam para o mesmo diretório.
///
/// Premissa: os callers passam paths absolutos já normalizados (sem `.`/`..`/
/// separadores duplos). Os testes usam tempdirs, que já são limpos. Para
/// fidelidade ao `path.resolve()` do TS (normalização sem I/O), comparamos os
/// componentes canônicos sem tocar no filesystem.
pub fn is_managed_install_root(repo_root: &Path, install_root: &Path) -> bool {
    normalize(repo_root) == normalize(install_root)
}

/// Normaliza um path sem I/O (elimina `.` e resolve `..`).
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Detecção via binário real (substitui CARGO_MANIFEST_DIR — hotfix 7.0.1)
// ---------------------------------------------------------------------------

/// Extrai um campo simples de `[package]` em um `Cargo.toml` (ex: `name`,
/// `version`). Parse de linha só — não lida com tabelas inline/arrays;
/// suficiente pro nosso próprio manifest (evita puxar a dep `toml` só pra
/// isso). Para de considerar campos assim que sai da seção `[package]`.
fn cargo_toml_package_field(content: &str, field: &str) -> Option<String> {
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() != field {
            continue;
        }
        let value = value.split('#').next().unwrap_or(value).trim();
        let value = value.trim_matches('"');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// Sobe a árvore de diretórios a partir do binário (`exe`) procurando um
/// checkout git (diretório com `.git/`) cujo `Cargo.toml` declare
/// `[package] name = "agent-bar"`. Substitui `CARGO_MANIFEST_DIR`.
pub fn find_repo_root(exe: &Path) -> Option<PathBuf> {
    let mut dir = exe.parent();
    while let Some(d) = dir {
        if d.join(".git").exists() {
            if let Ok(content) = fs::read_to_string(d.join("Cargo.toml")) {
                if cargo_toml_package_field(&content, "name").as_deref() == Some("agent-bar") {
                    return Some(d.to_path_buf());
                }
            }
        }
        dir = d.parent();
    }
    None
}

/// Lê `version` do `Cargo.toml` em `repo_root`. Substitui a leitura de versão
/// via `package.json` (que o caminho `Npm`, removido neste hotfix, usava).
pub fn read_cargo_version(repo_root: &Path) -> anyhow::Result<String> {
    let manifest = repo_root.join("Cargo.toml");
    let content = fs::read_to_string(&manifest)
        .map_err(|e| anyhow::anyhow!("Failed to read Cargo.toml: {e}"))?;
    cargo_toml_package_field(&content, "version")
        .ok_or_else(|| anyhow::anyhow!("Cargo.toml is missing package.version"))
}

/// Detecta o tipo de instalação em uso.
///
/// Ordem de precedência:
/// 1. `is_system_install()` → `System`
/// 2. `repo_root` resolvido (via `find_repo_root`) → `ManagedGit` se
///    `is_managed_install_root`, senão `DevGit`
/// 3. Nenhum checkout git encontrado a partir do binário → `Standalone`
pub fn detect_install_kind(repo_root: Option<&Path>, install_root: &Path) -> InstallKind {
    if is_system_install() {
        return InstallKind::System;
    }
    match repo_root {
        Some(root) if is_managed_install_root(root, install_root) => InstallKind::ManagedGit,
        Some(_) => InstallKind::DevGit,
        None => InstallKind::Standalone,
    }
}

// ---------------------------------------------------------------------------
// Helpers internos (fluxo git)
// ---------------------------------------------------------------------------

fn split_lines(output: &str) -> Vec<String> {
    output
        .split('\n')
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Executa um comando via `run_command`; falha com `anyhow::Error` se não ok.
fn require_command(
    run_command: &dyn Fn(&str, &[String], &Path) -> CommandResult,
    cwd: &Path,
    step: &str,
    cmd: &str,
    args: &[String],
) -> anyhow::Result<String> {
    let result = run_command(cmd, args, cwd);
    if !result.ok {
        let suffix = if result.output.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", result.output.trim())
        };
        bail!("{step} failed{suffix}");
    }
    Ok(result.output.trim().to_string())
}

/// Resolve o upstream tracking branch. Port de `resolveUpstream` no TS.
///
/// Primeiro tenta `@{u}` (direct run_command, não require_command);
/// se não disponível, usa `rev-parse --verify origin/master`.
fn resolve_upstream(
    run_command: &dyn Fn(&str, &[String], &Path) -> CommandResult,
    repo_root: &Path,
) -> anyhow::Result<String> {
    let args: Vec<String> = vec![
        "rev-parse".to_string(),
        "--abbrev-ref".to_string(),
        "--symbolic-full-name".to_string(),
        "@{u}".to_string(),
    ];
    let result = run_command("git", &args, repo_root);
    if result.ok && !result.output.trim().is_empty() {
        return Ok(result.output.trim().to_string());
    }

    require_command(
        run_command,
        repo_root,
        "Resolve origin/master",
        "git",
        &[
            "rev-parse".to_string(),
            "--verify".to_string(),
            "origin/master".to_string(),
        ],
    )
}

// ---------------------------------------------------------------------------
// UpdateSummary / ManagedUpdate*
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UpdateSummary {
    pub repo_root: PathBuf,
    pub install_root: PathBuf,
    pub current_commit: String,
    pub current_branch: String,
    pub upstream: String,
    pub commits: Vec<String>,
    pub local_changes: Vec<String>,
    pub has_updates: bool,
    pub has_local_changes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ManagedUpdateStatus {
    WrongRoot,
    UpToDate,
    Cancelled,
    Updated,
}

#[derive(Debug, Clone)]
pub struct ManagedUpdateResult {
    pub status: ManagedUpdateStatus,
    pub repo_root: PathBuf,
    pub install_root: PathBuf,
    pub summary: Option<UpdateSummary>,
}

pub struct ManagedUpdateOptions<'a> {
    pub repo_root: &'a Path,
    pub install_root: &'a Path,
    pub run_command: &'a dyn Fn(&str, &[String], &Path) -> CommandResult,
    /// Side-effect de setup; testes contam invocações via `Cell<u32>`.
    pub run_setup: &'a dyn Fn(),
    pub confirm: &'a dyn Fn(&UpdateSummary) -> bool,
}

/// Port de `runManagedUpdate` (`src/update.ts:161-252`), sem o passo de
/// instalação de dependências via `bun` — pós-cutover pra Rust não há mais
/// `package.json`/`node_modules` num checkout do agent-bar (hard rule: sem
/// Node/npm/bun em runtime).
pub fn run_managed_update(opts: ManagedUpdateOptions<'_>) -> anyhow::Result<ManagedUpdateResult> {
    let repo_root = opts.repo_root;
    let install_root = opts.install_root;

    // Early exit: wrong-root, sem rodar nenhum comando.
    if !is_managed_install_root(repo_root, install_root) {
        return Ok(ManagedUpdateResult {
            status: ManagedUpdateStatus::WrongRoot,
            repo_root: repo_root.to_path_buf(),
            install_root: install_root.to_path_buf(),
            summary: None,
        });
    }

    // --- Fase de inspeção ---
    require_command(
        opts.run_command,
        repo_root,
        "Check git repository",
        "git",
        &["rev-parse".to_string(), "--git-dir".to_string()],
    )?;

    let current_commit = require_command(
        opts.run_command,
        repo_root,
        "Read current commit",
        "git",
        &[
            "rev-parse".to_string(),
            "--short".to_string(),
            "HEAD".to_string(),
        ],
    )?;

    let current_branch = require_command(
        opts.run_command,
        repo_root,
        "Read current branch",
        "git",
        &["branch".to_string(), "--show-current".to_string()],
    )?;

    require_command(
        opts.run_command,
        repo_root,
        "Fetch origin",
        "git",
        &[
            "fetch".to_string(),
            "--prune".to_string(),
            "origin".to_string(),
        ],
    )?;

    let upstream = resolve_upstream(opts.run_command, repo_root)?;

    let commits = split_lines(&require_command(
        opts.run_command,
        repo_root,
        "List incoming commits",
        "git",
        &[
            "log".to_string(),
            "--oneline".to_string(),
            format!("HEAD..{upstream}"),
            "-10".to_string(),
        ],
    )?);

    let local_changes = split_lines(&require_command(
        opts.run_command,
        repo_root,
        "Read local changes",
        "git",
        &["status".to_string(), "--short".to_string()],
    )?);

    let summary = UpdateSummary {
        repo_root: repo_root.to_path_buf(),
        install_root: install_root.to_path_buf(),
        current_commit,
        current_branch,
        upstream: upstream.clone(),
        has_updates: !commits.is_empty(),
        has_local_changes: !local_changes.is_empty(),
        commits,
        local_changes,
    };

    // Sem mudanças → up-to-date; NÃO chamar confirm.
    if !summary.has_updates && !summary.has_local_changes {
        return Ok(ManagedUpdateResult {
            status: ManagedUpdateStatus::UpToDate,
            repo_root: repo_root.to_path_buf(),
            install_root: install_root.to_path_buf(),
            summary: Some(summary),
        });
    }

    // Confirmação destrutiva.
    let approved = (opts.confirm)(&summary);
    if !approved {
        return Ok(ManagedUpdateResult {
            status: ManagedUpdateStatus::Cancelled,
            repo_root: repo_root.to_path_buf(),
            install_root: install_root.to_path_buf(),
            summary: Some(summary),
        });
    }

    // --- Fase de mutação ---
    require_command(
        opts.run_command,
        repo_root,
        "Reset install checkout",
        "git",
        &["reset".to_string(), "--hard".to_string(), upstream.clone()],
    )?;

    require_command(
        opts.run_command,
        repo_root,
        "Clean install checkout",
        "git",
        &["clean".to_string(), "-fd".to_string()],
    )?;

    (opts.run_setup)();

    Ok(ManagedUpdateResult {
        status: ManagedUpdateStatus::Updated,
        repo_root: repo_root.to_path_buf(),
        install_root: install_root.to_path_buf(),
        summary: Some(summary),
    })
}

// ---------------------------------------------------------------------------
// Standalone self-update (GitHub Releases) — substitui o caminho `Npm`
// ---------------------------------------------------------------------------

/// Diretório temporário próprio: evita promover `tempfile` de dev-dependency
/// pra `[dependencies]` numa hotfix. Limpo via `Drop` (best-effort, como o
/// `trap "rm -rf ..." EXIT` do `install.sh`).
struct SelfUpdateTempDir(PathBuf);

impl SelfUpdateTempDir {
    fn new() -> anyhow::Result<Self> {
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("agent-bar-selfupdate-{pid}-{nanos}"));
        fs::create_dir_all(&dir)
            .map_err(|e| anyhow::anyhow!("Failed to create tempdir {}: {e}", dir.display()))?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for SelfUpdateTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Extrai `tag_name` de um payload JSON de release do GitHub. Port do
/// `grep '"tag_name"' | sed ...` do `install.sh`.
fn parse_release_tag(body: &str) -> anyhow::Result<String> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| anyhow::anyhow!("Failed to parse GitHub release response: {e}"))?;
    v.get("tag_name")
        .and_then(|t| t.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("GitHub release response missing tag_name"))
}

/// Remove o prefixo `v` de uma versão/tag (`v7.0.1` → `7.0.1`).
fn version_bare(v: &str) -> &str {
    v.strip_prefix('v').unwrap_or(v)
}

/// `true` se a tag da última release corresponde à versão compilada no
/// binário atual (comparação sem prefixo `v`).
fn is_up_to_date(current_version: &str, latest_tag: &str) -> bool {
    version_bare(current_version) == version_bare(latest_tag)
}

/// Nome do asset tarball, seguindo o padrão do `publish.yml`/`install.sh`.
fn asset_filename(ver_bare: &str) -> String {
    format!("agent-bar-{ver_bare}-{RELEASE_ASSET_ARCH}.tar.gz")
}

/// Diretório de dados (`icons/`, `scripts/`) usado pelo `install.sh`.
/// Resolução canônica em `config::agent_bar_data_dir` — compartilhada com
/// `waybar_contract::standalone_data_asset_dir` (hotfix 7.0.1: as duas
/// resolviam essa pasta de jeitos diferentes, split-brain pra quem setasse
/// só `XDG_DATA_HOME`).
pub fn default_data_dir(home: &Path) -> PathBuf {
    crate::config::agent_bar_data_dir(home)
}

async fn download_file(client: &reqwest::Client, url: &str, dest: &Path) -> anyhow::Result<()> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to download {url}: {e}"))?;
    if !resp.status().is_success() {
        bail!("Failed to download {url}: HTTP {}", resp.status());
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read response body from {url}: {e}"))?;
    fs::write(dest, &bytes)
        .map_err(|e| anyhow::anyhow!("Failed to write {}: {e}", dest.display()))?;
    Ok(())
}

/// Verifica o checksum via `sha256sum -c` (shell-out; sem dep nova de
/// criptografia). Caller garante que `sha256sum` existe no PATH antes de
/// chamar — nunca pula a verificação silenciosamente.
fn verify_checksum(
    run_command: &dyn Fn(&str, &[String], &Path) -> CommandResult,
    tmp_dir: &Path,
    asset: &str,
) -> anyhow::Result<()> {
    require_command(
        run_command,
        tmp_dir,
        "Verify checksum",
        "sha256sum",
        &["-c".to_string(), format!("{asset}.sha256")],
    )?;
    Ok(())
}

fn extract_tarball(
    run_command: &dyn Fn(&str, &[String], &Path) -> CommandResult,
    tmp_dir: &Path,
    asset: &str,
) -> anyhow::Result<()> {
    require_command(
        run_command,
        tmp_dir,
        "Extract release archive",
        "tar",
        &["xzf".to_string(), asset.to_string()],
    )?;
    Ok(())
}

/// Substitui o binário atual atomicamente: copia pra `<exe>.new` no mesmo
/// diretório, ajusta permissões 755, e `rename()` sobre o exe atual
/// (seguro no Linux mesmo com o exe em execução).
fn replace_binary_atomic(new_binary: &Path, target_exe: &Path) -> anyhow::Result<()> {
    let staging = target_exe.with_extension("new");
    fs::copy(new_binary, &staging)
        .map_err(|e| anyhow::anyhow!("Failed to stage new binary at {}: {e}", staging.display()))?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))
        .map_err(|e| anyhow::anyhow!("Failed to set permissions on {}: {e}", staging.display()))?;
    fs::rename(&staging, target_exe).map_err(|e| {
        anyhow::anyhow!(
            "Failed to install new binary at {}: {e}",
            target_exe.display()
        )
    })?;
    Ok(())
}

fn copy_dir_contents(src: &Path, dest: &Path) -> anyhow::Result<()> {
    for entry in
        fs::read_dir(src).map_err(|e| anyhow::anyhow!("Failed to read {}: {e}", src.display()))?
    {
        let entry =
            entry.map_err(|e| anyhow::anyhow!("Failed to read entry in {}: {e}", src.display()))?;
        let path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if path.is_dir() {
            fs::create_dir_all(&dest_path)
                .map_err(|e| anyhow::anyhow!("Failed to create {}: {e}", dest_path.display()))?;
            copy_dir_contents(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)
                .map_err(|e| anyhow::anyhow!("Failed to copy {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// Espelha o layout de assets do `install.sh` (linhas 139-148): `icons/.` →
/// `<data_dir>/icons/`, `scripts/agent-bar-open-terminal` → `<data_dir>/scripts/`
/// (755).
fn install_standalone_assets(extracted_dir: &Path, data_dir: &Path) -> anyhow::Result<()> {
    let icons_src = extracted_dir.join("icons");
    let icons_dest = data_dir.join("icons");
    let scripts_dest = data_dir.join("scripts");
    fs::create_dir_all(&icons_dest)
        .map_err(|e| anyhow::anyhow!("Failed to create {}: {e}", icons_dest.display()))?;
    fs::create_dir_all(&scripts_dest)
        .map_err(|e| anyhow::anyhow!("Failed to create {}: {e}", scripts_dest.display()))?;

    if icons_src.is_dir() {
        copy_dir_contents(&icons_src, &icons_dest)?;
    }

    let script_src = extracted_dir.join("scripts").join(TERMINAL_HELPER_NAME);
    if script_src.is_file() {
        let script_dest = scripts_dest.join(TERMINAL_HELPER_NAME);
        fs::copy(&script_src, &script_dest)
            .map_err(|e| anyhow::anyhow!("Failed to install {}: {e}", script_dest.display()))?;
        fs::set_permissions(&script_dest, fs::Permissions::from_mode(0o755)).map_err(|e| {
            anyhow::anyhow!(
                "Failed to set permissions on {}: {e}",
                script_dest.display()
            )
        })?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum StandaloneUpdateStatus {
    UpToDate {
        version: String,
    },
    Updated {
        old_version: String,
        new_version: String,
    },
}

pub struct StandaloneUpdateOptions<'a> {
    pub current_version: &'a str,
    pub exe_path: &'a Path,
    pub data_dir: &'a Path,
    pub run_command: &'a dyn Fn(&str, &[String], &Path) -> CommandResult,
    pub http: &'a reqwest::Client,
    pub releases_api_url: String,
    pub download_base_url: String,
    /// Seams de teste: substituem os checks reais de `sha256sum`/`tar` no PATH
    /// (produção usa `crate::install::has_cmd`).
    pub has_sha256sum: &'a dyn Fn() -> bool,
    pub has_tar: &'a dyn Fn() -> bool,
}

/// Self-update via GitHub Releases. Substitui `run_npm_update` (hotfix 7.0.1):
/// resolve a última release, compara com a versão compilada no binário atual,
/// baixa + verifica checksum + extrai, e substitui o binário atomicamente.
pub async fn run_standalone_update(
    opts: StandaloneUpdateOptions<'_>,
) -> anyhow::Result<StandaloneUpdateStatus> {
    let resp = opts
        .http
        .get(&opts.releases_api_url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to reach GitHub releases API: {e}"))?;
    if !resp.status().is_success() {
        bail!("GitHub releases API returned HTTP {}", resp.status());
    }
    let body = resp
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read GitHub releases API response: {e}"))?;
    let tag = parse_release_tag(&body)?;

    if is_up_to_date(opts.current_version, &tag) {
        return Ok(StandaloneUpdateStatus::UpToDate {
            version: opts.current_version.to_string(),
        });
    }

    if !(opts.has_sha256sum)() {
        bail!(
            "sha256sum not found. Install coreutils via your distro's package manager to verify the download."
        );
    }
    if !(opts.has_tar)() {
        bail!("tar not found. Install via your distro's package manager.");
    }

    let ver_bare = version_bare(&tag).to_string();
    let asset = asset_filename(&ver_bare);

    let tmp = SelfUpdateTempDir::new()?;
    let tmp_path = tmp.path();

    download_file(
        opts.http,
        &format!("{}/{tag}/{asset}", opts.download_base_url),
        &tmp_path.join(&asset),
    )
    .await?;
    download_file(
        opts.http,
        &format!("{}/{tag}/{asset}.sha256", opts.download_base_url),
        &tmp_path.join(format!("{asset}.sha256")),
    )
    .await?;

    verify_checksum(opts.run_command, tmp_path, &asset)?;
    extract_tarball(opts.run_command, tmp_path, &asset)?;

    let new_binary = tmp_path.join("agent-bar");
    if !new_binary.exists() {
        bail!("Extracted release archive is missing the agent-bar binary");
    }
    replace_binary_atomic(&new_binary, opts.exe_path)?;
    install_standalone_assets(tmp_path, opts.data_dir)?;

    Ok(StandaloneUpdateStatus::Updated {
        old_version: opts.current_version.to_string(),
        new_version: ver_bare,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::fs;

    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    // -----------------------------------------------------------------------
    // Fake runner
    // -----------------------------------------------------------------------

    struct Fake {
        outputs: HashMap<String, String>,
        commands: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Fake {
        fn new(pairs: &[(&str, &str)]) -> Self {
            Fake {
                outputs: pairs
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                commands: RefCell::new(vec![]),
            }
        }

        fn run(&self, cmd: &str, args: &[String], _cwd: &Path) -> CommandResult {
            self.commands
                .borrow_mut()
                .push((cmd.to_string(), args.to_vec()));
            let key = format!("{} {}", cmd, args.join(" "));
            CommandResult {
                ok: true,
                output: self.outputs.get(&key).cloned().unwrap_or_default(),
            }
        }
    }

    // -----------------------------------------------------------------------
    // runManagedUpdate tests
    // -----------------------------------------------------------------------

    #[test]
    fn managed_aborts_outside_install_root() {
        let fake = Fake::new(&[]);
        let r = run_managed_update(ManagedUpdateOptions {
            repo_root: Path::new("/tmp/dev/agent-bar"),
            install_root: Path::new("/home/test/.agent-bar"),
            run_command: &|c, a, p| fake.run(c, a, p),
            run_setup: &|| {},
            confirm: &|_| true,
        })
        .unwrap();
        assert!(matches!(r.status, ManagedUpdateStatus::WrongRoot));
        assert!(fake.commands.borrow().is_empty());
    }

    #[test]
    fn managed_resets_cleans_and_runs_setup() {
        let tmp = tempdir().unwrap();
        let install_root = tmp.path();
        let setup_count = Cell::new(0u32);

        let fake = Fake::new(&[
            ("git rev-parse --git-dir", ".git\n"),
            ("git rev-parse --short HEAD", "abc123\n"),
            ("git branch --show-current", "master\n"),
            (
                "git rev-parse --abbrev-ref --symbolic-full-name @{u}",
                "origin/master\n",
            ),
            (
                "git log --oneline HEAD..origin/master -10",
                "def456 update cli\n",
            ),
            ("git status --short", " M README.md\n?? scratch.txt\n"),
        ]);

        let r = run_managed_update(ManagedUpdateOptions {
            repo_root: install_root,
            install_root,
            run_command: &|c, a, p| fake.run(c, a, p),
            run_setup: &|| setup_count.set(setup_count.get() + 1),
            confirm: &|summary| {
                assert!(summary.has_local_changes);
                assert!(summary.has_updates);
                true
            },
        })
        .unwrap();

        assert!(matches!(r.status, ManagedUpdateStatus::Updated));
        assert_eq!(setup_count.get(), 1);

        let cmds: Vec<(String, Vec<String>)> = fake.commands.borrow().clone();
        let expected: Vec<(String, Vec<String>)> = vec![
            (
                "git".to_string(),
                vec!["rev-parse".to_string(), "--git-dir".to_string()],
            ),
            (
                "git".to_string(),
                vec![
                    "rev-parse".to_string(),
                    "--short".to_string(),
                    "HEAD".to_string(),
                ],
            ),
            (
                "git".to_string(),
                vec!["branch".to_string(), "--show-current".to_string()],
            ),
            (
                "git".to_string(),
                vec![
                    "fetch".to_string(),
                    "--prune".to_string(),
                    "origin".to_string(),
                ],
            ),
            (
                "git".to_string(),
                vec![
                    "rev-parse".to_string(),
                    "--abbrev-ref".to_string(),
                    "--symbolic-full-name".to_string(),
                    "@{u}".to_string(),
                ],
            ),
            (
                "git".to_string(),
                vec![
                    "log".to_string(),
                    "--oneline".to_string(),
                    "HEAD..origin/master".to_string(),
                    "-10".to_string(),
                ],
            ),
            (
                "git".to_string(),
                vec!["status".to_string(), "--short".to_string()],
            ),
            (
                "git".to_string(),
                vec![
                    "reset".to_string(),
                    "--hard".to_string(),
                    "origin/master".to_string(),
                ],
            ),
            (
                "git".to_string(),
                vec!["clean".to_string(), "-fd".to_string()],
            ),
        ];
        assert_eq!(cmds, expected);
    }

    #[test]
    fn managed_cancelled_does_not_reset_or_setup() {
        let tmp = tempdir().unwrap();
        let install_root = tmp.path();
        let setup_count = Cell::new(0u32);

        let fake = Fake::new(&[
            ("git rev-parse --git-dir", ".git\n"),
            ("git rev-parse --short HEAD", "abc123\n"),
            ("git branch --show-current", "master\n"),
            (
                "git rev-parse --abbrev-ref --symbolic-full-name @{u}",
                "origin/master\n",
            ),
            (
                "git log --oneline HEAD..origin/master -10",
                "def456 update cli\n",
            ),
            ("git status --short", " M README.md\n"),
        ]);

        let r = run_managed_update(ManagedUpdateOptions {
            repo_root: install_root,
            install_root,
            run_command: &|c, a, p| fake.run(c, a, p),
            run_setup: &|| setup_count.set(setup_count.get() + 1),
            confirm: &|_| false,
        })
        .unwrap();

        assert!(matches!(r.status, ManagedUpdateStatus::Cancelled));
        assert_eq!(setup_count.get(), 0);

        let cmds = fake.commands.borrow();
        assert!(!cmds
            .iter()
            .any(|(cmd, args)| cmd == "git" && args.first().map(|s| s.as_str()) == Some("reset")));
        assert!(!cmds
            .iter()
            .any(|(cmd, args)| cmd == "git" && args.first().map(|s| s.as_str()) == Some("clean")));
    }

    #[test]
    fn managed_up_to_date_does_not_call_confirm() {
        let tmp = tempdir().unwrap();
        let install_root = tmp.path();
        let setup_count = Cell::new(0u32);

        let fake = Fake::new(&[
            ("git rev-parse --git-dir", ".git\n"),
            ("git rev-parse --short HEAD", "abc123\n"),
            ("git branch --show-current", "master\n"),
            (
                "git rev-parse --abbrev-ref --symbolic-full-name @{u}",
                "origin/master\n",
            ),
            ("git log --oneline HEAD..origin/master -10", ""),
            ("git status --short", ""),
        ]);

        let r = run_managed_update(ManagedUpdateOptions {
            repo_root: install_root,
            install_root,
            run_command: &|c, a, p| fake.run(c, a, p),
            run_setup: &|| setup_count.set(setup_count.get() + 1),
            // confirm NÃO deve ser chamado — se chamar, pânico.
            confirm: &|_| unreachable!("confirm should not be called for no-op updates"),
        })
        .unwrap();

        assert!(matches!(r.status, ManagedUpdateStatus::UpToDate));
        assert_eq!(setup_count.get(), 0);

        let cmds = fake.commands.borrow();
        assert!(!cmds
            .iter()
            .any(|(cmd, args)| cmd == "git" && args.first().map(|s| s.as_str()) == Some("reset")));
    }

    // -----------------------------------------------------------------------
    // find_repo_root / read_cargo_version tests
    // -----------------------------------------------------------------------

    fn write_agent_bar_manifest(dir: &Path, version: &str) {
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"agent-bar\"\nversion = \"{version}\"\nedition = \"2021\"\n\n[[bin]]\nname = \"agent-bar\"\npath = \"src/main.rs\"\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn find_repo_root_walks_up_from_nested_exe() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path();
        write_agent_bar_manifest(repo_root, "7.0.0");

        let exe = repo_root.join("target").join("release").join("agent-bar");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();

        assert_eq!(find_repo_root(&exe), Some(repo_root.to_path_buf()));
    }

    #[test]
    fn find_repo_root_none_without_git() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path();
        fs::write(
            repo_root.join("Cargo.toml"),
            "[package]\nname = \"agent-bar\"\nversion = \"7.0.0\"\n",
        )
        .unwrap();
        // Sem .git

        let exe = repo_root.join("agent-bar");
        assert_eq!(find_repo_root(&exe), None);
    }

    #[test]
    fn find_repo_root_none_when_name_differs() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path();
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::write(
            repo_root.join("Cargo.toml"),
            "[package]\nname = \"some-other-crate\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        let exe = repo_root.join("target").join("release").join("agent-bar");
        assert_eq!(find_repo_root(&exe), None);
    }

    #[test]
    fn find_repo_root_none_when_binary_outside_any_checkout() {
        let tmp = tempdir().unwrap();
        let bin_dir = tmp.path().join("home").join(".local").join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let exe = bin_dir.join("agent-bar");

        assert_eq!(find_repo_root(&exe), None);
    }

    #[test]
    fn read_cargo_version_parses_line() {
        let tmp = tempdir().unwrap();
        write_agent_bar_manifest(tmp.path(), "7.0.1");
        assert_eq!(read_cargo_version(tmp.path()).unwrap(), "7.0.1");
    }

    #[test]
    fn read_cargo_version_errors_when_missing() {
        let tmp = tempdir().unwrap();
        assert!(read_cargo_version(tmp.path()).is_err());
    }

    // -----------------------------------------------------------------------
    // detect_install_kind tests
    // -----------------------------------------------------------------------

    #[test]
    fn detect_kind_managed_git() {
        let tmp = tempdir().unwrap();
        let install_root = tmp.path();
        fs::create_dir_all(install_root.join(".git")).unwrap();

        assert_eq!(
            detect_install_kind(Some(install_root), install_root),
            InstallKind::ManagedGit
        );
    }

    #[test]
    fn detect_kind_dev_git() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path();
        fs::create_dir_all(repo_root.join(".git")).unwrap();

        assert_eq!(
            detect_install_kind(Some(repo_root), Path::new("/home/test/.agent-bar")),
            InstallKind::DevGit
        );
    }

    #[test]
    fn detect_kind_standalone_when_no_repo_root() {
        assert_eq!(
            detect_install_kind(None, Path::new("/home/test/.agent-bar")),
            InstallKind::Standalone
        );
    }

    #[test]
    #[serial_test::serial]
    fn detect_kind_system_via_env() {
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", Some("1"), || {
            assert_eq!(
                detect_install_kind(None, Path::new("/home/test/.agent-bar")),
                InstallKind::System
            );
        });
    }

    // -----------------------------------------------------------------------
    // Standalone self-update: lógica pura
    // -----------------------------------------------------------------------

    #[test]
    fn parse_release_tag_reads_field() {
        let body = r#"{"tag_name":"v7.0.1","name":"v7.0.1"}"#;
        assert_eq!(parse_release_tag(body).unwrap(), "v7.0.1");
    }

    #[test]
    fn parse_release_tag_errors_when_missing() {
        assert!(parse_release_tag(r#"{"name":"whatever"}"#).is_err());
    }

    #[test]
    fn parse_release_tag_errors_on_invalid_json() {
        assert!(parse_release_tag("not json").is_err());
    }

    #[test]
    fn version_bare_strips_v_prefix() {
        assert_eq!(version_bare("v7.0.1"), "7.0.1");
        assert_eq!(version_bare("7.0.1"), "7.0.1");
    }

    #[test]
    fn is_up_to_date_ignores_v_prefix() {
        assert!(is_up_to_date("7.0.1", "v7.0.1"));
        assert!(is_up_to_date("7.0.1", "7.0.1"));
        assert!(!is_up_to_date("7.0.0", "v7.0.1"));
    }

    #[test]
    fn asset_filename_matches_publish_pattern() {
        assert_eq!(asset_filename("7.0.1"), "agent-bar-7.0.1-x86_64.tar.gz");
    }

    #[test]
    #[serial_test::serial]
    fn default_data_dir_uses_env_override() {
        temp_env::with_var("AGENT_BAR_DATA", Some("/custom/data"), || {
            assert_eq!(
                default_data_dir(Path::new("/home/test")),
                PathBuf::from("/custom/data")
            );
        });
    }

    #[test]
    #[serial_test::serial]
    fn default_data_dir_falls_back_to_home() {
        // XDG_DATA_HOME explicitamente unset — a máquina de dev real quase
        // sempre tem esse env setado (gotcha do repo: XDG_* precisa ser
        // controlado explicitamente em teste, nunca assumido ausente).
        temp_env::with_vars(
            [
                ("AGENT_BAR_DATA", None::<&str>),
                ("XDG_DATA_HOME", None::<&str>),
            ],
            || {
                assert_eq!(
                    default_data_dir(Path::new("/home/test")),
                    PathBuf::from("/home/test/.local/share/agent-bar")
                );
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn default_data_dir_honors_xdg_data_home() {
        // Hotfix 7.0.1 (fix(paths)): antes `default_data_dir` ignorava
        // XDG_DATA_HOME por completo — split-brain com
        // `waybar_contract::standalone_data_asset_dir`, que já o respeitava.
        temp_env::with_vars(
            [
                ("AGENT_BAR_DATA", None::<&str>),
                ("XDG_DATA_HOME", Some("/xdg/data")),
            ],
            || {
                assert_eq!(
                    default_data_dir(Path::new("/home/test")),
                    PathBuf::from("/xdg/data/agent-bar")
                );
            },
        );
    }

    #[test]
    fn verify_checksum_runs_sha256sum_c() {
        let fake = Fake::new(&[("sha256sum -c agent-bar-7.0.1-x86_64.tar.gz.sha256", "OK\n")]);
        let tmp = tempdir().unwrap();
        verify_checksum(
            &|c, a, p| fake.run(c, a, p),
            tmp.path(),
            "agent-bar-7.0.1-x86_64.tar.gz",
        )
        .unwrap();
        assert_eq!(
            fake.commands.borrow()[0],
            (
                "sha256sum".to_string(),
                vec![
                    "-c".to_string(),
                    "agent-bar-7.0.1-x86_64.tar.gz.sha256".to_string()
                ]
            )
        );
    }

    #[test]
    fn verify_checksum_fails_on_mismatch() {
        struct FailFake;
        impl FailFake {
            fn run(&self, _c: &str, _a: &[String], _p: &Path) -> CommandResult {
                CommandResult {
                    ok: false,
                    output: "checksum mismatch".to_string(),
                }
            }
        }
        let fake = FailFake;
        let tmp = tempdir().unwrap();
        let err =
            verify_checksum(&|c, a, p| fake.run(c, a, p), tmp.path(), "asset.tar.gz").unwrap_err();
        assert!(err.to_string().contains("Verify checksum failed"));
    }

    #[test]
    fn extract_tarball_runs_tar_xzf() {
        let fake = Fake::new(&[("tar xzf agent-bar-7.0.1-x86_64.tar.gz", "")]);
        let tmp = tempdir().unwrap();
        extract_tarball(
            &|c, a, p| fake.run(c, a, p),
            tmp.path(),
            "agent-bar-7.0.1-x86_64.tar.gz",
        )
        .unwrap();
        assert_eq!(
            fake.commands.borrow()[0],
            (
                "tar".to_string(),
                vec![
                    "xzf".to_string(),
                    "agent-bar-7.0.1-x86_64.tar.gz".to_string()
                ]
            )
        );
    }

    #[test]
    fn replace_binary_atomic_swaps_and_sets_perms() {
        let tmp = tempdir().unwrap();
        let new_binary = tmp.path().join("agent-bar-new");
        fs::write(&new_binary, b"new-binary-contents").unwrap();

        let target_dir = tmp.path().join("bin");
        fs::create_dir_all(&target_dir).unwrap();
        let target_exe = target_dir.join("agent-bar");
        fs::write(&target_exe, b"old-binary-contents").unwrap();

        replace_binary_atomic(&new_binary, &target_exe).unwrap();

        assert_eq!(fs::read(&target_exe).unwrap(), b"new-binary-contents");
        let mode = fs::metadata(&target_exe).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
        assert!(!target_dir.join("agent-bar.new").exists());
    }

    #[test]
    fn install_standalone_assets_copies_icons_and_script() {
        let tmp = tempdir().unwrap();
        let extracted = tmp.path().join("extracted");
        fs::create_dir_all(extracted.join("icons")).unwrap();
        fs::write(extracted.join("icons").join("a.png"), b"icon").unwrap();
        fs::create_dir_all(extracted.join("scripts")).unwrap();
        fs::write(
            extracted.join("scripts").join(TERMINAL_HELPER_NAME),
            b"#!/usr/bin/env bash\n",
        )
        .unwrap();

        let data_dir = tmp.path().join("data");
        install_standalone_assets(&extracted, &data_dir).unwrap();

        assert!(data_dir.join("icons").join("a.png").exists());
        let script_dest = data_dir.join("scripts").join(TERMINAL_HELPER_NAME);
        assert!(script_dest.exists());
        let mode = fs::metadata(&script_dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[test]
    fn install_standalone_assets_ok_when_source_missing() {
        let tmp = tempdir().unwrap();
        let extracted = tmp.path().join("extracted-empty");
        fs::create_dir_all(&extracted).unwrap();
        let data_dir = tmp.path().join("data");

        // Sem icons/ nem scripts/ no extraído — não deve falhar, só cria os dirs.
        install_standalone_assets(&extracted, &data_dir).unwrap();
        assert!(data_dir.join("icons").is_dir());
        assert!(data_dir.join("scripts").is_dir());
    }

    // -----------------------------------------------------------------------
    // run_standalone_update: orquestração, rede mockada via wiremock (NUNCA
    // bate na API real do GitHub)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn run_standalone_update_reports_up_to_date() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"tag_name":"v7.0.0"}"#))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let fake = Fake::new(&[]);
        let tmp = tempdir().unwrap();

        let status = run_standalone_update(StandaloneUpdateOptions {
            current_version: "7.0.0",
            exe_path: &tmp.path().join("agent-bar"),
            data_dir: &tmp.path().join("data"),
            run_command: &|c, a, p| fake.run(c, a, p),
            http: &client,
            releases_api_url: format!("{}/releases/latest", server.uri()),
            download_base_url: format!("{}/download", server.uri()),
            has_sha256sum: &|| true,
            has_tar: &|| true,
        })
        .await
        .unwrap();

        assert!(
            matches!(status, StandaloneUpdateStatus::UpToDate { ref version } if version == "7.0.0")
        );
        // Não chegou perto de baixar/verificar nada.
        assert!(fake.commands.borrow().is_empty());
    }

    #[tokio::test]
    async fn run_standalone_update_fails_clearly_without_sha256sum() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"tag_name":"v7.0.1"}"#))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let fake = Fake::new(&[]);
        let tmp = tempdir().unwrap();

        let err = run_standalone_update(StandaloneUpdateOptions {
            current_version: "7.0.0",
            exe_path: &tmp.path().join("agent-bar"),
            data_dir: &tmp.path().join("data"),
            run_command: &|c, a, p| fake.run(c, a, p),
            http: &client,
            releases_api_url: format!("{}/releases/latest", server.uri()),
            download_base_url: format!("{}/download", server.uri()),
            has_sha256sum: &|| false,
            has_tar: &|| true,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("sha256sum not found"));
        // Nunca chegou a rodar nenhum comando (nem tentou baixar).
        assert!(fake.commands.borrow().is_empty());
    }
}
