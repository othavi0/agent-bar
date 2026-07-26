//! Schema-v2 domain types and semantic validation.
//!
//! Structural JSON Schema checks live in `schemas/status-v2.schema.json`.
//! This module owns every cross-field invariant JSON Schema cannot express
//! safely, and is the only serialization boundary for status JSON.

use std::collections::HashSet;
use std::fmt;

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::cli::{CacheMode, ProviderId, SERIALIZATION};

const SCHEMA_VERSION: u32 = 2;
const PERCENT_SUM_TOLERANCE: f64 = 0.01;

/// Semantic or construction failure for status schema v2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError {
    message: String,
}

impl SchemaError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SchemaError {}

/// Failure writing status JSON (maps to helper exit code 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusOutputError {
    message: String,
}

impl StatusOutputError {
    pub fn semantics(err: SchemaError) -> Self {
        Self {
            message: err.message,
        }
    }

    pub fn serialize(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn exit_code(&self) -> i32 {
        SERIALIZATION
    }
}

impl fmt::Display for StatusOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for StatusOutputError {}

impl From<SchemaError> for StatusOutputError {
    fn from(value: SchemaError) -> Self {
        Self::semantics(value)
    }
}

/// Completed provider state (helper response; never `loading`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderState {
    Ready,
    Stale,
    CliMissing,
    Unauthenticated,
    RateLimited,
    NetworkError,
    ProviderError,
}

impl ProviderState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Stale => "stale",
            Self::CliMissing => "cli_missing",
            Self::Unauthenticated => "unauthenticated",
            Self::RateLimited => "rate_limited",
            Self::NetworkError => "network_error",
            Self::ProviderError => "provider_error",
        }
    }
}

/// Provenance of retained provider data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    Live,
    Cache,
}

impl DataSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Cache => "cache",
        }
    }
}

/// Closed error codes for provider failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    CliNotFound,
    AuthenticationRequired,
    RateLimited,
    NetworkError,
    ProviderError,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CliNotFound => "cli_not_found",
            Self::AuthenticationRequired => "authentication_required",
            Self::RateLimited => "rate_limited",
            Self::NetworkError => "network_error",
            Self::ProviderError => "provider_error",
        }
    }
}

/// Closed action kinds for QML service methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Retry,
    Login,
    ViewInstallation,
}

impl ActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::Login => "login",
            Self::ViewInstallation => "view_installation",
        }
    }
}

/// Typed provider error payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn new(code: ErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}

/// Typed provider action payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAction {
    pub kind: ActionKind,
    pub label: String,
    pub target: Option<String>,
}

impl ProviderAction {
    pub fn retry(label: impl Into<String>) -> Self {
        Self {
            kind: ActionKind::Retry,
            label: label.into(),
            target: None,
        }
    }

    pub fn login(label: impl Into<String>) -> Self {
        Self {
            kind: ActionKind::Login,
            label: label.into(),
            target: None,
        }
    }

    pub fn view_installation(
        label: impl Into<String>,
        target: impl Into<String>,
    ) -> Result<Self, SchemaError> {
        let target = target.into();
        if !target.starts_with("https://") {
            return Err(SchemaError::new(
                "view_installation target must be an https URL",
            ));
        }
        Ok(Self {
            kind: ActionKind::ViewInstallation,
            label: label.into(),
            target: Some(target),
        })
    }
}

/// Plan identity shown in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub label: String,
}

/// Account label (sanitized; never credentials).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub label: String,
}

/// One percentage quota window with validated percentages.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    id: String,
    label: String,
    used_percent: f64,
    remaining_percent: f64,
    #[serde(with = "time::serde::rfc3339::option")]
    resets_at: Option<OffsetDateTime>,
}

