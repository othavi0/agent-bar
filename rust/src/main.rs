#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", agent_bar::app_identity::VERSION);
        return;
    }
    eprintln!("agent-bar: CLI ainda não implementado (reescrita em andamento)");
    std::process::exit(1);
}
