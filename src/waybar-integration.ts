import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { APP_NAME, BACKUP_SUFFIX, WAYBAR_MODULE_PREFIX, WAYBAR_NAMESPACE } from './app-identity';
import { loadSettingsSync } from './settings';
import {
  exportWaybarCss,
  exportWaybarModules,
  getDefaultWaybarAssetPaths,
  normalizeProviderSelection,
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

const MANAGED_MODULE_PREFIXES = [WAYBAR_MODULE_PREFIX];

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
  if (!existsSync(backupPath) && existsSync(path)) {
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

/** Advance past a JSON string literal; `i` points at the opening quote. Returns the index just after the closing quote. */
function skipString(content: string, i: number): number {
  i += 1;
  while (i < content.length) {
    const c = content[i];
    if (c === '\\') {
      i += 2;
      continue;
    }
    if (c === '"') {
      return i + 1;
    }
    i += 1;
  }
  return i;
}

/**
 * Find the `]` that closes the `[` at `openIdx`, honoring nested brackets,
 * string literals, and JSONC comments. Returns -1 if unbalanced.
 *
 * A flat non-greedy regex (`\[([\s\S]*?)\]`) stops at the FIRST `]`, which
 * truncates and corrupts the config when an array element contains a nested
 * `]` (e.g. an inline object with its own array). This scanner is exact.
 */
function findMatchingBracket(content: string, openIdx: number): number {
  let depth = 0;
  let i = openIdx;
  while (i < content.length) {
    const c = content[i];
    if (c === '"') {
      i = skipString(content, i);
      continue;
    }
    if (c === '/' && content[i + 1] === '/') {
      const nl = content.indexOf('\n', i);
      i = nl === -1 ? content.length : nl;
      continue;
    }
    if (c === '/' && content[i + 1] === '*') {
      const end = content.indexOf('*/', i + 2);
      i = end === -1 ? content.length : end + 2;
      continue;
    }
    if (c === '[') {
      depth += 1;
    } else if (c === ']') {
      depth -= 1;
      if (depth === 0) {
        return i;
      }
    }
    i += 1;
  }
  return -1;
}

function rewriteStringArrayProperty(
  content: string,
  propertyName: string,
  transform: (values: string[]) => string[],
): RewriteArrayResult {
  const keyPattern = new RegExp(`"${escapeRegex(propertyName)}"\\s*:\\s*\\[`, 'g');

  let match: RegExpExecArray | null = keyPattern.exec(content);
  while (match !== null) {
    const matchStart = match.index;
    const lineStart = content.lastIndexOf('\n', matchStart) + 1;
    const linePrefix = content.slice(lineStart, matchStart);

    // Skip occurrences sitting inside a `//` line comment so commented-out
    // examples are never rewritten.
    if (linePrefix.includes('//')) {
      match = keyPattern.exec(content);
      continue;
    }

    const openIdx = matchStart + match[0].length - 1; // index of the `[`
    const closeIdx = findMatchingBracket(content, openIdx);
    if (closeIdx === -1) {
      match = keyPattern.exec(content);
      continue;
    }

    const body = content.slice(openIdx + 1, closeIdx);
    const currentValues = parseQuotedStrings(body);
    const nextValues = transform(currentValues);

    const indentMatch = linePrefix.match(/^\s*/);
    const indent = indentMatch ? indentMatch[0] : '';

    if (arraysEqual(currentValues, nextValues)) {
      return { content, found: true, changed: false };
    }

    const prefix = content.slice(matchStart, openIdx); // `"prop"\s*:\s*`
    const rewritten = `${content.slice(0, matchStart)}${prefix}${formatStringArray(nextValues, indent)}${content.slice(closeIdx + 1)}`;
    return { content: rewritten, found: true, changed: true };
  }

  return { content, found: false, changed: false };
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

function isManagedModule(value: string): boolean {
  return MANAGED_MODULE_PREFIXES.some((prefix) => value.startsWith(prefix));
}

function stripManagedStyleImports(content: string): string {
  return content
    .replace(new RegExp(`^\\s*\\/\\*\\s*${escapeRegex(APP_NAME)} managed import\\s*\\*\\/\\n?`, 'm'), '')
    .replace(
      new RegExp(`^\\s*@import\\s+url\\((['"])\\./${escapeRegex(WAYBAR_NAMESPACE)}/style\\.css\\1\\);?\\n?`, 'm'),
      '',
    )
    .replace(/^\s*\n/, '');
}

function ensureIncludePath(content: string, includePath: string): { content: string; changed: boolean } {
  const rewriteResult = rewriteStringArrayProperty(content, 'include', (values) => {
    const next = [...values];
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

  return normalized.providers;
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

export function getAppModuleIDs(order: WaybarProviderId[]): string[] {
  return order.map((provider) => `${WAYBAR_MODULE_PREFIX}${provider}`);
}

export function applyWaybarIntegration(options: ApplyWaybarIntegrationOptions = {}): ApplyWaybarIntegrationResult {
  const paths = options.paths ?? getDefaultWaybarIntegrationPaths();
  const defaults = getDefaultWaybarAssetPaths();

  const providerOrder = resolveProviderOrder();
  const moduleIDs = getAppModuleIDs(providerOrder);
  const settings = loadSettingsSync();

  const modules = exportWaybarModules(
    {
      appBin: options.appBin ?? defaults.appBin,
      terminalScript: options.terminalScript ?? defaults.terminalScript,
      signal: settings.waybar.signal,
    },
    providerOrder,
  ).modules;
  writeText(paths.modulesIncludePath, JSON.stringify(modules, null, 2));

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
    const includeResult = ensureIncludePath(currentConfig, paths.modulesIncludePath);
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

  const currentConfig = readText(paths.waybarConfigPath);
  let configChanged = false;

  if (currentConfig !== null) {
    const includeResult = removeIncludePaths(currentConfig, [paths.modulesIncludePath]);
    const modulesResult = removeModulesRight(includeResult.content);
    const nextConfig = modulesResult.content;
    configChanged = includeResult.changed || modulesResult.changed;
    if (configChanged) {
      // Back up before rewriting, mirroring applyWaybarIntegration — removal
      // mutates the user's config and must be recoverable.
      backupIfNeeded(paths.waybarConfigPath);
      writeText(paths.waybarConfigPath, nextConfig);
    }
  }

  const currentStyle = readText(paths.waybarStylePath);
  let styleChanged = false;
  if (currentStyle !== null) {
    const styleResult = removeStyleImport(currentStyle);
    styleChanged = styleResult.changed;
    if (styleChanged) {
      backupIfNeeded(paths.waybarStylePath);
      writeText(paths.waybarStylePath, styleResult.content);
    }
  }

  const removedIncludes: string[] = [];
  for (const path of [paths.modulesIncludePath, paths.styleIncludePath]) {
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
