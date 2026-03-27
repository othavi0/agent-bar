import { describe, expect, it } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installWaybarAssets } from "../src/waybar-contract";
import {
  APP_STYLE_IMPORT,
  applyWaybarIntegration,
  removeWaybarIntegration,
  type WaybarIntegrationPaths,
} from "../src/waybar-integration";

function repoRoot(): string {
  return join(import.meta.dir, "..");
}

describe("waybar integration flow", () => {
  it("migrates legacy qbar wiring to agent-bar-omarchy and removes it cleanly", async () => {
    const root = await mkdtemp(join(tmpdir(), "agent-bar-omarchy-waybar-test-"));
    const waybarRoot = join(root, "waybar");
    const configPath = join(waybarRoot, "config.jsonc");
    const stylePath = join(waybarRoot, "style.css");
    const appDir = join(waybarRoot, "agent-bar-omarchy");
    const legacyDir = join(waybarRoot, "qbar");
    const scriptsDir = join(waybarRoot, "scripts");

    await mkdir(waybarRoot, { recursive: true });
    await mkdir(legacyDir, { recursive: true });
    await mkdir(scriptsDir, { recursive: true });

    const paths: WaybarIntegrationPaths = {
      waybarConfigPath: configPath,
      waybarStylePath: stylePath,
      modulesIncludePath: join(appDir, "modules.jsonc"),
      styleIncludePath: join(appDir, "style.css"),
    };
    const legacyModulesIncludePath = join(legacyDir, "modules.jsonc");
    const legacyStyleIncludePath = join(legacyDir, "style.css");
    const legacyHelperPath = join(scriptsDir, "qbar-open-terminal");

    await writeFile(
      configPath,
      JSON.stringify(
        {
          include: [legacyModulesIncludePath],
          "modules-left": ["clock"],
          "modules-right": ["tray", "custom/qbar-codex", "custom/qbar-claude"],
        },
        null,
        2,
      ),
      "utf8",
    );
    await writeFile(
      stylePath,
      `/* qbar managed import */\n@import url("./qbar/style.css");\n\n#clock { color: #fff; }\n`,
      "utf8",
    );
    await writeFile(legacyModulesIncludePath, "{}", "utf8");
    await writeFile(legacyStyleIncludePath, "/* old */\n", "utf8");
    await writeFile(legacyHelperPath, "#!/usr/bin/env bash\n", "utf8");

    const assets = installWaybarAssets({
      waybarDir: appDir,
      scriptsDir,
      repoRoot: repoRoot(),
    });

    expect(existsSync(join(assets.iconsDir, "amp-icon.svg"))).toBe(true);
    expect(existsSync(assets.terminalScript)).toBe(true);

    const applyResult = applyWaybarIntegration({
      paths,
      iconsDir: assets.iconsDir,
      appBin: "$HOME/.local/bin/agent-bar-omarchy",
      terminalScript: assets.terminalScript,
    });

    expect(existsSync(paths.modulesIncludePath)).toBe(true);
    expect(existsSync(paths.styleIncludePath)).toBe(true);
    expect(applyResult.moduleIDs.length).toBeGreaterThan(0);

    const configAfterApply = await readFile(configPath, "utf8");
    expect(configAfterApply).toContain(paths.modulesIncludePath);
    expect(configAfterApply).not.toContain(legacyModulesIncludePath);
    expect(configAfterApply).not.toContain("custom/qbar-");
    expect(configAfterApply).toContain("custom/agent-bar-omarchy-claude");

    const styleAfterApply = await readFile(stylePath, "utf8");
    expect(styleAfterApply).toContain(APP_STYLE_IMPORT);
    expect(styleAfterApply).not.toContain('./qbar/style.css');

    const generatedModules = await readFile(paths.modulesIncludePath, "utf8");
    expect(generatedModules).toContain("custom/agent-bar-omarchy-");
    expect(generatedModules).toContain('"exec-on-event": true');

    const generatedStyle = await readFile(paths.styleIncludePath, "utf8");
    expect(generatedStyle).toContain("#custom-agent-bar-omarchy-claude");

    expect(existsSync(legacyDir)).toBe(false);
    expect(existsSync(legacyHelperPath)).toBe(false);

    const removeResult = removeWaybarIntegration({ paths });
    expect(removeResult.removedIncludes.length).toBe(2);

    const configAfterRemove = await readFile(configPath, "utf8");
    expect(configAfterRemove).not.toContain(paths.modulesIncludePath);
    expect(configAfterRemove).not.toContain("custom/agent-bar-omarchy-");

    const styleAfterRemove = await readFile(stylePath, "utf8");
    expect(styleAfterRemove).not.toContain(APP_STYLE_IMPORT);
    expect(existsSync(paths.modulesIncludePath)).toBe(false);
    expect(existsSync(paths.styleIncludePath)).toBe(false);

    await rm(root, { recursive: true, force: true });
  });
});
