import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import {
  APP_HIDDEN_CLASS,
  APP_NAME,
  LEGACY_TERMINAL_HELPER_NAME,
  LEGACY_WAYBAR_MODULE_PREFIX,
  LEGACY_WAYBAR_NAMESPACE,
  TERMINAL_HELPER_NAME,
  WAYBAR_MODULE_PREFIX,
  WAYBAR_NAMESPACE,
  WAYBAR_SELECTOR_PREFIX,
} from "./app-identity";
import { ONE_DARK } from "./theme";

export const WAYBAR_PROVIDERS = ["claude", "codex", "amp"] as const;
export type WaybarProviderId = (typeof WAYBAR_PROVIDERS)[number];

export interface InstallAssetsOptions {
  waybarDir: string;
  scriptsDir: string;
  repoRoot?: string;
}

export interface WaybarModuleExportOptions {
  appBin: string;
  terminalScript: string;
}

export interface WaybarModulesExport {
  providers: WaybarProviderId[];
  modules: Record<string, ReturnType<typeof moduleDefinition>>;
}

export interface WaybarCssExportOptions {
  iconsDir: string;
  providerOrder: WaybarProviderId[];
  separators:
    | "pill"
    | "gap"
    | "bare"
    | "glass"
    | "shadow"
    | "none";
}

const HOME = homedir();
const DEFAULT_REPO_ROOT = join(import.meta.dir, "..");
const SURFACE = ONE_DARK.overlay;

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

function moduleDefinition(
  provider: WaybarProviderId,
  appBin: string,
  terminalScript: string,
) {
  return {
    exec: `${appBin} --provider ${provider}`,
    "return-type": "json",
    interval: 120,
    "exec-on-event": true,
    tooltip: true,
    "on-click": `${terminalScript} ${appBin} menu`,
    "on-click-right": `${terminalScript} ${appBin} action-right ${provider}`,
  };
}

function separatorCss(
  providers: WaybarProviderId[],
  separatorStyle: WaybarCssExportOptions["separators"],
): string {
  if (providers.length === 0) {
    return "";
  }

  const providerSelectors = providers.map((provider) => `${WAYBAR_SELECTOR_PREFIX}${provider}`);
  const selectorBlock = providerSelectors.join(",\n");

  if (separatorStyle === "pill") {
    return [
      `/* ${APP_NAME} separators: pill */`,
      `${selectorBlock} {`,
      `  background-color: ${SURFACE};`,
      "  border-radius: 4px;",
      "}",
      "",
    ].join("\n");
  }

  if (separatorStyle === "gap") {
    return [
      `/* ${APP_NAME} separators: gap */`,
      `${selectorBlock} {`,
      "  border-color: transparent;",
      "}",
      "",
    ].join("\n");
  }

  if (separatorStyle === "bare") {
    return [
      `/* ${APP_NAME} separators: bare */`,
      `${selectorBlock} {`,
      "  border-color: transparent;",
      "  background-color: transparent;",
      "}",
      `${selectorBlock}:hover {`,
      "  background-color: transparent;",
      "  border-color: transparent;",
      "}",
      "",
    ].join("\n");
  }

  if (separatorStyle === "glass") {
    return [
      `/* ${APP_NAME} separators: glass */`,
      `${selectorBlock} {`,
      "  background-color: rgba(192, 201, 212, 0.04);",
      "  border-color: transparent;",
      "  border-radius: 4px;",
      "}",
      "",
    ].join("\n");
  }

  if (separatorStyle === "shadow") {
    return [
      `/* ${APP_NAME} separators: shadow */`,
      `${selectorBlock} {`,
      "  border-color: transparent;",
      "  border-radius: 4px;",
      "  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);",
      "}",
      "",
    ].join("\n");
  }

  return [
    `/* ${APP_NAME} separators: none */`,
    `${selectorBlock} {`,
    "  border-color: transparent;",
    "  margin: 0;",
    "}",
    "",
  ].join("\n");
}

export function getDefaultWaybarAssetPaths() {
  const waybarRoot = join(HOME, ".config", "waybar");

  return {
    waybarDir: join(waybarRoot, WAYBAR_NAMESPACE),
    scriptsDir: join(waybarRoot, "scripts"),
    iconsDir: join(waybarRoot, WAYBAR_NAMESPACE, "icons"),
    terminalScript: join(waybarRoot, "scripts", TERMINAL_HELPER_NAME),
    appBin: `$HOME/.local/bin/${APP_NAME}`,
  };
}

