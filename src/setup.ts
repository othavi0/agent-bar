#!/usr/bin/env bun

/**
 * qbar setup - Automated Waybar configuration
 *
 * Copies assets, adds modules to config, injects CSS styles.
 * Designed for Omarchy users.
 */

import * as p from "@clack/prompts";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { loadSettings } from "./settings";
import { catppuccin, colorize, semantic } from "./tui/colors";

const HOME = homedir();
const WAYBAR_CONFIG = join(HOME, ".config", "waybar");
const WAYBAR_CONFIG_FILE = join(WAYBAR_CONFIG, "config.jsonc");
const WAYBAR_STYLE_FILE = join(WAYBAR_CONFIG, "style.css");
const QBAR_ICONS_DIR = join(WAYBAR_CONFIG, "qbar", "icons");
const QBAR_SCRIPTS_DIR = join(WAYBAR_CONFIG, "scripts");

// Get the qbar repo root (where this script lives)
const REPO_ROOT = join(import.meta.dir, "..");

const MODULES_CONFIG = `
  "custom/qbar-claude": {
    "exec": "$HOME/.local/bin/qbar --provider claude",
    "return-type": "json",
    "interval": 120,
    "tooltip": true,
    "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
    "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right claude"
  },
  "custom/qbar-codex": {
    "exec": "$HOME/.local/bin/qbar --provider codex",
    "return-type": "json",
    "interval": 120,
    "tooltip": true,
    "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
    "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right codex"
  },
  "custom/qbar-antigravity": {
    "exec": "$HOME/.local/bin/qbar --provider antigravity",
    "return-type": "json",
    "interval": 120,
    "tooltip": true,
    "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
    "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right antigravity"
  },
  "custom/qbar-amp": {
    "exec": "$HOME/.local/bin/qbar --provider amp",
    "return-type": "json",
    "interval": 120,
    "tooltip": true,
    "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
    "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right amp"
  }`;

// Catppuccin Mocha palette for CSS
const CSS_COLORS = {
  green: "#a6e3a1",
  yellow: "#f9e2af",
  orange: "#fab387",
  red: "#f38ba8",
  muted: "#6c7086",
  surface0: "#313244",
};

type SeparatorStyle = "pipe" | "dot" | "subtle" | "none";

