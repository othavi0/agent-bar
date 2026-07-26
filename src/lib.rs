#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod app_identity;
pub mod cache;
pub mod cli;
pub mod notifications;
pub mod plugin;
pub mod providers;
pub mod settings;
pub mod status;
pub mod support;
