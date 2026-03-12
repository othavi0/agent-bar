import { logger } from "./logger";
import { ANSI } from "./theme";

export interface CliOptions {
  command:
    | "waybar"
    | "terminal"
    | "menu"
    | "status"
    | "help"
    | "action-right"
    | "setup"
    | "assets-install"
    | "export-waybar-modules"
    | "export-waybar-css"
    | "update"
    | "uninstall";
  refresh: boolean;
  provider?: string;
  verbose: boolean;
  waybarDir?: string;
  scriptsDir?: string;
  iconsDir?: string;
  qbarBin?: string;
  terminalScript?: string;
}

const C = {
  reset: ANSI.reset,
  bold: ANSI.bold,
  green: ANSI.green,
  yellow: ANSI.yellow,
  blue: ANSI.blue,
  mauve: ANSI.magenta,
  teal: ANSI.cyan,
  text: ANSI.text,
  subtext: ANSI.muted,
  muted: ANSI.comment,
  peach: ANSI.orange,
  lavender: ANSI.textBright,
};

// Box drawing (bold)
const B = {
  tl: "┏",
  bl: "┗",
  lt: "┣",
  h: "━",
  v: "┃",
  diamond: "◆",
  dot: "●",
};

const vc = C.mauve;
const v = () => `${vc}${B.v}${C.reset}`;
const label = (text: string) =>
  `${vc}${B.lt}${B.h}${C.reset} ${C.mauve}${C.bold}${B.diamond} ${text}${C.reset}`;

// Alignment columns
const COL1 = 22; // command/option column
const COL2 = 35; // description starts here

function cmdLine(name: string, desc: string): string {
  return `${v()}  ${C.green}${B.dot}${C.reset} ${C.lavender}${name.padEnd(COL1)}${C.reset}${C.subtext}${desc}${C.reset}`;
}

function optLine(flags: string, desc: string): string {
  return `${v()}  ${C.yellow}${B.dot}${C.reset} ${C.lavender}${flags.padEnd(COL1)}${C.reset}${C.subtext}${desc}${C.reset}`;
}

function exLine(cmd: string, desc: string): string {
  return `${v()}  ${C.teal}${B.dot}${C.reset} ${C.teal}${cmd.padEnd(COL1)}${C.reset}${C.muted}${desc}${C.reset}`;
}

function infoLine(key: string, val: string): string {
  return `${v()}  ${C.peach}${B.dot}${C.reset} ${C.peach}${key.padEnd(COL1)}${C.reset}${C.muted}${val}${C.reset}`;
}

function wbLine(action: string, desc: string): string {
  return `${v()}  ${C.lavender}${action.padEnd(COL1)}${C.reset}${C.muted}→${C.reset} ${C.subtext}${desc}${C.reset}`;
}

export function showHelp(): void {
  const version = "3.0.0";
  const w = 58;

  console.log();
  console.log(
    `${vc}${B.tl}${B.h}${C.reset} ${vc}${C.bold}qbar${C.reset} ${C.muted}v${version}${C.reset} ${vc}${B.h.repeat(w - 12)}${C.reset}`,
  );
  console.log(v());

  // Commands
  console.log(label("Commands"));
  console.log(cmdLine("menu", "Interactive TUI menu"));
  console.log(cmdLine("status", "Show quotas in terminal"));
  console.log(cmdLine("setup", "Safe setup wrapper (assets + guidance)"));
  console.log(cmdLine("assets install", "Install icons/helper only"));
  console.log(cmdLine("export waybar-modules", "Print Waybar JSON module contract"));
  console.log(cmdLine("export waybar-css", "Print Waybar CSS JSON contract"));
  console.log(cmdLine("update", "Update qbar to latest version"));
  console.log(cmdLine("uninstall", "Remove qbar from system"));
  console.log(v());

  // Waybar
  console.log(label("Waybar"));
  console.log(wbLine("Left click", "Interactive menu"));
  console.log(wbLine("Right click", "Refresh / Login"));
  console.log(wbLine("Hover", "Detailed tooltip"));
  console.log(v());

  console.log(label("Flags"));
  console.log(optLine("--waybar-dir <path>", "Assets install target"));
  console.log(optLine("--scripts-dir <path>", "Terminal helper target"));
  console.log(optLine("--icons-dir <path>", "CSS export icon directory"));
  console.log(optLine("--qbar-bin <path>", "Modules export qbar binary"));
  console.log(optLine("--terminal-script <path>", "Modules export launcher"));
  console.log(v());

  console.log(`${vc}${B.bl}${B.h.repeat(w)}${C.reset}`);
  console.log();
}

export function parseArgs(args: string[]): CliOptions {
  const options: CliOptions = {
    command: "waybar",
    refresh: false,
    verbose: false,
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    switch (arg) {
      case "menu":
        options.command = "menu";
        break;
      case "status":
        options.command = "status";
        break;
      case "setup":
        options.command = "setup";
        break;
      case "assets":
        if (args[i + 1] === "install") {
          options.command = "assets-install";
          i += 1;
        }
        break;
      case "export":
        if (args[i + 1] === "waybar-modules") {
          options.command = "export-waybar-modules";
          i += 1;
        } else if (args[i + 1] === "waybar-css") {
          options.command = "export-waybar-css";
          i += 1;
        }
        break;
      case "update":
        options.command = "update";
        break;
      case "uninstall":
        options.command = "uninstall";
        break;
      case "action-right":
        options.command = "action-right";
        options.provider = args[++i];
        break;
      case "--terminal":
      case "-t":
        options.command = "terminal";
        break;
      case "--refresh":
      case "-r":
        options.refresh = true;
        break;
      case "--provider":
      case "-p":
        options.provider = args[++i];
        break;
      case "--verbose":
      case "-v":
        options.verbose = true;
        break;
      case "--waybar-dir":
        options.waybarDir = args[++i];
        break;
      case "--scripts-dir":
        options.scriptsDir = args[++i];
        break;
      case "--icons-dir":
        options.iconsDir = args[++i];
        break;
      case "--qbar-bin":
        options.qbarBin = args[++i];
        break;
      case "--terminal-script":
        options.terminalScript = args[++i];
        break;
      case "--help":
      case "-h":
      case "help":
        options.command = "help";
        break;
      default:
        if (arg.startsWith("-")) {
          logger.warn(`Unknown option: ${arg}`);
        }
    }
  }

  return options;
}
