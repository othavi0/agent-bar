import * as p from "@clack/prompts";
import { loadSettings, saveSettings } from "../settings";
import { PROVIDER_ANSI } from "../theme";
import { oneDark, colorize, semantic } from "./colors";

const SEPARATOR_STYLES = [
  { value: "pill" as const, label: "Pill", preview: "[ 100% ] [ 65% ]" },
  {
    value: "underline" as const,
    label: "Underline",
    preview: " 100%   65%\n ────   ───",
  },
  { value: "gap" as const, label: "Gap (Minimal)", preview: " 100%     65% " },
  { value: "pipe" as const, label: "Pipe (Legacy)", preview: "│ 100% │ 65% │" },
  {
    value: "dot" as const,
    label: "Dotted (Legacy)",
    preview: "┊ 100% ┊ 65% ┊",
  },
  {
    value: "subtle" as const,
    label: "Subtle (Legacy)",
    preview: "  100%   65%  ",
  },
  { value: "none" as const, label: "None", preview: "100%  65%" },
];

const PROVIDER_NAMES: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  amp: "Amp",
};

const PROVIDER_COLORS: Record<string, string> = PROVIDER_ANSI;

export async function configureLayout(): Promise<boolean> {
  const settings = await loadSettings();

  // --- Provider Order ---
  p.note(
    [
      colorize("Current order:", semantic.subtitle),
      "",
      ...settings.waybar.providerOrder.map((id, i) => {
        const color = PROVIDER_COLORS[id] ?? oneDark.text;
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
        PROVIDER_COLORS[id] ?? oneDark.text,
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
  s.start("Saving layout settings...");

  settings.waybar.providerOrder = newOrder;
  settings.waybar.separators = newSeparator;
  await saveSettings(settings);

  s.stop("Layout preferences saved");

  // Show summary
  const orderStr = newOrder
    .map((id) =>
      colorize(
        PROVIDER_NAMES[id] ?? id,
        PROVIDER_COLORS[id] ?? oneDark.text,
      ),
    )
    .join(colorize(" → ", semantic.muted));
  p.log.info(colorize("Order:", semantic.subtitle) + " " + orderStr);
  p.log.info(
    colorize("Separator:", semantic.subtitle) +
      " " +
      colorize(newSeparator, oneDark.green),
  );
  p.log.info(
    colorize("Apply:", semantic.subtitle) +
      " run `qbar apply-local` to sync Waybar now.",
  );

  return true;
}
