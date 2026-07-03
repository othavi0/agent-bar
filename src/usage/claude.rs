//! Parser do session log do Claude (`~/.claude/projects/**/*.jsonl`). Ver spec §4b.

use std::collections::HashMap;

use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::UsageRecord;

/// Extrai `UsageRecord`s de linhas JSONL do Claude. Linhas sem `type:"assistant"`,
/// sem `message.usage`, sem timestamp parseável, ou não-JSON → puladas.
pub fn parse_claude_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<UsageRecord> {
    // Dedup de streaming: o Claude Code grava várias entradas por request
    // (mesmo requestId, output_tokens crescendo). A ÚLTIMA entrada vista
    // (ordem do arquivo) é o estado final da request — as anteriores são
    // parciais e somá-las multiplicaria tokens/custo (claude-devtools#74).
    // Chave: requestId; fallback message.id; sem ambos → índice da linha
    // (nunca colide: linha vale sozinha).
    let mut by_request: HashMap<String, UsageRecord> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for (i, line) in lines.enumerate() {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let msg = match v.get("message") {
            Some(m) => m,
            None => continue,
        };
        let usage = match msg.get("usage") {
            Some(u) => u,
            None => continue,
        };
        let ts = match v.get("timestamp").and_then(Value::as_str) {
            Some(s) => match OffsetDateTime::parse(s, &Rfc3339) {
                Ok(t) => t,
                Err(_) => continue,
            },
            None => continue,
        };
        let key = v
            .get("requestId")
            .and_then(Value::as_str)
            .or_else(|| msg.get("id").and_then(Value::as_str))
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("__line_{i}"));

        let u = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
        let cache_creation = usage.get("cache_creation");
        let tier = |k: &str| {
            cache_creation
                .and_then(|c| c.get(k))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        let rec = UsageRecord {
            provider: "claude".to_string(),
            model: msg
                .get("model")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
            input: u("input_tokens"),
            output: u("output_tokens"),
            cache_read: u("cache_read_input_tokens"),
            cache_write: u("cache_creation_input_tokens"),
            cache_write_1h: tier("ephemeral_1h_input_tokens"),
            fast: usage.get("speed").and_then(Value::as_str) == Some("fast"),
            geo_us: usage.get("inference_geo").and_then(Value::as_str) == Some("us"),
            ts,
        };
        if by_request.insert(key.clone(), rec).is_none() {
            order.push(key);
        }
    }
    order.into_iter().filter_map(|k| by_request.remove(&k)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"type":"assistant","timestamp":"2026-06-19T11:22:19.163Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":8285,"output_tokens":2481,"cache_creation_input_tokens":15031,"cache_read_input_tokens":16291}}}"#;

    #[test]
    fn parses_assistant_usage_line() {
        let recs = parse_claude_lines([LINE].into_iter());
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.provider, "claude");
        assert_eq!(r.model.as_deref(), Some("claude-opus-4-8"));
        assert_eq!(r.input, 8285);
        assert_eq!(r.output, 2481);
        assert_eq!(r.cache_write, 15031);
        assert_eq!(r.cache_read, 16291);
        assert_eq!(r.ts.year(), 2026);
    }

    #[test]
    fn skips_non_assistant_and_malformed() {
        let lines = [
            r#"{"type":"user","timestamp":"2026-06-19T11:00:00Z","message":{}}"#,
            "not json at all",
            r#"{"type":"assistant","message":{"model":"x"}}"#, // sem usage nem ts → pula
            LINE,
        ];
        let recs = parse_claude_lines(lines.into_iter());
        assert_eq!(recs.len(), 1); // só a LINE válida
    }

    #[test]
    fn missing_cache_fields_default_zero() {
        let line = r#"{"type":"assistant","timestamp":"2026-06-19T11:22:19Z","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let recs = parse_claude_lines([line].into_iter());
        assert_eq!(recs[0].cache_read, 0);
        assert_eq!(recs[0].cache_write, 0);
    }

    #[test]
    fn streaming_entries_same_request_dedupe_to_last() {
        // 3 entradas do MESMO requestId com output_tokens crescendo (streaming
        // real do Claude Code) — só a última pode contar, senão a request é
        // somada 3x (bug documentado em claude-devtools#74).
        let lines = [
            r#"{"type":"assistant","requestId":"req_1","timestamp":"2026-07-03T10:00:00.000Z","message":{"id":"msg_1","model":"claude-fable-5","usage":{"input_tokens":100,"output_tokens":5}}}"#,
            r#"{"type":"assistant","requestId":"req_1","timestamp":"2026-07-03T10:00:01.000Z","message":{"id":"msg_1","model":"claude-fable-5","usage":{"input_tokens":100,"output_tokens":50}}}"#,
            r#"{"type":"assistant","requestId":"req_1","timestamp":"2026-07-03T10:00:02.000Z","message":{"id":"msg_1","model":"claude-fable-5","usage":{"input_tokens":100,"output_tokens":90}}}"#,
            r#"{"type":"assistant","requestId":"req_2","timestamp":"2026-07-03T10:01:00.000Z","message":{"id":"msg_2","model":"claude-fable-5","usage":{"input_tokens":10,"output_tokens":1}}}"#,
        ];
        let recs = parse_claude_lines(lines.into_iter());
        assert_eq!(recs.len(), 2, "1 record por request, não por linha");
        let req1 = recs
            .iter()
            .find(|r| r.output == 90)
            .expect("última entrada de req_1");
        assert_eq!(req1.input, 100);
        assert_eq!(recs.iter().map(|r| r.output).sum::<u64>(), 91);
    }

    #[test]
    fn extracts_cache_tiers_speed_and_geo() {
        let line = r#"{"type":"assistant","requestId":"r9","timestamp":"2026-07-03T10:00:00Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":300,"cache_read_input_tokens":0,"cache_creation":{"ephemeral_5m_input_tokens":100,"ephemeral_1h_input_tokens":200},"speed":"fast","inference_geo":"us"}}}"#;
        let recs = parse_claude_lines([line].into_iter());
        let r = &recs[0];
        assert_eq!(r.cache_write, 300, "total continua o campo agregado");
        assert_eq!(r.cache_write_1h, 200);
        assert!(r.fast);
        assert!(r.geo_us);
    }

    #[test]
    fn missing_breakdown_defaults_to_zero_1h_and_standard() {
        let line = r#"{"type":"assistant","requestId":"r8","timestamp":"2026-07-03T10:00:00Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":300}}}"#;
        let r = &parse_claude_lines([line].into_iter())[0];
        // Fallback documentado da spec: sem breakdown, tratar tudo como 5m.
        assert_eq!(r.cache_write_1h, 0);
        assert!(!r.fast);
        assert!(!r.geo_us);
    }

    #[test]
    fn lines_without_request_id_fall_back_to_message_id_then_standalone() {
        // Sem requestId: dedup por message.id. Sem ambos: linha vale sozinha.
        let lines = [
            r#"{"type":"assistant","timestamp":"2026-07-03T10:00:00Z","message":{"id":"msg_a","model":"claude-fable-5","usage":{"input_tokens":1,"output_tokens":1}}}"#,
            r#"{"type":"assistant","timestamp":"2026-07-03T10:00:01Z","message":{"id":"msg_a","model":"claude-fable-5","usage":{"input_tokens":1,"output_tokens":7}}}"#,
            r#"{"type":"assistant","timestamp":"2026-07-03T10:00:02Z","message":{"model":"claude-fable-5","usage":{"input_tokens":3,"output_tokens":3}}}"#,
        ];
        let recs = parse_claude_lines(lines.into_iter());
        assert_eq!(recs.len(), 2);
        assert!(recs.iter().any(|r| r.output == 7), "msg_a dedupado pra última");
        assert!(recs.iter().any(|r| r.output == 3), "linha sem id vale sozinha");
    }
}
