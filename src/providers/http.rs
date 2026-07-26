//! Injectable HTTP client for provider collection (Claude).

use std::time::Duration;

pub use super::adapter::HttpResponse;
use super::adapter::{BoxFuture, HttpClient, HttpError};

/// Production reqwest client: HTTPS only, no redirects, body size limit.
#[derive(Debug, Clone)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl ReqwestHttpClient {
    pub fn new(timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { client })
    }
}

impl HttpClient for ReqwestHttpClient {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_body_bytes: usize,
    ) -> BoxFuture<'_, Result<HttpResponse, HttpError>> {
        let url = url.to_owned();
        let headers: Vec<(String, String)> = headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Box::pin(async move {
            if !(url.starts_with("https://")) {
                return Err(HttpError::InvalidResponse(
                    "only https URLs are allowed".into(),
                ));
            }
            let mut req = self.client.get(&url);
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            let response = req.send().await.map_err(|err| {
                // Never include authorization-bearing error details.
                HttpError::Network(err.without_url().to_string())
            })?;
            let status = response.status().as_u16();
            let final_url = response.url().to_string();
            if response.status().is_redirection() {
                return Err(HttpError::RedirectRefused(final_url));
            }
            // Cap body before buffering the full payload.
            let mut body = Vec::new();
            let mut stream = response.bytes_stream();
            use futures::StreamExt;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|err| HttpError::Network(err.to_string()))?;
                if body.len().saturating_add(chunk.len()) > max_body_bytes {
                    return Err(HttpError::BodyTooLarge);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(HttpResponse {
                status,
                final_url,
                body,
            })
        })
    }
}

/// Test double with scripted responses.
#[derive(Debug, Default)]
pub struct ScriptedHttpClient {
    pub responses: std::sync::Mutex<Vec<Result<HttpResponse, HttpError>>>,
    pub last_url: std::sync::Mutex<Option<String>>,
    pub last_headers: std::sync::Mutex<Vec<(String, String)>>,
}

impl ScriptedHttpClient {
    pub fn single(response: Result<HttpResponse, HttpError>) -> Self {
        Self {
            responses: std::sync::Mutex::new(vec![response]),
            last_url: std::sync::Mutex::new(None),
            last_headers: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl HttpClient for ScriptedHttpClient {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        _max_body_bytes: usize,
    ) -> BoxFuture<'_, Result<HttpResponse, HttpError>> {
        *self.last_url.lock().unwrap_or_else(|e| e.into_inner()) = Some(url.to_owned());
        *self.last_headers.lock().unwrap_or_else(|e| e.into_inner()) = headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let next = self
            .responses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop()
            .unwrap_or_else(|| Err(HttpError::Network("scripted HTTP client exhausted".into())));
        Box::pin(async move { next })
    }
}
