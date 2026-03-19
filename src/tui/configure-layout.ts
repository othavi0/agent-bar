import * as p from "@clack/prompts";
import { loadSettings, saveSettings } from "../settings";
import { providers } from "../providers";
import { PROVIDER_ANSI } from "../theme";
import { oneDark, colorize, semantic } from "./colors";
import { applyWaybarIntegration } from "../waybar-integration";
import { getAllQuotas } from "../providers";

const SEPARATOR_STYLES = [
  { value: "pill" as const, label: "Pill", preview: "[ 100% ] [ 65% ]" },
  { value: "gap" as const, label: "Gap", preview: "  100%     65%  " },
  { value: "bare" as const, label: "Bare", preview: "  100%   65%  (no bg, no hover)" },
  { value: "glass" as const, label: "Glass", preview: "  100%   65%  (translucent bg)" },
  { value: "shadow" as const, label: "Shadow", preview: "  100%   65%  (elevated)" },
  { value: "none" as const, label: "None", preview: "100%65% (compact)" },
];

const PROVIDER_NAMES: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  amp: "Amp",
};

const PROVIDER_COLORS: Record<string, string> = PROVIDER_ANSI;

function reloadWaybar(): void {
  try {
    Bun.spawn(["pkill", "-SIGUSR2", "waybar"], {
      stdout: "ignore",
      stderr: "ignore",
    });
  } catch { /* waybar may not be running */ }
}

export async function configureLayout(): Promise<boolean> {
  const settings = await loadSettings();

  // --- Step 1: Select which providers to show ---
  const availableProviders = await Promise.all(
    providers.map(async (provider) => ({
      id: provider.id,
      name: provider.name,
      available: await provider.isAvailable(),
    })),
  );

  p.note(
    [
      colorize("Space", semantic.highlight) + " toggle  " +
      colorize("Enter", semantic.highlight) + " confirm  " +
      colorize("q", semantic.highlight) + " back",
    ].join("\n"),
    colorize("Waybar Providers", semantic.title),
  );

  const providerResult = await p.multiselect({
    message: colorize("Select providers to show in Waybar", semantic.title),
    options: availableProviders.map((prov) => ({
      value: prov.id,
      label: prov.available
        ? colorize(prov.name, PROVIDER_COLORS[prov.id] ?? oneDark.text)
        : colorize(prov.name, oneDark.text) + colorize(" (not logged in)", semantic.muted),
      hint: prov.available ? undefined : "credentials not found",
    })),
    initialValues: settings.waybar.providers.filter((id) =>
      availableProviders.some((prov) => prov.id === id),
    ),
    required: false,
  });

  if (p.isCancel(providerResult)) return false;

  const selectedProviders = providerResult as string[];

  // --- Step 2: Provider order (only if 2+ selected) ---
  const currentOrder = settings.waybar.providerOrder.filter((id) =>
    selectedProviders.includes(id),
  );
  for (const id of selectedProviders) {
    if (!currentOrder.includes(id)) currentOrder.push(id);
  }
  let newOrder = currentOrder;

  if (selectedProviders.length >= 2) {
    const orderStr = currentOrder
      .map((id) =>
        colorize(
          PROVIDER_NAMES[id] ?? id,
          PROVIDER_COLORS[id] ?? oneDark.text,
        ),
      )
      .join(colorize(" → ", semantic.muted));

    const changeOrder = await p.confirm({
      message: colorize("Change display order?", semantic.title) +
        " " + colorize(`(${orderStr})`, semantic.muted),
      initialValue: false,
    });

    if (p.isCancel(changeOrder)) return false;

    if (changeOrder) {
      const ordered: string[] = [];
      let remaining = [...currentOrder];

      for (let i = 0; i < selectedProviders.length; i++) {
        if (remaining.length === 1) {
          ordered.push(remaining[0]);
          break;
        }

        const posLabel = i === 0 ? "1st (leftmost)" : i === 1 ? "2nd" : "3rd";

        const pick = await p.select({
          message: colorize(`${posLabel} provider`, semantic.title),
          options: remaining.map((id) => ({
            value: id,
            label: colorize(
              PROVIDER_NAMES[id] ?? id,
              PROVIDER_COLORS[id] ?? oneDark.text,
            ),
          })),
          initialValue: remaining[0],
        });

        if (p.isCancel(pick)) return false;

        const picked = pick as string;
        ordered.push(picked);
        remaining = remaining.filter((id) => id !== picked);
      }

      newOrder = ordered;
    }
  }

  // --- Step 3: Separator style ---
  const currentSep = settings.waybar.separators;

  const sepResult = await p.select({
    message: colorize("Separator style", semantic.title),
    options: SEPARATOR_STYLES.map((s) => ({
      value: s.value,
      label: colorize(
        s.label,
        s.value === currentSep ? oneDark.green : oneDark.text,
      ),
      hint: colorize(s.preview, semantic.muted),
    })),
    initialValue: currentSep,
  });

  if (p.isCancel(sepResult)) return false;

  const newSeparator = sepResult as typeof currentSep;

  // --- Apply ---
  const s = p.spinner();
  s.start("Applying changes...");

  settings.waybar.providers = selectedProviders;
  settings.waybar.providerOrder = newOrder;
  settings.waybar.separators = newSeparator;
  await saveSettings(settings);

  try {
    applyWaybarIntegration();
    s.message("Warming provider cache...");
    await getAllQuotas();
    reloadWaybar();
    s.message("Waiting for Waybar to reload...");
    await new Promise((r) => setTimeout(r, 2000));
    s.stop("Applied to Waybar");
  } catch {
    s.stop("Preferences saved");
    p.log.warn(
      colorize(
        "Could not sync Waybar automatically. Run `qbar apply-local` to update.",
        semantic.muted,
      ),
    );
  }

  // Show summary
  const orderSummary = newOrder
    .map((id) =>
      colorize(
        PROVIDER_NAMES[id] ?? id,
        PROVIDER_COLORS[id] ?? oneDark.text,
      ),
    )
    .join(colorize(" → ", semantic.muted));
  p.log.info(colorize("Providers:", semantic.subtitle) + " " + orderSummary);
  p.log.info(
    colorize("Style:", semantic.subtitle) +
      " " +
      colorize(newSeparator, oneDark.green),
  );
  p.log.info(
    colorize(
      "If changes didn't take effect, run `qbar apply-local` to refresh.",
      semantic.muted,
    ),
  );

  return true;
}
