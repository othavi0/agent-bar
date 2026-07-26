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
    extract_bundle_archive, BundleError, BundleReceipt, BundleValidator, ReleaseMetadata,
    MINIMUM_QUICKSHELL_VERSION, OFFICIAL_TARGET, OMARCHY_CONTRACT,
};
use crate::plugin::omarchy::{CommandOutput, CommandRunner, OmarchyError};
use crate::plugin::ownership::hash_bytes;
use crate::plugin::paths::{validate_txid, PathError, PluginPaths, PLUGIN_ID};
use crate::plugin::transaction::{
    copy_dir_all, exchange_paths, TransactionError, TransactionJournal, TxStep,
};
use crate::providers::amp_cli::which_in_path;
use crate::support::maintenance_gate::MaintenanceGate;
use crate::support::Clock;

/// Executable basename that selects maintenance-worker mode before CLI parsing.
pub const MAINTENANCE_WORKER_NAME: &str = "agent-bar-maintenance-worker";

/// Official GitHub releases listing (discovery only).
pub const RELEASES_API_URL: &str = "https://api.github.com/repos/othavi0/agent-bar/releases";

/// Every initial metadata/asset URL must stay under this path prefix.
pub const RELEASE_DOWNLOAD_PREFIX: &str = "https://github.com/othavi0/agent-bar/releases/";

/// User-Agent for release discovery/download (no provider credentials).
pub const RELEASE_USER_AGENT: &str = concat!("agent-bar-update/", env!("CARGO_PKG_VERSION"));

/// Max HTTPS redirects for an asset download.
pub const MAX_DOWNLOAD_REDIRECTS: u32 = 5;

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
    pub archive_url: String,
    pub checksum_url: String,
    pub archive_sha256: String,
    pub release_notes_url: String,
    pub source_commit: String,
}

/// Exact successful `update check` stdout document (BUNDLE-021).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCheckDocument {
    pub schema_version: u32,
    pub checked_at: String,
    pub current: UpdateCurrent,
    pub available: bool,
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
            validate_release_asset_url(&latest.archive_url)?;
            validate_release_asset_url(&latest.checksum_url)?;
            crate::plugin::bundle::validate_sha256_hex_pub(&latest.archive_sha256)
                .map_err(|e| MaintenanceError::msg(e.to_string()))?;
            crate::plugin::bundle::validate_source_commit(&latest.source_commit)
                .map_err(|e| MaintenanceError::msg(e.to_string()))?;
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
// Download URL policy
// ---------------------------------------------------------------------------

/// Validate an initial release asset / metadata URL (no redirect yet).
pub fn validate_release_asset_url(url: &str) -> Result<(), MaintenanceError> {
    if !url.starts_with(RELEASE_DOWNLOAD_PREFIX) {
        return Err(MaintenanceError::msg(format!(
            "release URL must start with {RELEASE_DOWNLOAD_PREFIX}"
        )));
    }
    validate_redirect_target(url, true)
}

/// Validate a redirect target under the closed download policy.
///
/// Rules (BUNDLE-025 / download policy):
/// - HTTPS only (no scheme downgrade)
/// - host is `github.com` or ends with `.githubusercontent.com`
/// - no userinfo
/// - no IP-literal hosts
/// - no non-default ports
pub fn validate_redirect_target(url: &str, initial: bool) -> Result<(), MaintenanceError> {
    // Lightweight parser (no extra crate): scheme://[userinfo@]host[:port]/path
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| MaintenanceError::msg("download URL must use https"))?;
    if rest.contains('@') {
        return Err(MaintenanceError::msg(
            "download URL must not contain userinfo",
        ));
    }
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err(MaintenanceError::msg("download URL missing host"));
    }
    // Port detection: host:port (IPv6 literals rejected via '[').
    if authority.starts_with('[') {
        return Err(MaintenanceError::msg(
            "download URL must not use an IP-literal host",
        ));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h, Some(p)),
        _ => (authority, None),
    };
    if port.is_some() {
        return Err(MaintenanceError::msg(
            "download URL must not specify a non-default port",
        ));
    }
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return Err(MaintenanceError::msg(
            "download URL must not use an IP-literal host",
        ));
    }
    let host_ok = host == "github.com" || host.ends_with(".githubusercontent.com");
    if !host_ok {
        return Err(MaintenanceError::msg(format!(
            "download host not allowed: {host}"
        )));
    }
    if initial && !url.starts_with(RELEASE_DOWNLOAD_PREFIX) {
        return Err(MaintenanceError::msg(
            "initial asset URL must remain under github.com/othavi0/agent-bar/releases/",
        ));
    }
    let _ = initial;
    Ok(())
}

