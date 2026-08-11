//! Update check, uninstall confirmation, and executable-path resolution.
//! `update apply` and `uninstall` both delegate their live mutation to the
//! omarchy CLI (git-plugin-distribution Tasks 2-3) — this module no longer
//! runs a worker over a copied helper binary, stages a tarball, or polls a
//! rescan for health.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::plugin::bundle::{
    BundleError, MINIMUM_QUICKSHELL_VERSION, OFFICIAL_TARGET, OMARCHY_CONTRACT,
};
use crate::plugin::omarchy::OmarchyError;
use crate::plugin::paths::{PathError, PLUGIN_ID};
use crate::support::Clock;

/// Distribution repo `bundle.json` receipt: the sole `update check` discovery
/// source under git-native distribution. Served directly by
/// raw.githubusercontent.com — no redirect-following is needed.
pub const DIST_RECEIPT_URL: &str =
    "https://raw.githubusercontent.com/othavi0/omarchy-agent-bar/master/bundle.json";

/// User-Agent for release discovery/download (no provider credentials).
pub const RELEASE_USER_AGENT: &str = concat!("agent-bar-update/", env!("CARGO_PKG_VERSION"));

/// Literal prefix of the computed `latestCompatible.releaseNotesUrl`.
pub const RELEASE_NOTES_URL_PREFIX: &str =
    "https://github.com/othavi0/omarchy-agent-bar/releases/tag/v";

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
// Uninstall confirmation
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Executable resolution
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::bundle::{BundleBuilder, BundleValidator};
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
            release_notes_url: "https://github.com/othavi0/omarchy-agent-bar/releases/tag/v10.1.0"
                .into(),
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
            "https://github.com/othavi0/omarchy-agent-bar/releases/tag/v10.1.0"
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

    fn fake_abs_bin(dir: &Path, name: &str) -> String {
        let p = dir.join(name);
        fs::write(&p, b"#!/bin/true\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p.canonicalize().unwrap().display().to_string()
    }

    fn write_min_plugin(root: &Path, version: &str) {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::create_dir_all(root.join("icons")).unwrap();
        fs::create_dir_all(root.join("components")).unwrap();
        fs::write(
            root.join("manifest.json"),
            format!(
                r#"{{"schemaVersion":1,"id":"othavi0.agent-bar","name":"Agent Bar","version":"{version}","author":"othavi0","license":"MIT","description":"x","kinds":["service","bar-widget"],"entryPoints":{{"service":"Service.qml","barWidget":"BarWidget.qml"}},"barWidget":{{"displayName":"Agent Bar","description":"x","category":"AI","aliases":["agent-bar"],"allowMultiple":false,"defaults":{{}},"schema":[]}}}}"#
            ),
        )
        .unwrap();
        fs::write(root.join("Service.qml"), format!("// {version}\n")).unwrap();
        for name in [
            "BarWidget.qml",
            "CoreMaintenance.js",
            "CoreScroll.js",
            "CoreService.js",
            "CoreSettings.js",
            "CoreView.js",
            "MaintenanceView.qml",
            "Popup.qml",
            "ProviderRail.qml",
            "ProviderView.qml",
            "SettingsView.qml",
        ] {
            fs::write(root.join(name), format!("// {name}\n")).unwrap();
        }
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
        fs::write(root.join("icons/claude.png"), b"png").unwrap();
        fs::write(root.join("components/Sample.qml"), b"// sample\n").unwrap();
        fs::write(root.join("README.md"), b"# Agent Bar\n").unwrap();
        fs::write(root.join("LICENSE"), b"MIT\n").unwrap();
        fs::write(root.join("preview.png"), b"png").unwrap();
        for p in [
            "Service.qml",
            "BarWidget.qml",
            "CoreMaintenance.js",
            "CoreScroll.js",
            "CoreService.js",
            "CoreSettings.js",
            "CoreView.js",
            "MaintenanceView.qml",
            "Popup.qml",
            "ProviderRail.qml",
            "ProviderView.qml",
            "SettingsView.qml",
            "manifest.json",
            "icons/claude.png",
            "components/Sample.qml",
            "README.md",
            "LICENSE",
            "preview.png",
        ] {
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
        let root = dir.path().join("othavi0.agent-bar");
        write_min_plugin(&root, "10.0.0");
        write_receipt(&root, "10.0.0");
        assert!(root.join("bin/agent-bar").is_file());
        assert!(!root.join("agent-bar").exists());
        assert!(!dir.path().join("usr/bin/agent-bar").exists());
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

    // -----------------------------------------------------------------------
    // Uninstall confirmation (Task 18 / BUNDLE-036). The quarantine/rollback
    // fault matrix that used to live here tested the worker chain deleted in
    // git-plugin-distribution Task 3 — `uninstall` no longer runs a worker at
    // all, so there is nothing left to fault-inject. `worker_holds_exclusive_
    // maintenance_gate`, which exercised `MaintenanceGate` through the same
    // dead journal/payload ceremony, is likewise gone — the primitive's own
    // exclusive-lock coverage lives in `support::maintenance_gate`'s tests.
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
}
