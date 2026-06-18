import { existsSync, readFileSync } from 'node:fs';
import { mkdir, rename } from 'fs/promises';
import { homedir } from 'os';
import { join } from 'path';
import { APP_NAME } from './app-identity';
import { normalizeProviderSelection } from './waybar-contract';

export type WindowPolicy = 'both' | 'five_hour' | 'seven_day';

const CURRENT_VERSION = 2;

const VALID_SEPARATORS = ['pill', 'gap', 'bare', 'glass', 'shadow', 'none'] as const;
type SeparatorStyle = (typeof VALID_SEPARATORS)[number];

const VALID_DISPLAY_MODES = ['remaining', 'used'] as const;
export type DisplayMode = (typeof VALID_DISPLAY_MODES)[number];

const VALID_WINDOW_POLICIES = ['both', 'five_hour', 'seven_day'] as const;

interface SettingsPaths {
  settingsDir: string;
  settingsFile: string;
}

function getSettingsPaths(): SettingsPaths {
  const xdgConfigHome = process.env.XDG_CONFIG_HOME ?? Bun.env.XDG_CONFIG_HOME ?? join(homedir(), '.config');
  const settingsDir = join(xdgConfigHome, APP_NAME);

  return {
    settingsDir,
    settingsFile: join(settingsDir, 'settings.json'),
  };
}

export interface Settings {
  version: number;
  waybar: {
    providers: string[];
    showPercentage: boolean;
    separators: SeparatorStyle;
    providerOrder: string[];
    displayMode: DisplayMode;
    /** Waybar SIGRTMIN+N signal number for on-demand refresh (1..30). Absent = disabled. */
    signal?: number;
  };
  tooltip: Record<string, never>;
  /** Per-provider model visibility. Key = provider id, value = array of model names to show. Empty array = show all. */
  models?: Record<string, string[]>;
  /** Per-provider window visibility policy. */
  windowPolicy?: Record<string, WindowPolicy>;
  /** Desktop notifications when a quota window crosses low (>=90% used) or critical (>=95% used). */
  notify?: { enabled: boolean };
}

const DEFAULT_SETTINGS: Settings = {
  version: CURRENT_VERSION,
  waybar: {
    providers: ['claude', 'codex', 'amp'],
    showPercentage: true,
    separators: 'gap',
    providerOrder: ['claude', 'codex', 'amp'],
    displayMode: 'remaining',
  },
  tooltip: {},
  models: {},
  windowPolicy: {
    codex: 'both',
  },
  notify: { enabled: true },
};

function isValidDisplayMode(value: unknown): value is DisplayMode {
  return typeof value === 'string' && (VALID_DISPLAY_MODES as readonly string[]).includes(value);
}

function isValidSeparator(value: unknown): value is SeparatorStyle {
  return typeof value === 'string' && (VALID_SEPARATORS as readonly string[]).includes(value);
}

function isValidWindowPolicy(value: unknown): value is WindowPolicy {
  return typeof value === 'string' && (VALID_WINDOW_POLICIES as readonly string[]).includes(value);
}

function isValidWaybarSignal(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 1 && value <= 30;
}

function normalizeSettings(data: Partial<Settings> | undefined): Settings {
  const merged: Settings = {
    version: CURRENT_VERSION,
    waybar: { ...DEFAULT_SETTINGS.waybar, ...data?.waybar },
    tooltip: { ...DEFAULT_SETTINGS.tooltip, ...data?.tooltip },
    models: { ...DEFAULT_SETTINGS.models, ...data?.models },
    windowPolicy: { ...DEFAULT_SETTINGS.windowPolicy, ...data?.windowPolicy },
    notify: { enabled: data?.notify?.enabled !== false },
  };

  // Validate separators
  if (!isValidSeparator(merged.waybar.separators)) {
    merged.waybar.separators = DEFAULT_SETTINGS.waybar.separators;
  }

  // Validate displayMode
  if (!isValidDisplayMode(merged.waybar.displayMode)) {
    merged.waybar.displayMode = DEFAULT_SETTINGS.waybar.displayMode;
  }

  // Validate optional Waybar refresh signal (drop invalid → disabled)
  if (merged.waybar.signal !== undefined && !isValidWaybarSignal(merged.waybar.signal)) {
    delete merged.waybar.signal;
  }

  // Validate window policies
  if (merged.windowPolicy) {
    for (const [key, value] of Object.entries(merged.windowPolicy)) {
      if (!isValidWindowPolicy(value)) {
        merged.windowPolicy[key] = 'both';
      }
    }
  }

  const normalizedWaybar = normalizeProviderSelection(merged.waybar.providers, merged.waybar.providerOrder);

  merged.waybar.providers = normalizedWaybar.providers;
  merged.waybar.providerOrder = normalizedWaybar.providerOrder;

  return merged;
}

function serializeSettings(settings: Settings): string {
  return JSON.stringify(settings);
}

export async function loadSettings(): Promise<Settings> {
  const { settingsFile } = getSettingsPaths();
  const file = Bun.file(settingsFile);

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
    process.stderr.write(`[${APP_NAME}] Settings parse error (using defaults): ${err}\n`);
    return normalizeSettings(undefined);
  }
}

export function loadSettingsSync(): Settings {
  const { settingsFile } = getSettingsPaths();
  try {
    if (!existsSync(settingsFile)) {
      return normalizeSettings(undefined);
    }
    const data = JSON.parse(readFileSync(settingsFile, 'utf-8'));
    return normalizeSettings(data);
  } catch (err) {
    process.stderr.write(`[${APP_NAME}] Settings sync read error (using defaults): ${err}\n`);
    return normalizeSettings(undefined);
  }
}

export async function saveSettings(settings: Settings): Promise<void> {
  const { settingsDir, settingsFile } = getSettingsPaths();
  await mkdir(settingsDir, { recursive: true });
  const tmp = `${settingsFile}.tmp`;
  await Bun.write(tmp, JSON.stringify(normalizeSettings(settings), null, 2));
  await rename(tmp, settingsFile);
}

export function getSettingsPath(): string {
  return getSettingsPaths().settingsFile;
}
