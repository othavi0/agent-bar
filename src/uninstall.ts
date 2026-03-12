#!/usr/bin/env bun

import * as p from "@clack/prompts";
import { existsSync, rmSync, unlinkSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { oneDark, colorize, semantic } from "./tui/colors";
import { CONFIG } from "./config";
import { getDefaultWaybarAssetPaths } from "./waybar-contract";

const HOME = homedir();
const defaults = getDefaultWaybarAssetPaths();
const QBAR_SETTINGS_DIR = join(HOME, ".config", "qbar");
const QBAR_SYMLINK = join(HOME, ".local", "bin", "qbar");
const OVERLAY_STATE_FILE = join(
  HOME,
  ".config",
  "waybar",
  ".flat-onedark-qbar-overlay.json",
);

function removeIfExists(path: string, removed: string[], failed: string[]) {
  if (!existsSync(path)) {
    return;
  }

  try {
    rmSync(path, { recursive: true, force: true });
    removed.push(path);
  } catch {
    failed.push(path);
  }
}

function unlinkIfExists(path: string, removed: string[], failed: string[]) {
  if (!existsSync(path)) {
    return;
  }

  try {
    unlinkSync(path);
    removed.push(path);
  } catch {
    failed.push(path);
  }
}

export async function main() {
  console.clear();

  p.intro(colorize("qbar uninstall", oneDark.red));

  if (existsSync(OVERLAY_STATE_FILE)) {
    p.log.warn(
      colorize(
        "flat-onedark qbar overlay appears to be enabled. Disable it from the theme repo before uninstalling qbar.",
        semantic.warning,
      ),
    );
  }

  p.note(
    [
      "This removes only qbar-owned files:",
      "",
      `  • ${defaults.waybarDir}`,
      `  • ${defaults.scriptsDir}/qbar-open-terminal`,
      `  • ${QBAR_SETTINGS_DIR}`,
      `  • ${CONFIG.paths.cache}`,
      `  • ${CONFIG.paths.legacyCache}`,
      `  • ${QBAR_SYMLINK}`,
      "",
      "Waybar config.jsonc and style.css are left untouched.",
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

  const removed: string[] = [];
  const failed: string[] = [];

  removeIfExists(defaults.waybarDir, removed, failed);
  unlinkIfExists(join(defaults.scriptsDir, "qbar-open-terminal"), removed, failed);
  removeIfExists(QBAR_SETTINGS_DIR, removed, failed);
  removeIfExists(CONFIG.paths.cache, removed, failed);
  removeIfExists(CONFIG.paths.legacyCache, removed, failed);
  unlinkIfExists(QBAR_SYMLINK, removed, failed);

  if (removed.length > 0) {
    p.log.success(colorize(`Removed ${removed.length} paths`, semantic.good));
  }

  if (failed.length > 0) {
    p.log.warn(colorize(`Failed to remove ${failed.length} paths`, semantic.warning));
  }

  p.outro(
    colorize(
      "qbar uninstalled. If the theme overlay was active, disable it separately from the theme repo.",
      semantic.good,
    ),
  );
}

if (import.meta.main) {
  main().catch((e) => {
    console.error("Uninstall failed:", e);
    process.exit(1);
  });
}
