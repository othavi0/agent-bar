import { logger } from './logger';

export interface CliOptions {
  terminal: boolean;
  refresh: boolean;
  provider?: string;
  verbose: boolean;
  help: boolean;
}

const HELP_TEXT = `
llm-usage - Show LLM provider quotas

USAGE:
  llm-usage [options]

OPTIONS:
  --terminal, -t    Output for terminal (ANSI colors) instead of Waybar JSON
  --refresh, -r     Force refresh cache before fetching
  --provider, -p    Only show specific provider (claude, codex, antigravity)
  --verbose, -v     Enable verbose logging
  --help, -h        Show this help message

EXAMPLES:
  llm-usage                    # Waybar JSON output
  llm-usage --terminal         # Terminal output with colors
  llm-usage -t -p claude       # Only Claude, terminal output
  llm-usage --refresh          # Force refresh all caches
`.trim();

export function parseArgs(args: string[]): CliOptions {
  const options: CliOptions = {
    terminal: false,
    refresh: false,
    verbose: false,
    help: false,
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    switch (arg) {
      case '--terminal':
      case '-t':
        options.terminal = true;
        break;

      case '--refresh':
      case '-r':
        options.refresh = true;
        break;

      case '--provider':
      case '-p':
        options.provider = args[++i];
        break;

      case '--verbose':
      case '-v':
        options.verbose = true;
        break;

      case '--help':
      case '-h':
        options.help = true;
        break;

      default:
        if (arg.startsWith('-')) {
          logger.warn(`Unknown option: ${arg}`);
        }
    }
  }

  return options;
}

export function showHelp(): void {
  console.log(HELP_TEXT);
}
