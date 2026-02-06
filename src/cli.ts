import { logger } from './logger';

export interface CliOptions {
  command: 'waybar' | 'terminal' | 'menu' | 'status' | 'help';
  refresh: boolean;
  provider?: string;
  verbose: boolean;
}

const HELP_TEXT = `
qbar - LLM quota monitor for Waybar

USAGE:
  qbar [command] [options]

COMMANDS:
  (default)         Output Waybar JSON (for waybar config)
  menu              Interactive TUI menu
  status            Show quotas in terminal (alias for -t)

OPTIONS:
  --terminal, -t    Output for terminal (ANSI colors)
  --refresh, -r     Force refresh cache before fetching
  --provider, -p    Only show specific provider (claude, codex, antigravity)
  --verbose, -v     Enable verbose logging
  --help, -h        Show this help message

EXAMPLES:
  qbar                     # Waybar JSON output (use in waybar config)
  qbar menu                # Interactive configuration
  qbar status              # Terminal output with colors
  qbar -t -p claude        # Only Claude, terminal output
  qbar --refresh           # Force refresh all caches

CONFIG:
  Settings stored in ~/.config/qbar/settings.json
`.trim();

export function parseArgs(args: string[]): CliOptions {
  const options: CliOptions = {
    command: 'waybar',
    refresh: false,
    verbose: false,
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    switch (arg) {
      // Commands
      case 'menu':
        options.command = 'menu';
        break;

      case 'status':
        options.command = 'status';
        break;

      // Options
      case '--terminal':
      case '-t':
        options.command = 'terminal';
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
      case 'help':
        options.command = 'help';
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
