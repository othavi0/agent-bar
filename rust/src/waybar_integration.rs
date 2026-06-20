//! Patcher cirúrgico do Waybar config/style (JSONC). NÃO usa crate de JSONC —
//! comentários e ordem do arquivo do usuário precisam sobreviver. Premissa: a
//! ESTRUTURA do JSONC é ASCII (chaves/colchetes/aspas); valores não-ASCII ficam
//! dentro de strings, puladas por `skip_string`. Port de `src/waybar-integration.ts`.
// T3b (apply/remove orchestration) usará as primitivas abaixo; até lá, suprimir dead_code.
#![allow(dead_code)]

use std::sync::OnceLock;

use regex::Regex;

/// Avança além de um literal de string JSON; `i` aponta à aspa de abertura.
/// Retorna o índice logo após a aspa de fechamento.
pub(crate) fn skip_string(content: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < content.len() {
        match content[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// Acha o `]` que fecha o `[` em `open_idx`, honrando colchetes aninhados,
/// strings e comentários JSONC. `None` se desbalanceado.
pub(crate) fn find_matching_bracket(content: &[u8], open_idx: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = open_idx;
    while i < content.len() {
        let c = content[i];
        if c == b'"' {
            i = skip_string(content, i);
            continue;
        }
        if c == b'/' && content.get(i + 1) == Some(&b'/') {
            // TS: i = nl === -1 ? content.length : nl (sem pular o \n — loop externo avança)
            match content[i..].iter().position(|&b| b == b'\n') {
                Some(rel) => {
                    i += rel;
                }
                None => {
                    i = content.len();
                    continue;
                }
            }
            continue;
        }
        if c == b'/' && content.get(i + 1) == Some(&b'*') {
            let rest = &content[i + 2..];
            match rest.windows(2).position(|w| w == b"*/") {
                Some(rel) => {
                    i = i + 2 + rel + 2;
                }
                None => {
                    i = content.len();
                    continue;
                }
            }
            continue;
        }
        if c == b'[' {
            depth += 1;
        } else if c == b']' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Extrai os valores de string de um corpo `"a", "b", ...` (JSON-unescape via serde).
pub(crate) fn parse_quoted_strings(block: &str) -> Vec<String> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    let re = match RE.get_or_init(|| Regex::new(r#""((?:\\.|[^"\\])*)""#).ok()) {
        Some(r) => r,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for cap in re.captures_iter(block) {
        let inner = &cap[1];
        if let Ok(s) = serde_json::from_str::<String>(&format!("\"{inner}\"")) {
            out.push(s);
        }
    }
    out
}

pub(crate) fn format_string_array(values: &[String], indent: &str) -> String {
    if values.is_empty() {
        return "[]".to_string();
    }
    let item_indent = format!("{indent}  ");
    let lines = values
        .iter()
        .map(|v| {
            format!(
                "{item_indent}{}",
                serde_json::to_string(v).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!("[\n{lines}\n{indent}]")
}

pub(crate) struct RewriteArrayResult {
    pub(crate) content: String,
    pub(crate) found: bool,
    pub(crate) changed: bool,
}

pub(crate) fn rewrite_string_array_property(
    content: &str,
    property: &str,
    transform: impl Fn(Vec<String>) -> Vec<String>,
) -> RewriteArrayResult {
    let pattern = format!(r#""{}"\s*:\s*\["#, regex::escape(property));
    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => {
            return RewriteArrayResult {
                content: content.to_string(),
                found: false,
                changed: false,
            }
        }
    };
    let bytes = content.as_bytes();
    for m in re.find_iter(content) {
        let match_start = m.start();
        let line_start = content[..match_start]
            .rfind('\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        let line_prefix = &content[line_start..match_start];
        if line_prefix.contains("//") {
            continue;
        }
        let open_idx = m.end() - 1; // índice do '['
        let close_idx = match find_matching_bracket(bytes, open_idx) {
            Some(c) => c,
            None => continue,
        };
        let body = &content[open_idx + 1..close_idx];
        let current = parse_quoted_strings(body);
        let next = transform(current.clone());

        let indent: String = line_prefix
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();

        if current == next {
            return RewriteArrayResult {
                content: content.to_string(),
                found: true,
                changed: false,
            };
        }
        let prefix = &content[match_start..open_idx]; // `"prop"\s*:\s*`
        let rewritten = format!(
            "{}{}{}{}",
            &content[..match_start],
            prefix,
            format_string_array(&next, &indent),
            &content[close_idx + 1..]
        );
        return RewriteArrayResult {
            content: rewritten,
            found: true,
            changed: true,
        };
    }
    RewriteArrayResult {
        content: content.to_string(),
        found: false,
        changed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_bracket_balances_nested_and_strings() {
        // [ "a", ["b","c"], "]" ]  → fecha no último ]
        let s = r#"[ "a", ["b","c"], "]" ]"#;
        let b = s.as_bytes();
        let open = 0;
        let close = find_matching_bracket(b, open).unwrap();
        assert_eq!(&s[open..=close], s); // o array inteiro
    }

    #[test]
    fn find_bracket_skips_line_and_block_comments() {
        let s = "[ \"a\", // ] not this\n \"b\" /* ] */ ]";
        let close = find_matching_bracket(s.as_bytes(), 0).unwrap();
        assert_eq!(s.as_bytes()[close], b']');
        // o ] final, não os comentados
        assert_eq!(close, s.len() - 1);
    }

    #[test]
    fn find_bracket_unbalanced_returns_none() {
        assert_eq!(find_matching_bracket(b"[ \"a\"", 0), None);
    }

    #[test]
    fn parse_quoted_unescapes_via_json() {
        let v = parse_quoted_strings(r#""a", "b\"c", "d\\e""#);
        assert_eq!(
            v,
            vec!["a".to_string(), "b\"c".to_string(), "d\\e".to_string()]
        );
    }

    #[test]
    fn format_array_empty_and_nonempty() {
        assert_eq!(format_string_array(&[], "  "), "[]");
        assert_eq!(
            format_string_array(&["x".into(), "y".into()], "  "),
            "[\n    \"x\",\n    \"y\"\n  ]"
        );
    }

    #[test]
    fn rewrite_appends_when_found() {
        let content = "{\n  \"include\": [\"a\"]\n}";
        let r = rewrite_string_array_property(content, "include", |mut v| {
            v.push("b".into());
            v
        });
        assert!(r.found && r.changed);
        assert!(r.content.contains("\"a\""));
        assert!(r.content.contains("\"b\""));
    }

    #[test]
    fn rewrite_skips_commented_line_and_reports_not_found() {
        let content = "{\n  // \"include\": [\"old\"],\n}";
        let r = rewrite_string_array_property(content, "include", |v| v);
        assert!(!r.found);
        assert_eq!(r.content, content);
    }

    #[test]
    fn rewrite_no_change_when_transform_identity() {
        let content = "{\n  \"include\": [\"a\"]\n}";
        let r = rewrite_string_array_property(content, "include", |v| v);
        assert!(r.found && !r.changed);
    }
}
