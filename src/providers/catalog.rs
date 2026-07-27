//! Locked v10 provider catalog and executable discovery.
//!
//! Descriptors hold metadata only. Collection behavior lives in adapters.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::ProviderId;

/// Re-export closed provider IDs as part of the catalog surface.
pub use crate::cli::ProviderId as CatalogProviderId;

const ONE_MIB: usize = 1024 * 1024;
const RETRY_DELAY: Duration = Duration::from_millis(250);

/// Root for a typed executable path template (never shell-expanded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRoot {
    /// `$HOME`
    Home,
    /// `$GROK_HOME` when set and valid, otherwise `$HOME/.grok`
    GrokHome,
}

/// Typed path template: root + path segments under that root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutablePath {
    pub root: PathRoot,
    pub segments: &'static [&'static str],
}

/// Retry policy attached to a provider descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicy {
    /// No automatic retry.
    None,
    /// One additional attempt after [`RETRY_DELAY`] for classified transients.
    OneTransient,
}

/// Sole catalog entry for a provider (stable product metadata).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub display_name: &'static str,
    pub icon_key: &'static str,
    pub executable_name: &'static str,
    pub fallback_executable_paths: &'static [ExecutablePath],
    /// Allowlisted official documentation HTTPS URL for `view_installation`.
    pub installation_url: &'static str,
    pub login_argv: &'static [&'static str],
    pub cache_ttl: Duration,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub retry_policy: RetryPolicy,
}

impl ProviderDescriptor {
    pub fn retry_delay(&self) -> Option<Duration> {
        match self.retry_policy {
            RetryPolicy::None => None,
            RetryPolicy::OneTransient => Some(RETRY_DELAY),
        }
    }
}

/// Whether a required collection executable was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionAvailability {
    Available { executable: PathBuf },
    Missing,
}

/// Whether the official login executable was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginAvailability {
    Available { executable: PathBuf },
    Missing,
}

/// Independent collection and login discovery outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub collection: CollectionAvailability,
    pub login: LoginAvailability,
}

impl Discovery {
    pub fn collection_executable(&self) -> Option<&Path> {
        match &self.collection {
            CollectionAvailability::Available { executable } => Some(executable.as_path()),
            CollectionAvailability::Missing => None,
        }
    }

    pub fn login_executable(&self) -> Option<&Path> {
        match &self.login {
            LoginAvailability::Available { executable } => Some(executable.as_path()),
            LoginAvailability::Missing => None,
        }
    }
}

/// Injected environment for discovery (never uses process CWD as HOME).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionEnvironment {
    pub home: PathBuf,
    /// Directories from `PATH`, in order.
    pub path_dirs: Vec<PathBuf>,
    /// `None` means unset (default `$HOME/.grok`). `Some` must be absolute nonempty.
    pub grok_home: Option<PathBuf>,
}

impl ExecutionEnvironment {
    /// Build from process environment. Invalid `GROK_HOME` is retained as-is so
    /// callers can surface a typed configuration error; discovery treats invalid
    /// set values as missing Grok fallbacks under that root.
    pub fn from_process() -> Self {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        let path_dirs = env::var_os("PATH")
            .map(|p| env::split_paths(&p).collect())
            .unwrap_or_default();
        let grok_home = env::var_os("GROK_HOME").map(PathBuf::from);
        Self {
            home,
            path_dirs,
            grok_home,
        }
    }

    pub fn resolve_grok_home(&self) -> Result<PathBuf, CatalogError> {
        match &self.grok_home {
            None => Ok(self.home.join(".grok")),
            Some(value) => {
                if value.as_os_str().is_empty() || !value.is_absolute() {
                    return Err(CatalogError::InvalidGrokHome);
                }
                Ok(value.clone())
            }
        }
    }
}

/// Catalog / discovery error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    InvalidGrokHome,
    UnknownProvider,
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGrokHome => {
                write!(f, "GROK_HOME must be a nonempty absolute path when set")
            }
            Self::UnknownProvider => write!(f, "unknown provider"),
        }
    }
}

