//! Detecção de tipo de instalação e lógica de update. Port de `src/update.ts:113-286`.

use std::path::{Path, PathBuf};

use anyhow::bail;
use serde_json::Value;

use crate::runtime::is_system_install;

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

const DEPENDENCY_FILES: &[&str] = &["package.json", "bun.lock", "bun.lockb"];

// ---------------------------------------------------------------------------
// InstallKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstallKind {
    ManagedGit,
    DevGit,
    Npm,
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
// is_managed_install_root / detect_install_kind
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

/// Detecta o tipo de instalação em uso.
///
/// Ordem de precedência (port de `detectInstallKind` no TS):
/// 1. `is_system_install()` → `System`
/// 2. `.git` ausente em `repo_root` → `Npm`
/// 3. `is_managed_install_root` → `ManagedGit`, senão `DevGit`
pub fn detect_install_kind(repo_root: &Path, install_root: &Path) -> InstallKind {
    if is_system_install() {
        return InstallKind::System;
    }
    if !repo_root.join(".git").exists() {
        return InstallKind::Npm;
    }
    if is_managed_install_root(repo_root, install_root) {
        InstallKind::ManagedGit
    } else {
        InstallKind::DevGit
    }
}

// ---------------------------------------------------------------------------
// Helpers internos
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
    pub dependency_files_changed: bool,
    pub needs_dependency_install: bool,
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
    pub installed_dependencies: bool,
}

pub struct ManagedUpdateOptions<'a> {
    pub repo_root: &'a Path,
    pub install_root: &'a Path,
    pub run_command: &'a dyn Fn(&str, &[String], &Path) -> CommandResult,
    /// Side-effect de setup; testes contam invocações via `Cell<u32>`.
    pub run_setup: &'a dyn Fn(),
    pub confirm: &'a dyn Fn(&UpdateSummary) -> bool,
}

/// Port de `runManagedUpdate` (`src/update.ts:161-252`).
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
            installed_dependencies: false,
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

    let diff_output = require_command(
        opts.run_command,
        repo_root,
        "Check dependency changes",
        "git",
        &{
            let mut args = vec![
                "diff".to_string(),
                "--name-only".to_string(),
                "HEAD".to_string(),
                upstream.clone(),
                "--".to_string(),
            ];
            args.extend(DEPENDENCY_FILES.iter().map(|s| s.to_string()));
            args
        },
    )?;
    let dependency_files_changed = !diff_output.trim().is_empty();

    let needs_dependency_install =
        dependency_files_changed || !repo_root.join("node_modules").exists();

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
        dependency_files_changed,
        needs_dependency_install,
    };

    // Sem mudanças → up-to-date; NÃO chamar confirm.
    if !summary.has_updates && !summary.has_local_changes {
        return Ok(ManagedUpdateResult {
            status: ManagedUpdateStatus::UpToDate,
            repo_root: repo_root.to_path_buf(),
            install_root: install_root.to_path_buf(),
            summary: Some(summary),
            installed_dependencies: false,
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
            installed_dependencies: false,
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

    let mut installed_dependencies = false;
    if needs_dependency_install {
        require_command(
            opts.run_command,
            repo_root,
            "Install dependencies",
            "bun",
            &["install".to_string()],
        )?;
        installed_dependencies = true;
    }

    (opts.run_setup)();

    Ok(ManagedUpdateResult {
        status: ManagedUpdateStatus::Updated,
        repo_root: repo_root.to_path_buf(),
        install_root: install_root.to_path_buf(),
        summary: Some(summary),
        installed_dependencies,
    })
}

// ---------------------------------------------------------------------------
// NpmUpdate*
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NpmUpdateSummary {
    pub package_name: String,
    pub current_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NpmUpdateStatus {
    Cancelled,
    Updated,
}

#[derive(Debug, Clone)]
pub struct NpmUpdateResult {
    pub status: NpmUpdateStatus,
    pub summary: NpmUpdateSummary,
}

pub struct NpmUpdateOptions<'a> {
    pub repo_root: &'a Path,
    pub run_command: &'a dyn Fn(&str, &[String], &Path) -> CommandResult,
    pub run_setup: &'a dyn Fn(),
    pub confirm_npm: &'a dyn Fn(&NpmUpdateSummary) -> bool,
}

/// Lê `name` e `version` de `package.json` em `repo_root`.
fn read_package_info(repo_root: &Path) -> anyhow::Result<NpmUpdateSummary> {
    let pkg_path = repo_root.join("package.json");
    let content = std::fs::read_to_string(&pkg_path)
        .map_err(|e| anyhow::anyhow!("Failed to read package.json: {e}"))?;
    let v: Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse package.json: {e}"))?;

    let name = v
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty());
    let version = v
        .get("version")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty());

    match (name, version) {
        (Some(n), Some(ver)) => Ok(NpmUpdateSummary {
            package_name: n.to_string(),
            current_version: ver.to_string(),
        }),
        _ => bail!("package.json is missing name or version"),
    }
}

