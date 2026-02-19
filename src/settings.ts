import { mkdir } from "fs/promises";
import { homedir } from "os";
import { join } from "path";

const XDG_CONFIG_HOME = Bun.env.XDG_CONFIG_HOME || join(homedir(), ".config");
const SETTINGS_DIR = join(XDG_CONFIG_HOME, "qbar");
const SETTINGS_FILE = join(SETTINGS_DIR, "settings.json");

export type WindowPolicy = "both" | "five_hour" | "seven_day";

export interface Settings {
  waybar: {
    providers: string[];
    showPercentage: boolean;
    separators: "pipe" | "dot" | "subtle" | "none";
    providerOrder: string[];
  };
  tooltip: {
    // Keep for backward compat, but simplify
    showWeekly: boolean;
    showResetTime: boolean;
    showProgressBar: boolean;
  };
  /** Per-provider model visibility. Key = provider id, value = array of model names to show. Empty array = show all. */
  models?: Record<string, string[]>;
  /** Per-provider window visibility policy. */
  windowPolicy?: Record<string, WindowPolicy>;
}

const DEFAULT_SETTINGS: Settings = {
  waybar: {
    providers: ["claude", "codex", "antigravity", "amp"],
    showPercentage: true,
    separators: "pipe",
    providerOrder: ["claude", "codex", "antigravity", "amp"],
  },
  tooltip: {
    showWeekly: true,
    showResetTime: true,
    showProgressBar: true,
  },
  models: {},
  windowPolicy: {
    codex: "both",
  },
};

export async function loadSettings(): Promise<Settings> {
  const file = Bun.file(SETTINGS_FILE);

  if (!(await file.exists())) {
    return { ...DEFAULT_SETTINGS };
  }

  try {
    const data = await file.json();
    return {
      waybar: { ...DEFAULT_SETTINGS.waybar, ...data.waybar },
      tooltip: { ...DEFAULT_SETTINGS.tooltip, ...data.tooltip },
      models: { ...DEFAULT_SETTINGS.models, ...data.models },
      windowPolicy: { ...DEFAULT_SETTINGS.windowPolicy, ...data.windowPolicy },
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function loadSettingsSync(): Settings {
  try {
    const { existsSync, readFileSync } = require("node:fs");
    if (!existsSync(SETTINGS_FILE)) {
      return { ...DEFAULT_SETTINGS };
    }
    const data = JSON.parse(readFileSync(SETTINGS_FILE, "utf-8"));
    return {
      waybar: { ...DEFAULT_SETTINGS.waybar, ...data.waybar },
      tooltip: { ...DEFAULT_SETTINGS.tooltip, ...data.tooltip },
      models: { ...DEFAULT_SETTINGS.models, ...data.models },
      windowPolicy: { ...DEFAULT_SETTINGS.windowPolicy, ...data.windowPolicy },
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export async function saveSettings(settings: Settings): Promise<void> {
  await mkdir(SETTINGS_DIR, { recursive: true });
  await Bun.write(SETTINGS_FILE, JSON.stringify(settings, null, 2));
}

export function getSettingsPath(): string {
  return SETTINGS_FILE;
}
