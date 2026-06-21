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
    find_amp_bin_with(home, which_in_path, |p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_paths_under_home() {
        let paths = amp_candidate_paths("/tmp/agent-bar-home");
        let got: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
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
        assert_eq!(
            found,
            Some(PathBuf::from("/tmp/agent-bar-home/.local/bin/amp"))
        );
    }

    #[test]
    fn none_when_unavailable() {
        let found = find_amp_bin_with("/tmp/agent-bar-home", |_| None, |_| false);
        assert_eq!(found, None);
    }

    #[test]
    fn install_command_is_official() {
        assert_eq!(
            AMP_INSTALL_COMMAND,
            "curl -fsSL https://ampcode.com/install.sh | bash"
        );
    }
}
