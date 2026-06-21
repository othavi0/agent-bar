//! Relógio injetável. Resolvido uma vez no `main` (startup single-thread) e passado
//! adiante — evita chamar `current_local_offset()` em runtime (frágil em multi-thread
//! e não-determinístico em CI). `format_reset_time` usa `local_offset` p/ HH:MM local.

use time::{OffsetDateTime, UtcOffset};

#[derive(Debug, Clone, Copy)]
pub struct Clock {
    pub now: OffsetDateTime,
    pub local_offset: UtcOffset,
}

impl Clock {
    /// Resolve agora (UTC) + offset local do SO. Se o offset não puder ser determinado
    /// (ex.: chamado após spawn de threads), cai para UTC.
    pub fn from_env() -> Self {
        let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        Self {
            now: OffsetDateTime::now_utc(),
            local_offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_does_not_panic() {
        let c = Clock::from_env();
        // smoke: campos acessíveis
        let _ = c.now.unix_timestamp();
        let _ = c.local_offset.whole_hours();
    }
}
