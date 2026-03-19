#!/usr/bin/env bun

import * as p from "@clack/prompts";
import { join } from "node:path";
import { getDefaultWaybarAssetPaths, installWaybarAssets } from "./waybar-contract";
import {
  applyWaybarIntegration,
  getDefaultWaybarIntegrationPaths,
} from "./waybar-integration";
import { oneDark, colorize, semantic } from "./tui/colors";

const REPO_ROOT = join(import.meta.dir, "..");

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

export async function main() {
  p.intro(colorize("qbar apply-local", oneDark.blue));

  const defaults = getDefaultWaybarAssetPaths();
  const integrationPaths = getDefaultWaybarIntegrationPaths();

  try {
    const s = p.spinner();

    s.start("Syncing assets...");
    const assets = installWaybarAssets({
      waybarDir: defaults.waybarDir,
      scriptsDir: defaults.scriptsDir,
      repoRoot: REPO_ROOT,
    });
    s.stop("Assets synced");

    s.start("Applying Waybar integration...");
    const integration = applyWaybarIntegration({
      iconsDir: assets.iconsDir,
      qbarBin: defaults.qbarBin,
      terminalScript: assets.terminalScript,
    });
    s.stop("Integration applied");

    s.start("Reloading Waybar...");
    reloadWaybar();
    s.stop("Waybar reloaded");

    p.log.success(colorize(`Icons: ${assets.iconsDir}`, semantic.good));
    p.log.success(colorize(`Helper: ${assets.terminalScript}`, semantic.good));
    p.log.success(
      colorize(
        integration.configChanged
          ? `Updated ${integrationPaths.waybarConfigPath}`
          : `${integrationPaths.waybarConfigPath} already in sync`,
        semantic.good,
      ),
    );
    p.log.success(
      colorize(
        integration.styleChanged
          ? `Updated ${integrationPaths.waybarStylePath}`
          : `${integrationPaths.waybarStylePath} already in sync`,
        semantic.good,
      ),
    );

    p.outro(colorize("Local apply complete", semantic.good));
  } catch (error) {
    p.outro(
      colorize(
        `Apply failed: ${error instanceof Error ? error.message : String(error)}`,
        semantic.danger,
      ),
    );
    process.exit(1);
  }
}

if (import.meta.main) {
  main().catch((error) => {
    console.error("Apply-local failed:", error);
    process.exit(1);
  });
}
