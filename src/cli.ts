import pkg from '../package.json';
import { APP_NAME } from './app-identity';
import { logger } from './logger';
import { ANSI, BOX } from './theme';

export interface CliOptions {
  command:
    | 'waybar'
    | 'terminal'
    | 'menu'
    | 'status'
    | 'help'
    | 'version'
    | 'action-right'
    | 'setup'
    | 'assets-install'
    | 'export-waybar-modules'
    | 'export-waybar-css'
    | 'update'
    | 'uninstall'
    | 'remove'
    | 'doctor';
  refresh: boolean;
  provider?: string;
  verbose: boolean;
  format: 'waybar' | 'json';
  watch: boolean;
  intervalSeconds: number;
  waybarDir?: string;
  scriptsDir?: string;
  iconsDir?: string;
  appBin?: string;
  terminalScript?: string;
  dryRun?: boolean;
  yes?: boolean;
}

const vc = ANSI.magenta;
const v = () => `${vc}${BOX.v}${ANSI.reset}`;
const label = (text: string) =>
  `${vc}${BOX.lt}${BOX.h}${ANSI.reset} ${ANSI.magenta}${ANSI.bold}${BOX.diamond} ${text}${ANSI.reset}`;

// Alignment columns
const COL1 = 22; // command/option column

function cmdLine(name: string, desc: string): string {
  return `${v()}  ${ANSI.green}${BOX.dot}${ANSI.reset} ${ANSI.textBright}${name.padEnd(COL1)}${ANSI.reset}${ANSI.muted}${desc}${ANSI.reset}`;
}

function optLine(flags: string, desc: string): string {
  return `${v()}  ${ANSI.yellow}${BOX.dot}${ANSI.reset} ${ANSI.textBright}${flags.padEnd(COL1)}${ANSI.reset}${ANSI.muted}${desc}${ANSI.reset}`;
}

function infoLine(key: string, val: string): string {
  return `${v()}  ${ANSI.orange}${BOX.dot}${ANSI.reset} ${ANSI.orange}${key.padEnd(COL1)}${ANSI.reset}${ANSI.comment}${val}${ANSI.reset}`;
}

function wbLine(action: string, desc: string): string {
  return `${v()}  ${ANSI.textBright}${action.padEnd(COL1)}${ANSI.reset}${ANSI.comment}→${ANSI.reset} ${ANSI.muted}${desc}${ANSI.reset}`;
}

