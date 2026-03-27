import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { loadSettingsSync } from "./settings";
import {
  WAYBAR_PROVIDERS,
  exportWaybarCss,
  exportWaybarModules,
  getDefaultWaybarAssetPaths,
  normalizeProviderSelection,
  type WaybarProviderId,
} from "./waybar-contract";

export interface WaybarIntegrationPaths {
  waybarConfigPath: string;
  waybarStylePath: string;
  modulesIncludePath: string;
  styleIncludePath: string;
}

export interface ApplyWaybarIntegrationOptions {
  paths?: WaybarIntegrationPaths;
  iconsDir?: string;
  qbarBin?: string;
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

export const QBAR_STYLE_IMPORT = '@import url("./qbar/style.css");';

function readText(path: string): string | null {
  if (!existsSync(path)) {
    return null;
  }

  return readFileSync(path, "utf8");
}

function writeText(path: string, content: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content.endsWith("\n") ? content : `${content}\n`, "utf8");
}

function backupIfNeeded(path: string): void {
  const backupPath = `${path}.qbar-backup`;
  if (!existsSync(backupPath) && existsSync(path)) {
    copyFileSync(path, backupPath);
  }
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function parseQuotedStrings(block: string): string[] {
  const values: string[] = [];
  const matches = block.matchAll(/"((?:\\.|[^"\\])*)"/g);
  for (const match of matches) {
    try {
      values.push(JSON.parse(`"${match[1]}"`) as string);
    } catch {
      continue;
    }
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
    return "[]";
  }

  const itemIndent = `${indent}  `;
  const lines = values.map((value) => `${itemIndent}${JSON.stringify(value)}`).join(",\n");
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
  const pattern = new RegExp(`("${escapeRegex(propertyName)}"\\s*:\\s*)\\[([\\s\\S]*?)\\]`, "g");

  let found = false;
  let changed = false;

  const rewritten = content.replace(
    pattern,
    (
      full: string,
      prefix: string,
      body: string,
      offset: number,
      source: string,
    ): string => {
      found = true;
      const lineStart = source.lastIndexOf("\n", offset) + 1;
      const linePrefix = source.slice(lineStart, offset);
      const indentMatch = linePrefix.match(/^\s*/);
      const indent = indentMatch ? indentMatch[0] : "";

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
  const braceIndex = content.indexOf("{");
  if (braceIndex === -1) {
    throw new Error("Waybar config must contain an object to insert qbar integration.");
  }

  const afterBrace = content.slice(braceIndex + 1);
  const indentMatch = afterBrace.match(/\n(\s*)"/);
  const indent = indentMatch ? indentMatch[1] : "  ";
  const firstToken = afterBrace.trimStart();
  const objectIsEmpty = firstToken.startsWith("}");
  const insertion = objectIsEmpty
    ? `\n${indent}${propertyText}\n`
    : `\n${indent}${propertyText},`;

  return `${content.slice(0, braceIndex + 1)}${insertion}${afterBrace}`;
}

function ensureIncludePath(content: string, includePath: string): { content: string; changed: boolean } {
  const rewriteResult = rewriteStringArrayProperty(content, "include", (values) => {
    if (values.includes(includePath)) {
      return values;
    }

    return [...values, includePath];
  });

  if (rewriteResult.found) {
    return { content: rewriteResult.content, changed: rewriteResult.changed };
  }

  const includeProperty = `"include": ${formatStringArray([includePath], "  ")}`;
  return {
    content: insertPropertyIntoFirstObject(content, includeProperty),
    changed: true,
  };
}

function removeIncludePath(content: string, includePath: string): { content: string; changed: boolean } {
  const rewriteResult = rewriteStringArrayProperty(content, "include", (values) =>
    values.filter((value) => value !== includePath),
  );

  return { content: rewriteResult.content, changed: rewriteResult.changed };
}

function ensureModulesRight(
  content: string,
  moduleIDs: string[],
): { content: string; changed: boolean } {
  const qbarPrefix = "custom/qbar-";

  const rewriteResult = rewriteStringArrayProperty(content, "modules-right", (values) => {
    // Add missing qbar modules
    const merged = [...values];
    for (const moduleID of moduleIDs) {
      if (!merged.includes(moduleID)) {
        merged.push(moduleID);
      }
    }

    // Reorder existing qbar modules to match desired order
    const qbarIndices = merged
      .map((v, i) => (v.startsWith(qbarPrefix) ? i : -1))
      .filter((i) => i !== -1);

    if (qbarIndices.length > 0) {
      const orderedQbar = moduleIDs.filter((id) => merged.includes(id));
      for (let i = 0; i < qbarIndices.length; i++) {
        merged[qbarIndices[i]] = orderedQbar[i];
      }
    }

    return merged;
  });

  if (rewriteResult.found) {
    return { content: rewriteResult.content, changed: rewriteResult.changed };
  }

  const modulesProperty = `"modules-right": ${formatStringArray(moduleIDs, "  ")}`;
  return {
    content: insertPropertyIntoFirstObject(content, modulesProperty),
    changed: true,
  };
}

function removeModulesRight(
  content: string,
  moduleIDs: string[],
): { content: string; changed: boolean } {
  const rewriteResult = rewriteStringArrayProperty(content, "modules-right", (values) =>
    values.filter((value) => !moduleIDs.includes(value)),
  );

  return { content: rewriteResult.content, changed: rewriteResult.changed };
}

function ensureStyleImport(content: string): { content: string; changed: boolean } {
  const styleImportPattern = /@import\s+url\((['"])\.\/qbar\/style\.css\1\);?/;
  if (styleImportPattern.test(content)) {
    return { content, changed: false };
  }

  return {
    content: `/* qbar managed import */\n${QBAR_STYLE_IMPORT}\n\n${content}`,
    changed: true,
  };
}

function removeStyleImport(content: string): { content: string; changed: boolean } {
  const next = content
    .replace(/^\s*\/\*\s*qbar managed import\s*\*\/\n?/m, "")
    .replace(/^\s*@import\s+url\((['"])\.\/qbar\/style\.css\1\);?\n?/m, "")
    .replace(/^\s*\n/, "");

  return { content: next, changed: next !== content };
}

function buildBootstrapConfig(moduleIDs: string[], includePath: string): string {
  return JSON.stringify(
    {
      layer: "top",
      position: "top",
      "modules-left": [],
      "modules-center": [],
      "modules-right": moduleIDs,
      include: [includePath],
    },
    null,
    2,
  );
}

function resolveProviderOrder(): WaybarProviderId[] {
  const settings = loadSettingsSync();
  const normalized = normalizeProviderSelection(
    settings.waybar.providers,
    settings.waybar.providerOrder,
  );

  if (normalized.providerOrder.length > 0) {
    return normalized.providerOrder;
  }

  if (normalized.providers.length > 0) {
    return normalized.providers;
  }

  return [...WAYBAR_PROVIDERS];
}

export function getDefaultWaybarIntegrationPaths(): WaybarIntegrationPaths {
  const waybarRoot = join(homedir(), ".config", "waybar");
  return {
    waybarConfigPath: join(waybarRoot, "config.jsonc"),
    waybarStylePath: join(waybarRoot, "style.css"),
    modulesIncludePath: join(waybarRoot, "qbar", "modules.jsonc"),
    styleIncludePath: join(waybarRoot, "qbar", "style.css"),
  };
}

export function getQbarModuleIDs(order: WaybarProviderId[]): string[] {
  return order.map((provider) => `custom/qbar-${provider}`);
}

export function applyWaybarIntegration(
  options: ApplyWaybarIntegrationOptions = {},
): ApplyWaybarIntegrationResult {
  const paths = options.paths ?? getDefaultWaybarIntegrationPaths();
  const defaults = getDefaultWaybarAssetPaths();

  const providerOrder = resolveProviderOrder();
  const moduleIDs = getQbarModuleIDs(providerOrder);

  const modules = exportWaybarModules(
    {
      qbarBin: options.qbarBin ?? defaults.qbarBin,
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
  const styleBase = currentStyle ?? "";
  const styleResult = ensureStyleImport(styleBase);
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

export function removeWaybarIntegration(
  options: RemoveWaybarIntegrationOptions = {},
): RemoveWaybarIntegrationResult {
  const paths = options.paths ?? getDefaultWaybarIntegrationPaths();
  const allModuleIDs = getQbarModuleIDs([...WAYBAR_PROVIDERS]);

  const currentConfig = readText(paths.waybarConfigPath);
  let configChanged = false;

  if (currentConfig !== null) {
    const includeResult = removeIncludePath(currentConfig, paths.modulesIncludePath);
    const modulesResult = removeModulesRight(includeResult.content, allModuleIDs);
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