function generateCSS(
  separatorStyle: SeparatorStyle = "pipe",
  providerOrder: string[] = ["claude", "codex", "antigravity", "amp"],
): string {
  const providers =
    providerOrder.length > 0
      ? providerOrder
      : ["claude", "codex", "antigravity", "amp"];
  const firstProvider = providers[0];
  const lastProvider = providers[providers.length - 1];

  // Base module styles
  let css = `
/* qbar - LLM quota monitor */
#custom-qbar-claude,
#custom-qbar-codex,
#custom-qbar-antigravity,
#custom-qbar-amp {
  padding-left: 22px;
  padding-right: 6px;
  background-size: 16px 16px;
  background-repeat: no-repeat;
  background-position: 4px center;
}

#custom-qbar-claude { background-image: url("qbar/icons/claude-code-icon.png"); }
#custom-qbar-codex { background-image: url("qbar/icons/codex-icon.png"); }
#custom-qbar-antigravity { background-image: url("qbar/icons/antigravity-icon.png"); }
#custom-qbar-amp { background-image: url("qbar/icons/amp-icon.svg"); }
`;

  // Separator styles
  if (separatorStyle !== "none") {
    const borderStyles: Record<string, { border: string; outer: string }> = {
      pipe: {
        border: `1px solid ${CSS_COLORS.muted}`,
        outer: `1px solid ${CSS_COLORS.muted}`,
      },
      dot: {
        border: `2px dashed ${CSS_COLORS.muted}`,
        outer: `2px dashed ${CSS_COLORS.muted}`,
      },
      subtle: {
        border: `1px solid ${CSS_COLORS.surface0}`,
        outer: `1px solid ${CSS_COLORS.surface0}`,
      },
    };

    const style = borderStyles[separatorStyle] ?? borderStyles.pipe;

    // Outer separators: left on first, right on last
    css += `
/* Separator: outer borders */
#custom-qbar-${firstProvider} { border-left: ${style.outer}; margin-left: 4px; padding-left: 26px; }
#custom-qbar-${lastProvider} { border-right: ${style.outer}; margin-right: 4px; }
`;

    // Inner separators: left border on every module except the first
    const innerModules = providers.slice(1);
    if (innerModules.length > 0) {
      css += `
/* Separator: inter-module borders */
${innerModules.map((p) => `#custom-qbar-${p}`).join(",\n")} {
  border-left: ${style.border};
}
`;
    }
  }

  // Status colors
  css += `
#custom-qbar-claude.ok, #custom-qbar-codex.ok, #custom-qbar-antigravity.ok, #custom-qbar-amp.ok { color: ${CSS_COLORS.green}; }
#custom-qbar-claude.low, #custom-qbar-codex.low, #custom-qbar-antigravity.low, #custom-qbar-amp.low { color: ${CSS_COLORS.yellow}; }
#custom-qbar-claude.warn, #custom-qbar-codex.warn, #custom-qbar-antigravity.warn, #custom-qbar-amp.warn { color: ${CSS_COLORS.orange}; }
#custom-qbar-claude.critical, #custom-qbar-codex.critical, #custom-qbar-antigravity.critical, #custom-qbar-amp.critical { color: ${CSS_COLORS.red}; }

#custom-qbar-claude.disconnected,
#custom-qbar-codex.disconnected,
#custom-qbar-antigravity.disconnected,
#custom-qbar-amp.disconnected {
  color: ${CSS_COLORS.red};
  padding-left: 30px;
  font-size: 14px;
}
`;

  return css;
}

function backup(filePath: string): string | null {
  if (!existsSync(filePath)) return null;
  const backupPath = `${filePath}.qbar-backup-${Date.now()}`;
  copyFileSync(filePath, backupPath);
  return backupPath;
}

function copyDir(src: string, dest: string): void {
  mkdirSync(dest, { recursive: true });
  for (const entry of readdirSync(src, { withFileTypes: true })) {
    const srcPath = join(src, entry.name);
    const destPath = join(dest, entry.name);
    if (entry.isDirectory()) {
      copyDir(srcPath, destPath);
    } else {
      copyFileSync(srcPath, destPath);
    }
  }
}

async function copyAssets(): Promise<boolean> {
  const spinner = p.spinner();
  spinner.start("Copying icons and scripts...");

  try {
    // Copy icons
    const iconsSource = join(REPO_ROOT, "icons");
    if (!existsSync(iconsSource)) {
      spinner.stop(colorize("Icons folder not found in repo", semantic.danger));
      return false;
    }
    copyDir(iconsSource, QBAR_ICONS_DIR);

    // Copy terminal helper script
    mkdirSync(QBAR_SCRIPTS_DIR, { recursive: true });
    const scriptSource = join(REPO_ROOT, "scripts", "qbar-open-terminal");
    const scriptDest = join(QBAR_SCRIPTS_DIR, "qbar-open-terminal");
    if (existsSync(scriptSource)) {
      copyFileSync(scriptSource, scriptDest);
      // Make executable
      const { chmodSync } = await import("node:fs");
      chmodSync(scriptDest, 0o755);
    }

    spinner.stop(colorize("Assets copied", semantic.good));
    return true;
  } catch (error) {
    spinner.stop(colorize(`Failed to copy assets: ${error}`, semantic.danger));
    return false;
  }
}

async function updateWaybarConfig(): Promise<boolean> {
  const spinner = p.spinner();
  spinner.start("Updating Waybar config...");

  try {
    if (!existsSync(WAYBAR_CONFIG_FILE)) {
      spinner.stop(
        colorize(
          "Waybar config not found at " + WAYBAR_CONFIG_FILE,
          semantic.danger,
        ),
      );
      return false;
    }

    // Backup first
    const backupPath = backup(WAYBAR_CONFIG_FILE);

    let content = readFileSync(WAYBAR_CONFIG_FILE, "utf-8");

    // Check if qbar modules already exist
    if (content.includes("custom/qbar-claude")) {
      spinner.stop(
        colorize("qbar modules already in config (skipped)", semantic.subtitle),
      );
      return true;
    }

    // Find modules-right and add qbar modules
    const modulesRightMatch = content.match(
      /"modules-right"\s*:\s*\[([^\]]*)\]/,
    );
    if (modulesRightMatch) {
      const existingModules = modulesRightMatch[1];
      const newModules =
        existingModules.trimEnd() +
        (existingModules.trim().endsWith(",") ? "" : ",") +
        '\n    "custom/qbar-claude",\n    "custom/qbar-codex",\n    "custom/qbar-antigravity",\n    "custom/qbar-amp"';
      content = content.replace(
        modulesRightMatch[0],
        `"modules-right": [${newModules}]`,
      );
    } else {
      spinner.stop(
        colorize("Could not find modules-right in config", semantic.danger),
      );
      return false;
    }

    // Add module definitions before the last closing brace
    const lastBrace = content.lastIndexOf("}");
    if (lastBrace === -1) {
      spinner.stop(
        colorize("Invalid JSON structure in config", semantic.danger),
      );
      return false;
    }

    // Check if there's content before the last brace (need comma)
    const beforeBrace = content.slice(0, lastBrace).trimEnd();
    const needsComma = !beforeBrace.endsWith(",") && !beforeBrace.endsWith("{");

    content = beforeBrace + (needsComma ? "," : "") + MODULES_CONFIG + "\n}";

    writeFileSync(WAYBAR_CONFIG_FILE, content);
    spinner.stop(
      colorize(`Config updated (backup: ${backupPath})`, semantic.good),
    );
    return true;
  } catch (error) {
    spinner.stop(
      colorize(`Failed to update config: ${error}`, semantic.danger),
    );
    return false;
  }
}

async function updateWaybarStyles(): Promise<boolean> {
  const spinner = p.spinner();
  spinner.start("Adding CSS styles...");

  try {
    if (!existsSync(WAYBAR_STYLE_FILE)) {
      spinner.stop(colorize("Waybar style.css not found", semantic.danger));
      return false;
    }

    // Backup first
    const backupPath = backup(WAYBAR_STYLE_FILE);

    // Load settings for separator style and provider order
    const settings = await loadSettings();
    const cssContent = generateCSS(
      settings.waybar.separators,
      settings.waybar.providerOrder,
    );

    let content = readFileSync(WAYBAR_STYLE_FILE, "utf-8");

    // Remove existing qbar CSS block if present (replace instead of skip)
    const qbarStart = content.indexOf("/* qbar - LLM quota monitor */");
    if (qbarStart !== -1) {
      // Find the end of the qbar block (last qbar-related rule)
      const qbarEnd = content.lastIndexOf("#custom-qbar-amp.disconnected");
      if (qbarEnd !== -1) {
        // Find the closing brace after the last rule
        const closingBrace = content.indexOf("}", qbarEnd);
        if (closingBrace !== -1) {
          content =
            content.substring(0, qbarStart).trimEnd() +
            "\n" +
            content.substring(closingBrace + 1).trimStart();
        }
      }
    }

    // Append fresh styles
    content = content.trimEnd() + "\n" + cssContent;
    writeFileSync(WAYBAR_STYLE_FILE, content);

    spinner.stop(
      colorize(
        `Styles ${qbarStart !== -1 ? "updated" : "added"} (backup: ${backupPath})`,
        semantic.good,
      ),
    );
    return true;
  } catch (error) {
    spinner.stop(
      colorize(`Failed to update styles: ${error}`, semantic.danger),
    );
    return false;
  }
}

async function createSymlink(): Promise<boolean> {
  const spinner = p.spinner();
  spinner.start("Creating qbar symlink...");

  try {
    const localBin = join(HOME, ".local", "bin");
    mkdirSync(localBin, { recursive: true });

    const target = join(REPO_ROOT, "scripts", "qbar");
    const link = join(localBin, "qbar");

    // Remove existing symlink if present
    const { unlinkSync, symlinkSync } = await import("node:fs");
    try {
      unlinkSync(link);
    } catch {}

    symlinkSync(target, link);
    spinner.stop(colorize("Symlink created: ~/.local/bin/qbar", semantic.good));
    return true;
  } catch (error) {
    spinner.stop(
      colorize(`Failed to create symlink: ${error}`, semantic.danger),
    );
    return false;
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
    spinner.stop(
      colorize("Could not reload Waybar (try manually)", semantic.warning),
    );
  }
}

export async function main() {
  console.clear();

  p.intro(colorize("qbar setup", catppuccin.mauve));

  p.note(
    [
      "This will configure qbar for your Waybar:",
      "",
      "  1. Copy icons to ~/.config/waybar/qbar/",
      "  2. Add qbar modules to waybar config",
      "  3. Add CSS styles for the modules",
      "  4. Create ~/.local/bin/qbar symlink",
      "  5. Reload Waybar",
      "",
      "Backups are created before modifying files.",
    ].join("\n"),
    colorize("What happens", semantic.title),
  );

  const proceed = await p.confirm({
    message: "Continue with setup?",
    initialValue: true,
  });

  if (p.isCancel(proceed) || !proceed) {
    p.outro(colorize("Setup cancelled", semantic.muted));
    return;
  }

  const steps = [
    { name: "Copy assets", fn: copyAssets },
    { name: "Update config", fn: updateWaybarConfig },
    { name: "Add styles", fn: updateWaybarStyles },
    { name: "Create symlink", fn: createSymlink },
  ];

  let allOk = true;
  for (const step of steps) {
    const ok = await step.fn();
    if (!ok) {
      allOk = false;
      const cont = await p.confirm({
        message: `${step.name} failed. Continue anyway?`,
        initialValue: false,
      });
      if (p.isCancel(cont) || !cont) {
        p.outro(colorize("Setup aborted", semantic.danger));
        return;
      }
    }
  }

  await reloadWaybar();

  if (allOk) {
    p.outro(
      colorize(
        'Setup complete! Use "qbar menu" to get started.',
        semantic.good,
      ),
    );
  } else {
    p.outro(
      colorize(
        "Setup finished with warnings. Check messages above.",
        semantic.warning,
      ),
    );
  }
}

// Only auto-run when executed directly (bun src/setup.ts), not when imported
if (import.meta.main) {
  main().catch((e) => {
    console.error("Setup failed:", e);
    process.exit(1);
  });
}
