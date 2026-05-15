import { existsSync, mkdirSync, readFileSync, renameSync } from 'node:fs';
import { mkdir, rename } from 'fs/promises';
import { homedir } from 'os';
import { join } from 'path';
import { APP_NAME, LEGACY_APP_NAME } from './app-identity';
import { normalizeProviderSelection } from './waybar-contract';

export type WindowPolicy = 'both' | 'five_hour' | 'seven_day';

const CURRENT_VERSION = 2;

const VALID_SEPARATORS = ['pill', 'gap', 'bare', 'glass', 'shadow', 'none'] as const;
type SeparatorStyle = (typeof VALID_SEPARATORS)[number];

const VALID_DISPLAY_MODES = ['remaining', 'used'] as const;
export type DisplayMode = (typeof VALID_DISPLAY_MODES)[number];

const VALID_WINDOW_POLICIES = ['both', 'five_hour', 'seven_day'] as const;
const LEGACY_DEFAULT_PROVIDERS = ['claude', 'codex', 'amp'] as const;

interface SettingsPaths {
  settingsDir: string;
  settingsFile: string;
  legacySettingsDir: string;
  legacySettingsFile: string;
}

const attemptedSettingsMigrations = new Set<string>();

function getSettingsPaths(): SettingsPaths {
  const xdgConfigHome = process.env.XDG_CONFIG_HOME ?? Bun.env.XDG_CONFIG_HOME ?? join(homedir(), '.config');
  const settingsDir = join(xdgConfigHome, APP_NAME);
  const legacySettingsDir = join(xdgConfigHome, LEGACY_APP_NAME);

  return {
    settingsDir,
    settingsFile: join(settingsDir, 'settings.json'),
    legacySettingsDir,
    legacySettingsFile: join(legacySettingsDir, 'settings.json'),
  };
}

function migrateLegacySettingsSync(): void {
  const paths = getSettingsPaths();
  const migrationKey = `${paths.legacySettingsDir}->${paths.settingsDir}`;

  if (attemptedSettingsMigrations.has(migrationKey)) {
    return;
  }

  attemptedSettingsMigrations.add(migrationKey);
  if (!existsSync(paths.legacySettingsDir) || existsSync(paths.settingsDir)) {
    return;
  }

  try {
    mkdirSync(join(paths.settingsDir, '..'), { recursive: true });
    renameSync(paths.legacySettingsDir, paths.settingsDir);
  } catch (err) {
    const code = err instanceof Error && 'code' in err ? String((err as NodeJS.ErrnoException).code ?? '') : '';

    if (code === 'EROFS' || code === 'EPERM' || code === 'EACCES') {
      return;
    }

    process.stderr.write(`[${APP_NAME}] Settings migration skipped: ${String(err)}\n`);
  }
}

export interface Settings {
  version: number;
  waybar: {
    providers: string[];
    showPercentage: boolean;
    separators: SeparatorStyle;
    providerOrder: string[];
    displayMode: DisplayMode;
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
    providers: ['claude', 'codex', 'copilot', 'amp'],
    showPercentage: true,
    separators: 'gap',
    providerOrder: ['claude', 'codex', 'copilot', 'amp'],
    displayMode: 'remaining',
  },
  tooltip: {},
  models: {},
  windowPolicy: {
    codex: 'both',
  },
};

/** Migrate settings from older schema versions. */
function migrateSettings(data: Record<string, unknown>, fromVersion: number): Record<string, unknown> {
  if (fromVersion < 2) {
    // v1 → v2: Add Copilot to legacy default provider lists (one-shot migration).
    const waybar = data.waybar as Record<string, unknown> | undefined;
    if (waybar) {
      if (isExactStringArray(waybar.providers, LEGACY_DEFAULT_PROVIDERS)) {
        waybar.providers = withCopilotAfterCodex(waybar.providers as string[]);
      }
      if (waybar.providerOrder === undefined || isExactStringArray(waybar.providerOrder, LEGACY_DEFAULT_PROVIDERS)) {
        waybar.providerOrder = withCopilotAfterCodex((waybar.providerOrder ?? waybar.providers) as string[]);
      }
    }
  }
  return data;
}

function isValidDisplayMode(value: unknown): value is DisplayMode {
  return typeof value === 'string' && (VALID_DISPLAY_MODES as readonly string[]).includes(value);
}

function isValidSeparator(value: unknown): value is SeparatorStyle {
  return typeof value === 'string' && (VALID_SEPARATORS as readonly string[]).includes(value);
}

function isValidWindowPolicy(value: unknown): value is WindowPolicy {
  return typeof value === 'string' && (VALID_WINDOW_POLICIES as readonly string[]).includes(value);
}

function isExactStringArray(value: unknown, expected: readonly string[]): boolean {
  return (
    Array.isArray(value) && value.length === expected.length && value.every((item, index) => item === expected[index])
  );
}

function withCopilotAfterCodex(providers: string[]): string[] {
  if (providers.includes('copilot')) {
    return providers;
  }

  const next = [...providers];
  const codexIndex = next.indexOf('codex');
  next.splice(codexIndex >= 0 ? codexIndex + 1 : next.length, 0, 'copilot');
  return next;
}

function normalizeSettings(data: Partial<Settings> | undefined): Settings {
  // Handle version migration
  const version = (data as Record<string, unknown>)?.version;
  if (typeof version === 'number' && version < CURRENT_VERSION) {
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

  // Validate displayMode
  if (!isValidDisplayMode(merged.waybar.displayMode)) {
    merged.waybar.displayMode = DEFAULT_SETTINGS.waybar.displayMode;
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
  migrateLegacySettingsSync();

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
  migrateLegacySettingsSync();

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
  migrateLegacySettingsSync();

  const { settingsDir, settingsFile } = getSettingsPaths();
  await mkdir(settingsDir, { recursive: true });
  const tmp = `${settingsFile}.tmp`;
  await Bun.write(tmp, JSON.stringify(normalizeSettings(settings), null, 2));
  await rename(tmp, settingsFile);
}

export function getSettingsPath(): string {
  migrateLegacySettingsSync();
  return getSettingsPaths().settingsFile;
}
