//! Cliente HTTP compartilhado. Um único `reqwest::Client` (pool de conexões +
//! init do rustls) reusado em todo o processo via `OnceLock` — construir um por
//! request desperdiça o pool. Só o Claude faz HTTP, então os headers default
//! (UA + anthropic-beta) são seguros como default do cliente.

use std::sync::OnceLock;
use std::time::Duration;

use crate::config::{CLAUDE_BETA_HEADER, CLAUDE_USER_AGENT, HTTP_TIMEOUT_SECS};

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Constrói (uma vez) e devolve o cliente compartilhado.
pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "anthropic-beta",
            reqwest::header::HeaderValue::from_static(CLAUDE_BETA_HEADER),
        );
        reqwest::Client::builder()
            .use_rustls_tls()
            .user_agent(CLAUDE_USER_AGENT)
            .default_headers(headers)
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_is_a_reused_singleton() {
        let a = client();
        let b = client();
        assert!(
            std::ptr::eq(a, b),
            "client() deve devolver a mesma instância"
        );
    }
}
