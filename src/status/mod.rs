//! Status collection domain: schema-v2 types, mapping, and human formatting.

pub mod collect;
pub mod human;
pub mod schema;

pub use collect::provider_status_from_result;
pub use human::format_human;
pub use schema::{
    Account, ActionKind, DataSource, ErrorCode, Plan, ProviderAction, ProviderError,
    ProviderResult, ProviderState, ProviderStatus, SchemaError, StatusEnvelope, StatusOutputError,
    StatusRequest, UsageWindow,
};
