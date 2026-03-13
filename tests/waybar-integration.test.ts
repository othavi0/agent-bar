import { describe, expect, it } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installWaybarAssets } from "../src/waybar-contract";
import {
  QBAR_STYLE_IMPORT,
  applyWaybarIntegration,
  removeWaybarIntegration,
  type WaybarIntegrationPaths,
} from "../src/waybar-integration";

function repoRoot(): string {
  return join(import.meta.dir, "..");
}

describe("waybar integration flow", () => {
  it("installs assets, applies integration, and removes integration", async () => {
    const root = await mkdtemp(join(tmpdir(), "qbar-waybar-test-"));
    const waybarRoot = join(root, "waybar");
    const configPath = join(waybarRoot, "config.jsonc");
    const stylePath = join(waybarRoot, "style.css");
    const qbarDir = join(waybarRoot, "qbar");
    const scriptsDir = join(waybarRoot, "scripts");

    await mkdir(waybarRoot, { recursive: true });

    const paths: WaybarIntegrationPaths = {
      waybarConfigPath: configPath,
      waybarStylePath: stylePath,
      modulesIncludePath: join(qbarDir, "modules.jsonc"),
      styleIncludePath: join(qbarDir, "style.css"),
    };

    await writeFile(
      configPath,
      JSON.stringify(
        {
          "modules-left": ["clock"],
          "modules-right": ["tray"],
        },
        null,
        2,
      ),
      "utf8",
    );
    await writeFile(stylePath, "#clock { color: #fff; }\n", "utf8");

    const assets = installWaybarAssets({
      waybarDir: qbarDir,
      scriptsDir,
      repoRoot: repoRoot(),
    });

    expect(existsSync(join(assets.iconsDir, "amp-icon.svg"))).toBe(true);
    expect(existsSync(assets.terminalScript)).toBe(true);

    const applyResult = applyWaybarIntegration({
      paths,
      iconsDir: assets.iconsDir,
      qbarBin: "$HOME/.local/bin/qbar",
      terminalScript: assets.terminalScript,
    });

    expect(existsSync(paths.modulesIncludePath)).toBe(true);
    expect(existsSync(paths.styleIncludePath)).toBe(true);
    expect(applyResult.moduleIDs.length).toBeGreaterThan(0);

    const configAfterApply = await readFile(configPath, "utf8");
    expect(configAfterApply).toContain(paths.modulesIncludePath);
    for (const moduleID of applyResult.moduleIDs) {
      expect(configAfterApply).toContain(`"${moduleID}"`);
    }

    const styleAfterApply = await readFile(stylePath, "utf8");
    expect(styleAfterApply).toContain(QBAR_STYLE_IMPORT);

    const generatedModules = await readFile(paths.modulesIncludePath, "utf8");
    expect(generatedModules).toContain("custom/qbar-");

    const generatedStyle = await readFile(paths.styleIncludePath, "utf8");
    expect(generatedStyle).toContain("#custom-qbar-claude");

    const removeResult = removeWaybarIntegration({ paths });
    expect(removeResult.removedIncludes.length).toBe(2);

    const configAfterRemove = await readFile(configPath, "utf8");
    expect(configAfterRemove).not.toContain(paths.modulesIncludePath);
    for (const moduleID of applyResult.moduleIDs) {
      expect(configAfterRemove).not.toContain(`"${moduleID}"`);
    }

    const styleAfterRemove = await readFile(stylePath, "utf8");
    expect(styleAfterRemove).not.toContain(QBAR_STYLE_IMPORT);
    expect(existsSync(paths.modulesIncludePath)).toBe(false);
    expect(existsSync(paths.styleIncludePath)).toBe(false);

    await rm(root, { recursive: true, force: true });
  });
});
