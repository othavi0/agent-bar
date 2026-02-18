#!/usr/bin/env bun

/**
 * qbar uninstall - Remove all qbar files from system
 */

import * as p from "@clack/prompts";
import {
  existsSync,
  readFileSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { catppuccin, colorize, semantic } from "./tui/colors";

const HOME = homedir();
const WAYBAR_CONFIG = join(HOME, ".config", "waybar");
const WAYBAR_CONFIG_FILE = join(WAYBAR_CONFIG, "config.jsonc");
const WAYBAR_STYLE_FILE = join(WAYBAR_CONFIG, "style.css");
const QBAR_DIR = join(WAYBAR_CONFIG, "qbar");
const QBAR_TERMINAL_SCRIPT = join(
  WAYBAR_CONFIG,
  "scripts",
  "qbar-open-terminal",
);
const QBAR_SETTINGS_DIR = join(HOME, ".config", "qbar");
const QBAR_SYMLINK = join(HOME, ".local", "bin", "qbar");

async function removeFiles(): Promise<void> {
  const spinner = p.spinner();
  spinner.start("Removing qbar files...");

  const removed: string[] = [];
  const failed: string[] = [];

  // Remove qbar directory (icons + cache)
  if (existsSync(QBAR_DIR)) {
    try {
      rmSync(QBAR_DIR, { recursive: true, force: true });
      removed.push("~/.config/waybar/qbar/");
    } catch {
      failed.push("~/.config/waybar/qbar/");
    }
  }

  // Remove terminal script
  if (existsSync(QBAR_TERMINAL_SCRIPT)) {
    try {
      unlinkSync(QBAR_TERMINAL_SCRIPT);
      removed.push("~/.config/waybar/scripts/qbar-open-terminal");
    } catch {
      failed.push("~/.config/waybar/scripts/qbar-open-terminal");
    }
  }

  // Remove settings
  if (existsSync(QBAR_SETTINGS_DIR)) {
    try {
      rmSync(QBAR_SETTINGS_DIR, { recursive: true, force: true });
      removed.push("~/.config/qbar/");
    } catch {
      failed.push("~/.config/qbar/");
    }
  }

  // Remove symlink
  if (existsSync(QBAR_SYMLINK)) {
    try {
      unlinkSync(QBAR_SYMLINK);
      removed.push("~/.local/bin/qbar");
    } catch {
      failed.push("~/.local/bin/qbar");
    }
  }

  spinner.stop(colorize(`Removed ${removed.length} items`, semantic.good));

  if (removed.length > 0) {
    p.log.info(colorize("Removed:", semantic.subtitle));
    for (const item of removed) {
      console.log(colorize(`  ✓ ${item}`, semantic.good));
    }
  }

  if (failed.length > 0) {
    p.log.warn(colorize("Failed to remove:", semantic.warning));
    for (const item of failed) {
      console.log(colorize(`  ✗ ${item}`, semantic.danger));
    }
  }
}

async function cleanWaybarConfig(): Promise<void> {
  const spinner = p.spinner();
  spinner.start("Cleaning Waybar config...");

  try {
    if (!existsSync(WAYBAR_CONFIG_FILE)) {
      spinner.stop(colorize("No Waybar config found", semantic.subtitle));
      return;
    }

    let content = readFileSync(WAYBAR_CONFIG_FILE, "utf-8");
    const original = content;

    // Remove qbar modules from modules-right
    content = content.replace(/"custom\/qbar-claude"\s*,?\s*/g, "");
    content = content.replace(/"custom\/qbar-codex"\s*,?\s*/g, "");
    content = content.replace(/"custom\/qbar-antigravity"\s*,?\s*/g, "");
    content = content.replace(/"custom\/qbar-amp"\s*,?\s*/g, "");

    // Remove module definitions (multi-line)
    content = content.replace(
      /,?\s*"custom\/qbar-claude"\s*:\s*\{[^}]*\}/gs,
      "",
    );
    content = content.replace(
      /,?\s*"custom\/qbar-codex"\s*:\s*\{[^}]*\}/gs,
      "",
    );
    content = content.replace(
      /,?\s*"custom\/qbar-antigravity"\s*:\s*\{[^}]*\}/gs,
      "",
    );
    content = content.replace(/,?\s*"custom\/qbar-amp"\s*:\s*\{[^}]*\}/gs, "");

    // Clean up trailing commas before ]
    content = content.replace(/,(\s*\])/g, "$1");

    if (content !== original) {
      writeFileSync(WAYBAR_CONFIG_FILE, content);
      spinner.stop(colorize("Waybar config cleaned", semantic.good));
    } else {
      spinner.stop(colorize("No qbar entries in config", semantic.subtitle));
    }
  } catch (error) {
    spinner.stop(colorize(`Failed to clean config: ${error}`, semantic.danger));
  }
}

