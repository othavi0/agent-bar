import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import {
  APP_NAME,
  BACKUP_SUFFIX,
  LEGACY_APP_NAME,
  LEGACY_BACKUP_SUFFIX,
  LEGACY_WAYBAR_MODULE_PREFIX,
  LEGACY_WAYBAR_NAMESPACE,
  WAYBAR_MODULE_PREFIX,
  WAYBAR_NAMESPACE,
} from './app-identity';
import { loadSettingsSync } from './settings';
import {
  cleanupLegacyWaybarAssets,
  exportWaybarCss,
  exportWaybarModules,
  getDefaultWaybarAssetPaths,
  normalizeProviderSelection,
  WAYBAR_PROVIDERS,
  type WaybarProviderId,
} from './waybar-contract';

export interface WaybarIntegrationPaths {
  waybarConfigPath: string;
  waybarStylePath: string;
  modulesIncludePath: string;
  styleIncludePath: string;
}

export interface ApplyWaybarIntegrationOptions {
  paths?: WaybarIntegrationPaths;
  iconsDir?: string;
  appBin?: string;
  terminalScript?: string;
}

export interface ApplyWaybarIntegrationResult {
  configChanged: boolean;
  styleChanged: boolean;
  moduleIDs: string[];
  modulesIncludePath: string;
  styleIncludePath: string;
}

export interface RemoveWaybarIntegrationOptions {
  paths?: WaybarIntegrationPaths;
}

export interface RemoveWaybarIntegrationResult {
  configChanged: boolean;
  styleChanged: boolean;
  removedIncludes: string[];
}

export const APP_STYLE_IMPORT = `@import url("./${WAYBAR_NAMESPACE}/style.css");`;
export const LEGACY_STYLE_IMPORT = `@import url("./${LEGACY_WAYBAR_NAMESPACE}/style.css");`;

const MANAGED_MODULE_PREFIXES = [WAYBAR_MODULE_PREFIX, LEGACY_WAYBAR_MODULE_PREFIX];

function readText(path: string): string | null {
  if (!existsSync(path)) {
    return null;
  }

  return readFileSync(path, 'utf8');
}

function writeText(path: string, content: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content.endsWith('\n') ? content : `${content}\n`, 'utf8');
}

function backupIfNeeded(path: string): void {
  const backupPath = `${path}${BACKUP_SUFFIX}`;
  if (!existsSync(backupPath) && !existsSync(`${path}${LEGACY_BACKUP_SUFFIX}`) && existsSync(path)) {
    copyFileSync(path, backupPath);
  }
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function parseQuotedStrings(block: string): string[] {
  const values: string[] = [];
  const matches = block.matchAll(/"((?:\\.|[^"\\])*)"/g);
  for (const match of matches) {
    try {
      values.push(JSON.parse(`"${match[1]}"`) as string);
    } catch {}
  }
  return values;
}

function arraysEqual(left: string[], right: string[]): boolean {
  if (left.length !== right.length) {
    return false;
  }

  for (let i = 0; i < left.length; i += 1) {
    if (left[i] !== right[i]) {
      return false;
    }
  }

  return true;
}

function formatStringArray(values: string[], indent: string): string {
  if (values.length === 0) {
    return '[]';
  }

  const itemIndent = `${indent}  `;
  const lines = values.map((value) => `${itemIndent}${JSON.stringify(value)}`).join(',\n');
  return `[\n${lines}\n${indent}]`;
}

interface RewriteArrayResult {
  content: string;
  found: boolean;
  changed: boolean;
}

