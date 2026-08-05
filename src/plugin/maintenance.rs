//! Update check, closed download policy, maintenance worker handoff, and health polls.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::plugin::bundle::{
    BundleError, BundleValidator, MINIMUM_QUICKSHELL_VERSION, OFFICIAL_TARGET, OMARCHY_CONTRACT,
};
use crate::plugin::omarchy::{CommandOutput, CommandRunner, OmarchyError};
use crate::plugin::ownership::hash_bytes;
use crate::plugin::paths::{validate_txid, PathError, PluginPaths, PLUGIN_ID};
use crate::plugin::transaction::{
    atomic_write_bytes, copy_dir_all, quarantine_rename, remove_exact_plugin_entries,
    TransactionError, TransactionJournal, TxStep,
};
use crate::support::maintenance_gate::MaintenanceGate;
use crate::support::Clock;

/// Executable basename that selects maintenance-worker mode before CLI parsing.
pub const MAINTENANCE_WORKER_NAME: &str = "agent-bar-maintenance-worker";

/// Distribution repo `bundle.json` receipt: the sole `update check` discovery
/// source under git-native distribution. Served directly by
/// raw.githubusercontent.com — no redirect-following is needed.
pub const DIST_RECEIPT_URL: &str =
    "https://raw.githubusercontent.com/othavi0/omarchy-agent-bar/master/bundle.json";

/// User-Agent for release discovery/download (no provider credentials).
pub const RELEASE_USER_AGENT: &str = concat!("agent-bar-update/", env!("CARGO_PKG_VERSION"));

/// Literal prefix of the computed `latestCompatible.releaseNotesUrl`.
pub const RELEASE_NOTES_URL_PREFIX: &str = "https://github.com/othavi0/agent-bar/releases/tag/v";

/// Rescan poll total deadline (BUNDLE-032G).
pub const RESCAN_POLL_DEADLINE: Duration = Duration::from_secs(15);

/// systemd RuntimeMaxSec hard bound.
pub const WORKER_RUNTIME_MAX_SECS: u64 = 600;
/// Preflight/download/stage must finish by this monotonic offset.
pub const DEADLINE_STAGE_SECS: u64 = 420;
/// Live mutation must finish by this offset.
pub const DEADLINE_MUTATION_SECS: u64 = 510;
/// Rollback must finish by this offset (30s reserve before hard bound).
pub const DEADLINE_ROLLBACK_SECS: u64 = 570;

/// Environment names forwarded into the transient worker unit.
pub const WORKER_ENV_ALLOWLIST: &[&str] = &[
    "HOME",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "WAYLAND_DISPLAY",
    "OMARCHY_PATH",
];

#[derive(Debug, Error)]
pub enum MaintenanceError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Bundle(#[from] BundleError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Omarchy(#[from] OmarchyError),
}

impl MaintenanceError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

// ---------------------------------------------------------------------------
// Update check document
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCurrent {
    pub version: String,
    pub target: String,
    pub omarchy_contract: u32,
    pub quickshell_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCompatible {
    pub version: String,
    pub omarchy_contract: u32,
    pub minimum_quickshell_version: String,
    pub release_notes_url: String,
}

/// Exact successful `update check` stdout document (BUNDLE-021 v-next).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCheckDocument {
    pub schema_version: u32,
    pub checked_at: String,
    pub current: UpdateCurrent,
    pub available: bool,
    pub reinstall_required: bool,
    pub latest_compatible: Option<UpdateCompatible>,
}

