use clap::{Parser, CommandFactory};
use jkim::Cli;
use jki_core::{resolve_subcommand, get_subcommand_suggestions};
use console::style;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    // 如果有子指令（args.len() > 1），嘗試模糊解析
    if args.len() > 1 && !args[1].starts_with('-') {
        let input = &args[1];
        let cmd = Cli::command();
        let valid_subcommands: Vec<String> = cmd.get_subcommands()
            .map(|c| c.get_name().to_string())
            .filter(|name| name != "help")
            .collect();

        // 只有當輸入不完全匹配現有指令時，才執行模糊搜尋
        if !valid_subcommands.contains(&input.to_string()) {
            if let Some(resolved) = resolve_subcommand(input, &valid_subcommands) {
                eprintln!("{} Running '{}' ({} matched)",
                    style("[Fuzzy]").yellow().bold(),
                    style(&resolved).cyan().bold(),
                    style(input).dim()
                );
                args[1] = resolved;
            } else {
                // 檢查是否值得給出建議
                let suggestions = get_subcommand_suggestions(input, &valid_subcommands);
                if !suggestions.is_empty() {
                    eprintln!("{} '{}' is not a valid command.", style("Error:").red().bold(), input);
                    eprintln!("\n{} Did you mean?", style("?").yellow().bold());
                    for s in suggestions {
                        eprintln!("  - {}", style(s).cyan());
                    }
                    eprintln!("");
                    std::process::exit(1);
                }
            }
        }
    }

    // 使用修正後的 args 進行解析
    let cli = Cli::parse_from(args);
    if let Err(e) = jkim::run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