/// Pure redirect chain validator (no network). Depth must be ≤ 5.
pub fn validate_redirect_chain(urls: &[&str]) -> Result<(), MaintenanceError> {
    if urls.is_empty() {
        return Err(MaintenanceError::msg("empty redirect chain"));
    }
    if urls.len() as u32 > MAX_DOWNLOAD_REDIRECTS + 1 {
        return Err(MaintenanceError::msg(format!(
            "redirect depth exceeds {MAX_DOWNLOAD_REDIRECTS}"
        )));
    }
    validate_release_asset_url(urls[0])?;
    for (i, u) in urls.iter().enumerate().skip(1) {
        validate_redirect_target(u, false)
            .map_err(|e| MaintenanceError::msg(format!("redirect[{i}] rejected: {e}")))?;
    }
    Ok(())
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

pub struct UpdateCheck;

impl UpdateCheck {
    /// Discover the latest compatible release and return the machine document.
    pub fn run<H: ReleaseHttp, C: Clock>(
        http: &H,
        clock: &C,
        probe: &UpdateCheckProbe,
    ) -> Result<UpdateCheckDocument, MaintenanceError> {
        let list = http.get(
            RELEASES_API_URL,
            &[
                ("Accept", "application/vnd.github+json"),
                ("User-Agent", RELEASE_USER_AGENT),
            ],
        )?;
        if list.status != 200 {
            return Err(MaintenanceError::msg(format!(
                "releases API returned HTTP {}",
                list.status
            )));
        }
        let releases: Vec<GitHubRelease> = serde_json::from_slice(&list.body)
            .map_err(|e| MaintenanceError::msg(format!("malformed releases list: {e}")))?;

        let current = semver::Version::parse(&probe.current_version)
            .map_err(|e| MaintenanceError::msg(format!("current version: {e}")))?;
        let qs = semver::Version::parse(&probe.quickshell_version)
            .map_err(|e| MaintenanceError::msg(format!("quickshell version: {e}")))?;
        let min_qs = semver::Version::parse(MINIMUM_QUICKSHELL_VERSION)
            .map_err(|e| MaintenanceError::msg(e.to_string()))?;

        let mut best: Option<(semver::Version, UpdateCompatible)> = None;

        for rel in &releases {
            if rel.draft || rel.prerelease {
                continue;
            }
            let tag = rel.tag_name.trim_start_matches('v');
            let ver = match semver::Version::parse(tag) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Locate metadata asset for this target.
            let meta_name = format!("{PLUGIN_ID}-{tag}-{OFFICIAL_TARGET}.metadata.json");
            let archive_name = format!("{PLUGIN_ID}-{tag}-{OFFICIAL_TARGET}.tar.zst");
            let checksum_name = format!("{archive_name}.sha256");

            let meta_asset = rel.assets.iter().find(|a| a.name == meta_name);
            let archive_asset = rel.assets.iter().find(|a| a.name == archive_name);
            let checksum_asset = rel.assets.iter().find(|a| a.name == checksum_name);

            // Incomplete assets for a release that claims the target → error
            // only when any of the three names is present without the others,
            // or metadata exists but is malformed. Fully absent → skip.
            let any = meta_asset.is_some() || archive_asset.is_some() || checksum_asset.is_some();
            if !any {
                continue;
            }
            let (meta_asset, archive_asset, checksum_asset) =
                match (meta_asset, archive_asset, checksum_asset) {
                    (Some(m), Some(a), Some(c)) => (m, a, c),
                    _ => {
                        return Err(MaintenanceError::msg(format!(
                            "incomplete release assets for {tag}"
                        )));
                    }
                };

            validate_release_asset_url(&meta_asset.browser_download_url)?;
            validate_release_asset_url(&archive_asset.browser_download_url)?;
            validate_release_asset_url(&checksum_asset.browser_download_url)?;

            let meta_resp = http.get(
                &meta_asset.browser_download_url,
                &[("User-Agent", RELEASE_USER_AGENT)],
            )?;
            if meta_resp.status != 200 {
                return Err(MaintenanceError::msg(format!(
                    "metadata download HTTP {} for {tag}",
                    meta_resp.status
                )));
            }
            // Reject credential leakage on redirects — scripted client already
            // rejects Authorization headers; production uses Policy::none so
            // callers must follow redirects explicitly via download_with_policy.
            let meta = ReleaseMetadata::parse_json(&meta_resp.body)
                .map_err(|e| MaintenanceError::msg(format!("malformed metadata for {tag}: {e}")))?;

            if meta.target != probe.target {
                continue;
            }
            if meta.omarchy_contract != probe.omarchy_contract {
                continue;
            }
            let meta_min_qs = semver::Version::parse(&meta.minimum_quickshell_version)
                .map_err(|e| MaintenanceError::msg(e.to_string()))?;
            if qs < meta_min_qs || qs < min_qs {
                // Locally incompatible — skip (not an error).
                continue;
            }
            if meta.version != tag && meta.version != ver.to_string() {
                return Err(MaintenanceError::msg(format!(
                    "metadata version {} does not equal tag {tag}",
                    meta.version
                )));
            }
            if meta.archive.file_name != archive_name {
                return Err(MaintenanceError::msg(format!(
                    "metadata archive fileName mismatch for {tag}"
                )));
            }

            let compatible = UpdateCompatible {
                version: meta.version.clone(),
                omarchy_contract: meta.omarchy_contract,
                minimum_quickshell_version: meta.minimum_quickshell_version.clone(),
                archive_url: archive_asset.browser_download_url.clone(),
                checksum_url: checksum_asset.browser_download_url.clone(),
                archive_sha256: meta.archive.sha256.clone(),
                release_notes_url: meta.release_notes_url.clone(),
                source_commit: meta.source_commit.clone(),
            };

            match &best {
                None => best = Some((ver, compatible)),
                Some((best_v, _)) if ver > *best_v => best = Some((ver, compatible)),
                _ => {}
            }
        }

        let latest_compatible = best.map(|(_, c)| c);
        let available = match &latest_compatible {
            Some(c) => {
                let lv = semver::Version::parse(&c.version)
                    .map_err(|e| MaintenanceError::msg(e.to_string()))?;
                lv > current
            }
            None => false,
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
            latest_compatible,
        };
        doc.validate()?;
        Ok(doc)
    }

    /// Download archive bytes following the closed redirect policy; verify sha256.
    pub fn download_archive<H: ReleaseHttp>(
        http: &H,
        archive_url: &str,
        expected_sha256: &str,
    ) -> Result<Vec<u8>, MaintenanceError> {
        let body = download_with_policy(http, archive_url)?;
        let digest = hash_bytes(&body);
        if digest != expected_sha256 {
            return Err(MaintenanceError::msg(
                "archive sha256 does not match pinned journal hash",
            ));
        }
        Ok(body)
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Follow ≤5 HTTPS redirects under the closed host policy. No credentials.
pub fn download_with_policy<H: ReleaseHttp>(
    http: &H,
    initial_url: &str,
) -> Result<Vec<u8>, MaintenanceError> {
    validate_release_asset_url(initial_url)?;
    let mut url = initial_url.to_string();
    for hop in 0..=MAX_DOWNLOAD_REDIRECTS {
        let resp = http.get(&url, &[("User-Agent", RELEASE_USER_AGENT)])?;
        if (300..400).contains(&resp.status) {
            if hop == MAX_DOWNLOAD_REDIRECTS {
                return Err(MaintenanceError::msg(format!(
                    "redirect depth exceeds {MAX_DOWNLOAD_REDIRECTS}"
                )));
            }
            let loc = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("location"))
                .map(|(_, v)| v.clone())
                .ok_or_else(|| MaintenanceError::msg("redirect without Location"))?;
            // Absolute HTTPS only — relative redirects are rejected (closed policy).
            if !loc.starts_with("https://") {
                return Err(MaintenanceError::msg(
                    "redirect Location must be an absolute https URL",
                ));
            }
            let next = loc;
            validate_redirect_target(&next, false)?;
            url = next;
            continue;
        }
        if resp.status != 200 {
            return Err(MaintenanceError::msg(format!(
                "download HTTP {} for {url}",
                resp.status
            )));
        }
        return Ok(resp.body);
    }
    Err(MaintenanceError::msg("redirect loop"))
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
}

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

/// High-level maintenance coordinator used by CLI update apply / uninstall.
pub struct MaintenanceWorker;

impl MaintenanceWorker {
    /// Preflight + handoff for an update. Does not perform live mutation itself.
    ///
    /// Holds the exclusive maintenance gate for the duration of preflight, journal
    /// write, and unit start so status/settings cannot race the handoff barrier.
    #[allow(clippy::too_many_arguments)]
    pub fn handoff_update<R: CommandRunner>(
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

        // Exclusive barrier (ARCH-026 / CACHE): block shared status/settings.
        let gate = MaintenanceGate::open(&paths.maintenance_lock)
            .map_err(|e| MaintenanceError::msg(format!("open maintenance lock: {e}")))?;
        let _exclusive = gate
            .lock_exclusive()
            .map_err(|e| MaintenanceError::msg(format!("exclusive maintenance lock: {e}")))?;

        // Preflight: absolute tools + user manager + shell ping (BUNDLE-032H).
        require_absolute_executable(&payload.omarchy_bin)?;
        require_absolute_executable(&payload.omarchy_shell_bin)?;
        require_absolute_executable(systemd_program)?;
        require_absolute_executable(systemctl_program)?;

        let ping = runner.run(&payload.omarchy_shell_bin, &["shell", "ping"])?;
        if ping.code != 0 {
            return Err(MaintenanceError::msg("shell ping failed during preflight"));
        }
        // user systemd manager
        let user = runner.run(systemctl_program, &["--user", "is-system-running"])?;
        // Accept running / degraded / starting as reachable managers.
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
        let mut journal = TransactionJournal::new(txid, "update");
        journal.record(TxStep::Preflight, "worker copy verified; shell ping ok");
        // Embed payload as detail JSON for the worker.
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
        // Successful handoff only after systemd accepted the unit.
        Ok(unit)
    }

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
            MaintenanceOp::Update => Self::worker_update(
                paths,
                runner,
                &payload,
                &journal_path,
                sleeper,
                &deadlines,
                now,
                fail,
            ),
            MaintenanceOp::Uninstall => {
                // Full uninstall matrix is Task 18; provide structural hook.
                Err(MaintenanceError::msg(
                    "uninstall worker path is implemented in the uninstall task",
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn worker_update<R: CommandRunner, S: Sleeper>(
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
        let expected = payload
            .expected_version
            .as_deref()
            .ok_or_else(|| MaintenanceError::msg("update payload missing expected_version"))?;

        let stage = PathBuf::from(&payload.stage_path);
        let target = PathBuf::from(&payload.plugin_root);
        let quarantine = PathBuf::from(&payload.quarantine_path);

        // Validate staged bundle before mutation.
        if now() >= deadlines.stage_by {
            return Err(MaintenanceError::msg("stage deadline exceeded"));
        }
        let _receipt = BundleValidator::validate_tree(&stage)?;
        journal.record(TxStep::ValidateStaged, "staged bundle validated");
        journal.write_to(journal_path)?;

        if let Some(WorkerFailPoint::BeforeMutation) = fail {
            return Err(MaintenanceError::msg("injected failure before mutation"));
        }
        if let Some(WorkerFailPoint::AtExchange) = fail {
            return Err(MaintenanceError::msg("injected exchange failure"));
        }
        deadlines.may_begin_mutation(now())?;

        let had_target = target.exists();
        if had_target {
            exchange_paths(&stage, &target)?;
            if quarantine.exists() {
                let _ = fs::remove_dir_all(&quarantine);
            }
            fs::rename(&stage, &quarantine)?;
        } else {
            fs::rename(&stage, &target)?;
        }
        journal.record(TxStep::Exchange, "destination-local exchange done");
        journal.write_to(journal_path)?;

        if let Some(WorkerFailPoint::AfterExchange) = fail {
            // Fall through to rollback path.
            Self::rollback_update(
                runner,
                shell_program,
                payload,
                &target,
                &quarantine,
                had_target,
                journal_path,
                &mut journal,
                sleeper,
                now,
            )?;
            return Err(MaintenanceError::msg("injected failure after exchange"));
        }

        // Rescan then health poll.
        let rescan = match runner.run(&payload.omarchy_bin, &["plugin", "rescan"]) {
            Ok(out) => out,
            Err(e) => {
                Self::rollback_update(
                    runner,
                    shell_program,
                    payload,
                    &target,
                    &quarantine,
                    had_target,
                    journal_path,
                    &mut journal,
                    sleeper,
                    now,
                )?;
                return Err(MaintenanceError::msg(format!("rescan failed: {e}")));
            }
        };
        if rescan.code != 0 {
            Self::rollback_update(
                runner,
                shell_program,
                payload,
                &target,
                &quarantine,
                had_target,
                journal_path,
                &mut journal,
                sleeper,
                now,
            )?;
            return Err(MaintenanceError::msg("rescan exit non-zero"));
        }
        journal.record(TxStep::Rescan, "plugin rescan issued");
        journal.write_to(journal_path)?;

        if let Some(WorkerFailPoint::AtHealth) = fail {
            Self::rollback_update(
                runner,
                shell_program,
                payload,
                &target,
                &quarantine,
                had_target,
                journal_path,
                &mut journal,
                sleeper,
                now,
            )?;
            return Err(MaintenanceError::msg("injected health failure"));
        }

        if let Err(e) = poll_update_health(runner, shell_program, expected, sleeper, now(), now) {
            Self::rollback_update(
                runner,
                shell_program,
                payload,
                &target,
                &quarantine,
                had_target,
                journal_path,
                &mut journal,
                sleeper,
                now,
            )?;
            return Err(e);
        }
        journal.record(TxStep::Health, format!("health ok for {expected}"));
        journal.record(TxStep::Commit, "update committed");
        journal.write_to(journal_path)?;

        // Post-commit GC of old bundle sibling (BUNDLE-032K).
        if quarantine.exists() {
            if let Err(err) = fs::remove_dir_all(&quarantine) {
                // Record residual; never claim rollback.
                let report = DurableReport {
                    txid: payload.txid.clone(),
                    ok: true,
                    rolled_back: false,
                    residual_paths: vec![quarantine.display().to_string()],
                    message: format!("post-commit GC failed: {err}"),
                };
                write_durable_report(paths, &report)?;
            }
        }

        // Successful cleanup may remove worker copy + journal last.
        let worker = paths.transactions_dir.join(MAINTENANCE_WORKER_NAME);
        let _ = fs::remove_file(&worker);
        let _ = fs::remove_file(journal_path);

        let report = DurableReport {
            txid: payload.txid.clone(),
            ok: true,
            rolled_back: false,
            residual_paths: vec![],
            message: "update committed".into(),
        };
        write_durable_report(paths, &report)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn rollback_update<R: CommandRunner, S: Sleeper>(
        runner: &R,
        shell_program: &str,
        payload: &MaintenanceJournalPayload,
        target: &Path,
        quarantine: &Path,
        had_target: bool,
        journal_path: &Path,
        journal: &mut TransactionJournal,
        sleeper: &S,
        now: &dyn Fn() -> Duration,
    ) -> Result<(), MaintenanceError> {
        if had_target && quarantine.exists() {
            exchange_paths(quarantine, target)?;
            let _ = fs::remove_dir_all(quarantine);
        } else if !had_target && target.exists() {
            let _ = fs::remove_dir_all(target);
        }
        journal.record(TxStep::Rollback, "restored previous plugin root");
        journal.write_to(journal_path)?;

        // Re-rescan + verify previous.
        let _ = runner.run(&payload.omarchy_bin, &["plugin", "rescan"]);
        if payload.is_fresh_install {
            // Fresh-install rollback: plugin absence + shell bytes (shell handled elsewhere).
            let list = runner.run(shell_program, &["shell", "listPlugins"])?;
            if list.code == 0 && !list_plugins_absent(&list.stdout)? {
                return Err(MaintenanceError::msg(
                    "fresh-install rollback: plugin still present",
                ));
            }
            return Ok(());
        }
        if payload.is_v9_rollback {
            // Structural: listPlugins must contain the entry; no health IPC.
            let list = runner.run(shell_program, &["shell", "listPlugins"])?;
            if !list_plugins_has_enabled(&list.stdout)? {
                return Err(MaintenanceError::msg(
                    "v9 rollback: listPlugins missing agent-bar.usage",
                ));
            }
            return Ok(());
        }
        // v10 health poll for previous version.
        if let Some(prev) = payload.previous_version.as_deref() {
            poll_update_health(runner, shell_program, prev, sleeper, now(), now)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerFailPoint {
    BeforeMutation,
    /// Fail before any directory exchange (live tree untouched).
    AtExchange,
    AfterExchange,
    AtHealth,
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

/// Stage a downloaded bundle under the destination-local stage path.
pub fn stage_update_bundle(
    paths: &PluginPaths,
    txid: &str,
    archive_bytes: &[u8],
) -> Result<(PathBuf, BundleReceipt), MaintenanceError> {
    validate_txid(txid)?;
    BundleValidator::validate_archive_bytes(archive_bytes)?;
    let stage = paths.stage_dir(txid)?;
    if stage.exists() {
        fs::remove_dir_all(&stage)?;
    }
    // Extract to a temp parent then move into stage (stage path is the plugin root itself).
    let tmp_parent = paths
        .plugins_dir
        .join(format!(".{PLUGIN_ID}.extract-{txid}"));
    if tmp_parent.exists() {
        fs::remove_dir_all(&tmp_parent)?;
    }
    fs::create_dir_all(&tmp_parent)?;
    let extracted = extract_bundle_archive(archive_bytes, &tmp_parent)?;
    fs::rename(&extracted, &stage)?;
    let _ = fs::remove_dir_all(&tmp_parent);
    let receipt = BundleValidator::validate_tree(&stage)?;
    Ok((stage, receipt))
}

/// Gate for `update apply <version>`: must match latestCompatible and available.
pub fn apply_version_allowed(
    doc: &UpdateCheckDocument,
    requested: &str,
) -> Result<UpdateCompatible, MaintenanceError> {
    if !doc.available {
        return Err(MaintenanceError::msg(
            "no newer compatible release is available",
        ));
    }
    let latest = doc
        .latest_compatible
        .clone()
        .ok_or_else(|| MaintenanceError::msg("latestCompatible is null"))?;
    if latest.version != requested {
        return Err(MaintenanceError::msg(format!(
            "requested version {requested} does not equal latestCompatible {}",
            latest.version
        )));
    }
    // Refuse downgrade explicitly (also covered by available).
    let cur = semver::Version::parse(&doc.current.version)
        .map_err(|e| MaintenanceError::msg(e.to_string()))?;
    let req =
        semver::Version::parse(requested).map_err(|e| MaintenanceError::msg(e.to_string()))?;
    if req < cur {
        return Err(MaintenanceError::msg("downgrade refused"));
    }
    Ok(latest)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::bundle::{BundleBuilder, ReleaseArchiveMeta};
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
            archive_url: "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst".into(),
            checksum_url: "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst.sha256".into(),
            archive_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            release_notes_url: "https://github.com/othavi0/agent-bar/releases/tag/v10.1.0".into(),
            source_commit: "0123456789abcdef0123456789abcdef01234567".into(),
        }
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
            latest_compatible: Some(sample_compatible()),
        };
        doc.validate().unwrap();
        let json = doc.to_stdout_json().unwrap();
        assert!(json.ends_with('\n'));
        assert!(json.contains("\"schemaVersion\":1") || json.contains("\"schemaVersion\": 1"));
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
            latest_compatible: Some(c),
        };
        doc.validate().unwrap();
        let mut bad = doc.clone();
        bad.available = true;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn redirect_policy_matrix() {
        validate_release_asset_url(
            "https://github.com/othavi0/agent-bar/releases/download/v10.0.0/x.tar.zst",
        )
        .unwrap();
        assert!(
            validate_release_asset_url("http://github.com/othavi0/agent-bar/releases/x").is_err()
        );
        assert!(
            validate_release_asset_url("https://evil.com/othavi0/agent-bar/releases/x").is_err()
        );
        assert!(validate_redirect_target("https://user:pass@github.com/foo", false).is_err());
        assert!(validate_redirect_target("https://github.com:8443/foo", false).is_err());
        assert!(validate_redirect_target("https://127.0.0.1/foo", false).is_err());
        assert!(
            validate_redirect_target("https://objects.githubusercontent.com/foo", false).is_ok()
        );
        assert!(validate_redirect_target("https://evilusercontent.com/x", false).is_err());

        validate_redirect_chain(&[
            "https://github.com/othavi0/agent-bar/releases/download/v1/a",
            "https://objects.githubusercontent.com/a",
        ])
        .unwrap();

        let mut deep: Vec<String> =
            vec!["https://github.com/othavi0/agent-bar/releases/download/v1/a".into()];
        for i in 0..6 {
            deep.push(format!("https://objects.githubusercontent.com/h{i}"));
        }
        let refs: Vec<&str> = deep.iter().map(String::as_str).collect();
        assert!(validate_redirect_chain(&refs).is_err());
    }

    #[test]
    fn credentials_rejected_on_download() {
        let http = ScriptedReleaseHttp::with_responses(vec![]);
        let err = http
            .get(
                "https://github.com/othavi0/agent-bar/releases/download/v1/a",
                &[("Authorization", "Bearer secret")],
            )
            .unwrap_err();
        assert!(err.to_string().contains("credentials"));
    }

    #[test]
    fn download_with_policy_follows_and_caps_redirects() {
        let url0 = "https://github.com/othavi0/agent-bar/releases/download/v1/a";
        let url1 = "https://objects.githubusercontent.com/a";
        // Scripted client pops from the end — push in reverse call order.
        let http = ScriptedReleaseHttp::with_responses(vec![
            Ok(ReleaseHttpResponse {
                status: 200,
                headers: vec![],
                body: b"archive-bytes".to_vec(),
            }),
            Ok(ReleaseHttpResponse {
                status: 302,
                headers: vec![("Location".into(), url1.into())],
                body: vec![],
            }),
        ]);
        let body = download_with_policy(&http, url0).unwrap();
        assert_eq!(body, b"archive-bytes");

        // Too many redirects.
        let mut seq: Vec<Result<ReleaseHttpResponse, MaintenanceError>> = Vec::new();
        for i in 0..7 {
            seq.push(Ok(ReleaseHttpResponse {
                status: 302,
                headers: vec![(
                    "Location".into(),
                    format!("https://objects.githubusercontent.com/r{i}"),
                )],
                body: vec![],
            }));
        }
        seq.reverse();
        let http = ScriptedReleaseHttp::with_responses(seq);
        assert!(download_with_policy(&http, url0).is_err());
    }

    #[test]
    fn update_check_selects_newest_compatible_skips_draft() {
        let meta_10_1 = ReleaseMetadata {
            schema_version: 1,
            plugin_id: PLUGIN_ID.into(),
            version: "10.1.0".into(),
            target: OFFICIAL_TARGET.into(),
            omarchy_contract: 1,
            minimum_quickshell_version: MINIMUM_QUICKSHELL_VERSION.into(),
            source_commit: "0123456789abcdef0123456789abcdef01234567".into(),
            archive: ReleaseArchiveMeta {
                file_name: "agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst".into(),
                size: 10,
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            },
            release_notes_url: "https://github.com/othavi0/agent-bar/releases/tag/v10.1.0".into(),
        };
        let meta_json = meta_10_1.to_pretty_json().unwrap();

        let releases = serde_json::json!([
            {
                "tag_name": "v10.2.0-rc1",
                "draft": false,
                "prerelease": true,
                "assets": []
            },
            {
                "tag_name": "v10.1.0",
                "draft": false,
                "prerelease": false,
                "assets": [
                    {
                        "name": "agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.metadata.json",
                        "browser_download_url": "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.metadata.json"
                    },
                    {
                        "name": "agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst",
                        "browser_download_url": "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst"
                    },
                    {
                        "name": "agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst.sha256",
                        "browser_download_url": "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst.sha256"
                    }
                ]
            },
            {
                "tag_name": "v9.0.0",
                "draft": true,
                "prerelease": false,
                "assets": []
            }
        ]);

        // Calls: list, then metadata for 10.1.0. Pop order = reverse push.
        let http = ScriptedReleaseHttp::with_responses(vec![
            Ok(ReleaseHttpResponse {
                status: 200,
                headers: vec![],
                body: meta_json.into_bytes(),
            }),
            Ok(ReleaseHttpResponse {
                status: 200,
                headers: vec![],
                body: serde_json::to_vec(&releases).unwrap(),
            }),
        ]);

        let clock = FixedClock(OffsetDateTime::parse("2026-07-26T18:42:00Z", &Rfc3339).unwrap());
        let probe = UpdateCheckProbe {
            current_version: "10.0.0".into(),
            quickshell_version: "0.3.0".into(),
            target: OFFICIAL_TARGET.into(),
            omarchy_contract: 1,
        };
        let doc = UpdateCheck::run(&http, &clock, &probe).unwrap();
        assert!(doc.available);
        assert_eq!(doc.latest_compatible.as_ref().unwrap().version, "10.1.0");
    }

    #[test]
    fn update_check_errors_on_incomplete_assets() {
        let releases = serde_json::json!([{
            "tag_name": "v10.1.0",
            "draft": false,
            "prerelease": false,
            "assets": [{
                "name": "agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst",
                "browser_download_url": "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst"
            }]
        }]);
        let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
            status: 200,
            headers: vec![],
            body: serde_json::to_vec(&releases).unwrap(),
        })]);
        let clock = FixedClock(OffsetDateTime::now_utc());
        let probe = UpdateCheckProbe {
            current_version: "10.0.0".into(),
            ..UpdateCheckProbe::default()
        };
        let err = UpdateCheck::run(&http, &clock, &probe).unwrap_err();
        assert!(err.to_string().contains("incomplete"));
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
        }
    }

    #[test]
    fn handoff_fails_before_mutation_when_ping_fails() {
        let dir = tempdir().unwrap();
        let paths = PluginPaths::with_plugins_dir(
            dir.path(),
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        fs::create_dir_all(&paths.plugins_dir).unwrap();
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let runner = RecordingRunner::default();
        {
            let mut q = runner.responses.lock().unwrap();
            // shell ping fails
            q.push(Ok(CommandOutput {
                code: 1,
                stdout: String::new(),
                stderr: "down".into(),
            }));
        }
        let exe = dir.path().join("agent-bar");
        fs::write(&exe, b"x").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        let txid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let payload = sample_payload(&paths, txid, "10.1.0", "10.0.0", &tools);
        let systemd = fake_abs_bin(&tools, "systemd-run");
        let systemctl = fake_abs_bin(&tools, "systemctl");
        let err = MaintenanceWorker::handoff_update(
            &paths,
            &runner,
            &exe,
            txid,
            &payload,
            &[],
            &systemd,
            &systemctl,
        )
        .unwrap_err();
        assert!(err.to_string().contains("ping"));
        // No journal mutation should leave a started unit — and no stage dir.
        assert!(!paths.stage_dir(txid).unwrap().exists());
    }

    #[test]
    fn handoff_rejects_bare_tool_names() {
        let dir = tempdir().unwrap();
        let paths = PluginPaths::with_plugins_dir(
            dir.path(),
            dir.path().join("plugins"),
            dir.path().join("state"),
        );
        fs::create_dir_all(&paths.plugins_dir).unwrap();
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        let exe = dir.path().join("agent-bar");
        fs::write(&exe, b"x").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        let txid = "dddddddddddddddddddddddddddddddd";
        let mut payload = sample_payload(&paths, txid, "10.1.0", "10.0.0", &tools);
        payload.omarchy_bin = "omarchy".into(); // bare name
        let err = MaintenanceWorker::handoff_update(
            &paths,
            &RecordingRunner::default(),
            &exe,
            txid,
            &payload,
            &[],
            &fake_abs_bin(&tools, "systemd-run"),
            &fake_abs_bin(&tools, "systemctl"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn worker_update_commit_and_health_mismatch_rollback() {
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

        let txid = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        // Prepare minimal staged + current plugin trees.
        let stage = paths.stage_dir(txid).unwrap();
        write_min_plugin(&stage, "10.1.0");
        write_receipt(&stage, "10.1.0");

        let current = &paths.plugin_root;
        write_min_plugin(current, "10.0.0");
        write_receipt(current, "10.0.0");

        let payload = sample_payload(&paths, txid, "10.1.0", "10.0.0", &tools);
        let mut journal = TransactionJournal::new(txid, "update");
        journal.record(TxStep::Preflight, "ok");
        journal.record(TxStep::Stage, serde_json::to_string(&payload).unwrap());
        let jp = paths.journal_path(txid).unwrap();
        journal.write_to(&jp).unwrap();

        // Success path: rescan ok, health ok.
        let runner = RecordingRunner::default();
        {
            let mut q = runner.responses.lock().unwrap();
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })); // rescan
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: "ok\n".into(),
                stderr: String::new(),
            })); // health
        }
        struct NoopSleeper;
        impl Sleeper for NoopSleeper {
            fn sleep(&self, _: Duration) {}
        }
        MaintenanceWorker::run_worker_from_journal(
            &paths,
            &runner,
            txid,
            &NoopSleeper,
            Duration::ZERO,
            &|| Duration::from_secs(1),
            None,
        )
        .unwrap();
        // New version on disk.
        let man: serde_json::Value =
            serde_json::from_slice(&fs::read(current.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(man["version"], "10.1.0");

        // --- Health mismatch rollback ---
        let txid2 = "cccccccccccccccccccccccccccccccc";
        let stage2 = paths.stage_dir(txid2).unwrap();
        write_min_plugin(&stage2, "10.2.0");
        write_receipt(&stage2, "10.2.0");
        // current is already 10.1.0 from previous step
        write_min_plugin(current, "10.1.0");
        write_receipt(current, "10.1.0");
        let before = fs::read(current.join("Service.qml")).unwrap();

        let payload2 = sample_payload(&paths, txid2, "10.2.0", "10.1.0", &tools);
        let mut j2 = TransactionJournal::new(txid2, "update");
        j2.record(TxStep::Preflight, "ok");
        j2.record(TxStep::Stage, serde_json::to_string(&payload2).unwrap());
        j2.write_to(&paths.journal_path(txid2).unwrap()).unwrap();

        let runner2 = RecordingRunner::default();
        {
            let mut q = runner2.responses.lock().unwrap();
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })); // rescan after exchange
                 // health always fails → timeout; but FakeMonotonic needed.
            for _ in 0..40 {
                q.push(Ok(CommandOutput {
                    code: 1,
                    stdout: "unknown\n".into(),
                    stderr: String::new(),
                }));
            }
            // rollback rescan
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            }));
            // rollback health for previous
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: "ok\n".into(),
                stderr: String::new(),
            }));
        }
        let mono = FakeMonotonic::new(0);
        struct Adv<'a>(&'a FakeMonotonic);
        impl Sleeper for Adv<'_> {
            fn sleep(&self, d: Duration) {
                self.0.advance(d);
            }
        }
        let err = MaintenanceWorker::run_worker_from_journal(
            &paths,
            &runner2,
            txid2,
            &Adv(&mono),
            Duration::ZERO,
            &|| mono.now(),
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("timeout") || err.to_string().contains("health"));
        // Rolled back to previous content.
        assert_eq!(fs::read(current.join("Service.qml")).unwrap(), before);
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
    fn downgrade_refusal_via_apply_gate() {
        // update apply only proceeds when available && version matches latest.
        let doc = UpdateCheckDocument {
            schema_version: 1,
            checked_at: "2026-07-26T18:42:00Z".into(),
            current: UpdateCurrent {
                version: "10.1.0".into(),
                target: OFFICIAL_TARGET.into(),
                omarchy_contract: 1,
                quickshell_version: "0.3.0".into(),
            },
            available: false,
            latest_compatible: Some(UpdateCompatible {
                version: "10.1.0".into(),
                ..sample_compatible()
            }),
        };
        doc.validate().unwrap();
        assert!(!doc.available);
        // Requesting apply of older version is rejected by gate helper.
        assert!(apply_version_allowed(&doc, "10.0.0").is_err());
        assert!(apply_version_allowed(&doc, "10.1.0").is_err()); // not available
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
    fn interrupted_download_rejects_sha_mismatch() {
        let http = ScriptedReleaseHttp::with_responses(vec![Ok(ReleaseHttpResponse {
            status: 200,
            headers: vec![],
            body: b"truncated-archive".to_vec(),
        })]);
        let err = UpdateCheck::download_archive(
            &http,
            "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/a.tar.zst",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        )
        .unwrap_err();
        assert!(err.to_string().contains("sha256"));
    }

    #[test]
    fn worker_exchange_failure_leaves_live_tree() {
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
        let txid = "ffffffffffffffffffffffffffffffff";
        let stage = paths.stage_dir(txid).unwrap();
        write_min_plugin(&stage, "10.1.0");
        write_receipt(&stage, "10.1.0");
        write_min_plugin(&paths.plugin_root, "10.0.0");
        write_receipt(&paths.plugin_root, "10.0.0");
        let before = fs::read(paths.plugin_root.join("Service.qml")).unwrap();
        let payload = sample_payload(&paths, txid, "10.1.0", "10.0.0", &tools);
        let mut journal = TransactionJournal::new(txid, "update");
        journal.record(TxStep::Preflight, "ok");
        journal.record(TxStep::Stage, serde_json::to_string(&payload).unwrap());
        journal
            .write_to(&paths.journal_path(txid).unwrap())
            .unwrap();
        struct NoopSleeper;
        impl Sleeper for NoopSleeper {
            fn sleep(&self, _: Duration) {}
        }
        let err = MaintenanceWorker::run_worker_from_journal(
            &paths,
            &RecordingRunner::default(),
            txid,
            &NoopSleeper,
            Duration::ZERO,
            &|| Duration::from_secs(1),
            Some(WorkerFailPoint::AtExchange),
        )
        .unwrap_err();
        assert!(err.to_string().contains("exchange"));
        assert_eq!(
            fs::read(paths.plugin_root.join("Service.qml")).unwrap(),
            before
        );
    }

    #[test]
    fn worker_rescan_failure_rolls_back() {
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
        let txid = "11111111111111111111111111111111";
        let stage = paths.stage_dir(txid).unwrap();
        write_min_plugin(&stage, "10.1.0");
        write_receipt(&stage, "10.1.0");
        write_min_plugin(&paths.plugin_root, "10.0.0");
        write_receipt(&paths.plugin_root, "10.0.0");
        let before = fs::read(paths.plugin_root.join("Service.qml")).unwrap();
        let payload = sample_payload(&paths, txid, "10.1.0", "10.0.0", &tools);
        let mut journal = TransactionJournal::new(txid, "update");
        journal.record(TxStep::Preflight, "ok");
        journal.record(TxStep::Stage, serde_json::to_string(&payload).unwrap());
        journal
            .write_to(&paths.journal_path(txid).unwrap())
            .unwrap();
        let runner = RecordingRunner::default();
        {
            let mut q = runner.responses.lock().unwrap();
            q.push(Ok(CommandOutput {
                code: 1,
                stdout: String::new(),
                stderr: "rescan failed".into(),
            }));
            // rollback rescan + health
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
        struct NoopSleeper;
        impl Sleeper for NoopSleeper {
            fn sleep(&self, _: Duration) {}
        }
        let err = MaintenanceWorker::run_worker_from_journal(
            &paths,
            &runner,
            txid,
            &NoopSleeper,
            Duration::ZERO,
            &|| Duration::from_secs(1),
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("rescan"));
        assert_eq!(
            fs::read(paths.plugin_root.join("Service.qml")).unwrap(),
            before
        );
    }

    #[test]
    fn worker_v9_structural_and_fresh_absence_rollback() {
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
        struct NoopSleeper;
        impl Sleeper for NoopSleeper {
            fn sleep(&self, _: Duration) {}
        }

        // Fresh install failure → absence after rollback.
        let txid = "22222222222222222222222222222222";
        let stage = paths.stage_dir(txid).unwrap();
        write_min_plugin(&stage, "10.0.0");
        write_receipt(&stage, "10.0.0");
        let mut payload = sample_payload(&paths, txid, "10.0.0", "0.0.0", &tools);
        payload.is_fresh_install = true;
        payload.previous_version = None;
        let mut journal = TransactionJournal::new(txid, "update");
        journal.record(TxStep::Preflight, "ok");
        journal.record(TxStep::Stage, serde_json::to_string(&payload).unwrap());
        journal
            .write_to(&paths.journal_path(txid).unwrap())
            .unwrap();
        let runner = RecordingRunner::default();
        {
            let mut q = runner.responses.lock().unwrap();
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })); // rescan before AtHealth
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })); // rollback rescan
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: "[]\n".into(),
                stderr: String::new(),
            })); // listPlugins empty (fresh absence)
        }
        let err = MaintenanceWorker::run_worker_from_journal(
            &paths,
            &runner,
            txid,
            &NoopSleeper,
            Duration::ZERO,
            &|| Duration::from_secs(1),
            Some(WorkerFailPoint::AtHealth),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("health"),
            "unexpected fresh rollback error: {err}"
        );
        assert!(!paths.plugin_root.exists());

        // v9 structural rollback after injected health failure.
        let txid2 = "33333333333333333333333333333333";
        let stage2 = paths.stage_dir(txid2).unwrap();
        write_min_plugin(&stage2, "10.0.0");
        write_receipt(&stage2, "10.0.0");
        // Live tree is v9-shaped (no receipt).
        write_min_plugin(&paths.plugin_root, "9.0.0");
        let before = fs::read(paths.plugin_root.join("Service.qml")).unwrap();
        let mut payload2 = sample_payload(&paths, txid2, "10.0.0", "9.0.0", &tools);
        payload2.is_v9_rollback = true;
        let mut j2 = TransactionJournal::new(txid2, "update");
        j2.record(TxStep::Preflight, "ok");
        j2.record(TxStep::Stage, serde_json::to_string(&payload2).unwrap());
        j2.write_to(&paths.journal_path(txid2).unwrap()).unwrap();
        let runner2 = RecordingRunner::default();
        {
            let mut q = runner2.responses.lock().unwrap();
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })); // rescan
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })); // rollback rescan
            q.push(Ok(CommandOutput {
                code: 0,
                stdout: r#"[{"id":"agent-bar.usage"}]"#.into(),
                stderr: String::new(),
            })); // listPlugins must contain agent-bar.usage
        }
        let err2 = MaintenanceWorker::run_worker_from_journal(
            &paths,
            &runner2,
            txid2,
            &NoopSleeper,
            Duration::ZERO,
            &|| Duration::from_secs(1),
            Some(WorkerFailPoint::AtHealth),
        )
        .unwrap_err();
        let msg2 = err2.to_string();
        assert!(
            msg2.contains("health"),
            "unexpected v9 rollback error: {msg2}"
        );
        assert_eq!(
            fs::read(paths.plugin_root.join("Service.qml")).unwrap(),
            before
        );
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
}
