//! Helper mínimo de prompt/status no terminal. Substitui @clack/prompts no contexto Rust.
//! Saída em STDERR; gate NO_COLOR para escape ANSI.

use std::io::{BufRead, IsTerminal, Write};

use crate::theme::{ColorToken, ANSI_RESET};

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

fn color(token: ColorToken, text: &str) -> String {
    if no_color() {
        text.to_string()
    } else {
        format!("{}{}{}", token.ansi(), text, ANSI_RESET)
    }
}

/// Imprime uma linha de status com label colorido em STDERR.
pub fn status(label: &str, msg: &str) {
    let prefix = color(ColorToken::Green, label);
    eprintln!("{prefix}: {msg}");
}

/// Imprime uma nota em STDERR.
pub fn note(text: &str) {
    let marker = color(ColorToken::Cyan, "┌");
    eprintln!("{marker} {text}");
}

/// `y`/`yes`→true, `n`/`no`→false, vazio→default_yes, não-reconhecido→default_yes.
/// Função pura, testável sem I/O.
pub fn parse_answer(input: &str, default_yes: bool) -> bool {
    let t = input.trim().to_lowercase();
    match t.as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default_yes,
        _ => default_yes,
    }
}

/// Lê uma linha de stdin e retorna true/false.
/// Se stdin não for TTY (pipe/teste) → retorna `default_yes` sem bloquear.
pub fn confirm(message: &str, default_yes: bool) -> bool {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return default_yes;
    }

    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    eprint!("{} {} ", message, hint);
    // flush stderr
    let _ = std::io::stderr().flush();

    let mut line = String::new();
    match stdin.lock().read_line(&mut line) {
        Ok(0) => default_yes, // EOF
        Ok(_) => parse_answer(&line, default_yes),
        Err(_) => default_yes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_answer_variants() {
        assert!(parse_answer("y", false));
        assert!(parse_answer("Yes", false));
        assert!(!parse_answer("n", true));
        assert!(!parse_answer("NO", true));
        assert!(parse_answer("", true)); // vazio → default
        assert!(!parse_answer("", false));
        assert!(!parse_answer("garbage", false)); // não-reconhecido → default
        assert!(parse_answer("  yes  ", false)); // trim
    }
}
