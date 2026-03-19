import { mkdir, rename } from "fs/promises";
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "os";
import { join } from "path";
import { normalizeProviderSelection } from "./waybar-contract";

const XDG_CONFIG_HOME = Bun.env.XDG_CONFIG_HOME || join(homedir(), ".config");
const SETTINGS_DIR = join(XDG_CONFIG_HOME, "qbar");
const SETTINGS_FILE = join(SETTINGS_DIR, "settings.json");

export type WindowPolicy = "both" | "five_hour" | "seven_day";

const CURRENT_VERSION = 1;

const VALID_SEPARATORS = ["pill", "gap", "bare", "glass", "shadow", "none"] as const;
type SeparatorStyle = (typeof VALID_SEPARATORS)[number];

const VALID_WINDOW_POLICIES = ["both", "five_hour", "seven_day"] as const;

export interface Settings {
  version: number;
  waybar: {
    providers: string[];
    showPercentage: boolean;
    separators: SeparatorStyle;
    providerOrder: string[];
  };
  tooltip: Record<string, never>;
  /** Per-provider model visibility. Key = provider id, value = array of model names to show. Empty array = show all. */
  models?: Record<string, string[]>;
  /** Per-provider window visibility policy. */
  windowPolicy?: Record<string, WindowPolicy>;
}

const DEFAULT_SETTINGS: Settings = {
  version: CURRENT_VERSION,
  waybar: {
    providers: ["claude", "codex", "amp"],
    showPercentage: true,
    separators: "gap",
    providerOrder: ["claude", "codex", "amp"],
  },
  tooltip: {},
  models: {},
  windowPolicy: {
    codex: "both",
  },
};

/** Migrate settings from older schema versions. Currently a noop (v1 is the first version). */
function migrateSettings(data: Record<string, unknown>, _fromVersion: number): Record<string, unknown> {
  // Future migrations go here:
  // if (fromVersion < 2) { /* migrate v1 → v2 */ }
  return data;
}

function isValidSeparator(value: unknown): value is SeparatorStyle {
  return typeof value === "string" && (VALID_SEPARATORS as readonly string[]).includes(value);
}

function isValidWindowPolicy(value: unknown): value is WindowPolicy {
  return typeof value === "string" && (VALID_WINDOW_POLICIES as readonly string[]).includes(value);
}

function normalizeSettings(data: Partial<Settings> | undefined): Settings {
  // Handle version migration
  const version = (data as Record<string, unknown>)?.version;
  if (typeof version === "number" && version < CURRENT_VERSION) {
    data = migrateSettings(data as Record<string, unknown>, version) as Partial<Settings>;
  }

  const merged: Settings = {
    version: CURRENT_VERSION,
    waybar: { ...DEFAULT_SETTINGS.waybar, ...data?.waybar },
    tooltip: { ...DEFAULT_SETTINGS.tooltip, ...data?.tooltip },
    models: { ...DEFAULT_SETTINGS.models, ...data?.models },
    windowPolicy: { ...DEFAULT_SETTINGS.windowPolicy, ...data?.windowPolicy },
  };

  // Validate separators
  if (!isValidSeparator(merged.waybar.separators)) {
    merged.waybar.separators = DEFAULT_SETTINGS.waybar.separators;
  }

  // Validate window policies
  if (merged.windowPolicy) {
    for (const [key, value] of Object.entries(merged.windowPolicy)) {
      if (!isValidWindowPolicy(value)) {
        merged.windowPolicy[key] = "both";
      }
    }
  }

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
  const tmp = SETTINGS_FILE + ".tmp";
  await Bun.write(tmp, JSON.stringify(normalizeSettings(settings), null, 2));
  await rename(tmp, SETTINGS_FILE);
}

export function getSettingsPath(): string {
  return SETTINGS_FILE;
}
