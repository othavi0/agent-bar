import * as p from "@clack/prompts";
import { loadSettings, saveSettings } from "../settings";
import { catppuccin, colorize, semantic } from "./colors";

const SEPARATOR_STYLES = [
  { value: "pipe" as const, label: "Pipe", preview: "│ 0% │ 100% │ 65% │" },
  { value: "dot" as const, label: "Dotted", preview: "┊ 0% ┊ 100% ┊ 65% ┊" },
  { value: "subtle" as const, label: "Subtle", preview: "  0%   100%   65%  " },
  { value: "none" as const, label: "None", preview: "0%  100%  65%" },
];

const PROVIDER_NAMES: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  antigravity: "Antigravity",
  amp: "Amp",
};

const PROVIDER_COLORS: Record<string, string> = {
  claude: catppuccin.peach,
  codex: catppuccin.green,
  antigravity: catppuccin.blue,
  amp: catppuccin.mauve,
};

/**
 * Apply CSS changes to waybar style.css
 */
async function applyCSS(
  separatorStyle: string,
  providerOrder: string[],
): Promise<void> {
  const { existsSync, readFileSync, writeFileSync } = await import("node:fs");
  const { join } = await import("node:path");
  const { homedir } = await import("node:os");

  const HOME = homedir();
  const styleFile = join(HOME, ".config", "waybar", "style.css");

  if (!existsSync(styleFile)) return;

  // We need generateCSS from setup — but to avoid circular deps, inline a lightweight version
  const muted = "#6c7086";
  const surface0 = "#313244";
  const providers =
    providerOrder.length > 0
      ? providerOrder
      : ["claude", "codex", "antigravity", "amp"];
  const first = providers[0];
  const last = providers[providers.length - 1];

  let content = readFileSync(styleFile, "utf-8");

  // Remove existing separator rules
  content = content.replace(
    /\/\* Separator: outer borders \*\/[\s\S]*?(?=\n\/\*|\n#custom-qbar-claude\.ok|\n$)/m,
    "",
  );
  content = content.replace(
    /\/\* Separator: inter-module borders \*\/[\s\S]*?\}\s*/m,
    "",
  );

  // Remove any previously added separator-specific padding overrides
  content = content.replace(/#custom-qbar-\w+ \{ border-left:.*?\}\n?/g, "");
  content = content.replace(/#custom-qbar-\w+ \{ border-right:.*?\}\n?/g, "");

  // Add new separator rules before the status color rules
  if (separatorStyle !== "none") {
    const styles: Record<string, { border: string; outer: string }> = {
      pipe: { border: `1px solid ${muted}`, outer: `1px solid ${muted}` },
      dot: { border: `2px dashed ${muted}`, outer: `2px dashed ${muted}` },
      subtle: {
        border: `1px solid ${surface0}`,
        outer: `1px solid ${surface0}`,
      },
    };

    const style = styles[separatorStyle] ?? styles.pipe;

    const separatorCSS = `
/* Separator: outer borders */
#custom-qbar-${first} { border-left: ${style.outer}; margin-left: 4px; padding-left: 26px; }
#custom-qbar-${last} { border-right: ${style.outer}; margin-right: 4px; }

/* Separator: inter-module borders */
${providers
  .slice(1)
  .map((pr) => `#custom-qbar-${pr}`)
  .join(",\n")} {
  border-left: ${style.border};
}
`;

    // Insert before status colors
    const insertPoint = content.indexOf("#custom-qbar-claude.ok");
    if (insertPoint !== -1) {
      content =
        content.substring(0, insertPoint) +
        separatorCSS +
        "\n" +
        content.substring(insertPoint);
    } else {
      content = content.trimEnd() + "\n" + separatorCSS;
    }
  }

  // Clean up extra blank lines
  content = content.replace(/\n{3,}/g, "\n\n");

  writeFileSync(styleFile, content);
}

/**
 * Apply provider order to waybar config.jsonc
 */
async function applyModuleOrder(providerOrder: string[]): Promise<void> {
  const { existsSync, readFileSync, writeFileSync } = await import("node:fs");
  const { join } = await import("node:path");
  const { homedir } = await import("node:os");

  const HOME = homedir();
  const configFile = join(HOME, ".config", "waybar", "config.jsonc");

  if (!existsSync(configFile)) return;

  let content = readFileSync(configFile, "utf-8");

  // Find the modules-right array and reorder qbar modules within it
  const modulesMatch = content.match(/"modules-right"\s*:\s*\[([\s\S]*?)\]/);
  if (!modulesMatch) return;

  const modulesContent = modulesMatch[1];
  const moduleList = modulesContent
    .split(",")
    .map((m) => m.trim().replace(/"/g, ""));

  // Separate non-qbar and qbar modules
  const nonQbar = moduleList.filter((m) => !m.startsWith("custom/qbar-"));
  const qbarModules = providerOrder.map((p) => `custom/qbar-${p}`);

  // Rebuild: non-qbar first, then qbar in order
  const newModules = [...nonQbar, ...qbarModules];
  const newModulesStr = newModules.map((m) => `\n    "${m}"`).join(",");

  content = content.replace(
    /"modules-right"\s*:\s*\[[\s\S]*?\]/,
    `"modules-right": [${newModulesStr}]`,
  );

  writeFileSync(configFile, content);
}

export async function configureLayout(): Promise<boolean> {
  const settings = await loadSettings();

  // --- Provider Order ---
  p.note(
    [
      colorize("Current order:", semantic.subtitle),
      "",
      ...settings.waybar.providerOrder.map((id, i) => {
        const color = PROVIDER_COLORS[id] ?? catppuccin.text;
        const name = PROVIDER_NAMES[id] ?? id;
        return `  ${colorize(`${i + 1}.`, semantic.muted)} ${colorize(name, color)}`;
      }),
      "",
      colorize(
        "Use the multiselect below to set the new order.",
        semantic.muted,
      ),
      colorize("Select providers in the order you want them.", semantic.muted),
    ].join("\n"),
    colorize("Provider Order", semantic.title),
  );

  const orderResult = await p.multiselect({
    message: colorize(
      "Select providers in display order (first selected = leftmost)",
      semantic.title,
    ),
    options: settings.waybar.providerOrder.map((id) => ({
      value: id,
      label: colorize(
        PROVIDER_NAMES[id] ?? id,
        PROVIDER_COLORS[id] ?? catppuccin.text,
      ),
    })),
    initialValues: settings.waybar.providerOrder,
    required: true,
  });

  if (p.isCancel(orderResult)) return false;

  const newOrder = orderResult as string[];

  // --- Separator Style ---
  const currentSep = settings.waybar.separators;

  const sepResult = await p.select({
    message: colorize("Separator style", semantic.title),
    options: SEPARATOR_STYLES.map((s) => ({
      value: s.value,
      label: colorize(
        s.label,
        s.value === currentSep ? catppuccin.green : catppuccin.text,
      ),
      hint: colorize(s.preview, semantic.muted),
    })),
    initialValue: currentSep,
  });

  if (p.isCancel(sepResult)) return false;

  const newSeparator = sepResult as typeof currentSep;

  // --- Apply ---
  const spinner = p.spinner();
  spinner.start("Applying layout changes...");

  settings.waybar.providerOrder = newOrder;
  settings.waybar.separators = newSeparator;
  await saveSettings(settings);

  // Apply CSS changes
  await applyCSS(newSeparator, newOrder);

  // Apply module order
  await applyModuleOrder(newOrder);

  // Reload waybar
  try {
    Bun.spawn(["pkill", "-USR2", "waybar"]);
    await Bun.sleep(500);
  } catch {}

  spinner.stop(colorize("Layout updated, Waybar reloaded", semantic.good));

  // Show summary
  const orderStr = newOrder
    .map((id) =>
      colorize(
        PROVIDER_NAMES[id] ?? id,
        PROVIDER_COLORS[id] ?? catppuccin.text,
      ),
    )
    .join(colorize(" → ", semantic.muted));
  p.log.info(colorize("Order:", semantic.subtitle) + " " + orderStr);
  p.log.info(
    colorize("Separator:", semantic.subtitle) +
      " " +
      colorize(newSeparator, catppuccin.green),
  );

  return true;
}