export function showHelp(): void {
  const version = pkg.version;
  const w = 58;

  console.log();
  console.log(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}${APP_NAME}${ANSI.reset} ${ANSI.comment}v${version}${ANSI.reset} ${vc}${BOX.h.repeat(Math.max(0, w - APP_NAME.length - 8))}${ANSI.reset}`,
  );
  console.log(v());

  // Commands
  console.log(label('Commands'));
  console.log(cmdLine('menu', 'Interactive TUI menu'));
  console.log(cmdLine('status', 'Show quotas in terminal'));
  console.log(cmdLine('setup', `Install + wire ${APP_NAME} in Waybar`));
  console.log(cmdLine('assets install', 'Install icons/helper only'));
  console.log(cmdLine('export waybar-modules', 'Print Waybar JSON module contract'));
  console.log(cmdLine('export waybar-css', 'Print Waybar CSS JSON contract'));
  console.log(cmdLine('update', 'Update the install (npm or managed checkout)'));
  console.log(cmdLine('uninstall', `Remove ${APP_NAME} + integration`));
  console.log(cmdLine('remove', 'Force remove without prompt'));
  console.log(cmdLine('doctor', `Detect & clean ${APP_NAME} leftovers in $HOME`));
  console.log(v());

  // Waybar
  console.log(label('Waybar'));
  console.log(wbLine('Left click', 'Interactive menu'));
  console.log(wbLine('Right click', 'Refresh / Login'));
  console.log(wbLine('Hover', 'Detailed tooltip'));
  console.log(v());

  console.log(label('Flags'));
  console.log(optLine('--provider, -p <id>', 'Single provider (Waybar module)'));
  console.log(optLine('--refresh, -r', 'Invalidate cache before output'));
  console.log(optLine('--verbose, -v', 'Debug logging to stderr'));
  console.log(optLine('--version, -V', 'Print version and exit'));
  console.log(optLine('--format <fmt>', 'Output format: waybar (default) | json'));
  console.log(optLine('--watch', 'Stream NDJSON (implies --format json)'));
  console.log(optLine('--interval <s>', 'Watch poll floor in seconds (default 60)'));
  console.log(optLine('--dry-run', 'Preview changes (doctor)'));
  console.log(optLine('--yes, -y', 'Assume yes (doctor/uninstall)'));
  console.log(optLine('--waybar-dir <path>', 'Assets install target'));
  console.log(optLine('--scripts-dir <path>', 'Terminal helper target'));
  console.log(optLine('--icons-dir <path>', 'CSS export icon directory'));
  console.log(optLine('--app-bin <path>', 'Modules export app binary'));
  console.log(optLine('--terminal-script <path>', 'Modules export launcher'));
  console.log(v());

  console.log(label('Info'));
  console.log(infoLine('Run with', `${APP_NAME}  or  bun run start`));
  console.log(v());

  console.log(`${vc}${BOX.bl}${BOX.h.repeat(w)}${ANSI.reset}`);
  console.log();
}

function requireNextArg(args: string[], i: number, flag: string): string {
  if (i + 1 >= args.length) {
    console.error(`Error: ${flag} requires a value`);
    process.exit(1);
  }
  return args[i + 1];
}

const KNOWN_COMMANDS = [
  'menu',
  'status',
  'setup',
  'assets',
  'export',
  'update',
  'uninstall',
  'remove',
  'action-right',
  'doctor',
  'help',
];

function levenshtein(a: string, b: string): number {
  const m = a.length;
  const n = b.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0));
  for (let i = 0; i <= m; i++) dp[i][0] = i;
  for (let j = 0; j <= n; j++) dp[0][j] = j;
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] = a[i - 1] === b[j - 1] ? dp[i - 1][j - 1] : 1 + Math.min(dp[i - 1][j], dp[i][j - 1], dp[i - 1][j - 1]);
    }
  }
  return dp[m][n];
}

function suggestCommand(input: string): string | null {
  let best: string | null = null;
  let bestDist = Infinity;
  for (const cmd of KNOWN_COMMANDS) {
    const d = levenshtein(input, cmd);
    if (d < bestDist) {
      bestDist = d;
      best = cmd;
    }
  }
  return bestDist <= 3 ? best : null;
}

export function parseArgs(args: string[]): CliOptions {
  const options: CliOptions = {
    command: 'waybar',
    refresh: false,
    verbose: false,
    format: 'waybar',
    watch: false,
    intervalSeconds: 60,
  };

  let formatGiven = false;
  let intervalGiven = false;

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    switch (arg) {
      case 'menu':
        options.command = 'menu';
        break;
      case 'status':
        options.command = 'status';
        break;
      case 'setup':
        options.command = 'setup';
        break;
      case 'assets':
        if (args[i + 1] === 'install') {
          options.command = 'assets-install';
          i += 1;
        } else {
          console.error("Unknown subcommand for 'assets'. Did you mean 'assets install'?");
          process.exit(1);
        }
        break;
      case 'export':
        if (args[i + 1] === 'waybar-modules') {
          options.command = 'export-waybar-modules';
          i += 1;
        } else if (args[i + 1] === 'waybar-css') {
          options.command = 'export-waybar-css';
          i += 1;
        } else {
          console.error("Unknown subcommand for 'export'. Use 'export waybar-modules' or 'export waybar-css'.");
          process.exit(1);
        }
        break;
      case 'update':
        options.command = 'update';
        break;
      case 'uninstall':
        options.command = 'uninstall';
        break;
      case 'remove':
        options.command = 'remove';
        break;
      case 'doctor':
        options.command = 'doctor';
        break;
      case 'action-right':
        options.command = 'action-right';
        options.provider = requireNextArg(args, i, 'action-right');
        i++;
        break;
      case '--dry-run':
        options.dryRun = true;
        break;
      case '--yes':
      case '-y':
        options.yes = true;
        break;
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
        options.provider = requireNextArg(args, i, '--provider');
        i++;
        break;
      case '--verbose':
      case '-v':
        options.verbose = true;
        break;
      case '--format': {
        const val = requireNextArg(args, i, '--format');
        if (val !== 'waybar' && val !== 'json') {
          console.error(`Error: --format must be 'waybar' or 'json' (got '${val}')`);
          process.exit(1);
        }
        options.format = val;
        formatGiven = true;
        i++;
        break;
      }
      case '--watch':
        options.watch = true;
        break;
      case '--interval': {
        const val = requireNextArg(args, i, '--interval');
        const n = Number.parseInt(val, 10);
        if (!Number.isInteger(n) || n <= 0) {
          console.error(`Error: --interval must be a positive integer (got '${val}')`);
          process.exit(1);
        }
        options.intervalSeconds = n;
        intervalGiven = true;
        i++;
        break;
      }
      case '--waybar-dir':
        options.waybarDir = requireNextArg(args, i, '--waybar-dir');
        i++;
        break;
      case '--scripts-dir':
        options.scriptsDir = requireNextArg(args, i, '--scripts-dir');
        i++;
        break;
      case '--icons-dir':
        options.iconsDir = requireNextArg(args, i, '--icons-dir');
        i++;
        break;
      case '--app-bin':
        options.appBin = requireNextArg(args, i, '--app-bin');
        i++;
        break;
      case '--terminal-script':
        options.terminalScript = requireNextArg(args, i, '--terminal-script');
        i++;
        break;
      case '--help':
      case '-h':
      case 'help':
        options.command = 'help';
        break;
      case '--version':
      case '-V':
        options.command = 'version';
        break;
      default:
        if (arg.startsWith('-')) {
          logger.warn(`Unknown option: ${arg}`);
        } else {
          const suggestion = suggestCommand(arg);
          if (suggestion) {
            console.error(`Unknown command: ${arg}. Did you mean '${suggestion}'?`);
          } else {
            console.error(`Unknown command: ${arg}. Run '${APP_NAME} help' for available commands.`);
          }
          process.exit(1);
        }
    }
  }

  if (options.watch) {
    if (formatGiven && options.format === 'waybar') {
      console.error('Error: --watch requires --format json');
      process.exit(1);
    }
    options.format = 'json';
  }
  if (intervalGiven && !options.watch) {
    console.error('[agent-bar] --interval has no effect without --watch');
  }

  return options;
}
