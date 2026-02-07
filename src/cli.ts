import { logger } from './logger';

export interface CliOptions {
  command: 'waybar' | 'terminal' | 'menu' | 'status' | 'help';
  refresh: boolean;
  provider?: string;
  verbose: boolean;
}

// Catppuccin Mocha ANSI colors
const C = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  green: '\x1b[38;2;166;227;161m',
  yellow: '\x1b[38;2;249;226;175m',
  blue: '\x1b[38;2;137;180;250m',
  mauve: '\x1b[38;2;203;166;247m',
  teal: '\x1b[38;2;148;226;213m',
  text: '\x1b[38;2;205;214;244m',
  subtext: '\x1b[38;2;166;173;200m',
  muted: '\x1b[38;2;108;112;134m',
  peach: '\x1b[38;2;250;179;135m',
  lavender: '\x1b[38;2;180;190;254m',
};

const B = {
  v: '│',
  tl: '┌',
  bl: '└',
  h: '─',
};

function line(content: string): string {
  return `${C.muted}${B.v}${C.reset}  ${content}`;
}

function header(title: string): string {
  return `${C.muted}${B.tl}${B.h}${C.reset} ${C.mauve}${C.bold}${title}${C.reset}`;
}

function footer(): string {
  return `${C.muted}${B.bl}${B.h}${B.h}${B.h}${C.reset}`;
}

function cmd(name: string, desc: string): string {
  return line(`${C.green}${name.padEnd(18)}${C.reset} ${C.subtext}${desc}${C.reset}`);
}

function opt(flags: string, desc: string): string {
  return line(`${C.yellow}${flags.padEnd(18)}${C.reset} ${C.subtext}${desc}${C.reset}`);
}

function ex(command: string, desc: string): string {
  return line(`${C.teal}${command.padEnd(26)}${C.reset} ${C.muted}${desc}${C.reset}`);
}

export function showHelp(): void {
  const version = '3.0.0';
  
  console.log();
  console.log(`${C.mauve}${C.bold}  qbar${C.reset} ${C.muted}v${version}${C.reset}`);
  console.log(`${C.subtext}  Monitor de quota de LLMs para Waybar${C.reset}`);
  console.log();
  
  // Commands
  console.log(header('Comandos'));
  console.log(cmd('menu', 'Menu interativo (TUI)'));
  console.log(cmd('status', 'Mostra quotas no terminal'));
  console.log(cmd('setup', 'Configura Waybar automaticamente'));
  console.log(cmd('update', 'Atualiza qbar para última versão'));
  console.log(cmd('uninstall', 'Remove qbar do sistema'));
  console.log(footer());
  console.log();
  
  // Options
  console.log(header('Opções'));
  console.log(opt('-t, --terminal', 'Saída para terminal (cores ANSI)'));
  console.log(opt('-p, --provider', 'Apenas um provider (claude|codex|antigravity)'));
  console.log(opt('-r, --refresh', 'Força refresh do cache'));
  console.log(opt('-v, --verbose', 'Log detalhado'));
  console.log(opt('-h, --help', 'Mostra esta ajuda'));
  console.log(footer());
  console.log();
  
  // Examples
  console.log(header('Exemplos'));
  console.log(ex('qbar', 'JSON para Waybar'));
  console.log(ex('qbar menu', 'Abre menu interativo'));
  console.log(ex('qbar status', 'Quotas no terminal'));
  console.log(ex('qbar -t -p claude', 'Só Claude, no terminal'));
  console.log(ex('qbar --refresh', 'Refresh forçado'));
  console.log(footer());
  console.log();
  
  // Waybar
  console.log(header('Waybar'));
  console.log(line(`${C.lavender}Click esquerdo${C.reset}  ${C.muted}→${C.reset}  ${C.subtext}Menu interativo${C.reset}`));
  console.log(line(`${C.lavender}Click direito${C.reset}   ${C.muted}→${C.reset}  ${C.subtext}Refresh / Login${C.reset}`));
  console.log(line(`${C.lavender}Hover${C.reset}           ${C.muted}→${C.reset}  ${C.subtext}Tooltip com detalhes${C.reset}`));
  console.log(footer());
  console.log();
  
  // Paths
  console.log(header('Arquivos'));
  console.log(line(`${C.peach}Config${C.reset}   ${C.muted}~/.config/qbar/settings.json${C.reset}`));
  console.log(line(`${C.peach}Cache${C.reset}    ${C.muted}~/.config/waybar/qbar/cache/${C.reset}`));
  console.log(line(`${C.peach}Icons${C.reset}    ${C.muted}~/.config/waybar/qbar/icons/${C.reset}`));
  console.log(footer());
  console.log();
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
