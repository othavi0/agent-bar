//! Humanização de ids de modelo pra display ("claude-opus-4-8" → "Opus 4.8")
//! e slot de série de gráfico por família (cor segue a entidade, nunca o rank).

/// Nome tratado pra display. Fallback: id original (o truncamento com `…`
/// continua sendo responsabilidade do render).
pub fn display_model_name(model: &str) -> String {
    let m = model.to_ascii_lowercase();

    // Claude: "claude-<família>-<maj>[-<min>][-<data YYYYMMDD>]"
    if let Some(rest) = m.strip_prefix("claude-") {
        let mut parts: Vec<&str> = rest.split('-').collect();
        // Descarta sufixo de data (8+ dígitos) se presente.
        if parts
            .last()
            .is_some_and(|p| p.len() >= 8 && p.chars().all(|c| c.is_ascii_digit()))
        {
            parts.pop();
        }
        if let Some((family, version)) = parts.split_first() {
            let fam = capitalize(family);
            if version.is_empty() {
                return fam;
            }
            return format!("{} {}", fam, version.join("."));
        }
    }

    // OpenAI/Codex: "gpt-5.5-codex" → "GPT-5.5 Codex"; "gpt-5.5" → "GPT-5.5".
    if let Some(rest) = m.strip_prefix("gpt-") {
        let (ver, suffix) = match rest.split_once('-') {
            Some((v, s)) => (v, Some(s)),
            None => (rest, None),
        };
        return match suffix {
            Some(s) => format!("GPT-{} {}", ver, capitalize(s)),
            None => format!("GPT-{ver}"),
        };
    }

    model.to_string()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Slot de série (0..=5) por família — mapeia pra ColorToken::Series1..6:
/// 0=fable/mythos, 1=opus, 2=sonnet, 3=haiku, 4=codex/gpt/o-series, 5=outros.
pub fn series_slot_for_model(model: &str) -> u8 {
    let m = model.to_ascii_lowercase();
    if m.contains("fable") || m.contains("mythos") {
        0
    } else if m.contains("opus") {
        1
    } else if m.contains("sonnet") {
        2
    } else if m.contains("haiku") {
        3
    } else if m.contains("codex") || m.starts_with("gpt-") || m.starts_with("o4") {
        4
    } else {
        5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treats_current_claude_models() {
        assert_eq!(display_model_name("claude-fable-5"), "Fable 5");
        assert_eq!(display_model_name("claude-opus-4-8"), "Opus 4.8");
        assert_eq!(display_model_name("claude-sonnet-5"), "Sonnet 5");
        assert_eq!(display_model_name("claude-haiku-4-5"), "Haiku 4.5");
        assert_eq!(display_model_name("claude-opus-4-5-20260101"), "Opus 4.5");
    }

    #[test]
    fn treats_codex_models() {
        assert_eq!(display_model_name("gpt-5.5-codex"), "GPT-5.5 Codex");
        assert_eq!(display_model_name("gpt-5.5"), "GPT-5.5");
        assert_eq!(display_model_name("gpt-5.3-codex"), "GPT-5.3 Codex");
    }

    #[test]
    fn unknown_model_falls_back_to_id() {
        assert_eq!(display_model_name("mystery-model-9"), "mystery-model-9");
        assert_eq!(display_model_name(""), "");
    }

    #[test]
    fn slots_follow_family() {
        assert_eq!(series_slot_for_model("claude-fable-5"), 0);
        assert_eq!(series_slot_for_model("claude-mythos-5"), 0);
        assert_eq!(series_slot_for_model("claude-opus-4-8"), 1);
        assert_eq!(series_slot_for_model("claude-sonnet-5"), 2);
        assert_eq!(series_slot_for_model("claude-haiku-4-5"), 3);
        assert_eq!(series_slot_for_model("gpt-5.5-codex"), 4);
        assert_eq!(series_slot_for_model("gpt-5.5"), 4);
        assert_eq!(series_slot_for_model("o4-mini"), 4);
        assert_eq!(series_slot_for_model("mystery"), 5);
    }
}
