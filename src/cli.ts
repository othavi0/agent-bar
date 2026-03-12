import { logger } from "./logger";
import { ANSI, BOX } from "./theme";
import pkg from "../package.json";

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

const vc = ANSI.magenta;
const v = () => `${vc}${BOX.v}${ANSI.reset}`;
const label = (text: string) =>
  `${vc}${BOX.lt}${BOX.h}${ANSI.reset} ${ANSI.magenta}${ANSI.bold}${BOX.diamond} ${text}${ANSI.reset}`;

// Alignment columns
const COL1 = 22; // command/option column
const COL2 = 35; // description starts here

function cmdLine(name: string, desc: string): string {
  return `${v()}  ${ANSI.green}${BOX.dot}${ANSI.reset} ${ANSI.textBright}${name.padEnd(COL1)}${ANSI.reset}${ANSI.muted}${desc}${ANSI.reset}`;
}

function optLine(flags: string, desc: string): string {
  return `${v()}  ${ANSI.yellow}${BOX.dot}${ANSI.reset} ${ANSI.textBright}${flags.padEnd(COL1)}${ANSI.reset}${ANSI.muted}${desc}${ANSI.reset}`;
}

function exLine(cmd: string, desc: string): string {
  return `${v()}  ${ANSI.cyan}${BOX.dot}${ANSI.reset} ${ANSI.cyan}${cmd.padEnd(COL1)}${ANSI.reset}${ANSI.comment}${desc}${ANSI.reset}`;
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
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}qbar${ANSI.reset} ${ANSI.comment}v${version}${ANSI.reset} ${vc}${BOX.h.repeat(w - 12)}${ANSI.reset}`,
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
        options.provider = requireNextArg(args, i, "action-right");
        i++;
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
        options.provider = requireNextArg(args, i, "--provider");
        i++;
        break;
      case "--verbose":
      case "-v":
        options.verbose = true;
        break;
      case "--waybar-dir":
        options.waybarDir = requireNextArg(args, i, "--waybar-dir");
        i++;
        break;
      case "--scripts-dir":
        options.scriptsDir = requireNextArg(args, i, "--scripts-dir");
        i++;
        break;
      case "--icons-dir":
        options.iconsDir = requireNextArg(args, i, "--icons-dir");
        i++;
        break;
      case "--qbar-bin":
        options.qbarBin = requireNextArg(args, i, "--qbar-bin");
        i++;
        break;
      case "--terminal-script":
        options.terminalScript = requireNextArg(args, i, "--terminal-script");
        i++;
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
