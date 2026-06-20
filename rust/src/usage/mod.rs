//! Engine de usage/custo: lê session logs locais → tokens → custo (US$/R$).
//! Subsistema PURO (sem TUI/ratatui). Ver spec §4b.

pub mod amp;
pub mod cache;
pub mod claude;
pub mod codex;
pub mod pricing;

use time::OffsetDateTime;

/// Uma chamada de API normalizada, extraída de um session log.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRecord {
    pub provider: String,
    pub model: Option<String>,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub ts: OffsetDateTime,
}
