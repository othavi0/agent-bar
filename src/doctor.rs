use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::app_identity::{APP_NAME, OMARCHY_PLUGIN_ID, VERSION};
use crate::omarchy_integration::{
    default_omarchy_plugins_dir, default_omarchy_shell_json_path, shell_json_has_plugin_entry,
};

const TARGET_PACKAGE: &str = "@noctuacore/agent-bar";
const LOCKFILE_NAMES: &[&str] = &["bun.lock", "bun.lockb", "package-lock.json"];

#[derive(Debug, Clone)]
pub struct DoctorFindings {
    pub package_json_path: Option<PathBuf>,
    pub package_json_orphan: bool,
    pub package_json_mixed: bool,
    pub node_modules_dir: Option<PathBuf>,
    pub lockfiles: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DoctorStatus {
    Clean,
    Cancelled,
    Cleaned,
    MixedOnly,
}

#[derive(Debug, Clone)]
pub struct DoctorResult {
    pub status: DoctorStatus,
    pub removed: Vec<PathBuf>,
    pub findings: DoctorFindings,
    pub omarchy: OmarchyFindings,
}

pub struct DoctorOptions<'a> {
    pub home: &'a Path,
    pub dry_run: bool,
    pub yes: bool,
    pub confirm: &'a dyn Fn(&DoctorFindings) -> bool,
}

fn read_json(path: &Path) -> Option<Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&content).ok()
}

fn classify_package_json(pkg: Option<&Value>) -> (bool, bool) {
    let pkg = match pkg {
        Some(p) => p,
        None => return (false, false),
    };

    let empty = serde_json::Map::new();
    let deps = pkg
        .get("dependencies")
        .and_then(|v| v.as_object())
        .unwrap_or(&empty);
    let dev_deps = pkg
        .get("devDependencies")
        .and_then(|v| v.as_object())
        .unwrap_or(&empty);

    let mut names: Vec<&str> = deps
        .keys()
        .chain(dev_deps.keys())
        .map(|s| s.as_str())
        .collect();

    names.sort_unstable();
    names.dedup();

    if !names.contains(&TARGET_PACKAGE) {
        return (false, false);
    }

    if names.len() == 1 {
        (true, false)
    } else {
        (false, true)
    }
}

