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
import { ONE_DARK } from "./theme";

export const WAYBAR_PROVIDERS = ["claude", "codex", "amp"] as const;
export type WaybarProviderId = (typeof WAYBAR_PROVIDERS)[number];

export interface InstallAssetsOptions {
  waybarDir: string;
  scriptsDir: string;
  repoRoot?: string;
}

export interface WaybarModuleExportOptions {
  qbarBin: string;
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
    | "underline"
    | "gap"
    | "pipe"
    | "dot"
    | "subtle"
    | "none";
}

const HOME = homedir();
const DEFAULT_REPO_ROOT = join(import.meta.dir, "..");
const QBAR_SURFACE = ONE_DARK.overlay;
const QBAR_MUTED = ONE_DARK.borderSoft;

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
  qbarBin: string,
  terminalScript: string,
) {
  return {
    exec: `${qbarBin} --provider ${provider}`,
    "return-type": "json",
    interval: 120,
    tooltip: true,
    "on-click": `${terminalScript} ${qbarBin} menu`,
    "on-click-right": `${terminalScript} ${qbarBin} action-right ${provider}`,
  };
}

function separatorCss(
  providers: WaybarProviderId[],
  separatorStyle: WaybarCssExportOptions["separators"],
): string {
  if (providers.length === 0) {
    return "";
  }

  const providerSelectors = providers.map((provider) => `#custom-qbar-${provider}`);

  if (separatorStyle === "pill") {
    return [
      "/* qbar separators: pill */",
      `${providerSelectors.join(",\n")} {`,
      `  background-color: ${QBAR_SURFACE};`,
      "  border-radius: 4px;",
      "}",
      "",
    ].join("\n");
  }

  if (separatorStyle === "underline") {
    return [
      "/* qbar separators: underline */",
      `${providerSelectors.join(",\n")} {`,
      `  border-bottom: 2px solid ${QBAR_MUTED};`,
      "  border-radius: 0;",
      "}",
      "",
    ].join("\n");
  }

  if (separatorStyle === "gap") {
    return [
      "/* qbar separators: gap */",
      `${providerSelectors.join(",\n")} {`,
      "  border-color: transparent;",
      "}",
      "",
    ].join("\n");
  }

  if (separatorStyle === "none") {
    return [
      "/* qbar separators: none */",
      `${providerSelectors.join(",\n")} {`,
      "  border-color: transparent;",
      "  margin: 0;",
      "}",
      "",
    ].join("\n");
  }

  const borderStyle =
    separatorStyle === "subtle"
      ? `1px solid ${QBAR_SURFACE}`
      : separatorStyle === "dot"
        ? `2px dashed ${QBAR_MUTED}`
        : `1px solid ${QBAR_MUTED}`;
  const lines = [
    `/* qbar separators: ${separatorStyle} */`,
    `#custom-qbar-${providers[0]} {`,
    `  border-left: ${borderStyle};`,
    "}",
  ];

  if (providers.length > 1) {
    lines.push(
      `${providers
        .slice(1)
        .map((provider) => `#custom-qbar-${provider}`)
        .join(",\n")} {`,
      `  border-left: ${borderStyle};`,
      "}",
    );
  }

  lines.push(
    `#custom-qbar-${providers[providers.length - 1]} {`,
    `  border-right: ${borderStyle};`,
    "}",
    "",
  );

  return lines.join("\n");
}

export function getDefaultWaybarAssetPaths() {
  const waybarDir = join(HOME, ".config", "waybar");

  return {
    waybarDir: join(waybarDir, "qbar"),
    scriptsDir: join(waybarDir, "scripts"),
    iconsDir: join(waybarDir, "qbar", "icons"),
    terminalScript: join(waybarDir, "scripts", "qbar-open-terminal"),
    qbarBin: "$HOME/.local/bin/qbar",
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
    modules[`custom/qbar-${provider}`] = moduleDefinition(
      provider,
      options.qbarBin,
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
    (provider) => `#custom-qbar-${provider}`,
  ).join(",\n");
  const stateSelectors = (state: string) =>
    WAYBAR_PROVIDERS.map((provider) => `#custom-qbar-${provider}.${state}`).join(", ");
  const separators = separatorCss(providerOrder, options.separators);

  return {
    css: [
      "/* qbar waybar stylesheet */",
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
      `#custom-qbar-claude { background-image: url("${iconRef("claude-code-icon.png")}"); }`,
      `#custom-qbar-codex { background-image: url("${iconRef("codex-icon.png")}"); }`,
      `#custom-qbar-amp { background-image: url("${iconRef("amp-icon.svg")}"); }`,
      "",
      `${stateSelectors("ok")} { color: ${ONE_DARK.green}; }`,
      `${stateSelectors("low")} { color: ${ONE_DARK.yellow}; }`,
      `${stateSelectors("warn")} { color: ${ONE_DARK.orange}; }`,
      `${stateSelectors("critical")} { color: ${ONE_DARK.red}; }`,
      `${stateSelectors("disconnected")} { color: ${ONE_DARK.red}; }`,
      `${stateSelectors("qbar-hidden")} {`,
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
  const qbarDir = options.waybarDir;
  const iconsSource = join(repoRoot, "icons");
  const iconsDest = join(qbarDir, "icons");
  const scriptSource = join(repoRoot, "scripts", "qbar-open-terminal");
  const scriptDest = join(options.scriptsDir, "qbar-open-terminal");

  if (!existsSync(iconsSource)) {
    throw new Error(`Icons folder not found: ${iconsSource}`);
  }

  if (!existsSync(scriptSource)) {
    throw new Error(`Terminal helper not found: ${scriptSource}`);
  }

  rmSync(iconsDest, { recursive: true, force: true });
  mkdirSync(qbarDir, { recursive: true });
  copyDir(iconsSource, iconsDest);

  mkdirSync(options.scriptsDir, { recursive: true });
  copyFileSync(scriptSource, scriptDest);
  chmodSync(scriptDest, 0o755);

  return {
    iconsDir: iconsDest,
    terminalScript: scriptDest,
  };
}
