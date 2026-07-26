//! Settings: v10 canonical store plus temporary v9 legacy surface.

pub mod legacy;
pub mod schema;
pub mod store;

// v10 surface
pub use schema::{
    DisplayMetric, DisplaySettings, NotificationSettings, ProviderIdJson, ProviderSetting,
    Settings as SettingsDocument, SettingsError,
};
pub use store::{
    default_maintenance_lock_path, default_settings_path, file_mtime, SettingsStore, StoreError,
};

// v9 surface kept until Task 19 removes legacy consumers.
pub use legacy::*;