fn find_node_modules_dir(home: &Path) -> Option<PathBuf> {
    let dir = home
        .join("node_modules")
        .join("@noctuacore")
        .join("agent-bar");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Classification {
    Orphan,
    Mixed,
    Legit,
    None,
}

fn find_lockfiles(home: &Path, classification: Classification) -> Vec<PathBuf> {
    if classification == Classification::Mixed || classification == Classification::Legit {
        return vec![];
    }
    LOCKFILE_NAMES
        .iter()
        .map(|name| home.join(name))
        .filter(|p| p.exists())
        .collect()
}

pub fn scan(home: &Path) -> DoctorFindings {
    let package_json_path = home.join("package.json");
    let pkg_exists = package_json_path.exists();
    let pkg = if pkg_exists {
        read_json(&package_json_path)
    } else {
        Option::None
    };

    let (orphan, mixed) = classify_package_json(pkg.as_ref());

    let classification = if orphan {
        Classification::Orphan
    } else if mixed {
        Classification::Mixed
    } else if pkg_exists {
        Classification::Legit
    } else {
        Classification::None
    };

    DoctorFindings {
        // package_json_path = Some only when pkg parsed successfully (pkg != null in TS)
        package_json_path: if pkg.is_some() {
            Some(package_json_path)
        } else {
            Option::None
        },
        package_json_orphan: orphan,
        package_json_mixed: mixed,
        node_modules_dir: find_node_modules_dir(home),
        lockfiles: find_lockfiles(home, classification),
    }
}

/// Achados Omarchy do `doctor`: drift binário↔plugin e referências
/// penduradas em `shell.json`. Leitura pura — NUNCA escreve nada, NUNCA
/// falha o comando (viram avisos, spec 2026-07-21 §F).
#[derive(Debug, Clone, Default)]
pub struct OmarchyFindings {
    /// `Some` quando o manifest instalado tem `version` diferente do binário.
    pub manifest_version_mismatch: Option<String>,
    /// `Some` quando o diretório do plugin existe mas `shell.json` não
    /// referencia `agent-bar.usage`.
    pub plugin_dir_without_shell_entry: Option<String>,
    /// `Some` quando `shell.json` referencia `agent-bar.usage` mas o
    /// diretório do plugin não existe.
    pub shell_entry_without_plugin_dir: Option<String>,
}

impl OmarchyFindings {
    /// Achados como mensagens prontas p/ `term_prompt::status("Aviso", ...)`.
    pub fn warnings(&self) -> Vec<String> {
        [
            &self.manifest_version_mismatch,
            &self.plugin_dir_without_shell_entry,
            &self.shell_entry_without_plugin_dir,
        ]
        .into_iter()
        .filter_map(|w| w.clone())
        .collect()
    }
}

pub fn scan_omarchy(home: &Path) -> OmarchyFindings {
    let plugin_dir = default_omarchy_plugins_dir(home).join(OMARCHY_PLUGIN_ID);
    let dir_exists = plugin_dir.is_dir();

    let manifest_version_mismatch = if dir_exists {
        read_json(&plugin_dir.join("manifest.json"))
            .and_then(|v| v.get("version").and_then(|v| v.as_str()).map(str::to_string))
            .filter(|installed| installed != VERSION)
            .map(|installed| {
                format!(
                    "Plugin omarchy instalado (v{installed}) diverge do binário (v{VERSION}). Rode `{APP_NAME} setup` para atualizá-lo."
                )
            })
    } else {
        None
    };

    let shell_json_path = default_omarchy_shell_json_path(home);
    let shell_has_entry = shell_json_has_plugin_entry(&shell_json_path);

    let plugin_dir_without_shell_entry = (dir_exists && !shell_has_entry).then(|| {
        format!(
            "Diretório do plugin existe ({}) mas {} não referencia `{OMARCHY_PLUGIN_ID}`. Rode `omarchy bar plugin add {OMARCHY_PLUGIN_ID}`.",
            plugin_dir.display(),
            shell_json_path.display()
        )
    });

    let shell_entry_without_plugin_dir = (!dir_exists && shell_has_entry).then(|| {
        format!(
            "{} referencia `{OMARCHY_PLUGIN_ID}` mas o diretório do plugin não existe ({}). Rode `{APP_NAME} setup`.",
            shell_json_path.display(),
            plugin_dir.display()
        )
    });

    OmarchyFindings {
        manifest_version_mismatch,
        plugin_dir_without_shell_entry,
        shell_entry_without_plugin_dir,
    }
}

fn planned_removals(findings: &DoctorFindings) -> Vec<PathBuf> {
    let mut items = Vec::new();
    if findings.package_json_orphan {
        if let Some(ref p) = findings.package_json_path {
            items.push(p.clone());
        }
    }
    if let Some(ref nm) = findings.node_modules_dir {
        items.push(nm.clone());
    }
    if !findings.package_json_mixed {
        items.extend(findings.lockfiles.iter().cloned());
    }
    items
}

pub fn run_doctor(opts: DoctorOptions<'_>) -> DoctorResult {
    let findings = scan(opts.home);
    let omarchy = scan_omarchy(opts.home);

    let nothing_to_do = !findings.package_json_orphan
        && !findings.package_json_mixed
        && findings.node_modules_dir.is_none()
        && findings.lockfiles.is_empty();

    if nothing_to_do {
        return DoctorResult {
            status: DoctorStatus::Clean,
            removed: vec![],
            findings,
            omarchy,
        };
    }

    let approved = opts.yes || (opts.confirm)(&findings);
    if !approved {
        return DoctorResult {
            status: DoctorStatus::Cancelled,
            removed: vec![],
            findings,
            omarchy,
        };
    }

    let removals = planned_removals(&findings);

    if !opts.dry_run {
        for path in &removals {
            if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }
    }

    let status = if findings.package_json_mixed && !findings.package_json_orphan {
        DoctorStatus::MixedOnly
    } else {
        DoctorStatus::Cleaned
    };

    DoctorResult {
        status,
        removed: removals,
        findings,
        omarchy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use tempfile::tempdir;

    #[test]
    fn scan_clean_when_nothing_relevant() {
        let h = tempdir().unwrap();
        let f = scan(h.path());
        assert!(!f.package_json_orphan && !f.package_json_mixed);
        assert!(f.node_modules_dir.is_none());
        assert!(f.lockfiles.is_empty());
    }

    #[test]
    fn scan_detects_orphan_package_json() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#,
        )
        .unwrap();
        let f = scan(h.path());
        assert!(f.package_json_orphan && !f.package_json_mixed);
    }

    #[test]
    fn scan_flags_mixed_package_json() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0","other":"1.0.0"}}"#,
        )
        .unwrap();
        let f = scan(h.path());
        assert!(!f.package_json_orphan && f.package_json_mixed);
    }

    #[test]
    fn scan_ignores_unrelated_package_json() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"other":"1.0.0"}}"#,
        )
        .unwrap();
        let f = scan(h.path());
        assert!(!f.package_json_orphan && !f.package_json_mixed);
    }

    #[test]
    fn scan_detects_node_modules() {
        let h = tempdir().unwrap();
        std::fs::create_dir_all(
            h.path()
                .join("node_modules")
                .join("@noctuacore")
                .join("agent-bar"),
        )
        .unwrap();
        let f = scan(h.path());
        assert_eq!(
            f.node_modules_dir,
            Some(
                h.path()
                    .join("node_modules")
                    .join("@noctuacore")
                    .join("agent-bar")
            )
        );
    }

    #[test]
    fn scan_lockfiles_only_when_orphan_or_missing() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("bun.lock"), "").unwrap();
        std::fs::write(h.path().join("package-lock.json"), "{}").unwrap();
        let f = scan(h.path());
        assert_eq!(
            f.lockfiles,
            vec![
                h.path().join("bun.lock"),
                h.path().join("package-lock.json")
            ]
        );
        // Add a legit package.json — lockfiles should disappear
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"other":"1.0.0"}}"#,
        )
        .unwrap();
        assert!(scan(h.path()).lockfiles.is_empty());
    }

    #[test]
    fn scan_considers_dev_dependencies() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"devDependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#,
        )
        .unwrap();
        assert!(scan(h.path()).package_json_orphan);
    }

    fn with_clean_xdg_config_home<T>(f: impl FnOnce() -> T) -> T {
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = f();
        match prev {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        result
    }

    #[test]
    #[serial_test::serial]
    fn scan_omarchy_clean_when_nothing_installed() {
        with_clean_xdg_config_home(|| {
            let h = tempdir().unwrap();
            let f = scan_omarchy(h.path());
            assert!(f.manifest_version_mismatch.is_none());
            assert!(f.plugin_dir_without_shell_entry.is_none());
            assert!(f.shell_entry_without_plugin_dir.is_none());
            assert!(f.warnings().is_empty());
        });
    }

    #[test]
    #[serial_test::serial]
    fn scan_omarchy_flags_manifest_version_mismatch() {
        with_clean_xdg_config_home(|| {
            let h = tempdir().unwrap();
            let plugin_dir = h
                .path()
                .join(".config")
                .join("omarchy")
                .join("plugins")
                .join(crate::app_identity::OMARCHY_PLUGIN_ID);
            std::fs::create_dir_all(&plugin_dir).unwrap();
            std::fs::write(
                plugin_dir.join("manifest.json"),
                r#"{"id":"agent-bar.usage","version":"0.0.1-old"}"#,
            )
            .unwrap();

            let f = scan_omarchy(h.path());
            assert!(f.manifest_version_mismatch.is_some());
            assert!(f
                .manifest_version_mismatch
                .as_ref()
                .unwrap()
                .contains("0.0.1-old"));
        });
    }

    #[test]
    #[serial_test::serial]
    fn scan_omarchy_flags_plugin_dir_without_shell_entry() {
        with_clean_xdg_config_home(|| {
            let h = tempdir().unwrap();
            let plugin_dir = h
                .path()
                .join(".config")
                .join("omarchy")
                .join("plugins")
                .join(crate::app_identity::OMARCHY_PLUGIN_ID);
            std::fs::create_dir_all(&plugin_dir).unwrap();
            std::fs::write(
                plugin_dir.join("manifest.json"),
                format!(
                    r#"{{"id":"agent-bar.usage","version":"{}"}}"#,
                    crate::app_identity::VERSION
                ),
            )
            .unwrap();
            let shell_json = h.path().join(".config").join("omarchy").join("shell.json");
            std::fs::create_dir_all(shell_json.parent().unwrap()).unwrap();
            std::fs::write(&shell_json, r#"{"bar":{"plugins":[]}}"#).unwrap();

            let f = scan_omarchy(h.path());
            assert!(f.manifest_version_mismatch.is_none());
            assert!(f.plugin_dir_without_shell_entry.is_some());
            assert!(f.shell_entry_without_plugin_dir.is_none());
        });
    }

    #[test]
    #[serial_test::serial]
    fn scan_omarchy_flags_shell_entry_without_plugin_dir() {
        with_clean_xdg_config_home(|| {
            let h = tempdir().unwrap();
            let shell_json = h.path().join(".config").join("omarchy").join("shell.json");
            std::fs::create_dir_all(shell_json.parent().unwrap()).unwrap();
            std::fs::write(
                &shell_json,
                r#"{"bar":{"plugins":[{"id":"agent-bar.usage"}]}}"#,
            )
            .unwrap();

            let f = scan_omarchy(h.path());
            assert!(f.shell_entry_without_plugin_dir.is_some());
            assert!(f.plugin_dir_without_shell_entry.is_none());
        });
    }

    #[test]
    fn run_doctor_clean() {
        let h = tempdir().unwrap();
        let r = run_doctor(DoctorOptions {
            home: h.path(),
            dry_run: false,
            yes: false,
            confirm: &|_| true,
        });
        assert!(matches!(r.status, DoctorStatus::Clean));
        assert!(r.removed.is_empty());
    }

    #[test]
    fn run_doctor_removes_orphan_set_when_confirmed() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(h.path().join("bun.lock"), "").unwrap();
        std::fs::create_dir_all(
            h.path()
                .join("node_modules")
                .join("@noctuacore")
                .join("agent-bar"),
        )
        .unwrap();
        let r = run_doctor(DoctorOptions {
            home: h.path(),
            dry_run: false,
            yes: false,
            confirm: &|_| true,
        });
        assert!(matches!(r.status, DoctorStatus::Cleaned));
        assert!(!h.path().join("package.json").exists());
        assert!(!h.path().join("bun.lock").exists());
        assert!(!h
            .path()
            .join("node_modules")
            .join("@noctuacore")
            .join("agent-bar")
            .exists());
    }

    #[test]
    fn run_doctor_mixed_keeps_package_json() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0","other":"1.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(h.path().join("bun.lock"), "").unwrap();
        std::fs::create_dir_all(
            h.path()
                .join("node_modules")
                .join("@noctuacore")
                .join("agent-bar"),
        )
        .unwrap();
        let r = run_doctor(DoctorOptions {
            home: h.path(),
            dry_run: false,
            yes: false,
            confirm: &|_| true,
        });
        assert!(matches!(r.status, DoctorStatus::MixedOnly));
        assert_eq!(
            r.removed,
            vec![h
                .path()
                .join("node_modules")
                .join("@noctuacore")
                .join("agent-bar")]
        );
        assert!(h.path().join("package.json").exists());
        assert!(h.path().join("bun.lock").exists());
    }

    #[test]
    fn run_doctor_cancelled() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#,
        )
        .unwrap();
        let r = run_doctor(DoctorOptions {
            home: h.path(),
            dry_run: false,
            yes: false,
            confirm: &|_| false,
        });
        assert!(matches!(r.status, DoctorStatus::Cancelled));
        assert!(h.path().join("package.json").exists());
    }

    #[test]
    fn run_doctor_dry_run_reports_without_removing() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(h.path().join("bun.lock"), "").unwrap();
        std::fs::create_dir_all(
            h.path()
                .join("node_modules")
                .join("@noctuacore")
                .join("agent-bar"),
        )
        .unwrap();
        let r = run_doctor(DoctorOptions {
            home: h.path(),
            dry_run: true,
            yes: false,
            confirm: &|_| true,
        });
        assert!(matches!(r.status, DoctorStatus::Cleaned));
        assert_eq!(r.removed.len(), 3);
        assert!(h.path().join("package.json").exists());
    }

    #[test]
    fn run_doctor_yes_skips_confirm() {
        let h = tempdir().unwrap();
        std::fs::write(
            h.path().join("package.json"),
            r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#,
        )
        .unwrap();
        let called = Cell::new(false);
        let r = run_doctor(DoctorOptions {
            home: h.path(),
            dry_run: false,
            yes: true,
            confirm: &|_| {
                called.set(true);
                false
            },
        });
        assert!(!called.get(), "confirm must not be called when yes=true");
        assert!(matches!(r.status, DoctorStatus::Cleaned));
        assert!(!h.path().join("package.json").exists());
    }
}
