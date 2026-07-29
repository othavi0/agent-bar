//! Status-v2 cache store and coordination.

pub mod coordinator;
pub mod schema;
pub mod store;

pub use coordinator::{CacheCoordinator, GenerationRecord};
pub use schema::{CacheDocument, CacheSchemaError, CachedProvider, CACHE_SCHEMA_VERSION};
pub use store::{entry_from_status, CachePaths, CacheStore, CacheStoreError};