impl std::error::Error for CatalogError {}

/// Locked catalog order: Claude, Codex, Amp, Grok.
pub static PROVIDERS: &[&ProviderDescriptor] = &[&CLAUDE, &CODEX, &AMP, &GROK];

pub static CLAUDE: ProviderDescriptor = ProviderDescriptor {
    id: ProviderId::Claude,
    display_name: "Claude",
    icon_key: "claude",
    executable_name: "claude",
    fallback_executable_paths: &[ExecutablePath {
        root: PathRoot::Home,
        segments: &[".local", "bin", "claude"],
    }],
    installation_url: "https://code.claude.com/docs/en/getting-started",
    login_argv: &["claude", "auth", "login"],
    cache_ttl: Duration::from_secs(300),
    timeout: Duration::from_secs(10),
    max_output_bytes: ONE_MIB,
    retry_policy: RetryPolicy::OneTransient,
};

pub static CODEX: ProviderDescriptor = ProviderDescriptor {
    id: ProviderId::Codex,
    display_name: "Codex",
    icon_key: "codex",
    executable_name: "codex",
    fallback_executable_paths: &[ExecutablePath {
        root: PathRoot::Home,
        segments: &[".local", "bin", "codex"],
    }],
    installation_url: "https://github.com/openai/codex",
    login_argv: &["codex", "login"],
    cache_ttl: Duration::from_secs(90),
    timeout: Duration::from_secs(10),
    max_output_bytes: ONE_MIB,
    retry_policy: RetryPolicy::OneTransient,
};

pub static AMP: ProviderDescriptor = ProviderDescriptor {
    id: ProviderId::Amp,
    display_name: "Amp",
    icon_key: "amp",
    executable_name: "amp",
    fallback_executable_paths: &[
        ExecutablePath {
            root: PathRoot::Home,
            segments: &[".local", "bin", "amp"],
        },
        ExecutablePath {
            root: PathRoot::Home,
            segments: &[".amp", "bin", "amp"],
        },
        ExecutablePath {
            root: PathRoot::Home,
            segments: &[".cache", ".bun", "bin", "amp"],
        },
        ExecutablePath {
            root: PathRoot::Home,
            segments: &[".bun", "bin", "amp"],
        },
    ],
    installation_url: "https://ampcode.com/manual",
    login_argv: &["amp", "login"],
    cache_ttl: Duration::from_secs(90),
    timeout: Duration::from_secs(10),
    max_output_bytes: ONE_MIB,
    retry_policy: RetryPolicy::OneTransient,
};

pub static GROK: ProviderDescriptor = ProviderDescriptor {
    id: ProviderId::Grok,
    display_name: "Grok",
    icon_key: "grok",
    executable_name: "grok",
    fallback_executable_paths: &[
        ExecutablePath {
            root: PathRoot::GrokHome,
            segments: &["bin", "grok"],
        },
        ExecutablePath {
            root: PathRoot::Home,
            segments: &[".grok", "bin", "grok"],
        },
        ExecutablePath {
            root: PathRoot::Home,
            segments: &[".local", "bin", "grok"],
        },
    ],
    installation_url: "https://x.ai/cli",
    login_argv: &["grok", "login"],
    cache_ttl: Duration::from_secs(90),
    timeout: Duration::from_secs(10),
    max_output_bytes: ONE_MIB,
    retry_policy: RetryPolicy::OneTransient,
};

/// Look up a descriptor by closed provider id.
pub fn descriptor(id: ProviderId) -> &'static ProviderDescriptor {
    match id {
        ProviderId::Claude => &CLAUDE,
        ProviderId::Codex => &CODEX,
        ProviderId::Amp => &AMP,
        ProviderId::Grok => &GROK,
    }
}

