//! Reusable TUI widget helpers.
//!
//! Design choice: these are functions/helpers rather than `Widget` trait impls.
//! Functions that return styled `Span`/`Line`/`ListItem` values compose better
//! into the existing `Paragraph`/`Table`/`List` call sites without fighting
//! ratatui's ownership model.

pub mod chips;
pub mod key_hint;
pub mod quota_gauge;
pub mod severity;
pub mod sparkline;
