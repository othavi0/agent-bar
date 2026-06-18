import { chmodSync, copyFileSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { isAbsolute, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import {
  APP_HIDDEN_CLASS,
  APP_NAME,
  TERMINAL_HELPER_NAME,
  WAYBAR_MODULE_PREFIX,
  WAYBAR_NAMESPACE,
  WAYBAR_SELECTOR_PREFIX,
} from './app-identity';
import { getRegisteredProviderIds } from './providers/registry';
import { isCompiledBinary } from './runtime';
import { ONE_DARK } from './theme';

export const WAYBAR_PROVIDERS = ['claude', 'codex', 'amp'] as const;
export type WaybarProviderId = (typeof WAYBAR_PROVIDERS)[number];

/**
 * Get all known provider IDs — built-in plus any dynamically registered.
 * Use this instead of WAYBAR_PROVIDERS when you need the full list.
 */
export function getAllProviderIds(): string[] {
  const builtIn = new Set<string>(WAYBAR_PROVIDERS);
  const registered = getRegisteredProviderIds();
  for (const id of registered) {
    builtIn.add(id);
  }
  return [...builtIn];
}

export interface InstallAssetsOptions {
  waybarDir: string;
  scriptsDir: string;
  repoRoot?: string;
}

export interface WaybarModuleExportOptions {
  appBin: string;
  terminalScript: string;
  /** Optional Waybar SIGRTMIN+N signal for on-demand refresh. Omitted from the module when absent. */
  signal?: number;
}

export interface WaybarModulesExport {
  providers: WaybarProviderId[];
  modules: Record<string, ReturnType<typeof moduleDefinition>>;
}

export interface WaybarCssExportOptions {
  iconsDir: string;
  providerOrder: WaybarProviderId[];
  separators: 'pill' | 'gap' | 'bare' | 'glass' | 'shadow' | 'none';
}

const HOME = homedir();
const DEFAULT_REPO_ROOT = join(import.meta.dir, '..');
const SURFACE = ONE_DARK.overlay;

/** System asset prefix for a packaged (AUR) install. */
const SYSTEM_ASSET_DIR = `/usr/share/${APP_NAME}`;

/**
 * Directory that holds the source `icons/` and `scripts/` to install into Waybar.
 *
 * - An explicit `AGENT_BAR_ASSET_DIR` override always wins, but must be an
 *   absolute path that contains `icons/` — an invalid override throws rather
 *   than being silently ignored.
 * - A compiled (system/AUR) binary reads from `/usr/share/<app>`; its
 *   `DEFAULT_REPO_ROOT` is a `$bunfs/..` virtual path, so it is never consulted.
 * - A source checkout / npm install reads from the repo root.
 */
export function resolveAssetSourceRoot(): string {
  const hasIcons = (dir: string) => existsSync(join(dir, 'icons'));

  const envDir = process.env.AGENT_BAR_ASSET_DIR;
  if (envDir) {
    if (!isAbsolute(envDir) || !hasIcons(envDir)) {
      throw new Error(`AGENT_BAR_ASSET_DIR must be an absolute path containing icons/ (got: ${envDir}).`);
    }
    return envDir;
  }

  if (isCompiledBinary()) {
    if (hasIcons(SYSTEM_ASSET_DIR)) return SYSTEM_ASSET_DIR;
    throw new Error(
      `Asset directory not found at ${SYSTEM_ASSET_DIR}. Reinstall the package, or set AGENT_BAR_ASSET_DIR.`,
    );
  }

  if (hasIcons(DEFAULT_REPO_ROOT)) return DEFAULT_REPO_ROOT;
  throw new Error('Asset directory not found. Run `agent-bar setup` from a checkout, or set AGENT_BAR_ASSET_DIR.');
}

function copyDir(src: string, dest: string): void {
  mkdirSync(dest, { recursive: true });

  for (const entry of readdirSync(src, { withFileTypes: true })) {
    const srcPath = join(src, entry.name);
    const destPath = join(dest, entry.name);

    if (entry.isDirectory()) {
      copyDir(srcPath, destPath);
      continue;
    }

    copyFileSync(srcPath, destPath);
  }
}

export interface WaybarModuleConfig {
  exec: string;
  'return-type': 'json';
  interval: number;
  'exec-on-event': boolean;
  tooltip: boolean;
  'on-click': string;
  'on-click-right': string;
  signal?: number;
}

function moduleDefinition(
  provider: WaybarProviderId,
  appBin: string,
  terminalScript: string,
  signal?: number,
): WaybarModuleConfig {
  const def: WaybarModuleConfig = {
    exec: `${appBin} --provider ${provider}`,
    'return-type': 'json',
    interval: 120,
    'exec-on-event': true,
    tooltip: true,
    'on-click': `${terminalScript} ${appBin} menu`,
    'on-click-right': `${terminalScript} ${appBin} action-right ${provider}`,
  };
  if (typeof signal === 'number') {
    def.signal = signal;
  }
  return def;
}

function separatorCss(providers: WaybarProviderId[], separatorStyle: WaybarCssExportOptions['separators']): string {
  if (providers.length === 0) {
    return '';
  }

  const providerSelectors = providers.map((provider) => `${WAYBAR_SELECTOR_PREFIX}${provider}`);
  const selectorBlock = providerSelectors.join(',\n');

  if (separatorStyle === 'pill') {
    return [
      `/* ${APP_NAME} separators: pill */`,
      `${selectorBlock} {`,
      `  background-color: ${SURFACE};`,
      '  border-radius: 4px;',
      '}',
      '',
    ].join('\n');
  }

  if (separatorStyle === 'gap') {
    return [`/* ${APP_NAME} separators: gap */`, `${selectorBlock} {`, '  border-color: transparent;', '}', ''].join(
      '\n',
    );
  }

  if (separatorStyle === 'bare') {
    return [
      `/* ${APP_NAME} separators: bare */`,
      `${selectorBlock} {`,
      '  border-color: transparent;',
      '  background-color: transparent;',
      '}',
      `${selectorBlock}:hover {`,
      '  background-color: transparent;',
      '  border-color: transparent;',
      '}',
      '',
    ].join('\n');
  }

  if (separatorStyle === 'glass') {
    return [
      `/* ${APP_NAME} separators: glass */`,
      `${selectorBlock} {`,
      '  background-color: rgba(192, 201, 212, 0.04);',
      '  border-color: transparent;',
      '  border-radius: 4px;',
      '}',
      '',
    ].join('\n');
  }

  if (separatorStyle === 'shadow') {
    return [
      `/* ${APP_NAME} separators: shadow */`,
      `${selectorBlock} {`,
      '  border-color: transparent;',
      '  border-radius: 4px;',
      '  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);',
      '}',
      '',
    ].join('\n');
  }

  return [
    `/* ${APP_NAME} separators: none */`,
    `${selectorBlock} {`,
    '  border-color: transparent;',
    '  margin: 0;',
    '}',
    '',
  ].join('\n');
}

export function getDefaultWaybarAssetPaths() {
  const waybarRoot = join(HOME, '.config', 'waybar');

  return {
    waybarDir: join(waybarRoot, WAYBAR_NAMESPACE),
    scriptsDir: join(waybarRoot, 'scripts'),
    iconsDir: join(waybarRoot, WAYBAR_NAMESPACE, 'icons'),
    terminalScript: join(waybarRoot, 'scripts', TERMINAL_HELPER_NAME),
    appBin: isCompiledBinary() ? APP_NAME : `$HOME/.local/bin/${APP_NAME}`,
  };
}

export function normalizeProviderSelection(
  providers: string[],
  providerOrder: string[],
): { providers: WaybarProviderId[]; providerOrder: WaybarProviderId[] } {
  const enabled = providers.filter((provider): provider is WaybarProviderId =>
    WAYBAR_PROVIDERS.includes(provider as WaybarProviderId),
  );
  const dedupedEnabled = Array.from(new Set(enabled));

  const normalizedOrder = providerOrder.filter((provider): provider is WaybarProviderId =>
    dedupedEnabled.includes(provider as WaybarProviderId),
  );

  for (const provider of dedupedEnabled) {
    if (!normalizedOrder.includes(provider)) {
      normalizedOrder.push(provider);
    }
  }

  return {
    providers: dedupedEnabled,
    providerOrder: normalizedOrder,
  };
}

export function exportWaybarModules(
  options: WaybarModuleExportOptions,
  providers: WaybarProviderId[],
): WaybarModulesExport {
  const modules: Record<string, ReturnType<typeof moduleDefinition>> = {};

  for (const provider of providers) {
    modules[`${WAYBAR_MODULE_PREFIX}${provider}`] = moduleDefinition(
      provider,
      options.appBin,
      options.terminalScript,
      options.signal,
    );
  }

  return { providers, modules };
}

export function exportWaybarCss(options: WaybarCssExportOptions): { css: string } {
  const iconRef = (name: string) => {
    const iconPath = join(options.iconsDir, name);
    return iconPath.startsWith('/') ? pathToFileURL(iconPath).toString() : iconPath;
  };

  const providerOrder = options.providerOrder.length > 0 ? options.providerOrder : [...WAYBAR_PROVIDERS];
  const allProviderSelectors = WAYBAR_PROVIDERS.map((provider) => `${WAYBAR_SELECTOR_PREFIX}${provider}`).join(',\n');
  const stateSelectors = (state: string) =>
    WAYBAR_PROVIDERS.map((provider) => `${WAYBAR_SELECTOR_PREFIX}${provider}.${state}`).join(', ');
  const separators = separatorCss(providerOrder, options.separators);

  return {
    css: [
      `/* ${APP_NAME} waybar stylesheet */`,
      `${allProviderSelectors} {`,
      '  padding-left: 26px;',
      '  padding-right: 10px;',
      '  background-size: 14px 14px;',
      '  background-repeat: no-repeat;',
      '  background-position: 6px center;',
      '  border-left: 1px solid #434d5d;',
      `  color: ${ONE_DARK.text};`,
      '  transition: color 120ms ease, background-color 120ms ease;',
      '}',
      '',
      `${allProviderSelectors}:hover {`,
      '  background-color: rgba(192, 201, 212, 0.04);',
      '  border-color: #3c4656;',
      `  color: ${ONE_DARK.textBright};`,
      '}',
      '',
      `${WAYBAR_SELECTOR_PREFIX}claude { background-image: url("${iconRef('claude-code-icon.png')}"); }`,
      `${WAYBAR_SELECTOR_PREFIX}codex { background-image: url("${iconRef('codex-icon.png')}"); }`,
      `${WAYBAR_SELECTOR_PREFIX}amp { background-image: url("${iconRef('amp-icon.svg')}"); }`,
      '',
      `${stateSelectors('ok')} { color: ${ONE_DARK.green}; }`,
      `${stateSelectors('low')} { color: ${ONE_DARK.yellow}; }`,
      `${stateSelectors('warn')} { color: ${ONE_DARK.orange}; }`,
      `${stateSelectors('critical')} { color: ${ONE_DARK.red}; }`,
      `${stateSelectors('disconnected')} { color: ${ONE_DARK.red}; }`,
      `${stateSelectors(APP_HIDDEN_CLASS)} {`,
      '  min-width: 0;',
      '  padding: 0;',
      '  margin: 0;',
      '  border: 0;',
      '  background-image: none;',
      '}',
      '',
      separators,
    ].join('\n'),
  };
}

export function installWaybarAssets(options: InstallAssetsOptions): {
  iconsDir: string;
  terminalScript: string;
} {
  const repoRoot = options.repoRoot ?? resolveAssetSourceRoot();
  const appDir = options.waybarDir;
  const iconsSource = join(repoRoot, 'icons');
  const iconsDest = join(appDir, 'icons');
  const scriptSource = join(repoRoot, 'scripts', TERMINAL_HELPER_NAME);
  const scriptDest = join(options.scriptsDir, TERMINAL_HELPER_NAME);

  if (!existsSync(iconsSource)) {
    throw new Error(`Icons folder not found: ${iconsSource}`);
  }

  if (!existsSync(scriptSource)) {
    throw new Error(`Terminal helper not found: ${scriptSource}`);
  }

  rmSync(iconsDest, { recursive: true, force: true });
  mkdirSync(appDir, { recursive: true });
  copyDir(iconsSource, iconsDest);

  mkdirSync(options.scriptsDir, { recursive: true });
  copyFileSync(scriptSource, scriptDest);
  chmodSync(scriptDest, 0o755);

  return {
    iconsDir: iconsDest,
    terminalScript: scriptDest,
  };
}
