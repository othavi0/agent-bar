//! Single-pass word-based CLI grammar. No filesystem or provider I/O.

use std::path::PathBuf;

use super::command::{
    CacheMode, Command, ConfigCommand, ConfigInput, DoctorCommand, HelpTopic, NotificationMode,
    ProviderId, ReleaseVersion, SetupOptions, StatusFormat, StatusOptions, UpdateCommand,
};
use super::exit::CliFailure;

/// Shared with `super::validate_plugins_dir`, which re-checks the same two
/// conditions after the filesystem is consulted. One definition means the
/// parse path and the validate path can never disagree about the wording.
pub(crate) const SETUP_PLUGINS_DIR_ABSOLUTE: &str = "setup plugins-dir path must be absolute";
pub(crate) const SETUP_PLUGINS_DIR_NOT_PLUGIN_ROOT: &str =
    "setup plugins-dir path must be the parent directory, not the plugin root";

const CONFIG_APPLY_USAGE: &str = "config apply requires stdin, file <path>, or json <value>";
const SETUP_PLUGINS_DIR_REQUIRES_PATH: &str = "setup plugins-dir requires a path";

/// Parse argv words after the program name into a closed [`Command`].
///
/// Grammar failures map to exit code 2. Parsing never touches the filesystem,
/// network, or provider adapters.
pub fn parse<I, S>(args: I) -> Result<Command, CliFailure>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let tokens: Vec<String> = args.into_iter().map(|s| s.as_ref().to_owned()).collect();
    parse_tokens(&tokens)
}

