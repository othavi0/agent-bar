//! Descoberta do binário `grok` (locator). Ordem: PATH (`which`), depois
//! caminhos conhecidos sob `$HOME`.

use std::path::{Path, PathBuf};

/// Caminhos candidatos sob `$HOME`, na ordem de preferência. Vazio se `home` vazio.
pub fn grok_candidate_paths(home: &str) -> Vec<PathBuf> {
    if home.is_empty() {
        return Vec::new();
    }
    let h = Path::new(home);
    vec![
        h.join(".grok").join("bin").join("grok"),
        h.join(".local").join("bin").join("grok"),
    ]
}

/// Locator com seams injetáveis (`which`/`exists`) para teste. PATH primeiro;
/// depois o 1º candidato que existe; senão `None`.
pub fn find_grok_bin_with(
    home: &str,
    which: impl Fn(&str) -> Option<PathBuf>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(p) = which("grok") {
        return Some(p);
    }
    grok_candidate_paths(home)
        .into_iter()
        .find(|p| exists(p))
}

/// Locator de produção: `which_in_path` + `Path::is_file`.
pub fn find_grok_bin(home: &str) -> Option<PathBuf> {
    find_grok_bin_with(home, crate::providers::amp_cli::which_in_path, |p| {
        p.is_file()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_paths_under_home() {
        let paths = grok_candidate_paths("/tmp/agent-bar-home");
        let got: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            got,
            vec![
                "/tmp/agent-bar-home/.grok/bin/grok",
                "/tmp/agent-bar-home/.local/bin/grok",
            ]
        );
    }

    #[test]
    fn empty_home_yields_no_candidates() {
        assert!(grok_candidate_paths("").is_empty());
    }

    #[test]
    fn prefers_path_when_available() {
        let found = find_grok_bin_with(
            "/tmp/agent-bar-home",
            |_| Some(PathBuf::from("/usr/local/bin/grok")),
            |_| false,
        );
        assert_eq!(found, Some(PathBuf::from("/usr/local/bin/grok")));
    }

    #[test]
    fn falls_back_to_known_locations() {
        let found = find_grok_bin_with(
            "/tmp/agent-bar-home",
            |_| None,
            |p| p == Path::new("/tmp/agent-bar-home/.grok/bin/grok"),
        );
        assert_eq!(
            found,
            Some(PathBuf::from("/tmp/agent-bar-home/.grok/bin/grok"))
        );
    }

    #[test]
    fn none_when_unavailable() {
        let found = find_grok_bin_with("/tmp/agent-bar-home", |_| None, |_| false);
        assert_eq!(found, None);
    }
}