function rewriteStringArrayProperty(
  content: string,
  propertyName: string,
  transform: (values: string[]) => string[],
): RewriteArrayResult {
  const pattern = new RegExp(`("${escapeRegex(propertyName)}"\\s*:\\s*)\\[([\\s\\S]*?)\\]`, 'g');

  let found = false;
  let changed = false;

  const rewritten = content.replace(
    pattern,
    (full: string, prefix: string, body: string, offset: number, source: string): string => {
      found = true;
      const lineStart = source.lastIndexOf('\n', offset) + 1;
      const linePrefix = source.slice(lineStart, offset);
      const indentMatch = linePrefix.match(/^\s*/);
      const indent = indentMatch ? indentMatch[0] : '';

      const currentValues = parseQuotedStrings(body);
      const nextValues = transform(currentValues);

      if (arraysEqual(currentValues, nextValues)) {
        return full;
      }

      changed = true;
      return `${prefix}${formatStringArray(nextValues, indent)}`;
    },
  );

  return { content: rewritten, found, changed };
}

function insertPropertyIntoFirstObject(content: string, propertyText: string): string {
  const braceIndex = content.indexOf('{');
  if (braceIndex === -1) {
    throw new Error(`Waybar config must contain an object to insert ${APP_NAME} integration.`);
  }

  const afterBrace = content.slice(braceIndex + 1);
  const indentMatch = afterBrace.match(/\n(\s*)"/);
  const indent = indentMatch ? indentMatch[1] : '  ';
  const firstToken = afterBrace.trimStart();
  const objectIsEmpty = firstToken.startsWith('}');
  const insertion = objectIsEmpty ? `\n${indent}${propertyText}\n` : `\n${indent}${propertyText},`;

  return `${content.slice(0, braceIndex + 1)}${insertion}${afterBrace}`;
}

function getManagedWaybarRoot(paths: WaybarIntegrationPaths): string {
  return dirname(dirname(paths.modulesIncludePath));
}

function isManagedModule(value: string): boolean {
  return MANAGED_MODULE_PREFIXES.some((prefix) => value.startsWith(prefix));
}

function stripManagedStyleImports(content: string): string {
  return content
    .replace(new RegExp(`^\\s*\\/\\*\\s*${escapeRegex(APP_NAME)} managed import\\s*\\*\\/\\n?`, 'm'), '')
    .replace(new RegExp(`^\\s*\\/\\*\\s*${escapeRegex(LEGACY_APP_NAME)} managed import\\s*\\*\\/\\n?`, 'm'), '')
    .replace(
      new RegExp(`^\\s*@import\\s+url\\((['"])\\./${escapeRegex(WAYBAR_NAMESPACE)}/style\\.css\\1\\);?\\n?`, 'm'),
      '',
    )
    .replace(
      new RegExp(
        `^\\s*@import\\s+url\\((['"])\\./${escapeRegex(LEGACY_WAYBAR_NAMESPACE)}/style\\.css\\1\\);?\\n?`,
        'm',
      ),
      '',
    )
    .replace(/^\s*\n/, '');
}

function ensureIncludePath(
  content: string,
  includePath: string,
  legacyIncludePath: string,
): { content: string; changed: boolean } {
  const rewriteResult = rewriteStringArrayProperty(content, 'include', (values) => {
    const next = values.filter((value) => value !== legacyIncludePath);
    if (!next.includes(includePath)) {
      next.push(includePath);
    }
    return next;
  });

  if (rewriteResult.found) {
    return { content: rewriteResult.content, changed: rewriteResult.changed };
  }

  const includeProperty = `"include": ${formatStringArray([includePath], '  ')}`;
  return {
    content: insertPropertyIntoFirstObject(content, includeProperty),
    changed: true,
  };
}

function removeIncludePaths(content: string, includePaths: string[]): { content: string; changed: boolean } {
  const includeSet = new Set(includePaths);
  const rewriteResult = rewriteStringArrayProperty(content, 'include', (values) =>
    values.filter((value) => !includeSet.has(value)),
  );

  return { content: rewriteResult.content, changed: rewriteResult.changed };
}

function reconcileManagedModules(values: string[], moduleIDs: string[]): string[] {
  const next: string[] = [];
  let moduleIndex = 0;

  for (const value of values) {
    if (isManagedModule(value)) {
      if (moduleIndex < moduleIDs.length) {
        next.push(moduleIDs[moduleIndex]);
        moduleIndex += 1;
      }
      continue;
    }

    next.push(value);
  }

  while (moduleIndex < moduleIDs.length) {
    next.push(moduleIDs[moduleIndex]);
    moduleIndex += 1;
  }

  return next;
}

function ensureModulesRight(content: string, moduleIDs: string[]): { content: string; changed: boolean } {
  const rewriteResult = rewriteStringArrayProperty(content, 'modules-right', (values) =>
    reconcileManagedModules(values, moduleIDs),
  );

  if (rewriteResult.found) {
    return { content: rewriteResult.content, changed: rewriteResult.changed };
  }

  const modulesProperty = `"modules-right": ${formatStringArray(moduleIDs, '  ')}`;
  return {
    content: insertPropertyIntoFirstObject(content, modulesProperty),
    changed: true,
  };
}

function removeModulesRight(content: string): { content: string; changed: boolean } {
  const rewriteResult = rewriteStringArrayProperty(content, 'modules-right', (values) =>
    values.filter((value) => !isManagedModule(value)),
  );

  return { content: rewriteResult.content, changed: rewriteResult.changed };
}

function ensureStyleImport(content: string): { content: string; changed: boolean } {
  const stripped = stripManagedStyleImports(content);
  const next =
    stripped.length > 0
      ? `/* ${APP_NAME} managed import */\n${APP_STYLE_IMPORT}\n\n${stripped}`
      : `/* ${APP_NAME} managed import */\n${APP_STYLE_IMPORT}\n`;

  return { content: next, changed: next !== content };
}

function removeStyleImport(content: string): { content: string; changed: boolean } {
  const next = stripManagedStyleImports(content);
  return { content: next, changed: next !== content };
}

function buildBootstrapConfig(moduleIDs: string[], includePath: string): string {
  return JSON.stringify(
    {
      layer: 'top',
      position: 'top',
      'modules-left': [],
      'modules-center': [],
      'modules-right': moduleIDs,
      include: [includePath],
    },
    null,
    2,
  );
}

function resolveProviderOrder(): WaybarProviderId[] {
  const settings = loadSettingsSync();
  const normalized = normalizeProviderSelection(settings.waybar.providers, settings.waybar.providerOrder);

  if (normalized.providerOrder.length > 0) {
    return normalized.providerOrder;
  }

  if (normalized.providers.length > 0) {
    return normalized.providers;
  }

  return [...WAYBAR_PROVIDERS];
}

export function getDefaultWaybarIntegrationPaths(): WaybarIntegrationPaths {
  const waybarRoot = join(homedir(), '.config', 'waybar');
  return {
    waybarConfigPath: join(waybarRoot, 'config.jsonc'),
    waybarStylePath: join(waybarRoot, 'style.css'),
    modulesIncludePath: join(waybarRoot, WAYBAR_NAMESPACE, 'modules.jsonc'),
    styleIncludePath: join(waybarRoot, WAYBAR_NAMESPACE, 'style.css'),
  };
}

export function getLegacyWaybarIntegrationPaths(
  waybarRoot = join(homedir(), '.config', 'waybar'),
): WaybarIntegrationPaths {
  return {
    waybarConfigPath: join(waybarRoot, 'config.jsonc'),
    waybarStylePath: join(waybarRoot, 'style.css'),
    modulesIncludePath: join(waybarRoot, LEGACY_WAYBAR_NAMESPACE, 'modules.jsonc'),
    styleIncludePath: join(waybarRoot, LEGACY_WAYBAR_NAMESPACE, 'style.css'),
  };
}

export function getAppModuleIDs(order: WaybarProviderId[]): string[] {
  return order.map((provider) => `${WAYBAR_MODULE_PREFIX}${provider}`);
}

export function getLegacyModuleIDs(order: WaybarProviderId[]): string[] {
  return order.map((provider) => `${LEGACY_WAYBAR_MODULE_PREFIX}${provider}`);
}

export function applyWaybarIntegration(options: ApplyWaybarIntegrationOptions = {}): ApplyWaybarIntegrationResult {
  const paths = options.paths ?? getDefaultWaybarIntegrationPaths();
  const defaults = getDefaultWaybarAssetPaths();
  const legacyPaths = getLegacyWaybarIntegrationPaths(getManagedWaybarRoot(paths));

  const providerOrder = resolveProviderOrder();
  const moduleIDs = getAppModuleIDs(providerOrder);

  const modules = exportWaybarModules(
    {
      appBin: options.appBin ?? defaults.appBin,
      terminalScript: options.terminalScript ?? defaults.terminalScript,
    },
    providerOrder,
  ).modules;
  writeText(paths.modulesIncludePath, JSON.stringify(modules, null, 2));

  const settings = loadSettingsSync();
  const css = exportWaybarCss({
    iconsDir: options.iconsDir ?? defaults.iconsDir,
    providerOrder,
    separators: settings.waybar.separators,
  }).css;
  writeText(paths.styleIncludePath, css);

  const currentConfig = readText(paths.waybarConfigPath);
  let nextConfig: string;

  if (currentConfig === null) {
    nextConfig = buildBootstrapConfig(moduleIDs, paths.modulesIncludePath);
  } else {
    const includeResult = ensureIncludePath(currentConfig, paths.modulesIncludePath, legacyPaths.modulesIncludePath);
    const modulesResult = ensureModulesRight(includeResult.content, moduleIDs);
    nextConfig = modulesResult.content;
  }

  const configChanged = currentConfig !== nextConfig;
  if (configChanged) {
    backupIfNeeded(paths.waybarConfigPath);
    writeText(paths.waybarConfigPath, nextConfig);
  }

  const currentStyle = readText(paths.waybarStylePath);
  const styleResult = ensureStyleImport(currentStyle ?? '');
  if (styleResult.changed || currentStyle === null) {
    backupIfNeeded(paths.waybarStylePath);
    writeText(paths.waybarStylePath, styleResult.content);
  }

  for (const includePath of [legacyPaths.modulesIncludePath, legacyPaths.styleIncludePath]) {
    if (existsSync(includePath)) {
      rmSync(includePath, { force: true });
    }
  }
  cleanupLegacyWaybarAssets(getManagedWaybarRoot(paths));

  return {
    configChanged,
    styleChanged: styleResult.changed || currentStyle === null,
    moduleIDs,
    modulesIncludePath: paths.modulesIncludePath,
    styleIncludePath: paths.styleIncludePath,
  };
}

export function removeWaybarIntegration(options: RemoveWaybarIntegrationOptions = {}): RemoveWaybarIntegrationResult {
  const paths = options.paths ?? getDefaultWaybarIntegrationPaths();
  const legacyPaths = getLegacyWaybarIntegrationPaths(getManagedWaybarRoot(paths));

  const currentConfig = readText(paths.waybarConfigPath);
  let configChanged = false;

  if (currentConfig !== null) {
    const includeResult = removeIncludePaths(currentConfig, [paths.modulesIncludePath, legacyPaths.modulesIncludePath]);
    const modulesResult = removeModulesRight(includeResult.content);
    const nextConfig = modulesResult.content;
    configChanged = includeResult.changed || modulesResult.changed;
    if (configChanged) {
      writeText(paths.waybarConfigPath, nextConfig);
    }
  }

  const currentStyle = readText(paths.waybarStylePath);
  let styleChanged = false;
  if (currentStyle !== null) {
    const styleResult = removeStyleImport(currentStyle);
    styleChanged = styleResult.changed;
    if (styleChanged) {
      writeText(paths.waybarStylePath, styleResult.content);
    }
  }

  const removedIncludes: string[] = [];
  for (const path of [
    paths.modulesIncludePath,
    paths.styleIncludePath,
    legacyPaths.modulesIncludePath,
    legacyPaths.styleIncludePath,
  ]) {
    if (existsSync(path)) {
      rmSync(path, { force: true });
      removedIncludes.push(path);
    }
  }

  return {
    configChanged,
    styleChanged,
    removedIncludes,
  };
}
