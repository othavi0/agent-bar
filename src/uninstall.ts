#!/usr/bin/env bun

import * as p from "@clack/prompts";
import { existsSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { APP_NAME, LEGACY_APP_NAME } from "./app-identity";
import { oneDark, colorize, semantic } from "./tui/colors";
import { CONFIG } from "./config";
import { getDefaultWaybarAssetPaths, getLegacyWaybarAssetPaths } from "./waybar-contract";
import {
  getDefaultWaybarIntegrationPaths,
  removeWaybarIntegration,
} from "./waybar-integration";

const HOME = homedir();
const defaults = getDefaultWaybarAssetPaths();
const legacyDefaults = getLegacyWaybarAssetPaths(join(HOME, ".config", "waybar"));
const SETTINGS_DIR = join(HOME, ".config", APP_NAME);
const LEGACY_SETTINGS_DIR = join(HOME, ".config", LEGACY_APP_NAME);
const APP_SYMLINK = join(HOME, ".local", "bin", APP_NAME);
const LEGACY_SYMLINK = join(HOME, ".local", "bin", LEGACY_APP_NAME);

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
  const title = options.title ?? `${APP_NAME} uninstall`;
  const integrationPaths = getDefaultWaybarIntegrationPaths();

  console.clear();
  p.intro(colorize(title, oneDark.red));

  p.note(
    [
      `This removes ${APP_NAME} integration and owned paths, plus legacy ${LEGACY_APP_NAME} artifacts:`,
      "",
      `  • ${integrationPaths.waybarConfigPath} (${APP_NAME} entries only)`,
      `  • ${integrationPaths.waybarStylePath} (${APP_NAME} import only)`,
      `  • ${integrationPaths.modulesIncludePath}`,
      `  • ${integrationPaths.styleIncludePath}`,
      `  • ${defaults.waybarDir}`,
      `  • ${defaults.terminalScript}`,
      `  • ${legacyDefaults.waybarDir}`,
      `  • ${legacyDefaults.terminalScript}`,
      `  • ${SETTINGS_DIR}`,
      `  • ${LEGACY_SETTINGS_DIR}`,
      `  • ${CONFIG.paths.cache}`,
      `  • ${CONFIG.paths.legacyCache}`,
      `  • ${CONFIG.paths.waybarLegacyCache}`,
      `  • ${APP_SYMLINK}`,
      `  • ${LEGACY_SYMLINK}`,
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
  const s = p.spinner();

  s.start("Removing Waybar integration...");
  const integrationResult = removeWaybarIntegration({ paths: integrationPaths });
  s.stop("Waybar integration removed");

  s.start("Cleaning up files...");
  removePathIfExists(defaults.waybarDir, removed, failed);
  removePathIfExists(defaults.terminalScript, removed, failed);
  removePathIfExists(legacyDefaults.waybarDir, removed, failed);
  removePathIfExists(legacyDefaults.terminalScript, removed, failed);
  removePathIfExists(SETTINGS_DIR, removed, failed);
  removePathIfExists(LEGACY_SETTINGS_DIR, removed, failed);
  removePathIfExists(CONFIG.paths.cache, removed, failed);
  removePathIfExists(CONFIG.paths.legacyCache, removed, failed);
  removePathIfExists(CONFIG.paths.waybarLegacyCache, removed, failed);
  removePathIfExists(APP_SYMLINK, removed, failed);
  removePathIfExists(LEGACY_SYMLINK, removed, failed);
  s.stop("Files cleaned up");

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
        `Removed ${integrationResult.removedIncludes.length} generated include files`,
        semantic.good,
      ),
    );
  }

  if (integrationResult.configChanged || integrationResult.styleChanged) {
    s.start("Reloading Waybar...");
    reloadWaybar();
    s.stop("Waybar reloaded");
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