fn parse_tokens(tokens: &[String]) -> Result<Command, CliFailure> {
    if tokens.is_empty() {
        return Ok(Command::Status(StatusOptions::default()));
    }

    let first = tokens[0].as_str();
    match first {
        "--help" => {
            if tokens.len() != 1 {
                return Err(CliFailure::grammar("unexpected arguments after --help"));
            }
            return Ok(Command::Help(None));
        }
        "--version" => {
            if tokens.len() != 1 {
                return Err(CliFailure::grammar("unexpected arguments after --version"));
            }
            return Ok(Command::Version);
        }
        flag if flag.starts_with("--") => {
            return Err(CliFailure::grammar(format!(
                "unsupported flag '{flag}'; only --help and --version are accepted"
            )));
        }
        flag if flag.starts_with('-') && flag != "-" => {
            return Err(CliFailure::grammar(format!("unsupported flag '{flag}'")));
        }
        _ => {}
    }

    match first {
        "status" => parse_status(&tokens[1..]),
        "login" => parse_login(&tokens[1..]),
        "config" => parse_config(&tokens[1..]),
        "setup" => parse_setup(&tokens[1..]),
        "update" => parse_update(&tokens[1..]),
        "uninstall" => parse_uninstall(&tokens[1..]),
        "doctor" => parse_doctor(&tokens[1..]),
        "help" => parse_help(&tokens[1..]),
        "version" => {
            if tokens.len() != 1 {
                return Err(CliFailure::grammar("unexpected arguments after version"));
            }
            Ok(Command::Version)
        }
        other => Err(CliFailure::grammar(format!("unknown command '{other}'"))),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatusClause {
    Format,
    Provider,
    Cache,
    Notifications,
}

fn parse_status(tokens: &[String]) -> Result<Command, CliFailure> {
    let mut opts = StatusOptions::default();
    let mut seen = [false; 4];
    let mut i = 0;
    while i < tokens.len() {
        let word = tokens[i].as_str();
        if word.starts_with('-') {
            return Err(CliFailure::grammar(format!(
                "unsupported flag '{word}' in status"
            )));
        }
        let clause = match word {
            "format" => StatusClause::Format,
            "provider" => StatusClause::Provider,
            "cache" => StatusClause::Cache,
            "notifications" => StatusClause::Notifications,
            other => {
                return Err(CliFailure::grammar(format!(
                    "unknown argument '{other}' for status"
                )));
            }
        };
        let idx = clause as usize;
        if seen[idx] {
            return Err(CliFailure::grammar(format!(
                "repeated argument '{word}' for status"
            )));
        }
        seen[idx] = true;
        i += 1;
        let value = tokens
            .get(i)
            .map(String::as_str)
            .ok_or_else(|| missing_status_value(word))?;
        if value.starts_with('-') {
            return Err(missing_status_value(word));
        }
        match clause {
            StatusClause::Format => {
                opts.format = match value {
                    "human" => StatusFormat::Human,
                    "json" => StatusFormat::Json,
                    other => {
                        return Err(CliFailure::grammar(format!(
                            "unsupported status format '{other}'"
                        )));
                    }
                };
            }
            StatusClause::Provider => {
                opts.provider = Some(ProviderId::parse_word(value).ok_or_else(|| {
                    CliFailure::grammar(format!("unsupported provider '{value}'"))
                })?);
            }
            StatusClause::Cache => {
                opts.cache = match value {
                    "use" => CacheMode::Use,
                    "bypass" => CacheMode::Bypass,
                    other => {
                        return Err(CliFailure::grammar(format!(
                            "unsupported cache mode '{other}'"
                        )));
                    }
                };
            }
            StatusClause::Notifications => {
                opts.notifications = match value {
                    "evaluate" => NotificationMode::Evaluate,
                    "skip" => NotificationMode::Skip,
                    other => {
                        return Err(CliFailure::grammar(format!(
                            "unsupported notifications mode '{other}'"
                        )));
                    }
                };
            }
        }
        i += 1;
    }
    Ok(Command::Status(opts))
}

/// Both status clause errors — no value present, and a value that looks like
/// a flag — take the same `word` and want the same sentence.
fn missing_status_value(word: &str) -> CliFailure {
    CliFailure::grammar(format!("missing value for status {word}"))
}

fn parse_login(tokens: &[String]) -> Result<Command, CliFailure> {
    match tokens {
        [] => Err(CliFailure::grammar("login requires a provider id")),
        [provider] => {
            if provider.starts_with('-') {
                return Err(CliFailure::grammar(format!(
                    "unsupported flag '{provider}'"
                )));
            }
            let id = ProviderId::parse_word(provider)
                .ok_or_else(|| CliFailure::grammar(format!("unsupported provider '{provider}'")))?;
            Ok(Command::Login(id))
        }
        _ => Err(CliFailure::grammar(
            "unexpected arguments after login <provider>",
        )),
    }
}

fn parse_config(tokens: &[String]) -> Result<Command, CliFailure> {
    match tokens {
        [] => Err(CliFailure::grammar("config requires show or apply")),
        [cmd] if cmd == "show" => Ok(Command::Config(ConfigCommand::Show)),
        [cmd] if cmd == "apply" => Err(CliFailure::grammar(CONFIG_APPLY_USAGE)),
        [cmd, mode] if cmd == "apply" && mode == "stdin" => {
            Ok(Command::Config(ConfigCommand::Apply(ConfigInput::Stdin)))
        }
        [cmd, mode, path] if cmd == "apply" && mode == "file" => {
            if path.starts_with('-') && path != "-" {
                return Err(CliFailure::grammar(format!(
                    "invalid config apply file path '{path}'"
                )));
            }
            Ok(Command::Config(ConfigCommand::Apply(ConfigInput::File(
                PathBuf::from(path),
            ))))
        }
        [cmd, mode, value] if cmd == "apply" && mode == "json" => Ok(Command::Config(
            ConfigCommand::Apply(ConfigInput::Json(value.clone())),
        )),
        [cmd, ..] if cmd == "apply" => Err(CliFailure::grammar(CONFIG_APPLY_USAGE)),
        [other, ..] => Err(CliFailure::grammar(format!(
            "unknown config command '{other}'"
        ))),
    }
}

fn parse_setup(tokens: &[String]) -> Result<Command, CliFailure> {
    match tokens {
        [] => Ok(Command::Setup(SetupOptions::Production)),
        [word, path] if word == "plugins-dir" => {
            if path.is_empty() {
                return Err(CliFailure::grammar(SETUP_PLUGINS_DIR_REQUIRES_PATH));
            }
            // Path shape checks that need no filesystem: relative and direct
            // plugin-root forms are rejected here; existence/writability are
            // validated after parse.
            let pb = PathBuf::from(path);
            if !pb.is_absolute() {
                return Err(CliFailure::grammar(SETUP_PLUGINS_DIR_ABSOLUTE));
            }
            if path_ends_with_plugin_id(&pb) {
                return Err(CliFailure::grammar(SETUP_PLUGINS_DIR_NOT_PLUGIN_ROOT));
            }
            Ok(Command::Setup(SetupOptions::PluginsDir(pb)))
        }
        [word] if word == "plugins-dir" => {
            Err(CliFailure::grammar(SETUP_PLUGINS_DIR_REQUIRES_PATH))
        }
        [other, ..] => Err(CliFailure::grammar(format!(
            "unknown argument '{other}' for setup"
        ))),
    }
}

fn path_ends_with_plugin_id(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name == "agent-bar.usage")
}

fn parse_update(tokens: &[String]) -> Result<Command, CliFailure> {
    match tokens {
        [] => Ok(Command::Update(UpdateCommand::Interactive)),
        [word] if word == "check" => Ok(Command::Update(UpdateCommand::Check)),
        [word] if word == "apply" => Err(CliFailure::grammar("update apply requires a version")),
        [word, version] if word == "apply" => {
            let release = ReleaseVersion::parse(version).map_err(|_| {
                CliFailure::grammar(format!(
                    "update apply requires a strict semantic version; got '{version}'"
                ))
            })?;
            Ok(Command::Update(UpdateCommand::Apply(release)))
        }
        [other, ..] => Err(CliFailure::grammar(format!(
            "unknown argument '{other}' for update"
        ))),
    }
}

fn parse_uninstall(tokens: &[String]) -> Result<Command, CliFailure> {
    match tokens {
        [] => Ok(Command::Uninstall { purge: false }),
        [word] if word == "purge" => Ok(Command::Uninstall { purge: true }),
        [other, ..] => Err(CliFailure::grammar(format!(
            "unknown argument '{other}' for uninstall"
        ))),
    }
}

fn parse_doctor(tokens: &[String]) -> Result<Command, CliFailure> {
    match tokens {
        [word] if word == "scan" => Ok(Command::Doctor(DoctorCommand::Scan)),
        [word] if word == "clean" => Ok(Command::Doctor(DoctorCommand::Clean)),
        [] => Err(CliFailure::grammar("doctor requires scan or clean")),
        [other, ..] => Err(CliFailure::grammar(format!(
            "unknown argument '{other}' for doctor"
        ))),
    }
}

fn parse_help(tokens: &[String]) -> Result<Command, CliFailure> {
    match tokens {
        [] => Ok(Command::Help(None)),
        [topic] => {
            let topic = HelpTopic::parse_word(topic)
                .ok_or_else(|| CliFailure::grammar(format!("unknown help topic '{topic}'")))?;
            Ok(Command::Help(Some(topic)))
        }
        _ => Err(CliFailure::grammar(
            "unexpected arguments after help <topic>",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::exit::GRAMMAR;

    fn words(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn bare_invocation_is_status_human_defaults() {
        let cmd = parse(words(&[])).unwrap();
        assert_eq!(
            cmd,
            Command::Status(StatusOptions {
                format: StatusFormat::Human,
                provider: None,
                cache: CacheMode::Use,
                notifications: NotificationMode::Skip,
            })
        );
    }

    #[test]
    fn rejects_legacy_double_dash_flags() {
        for flag in [
            "--format",
            "--json",
            "--provider",
            "--verbose",
            "--watch",
            "--interval",
            "--yes",
            "--dry-run",
        ] {
            let err = parse(words(&[flag])).unwrap_err();
            assert_eq!(err.exit_code, GRAMMAR, "{flag}");
        }
    }

    #[test]
    fn accepts_only_help_and_version_aliases() {
        assert_eq!(parse(words(&["--help"])).unwrap(), Command::Help(None));
        assert_eq!(parse(words(&["--version"])).unwrap(), Command::Version);
    }
}