impl UsageWindow {
    /// Construct a window after enforcing finite range and sum invariants.
    pub fn try_new(
        id: impl Into<String>,
        label: impl Into<String>,
        used_percent: f64,
        remaining_percent: f64,
        resets_at: Option<OffsetDateTime>,
    ) -> Result<Self, SchemaError> {
        let id = id.into();
        let label = label.into();
        if id.is_empty() {
            return Err(SchemaError::new("window id must be non-empty"));
        }
        if label.is_empty() {
            return Err(SchemaError::new("window label must be non-empty"));
        }
        validate_percent("usedPercent", used_percent)?;
        validate_percent("remainingPercent", remaining_percent)?;
        let sum = used_percent + remaining_percent;
        if (sum - 100.0).abs() > PERCENT_SUM_TOLERANCE {
            return Err(SchemaError::new(format!(
                "usedPercent + remainingPercent must equal 100 ± {PERCENT_SUM_TOLERANCE}; got {sum}"
            )));
        }
        if let Some(ts) = resets_at {
            require_utc(ts, "resetsAt")?;
        }
        Ok(Self {
            id,
            label,
            used_percent,
            remaining_percent,
            resets_at,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn used_percent(&self) -> f64 {
        self.used_percent
    }

    pub fn remaining_percent(&self) -> f64 {
        self.remaining_percent
    }

    pub fn resets_at(&self) -> Option<OffsetDateTime> {
        self.resets_at
    }
}

/// One provider row in a status envelope.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    id: ProviderIdSerde,
    name: String,
    state: ProviderState,
    source: Option<DataSource>,
    plan: Option<Plan>,
    account: Option<Account>,
    windows: Vec<UsageWindow>,
    #[serde(with = "time::serde::rfc3339::option")]
    last_success_at: Option<OffsetDateTime>,
    error: Option<ProviderError>,
    action: Option<ProviderAction>,
}

/// Serde adapter so [`ProviderId`] serializes as a lowercase string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProviderIdSerde(ProviderId);

impl Serialize for ProviderIdSerde {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl ProviderStatus {
    pub fn id(&self) -> ProviderId {
        self.id.0
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn state(&self) -> ProviderState {
        self.state
    }

    pub fn source(&self) -> Option<DataSource> {
        self.source
    }

    pub fn windows(&self) -> &[UsageWindow] {
        &self.windows
    }

    pub fn last_success_at(&self) -> Option<OffsetDateTime> {
        self.last_success_at
    }

    pub fn error(&self) -> Option<&ProviderError> {
        self.error.as_ref()
    }

    pub fn action(&self) -> Option<&ProviderAction> {
        self.action.as_ref()
    }

    pub fn ready(
        id: ProviderId,
        name: impl Into<String>,
        source: DataSource,
        plan: Option<Plan>,
        account: Option<Account>,
        windows: Vec<UsageWindow>,
        last_success_at: OffsetDateTime,
    ) -> Result<Self, SchemaError> {
        require_utc(last_success_at, "lastSuccessAt")?;
        ensure_unique_window_ids(&windows)?;
        Ok(Self {
            id: ProviderIdSerde(id),
            name: name.into(),
            state: ProviderState::Ready,
            source: Some(source),
            plan,
            account,
            windows,
            last_success_at: Some(last_success_at),
            error: None,
            action: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn stale(
        id: ProviderId,
        name: impl Into<String>,
        plan: Option<Plan>,
        account: Option<Account>,
        windows: Vec<UsageWindow>,
        last_success_at: OffsetDateTime,
        error: ProviderError,
        action: ProviderAction,
    ) -> Result<Self, SchemaError> {
        require_utc(last_success_at, "lastSuccessAt")?;
        ensure_unique_window_ids(&windows)?;
        if action.kind != ActionKind::Retry {
            return Err(SchemaError::new("stale action must be retry"));
        }
        Ok(Self {
            id: ProviderIdSerde(id),
            name: name.into(),
            state: ProviderState::Stale,
            source: Some(DataSource::Cache),
            plan,
            account,
            windows,
            last_success_at: Some(last_success_at),
            error: Some(error),
            action: Some(action),
        })
    }

    pub fn cli_missing(
        id: ProviderId,
        name: impl Into<String>,
        error: ProviderError,
        action: ProviderAction,
    ) -> Result<Self, SchemaError> {
        if error.code != ErrorCode::CliNotFound {
            return Err(SchemaError::new(
                "cli_missing error code must be cli_not_found",
            ));
        }
        if action.kind != ActionKind::ViewInstallation {
            return Err(SchemaError::new(
                "cli_missing action must be view_installation",
            ));
        }
        Ok(Self {
            id: ProviderIdSerde(id),
            name: name.into(),
            state: ProviderState::CliMissing,
            source: None,
            plan: None,
            account: None,
            windows: Vec::new(),
            last_success_at: None,
            error: Some(error),
            action: Some(action),
        })
    }

    pub fn unauthenticated(
        id: ProviderId,
        name: impl Into<String>,
        error: ProviderError,
        action: ProviderAction,
    ) -> Result<Self, SchemaError> {
        if error.code != ErrorCode::AuthenticationRequired {
            return Err(SchemaError::new(
                "unauthenticated error code must be authentication_required",
            ));
        }
        if !matches!(
            action.kind,
            ActionKind::Login | ActionKind::ViewInstallation
        ) {
            return Err(SchemaError::new(
                "unauthenticated action must be login or view_installation",
            ));
        }
        Ok(Self {
            id: ProviderIdSerde(id),
            name: name.into(),
            state: ProviderState::Unauthenticated,
            source: None,
            plan: None,
            account: None,
            windows: Vec::new(),
            last_success_at: None,
            error: Some(error),
            action: Some(action),
        })
    }

    pub fn rate_limited(
        id: ProviderId,
        name: impl Into<String>,
        error: ProviderError,
        action: ProviderAction,
    ) -> Result<Self, SchemaError> {
        failure_state(
            id,
            name,
            ProviderState::RateLimited,
            ErrorCode::RateLimited,
            true,
            error,
            action,
        )
    }

    pub fn network_error(
        id: ProviderId,
        name: impl Into<String>,
        error: ProviderError,
        action: ProviderAction,
    ) -> Result<Self, SchemaError> {
        failure_state(
            id,
            name,
            ProviderState::NetworkError,
            ErrorCode::NetworkError,
            true,
            error,
            action,
        )
    }

    pub fn provider_error(
        id: ProviderId,
        name: impl Into<String>,
        error: ProviderError,
        action: ProviderAction,
    ) -> Result<Self, SchemaError> {
        if error.code != ErrorCode::ProviderError {
            return Err(SchemaError::new(
                "provider_error error code must be provider_error",
            ));
        }
        if action.kind != ActionKind::Retry {
            return Err(SchemaError::new("provider_error action must be retry"));
        }
        Ok(Self {
            id: ProviderIdSerde(id),
            name: name.into(),
            state: ProviderState::ProviderError,
            source: None,
            plan: None,
            account: None,
            windows: Vec::new(),
            last_success_at: None,
            error: Some(error),
            action: Some(action),
        })
    }

    fn validate_state_shape(&self) -> Result<(), SchemaError> {
        match self.state {
            ProviderState::Ready => {
                if self.source.is_none() {
                    return Err(SchemaError::new("ready requires source live|cache"));
                }
                if self.last_success_at.is_none() {
                    return Err(SchemaError::new("ready requires lastSuccessAt"));
                }
                if self.error.is_some() || self.action.is_some() {
                    return Err(SchemaError::new("ready requires null error and action"));
                }
            }
            ProviderState::Stale => {
                if self.source != Some(DataSource::Cache) {
                    return Err(SchemaError::new("stale requires source cache"));
                }
                if self.last_success_at.is_none() {
                    return Err(SchemaError::new("stale requires lastSuccessAt"));
                }
                if self.error.is_none() {
                    return Err(SchemaError::new("stale requires error"));
                }
                if self.action.as_ref().map(|a| a.kind) != Some(ActionKind::Retry) {
                    return Err(SchemaError::new("stale requires retry action"));
                }
            }
            ProviderState::CliMissing
            | ProviderState::Unauthenticated
            | ProviderState::RateLimited
            | ProviderState::NetworkError
            | ProviderState::ProviderError => {
                if self.source.is_some() {
                    return Err(SchemaError::new(format!(
                        "{} requires null source",
                        self.state.as_str()
                    )));
                }
                if !self.windows.is_empty() {
                    return Err(SchemaError::new(format!(
                        "{} requires empty windows",
                        self.state.as_str()
                    )));
                }
                if self.last_success_at.is_some() {
                    return Err(SchemaError::new(format!(
                        "{} requires null lastSuccessAt",
                        self.state.as_str()
                    )));
                }
                if self.error.is_none() || self.action.is_none() {
                    return Err(SchemaError::new(format!(
                        "{} requires error and action",
                        self.state.as_str()
                    )));
                }
            }
        }
        ensure_unique_window_ids(&self.windows)?;
        Ok(())
    }
}

fn failure_state(
    id: ProviderId,
    name: impl Into<String>,
    state: ProviderState,
    expected_code: ErrorCode,
    expect_retryable: bool,
    error: ProviderError,
    action: ProviderAction,
) -> Result<ProviderStatus, SchemaError> {
    if error.code != expected_code {
        return Err(SchemaError::new(format!(
            "{} error code must be {}",
            state.as_str(),
            expected_code.as_str()
        )));
    }
    if expect_retryable && !error.retryable {
        return Err(SchemaError::new(format!(
            "{} error must be retryable",
            state.as_str()
        )));
    }
    if action.kind != ActionKind::Retry {
        return Err(SchemaError::new(format!(
            "{} action must be retry",
            state.as_str()
        )));
    }
    Ok(ProviderStatus {
        id: ProviderIdSerde(id),
        name: name.into(),
        state,
        source: None,
        plan: None,
        account: None,
        windows: Vec::new(),
        last_success_at: None,
        error: Some(error),
        action: Some(action),
    })
}

/// Request echo embedded in the status envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusRequest {
    #[serde(serialize_with = "serialize_optional_provider")]
    pub provider: Option<ProviderId>,
    #[serde(serialize_with = "serialize_cache_mode")]
    pub cache: CacheMode,
}

fn serialize_optional_provider<S>(
    value: &Option<ProviderId>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match value {
        Some(id) => serializer.serialize_some(id.as_str()),
        None => serializer.serialize_none(),
    }
}

fn serialize_cache_mode<S>(value: &CacheMode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let word = match value {
        CacheMode::Use => "use",
        CacheMode::Bypass => "bypass",
    };
    serializer.serialize_str(word)
}

/// Completed status schema-v2 envelope.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEnvelope {
    schema_version: u32,
    helper_version: String,
    #[serde(with = "time::serde::rfc3339")]
    generated_at: OffsetDateTime,
    request: StatusRequest,
    providers: Vec<ProviderStatus>,
}

impl StatusEnvelope {
    pub fn try_new(
        helper_version: impl Into<String>,
        generated_at: OffsetDateTime,
        request: StatusRequest,
        providers: Vec<ProviderStatus>,
    ) -> Result<Self, SchemaError> {
        require_utc(generated_at, "generatedAt")?;
        let envelope = Self {
            schema_version: SCHEMA_VERSION,
            helper_version: helper_version.into(),
            generated_at,
            request,
            providers,
        };
        envelope.validate_semantics()?;
        Ok(envelope)
    }

    /// Build an envelope using the running package version as `helperVersion`.
    pub fn try_new_for_package(
        generated_at: OffsetDateTime,
        request: StatusRequest,
        providers: Vec<ProviderStatus>,
    ) -> Result<Self, SchemaError> {
        Self::try_new(env!("CARGO_PKG_VERSION"), generated_at, request, providers)
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn helper_version(&self) -> &str {
        &self.helper_version
    }

    pub fn generated_at(&self) -> OffsetDateTime {
        self.generated_at
    }

    pub fn request(&self) -> &StatusRequest {
        &self.request
    }

    pub fn providers(&self) -> &[ProviderStatus] {
        &self.providers
    }

    /// Every Rust-owned semantic invariant for a completed envelope.
    pub fn validate_semantics(&self) -> Result<(), SchemaError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(SchemaError::new("schemaVersion must be 2"));
        }
        if self.helper_version != env!("CARGO_PKG_VERSION") {
            return Err(SchemaError::new(format!(
                "helperVersion must equal package version {}; got {}",
                env!("CARGO_PKG_VERSION"),
                self.helper_version
            )));
        }
        require_utc(self.generated_at, "generatedAt")?;

        // Explicit-provider response contains exactly that provider.
        if let Some(expected) = self.request.provider {
            if self.providers.len() != 1 {
                return Err(SchemaError::new(
                    "explicit provider request must return exactly one provider",
                ));
            }
            if self.providers[0].id() != expected {
                return Err(SchemaError::new(
                    "explicit provider response must match the requested provider id",
                ));
            }
        }

        let mut seen_ids = HashSet::new();
        for provider in &self.providers {
            if !seen_ids.insert(provider.id()) {
                return Err(SchemaError::new(format!(
                    "duplicate provider id '{}'",
                    provider.id()
                )));
            }
            provider.validate_state_shape()?;
            for window in provider.windows() {
                validate_percent("usedPercent", window.used_percent())?;
                validate_percent("remainingPercent", window.remaining_percent())?;
                let sum = window.used_percent() + window.remaining_percent();
                if (sum - 100.0).abs() > PERCENT_SUM_TOLERANCE {
                    return Err(SchemaError::new(format!(
                        "window '{}' percent sum out of tolerance",
                        window.id()
                    )));
                }
                if let Some(ts) = window.resets_at() {
                    require_utc(ts, "resetsAt")?;
                }
            }
            if let Some(ts) = provider.last_success_at() {
                require_utc(ts, "lastSuccessAt")?;
            }
        }

        // Request/settings order: when no explicit provider, IDs must be unique
        // (already checked) and callers supply settings order. We only prove
        // uniqueness here; order preservation is the coordinator's duty and is
        // checked when a requested order is provided.
        Ok(())
    }

    /// Validate then serialize exactly one JSON object plus a trailing newline.
    pub fn to_json_line(&self) -> Result<String, StatusOutputError> {
        self.validate_semantics()?;
        let mut body = serde_json::to_string(self).map_err(|err| {
            StatusOutputError::serialize(format!("status serialization failed: {err}"))
        })?;
        body.push('\n');
        Ok(body)
    }

    /// Validate request/settings provider order against an expected sequence.
    pub fn validate_provider_order(&self, expected: &[ProviderId]) -> Result<(), SchemaError> {
        let actual: Vec<ProviderId> = self.providers.iter().map(ProviderStatus::id).collect();
        if actual.as_slice() != expected {
            return Err(SchemaError::new(format!(
                "providers must follow request/settings order; expected {expected:?}, got {actual:?}"
            )));
        }
        Ok(())
    }
}

/// Temporary collection result owned here until Task 6 moves it to providers.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderResult {
    Ready {
        id: ProviderId,
        name: String,
        source: DataSource,
        plan: Option<Plan>,
        account: Option<Account>,
        windows: Vec<UsageWindow>,
        last_success_at: OffsetDateTime,
    },
    Stale {
        id: ProviderId,
        name: String,
        plan: Option<Plan>,
        account: Option<Account>,
        windows: Vec<UsageWindow>,
        last_success_at: OffsetDateTime,
        error: ProviderError,
    },
    CliMissing {
        id: ProviderId,
        name: String,
        message: String,
        installation_url: String,
    },
    Unauthenticated {
        id: ProviderId,
        name: String,
        message: String,
        login_available: bool,
        installation_url: String,
    },
    RateLimited {
        id: ProviderId,
        name: String,
        message: String,
    },
    NetworkError {
        id: ProviderId,
        name: String,
        message: String,
    },
    ProviderError {
        id: ProviderId,
        name: String,
        message: String,
        retryable: bool,
    },
}

fn validate_percent(field: &str, value: f64) -> Result<(), SchemaError> {
    if !value.is_finite() {
        return Err(SchemaError::new(format!("{field} must be a finite number")));
    }
    if !(0.0..=100.0).contains(&value) {
        return Err(SchemaError::new(format!(
            "{field} must be in 0..=100; got {value}"
        )));
    }
    Ok(())
}

fn require_utc(ts: OffsetDateTime, field: &str) -> Result<(), SchemaError> {
    if ts.offset() != UtcOffset::UTC {
        return Err(SchemaError::new(format!("{field} must be UTC RFC 3339")));
    }
    // Round-trip through RFC3339 to reject pathological values early.
    ts.format(&Rfc3339).map_err(|err| {
        SchemaError::new(format!("{field} is not a valid RFC 3339 timestamp: {err}"))
    })?;
    Ok(())
}

fn ensure_unique_window_ids(windows: &[UsageWindow]) -> Result<(), SchemaError> {
    let mut seen = HashSet::new();
    for window in windows {
        if !seen.insert(window.id().to_owned()) {
            return Err(SchemaError::new(format!(
                "duplicate window id '{}'",
                window.id()
            )));
        }
    }
    Ok(())
}

/// Test-only hook: force `to_json_line` serialization path failure mapping.
#[cfg(test)]
pub(crate) fn map_serialize_error(err: impl fmt::Display) -> StatusOutputError {
    StatusOutputError::serialize(format!("status serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ProviderId;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use time::macros::datetime;

    fn ts() -> OffsetDateTime {
        datetime!(2026-07-26 18:42:00 UTC)
    }

    fn session_window() -> UsageWindow {
        UsageWindow::try_new(
            "session",
            "Session",
            42.0,
            58.0,
            Some(datetime!(2026-07-26 22:00:00 UTC)),
        )
        .unwrap()
    }

    fn ready_claude() -> ProviderStatus {
        ProviderStatus::ready(
            ProviderId::Claude,
            "Claude",
            DataSource::Live,
            Some(Plan {
                id: "max".into(),
                label: "Max".into(),
            }),
            Some(Account {
                label: "Personal".into(),
            }),
            vec![session_window()],
            ts(),
        )
        .unwrap()
    }

    fn package_envelope(providers: Vec<ProviderStatus>, request: StatusRequest) -> StatusEnvelope {
        StatusEnvelope::try_new_for_package(ts(), request, providers).unwrap()
    }

    #[test]
    fn usage_window_rejects_invalid_percent_and_sum() {
        assert!(UsageWindow::try_new("s", "S", 101.0, 0.0, None).is_err());
        assert!(UsageWindow::try_new("s", "S", -1.0, 101.0, None).is_err());
        assert!(UsageWindow::try_new("s", "S", f64::NAN, 0.0, None).is_err());
        assert!(UsageWindow::try_new("s", "S", f64::INFINITY, 0.0, None).is_err());
        assert!(UsageWindow::try_new("s", "S", 40.0, 50.0, None).is_err());
        assert!(UsageWindow::try_new("s", "S", 40.0, 60.0, None).is_ok());
        assert!(UsageWindow::try_new("s", "S", 40.005, 59.995, None).is_ok());
    }

    #[test]
    fn usage_window_rejects_non_utc_reset() {
        let local = datetime!(2026-07-26 22:00:00 +01:00:00);
        assert!(UsageWindow::try_new("s", "S", 50.0, 50.0, Some(local)).is_err());
    }

    #[test]
    fn truth_table_states_serialize() {
        let cases: Vec<ProviderStatus> = vec![
            ready_claude(),
            ProviderStatus::ready(
                ProviderId::Codex,
                "Codex",
                DataSource::Live,
                Some(Plan {
                    id: "plus".into(),
                    label: "Plus".into(),
                }),
                Some(Account {
                    label: "Work".into(),
                }),
                vec![],
                ts(),
            )
            .unwrap(),
            ProviderStatus::stale(
                ProviderId::Grok,
                "Grok",
                Some(Plan {
                    id: "super".into(),
                    label: "SuperGrok".into(),
                }),
                Some(Account {
                    label: "Personal".into(),
                }),
                vec![UsageWindow::try_new("session", "Session", 90.0, 10.0, None).unwrap()],
                datetime!(2026-07-26 18:40:00 UTC),
                ProviderError::new(ErrorCode::NetworkError, "Temporary network failure", true),
                ProviderAction::retry("Retry"),
            )
            .unwrap(),
            ProviderStatus::cli_missing(
                ProviderId::Amp,
                "Amp",
                ProviderError::new(ErrorCode::CliNotFound, "Amp CLI was not found.", false),
                ProviderAction::view_installation(
                    "View installation",
                    "https://ampcode.com/manual",
                )
                .unwrap(),
            )
            .unwrap(),
            ProviderStatus::unauthenticated(
                ProviderId::Claude,
                "Claude",
                ProviderError::new(
                    ErrorCode::AuthenticationRequired,
                    "Claude is not authenticated.",
                    false,
                ),
                ProviderAction::login("Log in"),
            )
            .unwrap(),
            ProviderStatus::rate_limited(
                ProviderId::Codex,
                "Codex",
                ProviderError::new(ErrorCode::RateLimited, "rate limited", true),
                ProviderAction::retry("Retry"),
            )
            .unwrap(),
            ProviderStatus::network_error(
                ProviderId::Grok,
                "Grok",
                ProviderError::new(ErrorCode::NetworkError, "network", true),
                ProviderAction::retry("Retry"),
            )
            .unwrap(),
            ProviderStatus::provider_error(
                ProviderId::Amp,
                "Amp",
                ProviderError::new(ErrorCode::ProviderError, "bad payload", false),
                ProviderAction::retry("Retry"),
            )
            .unwrap(),
        ];

        for provider in cases {
            let request = StatusRequest {
                provider: Some(provider.id()),
                cache: CacheMode::Use,
            };
            let envelope = package_envelope(vec![provider], request);
            let line = envelope.to_json_line().unwrap();
            assert!(line.ends_with('\n'));
            assert_eq!(line.matches('\n').count(), 1);
            let value: Value = serde_json::from_str(line.trim_end()).unwrap();
            assert_eq!(value["schemaVersion"], 2);
            assert_eq!(value["helperVersion"], env!("CARGO_PKG_VERSION"));
            assert!(value.get("spend").is_none());
            assert!(value.get("balance").is_none());
            assert!(value.get("credits").is_none());
            let state = value["providers"][0]["state"].as_str().unwrap();
            assert_ne!(state, "loading");
        }
    }

    #[test]
    fn partial_failure_does_not_invalidate_sibling() {
        let ready = ready_claude();
        let missing = ProviderStatus::cli_missing(
            ProviderId::Codex,
            "Codex",
            ProviderError::new(ErrorCode::CliNotFound, "missing", false),
            ProviderAction::view_installation("View installation", "https://example.com/codex")
                .unwrap(),
        )
        .unwrap();
        let envelope = package_envelope(
            vec![ready, missing],
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
        );
        let value: Value =
            serde_json::from_str(envelope.to_json_line().unwrap().trim_end()).unwrap();
        assert_eq!(value["providers"][0]["state"], "ready");
        assert_eq!(value["providers"][1]["state"], "cli_missing");
    }

    #[test]
    fn rejects_duplicate_provider_and_window_ids() {
        let a = ready_claude();
        let b = ProviderStatus::ready(
            ProviderId::Claude,
            "Claude",
            DataSource::Cache,
            None,
            None,
            vec![],
            ts(),
        )
        .unwrap();
        let err = StatusEnvelope::try_new_for_package(
            ts(),
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
            vec![a, b],
        )
        .unwrap_err();
        assert!(err.message().contains("duplicate provider"));

        let dup_windows = ProviderStatus::ready(
            ProviderId::Claude,
            "Claude",
            DataSource::Live,
            None,
            None,
            vec![
                UsageWindow::try_new("session", "Session", 10.0, 90.0, None).unwrap(),
                UsageWindow::try_new("session", "Session again", 20.0, 80.0, None).unwrap(),
            ],
            ts(),
        );
        assert!(dup_windows.is_err());
    }

    #[test]
    fn explicit_provider_must_match_single_row() {
        let err = StatusEnvelope::try_new_for_package(
            ts(),
            StatusRequest {
                provider: Some(ProviderId::Claude),
                cache: CacheMode::Bypass,
            },
            vec![
                ready_claude(),
                ProviderStatus::ready(
                    ProviderId::Codex,
                    "Codex",
                    DataSource::Live,
                    None,
                    None,
                    vec![],
                    ts(),
                )
                .unwrap(),
            ],
        )
        .unwrap_err();
        assert!(err.message().contains("exactly one provider"));

        let err = StatusEnvelope::try_new_for_package(
            ts(),
            StatusRequest {
                provider: Some(ProviderId::Amp),
                cache: CacheMode::Use,
            },
            vec![ready_claude()],
        )
        .unwrap_err();
        assert!(err.message().contains("match the requested provider"));
    }

    #[test]
    fn helper_version_must_equal_package_version() {
        let err = StatusEnvelope::try_new(
            "10.0.0",
            ts(),
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
            vec![ready_claude()],
        )
        .unwrap_err();
        assert!(err.message().contains("helperVersion"));
        assert!(err.message().contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn provider_order_validation() {
        let envelope = package_envelope(
            vec![
                ready_claude(),
                ProviderStatus::ready(
                    ProviderId::Codex,
                    "Codex",
                    DataSource::Live,
                    None,
                    None,
                    vec![],
                    ts(),
                )
                .unwrap(),
            ],
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
        );
        assert!(envelope
            .validate_provider_order(&[ProviderId::Claude, ProviderId::Codex])
            .is_ok());
        assert!(envelope
            .validate_provider_order(&[ProviderId::Codex, ProviderId::Claude])
            .is_err());
    }

    #[test]
    fn unknown_action_target_rejected() {
        let err = ProviderAction::view_installation("Install", "http://insecure.example");
        assert!(err.is_err());
    }

    #[test]
    fn json_line_is_one_object_plus_newline() {
        let envelope = package_envelope(
            vec![ready_claude()],
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
        );
        let line = envelope.to_json_line().unwrap();
        assert!(line.ends_with('\n'));
        let trimmed = line.trim_end_matches('\n');
        assert!(!trimmed.contains('\n'));
        let _: Value = serde_json::from_str(trimmed).unwrap();
    }

    #[test]
    fn serialize_error_maps_to_exit_4() {
        let err = map_serialize_error("forced failure");
        assert_eq!(err.exit_code(), SERIALIZATION);
        assert!(!err.message().is_empty());
        assert!(err.message().contains("forced failure"));
    }

    #[test]
    fn money_field_fixture_never_round_trips_into_domain() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let raw =
            fs::read_to_string(root.join("tests/fixtures/status-v2/money-field.json")).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        // Domain construction has no spend field; fixture remains a rejection sample only.
        assert!(value["providers"][0]["windows"][0]
            .get("spendUsd")
            .is_some());
        let envelope = package_envelope(
            vec![ready_claude()],
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
        );
        let line = envelope.to_json_line().unwrap();
        assert!(!line.contains("spend"));
        assert!(!line.contains("balance"));
        assert!(!line.contains("credits"));
        assert!(!line.contains("currency"));
    }

    #[test]
    fn valid_fixtures_share_shape_with_domain_serialization() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ready = fs::read_to_string(root.join("tests/fixtures/status-v2/ready.json")).unwrap();
        let fixture: Value = serde_json::from_str(&ready).unwrap();
        let envelope = package_envelope(
            vec![ready_claude()],
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
        );
        let produced: Value =
            serde_json::from_str(envelope.to_json_line().unwrap().trim_end()).unwrap();
        assert_eq!(produced["schemaVersion"], fixture["schemaVersion"]);
        assert_eq!(produced["request"], fixture["request"]);
        assert_eq!(
            produced["providers"][0]["state"],
            fixture["providers"][0]["state"]
        );
        assert_eq!(
            produced["providers"][0]["windows"][0]["usedPercent"],
            fixture["providers"][0]["windows"][0]["usedPercent"]
        );
    }

    #[test]
    fn jsonschema_accepts_domain_output() {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let schema: Value = serde_json::from_str(
            &fs::read_to_string(root.join("schemas/status-v2.schema.json")).unwrap(),
        )
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let envelope = package_envelope(
            vec![ready_claude()],
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
        );
        let value: Value =
            serde_json::from_str(envelope.to_json_line().unwrap().trim_end()).unwrap();
        let errors: Vec<_> = validator
            .iter_errors(&value)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
    }
}
