//! Parser do session log do Claude (`~/.claude/projects/**/*.jsonl`). Ver spec §4b.

use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::UsageRecord;

/// Extrai `UsageRecord`s de linhas JSONL do Claude. Linhas sem `type:"assistant"`,
/// sem `message.usage`, sem timestamp parseável, ou não-JSON → puladas.
pub fn parse_claude_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<UsageRecord> {
    let mut out = Vec::new();
    for line in lines {
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
        let u = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
        let project = v
            .get("cwd")
            .and_then(Value::as_str)
            .and_then(|c| std::path::Path::new(c).file_name())
            .and_then(|b| b.to_str())
            .map(str::to_string);
        out.push(UsageRecord {
            provider: "claude".to_string(),
            model: msg
                .get("model")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
            input: u("input_tokens"),
            output: u("output_tokens"),
            cache_read: u("cache_read_input_tokens"),
            cache_write: u("cache_creation_input_tokens"),
            ts,
            session_id: None,
            project,
        });
    }
    out
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
    fn extracts_project_from_cwd() {
        let line = r#"{"type":"assistant","timestamp":"2026-07-10T10:00:00Z","cwd":"/home/o/Projects/agent-bar","message":{"model":"claude-fable-5","usage":{"input_tokens":1,"output_tokens":1}}}"#;
        let recs = parse_claude_lines([line].into_iter());
        assert_eq!(recs[0].project.as_deref(), Some("agent-bar"));
    }

    #[test]
    fn missing_cwd_yields_none_project() {
        let recs = parse_claude_lines([LINE].into_iter());
        assert_eq!(recs[0].project, None);
    }
}
