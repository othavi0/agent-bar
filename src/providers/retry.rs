//! Shared one-transient-retry loop for HTTP collection.
//!
//! The catalog declares [`RetryPolicy::OneTransient`] for every provider; this
//! is the single implementation of that promise. Only network errors retry.

use super::adapter::{HttpClient, HttpError, HttpResponse};
use super::catalog::ProviderDescriptor;

/// GET with at most one extra attempt after a transient network error,
/// honoring the descriptor's retry policy and delay.
pub(crate) async fn http_get_with_retry(
    http: &dyn HttpClient,
    descriptor: &ProviderDescriptor,
    url: &str,
    headers: &[(&str, &str)],
    max_body_bytes: usize,
) -> Result<HttpResponse, HttpError> {
    match http.get(url, headers, max_body_bytes).await {
        Err(HttpError::Network(first)) => {
            let Some(delay) = descriptor.retry_delay() else {
                return Err(HttpError::Network(first));
            };
            tokio::time::sleep(delay).await;
            http.get(url, headers, max_body_bytes).await
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::catalog::CLAUDE;
    use crate::providers::http::ScriptedHttpClient;

    fn ok_response() -> HttpResponse {
        HttpResponse {
            status: 200,
            final_url: "https://example.invalid/".into(),
            body: b"{}".to_vec(),
        }
    }

    #[tokio::test]
    async fn retries_once_after_network_error() {
        // ScriptedHttpClient pops from the END: last entry is served first.
        let http = ScriptedHttpClient {
            responses: std::sync::Mutex::new(vec![
                Ok(ok_response()),
                Err(HttpError::Network("transient".into())),
            ]),
            last_url: std::sync::Mutex::new(None),
            last_headers: std::sync::Mutex::new(Vec::new()),
        };
        let result = http_get_with_retry(&http, &CLAUDE, "https://x/", &[], 1024).await;
        assert!(result.is_ok(), "expected retry to succeed: {result:?}");
        assert!(
            http.responses.lock().unwrap().is_empty(),
            "both scripted responses must be consumed (two attempts)"
        );
    }

    #[tokio::test]
    async fn non_network_errors_do_not_retry() {
        let http = ScriptedHttpClient {
            responses: std::sync::Mutex::new(vec![
                Ok(ok_response()),
                Err(HttpError::RedirectRefused("https://evil/".into())),
            ]),
            last_url: std::sync::Mutex::new(None),
            last_headers: std::sync::Mutex::new(Vec::new()),
        };
        let result = http_get_with_retry(&http, &CLAUDE, "https://x/", &[], 1024).await;
        assert!(matches!(result, Err(HttpError::RedirectRefused(_))));
        assert_eq!(
            http.responses.lock().unwrap().len(),
            1,
            "second scripted response must remain unconsumed (single attempt)"
        );
    }

    #[tokio::test]
    async fn second_network_error_is_returned() {
        let http = ScriptedHttpClient {
            responses: std::sync::Mutex::new(vec![
                Err(HttpError::Network("second".into())),
                Err(HttpError::Network("first".into())),
            ]),
            last_url: std::sync::Mutex::new(None),
            last_headers: std::sync::Mutex::new(Vec::new()),
        };
        let result = http_get_with_retry(&http, &CLAUDE, "https://x/", &[], 1024).await;
        assert!(matches!(result, Err(HttpError::Network(_))));
    }
}
