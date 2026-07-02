//! Fetch de quotas em thread dedicada — o event loop NUNCA espera rede.

use tokio::sync::mpsc::UnboundedSender;

use crate::providers::{fetch_one, iso_from_ms, registry, OwnedCtx, Provider};

use super::action::Action;

/// Dispara o fetch (todos os providers, ou só `only`) numa thread própria com
/// runtime tokio current_thread. Resultados chegam via `tx` como Actions.
/// `silent` viaja até o `FetchCompleted` final (T16: gate do sweep — só
/// ondas pedidas pelo usuário disparam o efeito, não o poll de 60s).
pub fn spawn_fetch(
    tx: &UnboundedSender<Action>,
    octx: OwnedCtx,
    only: Option<String>,
    silent: bool,
) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(Action::FetchFailed(format!("fetch runtime: {e}")));
                return;
            }
        };
        rt.block_on(async move {
            let providers: Vec<Box<dyn Provider>> = registry()
                .into_iter()
                .filter(|p| match &only {
                    Some(id) => p.id() == id,
                    None => true,
                })
                .collect();
            let ids: Vec<String> = providers.iter().map(|p| p.id().to_string()).collect();
            let _ = tx.send(Action::FetchStarted(ids));
            let now = OwnedCtx::now_ms();
            let ctx = octx.as_ctx(now);
            // Sequencial dentro da thread é aceitável (cada provider já tem
            // timeout de 10s + retry 1); mas mantemos o join concorrente:
            let futs = providers.iter().map(|p| fetch_one(p.as_ref(), &ctx));
            let mut stream = futures::stream::FuturesUnordered::from_iter(futs);
            use futures::StreamExt as _;
            while let Some(q) = stream.next().await {
                let _ = tx.send(Action::ProviderFetched(Box::new(q)));
            }
            let _ = tx.send(Action::FetchCompleted {
                fetched_at: iso_from_ms(now),
                silent,
            });
        });
    });
}
