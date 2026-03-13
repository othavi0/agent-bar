#!/usr/bin/env bun

import * as p from "@clack/prompts";
import { existsSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { oneDark, colorize, semantic } from "./tui/colors";
import { CONFIG } from "./config";
import { getDefaultWaybarAssetPaths } from "./waybar-contract";
import {
  getDefaultWaybarIntegrationPaths,
  removeWaybarIntegration,
} from "./waybar-integration";

const HOME = homedir();
const defaults = getDefaultWaybarAssetPaths();
const QBAR_SETTINGS_DIR = join(HOME, ".config", "qbar");
const QBAR_SYMLINK = join(HOME, ".local", "bin", "qbar");

export interface UninstallOptions {
  force?: boolean;
  title?: string;
}

function removePathIfExists(path: string, removed: string[], failed: string[]) {
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

function reloadWaybar(): void {
  try {
    Bun.spawn(["pkill", "-SIGUSR2", "waybar"], {
      stdout: "ignore",
      stderr: "ignore",
    });
  } catch {
    // noop
  }
}

export async function runUninstall(options: UninstallOptions = {}): Promise<void> {
  const force = options.force ?? false;
  const title = options.title ?? "qbar uninstall";
  const integrationPaths = getDefaultWaybarIntegrationPaths();

  console.clear();
  p.intro(colorize(title, oneDark.red));

  p.note(
    [
      "This removes qbar integration and qbar-owned paths:",
      "",
      `  • ${integrationPaths.waybarConfigPath} (qbar entries only)`,
      `  • ${integrationPaths.waybarStylePath} (qbar import only)`,
      `  • ${integrationPaths.modulesIncludePath}`,
      `  • ${integrationPaths.styleIncludePath}`,
      `  • ${defaults.waybarDir}`,
      `  • ${defaults.scriptsDir}/qbar-open-terminal`,
      `  • ${QBAR_SETTINGS_DIR}`,
      `  • ${CONFIG.paths.cache}`,
      `  • ${CONFIG.paths.legacyCache}`,
      `  • ${QBAR_SYMLINK}`,
    ].join("\n"),
    colorize("What gets removed", semantic.title),
  );

  if (!force) {
    const proceed = await p.confirm({
      message: "Continue with uninstall?",
      initialValue: false,
    });

    if (p.isCancel(proceed) || !proceed) {
      p.outro(colorize("Uninstall cancelled", semantic.muted));
      return;
    }
  }

  const removed: string[] = [];
  const failed: string[] = [];

  const integrationResult = removeWaybarIntegration({ paths: integrationPaths });

  removePathIfExists(defaults.waybarDir, removed, failed);
  removePathIfExists(join(defaults.scriptsDir, "qbar-open-terminal"), removed, failed);
  removePathIfExists(QBAR_SETTINGS_DIR, removed, failed);
  removePathIfExists(CONFIG.paths.cache, removed, failed);
  removePathIfExists(CONFIG.paths.legacyCache, removed, failed);
  removePathIfExists(QBAR_SYMLINK, removed, failed);

  if (integrationResult.configChanged) {
    p.log.success(
      colorize(`Updated ${integrationPaths.waybarConfigPath}`, semantic.good),
    );
  }

  if (integrationResult.styleChanged) {
    p.log.success(
      colorize(`Updated ${integrationPaths.waybarStylePath}`, semantic.good),
    );
  }

  if (integrationResult.removedIncludes.length > 0) {
    p.log.success(
      colorize(
        `Removed ${integrationResult.removedIncludes.length} generated qbar include files`,
        semantic.good,
      ),
    );
  }

  if (integrationResult.configChanged || integrationResult.styleChanged) {
    reloadWaybar();
    p.log.info(colorize("Waybar reload signal sent", semantic.subtitle));
  }

  if (removed.length > 0) {
    p.log.success(colorize(`Removed ${removed.length} paths`, semantic.good));
  }

  if (failed.length > 0) {
    p.log.warn(colorize(`Failed to remove ${failed.length} paths`, semantic.warning));
  }

  p.outro(colorize(`${title} complete`, semantic.good));
}

export async function main() {
  await runUninstall();
}

if (import.meta.main) {
  main().catch((e) => {
    console.error("Uninstall failed:", e);
    process.exit(1);
  });
}