impl UpdateCheckDocument {
    pub fn to_stdout_json(&self) -> Result<String, MaintenanceError> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }

    pub fn parse_json(bytes: &[u8]) -> Result<Self, MaintenanceError> {
        let doc: Self = serde_json::from_slice(bytes)?;
        doc.validate()?;
        Ok(doc)
    }

    pub fn validate(&self) -> Result<(), MaintenanceError> {
        if self.schema_version != 1 {
            return Err(MaintenanceError::msg(
                "update check schemaVersion must be 1",
            ));
        }
        // RFC3339 checkedAt
        OffsetDateTime::parse(&self.checked_at, &Rfc3339)
            .map_err(|e| MaintenanceError::msg(format!("checkedAt is not RFC3339: {e}")))?;
        if self.current.target != OFFICIAL_TARGET {
            return Err(MaintenanceError::msg(format!(
                "current.target must be {OFFICIAL_TARGET}"
            )));
        }
        if self.current.omarchy_contract != OMARCHY_CONTRACT {
            return Err(MaintenanceError::msg("current.omarchyContract must be 1"));
        }
        // A reinstall-required document cannot also offer an update: the QML
        // side (later task) shows only the reinstall message in that phase.
        if self.reinstall_required && (self.available || self.latest_compatible.is_some()) {
            return Err(MaintenanceError::msg(
                "reinstallRequired documents must have available:false and latestCompatible:null",
            ));
        }
        if let Some(ref latest) = self.latest_compatible {
            if latest.omarchy_contract != OMARCHY_CONTRACT {
                return Err(MaintenanceError::msg(
                    "latestCompatible.omarchyContract must be 1",
                ));
            }
            if latest.minimum_quickshell_version != MINIMUM_QUICKSHELL_VERSION {
                return Err(MaintenanceError::msg(format!(
                    "latestCompatible.minimumQuickshellVersion must be {MINIMUM_QUICKSHELL_VERSION}"
                )));
            }
            if !latest
                .release_notes_url
                .starts_with(RELEASE_NOTES_URL_PREFIX)
            {
                return Err(MaintenanceError::msg(format!(
                    "latestCompatible.releaseNotesUrl must start with {RELEASE_NOTES_URL_PREFIX}"
                )));
            }
            let current = semver::Version::parse(&self.current.version)
                .map_err(|e| MaintenanceError::msg(e.to_string()))?;
            let latest_v = semver::Version::parse(&latest.version)
                .map_err(|e| MaintenanceError::msg(e.to_string()))?;
            let expected_available = latest_v > current;
            if self.available != expected_available {
                return Err(MaintenanceError::msg(
                    "available must be true exactly when latestCompatible is strictly newer",
                ));
            }
        } else if self.available {
            return Err(MaintenanceError::msg(
                "available cannot be true when latestCompatible is null",
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Injectable HTTP for releases
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReleaseHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub trait ReleaseHttp: Send + Sync {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<ReleaseHttpResponse, MaintenanceError>;
}

/// Production HTTPS client: no automatic redirects, no default credentials.
///
/// Uses a short-lived current-thread Tokio runtime so the synchronous
/// maintenance path can share the async `reqwest` client already in the tree.
pub struct ReqwestReleaseHttp {
    client: reqwest::Client,
}

impl ReqwestReleaseHttp {
    pub fn new() -> Result<Self, MaintenanceError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(RELEASE_USER_AGENT)
            .build()
            .map_err(|e| MaintenanceError::msg(format!("http client: {e}")))?;
        Ok(Self { client })
    }
}

impl ReleaseHttp for ReqwestReleaseHttp {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<ReleaseHttpResponse, MaintenanceError> {
        // Never attach Authorization / Cookie.
        for (k, _) in headers {
            let lower = k.to_ascii_lowercase();
            if lower == "authorization" || lower == "cookie" || lower == "proxy-authorization" {
                return Err(MaintenanceError::msg(
                    "credentials must not be attached to release downloads",
                ));
            }
        }
        let url = url.to_owned();
        let headers: Vec<(String, String)> = headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let client = self.client.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| MaintenanceError::msg(format!("tokio runtime: {e}")))?;
        rt.block_on(async move {
            let mut req = client.get(&url);
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            let response = req
                .send()
                .await
                .map_err(|e| MaintenanceError::msg(format!("release GET failed: {e}")))?;
            let status = response.status().as_u16();
            let hdrs = response
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let body = response
                .bytes()
                .await
                .map_err(|e| MaintenanceError::msg(format!("body read: {e}")))?
                .to_vec();
            Ok(ReleaseHttpResponse {
                status,
                headers: hdrs,
                body,
            })
        })
    }
}

type HeaderPairs = Vec<(String, String)>;
type ScriptedCall = (String, HeaderPairs);
type ScriptedResponse = Result<ReleaseHttpResponse, MaintenanceError>;

/// Scripted HTTP for tests.
#[derive(Debug, Default)]
pub struct ScriptedReleaseHttp {
    pub responses: Mutex<Vec<ScriptedResponse>>,
    pub calls: Mutex<Vec<ScriptedCall>>,
}

impl ScriptedReleaseHttp {
    pub fn with_responses(responses: Vec<Result<ReleaseHttpResponse, MaintenanceError>>) -> Self {
        Self {
            responses: Mutex::new(responses),
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl ReleaseHttp for ScriptedReleaseHttp {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<ReleaseHttpResponse, MaintenanceError> {
        for (k, _) in headers {
            let lower = k.to_ascii_lowercase();
            if lower == "authorization" || lower == "cookie" {
                return Err(MaintenanceError::msg(
                    "credentials must not be attached to release downloads",
                ));
            }
        }
        self.calls.lock().unwrap_or_else(|e| e.into_inner()).push((
            url.to_string(),
            headers
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        ));
        let mut q = self.responses.lock().unwrap_or_else(|e| e.into_inner());
        q.pop().unwrap_or_else(|| {
            Err(MaintenanceError::msg(
                "scripted release HTTP client exhausted",
            ))
        })
    }
}

// ---------------------------------------------------------------------------
// Update check
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UpdateCheckProbe {
    pub current_version: String,
    pub quickshell_version: String,
    pub target: String,
    pub omarchy_contract: u32,
}

impl Default for UpdateCheckProbe {
    fn default() -> Self {
        Self {
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            quickshell_version: MINIMUM_QUICKSHELL_VERSION.to_string(),
            target: OFFICIAL_TARGET.to_string(),
            omarchy_contract: OMARCHY_CONTRACT,
        }
    }
}

/// Distribution repo `bundle.json` receipt shape (BUNDLE-012 producer, this
/// task's consumer). Unknown fields (`sourceCommit`, `files`, ...) are
/// intentionally tolerated — `update check` only needs discovery fields.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistReceipt {
    schema_version: u32,
    plugin_id: String,
    version: String,
    target: String,
    omarchy_contract: u32,
    minimum_quickshell_version: String,
}

pub struct UpdateCheck;

impl UpdateCheck {
    /// Fetch the dist repo receipt and return the machine `update check` document.
    ///
    /// `reinstall_required` is computed by the caller (BUNDLE-021 v-next: a
    /// non-git plugin root cannot be fast-forwarded by `omarchy plugin
    /// update`) so unit tests can script both states without touching the
    /// filesystem. When true, the receipt is still fetched and validated for
    /// its own sake, but `available`/`latestCompatible` are forced to their
    /// null state to satisfy `UpdateCheckDocument::validate`.
    pub fn run<H: ReleaseHttp, C: Clock>(
        http: &H,
        clock: &C,
        probe: &UpdateCheckProbe,
        reinstall_required: bool,
    ) -> Result<UpdateCheckDocument, MaintenanceError> {
        let resp = http.get(
            DIST_RECEIPT_URL,
            &[
                ("Accept", "application/json"),
                ("User-Agent", RELEASE_USER_AGENT),
            ],
        )?;
        if resp.status != 200 {
            return Err(MaintenanceError::msg(format!(
                "dist receipt fetch returned HTTP {}",
                resp.status
            )));
        }
        let receipt: DistReceipt = serde_json::from_slice(&resp.body)
            .map_err(|e| MaintenanceError::msg(format!("malformed dist receipt: {e}")))?;

        if receipt.schema_version != 1 {
            return Err(MaintenanceError::msg(
                "dist receipt schemaVersion must be 1",
            ));
        }
        if receipt.plugin_id != PLUGIN_ID {
            return Err(MaintenanceError::msg(format!(
                "dist receipt pluginId must be {PLUGIN_ID}"
            )));
        }
        if receipt.target != OFFICIAL_TARGET {
            return Err(MaintenanceError::msg(format!(
                "dist receipt target must be {OFFICIAL_TARGET}"
            )));
        }
        if receipt.omarchy_contract != OMARCHY_CONTRACT {
            return Err(MaintenanceError::msg(format!(
                "dist receipt omarchyContract must be {OMARCHY_CONTRACT}"
            )));
        }

        let current = semver::Version::parse(&probe.current_version)
            .map_err(|e| MaintenanceError::msg(format!("current version: {e}")))?;
        let receipt_version = semver::Version::parse(&receipt.version)
            .map_err(|e| MaintenanceError::msg(format!("dist receipt version: {e}")))?;
        let qs = semver::Version::parse(&probe.quickshell_version)
            .map_err(|e| MaintenanceError::msg(format!("quickshell version: {e}")))?;
        let receipt_min_qs = semver::Version::parse(&receipt.minimum_quickshell_version)
            .map_err(|e| MaintenanceError::msg(e.to_string()))?;

        let compatible = qs >= receipt_min_qs;
        let (available, latest_compatible) = if reinstall_required {
            (false, None)
        } else if compatible {
            (
                receipt_version > current,
                Some(UpdateCompatible {
                    version: receipt.version.clone(),
                    omarchy_contract: receipt.omarchy_contract,
                    minimum_quickshell_version: receipt.minimum_quickshell_version.clone(),
                    release_notes_url: format!("{RELEASE_NOTES_URL_PREFIX}{}", receipt.version),
                }),
            )
        } else {
            // Locally incompatible — not an error, just nothing to offer.
            (false, None)
        };

        let checked_at = clock
            .now_utc()
            .format(&Rfc3339)
            .map_err(|e| MaintenanceError::msg(format!("time format: {e}")))?;

        let doc = UpdateCheckDocument {
            schema_version: 1,
            checked_at,
            current: UpdateCurrent {
                version: probe.current_version.clone(),
                target: probe.target.clone(),
                omarchy_contract: probe.omarchy_contract,
                quickshell_version: probe.quickshell_version.clone(),
            },
            available,
            reinstall_required,
            latest_compatible,
        };
        doc.validate()?;
        Ok(doc)
    }
}

// ---------------------------------------------------------------------------
// Rescan / health poll
// ---------------------------------------------------------------------------

/// Poll delays: 100, 200, 400, 500, 500... ms.
pub fn rescan_poll_delays() -> impl Iterator<Item = Duration> {
    let mut n = 0u32;
    std::iter::from_fn(move || {
        let ms = match n {
            0 => 100,
            1 => 200,
            2 => 400,
            _ => 500,
        };
        n = n.saturating_add(1);
        Some(Duration::from_millis(ms))
    })
}

/// Health IPC argv: `omarchy-shell agent-bar.usage health <expectedVersion>`.
pub fn health_argv(expected_version: &str) -> Vec<String> {
    vec![
        "agent-bar.usage".into(),
        "health".into(),
        expected_version.into(),
    ]
}

/// True when health stdout is exactly `ok\n` and exit is 0.
pub fn health_is_ok(out: &CommandOutput) -> bool {
    out.code == 0 && out.stdout == "ok\n"
}

/// Parse `listPlugins` JSON and require absence of exact ID `agent-bar.usage`.
pub fn list_plugins_absent(stdout: &str) -> Result<bool, MaintenanceError> {
    let value: serde_json::Value = serde_json::from_str(stdout)
        .map_err(|e| MaintenanceError::msg(format!("malformed listPlugins JSON: {e}")))?;
    let arr = value
        .as_array()
        .ok_or_else(|| MaintenanceError::msg("listPlugins JSON must be an array"))?;
    for entry in arr {
        let id = entry
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MaintenanceError::msg("listPlugins entry missing id"))?;
        if id == PLUGIN_ID {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Parse `listPlugins` JSON and require presence of exact enabled entry.
pub fn list_plugins_has_enabled(stdout: &str) -> Result<bool, MaintenanceError> {
    let value: serde_json::Value = serde_json::from_str(stdout)
        .map_err(|e| MaintenanceError::msg(format!("malformed listPlugins JSON: {e}")))?;
    let arr = value
        .as_array()
        .ok_or_else(|| MaintenanceError::msg("listPlugins JSON must be an array"))?;
    for entry in arr {
        let id = entry.get("id").and_then(|v| v.as_str());
        if id == Some(PLUGIN_ID) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Monotonic poll helper with injectable sleeper / clock.
pub trait Sleeper {
    fn sleep(&self, d: Duration);
}

pub struct RealSleeper;
impl Sleeper for RealSleeper {
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// Fake clock that advances on demand (tests).
#[derive(Debug, Default)]
pub struct FakeMonotonic {
    pub millis: Mutex<u64>,
}

impl FakeMonotonic {
    pub fn new(start_ms: u64) -> Self {
        Self {
            millis: Mutex::new(start_ms),
        }
    }
    pub fn now(&self) -> Duration {
        Duration::from_millis(*self.millis.lock().unwrap_or_else(|e| e.into_inner()))
    }
    pub fn advance(&self, d: Duration) {
        *self.millis.lock().unwrap_or_else(|e| e.into_inner()) += d.as_millis() as u64;
    }
}

/// Poll health after asynchronous rescan until success or 15s deadline.
pub fn poll_update_health<R: CommandRunner, S: Sleeper>(
    runner: &R,
    shell_program: &str,
    expected_version: &str,
    sleeper: &S,
    start: Duration,
    now: &dyn Fn() -> Duration,
) -> Result<(), MaintenanceError> {
    let deadline = start + RESCAN_POLL_DEADLINE;
    let mut delays = rescan_poll_delays();
    loop {
        let args = health_argv(expected_version);
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = runner.run(shell_program, &arg_refs)?;
        if health_is_ok(&out) {
            return Ok(());
        }
        // Malformed non-ok that is not simply "not ready" still continues until
        // deadline; final failure reports mismatch.
        let t = now();
        if t >= deadline {
            return Err(MaintenanceError::msg(format!(
                "health poll timeout: last stdout={:?} code={}",
                out.stdout, out.code
            )));
        }
        let delay = delays.next().unwrap_or(Duration::from_millis(500));
        let remaining = deadline.saturating_sub(t);
        sleeper.sleep(delay.min(remaining));
    }
}

// ---------------------------------------------------------------------------
// Maintenance worker journal payload + handoff
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceOp {
    Update,
    Uninstall,
}

/// Durable payload written into the transaction journal for the worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaintenanceJournalPayload {
    pub txid: String,
    pub operation: MaintenanceOp,
    pub expected_version: Option<String>,
    pub previous_version: Option<String>,
    pub stage_path: String,
    pub plugin_root: String,
    pub quarantine_path: String,
    pub selected: Option<UpdateCompatible>,
    /// Absolute paths recorded at preflight for worker-internal tools.
    pub omarchy_bin: String,
    pub omarchy_shell_bin: String,
    pub is_fresh_install: bool,
    pub is_v9_rollback: bool,
    /// Uninstall: when true, also quarantine settings and owned backups.
    #[serde(default)]
    pub purge_settings_and_backups: bool,
    /// Absolute path to Omarchy `shell.json` (literal `$HOME/.config/omarchy/...`).
    #[serde(default)]
    pub shell_json_path: String,
    /// Absolute path to product `settings.json` (may be purged).
    #[serde(default)]
    pub settings_path: String,
    /// Absolute `$XDG_CACHE_HOME/agent-bar` cache root (quarantined by both forms).
    #[serde(default)]
    pub cache_root: String,
    /// Absolute migration backups directory (purged only with purge=true).
    #[serde(default)]
    pub backups_dir: String,
}

/// Non-TTY structured uninstall confirmation (CLI-036 / BUNDLE-036).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UninstallConfirmation {
    pub schema_version: u32,
    pub operation: String,
    pub confirmed: bool,
    pub purge_settings_and_backups: bool,
}

impl UninstallConfirmation {
    pub fn expected(purge: bool) -> Self {
        Self {
            schema_version: 1,
            operation: "uninstall".into(),
            confirmed: true,
            purge_settings_and_backups: purge,
        }
    }

    /// Parse exactly one JSON object followed by optional whitespace and EOF.
    pub fn parse_strict(bytes: &[u8], expect_purge: bool) -> Result<Self, MaintenanceError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| MaintenanceError::msg("uninstall confirmation is not valid UTF-8"))?;
        let trimmed = text.trim();
        // Reject concatenated second values / trailing garbage by requiring the
        // entire trimmed buffer to be exactly one JSON value.
        let doc: Self = serde_json::from_str(trimmed)
            .map_err(|e| MaintenanceError::msg(format!("malformed uninstall confirmation: {e}")))?;
        // `from_str` tolerates trailing whitespace only; re-check by round-trip
        // stream: if a second value exists, Value::deserialize from remaining fails
        // the "exactly one object" rule via trailing non-ws after first value.
        let mut stream =
            serde_json::Deserializer::from_str(trimmed).into_iter::<serde_json::Value>();
        let first = stream
            .next()
            .transpose()
            .map_err(|e| MaintenanceError::msg(format!("malformed uninstall confirmation: {e}")))?;
        if first.is_none() {
            return Err(MaintenanceError::msg("empty uninstall confirmation"));
        }
        if let Some(extra) = stream.next() {
            match extra {
                Ok(_) => {
                    return Err(MaintenanceError::msg(
                        "uninstall confirmation has trailing non-whitespace content",
                    ));
                }
                Err(e) => {
                    return Err(MaintenanceError::msg(format!(
                        "uninstall confirmation has trailing non-whitespace content: {e}"
                    )));
                }
            }
        }
        doc.validate(expect_purge)?;
        Ok(doc)
    }

    pub fn validate(&self, expect_purge: bool) -> Result<(), MaintenanceError> {
        if self.schema_version != 1 {
            return Err(MaintenanceError::msg(
                "uninstall confirmation schemaVersion must be 1",
            ));
        }
        if self.operation != "uninstall" {
            return Err(MaintenanceError::msg(
                "uninstall confirmation operation must be \"uninstall\"",
            ));
        }
        if !self.confirmed {
            return Err(MaintenanceError::msg(
                "uninstall confirmation requires confirmed: true",
            ));
        }
        if self.purge_settings_and_backups != expect_purge {
            return Err(MaintenanceError::msg(
                "uninstall confirmation purgeSettingsAndBackups does not match command",
            ));
        }
        Ok(())
    }
}

/// Exact TTY phrase required for interactive uninstall.
pub const UNINSTALL_TTY_PHRASE: &str = "uninstall agent-bar";

/// TTY prompt text written to stderr (no trailing newline required by contract).
pub const UNINSTALL_TTY_PROMPT: &str = "Type uninstall agent-bar to continue:";

/// Build the exact transient unit name.
pub fn maintenance_unit_name(txid: &str) -> Result<String, MaintenanceError> {
    validate_txid(txid)?;
    Ok(format!("agent-bar-maintenance-{txid}.service"))
}

/// Exact `systemd-run --user` argv for the transient worker unit.
pub fn systemd_run_argv(
    unit_name: &str,
    worker_path: &Path,
    txid: &str,
    env_pairs: &[(String, String)],
) -> Vec<String> {
    let mut argv = vec![
        "--user".into(),
        "--collect".into(),
        format!("--unit={unit_name}"),
        "--property=Type=exec".into(),
        "--property=UMask=0077".into(),
        "--property=TimeoutStartSec=120".into(),
        format!("--property=RuntimeMaxSec={WORKER_RUNTIME_MAX_SECS}"),
    ];
    for (k, v) in env_pairs {
        argv.push(format!("--setenv={k}={v}"));
    }
    argv.push(worker_path.display().to_string());
    argv.push(txid.to_string());
    argv
}

/// Filter process environment down to the allowlist (missing optional omitted).
pub fn collect_worker_env(
    env: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
) -> Vec<(String, String)> {
    let allow: std::collections::HashSet<&str> = WORKER_ENV_ALLOWLIST.iter().copied().collect();
    let mut out = Vec::new();
    for (k, v) in env {
        let k = k.as_ref();
        let v = v.as_ref();
        if allow.contains(k) && !v.is_empty() {
            out.push((k.to_string(), v.to_string()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Copy current helper into the transaction directory as the maintenance worker.
pub fn install_worker_copy(
    current_exe: &Path,
    transactions_dir: &Path,
) -> Result<PathBuf, MaintenanceError> {
    fs::create_dir_all(transactions_dir)?;
    let dest = transactions_dir.join(MAINTENANCE_WORKER_NAME);
    fs::copy(current_exe, &dest)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
    }
    // Verify the copy is a regular executable file.
    let meta = fs::metadata(&dest)?;
    if !meta.is_file() {
        return Err(MaintenanceError::msg("worker copy is not a regular file"));
    }
    let src_hash = hash_bytes(&fs::read(current_exe)?);
    let dst_hash = hash_bytes(&fs::read(&dest)?);
    if src_hash != dst_hash {
        return Err(MaintenanceError::msg(
            "worker copy checksum does not match source helper",
        ));
    }
    Ok(dest)
}

/// True when argv0 basename selects worker mode.
pub fn is_maintenance_worker_exe(exe: &Path) -> bool {
    exe.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n == MAINTENANCE_WORKER_NAME)
}

/// Deadlines relative to worker start (monotonic seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerDeadlines {
    pub stage_by: Duration,
    pub mutation_by: Duration,
    pub rollback_by: Duration,
    pub hard: Duration,
}

impl WorkerDeadlines {
    pub fn from_start(start: Duration) -> Self {
        Self {
            stage_by: start + Duration::from_secs(DEADLINE_STAGE_SECS),
            mutation_by: start + Duration::from_secs(DEADLINE_MUTATION_SECS),
            rollback_by: start + Duration::from_secs(DEADLINE_ROLLBACK_SECS),
            hard: start + Duration::from_secs(WORKER_RUNTIME_MAX_SECS),
        }
    }

    /// Refuse to begin live mutation without the reserved rollback window.
    pub fn may_begin_mutation(&self, now: Duration) -> Result<(), MaintenanceError> {
        if now >= self.mutation_by {
            return Err(MaintenanceError::msg(
                "mutation deadline exceeded before live mutation",
            ));
        }
        // Need enough time for rollback reserve: rollback_by must be after now.
        if now >= self.rollback_by {
            return Err(MaintenanceError::msg(
                "insufficient rollback reserve before mutation",
            ));
        }
        Ok(())
    }
}

/// Locate `cmd` on `$PATH` (first regular file hit).
fn which_in_path(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(cmd))
        .find(|p| p.is_file())
}

/// Resolve a bare tool name (or absolute path) to an absolute executable path
/// during preflight (BUNDLE-032H).
pub fn resolve_absolute_executable(name_or_path: &str) -> Result<String, MaintenanceError> {
    let path = Path::new(name_or_path);
    if path.is_absolute() {
        require_absolute_executable(name_or_path)?;
        return Ok(name_or_path.to_string());
    }
    if name_or_path.contains('/') {
        return Err(MaintenanceError::msg(format!(
            "relative tool path rejected: {name_or_path}"
        )));
    }
    let found = which_in_path(name_or_path).ok_or_else(|| {
        MaintenanceError::msg(format!("executable not found on PATH: {name_or_path}"))
    })?;
    let absolute = found.canonicalize().unwrap_or(found);
    let s = absolute.display().to_string();
    require_absolute_executable(&s)?;
    Ok(s)
}

/// Require a preflight-recorded absolute executable path.
pub fn require_absolute_executable(path: &str) -> Result<(), MaintenanceError> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(MaintenanceError::msg(format!(
            "tool path must be absolute: {path}"
        )));
    }
    let meta = fs::metadata(p)
        .map_err(|e| MaintenanceError::msg(format!("tool path not accessible ({path}): {e}")))?;
    if !meta.is_file() {
        return Err(MaintenanceError::msg(format!(
            "tool path is not a regular file: {path}"
        )));
    }
    Ok(())
}

/// Classification of the live plugin root before update (BUNDLE-029 / 032G).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalPluginClass {
    /// No live plugin root — fresh install path.
    Absent,
    /// `bundle.json` inventory validates (owned current v10).
    OwnedCurrent,
    /// Receipt present but inventory/content diverges from it.
    Modified,
    /// Looks like a pre-v10 Agent Bar tree (no receipt).
    V9Structural,
    /// Exists but is not a recognizable owned plugin tree.
    Ambiguous,
}

/// Classify the live plugin directory without mutating it.
pub fn classify_local_plugin(plugin_root: &Path) -> LocalPluginClass {
    if !plugin_root.exists() {
        return LocalPluginClass::Absent;
    }
    if !plugin_root.is_dir() {
        return LocalPluginClass::Ambiguous;
    }
    let has_receipt = plugin_root.join("bundle.json").is_file();
    let has_manifest = plugin_root.join("manifest.json").is_file();
    let has_helper =
        plugin_root.join("bin/agent-bar").is_file() || plugin_root.join("agent-bar").is_file();
    if has_receipt {
        match BundleValidator::validate_tree(plugin_root) {
            Ok(_) => LocalPluginClass::OwnedCurrent,
            Err(_) => LocalPluginClass::Modified,
        }
    } else if has_manifest || has_helper {
        LocalPluginClass::V9Structural
    } else {
        LocalPluginClass::Ambiguous
    }
}

/// Result of pre-update local-root preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPluginPrep {
    pub class: LocalPluginClass,
    pub is_fresh_install: bool,
    pub is_v9_rollback: bool,
    /// Durable backup of a modified accepted tree, when created.
    pub modified_backup: Option<PathBuf>,
}

/// Gate modified/ambiguous roots (BUNDLE-029). Modified trees are preserved in
/// durable backup before the caller may replace them; ambiguous roots refuse.
pub fn prepare_local_plugin_for_update(
    paths: &PluginPaths,
    txid: &str,
) -> Result<LocalPluginPrep, MaintenanceError> {
    validate_txid(txid)?;
    let class = classify_local_plugin(&paths.plugin_root);
    match class {
        LocalPluginClass::Absent => Ok(LocalPluginPrep {
            class,
            is_fresh_install: true,
            is_v9_rollback: false,
            modified_backup: None,
        }),
        LocalPluginClass::OwnedCurrent => Ok(LocalPluginPrep {
            class,
            is_fresh_install: false,
            is_v9_rollback: false,
            modified_backup: None,
        }),
        LocalPluginClass::V9Structural => Ok(LocalPluginPrep {
            class,
            is_fresh_install: false,
            is_v9_rollback: true,
            modified_backup: None,
        }),
        LocalPluginClass::Modified => {
            // Preserve modified accepted tree before replacement (BUNDLE-029/032K).
            fs::create_dir_all(&paths.backups_dir)?;
            let dest = paths.backup_root(&format!("modified-pre-update-{txid}"));
            if dest.exists() {
                fs::remove_dir_all(&dest)?;
            }
            copy_dir_all(&paths.plugin_root, &dest).map_err(|e| {
                MaintenanceError::msg(format!(
                    "failed to back up modified plugin tree to {}: {e}",
                    dest.display()
                ))
            })?;
            let report = DurableReport {
                txid: txid.to_string(),
                ok: true,
                rolled_back: false,
                residual_paths: vec![dest.display().to_string()],
                message: "modified local bundle preserved in durable backup before update".into(),
            };
            write_durable_report(paths, &report)?;
            Ok(LocalPluginPrep {
                class,
                is_fresh_install: false,
                is_v9_rollback: false,
                modified_backup: Some(dest),
            })
        }
        LocalPluginClass::Ambiguous => Err(MaintenanceError::msg(
            "refuse to replace ambiguous plugin directory without ownership proof (BUNDLE-029)",
        )),
    }
}

/// Single-shot preflight health probe for an existing v10 install (before download).
pub fn preflight_existing_health<R: CommandRunner>(
    runner: &R,
    shell_program: &str,
    expected_version: &str,
) -> Result<(), MaintenanceError> {
    require_absolute_executable(shell_program)?;
    let args = health_argv(expected_version);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = runner.run(shell_program, &arg_refs)?;
    if health_is_ok(&out) {
        return Ok(());
    }
    Err(MaintenanceError::msg(format!(
        "existing Agent Bar health endpoint failed before download/swap (stdout={:?} code={})",
        out.stdout, out.code
    )))
}

/// High-level maintenance coordinator used by CLI uninstall. `update apply`
/// (git-plugin-distribution Task 2) no longer routes through a journal/worker
/// handoff at all — it delegates straight to the omarchy CLI.
pub struct MaintenanceWorker;

impl MaintenanceWorker {
    /// Worker entry: load journal, exchange, rescan, health, commit/rollback.
    ///
    /// Holds exclusive maintenance gate for the full mutation/rollback window.
    #[allow(clippy::too_many_arguments)]
    pub fn run_worker_from_journal<R: CommandRunner, S: Sleeper>(
        paths: &PluginPaths,
        runner: &R,
        txid: &str,
        sleeper: &S,
        start: Duration,
        now: &dyn Fn() -> Duration,
        fail: Option<WorkerFailPoint>,
    ) -> Result<(), MaintenanceError> {
        validate_txid(txid)?;
        // Exclusive barrier for live mutation (ARCH-026).
        let gate = MaintenanceGate::open(&paths.maintenance_lock)
            .map_err(|e| MaintenanceError::msg(format!("open maintenance lock: {e}")))?;
        let _exclusive = gate
            .lock_exclusive()
            .map_err(|e| MaintenanceError::msg(format!("exclusive maintenance lock: {e}")))?;

        let deadlines = WorkerDeadlines::from_start(start);
        let journal_path = paths.journal_path(txid)?;
        let journal = TransactionJournal::read_from(&journal_path)
            .map_err(|e| MaintenanceError::msg(format!("journal read: {e}")))?;

        let payload = journal
            .entries
            .iter()
            .rev()
            .find(|e| e.step == TxStep::Stage)
            .ok_or_else(|| MaintenanceError::msg("journal missing stage payload"))?;
        let payload: MaintenanceJournalPayload = serde_json::from_str(&payload.detail)?;
        require_absolute_executable(&payload.omarchy_bin)?;
        require_absolute_executable(&payload.omarchy_shell_bin)?;

        match payload.operation {
            // No handoff ever writes this journal shape anymore: `update apply`
            // (git-plugin-distribution Task 2) delegates to the omarchy CLI
            // directly instead of staging a worker transaction. Kept as a typed,
            // fail-closed branch rather than deleting the variant so a payload
            // that somehow claims Update cannot silently fall through.
            MaintenanceOp::Update => Err(MaintenanceError::msg(
                "update worker journals are retired; 'update apply' delegates to \
                 the omarchy CLI directly",
            )),
            MaintenanceOp::Uninstall => Self::worker_uninstall(
                paths,
                runner,
                &payload,
                &journal_path,
                sleeper,
                &deadlines,
                now,
                fail,
            ),
        }
    }

    /// Preflight + handoff for uninstall (standard or purge).
    ///
    /// Holds exclusive maintenance gate for preflight, journal write, and unit start.
    #[allow(clippy::too_many_arguments)]
    pub fn handoff_uninstall<R: CommandRunner>(
        paths: &PluginPaths,
        runner: &R,
        current_exe: &Path,
        txid: &str,
        payload: &MaintenanceJournalPayload,
        env_pairs: &[(String, String)],
        systemd_program: &str,
        systemctl_program: &str,
    ) -> Result<String, MaintenanceError> {
        validate_txid(txid)?;
        if payload.txid != txid {
            return Err(MaintenanceError::msg("payload txid mismatch"));
        }
        if payload.operation != MaintenanceOp::Uninstall {
            return Err(MaintenanceError::msg(
                "handoff_uninstall requires uninstall operation",
            ));
        }

        let gate = MaintenanceGate::open(&paths.maintenance_lock)
            .map_err(|e| MaintenanceError::msg(format!("open maintenance lock: {e}")))?;
        let _exclusive = gate
            .lock_exclusive()
            .map_err(|e| MaintenanceError::msg(format!("exclusive maintenance lock: {e}")))?;

        require_absolute_executable(&payload.omarchy_bin)?;
        require_absolute_executable(&payload.omarchy_shell_bin)?;
        require_absolute_executable(systemd_program)?;
        require_absolute_executable(systemctl_program)?;

        let ping = runner.run(&payload.omarchy_shell_bin, &["shell", "ping"])?;
        if ping.code != 0 {
            return Err(MaintenanceError::msg("shell ping failed during preflight"));
        }
        let user = runner.run(systemctl_program, &["--user", "is-system-running"])?;
        let state = user.stdout.trim();
        if user.code != 0 && state != "running" && state != "degraded" && state != "starting" {
            return Err(MaintenanceError::msg(
                "user systemd manager is not reachable",
            ));
        }

        fs::create_dir_all(&paths.transactions_dir)?;
        fs::create_dir_all(&paths.reports_dir)?;

        let worker = install_worker_copy(current_exe, &paths.transactions_dir)?;
        let journal_path = paths.journal_path(txid)?;
        let mut journal = TransactionJournal::new(txid, "uninstall");
        journal.record(TxStep::Preflight, "worker copy verified; shell ping ok");
        let payload_json = serde_json::to_string(payload)?;
        journal.record(TxStep::Stage, payload_json);
        journal.write_to(&journal_path)?;

        let unit = maintenance_unit_name(txid)?;
        let argv = systemd_run_argv(&unit, &worker, txid, env_pairs);
        let arg_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let out = runner.run(systemd_program, &arg_refs)?;
        if out.code != 0 {
            return Err(MaintenanceError::msg(format!(
                "failed to start maintenance unit: {}",
                out.stderr.trim()
            )));
        }
        Ok(unit)
    }

    /// Uninstall worker: quarantine-first removal, shell exact-ID strip, absence
    /// poll, fsynced commit, post-commit GC (BUNDLE-033..038C).
    #[allow(clippy::too_many_arguments)]
    fn worker_uninstall<R: CommandRunner, S: Sleeper>(
        paths: &PluginPaths,
        runner: &R,
        payload: &MaintenanceJournalPayload,
        journal_path: &Path,
        sleeper: &S,
        deadlines: &WorkerDeadlines,
        now: &dyn Fn() -> Duration,
        fail: Option<WorkerFailPoint>,
    ) -> Result<(), MaintenanceError> {
        let shell_program = payload.omarchy_shell_bin.as_str();
        let mut journal = TransactionJournal::read_from(journal_path)?;
        let mut residual: Vec<String> = Vec::new();
        let mut retained_ambiguous: Vec<String> = Vec::new();

        // Track quarantine destinations for rollback / GC.
        let mut state = UninstallQuarantineState::default();

        deadlines.may_begin_mutation(now())?;

        // --- Reversible phase ---
        if let Err(err) = Self::uninstall_reversible(
            paths,
            runner,
            payload,
            journal_path,
            &mut journal,
            sleeper,
            now,
            fail,
            &mut state,
            &mut retained_ambiguous,
        ) {
            let rb = Self::rollback_uninstall(
                paths,
                runner,
                shell_program,
                payload,
                journal_path,
                &mut journal,
                sleeper,
                now,
                &state,
            );
            residual.extend(retained_ambiguous);
            let report = DurableReport {
                txid: payload.txid.clone(),
                ok: false,
                rolled_back: rb.is_ok(),
                residual_paths: residual,
                message: format!("uninstall rolled back: {err}"),
            };
            let _ = write_durable_report(paths, &report);
            return Err(err);
        }

        // --- Irreversible commit boundary (BUNDLE-038B) ---
        if let Some(WorkerFailPoint::AtCommitFsync) = fail {
            let rb = Self::rollback_uninstall(
                paths,
                runner,
                shell_program,
                payload,
                journal_path,
                &mut journal,
                sleeper,
                now,
                &state,
            );
            let report = DurableReport {
                txid: payload.txid.clone(),
                ok: false,
                rolled_back: rb.is_ok(),
                residual_paths: retained_ambiguous,
                message: "injected failure at commit fsync".into(),
            };
            let _ = write_durable_report(paths, &report);
            return Err(MaintenanceError::msg("injected failure at commit fsync"));
        }

        journal.record(TxStep::Commit, "uninstall committed");
        journal.write_to(journal_path)?;

        // Post-commit GC — never claims rollback (BUNDLE-038B/C).
        let mut gc_residual = Self::uninstall_post_commit_gc(paths, payload, &state, fail);
        residual.append(&mut gc_residual);
        residual.extend(retained_ambiguous);

        // Successful cleanup removes worker copy + journal last (BUNDLE-038C).
        if residual.is_empty() {
            let worker = paths.transactions_dir.join(MAINTENANCE_WORKER_NAME);
            let _ = fs::remove_file(&worker);
            let _ = fs::remove_file(journal_path);
        }

        let message = if residual.is_empty() {
            "uninstall committed".to_string()
        } else {
            format!(
                "uninstall committed with residual paths: {}",
                residual.join(", ")
            )
        };
        let report = DurableReport {
            txid: payload.txid.clone(),
            ok: true,
            rolled_back: false,
            residual_paths: residual,
            message: message.clone(),
        };
        write_durable_report(paths, &report)?;

        // Desktop notification after UI is gone (BUNDLE-038) — best-effort.
        let _ = notify_uninstall_complete(payload.purge_settings_and_backups, &message);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn uninstall_reversible<R: CommandRunner, S: Sleeper>(
        paths: &PluginPaths,
        runner: &R,
        payload: &MaintenanceJournalPayload,
        journal_path: &Path,
        journal: &mut TransactionJournal,
        sleeper: &S,
        now: &dyn Fn() -> Duration,
        fail: Option<WorkerFailPoint>,
        state: &mut UninstallQuarantineState,
        retained_ambiguous: &mut Vec<String>,
    ) -> Result<(), MaintenanceError> {
        let shell_path = PathBuf::from(&payload.shell_json_path);
        let plugin_root = PathBuf::from(&payload.plugin_root);
        let quarantine = PathBuf::from(&payload.quarantine_path);
        let cache_root = PathBuf::from(&payload.cache_root);
        let settings_path = PathBuf::from(&payload.settings_path);
        let backups_dir = PathBuf::from(&payload.backups_dir);

        if let Some(WorkerFailPoint::BeforeShellBackup) = fail {
            return Err(MaintenanceError::msg(
                "injected failure before shell backup",
            ));
        }
        if let Some(WorkerFailPoint::BeforeMutation) = fail {
            return Err(MaintenanceError::msg("injected failure before mutation"));
        }

        // 1) Backup exact shell bytes.
        let shell_bak = paths
            .transactions_dir
            .join(format!("{}.shell.json.bak", payload.txid));
        if shell_path.is_file() {
            let bytes = fs::read(&shell_path)?;
            fs::write(&shell_bak, &bytes)?;
            if let Ok(f) = OpenOptions::new().write(true).open(&shell_bak) {
                let _ = f.sync_all();
            }
            state.shell_backup = Some(shell_bak.clone());
            state.shell_before = Some(bytes);
            journal.record(
                TxStep::Backup,
                format!("shell backup at {}", shell_bak.display()),
            );
        } else {
            journal.record(TxStep::Backup, "shell.json absent before uninstall");
        }
        journal.write_to(journal_path)?;

        if let Some(WorkerFailPoint::AfterShellBackup) = fail {
            return Err(MaintenanceError::msg("injected failure after shell backup"));
        }

        // 2) Quarantine bundle (same-filesystem rename) — BUNDLE-038A.
        if let Some(WorkerFailPoint::AtQuarantineRename) = fail {
            return Err(MaintenanceError::msg(
                "injected failure at quarantine rename",
            ));
        }
        if plugin_root.exists() {
            quarantine_rename(&plugin_root, &quarantine)?;
            state.plugin_quarantine = Some(quarantine.clone());
            journal.record(
                TxStep::Exchange,
                format!("plugin quarantined at {}", quarantine.display()),
            );
            journal.write_to(journal_path)?;
        }

        // 3) Quarantine cache (both standard and purge).
        if !payload.cache_root.is_empty() && cache_root.exists() {
            let dest = PluginPaths::cache_quarantine(&cache_root, &payload.txid)?;
            quarantine_rename(&cache_root, &dest)?;
            state.cache_quarantine = Some(dest);
        }

        // 4) Confirmed owned legacy → quarantine under transactions (never auto-delete
        // ambiguous/modified). Paths retained appear in the completion report.
        let rules = crate::plugin::doctor::default_ownership_rules(&paths.home);
        let legacy_scan = crate::plugin::doctor::doctor_scan(&paths.home, &[], &rules);
        for path in legacy_scan.retained {
            retained_ambiguous.push(path.display().to_string());
        }
        for path in legacy_scan.removable {
            if !path.exists() {
                continue;
            }
            let dest = paths.transactions_dir.join(format!(
                "{}.legacy-{}",
                payload.txid,
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("artifact")
            ));
            if let Err(err) = quarantine_rename(&path, &dest) {
                // Best-effort: leave residual note; do not abort if single legacy fails
                // after bundle quarantine — treat as reversible-phase error.
                return Err(MaintenanceError::msg(format!(
                    "legacy quarantine failed for {}: {err}",
                    path.display()
                )));
            }
            state.legacy_quarantines.push((path, dest));
        }

        // 5) Purge paths (settings + backups) — destination-local quarantine only.
        if payload.purge_settings_and_backups {
            if let Some(WorkerFailPoint::AtSettingsPurgeQuarantine) = fail {
                return Err(MaintenanceError::msg(
                    "injected failure at settings purge quarantine",
                ));
            }
            if !payload.settings_path.is_empty() && settings_path.exists() {
                let dest = PluginPaths::settings_quarantine(&settings_path, &payload.txid)?;
                quarantine_rename(&settings_path, &dest)?;
                state.settings_quarantine = Some((settings_path.clone(), dest));
            }
            if let Some(WorkerFailPoint::AtBackupsPurgeQuarantine) = fail {
                return Err(MaintenanceError::msg(
                    "injected failure at backups purge quarantine",
                ));
            }
            if !payload.backups_dir.is_empty() && backups_dir.exists() {
                let dest = PluginPaths::backups_quarantine(&backups_dir, &payload.txid)?;
                quarantine_rename(&backups_dir, &dest)?;
                state.backups_quarantine = Some((backups_dir.clone(), dest));
            }
        }

        // 6) Exact-ID shell entry removal.
        if let Some(WorkerFailPoint::AtExactIdRemoval) = fail {
            return Err(MaintenanceError::msg(
                "injected failure at exact-ID removal",
            ));
        }
        if shell_path.is_file() {
            let current = fs::read(&shell_path)?;
            let stripped = remove_exact_plugin_entries(&current)?;
            atomic_write_bytes(&shell_path, &stripped)?;
            journal.record(TxStep::Stage, "exact agent-bar.usage shell entries removed");
            journal.write_to(journal_path)?;
        }

        // 7) Rescan.
        if let Some(WorkerFailPoint::AtRescan) = fail {
            return Err(MaintenanceError::msg("injected failure at rescan"));
        }
        let rescan = runner.run(&payload.omarchy_bin, &["plugin", "rescan"])?;
        if rescan.code != 0 {
            return Err(MaintenanceError::msg("rescan exit non-zero"));
        }
        journal.record(TxStep::Rescan, "plugin rescan issued");
        journal.write_to(journal_path)?;

        // 8) Absence poll (listPlugins + old health failure).
        if let Some(WorkerFailPoint::AtAbsenceCheck) = fail {
            return Err(MaintenanceError::msg("injected failure at absence check"));
        }
        if let Some(WorkerFailPoint::AtHealth) = fail {
            return Err(MaintenanceError::msg("injected health/absence failure"));
        }
        poll_uninstall_absence(
            runner,
            payload.omarchy_shell_bin.as_str(),
            payload.previous_version.as_deref(),
            sleeper,
            now(),
            now,
        )?;
        journal.record(TxStep::Health, "listPlugins absence verified");
        journal.write_to(journal_path)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn rollback_uninstall<R: CommandRunner, S: Sleeper>(
        paths: &PluginPaths,
        runner: &R,
        shell_program: &str,
        payload: &MaintenanceJournalPayload,
        journal_path: &Path,
        journal: &mut TransactionJournal,
        sleeper: &S,
        now: &dyn Fn() -> Duration,
        state: &UninstallQuarantineState,
    ) -> Result<(), MaintenanceError> {
        // Restore shell bytes first (exact previous).
        if let (Some(before), true) = (
            state.shell_before.as_ref(),
            !payload.shell_json_path.is_empty(),
        ) {
            let shell_path = PathBuf::from(&payload.shell_json_path);
            if let Some(parent) = shell_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            atomic_write_bytes(&shell_path, before)?;
        }

        // Restore plugin bundle from quarantine.
        if let Some(q) = state.plugin_quarantine.as_ref() {
            let target = PathBuf::from(&payload.plugin_root);
            if target.exists() {
                let _ = fs::remove_dir_all(&target);
            }
            if q.exists() {
                quarantine_rename(q, &target)?;
            }
        }

        // Restore cache.
        if let Some(q) = state.cache_quarantine.as_ref() {
            let target = PathBuf::from(&payload.cache_root);
            if q.exists() {
                if target.exists() {
                    let _ = fs::remove_dir_all(&target);
                }
                let _ = quarantine_rename(q, &target);
            }
        }

        // Restore purge quarantines.
        if let Some((orig, q)) = state.settings_quarantine.as_ref() {
            if q.exists() {
                let _ = quarantine_rename(q, orig);
            }
        }
        if let Some((orig, q)) = state.backups_quarantine.as_ref() {
            if q.exists() {
                let _ = quarantine_rename(q, orig);
            }
        }
        for (orig, q) in &state.legacy_quarantines {
            if q.exists() {
                let _ = quarantine_rename(q, orig);
            }
        }

        journal.record(TxStep::Rollback, "restored quarantines and shell bytes");
        let _ = journal.write_to(journal_path);

        let _ = runner.run(&payload.omarchy_bin, &["plugin", "rescan"]);
        // Verify old service health when previous version known.
        if let Some(prev) = payload.previous_version.as_deref() {
            poll_update_health(runner, shell_program, prev, sleeper, now(), now)?;
        }
        let _ = paths;
        Ok(())
    }

    fn uninstall_post_commit_gc(
        paths: &PluginPaths,
        payload: &MaintenanceJournalPayload,
        state: &UninstallQuarantineState,
        fail: Option<WorkerFailPoint>,
    ) -> Vec<String> {
        let mut residual = Vec::new();
        if let Some(WorkerFailPoint::AtPostCommitGc) = fail {
            if let Some(q) = state.plugin_quarantine.as_ref() {
                residual.push(q.display().to_string());
            }
            residual.push("injected post-commit GC failure".into());
            return residual;
        }

        let try_rm = |p: &Path, residual: &mut Vec<String>| {
            if !p.exists() {
                return;
            }
            let result = if p.is_dir() {
                fs::remove_dir_all(p)
            } else {
                fs::remove_file(p)
            };
            if result.is_err() {
                residual.push(p.display().to_string());
            }
        };

        if let Some(q) = state.plugin_quarantine.as_ref() {
            try_rm(q, &mut residual);
        }
        if let Some(q) = state.cache_quarantine.as_ref() {
            try_rm(q, &mut residual);
        }
        if let Some((_, q)) = state.settings_quarantine.as_ref() {
            try_rm(q, &mut residual);
        }
        if let Some((_, q)) = state.backups_quarantine.as_ref() {
            try_rm(q, &mut residual);
        }
        for (_, q) in &state.legacy_quarantines {
            try_rm(q, &mut residual);
        }
        if let Some(bak) = state.shell_backup.as_ref() {
            try_rm(bak, &mut residual);
        }
        let _ = (paths, payload);
        residual
    }
}

#[derive(Debug, Default)]
struct UninstallQuarantineState {
    shell_backup: Option<PathBuf>,
    shell_before: Option<Vec<u8>>,
    plugin_quarantine: Option<PathBuf>,
    cache_quarantine: Option<PathBuf>,
    settings_quarantine: Option<(PathBuf, PathBuf)>,
    backups_quarantine: Option<(PathBuf, PathBuf)>,
    legacy_quarantines: Vec<(PathBuf, PathBuf)>,
}

/// Poll until `listPlugins` shows absence of `agent-bar.usage` and the previous
/// service health endpoint fails/is absent (BUNDLE-032F / 032G uninstall path).
pub fn poll_uninstall_absence<R: CommandRunner, S: Sleeper>(
    runner: &R,
    shell_program: &str,
    previous_version: Option<&str>,
    sleeper: &S,
    start: Duration,
    now: &dyn Fn() -> Duration,
) -> Result<(), MaintenanceError> {
    let deadline = start + RESCAN_POLL_DEADLINE;
    let mut delays = rescan_poll_delays();
    loop {
        let list = runner.run(shell_program, &["shell", "listPlugins"])?;
        if list.code != 0 {
            let t = now();
            if t >= deadline {
                return Err(MaintenanceError::msg(format!(
                    "listPlugins failed during absence poll: code={}",
                    list.code
                )));
            }
        } else {
            let absent = list_plugins_absent(&list.stdout)?;
            let health_gone = match previous_version {
                Some(ver) => {
                    let args = health_argv(ver);
                    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                    match runner.run(shell_program, &arg_refs) {
                        Ok(out) => !health_is_ok(&out),
                        Err(_) => true,
                    }
                }
                None => true,
            };
            if absent && health_gone {
                return Ok(());
            }
        }
        let t = now();
        if t >= deadline {
            return Err(MaintenanceError::msg(
                "absence poll timeout: plugin still present or health still ok",
            ));
        }
        let delay = delays.next().unwrap_or(Duration::from_millis(500));
        let remaining = deadline.saturating_sub(t);
        sleeper.sleep(delay.min(remaining));
    }
}

/// Best-effort desktop notification after successful uninstall (BUNDLE-038).
pub fn notify_uninstall_complete(purged: bool, detail: &str) -> Result<(), MaintenanceError> {
    let body = if purged {
        format!("Agent Bar uninstalled (settings and backups removed). {detail}")
    } else {
        format!("Agent Bar uninstalled (settings preserved). {detail}")
    };
    let body = crate::support::redact::strip_ansi_and_controls(&body);
    let status = std::process::Command::new("notify-send")
        .args([
            "--app-name=Agent Bar",
            "--urgency=normal",
            "Agent Bar uninstalled",
            &body,
        ])
        .status()
        .map_err(|e| MaintenanceError::msg(format!("notify-send spawn failed: {e}")))?;
    if !status.success() {
        return Err(MaintenanceError::msg(format!(
            "notify-send exited {:?}",
            status.code()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerFailPoint {
    BeforeMutation,
    AtHealth,
    // --- Uninstall fault matrix (BUNDLE-037 / 038A–C) ---
    BeforeShellBackup,
    AfterShellBackup,
    AtQuarantineRename,
    AtExactIdRemoval,
    AtRescan,
    AtAbsenceCheck,
    AtCommitFsync,
    AtSettingsPurgeQuarantine,
    AtBackupsPurgeQuarantine,
    /// Post-commit GC: durable residual, never rollback.
    AtPostCommitGc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DurableReport {
    pub txid: String,
    pub ok: bool,
    pub rolled_back: bool,
    pub residual_paths: Vec<String>,
    pub message: String,
}

fn write_durable_report(
    paths: &PluginPaths,
    report: &DurableReport,
) -> Result<(), MaintenanceError> {
    fs::create_dir_all(&paths.reports_dir)?;
    let path = paths.reports_dir.join(format!("{}.json", report.txid));
    let json = serde_json::to_vec_pretty(report)?;
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path)?;
    f.write_all(&json)?;
    f.sync_all()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::bundle::BundleBuilder;
    use crate::plugin::omarchy::RecordingRunner;
    use crate::support::Clock;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    const ZERO_COMMIT: &str = "0000000000000000000000000000000000000000";

    struct FixedClock(OffsetDateTime);
    impl Clock for FixedClock {
        fn now_utc(&self) -> OffsetDateTime {
            self.0
        }
    }

    fn sample_compatible() -> UpdateCompatible {
        UpdateCompatible {
            version: "10.1.0".into(),
            omarchy_contract: 1,
            minimum_quickshell_version: MINIMUM_QUICKSHELL_VERSION.into(),
            release_notes_url: "https://github.com/othavi0/agent-bar/releases/tag/v10.1.0".into(),
        }
    }

    /// A `bundle.json`-shaped dist receipt (BUNDLE-012 producer shape,
    /// including fields `update check` does not read) so parsing coverage
    /// matches what `BundleBuilder` actually emits.
    fn receipt_json(version: &str, minimum_quickshell_version: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "pluginId": PLUGIN_ID,
            "version": version,
            "target": OFFICIAL_TARGET,
            "omarchyContract": OMARCHY_CONTRACT,
            "minimumQuickshellVersion": minimum_quickshell_version,
            "sourceCommit": ZERO_COMMIT,
            "files": [],
        }))
        .unwrap()
    }

    #[test]
    fn update_check_literal_shape() {
        let doc = UpdateCheckDocument {
            schema_version: 1,
            checked_at: "2026-07-26T18:42:00Z".into(),
            current: UpdateCurrent {
                version: "10.0.0".into(),
                target: OFFICIAL_TARGET.into(),
                omarchy_contract: 1,
                quickshell_version: "0.3.0".into(),
            },
            available: true,
            reinstall_required: false,
            latest_compatible: Some(sample_compatible()),
        };
        doc.validate().unwrap();
        let json = doc.to_stdout_json().unwrap();
        assert!(json.ends_with('\n'));
        assert!(json.contains("\"schemaVersion\":1") || json.contains("\"schemaVersion\": 1"));
        assert!(json.contains("\"reinstallRequired\":false"));
        let parsed = UpdateCheckDocument::parse_json(json.trim_end().as_bytes()).unwrap();
        assert!(parsed.available);
        // Unknown fields rejected.
        let mut v = serde_json::to_value(&doc).unwrap();
        v.as_object_mut()
            .unwrap()
            .insert("extra".into(), serde_json::json!(true));
        assert!(UpdateCheckDocument::parse_json(&serde_json::to_vec(&v).unwrap()).is_err());
    }

    #[test]
    fn update_check_available_false_when_same_version() {
        let mut c = sample_compatible();
        c.version = "10.0.0".into();
        let doc = UpdateCheckDocument {
            schema_version: 1,
            checked_at: "2026-07-26T18:42:00Z".into(),
            current: UpdateCurrent {
                version: "10.0.0".into(),
                target: OFFICIAL_TARGET.into(),
                omarchy_contract: 1,
                quickshell_version: "0.3.0".into(),
            },
            available: false,
            reinstall_required: false,
            latest_compatible: Some(c),
        };
        doc.validate().unwrap();
        let mut bad = doc.clone();
        bad.available = true;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn reinstall_required_document_forbids_an_offer() {
        let mut doc = UpdateCheckDocument {
            schema_version: 1,
            checked_at: "2026-07-26T18:42:00Z".into(),
            current: UpdateCurrent {
                version: "10.0.0".into(),
                target: OFFICIAL_TARGET.into(),
                omarchy_contract: 1,
                quickshell_version: "0.3.0".into(),
            },
            available: false,
            reinstall_required: true,
            latest_compatible: None,
        };
        doc.validate().unwrap();

        let mut with_latest = doc.clone();
        with_latest.latest_compatible = Some(sample_compatible());
        assert!(with_latest.validate().is_err());

        doc.available = true;
        assert!(doc.validate().is_err());
    }

    #[test]
    fn credentials_rejected_on_download() {
        let http = ScriptedReleaseHttp::with_responses(vec![]);
        let err = http
            .get(DIST_RECEIPT_URL, &[("Authorization", "Bearer secret")])
            .unwrap_err();
        assert!(err.to_string().contains("credentials"));
    }

    #[test]
    fn update_check_available_when_receipt_newer() {
        let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
            status: 200,
            headers: vec![],
            body: receipt_json("10.1.0", MINIMUM_QUICKSHELL_VERSION),
        })]);
        let clock = FixedClock(OffsetDateTime::parse("2026-07-26T18:42:00Z", &Rfc3339).unwrap());
        let probe = UpdateCheckProbe {
            current_version: "10.0.0".into(),
            quickshell_version: "0.3.0".into(),
            target: OFFICIAL_TARGET.into(),
            omarchy_contract: OMARCHY_CONTRACT,
        };
        let doc = UpdateCheck::run(&http, &clock, &probe, false).unwrap();
        assert!(doc.available);
        assert!(!doc.reinstall_required);
        let latest = doc.latest_compatible.expect("newer receipt names a target");
        assert_eq!(latest.version, "10.1.0");
        assert_eq!(
            latest.release_notes_url,
            "https://github.com/othavi0/agent-bar/releases/tag/v10.1.0"
        );

        // Exactly one GET, to the dist receipt URL with the expected headers.
        let calls = http.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, DIST_RECEIPT_URL);
        assert!(calls[0]
            .1
            .iter()
            .any(|(k, v)| k == "Accept" && v == "application/json"));
    }

    #[test]
    fn update_check_up_to_date_when_receipt_equal() {
        let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
            status: 200,
            headers: vec![],
            body: receipt_json("10.0.0", MINIMUM_QUICKSHELL_VERSION),
        })]);
        let clock = FixedClock(OffsetDateTime::now_utc());
        let probe = UpdateCheckProbe {
            current_version: "10.0.0".into(),
            ..UpdateCheckProbe::default()
        };
        let doc = UpdateCheck::run(&http, &clock, &probe, false).unwrap();
        assert!(!doc.available);
        let latest = doc
            .latest_compatible
            .expect("still names the newest compatible receipt");
        assert_eq!(latest.version, "10.0.0");
    }

    #[test]
    fn update_check_incompatible_quickshell_yields_no_offer() {
        let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
            status: 200,
            headers: vec![],
            body: receipt_json("10.1.0", "99.0.0"),
        })]);
        let clock = FixedClock(OffsetDateTime::now_utc());
        let probe = UpdateCheckProbe {
            current_version: "10.0.0".into(),
            quickshell_version: "0.3.0".into(),
            ..UpdateCheckProbe::default()
        };
        // Locally incompatible is not an error — just nothing to offer.
        let doc = UpdateCheck::run(&http, &clock, &probe, false).unwrap();
        assert!(!doc.available);
        assert!(doc.latest_compatible.is_none());
    }

    #[test]
    fn update_check_reinstall_required_forces_null_offer() {
        // Even a genuinely newer, compatible receipt must not surface as an
        // offer when the live plugin root is not a git checkout.
        let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
            status: 200,
            headers: vec![],
            body: receipt_json("10.1.0", MINIMUM_QUICKSHELL_VERSION),
        })]);
        let clock = FixedClock(OffsetDateTime::now_utc());
        let probe = UpdateCheckProbe {
            current_version: "10.0.0".into(),
            quickshell_version: "0.3.0".into(),
            ..UpdateCheckProbe::default()
        };
        let doc = UpdateCheck::run(&http, &clock, &probe, true).unwrap();
        assert!(doc.reinstall_required);
        assert!(!doc.available);
        assert!(doc.latest_compatible.is_none());
        doc.validate().unwrap();
    }

    #[test]
    fn update_check_rejects_receipt_identity_mismatches() {
        let clock = FixedClock(OffsetDateTime::now_utc());
        let probe = UpdateCheckProbe::default();
        let cases: [(&str, serde_json::Value); 4] = [
            (
                "schemaVersion",
                serde_json::json!({
                    "schemaVersion": 2, "pluginId": PLUGIN_ID, "version": "10.1.0",
                    "target": OFFICIAL_TARGET, "omarchyContract": OMARCHY_CONTRACT,
                    "minimumQuickshellVersion": MINIMUM_QUICKSHELL_VERSION,
                }),
            ),
            (
                "pluginId",
                serde_json::json!({
                    "schemaVersion": 1, "pluginId": "some.other.plugin", "version": "10.1.0",
                    "target": OFFICIAL_TARGET, "omarchyContract": OMARCHY_CONTRACT,
                    "minimumQuickshellVersion": MINIMUM_QUICKSHELL_VERSION,
                }),
            ),
            (
                "target",
                serde_json::json!({
                    "schemaVersion": 1, "pluginId": PLUGIN_ID, "version": "10.1.0",
                    "target": "aarch64-unknown-linux-gnu", "omarchyContract": OMARCHY_CONTRACT,
                    "minimumQuickshellVersion": MINIMUM_QUICKSHELL_VERSION,
                }),
            ),
            (
                "omarchyContract",
                serde_json::json!({
                    "schemaVersion": 1, "pluginId": PLUGIN_ID, "version": "10.1.0",
                    "target": OFFICIAL_TARGET, "omarchyContract": 2,
                    "minimumQuickshellVersion": MINIMUM_QUICKSHELL_VERSION,
                }),
            ),
        ];
        for (needle, body) in cases {
            let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
                status: 200,
                headers: vec![],
                body: serde_json::to_vec(&body).unwrap(),
            })]);
            let err = UpdateCheck::run(&http, &clock, &probe, false).unwrap_err();
            assert!(err.to_string().contains(needle), "{needle}: {err}");
        }
    }

    #[test]
    fn update_check_rejects_non_200_and_malformed_body() {
        let clock = FixedClock(OffsetDateTime::now_utc());
        let probe = UpdateCheckProbe::default();

        let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
            status: 404,
            headers: vec![],
            body: vec![],
        })]);
        let err = UpdateCheck::run(&http, &clock, &probe, false).unwrap_err();
        assert!(err.to_string().contains("404"), "{err}");

        let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
            status: 200,
            headers: vec![],
            body: b"not json".to_vec(),
        })]);
        let err = UpdateCheck::run(&http, &clock, &probe, false).unwrap_err();
        assert!(err.to_string().contains("malformed"), "{err}");
    }

    #[test]
    fn health_ok_and_list_plugins_parsing() {
        assert!(health_is_ok(&CommandOutput {
            code: 0,
            stdout: "ok\n".into(),
            stderr: String::new(),
        }));
        assert!(!health_is_ok(&CommandOutput {
            code: 0,
            stdout: "ok".into(),
            stderr: String::new(),
        }));
        assert!(list_plugins_absent(r#"[{"id":"other.plugin"}]"#).unwrap());
        assert!(!list_plugins_absent(r#"[{"id":"agent-bar.usage"}]"#).unwrap());
        assert!(list_plugins_has_enabled(r#"[{"id":"agent-bar.usage"}]"#).unwrap());
        assert!(list_plugins_absent("not-json").is_err());
    }

    #[test]
    fn rescan_poll_delay_sequence() {
        let d: Vec<_> = rescan_poll_delays().take(6).collect();
        assert_eq!(
            d,
            vec![
                Duration::from_millis(100),
                Duration::from_millis(200),
                Duration::from_millis(400),
                Duration::from_millis(500),
                Duration::from_millis(500),
                Duration::from_millis(500),
            ]
        );
    }

    #[test]
    fn poll_health_succeeds_and_times_out() {
        let mono = FakeMonotonic::new(0);
        let runner = RecordingRunner::default();
        // First call not ready, second ok — but we need custom responses.
        {
            let mut q = runner.responses.lock().unwrap();
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: "ok\n".into(),
                stderr: String::new(),
            }));
            q.push(Ok(CommandOutput {
                code: 1,
                stdout: "unknown\n".into(),
                stderr: String::new(),
            }));
            // pop removes from front in RecordingRunner? It uses remove(0) — FIFO.
        }
        struct AdvancingSleeper<'a>(&'a FakeMonotonic);
        impl Sleeper for AdvancingSleeper<'_> {
            fn sleep(&self, d: Duration) {
                self.0.advance(d);
            }
        }
        let sleeper = AdvancingSleeper(&mono);
        poll_update_health(
            &runner,
            "omarchy-shell",
            "10.0.0",
            &sleeper,
            Duration::ZERO,
            &|| mono.now(),
        )
        .unwrap();

        // Timeout path.
        let mono2 = FakeMonotonic::new(0);
        let runner2 = RecordingRunner::default();
        {
            // Always unknown; sleeper advances past 15s.
            // RecordingRunner returns empty-ok when exhausted — push many unknowns.
            let mut q = runner2.responses.lock().unwrap();
            for _ in 0..50 {
                q.push(Ok(CommandOutput {
                    code: 1,
                    stdout: "unknown\n".into(),
                    stderr: String::new(),
                }));
            }
        }
        let sleeper2 = AdvancingSleeper(&mono2);
        let err = poll_update_health(
            &runner2,
            "omarchy-shell",
            "10.0.0",
            &sleeper2,
            Duration::ZERO,
            &|| mono2.now(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn unit_name_and_systemd_argv_contract() {
        let txid = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            maintenance_unit_name(txid).unwrap(),
            format!("agent-bar-maintenance-{txid}.service")
        );
        assert!(maintenance_unit_name("short").is_err());

        let worker = PathBuf::from("/tmp/agent-bar-maintenance-worker");
        let env = collect_worker_env([
            ("HOME", "/home/u"),
            ("SECRET", "nope"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("EMPTY", ""),
        ]);
        assert!(env
            .iter()
            .all(|(k, _)| WORKER_ENV_ALLOWLIST.contains(&k.as_str())));
        assert!(!env.iter().any(|(k, _)| k == "SECRET"));

        let argv = systemd_run_argv(&maintenance_unit_name(txid).unwrap(), &worker, txid, &env);
        assert_eq!(argv[0], "--user");
        assert!(argv.iter().any(|a| a == "--collect"));
        assert!(argv.iter().any(|a| a == "--property=Type=exec"));
        assert!(argv.iter().any(|a| a == "--property=UMask=0077"));
        assert!(argv.iter().any(|a| a == "--property=TimeoutStartSec=120"));
        assert!(argv
            .iter()
            .any(|a| a == &format!("--property=RuntimeMaxSec={WORKER_RUNTIME_MAX_SECS}")));
        assert_eq!(argv[argv.len() - 2], worker.display().to_string());
        assert_eq!(argv[argv.len() - 1], txid);
    }

    #[test]
    fn worker_deadlines_monotonic_and_reserve() {
        let d = WorkerDeadlines::from_start(Duration::from_secs(0));
        assert_eq!(d.stage_by, Duration::from_secs(DEADLINE_STAGE_SECS));
        assert_eq!(d.mutation_by, Duration::from_secs(DEADLINE_MUTATION_SECS));
        assert_eq!(d.rollback_by, Duration::from_secs(DEADLINE_ROLLBACK_SECS));
        assert_eq!(d.hard, Duration::from_secs(WORKER_RUNTIME_MAX_SECS));
        d.may_begin_mutation(Duration::from_secs(100)).unwrap();
        assert!(d
            .may_begin_mutation(Duration::from_secs(DEADLINE_MUTATION_SECS))
            .is_err());
    }

    #[test]
    fn is_worker_exe_by_basename() {
        assert!(is_maintenance_worker_exe(Path::new(
            "/tmp/x/agent-bar-maintenance-worker"
        )));
        assert!(!is_maintenance_worker_exe(Path::new("/tmp/x/agent-bar")));
    }

    #[test]
    fn worker_copy_verifies_checksum() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("agent-bar");
        fs::write(&src, b"helper-bytes").unwrap();
        fs::set_permissions(&src, fs::Permissions::from_mode(0o755)).unwrap();
        let tx = dir.path().join("transactions");
        let dest = install_worker_copy(&src, &tx).unwrap();
        assert_eq!(dest.file_name().unwrap(), MAINTENANCE_WORKER_NAME);
        assert_eq!(fs::read(&dest).unwrap(), b"helper-bytes");
    }

    fn fake_abs_bin(dir: &Path, name: &str) -> String {
        let p = dir.join(name);
        fs::write(&p, b"#!/bin/true\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p.canonicalize().unwrap().display().to_string()
    }

    fn sample_payload(
        paths: &PluginPaths,
        txid: &str,
        expected: &str,
        previous: &str,
        tools: &Path,
    ) -> MaintenanceJournalPayload {
        MaintenanceJournalPayload {
            txid: txid.into(),
            operation: MaintenanceOp::Update,
            expected_version: Some(expected.into()),
            previous_version: Some(previous.into()),
            stage_path: paths.stage_dir(txid).unwrap().display().to_string(),
            plugin_root: paths.plugin_root.display().to_string(),
            quarantine_path: paths.quarantine_dir(txid).unwrap().display().to_string(),
            selected: None,
            omarchy_bin: fake_abs_bin(tools, "omarchy"),
            omarchy_shell_bin: fake_abs_bin(tools, "omarchy-shell"),
            is_fresh_install: false,
            is_v9_rollback: false,
            purge_settings_and_backups: false,
            shell_json_path: String::new(),
            settings_path: String::new(),
            cache_root: String::new(),
            backups_dir: String::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sample_uninstall_payload(
        paths: &PluginPaths,
        txid: &str,
        previous: &str,
        tools: &Path,
        purge: bool,
        shell_json: &Path,
        settings: &Path,
        cache_root: &Path,
    ) -> MaintenanceJournalPayload {
        MaintenanceJournalPayload {
            txid: txid.into(),
            operation: MaintenanceOp::Uninstall,
            expected_version: None,
            previous_version: Some(previous.into()),
            stage_path: String::new(),
            plugin_root: paths.plugin_root.display().to_string(),
            quarantine_path: paths.quarantine_dir(txid).unwrap().display().to_string(),
            selected: None,
            omarchy_bin: fake_abs_bin(tools, "omarchy"),
            omarchy_shell_bin: fake_abs_bin(tools, "omarchy-shell"),
            is_fresh_install: false,
            is_v9_rollback: false,
            purge_settings_and_backups: purge,
            shell_json_path: shell_json.display().to_string(),
            settings_path: settings.display().to_string(),
            cache_root: cache_root.display().to_string(),
            backups_dir: paths.backups_dir.display().to_string(),
        }
    }

    fn write_shell_with_plugin(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(
            path,
            r#"{
  "bar": {
    "left": [
      {"id": "omarchy.menu"},
      {"id": "agent-bar.usage"},
      {"id": "omarchy.workspaces"}
    ]
  }
}
"#,
        )
        .unwrap();
    }

    fn write_min_plugin(root: &Path, version: &str) {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(
            root.join("manifest.json"),
            format!(
                r#"{{"schemaVersion":1,"id":"agent-bar.usage","name":"Agent Bar","version":"{version}","author":"othavi0","license":"MIT","description":"x","kinds":["service","bar-widget"],"entryPoints":{{"service":"Service.qml","barWidget":"BarWidget.qml"}},"barWidget":{{"displayName":"Agent Bar","description":"x","category":"AI","aliases":["agent-bar"],"allowMultiple":false,"defaults":{{}},"schema":[]}}}}"#
            ),
        )
        .unwrap();
        fs::write(root.join("Service.qml"), format!("// {version}\n")).unwrap();
        fs::write(root.join("BarWidget.qml"), b"// bar\n").unwrap();
        fs::write(
            root.join("bin/agent-bar"),
            format!(
                "#!/bin/sh\nif [ \"$1\" = version ] || [ \"$1\" = --version ]; then echo {version}; fi\n"
            ),
        )
        .unwrap();
        fs::set_permissions(
            root.join("bin/agent-bar"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        fs::write(
            root.join("scripts/agent-bar-open-terminal"),
            b"#!/bin/bash\n",
        )
        .unwrap();
        fs::set_permissions(
            root.join("scripts/agent-bar-open-terminal"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        for p in ["Service.qml", "BarWidget.qml", "manifest.json"] {
            fs::set_permissions(root.join(p), fs::Permissions::from_mode(0o644)).unwrap();
        }
    }

    fn write_receipt(root: &Path, version: &str) {
        let builder = BundleBuilder::new(version, ZERO_COMMIT).unwrap();
        let receipt = BundleValidator::build_receipt(&builder, root).unwrap();
        fs::write(root.join("bundle.json"), receipt.to_pretty_json().unwrap()).unwrap();
        fs::set_permissions(root.join("bundle.json"), fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn no_global_executable_in_bundle_layout() {
        // Product is plugin-only: helper lives at bin/agent-bar inside the plugin.
        let dir = tempdir().unwrap();
        let root = dir.path().join("agent-bar.usage");
        write_min_plugin(&root, "10.0.0");
        write_receipt(&root, "10.0.0");
        assert!(root.join("bin/agent-bar").is_file());
        assert!(!root.join("agent-bar").exists());
        assert!(!dir.path().join("usr/bin/agent-bar").exists());
    }

    #[test]
    fn classify_local_plugin_owned_modified_v9_ambiguous() {
        let dir = tempdir().unwrap();
        assert_eq!(
            classify_local_plugin(&dir.path().join("missing")),
            LocalPluginClass::Absent
        );

        let owned = dir.path().join("owned");
        write_min_plugin(&owned, "10.0.0");
        write_receipt(&owned, "10.0.0");
        assert_eq!(
            classify_local_plugin(&owned),
            LocalPluginClass::OwnedCurrent
        );

        let modified = dir.path().join("modified");
        write_min_plugin(&modified, "10.0.0");
        write_receipt(&modified, "10.0.0");
        fs::write(modified.join("extra.txt"), b"local edit").unwrap();
        assert_eq!(classify_local_plugin(&modified), LocalPluginClass::Modified);

        let v9 = dir.path().join("v9");
        write_min_plugin(&v9, "9.0.0");
        // no bundle.json → structural v9
        assert_eq!(classify_local_plugin(&v9), LocalPluginClass::V9Structural);

        let amb = dir.path().join("amb");
        fs::create_dir_all(&amb).unwrap();
        fs::write(amb.join("readme.txt"), b"noise").unwrap();
        assert_eq!(classify_local_plugin(&amb), LocalPluginClass::Ambiguous);
    }

    #[test]
    fn prepare_refuses_ambiguous_and_backs_up_modified() {
        let dir = tempdir().unwrap();
        let paths = PluginPaths::with_plugins_dir(
            dir.path(),
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        fs::create_dir_all(&paths.plugins_dir).unwrap();
        fs::create_dir_all(&paths.plugin_root).unwrap();
        fs::write(paths.plugin_root.join("noise"), b"x").unwrap();
        let txid = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
        let err = prepare_local_plugin_for_update(&paths, txid).unwrap_err();
        assert!(err.to_string().contains("ambiguous"));

        // Modified: preserve then allow.
        fs::remove_dir_all(&paths.plugin_root).unwrap();
        write_min_plugin(&paths.plugin_root, "10.0.0");
        write_receipt(&paths.plugin_root, "10.0.0");
        fs::write(paths.plugin_root.join("local.patch"), b"edit").unwrap();
        let prep = prepare_local_plugin_for_update(&paths, txid).unwrap();
        assert_eq!(prep.class, LocalPluginClass::Modified);
        let backup = prep.modified_backup.expect("backup path");
        assert!(backup.join("local.patch").is_file());
        let report = fs::read_to_string(paths.reports_dir.join(format!("{txid}.json"))).unwrap();
        assert!(report.contains("modified local bundle"));
    }

    #[test]
    fn require_absolute_and_resolve_executable() {
        let dir = tempdir().unwrap();
        let bin = fake_abs_bin(dir.path(), "tool");
        require_absolute_executable(&bin).unwrap();
        assert!(require_absolute_executable("tool").is_err());
        // Absolute path that exists is accepted by resolve.
        let again = resolve_absolute_executable(&bin).unwrap();
        assert_eq!(again, bin);
    }

    #[test]
    fn worker_holds_exclusive_maintenance_gate() {
        let dir = tempdir().unwrap();
        let paths = PluginPaths::with_plugins_dir(
            dir.path(),
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        fs::create_dir_all(&paths.plugins_dir).unwrap();
        fs::create_dir_all(&paths.transactions_dir).unwrap();
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let txid = "44444444444444444444444444444444";
        let stage = paths.stage_dir(txid).unwrap();
        write_min_plugin(&stage, "10.1.0");
        write_receipt(&stage, "10.1.0");
        write_min_plugin(&paths.plugin_root, "10.0.0");
        write_receipt(&paths.plugin_root, "10.0.0");
        let payload = sample_payload(&paths, txid, "10.1.0", "10.0.0", &tools);
        let mut journal = TransactionJournal::new(txid, "update");
        journal.record(TxStep::Preflight, "ok");
        journal.record(TxStep::Stage, serde_json::to_string(&payload).unwrap());
        journal
            .write_to(&paths.journal_path(txid).unwrap())
            .unwrap();

        // Hold exclusive first so the worker cannot begin mutation.
        let gate = MaintenanceGate::open(&paths.maintenance_lock).unwrap();
        let exclusive = gate.lock_exclusive().unwrap();
        let worker_gate = MaintenanceGate::open(&paths.maintenance_lock).unwrap();
        assert!(
            worker_gate.try_lock_exclusive().unwrap().is_none(),
            "exclusive maintenance barrier must block concurrent workers"
        );
        drop(exclusive);
        // After release, try_lock succeeds (worker path can acquire).
        assert!(worker_gate.try_lock_exclusive().unwrap().is_some());
    }

    // -----------------------------------------------------------------------
    // Uninstall confirmation + fault matrix (Task 18 / BUNDLE-033..038C)
    // -----------------------------------------------------------------------

    #[test]
    fn uninstall_confirmation_accepts_exact_document() {
        let raw = br#"{
  "schemaVersion": 1,
  "operation": "uninstall",
  "confirmed": true,
  "purgeSettingsAndBackups": false
}"#;
        UninstallConfirmation::parse_strict(raw, false).unwrap();
        let purge = br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":true}"#;
        UninstallConfirmation::parse_strict(purge, true).unwrap();
    }

    #[test]
    fn uninstall_confirmation_rejects_false_mismatch_extra_and_malformed() {
        // confirmed: false
        let err = UninstallConfirmation::parse_strict(
            br#"{"schemaVersion":1,"operation":"uninstall","confirmed":false,"purgeSettingsAndBackups":false}"#,
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("confirmed"));

        // purge mismatch
        let err = UninstallConfirmation::parse_strict(
            br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":true}"#,
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("purge"));

        // trailing non-whitespace
        let err = UninstallConfirmation::parse_strict(
            br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":false} extra"#,
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("trailing") || err.to_string().contains("malformed"));

        // unknown field
        let err = UninstallConfirmation::parse_strict(
            br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":false,"extra":1}"#,
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("malformed") || err.to_string().contains("unknown"));

        // wrong operation
        let err = UninstallConfirmation::parse_strict(
            br#"{"schemaVersion":1,"operation":"update","confirmed":true,"purgeSettingsAndBackups":false}"#,
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("operation"));
    }

    fn seed_uninstall_layout(
        dir: &Path,
        paths: &PluginPaths,
        version: &str,
    ) -> (PathBuf, PathBuf, PathBuf) {
        fs::create_dir_all(&paths.plugins_dir).unwrap();
        fs::create_dir_all(&paths.transactions_dir).unwrap();
        fs::create_dir_all(&paths.reports_dir).unwrap();
        fs::create_dir_all(&paths.backups_dir).unwrap();
        write_min_plugin(&paths.plugin_root, version);
        write_receipt(&paths.plugin_root, version);

        let shell = dir.join("config/omarchy/shell.json");
        write_shell_with_plugin(&shell);

        let settings = dir.join("config/agent-bar/settings.json");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(&settings, br#"{"schemaVersion":1}"#).unwrap();

        let cache = dir.join("cache/agent-bar");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("status-v2.json"), b"{}").unwrap();
        fs::write(cache.join("notification-state-v1.json"), b"{}").unwrap();

        let bak = paths.backups_dir.join("old/plugin/marker");
        fs::create_dir_all(bak.parent().unwrap()).unwrap();
        fs::write(&bak, b"bak").unwrap();
        (shell, settings, cache)
    }

    fn run_uninstall_worker(
        paths: &PluginPaths,
        payload: &MaintenanceJournalPayload,
        runner: &RecordingRunner,
        fail: Option<WorkerFailPoint>,
    ) -> Result<(), MaintenanceError> {
        let mut journal = TransactionJournal::new(&payload.txid, "uninstall");
        journal.record(TxStep::Preflight, "ok");
        journal.record(TxStep::Stage, serde_json::to_string(payload).unwrap());
        journal
            .write_to(&paths.journal_path(&payload.txid).unwrap())
            .unwrap();
        struct NoopSleeper;
        impl Sleeper for NoopSleeper {
            fn sleep(&self, _: Duration) {}
        }
        MaintenanceWorker::run_worker_from_journal(
            paths,
            runner,
            &payload.txid,
            &NoopSleeper,
            Duration::ZERO,
            &|| Duration::from_secs(1),
            fail,
        )
    }

    fn push_uninstall_success_ipc(runner: &RecordingRunner) {
        let mut q = runner.responses.lock().unwrap();
        q.push(Ok(CommandOutput {
            code: 0,
            stdout: String::new(),
            stderr: String::new(),
        })); // rescan
        q.push(Ok(CommandOutput {
            code: 0,
            stdout: r#"[{"id":"omarchy.menu"}]"#.into(),
            stderr: String::new(),
        })); // listPlugins absent
        q.push(Ok(CommandOutput {
            code: 1,
            stdout: "unknown\n".into(),
            stderr: String::new(),
        })); // health gone
    }

    fn push_uninstall_rollback_ipc(runner: &RecordingRunner) {
        let mut q = runner.responses.lock().unwrap();
        // rollback rescan + health ok for previous
        q.push(Ok(CommandOutput {
            code: 0,
            stdout: String::new(),
            stderr: String::new(),
        }));
        q.push(Ok(CommandOutput {
            code: 0,
            stdout: "ok\n".into(),
            stderr: String::new(),
        }));
    }

    #[test]
    fn uninstall_standard_commits_preserves_settings_and_backups() {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let paths = PluginPaths::with_plugins_dir(
            &home,
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let (shell, settings, cache) = seed_uninstall_layout(dir.path(), &paths, "10.0.0");
        let settings_before = fs::read(&settings).unwrap();
        let backups_marker = paths.backups_dir.join("old/plugin/marker");
        assert!(backups_marker.is_file());

        let txid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let payload = sample_uninstall_payload(
            &paths, txid, "10.0.0", &tools, false, &shell, &settings, &cache,
        );
        let runner = RecordingRunner::default();
        push_uninstall_success_ipc(&runner);
        run_uninstall_worker(&paths, &payload, &runner, None).unwrap();

        assert!(!paths.plugin_root.exists(), "plugin root removed");
        assert!(!cache.exists(), "cache quarantined/GC'd");
        assert_eq!(
            fs::read(&settings).unwrap(),
            settings_before,
            "settings preserved"
        );
        assert!(
            backups_marker.is_file(),
            "migration backups preserved on standard uninstall"
        );
        let shell_after = fs::read_to_string(&shell).unwrap();
        assert!(!shell_after.contains("agent-bar.usage"));
        assert!(shell_after.contains("omarchy.menu"));
        let report: DurableReport = serde_json::from_slice(
            &fs::read(paths.reports_dir.join(format!("{txid}.json"))).unwrap(),
        )
        .unwrap();
        assert!(report.ok);
        assert!(!report.rolled_back);
    }

    #[test]
    fn uninstall_purge_quarantines_settings_and_backups() {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let paths = PluginPaths::with_plugins_dir(
            &home,
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let (shell, settings, cache) = seed_uninstall_layout(dir.path(), &paths, "10.0.0");
        let txid = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let payload = sample_uninstall_payload(
            &paths, txid, "10.0.0", &tools, true, &shell, &settings, &cache,
        );
        let runner = RecordingRunner::default();
        push_uninstall_success_ipc(&runner);
        run_uninstall_worker(&paths, &payload, &runner, None).unwrap();
        assert!(!settings.exists(), "settings purged");
        assert!(!paths.backups_dir.exists(), "backups purged");
        assert!(!paths.plugin_root.exists());
    }

    #[test]
    fn uninstall_pre_commit_failures_roll_back_and_verify_service() {
        let cases = [
            WorkerFailPoint::BeforeShellBackup,
            WorkerFailPoint::AfterShellBackup,
            WorkerFailPoint::AtQuarantineRename,
            WorkerFailPoint::AtExactIdRemoval,
            WorkerFailPoint::AtRescan,
            WorkerFailPoint::AtAbsenceCheck,
            WorkerFailPoint::AtCommitFsync,
            WorkerFailPoint::AtSettingsPurgeQuarantine,
            WorkerFailPoint::AtBackupsPurgeQuarantine,
        ];
        for (i, point) in cases.into_iter().enumerate() {
            let dir = tempdir().unwrap();
            let home = dir.path().join("home");
            fs::create_dir_all(&home).unwrap();
            let paths = PluginPaths::with_plugins_dir(
                &home,
                dir.path().join("plugins"),
                dir.path().join("state"),
            );
            let tools = dir.path().join("tools");
            fs::create_dir_all(&tools).unwrap();
            let (shell, settings, cache) = seed_uninstall_layout(dir.path(), &paths, "10.0.0");
            let shell_before = fs::read(&shell).unwrap();
            let plugin_before = fs::read(paths.plugin_root.join("Service.qml")).unwrap();
            let settings_before = fs::read(&settings).unwrap();
            let txid = format!("{:032x}", i + 1);
            let purge = matches!(
                point,
                WorkerFailPoint::AtSettingsPurgeQuarantine
                    | WorkerFailPoint::AtBackupsPurgeQuarantine
            );
            let payload = sample_uninstall_payload(
                &paths, &txid, "10.0.0", &tools, purge, &shell, &settings, &cache,
            );
            let runner = RecordingRunner::default();
            // Some fail points happen after rescan is issued.
            match point {
                WorkerFailPoint::AtRescan
                | WorkerFailPoint::AtAbsenceCheck
                | WorkerFailPoint::AtCommitFsync => {
                    // rescan may run; for AtAbsenceCheck inject before success
                    if matches!(point, WorkerFailPoint::AtCommitFsync) {
                        push_uninstall_success_ipc(&runner);
                    } else if matches!(point, WorkerFailPoint::AtRescan) {
                        // fail inject before run — no ipc needed before fail
                    } else if matches!(point, WorkerFailPoint::AtAbsenceCheck) {
                        let mut q = runner.responses.lock().unwrap();
                        q.push(Ok(CommandOutput {
                            code: 0,
                            stdout: String::new(),
                            stderr: String::new(),
                        })); // rescan ok then inject
                    }
                    push_uninstall_rollback_ipc(&runner);
                }
                WorkerFailPoint::AtExactIdRemoval => {
                    // quarantine done; fail before shell strip — rollback needs rescan+health
                    push_uninstall_rollback_ipc(&runner);
                }
                WorkerFailPoint::AtQuarantineRename
                | WorkerFailPoint::BeforeShellBackup
                | WorkerFailPoint::AfterShellBackup
                | WorkerFailPoint::AtSettingsPurgeQuarantine
                | WorkerFailPoint::AtBackupsPurgeQuarantine => {
                    push_uninstall_rollback_ipc(&runner);
                }
                _ => {}
            }
            let err = run_uninstall_worker(&paths, &payload, &runner, Some(point)).unwrap_err();
            assert!(
                err.to_string().contains("injected")
                    || err.to_string().contains("rescan")
                    || err.to_string().contains("absence"),
                "point={point:?} err={err}"
            );
            // Pre-commit: plugin + shell restored, settings untouched for standard.
            assert_eq!(
                fs::read(paths.plugin_root.join("Service.qml")).unwrap(),
                plugin_before,
                "point={point:?} plugin restored"
            );
            assert_eq!(
                fs::read(&shell).unwrap(),
                shell_before,
                "point={point:?} shell exact bytes restored"
            );
            if !purge {
                assert_eq!(
                    fs::read(&settings).unwrap(),
                    settings_before,
                    "point={point:?} settings untouched"
                );
            }
            // Pre-commit failures must write a durable report with rolled_back +
            // service-verified rollback (rescan + previous-version health).
            let report_path = paths.reports_dir.join(format!("{txid}.json"));
            assert!(
                report_path.is_file(),
                "point={point:?} durable rollback report required"
            );
            let report: DurableReport =
                serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
            assert!(!report.ok, "point={point:?} must not claim ok");
            assert!(
                report.rolled_back,
                "point={point:?} must claim rolled_back after pre-commit failure"
            );
            assert!(
                report.message.contains("rolled back") || report.message.contains("injected"),
                "point={point:?} message={:?}",
                report.message
            );
            let calls = runner.recorded();
            let has_rescan = calls
                .iter()
                .any(|(_, args)| args.iter().any(|a| a == "rescan"));
            let has_health = calls
                .iter()
                .any(|(_, args)| args.iter().any(|a| a == "health"));
            assert!(
                has_rescan,
                "point={point:?} rollback must issue plugin rescan; calls={calls:?}"
            );
            assert!(
                has_health,
                "point={point:?} rollback must verify old service health; calls={calls:?}"
            );
        }
    }

    #[test]
    fn uninstall_post_commit_gc_failure_leaves_residual_without_rollback() {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let paths = PluginPaths::with_plugins_dir(
            &home,
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let (shell, settings, cache) = seed_uninstall_layout(dir.path(), &paths, "10.0.0");
        let settings_before = fs::read(&settings).unwrap();
        let txid = "cccccccccccccccccccccccccccccccc";
        let payload = sample_uninstall_payload(
            &paths, txid, "10.0.0", &tools, false, &shell, &settings, &cache,
        );
        let runner = RecordingRunner::default();
        push_uninstall_success_ipc(&runner);
        // Should succeed overall with residual — not an Err for post-commit GC.
        run_uninstall_worker(
            &paths,
            &payload,
            &runner,
            Some(WorkerFailPoint::AtPostCommitGc),
        )
        .unwrap();
        assert!(!paths.plugin_root.exists());
        assert_eq!(fs::read(&settings).unwrap(), settings_before);
        let report: DurableReport = serde_json::from_slice(
            &fs::read(paths.reports_dir.join(format!("{txid}.json"))).unwrap(),
        )
        .unwrap();
        assert!(report.ok);
        assert!(!report.rolled_back, "post-commit must never claim rollback");
        assert!(!report.residual_paths.is_empty());
    }

    #[test]
    fn uninstall_standard_leaves_ambiguous_legacy_untouched() {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let paths = PluginPaths::with_plugins_dir(
            &home,
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let (shell, settings, cache) = seed_uninstall_layout(dir.path(), &paths, "10.0.0");
        // Ambiguous/modified legacy path: known location but non-marker content.
        let ambiguous = home.join(".config/waybar/agent-bar");
        fs::create_dir_all(ambiguous.parent().unwrap()).unwrap();
        fs::write(&ambiguous, b"user-edited content without markers").unwrap();
        let txid = "dddddddddddddddddddddddddddddddd";
        let payload = sample_uninstall_payload(
            &paths, txid, "10.0.0", &tools, false, &shell, &settings, &cache,
        );
        let runner = RecordingRunner::default();
        push_uninstall_success_ipc(&runner);
        run_uninstall_worker(&paths, &payload, &runner, None).unwrap();
        assert!(
            ambiguous.is_file(),
            "ambiguous legacy must remain after standard uninstall"
        );
        let report: DurableReport = serde_json::from_slice(
            &fs::read(paths.reports_dir.join(format!("{txid}.json"))).unwrap(),
        )
        .unwrap();
        // Residual/report may list the retained path.
        assert!(report.residual_paths.iter().any(|p| p.contains("waybar")) || ambiguous.is_file());
    }

    #[test]
    fn uninstall_exclusive_gate_blocks_shared_status_lane() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc;
        use std::thread;
        use std::time::Instant;

        let dir = tempdir().unwrap();
        let paths = PluginPaths::with_plugins_dir(
            dir.path(),
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        fs::create_dir_all(&paths.plugins_dir).unwrap();
        fs::create_dir_all(&paths.transactions_dir).unwrap();
        fs::create_dir_all(&paths.reports_dir).unwrap();
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let shell = dir.path().join("shell.json");
        write_shell_with_plugin(&shell);
        write_min_plugin(&paths.plugin_root, "10.0.0");
        write_receipt(&paths.plugin_root, "10.0.0");
        let settings = dir.path().join("settings.json");
        fs::write(&settings, b"{}").unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("status-v2.json"), b"{}").unwrap();
        let notif = cache.join("notification-state-v1.json");
        fs::write(&notif, b"{}").unwrap();
        let txid = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
        let payload = sample_uninstall_payload(
            &paths, txid, "10.0.0", &tools, false, &shell, &settings, &cache,
        );
        let mut journal = TransactionJournal::new(txid, "uninstall");
        journal.record(TxStep::Preflight, "ok");
        journal.record(TxStep::Stage, serde_json::to_string(&payload).unwrap());
        journal
            .write_to(&paths.journal_path(txid).unwrap())
            .unwrap();

        let lock_path = paths.maintenance_lock.clone();

        // Phase 1: external status helper already holding shared (lanes not drained)
        // must block exclusive handoff/mutation.
        {
            let gate = MaintenanceGate::open(&lock_path).unwrap();
            let shared = gate.lock_shared().unwrap();
            assert!(
                gate.try_lock_exclusive().unwrap().is_none(),
                "status shared hold must block exclusive maintenance handoff"
            );
            drop(shared); // service-owned status lane drained
        }

        // Phase 2: concurrent shared waiter awaits while exclusive worker mutates.
        // Prove shared try fails under exclusive, waiter unblocks only after worker,
        // and cache/notification paths are not recreated.
        let saw_exclusive_block = std::sync::Arc::new(AtomicBool::new(false));
        let saw_flag = std::sync::Arc::clone(&saw_exclusive_block);
        let (start_tx, start_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel::<()>();
        let lock_for_waiter = lock_path.clone();
        let cache_for_waiter = cache.clone();
        let notif_for_waiter = notif.clone();
        let waiter = thread::spawn(move || {
            start_rx.recv().expect("start signal");
            let gate = MaintenanceGate::open(&lock_for_waiter).unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if gate.try_lock_shared().unwrap().is_none() {
                    saw_flag.store(true, Ordering::SeqCst);
                    // Block until exclusive fully released (worker finished).
                    let _shared = gate.lock_shared().unwrap();
                    // After shared re-acquires: no cache/notification recreation.
                    assert!(
                        !cache_for_waiter.exists(),
                        "cache must not be recreated after exclusive uninstall"
                    );
                    assert!(
                        !notif_for_waiter.exists(),
                        "notification state must not be recreated after exclusive uninstall"
                    );
                    let _ = done_tx.send(());
                    return;
                }
                thread::sleep(Duration::from_millis(1));
            }
            let _ = done_tx.send(());
        });

        start_tx.send(()).unwrap();
        // Give waiter a polling head-start before exclusive acquisition.
        thread::sleep(Duration::from_millis(10));

        let runner = RecordingRunner::default();
        // Force at least one absence-poll sleep under exclusive so the concurrent
        // shared waiter reliably observes the barrier (not a pure race).
        {
            let mut q = runner.responses.lock().unwrap();
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })); // rescan
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: r#"[{"id":"agent-bar.usage"}]"#.into(),
                stderr: String::new(),
            })); // still present → sleep
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: "ok\n".into(),
                stderr: String::new(),
            })); // health still ok
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: r#"[{"id":"omarchy.menu"}]"#.into(),
                stderr: String::new(),
            })); // absent
            q.push(Ok(CommandOutput {
                code: 1,
                stdout: "unknown\n".into(),
                stderr: String::new(),
            })); // health gone
        }
        // Monotonic now advances only on sleep so the poll loop does not
        // immediately hit the deadline while the sleeper holds the exclusive window.
        let tick = std::sync::atomic::AtomicU64::new(0);
        let now = || Duration::from_millis(tick.load(Ordering::SeqCst));
        struct DelaySleeperTick<'a> {
            tick: &'a std::sync::atomic::AtomicU64,
        }
        impl Sleeper for DelaySleeperTick<'_> {
            fn sleep(&self, d: Duration) {
                self.tick.fetch_add(d.as_millis() as u64, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(40));
            }
        }
        MaintenanceWorker::run_worker_from_journal(
            &paths,
            &runner,
            txid,
            &DelaySleeperTick { tick: &tick },
            Duration::ZERO,
            &now,
            None,
        )
        .unwrap();

        done_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("shared waiter must complete after exclusive release");
        waiter.join().expect("waiter thread");
        assert!(
            saw_exclusive_block.load(Ordering::SeqCst),
            "shared status lane must observe exclusive block during uninstall worker"
        );
        assert!(!cache.exists(), "cache removed and not recreated");
        assert!(
            !notif.exists(),
            "notification path removed and not recreated"
        );
        assert!(!paths.plugin_root.exists());
    }
}
