//! Parser do session log do Codex (`~/.codex/sessions/**/*.jsonl`). Ver spec §4b.
//! Modelo vem de session_meta/turn_context; tokens de event_msg/token_count.

use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::UsageRecord;

pub fn parse_codex_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<UsageRecord> {
    let mut out = Vec::new();
    let mut current_model: Option<String> = None;

    for line in lines {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match v.get("type").and_then(Value::as_str) {
            Some("session_meta") | Some("turn_context") => {
                if let Some(m) = v.get("model").and_then(Value::as_str) {
                    current_model = Some(m.to_string());
                }
            }
            Some("event_msg") => {
                let payload = match v.get("payload") {
                    Some(p) => p,
                    None => continue,
                };
                if payload.get("type").and_then(Value::as_str) != Some("token_count") {
                    continue;
                }
                let last = match payload.get("info").and_then(|i| i.get("last_token_usage")) {
                    Some(l) => l,
                    None => continue,
                };
                let ts = match v.get("timestamp").and_then(Value::as_str) {
                    Some(s) => match OffsetDateTime::parse(s, &Rfc3339) {
                        Ok(t) => t,
                        Err(_) => continue,
                    },
                    None => continue,
                };
                let u = |k: &str| last.get(k).and_then(Value::as_u64).unwrap_or(0);
                // input_tokens reportado como vem (cached é subconjunto contado à parte
                // no preço de cache_read — premissa conservadora, ver spec §4b).
                out.push(UsageRecord {
                    provider: "codex".to_string(),
                    model: current_model.clone(),
                    input: u("input_tokens"),
                    output: u("output_tokens") + u("reasoning_output_tokens"),
                    cache_read: u("cached_input_tokens"),
                    cache_write: 0,
                    ts,
                    session_id: None,
                    project: None,
                });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str =
        r#"{"type":"session_meta","timestamp":"2026-06-16T14:35:51Z","model":"gpt-5.5"}"#;
    const TOKENS: &str = r#"{"type":"event_msg","timestamp":"2026-06-16T14:36:00Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":26016,"cached_input_tokens":2432,"output_tokens":582,"reasoning_output_tokens":175,"total_tokens":26598}}}}"#;

    #[test]
    fn associates_model_from_meta_with_token_events() {
        let recs = parse_codex_lines([META, TOKENS].into_iter());
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.provider, "codex");
        assert_eq!(r.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(r.input, 26016);
        assert_eq!(r.output, 582 + 175); // output + reasoning
        assert_eq!(r.cache_read, 2432);
        assert_eq!(r.cache_write, 0);
    }

    #[test]
    fn token_event_before_any_model_has_none_model() {
        let recs = parse_codex_lines([TOKENS].into_iter());
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].model, None); // sem session_meta antes
    }

    #[test]
    fn turn_context_updates_current_model() {
        let turn =
            r#"{"type":"turn_context","timestamp":"2026-06-16T14:40:00Z","model":"gpt-5.5-codex"}"#;
        let recs = parse_codex_lines([META, TOKENS, turn, TOKENS].into_iter());
        assert_eq!(recs[0].model.as_deref(), Some("gpt-5.5"));
        assert_eq!(recs[1].model.as_deref(), Some("gpt-5.5-codex"));
    }

    #[test]
    fn skips_non_token_events_and_malformed() {
        let recs = parse_codex_lines(
            [
                "garbage",
                r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#,
            ]
            .into_iter(),
        );
        assert_eq!(recs.len(), 0);
    }
}
