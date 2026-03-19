#!/usr/bin/env bun

import * as p from "@clack/prompts";
import { mkdirSync, symlinkSync, unlinkSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { oneDark, colorize, semantic } from "./tui/colors";
import {
  getDefaultWaybarAssetPaths,
  installWaybarAssets,
} from "./waybar-contract";
import {
  applyWaybarIntegration,
  getDefaultWaybarIntegrationPaths,
} from "./waybar-integration";

const HOME = homedir();
const REPO_ROOT = join(import.meta.dir, "..");

export function createSymlink(): string {
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
  console.clear();

  p.intro(colorize("qbar setup", oneDark.blue));

  const defaults = getDefaultWaybarAssetPaths();
  const integrationPaths = getDefaultWaybarIntegrationPaths();

  p.note(
    [
      "This setup is theme-agnostic and fully managed by qbar.",
      "",
      "It will:",
      "  1. Install qbar icons + terminal helper",
      "  2. Create ~/.local/bin/qbar symlink",
      `  3. Wire ${integrationPaths.waybarConfigPath}`,
      `  4. Wire ${integrationPaths.waybarStylePath}`,
      "  5. Reload Waybar",
    ].join("\n"),
    colorize("Setup", semantic.title),
  );

  const proceed = await p.confirm({
    message: "Apply qbar setup now?",
    initialValue: true,
  });

  if (p.isCancel(proceed) || !proceed) {
    p.outro(colorize("Setup cancelled", semantic.muted));
    return;
  }

  try {
    const s = p.spinner();

    s.start("Installing icons and terminal helper...");
    const assetResult = installWaybarAssets({
      waybarDir: defaults.waybarDir,
      scriptsDir: defaults.scriptsDir,
      repoRoot: REPO_ROOT,
    });
    s.stop("Assets installed");

    s.start("Creating symlink...");
    const link = createSymlink();
    s.stop("Symlink created");

    s.start("Wiring Waybar config and styles...");
    const integrationResult = applyWaybarIntegration({
      iconsDir: assetResult.iconsDir,
      qbarBin: defaults.qbarBin,
      terminalScript: assetResult.terminalScript,
    });
    s.stop("Waybar integration applied");

    s.start("Reloading Waybar...");
    reloadWaybar();
    s.stop("Waybar reloaded");

    p.log.success(colorize(`Icons: ${assetResult.iconsDir}`, semantic.good));
    p.log.success(colorize(`Helper: ${assetResult.terminalScript}`, semantic.good));
    p.log.success(colorize(`Symlink: ${link}`, semantic.good));
    p.log.success(
      colorize(
        integrationResult.configChanged
          ? `Updated ${integrationPaths.waybarConfigPath}`
          : `${integrationPaths.waybarConfigPath} already in sync`,
        semantic.good,
      ),
    );
    p.log.success(
      colorize(
        integrationResult.styleChanged
          ? `Updated ${integrationPaths.waybarStylePath}`
          : `${integrationPaths.waybarStylePath} already in sync`,
        semantic.good,
      ),
    );

    const localBin = join(HOME, ".local", "bin");
    const pathDirs = (process.env.PATH ?? "").split(":");
    if (!pathDirs.some((d) => resolve(d) === resolve(localBin))) {
      p.log.warn(
        colorize(
          `~/.local/bin is not in your PATH. Add to your shell profile:\n  export PATH="$HOME/.local/bin:$PATH"`,
          semantic.warning,
        ),
      );
    }

    p.outro(colorize("Setup complete", semantic.good));
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
