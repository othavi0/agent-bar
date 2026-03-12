import * as p from "@clack/prompts";
import { oneDark, colorize, semantic } from "./colors";
import { configureLayout } from "./configure-layout";
import { configureModels } from "./configure-models";
import { configureWaybar } from "./configure-waybar";
import { showListAll } from "./list-all";
import { loginProviderFlow } from "./login";

import pkg from "../../package.json";

const VERSION = pkg.version;
/** Delay between animation frames in ms */
const LOGO_ANIM_FRAME_MS = 12;

type MenuAction = "list" | "waybar" | "models" | "layout" | "login";

// Block-style logo inspired by Omarchy branding
const LOGO_LINES = [
  "         █████    ████   █████  ",
  "  ████   ██  ██  ██  ██  ██  ██ ",
  " ██  ██  █████   ██  ██  █████  ",
  " ██  ██  ██  ██  ██████  ██  ██ ",
  "  █████  █████   ██  ██  ██  ██ ",
  "     ██                         ",
];

const GRADIENT: number[][] = [
  [209, 154, 102], // orange  #d19a66
  [229, 192, 123], // yellow  #e5c07b
  [152, 195, 121], // green   #98c379
  [86, 182, 194],  // cyan    #56b6c2
  [97, 175, 239],  // blue    #61afef
  [198, 120, 221], // magenta #c678dd
];

function gradientColor(t: number): string {
  const seg = t * (GRADIENT.length - 1);
  const i = Math.min(Math.floor(seg), GRADIENT.length - 2);
  const f = seg - i;
  const r = Math.round(GRADIENT[i][0] + (GRADIENT[i + 1][0] - GRADIENT[i][0]) * f);
  const g = Math.round(GRADIENT[i][1] + (GRADIENT[i + 1][1] - GRADIENT[i][1]) * f);
  const b = Math.round(GRADIENT[i][2] + (GRADIENT[i + 1][2] - GRADIENT[i][2]) * f);
  return `\x1b[38;2;${r};${g};${b}m`;
}

function colorLogo(): string {
  const maxLen = Math.max(...LOGO_LINES.map((l) => l.length));
  return LOGO_LINES.map((line) => {
    const chars = [...line];
    return (
      chars
        .map((ch, idx) => {
          if (!ch.trim()) return ch;
          return gradientColor(idx / maxLen) + ch;
        })
        .join("") + oneDark.reset
    );
  }).join("\n");
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

async function animateLogo(): Promise<void> {
  const maxLen = Math.max(...LOGO_LINES.map((l) => l.length));
  const height = LOGO_LINES.length;

  // Hide cursor (only if writing to a real terminal)
  if (process.stdout.isTTY) process.stdout.write("\x1b[?25l");

  // Reserve lines
  for (let i = 0; i < height; i++) process.stdout.write("\n");

  // Reveal column by column in chunks
  const step = 2;
  for (let col = 0; col <= maxLen; col += step) {
    // Move cursor up to first logo line
    process.stdout.write(`\x1b[${height}A`);

    for (let row = 0; row < height; row++) {
      const line = LOGO_LINES[row];
      let out = "\r";
      for (let c = 0; c < col && c < line.length; c++) {
        const ch = line[c];
        if (!ch.trim()) {
          out += ch;
        } else {
          out += gradientColor(c / maxLen) + ch;
        }
      }
      out += oneDark.reset + "\x1b[K\n";
      process.stdout.write(out);
    }

    await sleep(LOGO_ANIM_FRAME_MS);
  }

  // Final full render
  process.stdout.write(`\x1b[${height}A`);
  for (const line of LOGO_LINES) {
    let out = "\r";
    for (let c = 0; c < line.length; c++) {
      const ch = line[c];
      if (!ch.trim()) {
        out += ch;
      } else {
        out += gradientColor(c / maxLen) + ch;
      }
    }
    process.stdout.write(out + oneDark.reset + "\x1b[K\n");
  }

  // Show cursor
  if (process.stdout.isTTY) process.stdout.write("\x1b[?25h");
}

export async function runTui(): Promise<void> {
  console.clear();

  await animateLogo();
  console.log();

  p.intro(colorize(`  v${VERSION}`, semantic.subtitle));

  p.note(
    [
      colorize("↑↓", semantic.highlight) +
        " navigate  " +
        colorize("Enter", semantic.highlight) +
        " select  " +
        colorize("q", semantic.highlight) +
        " quit",
    ].join("\n"),
    colorize("Controls", semantic.title),
  );

  let running = true;

  while (running) {
    const result = await p.select({
      message: colorize("What would you like to do?", semantic.title),
      options: [
        {
          value: "list" as const,
          label: colorize("List all", oneDark.text),
          hint: colorize("view quotas for all providers", semantic.muted),
        },
        {
          value: "waybar" as const,
          label: colorize("Configure Waybar", oneDark.text),
          hint: colorize("select providers for the bar", semantic.muted),
        },
        {
          value: "models" as const,
          label: colorize("Configure Models", oneDark.text),
          hint: colorize("show/hide models in tooltip", semantic.muted),
        },
        {
          value: "layout" as const,
          label: colorize("Customize Layout", oneDark.text),
          hint: colorize("reorder providers, separator style", semantic.muted),
        },
        {
          value: "login" as const,
          label: colorize("Provider login", oneDark.text),
          hint: colorize("launch provider CLI login flows", semantic.muted),
        },
      ],
    });

    if (p.isCancel(result)) {
      running = false;
      continue;
    }

    const action = result as MenuAction;

    p.log.step(colorize(`→ ${action}`, semantic.accent));

    switch (action) {
      case "list":
        await showListAll();
        break;

      case "waybar":
        await configureWaybar();
        break;

      case "models":
        await configureModels();
        break;

      case "layout":
        await configureLayout();
        break;

      case "login":
        await loginProviderFlow();
        break;
    }
  }

  p.outro(colorize("Goodbye!", semantic.muted));
}

process.on("SIGINT", () => {
  if (process.stdout.isTTY) process.stdout.write("\x1b[?25h");
  console.log("");
  p.outro(colorize("Cancelled", semantic.muted));
  process.exit(0);
});
