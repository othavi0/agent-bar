//! Detecção única de plataforma (Omarchy-shell / Waybar) usada por todos os
//! gates de escrita em `~/.config/waybar/` (spec 2026-07-21, seção E): setup,
//! `update` (ManagedGit + Standalone) e o Save da TUI Config leem `detect()`
//! em vez de reimplementar a checagem cada um a seu jeito.

use std::ffi::OsStr;
use std::path::Path;

use crate::app_identity::OMARCHY_SHELL_DIR;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// omarchy-shell presente (`OMARCHY_SHELL_DIR` existe + CLI `omarchy` no PATH).
    pub omarchy: bool,
    /// Binário `waybar` no PATH.
    pub waybar: bool,
}

/// Núcleo testável: mesma composição de `detect()`, com `shell_dir`/`path_var`
/// injetáveis — mesmo padrão de mock de
/// `omarchy_integration::omarchy_shell_present`/`setup::waybar_present`.
pub fn detect_with(shell_dir: &Path, path_var: Option<&OsStr>) -> Platform {
    Platform {
        omarchy: crate::omarchy_integration::omarchy_shell_present(shell_dir, path_var),
        waybar: crate::setup::waybar_present(path_var),
    }
}

/// Detecção real do processo — único ponto de decisão consumido pelos gates
/// de plataforma (setup, update, TUI Save).
pub fn detect() -> Platform {
    detect_with(
        Path::new(OMARCHY_SHELL_DIR),
        std::env::var_os("PATH").as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detect_with_omarchy_only() {
        let shell = tempdir().unwrap();
        let bin = tempdir().unwrap();
        std::fs::write(bin.path().join("omarchy"), "#!/bin/sh\n").unwrap();
        let path_var = std::ffi::OsString::from(bin.path());

        let platform = detect_with(shell.path(), Some(&path_var));
        assert!(platform.omarchy);
        assert!(!platform.waybar);
    }

    #[test]
    fn detect_with_waybar_only() {
        let shell = tempdir().unwrap(); // dir existe, mas sem CLI `omarchy` no PATH
        let bin = tempdir().unwrap();
        std::fs::write(bin.path().join("waybar"), "#!/bin/sh\n").unwrap();
        let path_var = std::ffi::OsString::from(bin.path());

        let platform = detect_with(shell.path(), Some(&path_var));
        assert!(!platform.omarchy);
        assert!(platform.waybar);
    }

    #[test]
    fn detect_with_neither_present() {
        let shell = tempdir().unwrap();
        let bin = tempdir().unwrap();
        let path_var = std::ffi::OsString::from(bin.path());

        let platform = detect_with(&shell.path().join("nope"), Some(&path_var));
        assert!(!platform.omarchy);
        assert!(!platform.waybar);
    }

    #[test]
    fn detect_with_both_present() {
        let shell = tempdir().unwrap();
        let bin = tempdir().unwrap();
        std::fs::write(bin.path().join("omarchy"), "#!/bin/sh\n").unwrap();
        std::fs::write(bin.path().join("waybar"), "#!/bin/sh\n").unwrap();
        let path_var = std::ffi::OsString::from(bin.path());

        let platform = detect_with(shell.path(), Some(&path_var));
        assert!(platform.omarchy);
        assert!(platform.waybar);
    }
}