/// Discover collection and login executables independently.
///
/// Both currently share the same executable search (PATH then fallbacks).
/// Collection availability is not inferred from login success semantics;
/// adapters decide how missing collection sources map to domain states.
pub fn discover(
    descriptor: &ProviderDescriptor,
    env: &ExecutionEnvironment,
) -> Result<Discovery, CatalogError> {
    let resolved = resolve_executable(descriptor, env)?;
    let collection = match &resolved {
        Some(path) => CollectionAvailability::Available {
            executable: path.clone(),
        },
        None => CollectionAvailability::Missing,
    };
    let login = match resolved {
        Some(path) => LoginAvailability::Available { executable: path },
        None => LoginAvailability::Missing,
    };
    Ok(Discovery { collection, login })
}

/// Build login argv with the resolved absolute executable as element zero.
pub fn login_process_argv(
    descriptor: &ProviderDescriptor,
    discovery: &Discovery,
) -> Option<Vec<String>> {
    let exe = discovery.login_executable()?;
    let mut argv: Vec<String> = descriptor
        .login_argv
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    if argv.is_empty() {
        return None;
    }
    argv[0] = exe.to_string_lossy().into_owned();
    Some(argv)
}

fn resolve_executable(
    descriptor: &ProviderDescriptor,
    env: &ExecutionEnvironment,
) -> Result<Option<PathBuf>, CatalogError> {
    for dir in &env.path_dirs {
        let candidate = dir.join(descriptor.executable_name);
        if is_executable_file(&candidate) {
            return Ok(Some(canonicalize_best_effort(&candidate)));
        }
    }
    for template in descriptor.fallback_executable_paths {
        let candidate = expand_template(template, env)?;
        if is_executable_file(&candidate) {
            return Ok(Some(canonicalize_best_effort(&candidate)));
        }
    }
    Ok(None)
}