async function cleanWaybarStyles(): Promise<void> {
  const spinner = p.spinner();
  spinner.start("Cleaning CSS styles...");

  try {
    if (!existsSync(WAYBAR_STYLE_FILE)) {
      spinner.stop(colorize("No Waybar styles found", semantic.subtitle));
      return;
    }

    let content = readFileSync(WAYBAR_STYLE_FILE, "utf-8");
    const original = content;

    // Remove qbar CSS blocks (all variations)
    content = content.replace(
      /\/\* qbar.*?\*\/[\s\S]*?#custom-qbar-(?:antigravity|amp)\.disconnected\s*\{[^}]*\}\s*/g,
      "",
    );
    content = content.replace(
      /#custom-qbar-claude[\s\S]*?#custom-qbar-(?:antigravity|amp)\.disconnected\s*\{[^}]*\}\s*/g,
      "",
    );

    if (content !== original) {
      writeFileSync(WAYBAR_STYLE_FILE, content);
      spinner.stop(colorize("CSS styles cleaned", semantic.good));
    } else {
      spinner.stop(colorize("No qbar styles found", semantic.subtitle));
    }
  } catch (error) {
    spinner.stop(colorize(`Failed to clean styles: ${error}`, semantic.danger));
  }
}

async function reloadWaybar(): Promise<void> {
  const spinner = p.spinner();
  spinner.start("Reloading Waybar...");

  try {
    Bun.spawn(["pkill", "-USR2", "waybar"]);
    await Bun.sleep(500);
    spinner.stop(colorize("Waybar reloaded", semantic.good));
  } catch {
    spinner.stop(colorize("Could not reload Waybar", semantic.subtitle));
  }
}

export async function main() {
  console.clear();

  p.intro(colorize("qbar uninstall", catppuccin.red));

  p.note(
    [
      "This will remove:",
      "",
      "  • ~/.config/waybar/qbar/ (icons, cache)",
      "  • ~/.config/waybar/scripts/qbar-open-terminal",
      "  • ~/.config/qbar/ (settings)",
      "  • ~/.local/bin/qbar (symlink)",
      "  • qbar entries from Waybar config",
      "  • qbar styles from Waybar CSS",
      "",
      "The qbar source code (this folder) is NOT deleted.",
    ].join("\n"),
    colorize("What gets removed", semantic.title),
  );

  const proceed = await p.confirm({
    message: "Continue with uninstall?",
    initialValue: false,
  });

  if (p.isCancel(proceed) || !proceed) {
    p.outro(colorize("Uninstall cancelled", semantic.muted));
    return;
  }

  await removeFiles();
  await cleanWaybarConfig();
  await cleanWaybarStyles();
  await reloadWaybar();

  p.outro(
    colorize(
      "qbar uninstalled. You can delete this folder manually if needed.",
      semantic.good,
    ),
  );
}

// Only auto-run when executed directly
if (import.meta.main) {
  main().catch((e) => {
    console.error("Uninstall failed:", e);
    process.exit(1);
  });
}
