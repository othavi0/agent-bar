//! Emissor NDJSON long-running para `agent-bar --watch`.
//!
//! `build_watch_line` é pura e testável; `start_watch` é o loop de I/O
//! serializado (sem overlap entre ticks). Espelha `src/watch.ts`.

use std::io::IsTerminal as _;
use std::time::Duration;

use tokio::io::AsyncWriteExt as _;
use tokio::time::MissedTickBehavior;

use crate::providers::types::AllQuotas;
use crate::providers::Ctx;

/// Serializa um snapshot de quotas como uma linha NDJSON (JSON + `"\n"`).
/// Espelha `buildWatchLine` de `src/watch.ts`. Em produção `to_json_string`
/// nunca falha; `unwrap_or_default` evita panic sem usar `unwrap()`.
pub fn build_watch_line(quotas: &AllQuotas) -> String {
    format!(
        "{}\n",
        crate::formatters::json::to_json_string(quotas).unwrap_or_default()
    )
}

/// Busca quotas para um tick: single-provider ou fan-out de todos os providers.
/// Recomputa `fetched_at` via `config::now_ms()` a cada tick (espelha
/// `new Date().toISOString()` do TS), para que o timestamp avance entre ticks.
async fn fetch(provider: Option<&str>, ctx: &Ctx<'_>) -> AllQuotas {
    match provider {
        Some(id) => {
            let quota = crate::providers::get_quota_for(id, ctx).await;
            AllQuotas {
                providers: quota.map(|q| vec![q]).unwrap_or_default(),
                fetched_at: crate::providers::iso_from_ms(crate::config::now_ms()),
            }
        }
        None => {
            // fetch_all já preenche fetched_at a partir de ctx.now_ms, mas ctx
            // é fixo no startup. Construímos um Ctx temporário com now_ms fresco
            // para que o fetched_at avance entre ticks.
            let fresh_ms = crate::config::now_ms();
            let fresh_ctx = Ctx {
                client: ctx.client,
                paths: ctx.paths,
                settings: ctx.settings,
                now_ms: fresh_ms,
                local_offset: ctx.local_offset,
                claude_usage_url: ctx.claude_usage_url.clone(),
                version: ctx.version,
                home: ctx.home.clone(),
            };
            crate::providers::fetch_all(&crate::providers::registry(), &fresh_ctx).await
        }
    }
}

/// Emissor NDJSON long-running serializado. Emite já, depois a cada `interval`
/// APÓS o write anterior completar (backpressure; sem overlap). Sai 0 em EPIPE
/// (consumidor fechou o pipe). Nunca retorna no caminho normal.
///
/// Espelha `startWatch` de `src/watch.ts`.
pub async fn start_watch(
    provider: Option<&str>,
    interval: Duration,
    ctx: &Ctx<'_>,
) -> std::io::Result<()> {
    // 1. Valida provider desconhecido antes de qualquer payload.
    if let Some(id) = provider {
        if crate::providers::get_provider(id).is_none() {
            log::error!("[agent-bar] Unknown provider: {id}");
            std::process::exit(1);
        }
    }

    // 2. Avisa se stdout é terminal (o usuário deve pipar p/ um consumidor).
    if std::io::stdout().is_terminal() {
        log::warn!("[agent-bar] watch mode: output is NDJSON — pipe to a consumer");
    }

    // 3. Loop serializado.
    //    `MissedTickBehavior::Delay`: se um tick demorar mais que `interval`,
    //    o próximo tick só dispara após `interval` a partir do término do anterior
    //    — sem acúmulo de ticks atrasados. Espelha o `setTimeout(tick, intervalMs)`
    //    pós-write do TS.
    //    O 1º `tick().await` dispara imediatamente (emite já).
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut out = tokio::io::stdout();

    loop {
        ticker.tick().await;
        let quotas = fetch(provider, ctx).await;
        let line = build_watch_line(&quotas);

        match out.write_all(line.as_bytes()).await {
            Ok(()) => {
                if let Err(e) = out.flush().await {
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        // Consumidor fechou o pipe; sair limpo (espelha exit(0) do EPIPE handler TS).
                        std::process::exit(0);
                    }
                    log::error!("stdout write error: {e}");
                    std::process::exit(1);
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                }
                log::error!("stdout write error: {e}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::AllQuotas;

    fn empty_quotas() -> AllQuotas {
        AllQuotas {
            providers: vec![],
            fetched_at: "2026-01-01T00:00:00.000Z".into(),
        }
    }

    #[test]
    fn build_watch_line_ends_with_single_newline() {
        let quotas = empty_quotas();
        let line = build_watch_line(&quotas);
        assert!(line.ends_with('\n'), "a linha deve terminar com '\\n'");
        assert_eq!(
            line.matches('\n').count(),
            1,
            "deve haver exatamente um '\\n'"
        );
    }

    #[test]
    fn build_watch_line_body_matches_to_json_string() {
        let quotas = empty_quotas();
        let line = build_watch_line(&quotas);
        // Remove o '\n' final e compara com to_json_string.
        let body = line.trim_end_matches('\n');
        let expected = crate::formatters::json::to_json_string(&quotas).unwrap();
        assert_eq!(body, expected, "corpo deve ser idêntico a to_json_string");
    }

    #[test]
    fn build_watch_line_is_valid_json() {
        let quotas = empty_quotas();
        let line = build_watch_line(&quotas);
        let body = line.trim_end_matches('\n');
        let parsed: serde_json::Value =
            serde_json::from_str(body).expect("corpo deve ser JSON válido");
        assert!(parsed.is_object(), "JSON deve ser um objeto");
    }
}