/// Port de `runNpmUpdate` (`src/update.ts:265-286`).
pub fn run_npm_update(opts: NpmUpdateOptions<'_>) -> anyhow::Result<NpmUpdateResult> {
    let summary = read_package_info(opts.repo_root)?;

    let approved = (opts.confirm_npm)(&summary);
    if !approved {
        return Ok(NpmUpdateResult {
            status: NpmUpdateStatus::Cancelled,
            summary,
        });
    }

    require_command(
        opts.run_command,
        opts.repo_root,
        "Update package",
        "bun",
        &[
            "add".to_string(),
            "-g".to_string(),
            summary.package_name.clone(),
        ],
    )?;

    (opts.run_setup)();

    Ok(NpmUpdateResult {
        status: NpmUpdateStatus::Updated,
        summary,
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
    fn managed_discards_and_resets_installs_deps_runs_setup() {
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
            (
                "git diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb",
                "package.json\n",
            ),
        ]);

        let r = run_managed_update(ManagedUpdateOptions {
            repo_root: install_root,
            install_root,
            run_command: &|c, a, p| fake.run(c, a, p),
            run_setup: &|| setup_count.set(setup_count.get() + 1),
            confirm: &|summary| {
                assert!(summary.has_local_changes);
                assert!(summary.has_updates);
                assert!(summary.dependency_files_changed);
                true
            },
        })
        .unwrap();

        assert!(matches!(r.status, ManagedUpdateStatus::Updated));
        assert!(r.installed_dependencies);
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
                    "diff".to_string(),
                    "--name-only".to_string(),
                    "HEAD".to_string(),
                    "origin/master".to_string(),
                    "--".to_string(),
                    "package.json".to_string(),
                    "bun.lock".to_string(),
                    "bun.lockb".to_string(),
                ],
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
            ("bun".to_string(), vec!["install".to_string()]),
        ];
        assert_eq!(cmds, expected);
    }

    #[test]
    fn managed_skips_bun_install_when_deps_unchanged_and_node_modules_exists() {
        let tmp = tempdir().unwrap();
        let install_root = tmp.path();
        fs::create_dir_all(install_root.join("node_modules")).unwrap();
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
                "def456 update docs\n",
            ),
            ("git status --short", ""),
            (
                "git diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb",
                "",
            ),
        ]);

        let r = run_managed_update(ManagedUpdateOptions {
            repo_root: install_root,
            install_root,
            run_command: &|c, a, p| fake.run(c, a, p),
            run_setup: &|| setup_count.set(setup_count.get() + 1),
            confirm: &|_| true,
        })
        .unwrap();

        assert!(matches!(r.status, ManagedUpdateStatus::Updated));
        assert!(!r.installed_dependencies);
        assert_eq!(setup_count.get(), 1);

        let cmds = fake.commands.borrow();
        assert!(
            !cmds
                .iter()
                .any(|(cmd, args)| cmd == "bun"
                    && args.first().map(|s| s.as_str()) == Some("install"))
        );
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
            (
                "git diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb",
                "",
            ),
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
        fs::create_dir_all(install_root.join("node_modules")).unwrap();
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
            (
                "git diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb",
                "",
            ),
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
        assert!(
            !cmds
                .iter()
                .any(|(cmd, args)| cmd == "bun"
                    && args.first().map(|s| s.as_str()) == Some("install"))
        );
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
            detect_install_kind(install_root, install_root),
            InstallKind::ManagedGit
        );
    }

    #[test]
    fn detect_kind_dev_git() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path();
        fs::create_dir_all(repo_root.join(".git")).unwrap();

        assert_eq!(
            detect_install_kind(repo_root, Path::new("/home/test/.agent-bar")),
            InstallKind::DevGit
        );
    }

    #[test]
    fn detect_kind_npm() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path();
        // Sem .git

        assert_eq!(
            detect_install_kind(repo_root, Path::new("/home/test/.agent-bar")),
            InstallKind::Npm
        );
    }

    #[test]
    #[serial_test::serial]
    fn detect_kind_system_via_env() {
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", Some("1"), || {
            assert_eq!(
                detect_install_kind(Path::new("/whatever"), Path::new("/home/test/.agent-bar")),
                InstallKind::System
            );
        });
    }

    // -----------------------------------------------------------------------
    // runNpmUpdate tests
    // -----------------------------------------------------------------------

    #[test]
    fn npm_update_runs_bun_add_and_setup() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path();
        fs::write(
            repo_root.join("package.json"),
            r#"{"name":"@noctuacore/agent-bar","version":"4.0.1"}"#,
        )
        .unwrap();
        let setup_count = Cell::new(0u32);
        let fake = Fake::new(&[]);
        let captured_name: RefCell<String> = RefCell::new(String::new());
        let captured_version: RefCell<String> = RefCell::new(String::new());

        let r = run_npm_update(NpmUpdateOptions {
            repo_root,
            run_command: &|c, a, p| fake.run(c, a, p),
            run_setup: &|| setup_count.set(setup_count.get() + 1),
            confirm_npm: &|summary| {
                *captured_name.borrow_mut() = summary.package_name.clone();
                *captured_version.borrow_mut() = summary.current_version.clone();
                true
            },
        })
        .unwrap();

        assert!(matches!(r.status, NpmUpdateStatus::Updated));
        assert_eq!(setup_count.get(), 1);
        assert_eq!(*captured_name.borrow(), "@noctuacore/agent-bar");
        assert_eq!(*captured_version.borrow(), "4.0.1");

        let cmds: Vec<(String, Vec<String>)> = fake.commands.borrow().clone();
        assert_eq!(
            cmds,
            vec![(
                "bun".to_string(),
                vec![
                    "add".to_string(),
                    "-g".to_string(),
                    "@noctuacore/agent-bar".to_string()
                ]
            )]
        );
    }

    #[test]
    fn npm_update_cancelled_no_bun_add_no_setup() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path();
        fs::write(
            repo_root.join("package.json"),
            r#"{"name":"@noctuacore/agent-bar","version":"4.0.1"}"#,
        )
        .unwrap();
        let setup_count = Cell::new(0u32);
        let fake = Fake::new(&[]);

        let r = run_npm_update(NpmUpdateOptions {
            repo_root,
            run_command: &|c, a, p| fake.run(c, a, p),
            run_setup: &|| setup_count.set(setup_count.get() + 1),
            confirm_npm: &|_| false,
        })
        .unwrap();

        assert!(matches!(r.status, NpmUpdateStatus::Cancelled));
        assert_eq!(setup_count.get(), 0);
        assert!(fake.commands.borrow().is_empty());
    }

    // -----------------------------------------------------------------------
    // Testes adicionais de cobertura
    // -----------------------------------------------------------------------

    #[test]
    fn is_managed_install_root_same_path() {
        assert!(is_managed_install_root(
            Path::new("/home/user/.agent-bar"),
            Path::new("/home/user/.agent-bar")
        ));
    }

    #[test]
    fn is_managed_install_root_different_paths() {
        assert!(!is_managed_install_root(
            Path::new("/tmp/dev/agent-bar"),
            Path::new("/home/user/.agent-bar")
        ));
    }
}
