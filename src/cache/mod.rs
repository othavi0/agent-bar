//! Status-v2 cache store and coordination, plus temporary v9 legacy cache.

pub mod coordinator;
pub mod legacy;
pub mod schema;
pub mod store;

pub use coordinator::{CacheCoordinator, ForcedTargets, GenerationRecord};
pub use schema::{CacheDocument, CacheSchemaError, CachedProvider, CACHE_SCHEMA_VERSION};
pub use store::{entry_from_status, CachePaths, CacheStore, CacheStoreError};

// v9 surface for remaining legacy consumers.
pub use legacy::*;
