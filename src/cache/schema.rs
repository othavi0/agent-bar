//! Normalized status-v2 cache document (CACHE schema).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::cli::ProviderId;
use crate::status::schema::ProviderStatus;

pub const CACHE_SCHEMA_VERSION: u32 = 2;

/// One provider row in the normalized cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedProvider {
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub completed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    /// Validated provider status object (no request envelope fields).
    pub status: ProviderStatus,
}

/// Closed cache document under `$XDG_CACHE_HOME/agent-bar/status-v2.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CacheDocument {
    pub schema_version: u32,
    pub revision: u64,
    pub providers: BTreeMap<String, CachedProvider>,
}

impl CacheDocument {
    pub fn empty() -> Self {
        Self {
            schema_version: CACHE_SCHEMA_VERSION,
            revision: 0,
            providers: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<(), CacheSchemaError> {
        if self.schema_version != CACHE_SCHEMA_VERSION {
            return Err(CacheSchemaError::Version);
        }
        for (key, entry) in &self.providers {
            if ProviderId::parse_word(key).is_none() {
                return Err(CacheSchemaError::UnknownProvider(key.clone()));
            }
            if entry.status.id().as_str() != key {
                return Err(CacheSchemaError::IdMismatch);
            }
            entry
                .status
                .validate_state_shape()
                .map_err(|err| CacheSchemaError::InvalidJson(err.message().to_owned()))?;
        }
        Ok(())
    }

    pub fn get(&self, id: ProviderId) -> Option<&CachedProvider> {
        self.providers.get(id.as_str())
    }

    pub fn is_fresh(&self, id: ProviderId, now: OffsetDateTime) -> bool {
        self.get(id).is_some_and(|entry| now < entry.expires_at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheSchemaError {
    Version,
    UnknownProvider(String),
    IdMismatch,
    InvalidJson(String),
}

impl std::fmt::Display for CacheSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Version => write!(f, "cache schemaVersion must be {CACHE_SCHEMA_VERSION}"),
            Self::UnknownProvider(id) => write!(f, "unknown cache provider '{id}'"),
            Self::IdMismatch => write!(f, "cache provider key/status id mismatch"),
            Self::InvalidJson(msg) => write!(f, "invalid cache JSON: {msg}"),
        }
    }
}

impl std::error::Error for CacheSchemaError {}

// ProviderStatus needs a public validate_state_shape - it's private today.
// We'll add a thin public wrapper on ProviderStatus.
