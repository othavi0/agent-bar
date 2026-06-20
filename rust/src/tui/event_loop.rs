use futures::StreamExt as _;
use ratatui::crossterm::event::{Event, EventStream};
use ratatui::widgets::{Block, List, ListItem, Tabs};
use ratatui::{DefaultTerminal, Frame};
use tokio::time::{interval, Duration};

use crate::providers::{fetch_all, registry, Ctx};

use super::action::Action;
use super::state::{AppState, FetchStatus};
use super::update::update;

/// Render minimo para T2: abas no topo + lista de nomes dos providers na lateral.
fn render_min(state: &AppState, frame: &mut Frame) {
    use ratatui::layout::{Constraint, Direction, Layout};

    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // Tab bar
    let tab_titles: Vec<&str> = ["Dashboard", "Waybar", "History", "Login"].to_vec();
    let selected_tab = state.tab.index();
    let tabs = Tabs::new(tab_titles)
        .select(selected_tab)
        .block(Block::default());
    frame.render_widget(tabs, chunks[0]);

    // Provider list in the body
    let status_line = match &state.status {
        FetchStatus::Idle => "Idle".to_string(),
        FetchStatus::Loading => "Loading...".to_string(),
        FetchStatus::Loaded => {
            let ts = state
                .last_update
                .map(|t| format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second()))
                .unwrap_or_default();
            format!("Updated {ts}")
        }
        FetchStatus::Failed(msg) => format!("Error: {msg}"),
    };

    let mut items: Vec<ListItem> = state
        .providers
        .iter()
        .enumerate()
        .map(|(i, pv)| {
            let prefix = if i == state.selected { "> " } else { "  " };
            ListItem::new(format!("{prefix}{}", pv.quota.display_name))
        })
        .collect();

    items.push(ListItem::new(status_line));

    let list = List::new(items).block(Block::bordered().title("agent-bar"));
    frame.render_widget(list, chunks[1]);
}

/// Event loop principal. Corre até `state.should_quit`.
pub async fn run(ctx: &Ctx<'_>, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let mut state = AppState::new();

    let mut events = EventStream::new();
    let mut data_tick = interval(Duration::from_secs(60));
    let mut anim_tick = interval(Duration::from_millis(30));

    // Tick imediato: dispara o fetch inicial sem esperar 60s.
    // Marcamos como Loading e fazemos o fetch logo na 1a iteracao.
    let mut initial_fetch = true;

    loop {
        terminal.draw(|f| render_min(&state, f))?;

        if state.should_quit {
            break;
        }

        if initial_fetch {
            initial_fetch = false;
            state.status = FetchStatus::Loading;
            let providers = registry();
            // Fetch inline: bloqueia o loop mas e aceitavel no v1 (timeout 10s por provider).
            match tokio::time::timeout(Duration::from_secs(30), fetch_all(&providers, ctx)).await {
                Ok(quotas) => {
                    for a in update(&mut state, Action::DataFetched(quotas)) {
                        update(&mut state, a);
                    }
                }
                Err(_) => {
                    for a in update(&mut state, Action::FetchFailed("fetch timeout".to_string())) {
                        update(&mut state, a);
                    }
                }
            }
            continue;
        }

        tokio::select! {
            maybe_ev = events.next() => {
                if let Some(Ok(Event::Key(key))) = maybe_ev {
                    let follow_ups = update(&mut state, Action::Key(key));
                    for a in follow_ups {
                        update(&mut state, a);
                    }
                }
            }

            _ = data_tick.tick() => {
                state.status = FetchStatus::Loading;
                let providers = registry();
                match tokio::time::timeout(
                    Duration::from_secs(30),
                    fetch_all(&providers, ctx),
                )
                .await
                {
                    Ok(quotas) => {
                        for a in update(&mut state, Action::DataFetched(quotas)) {
                            update(&mut state, a);
                        }
                    }
                    Err(_) => {
                        for a in update(
                            &mut state,
                            Action::FetchFailed("fetch timeout".to_string()),
                        ) {
                            update(&mut state, a);
                        }
                    }
                }
            }

            _ = anim_tick.tick() => {
                update(&mut state, Action::AnimTick);
            }
        }
    }

    Ok(())
}
