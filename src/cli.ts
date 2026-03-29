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
    | 'action-right'
    | 'setup'
    | 'assets-install'
    | 'apply-local'
    | 'export-waybar-modules'
    | 'export-waybar-css'
    | 'update'
    | 'uninstall'
    | 'remove';
  refresh: boolean;
  provider?: string;
  verbose: boolean;
  waybarDir?: string;
  scriptsDir?: string;
  iconsDir?: string;
  appBin?: string;
  terminalScript?: string;
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
  console.log(cmdLine('apply-local', 'Re-apply local repo changes'));
  console.log(cmdLine('assets install', 'Install icons/helper only'));
  console.log(cmdLine('export waybar-modules', 'Print Waybar JSON module contract'));
  console.log(cmdLine('export waybar-css', 'Print Waybar CSS JSON contract'));
  console.log(cmdLine('update', `Update ${APP_NAME} to latest version`));
  console.log(cmdLine('uninstall', `Remove ${APP_NAME} + integration`));
  console.log(cmdLine('remove', 'Force remove without prompt'));
  console.log(v());

  // Waybar
  console.log(label('Waybar'));
  console.log(wbLine('Left click', 'Interactive menu'));
  console.log(wbLine('Right click', 'Refresh / Login'));
  console.log(wbLine('Hover', 'Detailed tooltip'));
  console.log(v());

  console.log(label('Flags'));
  console.log(optLine('--waybar-dir <path>', 'Assets install target'));
  console.log(optLine('--scripts-dir <path>', 'Terminal helper target'));
  console.log(optLine('--icons-dir <path>', 'CSS export icon directory'));
  console.log(optLine('--app-bin <path>', 'Modules export app binary'));
  console.log(optLine('--terminal-script <path>', 'Modules export launcher'));
  console.log(v());

  console.log(label('Info'));
  console.log(infoLine('Run with', `./scripts/${APP_NAME}  or  bun run start`));
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
  'apply',
  'apply-local',
  'export',
  'update',
  'uninstall',
  'remove',
  'action-right',
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
  };

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
        }
        break;
      case 'apply':
        if (args[i + 1] === 'local') {
          options.command = 'apply-local';
          i += 1;
        }
        break;
      case 'apply-local':
        options.command = 'apply-local';
        break;
      case 'export':
        if (args[i + 1] === 'waybar-modules') {
          options.command = 'export-waybar-modules';
          i += 1;
        } else if (args[i + 1] === 'waybar-css') {
          options.command = 'export-waybar-css';
          i += 1;
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
      case 'action-right':
        options.command = 'action-right';
        options.provider = requireNextArg(args, i, 'action-right');
        i++;
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
        }
    }
  }

  return options;
}
