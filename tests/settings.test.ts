import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  getSettingsPath,
  loadSettings,
  loadSettingsSync,
  saveSettings,
} from "../src/settings";

describe("settings", () => {
  let testRoot = "";
  let previousXdgConfigHome: string | undefined;

  beforeEach(async () => {
    testRoot = await mkdtemp(join(tmpdir(), "agent-bar-omarchy-settings-test-"));
    previousXdgConfigHome = process.env.XDG_CONFIG_HOME;
    process.env.XDG_CONFIG_HOME = testRoot;
  });

  afterEach(async () => {
    if (previousXdgConfigHome === undefined) {
      delete process.env.XDG_CONFIG_HOME;
    } else {
      process.env.XDG_CONFIG_HOME = previousXdgConfigHome;
    }

    await rm(testRoot, { recursive: true, force: true });
  });

  it("returns normalized defaults when no file exists", () => {
    const settings = loadSettingsSync();

    expect(settings.version).toBe(1);
    expect(settings.waybar.providers).toEqual(["claude", "codex", "amp"]);
    expect(settings.waybar.providerOrder).toEqual(["claude", "codex", "amp"]);
    expect(settings.waybar.separators).toBe("gap");
    expect(getSettingsPath()).toBe(
      join(testRoot, "agent-bar-omarchy", "settings.json"),
    );
  });

  it("saves and loads settings from the new namespace", async () => {
    await saveSettings({
      version: 1,
      waybar: {
        providers: ["codex", "claude"],
        showPercentage: false,
        separators: "pill",
        providerOrder: ["codex", "claude"],
      },
      tooltip: {},
      models: {},
      windowPolicy: { codex: "both" },
    });

    const settingsPath = getSettingsPath();
    expect(settingsPath).toBe(join(testRoot, "agent-bar-omarchy", "settings.json"));
    expect(existsSync(settingsPath)).toBe(true);

    const loaded = await loadSettings();
    expect(loaded.waybar.providers).toEqual(["codex", "claude"]);
    expect(loaded.waybar.providerOrder).toEqual(["codex", "claude"]);
    expect(loaded.waybar.separators).toBe("pill");

    const onDisk = JSON.parse(await readFile(settingsPath, "utf8"));
    expect(onDisk.waybar.providers).toEqual(["codex", "claude"]);
  });

  it("moves legacy qbar settings into the new namespace", () => {
    const legacyDir = join(testRoot, "qbar");
    const legacyFile = join(legacyDir, "settings.json");

    return mkdir(legacyDir, { recursive: true })
      .then(() =>
        writeFile(
          legacyFile,
          JSON.stringify({
            version: 1,
            waybar: {
              providers: ["amp", "claude"],
              showPercentage: true,
              separators: "glass",
              providerOrder: ["amp", "claude"],
            },
            tooltip: {},
            models: {},
            windowPolicy: {},
          }),
        ),
      )
      .then(() => {
        const settings = loadSettingsSync();
        const newFile = join(testRoot, "agent-bar-omarchy", "settings.json");

        expect(settings.waybar.providers).toEqual(["amp", "claude"]);
        expect(existsSync(newFile)).toBe(true);
        expect(existsSync(legacyFile)).toBe(false);
      });
  });

  it("does not overwrite existing new settings when a legacy directory still exists", async () => {
    const newDir = join(testRoot, "agent-bar-omarchy");
    const legacyDir = join(testRoot, "qbar");
    const newFile = join(newDir, "settings.json");
    const legacyFile = join(legacyDir, "settings.json");

    await mkdir(newDir, { recursive: true });
    await mkdir(legacyDir, { recursive: true });

    await writeFile(
      newFile,
      JSON.stringify({
        version: 1,
        waybar: {
          providers: ["claude"],
          showPercentage: true,
          separators: "gap",
          providerOrder: ["claude"],
        },
        tooltip: {},
        models: {},
        windowPolicy: {},
      }),
    );
    await writeFile(
      legacyFile,
      JSON.stringify({
        version: 1,
        waybar: {
          providers: ["amp"],
          showPercentage: true,
          separators: "shadow",
          providerOrder: ["amp"],
        },
        tooltip: {},
        models: {},
        windowPolicy: {},
      }),
    );

    const settings = loadSettingsSync();
    expect(settings.waybar.providers).toEqual(["claude"]);
    expect(JSON.parse(await readFile(newFile, "utf8")).waybar.providers).toEqual(["claude"]);
    expect(existsSync(legacyFile)).toBe(true);
  });
});
