//! Settings: v10 canonical store and v9→v10 data migration.

pub mod migration;
pub mod schema;
pub mod store;

pub use migration::{
    apply_migration_plan, migrate_live_paths, MigrationApplyReport, MigrationError, MigrationPlan,
};
pub use schema::{
    DisplayMetric, DisplaySettings, NotificationSettings, ProviderIdJson, ProviderSetting,
    Settings as SettingsDocument, SettingsError,
};
pub use store::{
    default_maintenance_lock_path, default_settings_path, file_mtime, SettingsStore, StoreError,
};
