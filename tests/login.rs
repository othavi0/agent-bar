//! Login argv, IPC refresh, and CLI-017 exit mapping.

use std::path::PathBuf;
use std::sync::Mutex;

use agent_bar::cli::ProviderId;
use agent_bar::providers::adapter::{run_login, ProviderAdapter};
use agent_bar::providers::adapters::{AMP_ADAPTER, CLAUDE_ADAPTER, CODEX_ADAPTER, GROK_ADAPTER};
use agent_bar::providers::catalog::{
    CollectionAvailability, Discovery, LoginAvailability, AMP, CLAUDE, CODEX, GROK,
};
use agent_bar::providers::process::{ProcessError, ProcessOutput, ProcessRunner, ProcessSpec};

struct ScriptedRunner {
    outputs: Mutex<Vec<Result<ProcessOutput, ProcessError>>>,
    pub specs: Mutex<Vec<ProcessSpec>>,
}

impl ScriptedRunner {
    fn from_outputs(outputs: Vec<Result<ProcessOutput, ProcessError>>) -> Self {
        Self {
            outputs: Mutex::new(outputs),
            specs: Mutex::new(Vec::new()),
        }
    }
}

impl ProcessRunner for ScriptedRunner {
    fn run<'a>(
        &'a self,
        spec: &'a ProcessSpec,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ProcessOutput, ProcessError>> + Send + 'a>,
    > {
        self.specs.lock().unwrap().push(spec.clone());
        let next = self
            .outputs
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| Err(ProcessError::Spawn("empty".into())));
        Box::pin(async move { next })
    }
}

fn discovery_ok(exe: &str) -> Discovery {
    Discovery {
        collection: CollectionAvailability::Available {
            executable: PathBuf::from(exe),
        },
        login: LoginAvailability::Available {
            executable: PathBuf::from(exe),
        },
    }
}

fn ok_output() -> ProcessOutput {
    ProcessOutput {
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        timed_out: false,
        stdout_truncated: false,
        stderr_truncated: false,
    }
}

#[tokio::test]
async fn login_argv_matches_catalog_with_resolved_executable() {
    for (adapter, exe, expected_tail) in [
        (
            &AMP_ADAPTER as &dyn ProviderAdapter,
            "/opt/bin/amp",
            vec!["login"],
        ),
        (&GROK_ADAPTER, "/opt/bin/grok", vec!["login"]),
        (&CODEX_ADAPTER, "/opt/bin/codex", vec!["login"]),
        (&CLAUDE_ADAPTER, "/opt/bin/claude", vec!["auth", "login"]),
    ] {
        let discovery = discovery_ok(exe);
        let spec = adapter.login_command(&discovery).unwrap();
        assert_eq!(spec.program, PathBuf::from(exe));
        assert_eq!(
            spec.args,
            expected_tail
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        // Element zero of full argv is absolute path, not bare name.
        assert!(spec.program.is_absolute() || spec.program.starts_with("/"));
    }
    assert_eq!(AMP.login_argv, &["amp", "login"]);
    assert_eq!(CLAUDE.login_argv, &["claude", "auth", "login"]);
    assert_eq!(CODEX.login_argv, &["codex", "login"]);
    assert_eq!(GROK.login_argv, &["grok", "login"]);
}

#[tokio::test]
async fn successful_login_runs_exact_refresh_ipc_argv() {
    let provider = ScriptedRunner::from_outputs(vec![Ok(ok_output())]);
    let ipc = ScriptedRunner::from_outputs(vec![Ok(ok_output())]);

    let discovery = discovery_ok("/usr/local/bin/claude");
    let outcome = run_login(&CLAUDE_ADAPTER, &discovery, &provider, &ipc)
        .await
        .unwrap();
    assert_eq!(outcome.exit_code, 0);
    assert!(outcome.refresh_attempted);
    assert_eq!(
        outcome.refresh_argv.as_ref().unwrap(),
        &vec![
            "omarchy-shell".to_owned(),
            "-q".to_owned(),
            "agent-bar.usage".to_owned(),
            "refresh".to_owned(),
            "claude".to_owned(),
        ]
    );
    let ipc_specs = ipc.specs.lock().unwrap();
    assert_eq!(ipc_specs.len(), 1);
    assert_eq!(ipc_specs[0].program, PathBuf::from("omarchy-shell"));
    assert_eq!(
        ipc_specs[0].args,
        vec![
            "-q".to_owned(),
            "agent-bar.usage".to_owned(),
            "refresh".to_owned(),
            "claude".to_owned(),
        ]
    );
    let prov_specs = provider.specs.lock().unwrap();
    assert_eq!(
        prov_specs[0].program,
        PathBuf::from("/usr/local/bin/claude")
    );
    assert_eq!(
        prov_specs[0].args,
        vec!["auth".to_owned(), "login".to_owned()]
    );
}

#[tokio::test]
async fn nonzero_login_does_not_refresh() {
    let provider = ScriptedRunner::from_outputs(vec![Ok(ProcessOutput {
        exit_code: Some(3),
        stdout: String::new(),
        stderr: String::new(),
        timed_out: false,
        stdout_truncated: false,
        stderr_truncated: false,
    })]);
    let ipc = ScriptedRunner::from_outputs(vec![Ok(ok_output())]);
    let discovery = discovery_ok("/usr/bin/amp");
    let outcome = run_login(&AMP_ADAPTER, &discovery, &provider, &ipc)
        .await
        .unwrap();
    assert_eq!(outcome.exit_code, 3);
    assert!(!outcome.refresh_attempted);
    assert!(ipc.specs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn signal_or_timeout_maps_to_exit_1_without_refresh() {
    let provider = ScriptedRunner::from_outputs(vec![Ok(ProcessOutput {
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        timed_out: true,
        stdout_truncated: false,
        stderr_truncated: false,
    })]);
    let ipc = ScriptedRunner::from_outputs(vec![Ok(ok_output())]);
    let discovery = discovery_ok("/usr/bin/grok");
    let outcome = run_login(&GROK_ADAPTER, &discovery, &provider, &ipc)
        .await
        .unwrap();
    assert_eq!(outcome.exit_code, 1);
    assert!(!outcome.refresh_attempted);
}

#[tokio::test]
async fn refresh_failure_does_not_change_successful_login_status() {
    let provider = ScriptedRunner::from_outputs(vec![Ok(ok_output())]);
    let ipc = ScriptedRunner::from_outputs(vec![Err(ProcessError::Spawn("ipc down".into()))]);
    let discovery = discovery_ok("/usr/bin/codex");
    let outcome = run_login(&CODEX_ADAPTER, &discovery, &provider, &ipc)
        .await
        .unwrap();
    assert_eq!(outcome.exit_code, 0);
    assert!(outcome.refresh_attempted);
}

#[test]
fn adapter_for_covers_all_provider_ids() {
    use agent_bar::providers::adapter_for;
    for id in ProviderId::ALL {
        let adapter = adapter_for(id);
        assert_eq!(adapter.descriptor().id, id);
    }
}
