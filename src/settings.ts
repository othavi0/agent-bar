import { mkdir } from "fs/promises";
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "os";
import { join } from "path";
import { normalizeProviderSelection } from "./waybar-contract";

const XDG_CONFIG_HOME = Bun.env.XDG_CONFIG_HOME || join(homedir(), ".config");
const SETTINGS_DIR = join(XDG_CONFIG_HOME, "qbar");
const SETTINGS_FILE = join(SETTINGS_DIR, "settings.json");

export type WindowPolicy = "both" | "five_hour" | "seven_day";

export interface Settings {
  waybar: {
    providers: string[];
    showPercentage: boolean;
    separators:
      | "pill"
      | "underline"
      | "gap"
      | "pipe"
      | "dot"
      | "subtle"
      | "none";
    providerOrder: string[];
  };
  tooltip: Record<string, never>;
  /** Per-provider model visibility. Key = provider id, value = array of model names to show. Empty array = show all. */
  models?: Record<string, string[]>;
  /** Per-provider window visibility policy. */
  windowPolicy?: Record<string, WindowPolicy>;
}

const DEFAULT_SETTINGS: Settings = {
  waybar: {
    providers: ["claude", "codex", "amp"],
    showPercentage: true,
    separators: "pipe",
    providerOrder: ["claude", "codex", "amp"],
  },
  tooltip: {},
  models: {},
  windowPolicy: {
    codex: "both",
  },
};

function normalizeSettings(data: Partial<Settings> | undefined): Settings {
  const merged: Settings = {
    waybar: { ...DEFAULT_SETTINGS.waybar, ...data?.waybar },
    tooltip: { ...DEFAULT_SETTINGS.tooltip, ...data?.tooltip },
    models: { ...DEFAULT_SETTINGS.models, ...data?.models },
    windowPolicy: { ...DEFAULT_SETTINGS.windowPolicy, ...data?.windowPolicy },
  };

  const normalizedWaybar = normalizeProviderSelection(
    merged.waybar.providers,
    merged.waybar.providerOrder,
  );

  merged.waybar.providers = normalizedWaybar.providers;
  merged.waybar.providerOrder = normalizedWaybar.providerOrder;

  return merged;
}

function serializeSettings(settings: Settings): string {
  return JSON.stringify(settings);
}

export async function loadSettings(): Promise<Settings> {
  const file = Bun.file(SETTINGS_FILE);

  if (!(await file.exists())) {
    return normalizeSettings(undefined);
  }

  try {
    const data = await file.json();
    const normalized = normalizeSettings(data);

    if (serializeSettings(normalized) !== JSON.stringify(data)) {
      await saveSettings(normalized);
    }

    return normalized;
  } catch (err) {
    process.stderr.write(`[qbar] Settings parse error (using defaults): ${err}\n`);
    return normalizeSettings(undefined);
  }
}

export function loadSettingsSync(): Settings {
  try {
    if (!existsSync(SETTINGS_FILE)) {
      return normalizeSettings(undefined);
    }
    const data = JSON.parse(readFileSync(SETTINGS_FILE, "utf-8"));
    return normalizeSettings(data);
  } catch (err) {
    process.stderr.write(`[qbar] Settings sync read error (using defaults): ${err}\n`);
    return normalizeSettings(undefined);
  }
}

export async function saveSettings(settings: Settings): Promise<void> {
  await mkdir(SETTINGS_DIR, { recursive: true });
  await Bun.write(SETTINGS_FILE, JSON.stringify(normalizeSettings(settings), null, 2));
}

export function getSettingsPath(): string {
  return SETTINGS_FILE;
}
