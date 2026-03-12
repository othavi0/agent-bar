#!/usr/bin/env bun

import * as p from "@clack/prompts";
import { mkdirSync, symlinkSync, unlinkSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { oneDark, colorize, semantic } from "./tui/colors";
import {
  getDefaultWaybarAssetPaths,
  installWaybarAssets,
} from "./waybar-contract";

const HOME = homedir();
const REPO_ROOT = join(import.meta.dir, "..");

function createSymlink(): string {
  const localBin = join(HOME, ".local", "bin");
  const link = join(localBin, "qbar");
  const target = join(REPO_ROOT, "scripts", "qbar");

  mkdirSync(localBin, { recursive: true });

  try {
    unlinkSync(link);
  } catch {}

  symlinkSync(target, link);
  return link;
}

export async function main() {
  console.clear();

  p.intro(colorize("qbar setup", oneDark.blue));

  const defaults = getDefaultWaybarAssetPaths();

  p.note(
    [
      "This setup no longer edits Waybar config.jsonc or style.css.",
      "",
      "It only does the safe qbar-owned work:",
      "  1. Copies icons into ~/.config/waybar/qbar/icons",
      "  2. Installs qbar-open-terminal into ~/.config/waybar/scripts",
      "  3. Creates ~/.local/bin/qbar",
      "",
      "Enable the actual Waybar overlay from the flat-onedark theme repo.",
    ].join("\n"),
    colorize("Safe setup", semantic.title),
  );

  const proceed = await p.confirm({
    message: "Install qbar assets and symlink?",
    initialValue: true,
  });

  if (p.isCancel(proceed) || !proceed) {
    p.outro(colorize("Setup cancelled", semantic.muted));
    return;
  }

  try {
    const assetResult = installWaybarAssets({
      waybarDir: defaults.waybarDir,
      scriptsDir: defaults.scriptsDir,
      repoRoot: REPO_ROOT,
    });
    const link = createSymlink();

    p.log.success(
      colorize(
        `Installed icons to ${assetResult.iconsDir}`,
        semantic.good,
      ),
    );
    p.log.success(
      colorize(
        `Installed terminal helper to ${assetResult.terminalScript}`,
        semantic.good,
      ),
    );
    p.log.success(colorize(`Installed symlink at ${link}`, semantic.good));

    p.note(
      "Use your theme-owned qbar overlay script to wire qbar into Waybar.\nFor flat-onedark, run scripts/enable-qbar-safe.sh from the theme repo.",
      colorize("Next step", semantic.subtitle),
    );

    p.outro(colorize("Safe setup complete", semantic.good));
  } catch (error) {
    p.outro(
      colorize(
        `Setup failed: ${error instanceof Error ? error.message : String(error)}`,
        semantic.danger,
      ),
    );
    process.exit(1);
  }
}

if (import.meta.main) {
  main().catch((e) => {
    console.error("Setup failed:", e);
    process.exit(1);
  });
}