export function getLegacyWaybarAssetPaths(waybarRoot = join(HOME, ".config", "waybar")) {
  return {
    waybarDir: join(waybarRoot, LEGACY_WAYBAR_NAMESPACE),
    scriptsDir: join(waybarRoot, "scripts"),
    iconsDir: join(waybarRoot, LEGACY_WAYBAR_NAMESPACE, "icons"),
    terminalScript: join(waybarRoot, "scripts", LEGACY_TERMINAL_HELPER_NAME),
    appBin: `$HOME/.local/bin/${LEGACY_WAYBAR_NAMESPACE}`,
  };
}

export function cleanupLegacyWaybarAssets(waybarRoot: string): string[] {
  const legacy = getLegacyWaybarAssetPaths(waybarRoot);
  const removed: string[] = [];

  for (const path of [legacy.waybarDir, legacy.terminalScript]) {
    if (!existsSync(path)) {
      continue;
    }

    rmSync(path, { recursive: true, force: true });
    removed.push(path);
  }

  return removed;
}

export function normalizeProviderSelection(
  providers: string[],
  providerOrder: string[],
): { providers: WaybarProviderId[]; providerOrder: WaybarProviderId[] } {
  const enabled = providers.filter((provider): provider is WaybarProviderId =>
    WAYBAR_PROVIDERS.includes(provider as WaybarProviderId),
  );
  const dedupedEnabled = Array.from(new Set(enabled));

  const normalizedOrder = providerOrder.filter(
    (provider): provider is WaybarProviderId =>
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
    );
  }

  return { providers, modules };
}

export function exportWaybarCss(options: WaybarCssExportOptions): { css: string } {
  const iconRef = (name: string) => {
    const iconPath = join(options.iconsDir, name);
    return iconPath.startsWith("/") ? pathToFileURL(iconPath).toString() : iconPath;
  };

  const providerOrder =
    options.providerOrder.length > 0 ? options.providerOrder : [...WAYBAR_PROVIDERS];
  const allProviderSelectors = WAYBAR_PROVIDERS.map(
    (provider) => `${WAYBAR_SELECTOR_PREFIX}${provider}`,
  ).join(",\n");
  const stateSelectors = (state: string) =>
    WAYBAR_PROVIDERS.map((provider) => `${WAYBAR_SELECTOR_PREFIX}${provider}.${state}`).join(", ");
  const separators = separatorCss(providerOrder, options.separators);

  return {
    css: [
      `/* ${APP_NAME} waybar stylesheet */`,
      `${allProviderSelectors} {`,
      "  padding-left: 26px;",
      "  padding-right: 10px;",
      "  background-size: 14px 14px;",
      "  background-repeat: no-repeat;",
      "  background-position: 6px center;",
      "  border-left: 1px solid #434d5d;",
      `  color: ${ONE_DARK.text};`,
      "  transition: color 120ms ease, background-color 120ms ease;",
      "}",
      "",
      `${allProviderSelectors}:hover {`,
      "  background-color: rgba(192, 201, 212, 0.04);",
      "  border-color: #3c4656;",
      `  color: ${ONE_DARK.textBright};`,
      "}",
      "",
      `${WAYBAR_SELECTOR_PREFIX}claude { background-image: url("${iconRef("claude-code-icon.png")}"); }`,
      `${WAYBAR_SELECTOR_PREFIX}codex { background-image: url("${iconRef("codex-icon.png")}"); }`,
      `${WAYBAR_SELECTOR_PREFIX}amp { background-image: url("${iconRef("amp-icon.svg")}"); }`,
      "",
      `${stateSelectors("ok")} { color: ${ONE_DARK.green}; }`,
      `${stateSelectors("low")} { color: ${ONE_DARK.yellow}; }`,
      `${stateSelectors("warn")} { color: ${ONE_DARK.orange}; }`,
      `${stateSelectors("critical")} { color: ${ONE_DARK.red}; }`,
      `${stateSelectors("disconnected")} { color: ${ONE_DARK.red}; }`,
      `${stateSelectors(APP_HIDDEN_CLASS)} {`,
      "  min-width: 0;",
      "  padding: 0;",
      "  margin: 0;",
      "  border: 0;",
      "  background-image: none;",
      "}",
      "",
      separators,
    ].join("\n"),
  };
}

export function installWaybarAssets(options: InstallAssetsOptions): {
  iconsDir: string;
  terminalScript: string;
} {
  const repoRoot = options.repoRoot ?? DEFAULT_REPO_ROOT;
  const appDir = options.waybarDir;
  const iconsSource = join(repoRoot, "icons");
  const iconsDest = join(appDir, "icons");
  const scriptSource = join(repoRoot, "scripts", TERMINAL_HELPER_NAME);
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