fn expand_template(
    template: &ExecutablePath,
    env: &ExecutionEnvironment,
) -> Result<PathBuf, CatalogError> {
    let mut path = match template.root {
        PathRoot::Home => env.home.clone(),
        PathRoot::GrokHome => env.resolve_grok_home()?,
    };
    for segment in template.segments {
        path.push(segment);
    }
    Ok(path)
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::OpenOptionsExt;

    fn write_exec(path: &Path, executable: bool) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        if executable {
            opts.mode(0o755);
        } else {
            opts.mode(0o644);
        }
        opts.open(path).unwrap();
    }

    #[test]
    fn catalog_order_and_literal_table() {
        let ids: Vec<_> = PROVIDERS.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec![
                ProviderId::Claude,
                ProviderId::Codex,
                ProviderId::Amp,
                ProviderId::Grok
            ]
        );

        assert_eq!(CLAUDE.display_name, "Claude");
        assert_eq!(CLAUDE.icon_key, "claude");
        assert_eq!(CLAUDE.executable_name, "claude");
        assert_eq!(CLAUDE.login_argv, &["claude", "auth", "login"]);
        assert_eq!(
            CLAUDE.installation_url,
            "https://code.claude.com/docs/en/getting-started"
        );
        assert!(CLAUDE.installation_url.starts_with("https://"));
        assert_eq!(CLAUDE.cache_ttl, Duration::from_secs(300));
        assert_eq!(CLAUDE.timeout, Duration::from_secs(10));
        assert_eq!(CLAUDE.max_output_bytes, ONE_MIB);
        assert_eq!(CLAUDE.retry_policy, RetryPolicy::OneTransient);
        assert_eq!(
            CLAUDE.fallback_executable_paths,
            &[ExecutablePath {
                root: PathRoot::Home,
                segments: &[".local", "bin", "claude"],
            }]
        );

        assert_eq!(CODEX.display_name, "Codex");
        assert_eq!(CODEX.icon_key, "codex");
        assert_eq!(CODEX.login_argv, &["codex", "login"]);
        assert_eq!(CODEX.installation_url, "https://github.com/openai/codex");
        assert_eq!(CODEX.cache_ttl, Duration::from_secs(90));

        assert_eq!(AMP.display_name, "Amp");
        assert_eq!(AMP.icon_key, "amp");
        assert_eq!(AMP.login_argv, &["amp", "login"]);
        assert_eq!(AMP.installation_url, "https://ampcode.com/manual");
        assert_eq!(AMP.fallback_executable_paths.len(), 4);

        assert_eq!(GROK.display_name, "Grok");
        assert_eq!(GROK.icon_key, "grok");
        assert_eq!(GROK.login_argv, &["grok", "login"]);
        assert_eq!(GROK.installation_url, "https://x.ai/cli");
        assert_eq!(GROK.retry_policy, RetryPolicy::OneTransient);
        assert_eq!(GROK.fallback_executable_paths[0].root, PathRoot::GrokHome);
    }

    #[test]
    fn installation_urls_are_docs_not_installer_scripts() {
        for provider in PROVIDERS {
            assert!(
                provider.installation_url.starts_with("https://"),
                "{}",
                provider.id
            );
            assert!(
                !provider.installation_url.contains("install.sh"),
                "{}",
                provider.id
            );
            assert!(
                !provider.installation_url.contains("curl "),
                "{}",
                provider.id
            );
        }
    }

    #[test]
    fn rejects_non_executable_files() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let path_dir = dir.path().join("path");
        let bin = path_dir.join("claude");
        write_exec(&bin, false);
        let env = ExecutionEnvironment {
            home,
            path_dirs: vec![path_dir],
            grok_home: None,
        };
        let discovery = discover(&CLAUDE, &env).unwrap();
        assert_eq!(discovery.collection, CollectionAvailability::Missing);
        assert_eq!(discovery.login, LoginAvailability::Missing);
    }

    #[test]
    fn path_wins_over_fallback_and_preserves_login_argv_tail() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let path_dir = dir.path().join("path");
        let path_bin = path_dir.join("claude");
        let fallback = home.join(".local/bin/claude");
        write_exec(&path_bin, true);
        write_exec(&fallback, true);
        let env = ExecutionEnvironment {
            home,
            path_dirs: vec![path_dir],
            grok_home: None,
        };
        let discovery = discover(&CLAUDE, &env).unwrap();
        let exe = discovery.login_executable().unwrap();
        assert!(exe.ends_with("claude"));
        let argv = login_process_argv(&CLAUDE, &discovery).unwrap();
        assert_eq!(argv[0], exe.to_string_lossy());
        assert_eq!(argv[1], "auth");
        assert_eq!(argv[2], "login");
    }

    #[test]
    fn fallback_used_when_path_empty() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let fallback = home.join(".local/bin/codex");
        write_exec(&fallback, true);
        let env = ExecutionEnvironment {
            home: home.clone(),
            path_dirs: vec![dir.path().join("empty-path")],
            grok_home: None,
        };
        let discovery = discover(&CODEX, &env).unwrap();
        let exe = discovery.collection_executable().unwrap();
        assert_eq!(
            fs::canonicalize(exe).unwrap(),
            fs::canonicalize(&fallback).unwrap()
        );
    }

    #[test]
    fn collection_and_login_reported_independently_as_enums() {
        let dir = tempfile::tempdir().unwrap();
        let env = ExecutionEnvironment {
            home: dir.path().join("home"),
            path_dirs: vec![],
            grok_home: None,
        };
        let discovery = discover(&AMP, &env).unwrap();
        assert!(matches!(
            discovery.collection,
            CollectionAvailability::Missing
        ));
        assert!(matches!(discovery.login, LoginAvailability::Missing));
    }

    #[test]
    fn grok_home_must_be_absolute_when_set() {
        let env = ExecutionEnvironment {
            home: PathBuf::from("/home/user"),
            path_dirs: vec![],
            grok_home: Some(PathBuf::from("relative")),
        };
        assert_eq!(
            env.resolve_grok_home().unwrap_err(),
            CatalogError::InvalidGrokHome
        );
        let env = ExecutionEnvironment {
            home: PathBuf::from("/home/user"),
            path_dirs: vec![],
            grok_home: None,
        };
        assert_eq!(
            env.resolve_grok_home().unwrap(),
            PathBuf::from("/home/user/.grok")
        );
    }
}
