import { getColorForPercent } from "../config";
import type {
  AllQuotas,
  ProviderQuota,
  QuotaWindow,
} from "../providers/types";
import {
  formatPercent,
  formatEta,
  formatResetTime,
  classifyWindow,
  normalizePlanLabel,
  codexModelsFromQuota,
  applyCodexModelFilter,
} from "./shared";
import { loadSettingsSync, type WindowPolicy } from "../settings";
import { ONE_DARK } from "../theme";

const C = {
  green: ONE_DARK.green,
  yellow: ONE_DARK.yellow,
  orange: ONE_DARK.orange,
  red: ONE_DARK.red,
  text: ONE_DARK.text,
  subtext: ONE_DARK.muted,
  muted: ONE_DARK.comment,
  lavender: ONE_DARK.textBright,
  teal: ONE_DARK.cyan,
  blue: ONE_DARK.blue,
  mauve: ONE_DARK.magenta,
  peach: ONE_DARK.orange,
  sapphire: ONE_DARK.brightBlue,
  pink: ONE_DARK.brightMagenta,
  sky: ONE_DARK.cyan,
} as const;

// Box drawing - BOLD characters
const B = {
  tl: "┏",
  bl: "┗",
  lt: "┣", // left tee for connecting labels
  h: "━",
  v: "┃",
  dot: "●",
  dotO: "○",
  diamond: "◆",
};

interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
}

const s = (color: string, text: string, bold = false) =>
  `<span foreground='${color}'${bold ? " weight='bold'" : ""}>${text}</span>`;

function pctColored(val: number | null): string {
  return s(getColorForPercent(val), formatPercent(val));
}

function bar(val: number | null): string {
  if (val === null) return s(C.muted, "░".repeat(20));
  const filled = Math.floor(val / 5);
  return (
    s(getColorForPercent(val), "█".repeat(filled)) +
    s(C.muted, "░".repeat(20 - filled))
  );
}

function indicator(val: number | null): string {
  if (val === null) return s(C.muted, B.dotO);
  if (val < 10) return s(C.red, B.dot);
  if (val < 30) return s(C.orange, B.dot);
  if (val < 60) return s(C.yellow, B.dot);
  return s(C.green, B.dot);
}

function codexModelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  v: string,
): string {
  const rem = window?.remaining ?? null;
  const nameS = s(C.lavender, name.padEnd(maxLen));
  const b = bar(rem);
  const pctS = s(getColorForPercent(rem), formatPercent(rem).padStart(4));
  const etaS = window?.resetsAt
    ? s(
        C.teal,
        `→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`,
      )
    : s(C.teal, "→ N/A");
  return (
    v + "  " + indicator(rem) + " " + nameS + " " + b + " " + pctS + " " + etaS
  );
}

// Section label with connecting line: ┣━ ◆ Label
const label = (text: string, color: string) =>
  s(color, B.lt + B.h) + " " + s(C.mauve, B.diamond + " " + text, true);

/**
 * Build Claude tooltip
 */
function buildClaudeTooltip(p: ProviderQuota): string {
  const lines: string[] = [];
  const v = s(C.peach, B.v);

  lines.push(
    s(C.peach, B.tl + B.h) +
      " " +
      s(C.peach, "Claude", true) +
      " " +
      s(C.peach, B.h.repeat(50)),
  );
  lines.push(v);

  if (p.error) {
    lines.push(v + "  " + s(C.red, `⚠️ ${p.error}`));
  } else {
    const maxLen = 20;

    if (p.primary) {
      lines.push(label("5-hour limit (shared)", C.peach));
      const name = s(C.lavender, "All Models".padEnd(maxLen));
      const b = bar(p.primary.remaining);
      const pctS = s(
        getColorForPercent(p.primary.remaining),
        formatPercent(p.primary.remaining).padStart(4),
      );
      const etaS = s(
        C.teal,
        `→ ${formatEta(p.primary.resetsAt, p.primary.remaining)} ${formatResetTime(p.primary.resetsAt, p.primary.remaining)}`,
      );
      lines.push(
        v +
          "  " +
          indicator(p.primary.remaining) +
          " " +
          name +
          " " +
          b +
          " " +
          pctS +
          " " +
          etaS,
      );
    }

    // Per-model weekly quotas (when API provides them)
    if (p.weeklyModels && Object.keys(p.weeklyModels).length > 0) {
      lines.push(v);
      lines.push(label("Weekly per model", C.peach));
      const entries = Object.entries(p.weeklyModels);
      const wMaxLen = Math.max(...entries.map(([name]) => name.length), 20);

      for (const [name, window] of entries) {
        const nameS = s(C.lavender, name.padEnd(wMaxLen));
        const b = bar(window.remaining);
        const pctS = s(
          getColorForPercent(window.remaining),
          formatPercent(window.remaining).padStart(4),
        );
        const etaS = s(
          C.teal,
          `→ ${formatEta(window.resetsAt, window.remaining)} ${formatResetTime(window.resetsAt, window.remaining)}`,
        );
        lines.push(
          v +
            "  " +
            indicator(window.remaining) +
            " " +
            nameS +
            " " +
            b +
            " " +
            pctS +
            " " +
            etaS,
        );
      }
    }

    // Generic weekly (shared)
    if (p.secondary) {
      lines.push(v);
      lines.push(label("Weekly limit (shared)", C.peach));
      const name = s(C.lavender, "All Models".padEnd(20));
      const b = bar(p.secondary.remaining);
      const pctS = s(
        getColorForPercent(p.secondary.remaining),
        formatPercent(p.secondary.remaining).padStart(4),
      );
      const etaS = s(
        C.teal,
        `→ ${formatEta(p.secondary.resetsAt, p.secondary.remaining)} ${formatResetTime(p.secondary.resetsAt, p.secondary.remaining)}`,
      );
      lines.push(
        v +
          "  " +
          indicator(p.secondary.remaining) +
          " " +
          name +
          " " +
          b +
          " " +
          pctS +
          " " +
          etaS,
      );
    }

    if (p.extraUsage?.enabled && p.extraUsage.limit > 0) {
      const { remaining, used, limit } = p.extraUsage;
      lines.push(v);
      lines.push(label("Extra Usage", C.peach));
      const name = s(C.lavender, "Budget".padEnd(20));
      const b = bar(remaining);
      const pctS = s(getColorForPercent(remaining), formatPercent(remaining).padStart(4));
      const usedS = s(
        C.teal,
        `$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}`,
      );
      lines.push(
        v +
          "  " +
          indicator(remaining) +
          " " +
          name +
          " " +
          b +
          " " +
          pctS +
          " " +
          usedS,
      );
    }
  }

  lines.push(v);
  lines.push(s(C.peach, B.bl + B.h.repeat(55)));

  return lines.join("\n");
}

/**
 * Build Codex tooltip
 */
function buildCodexTooltip(p: ProviderQuota): string {
  const lines: string[] = [];
  const v = s(C.green, B.v);
  const settings = loadSettingsSync();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? "both";
  const planLabel = normalizePlanLabel(p);

  lines.push(
    s(C.green, B.tl + B.h) +
      " " +
      s(C.green, "Codex", true) +
      " " +
      s(C.green, B.h.repeat(51)),
  );
  lines.push(v);

  if (p.error) {
    lines.push(v + "  " + s(C.red, `⚠️ ${p.error}`));
  } else {
    lines.push(v + "  " + s(C.subtext, `Plan: ${planLabel}`));

    let models = codexModelsFromQuota(p);
    models = applyCodexModelFilter(models, settings.models?.[p.provider]);

    if (models.length === 0) {
      lines.push(v);
      lines.push(label("Available Models", C.green));
      lines.push(v + "  " + s(C.muted, "No models selected"));
    } else {
      const maxLen = Math.max(...models.map((m) => m.name.length), 20);

      if (policy !== "seven_day") {
        lines.push(v);
        lines.push(label("5-hour limit", C.green));
        for (const model of models) {
          lines.push(
            codexModelLine(model.name, model.windows.fiveHour, maxLen, v),
          );
        }
      }

      if (policy !== "five_hour") {
        lines.push(v);
        lines.push(label("7-day limit", C.green));
        for (const model of models) {
          lines.push(
            codexModelLine(model.name, model.windows.sevenDay, maxLen, v),
          );
        }
      }
    }

    if (p.extraUsage?.enabled) {
      lines.push(v);
      lines.push(label("Credits", C.green));
      const name = s(C.lavender, "Balance".padEnd(20));
      const b = bar(p.extraUsage.remaining);
      const pctS = s(
        getColorForPercent(p.extraUsage.remaining),
        formatPercent(p.extraUsage.remaining).padStart(4),
      );
      const limitS =
        p.extraUsage.limit === -1
          ? s(C.teal, "Unlimited")
          : s(C.teal, "Balance");
      lines.push(
        v +
          "  " +
          indicator(p.extraUsage.remaining) +
          " " +
          name +
          " " +
          b +
          " " +
          pctS +
          " " +
          limitS,
      );
    }
  }

  lines.push(v);
  lines.push(s(C.green, B.bl + B.h.repeat(55)));

  return lines.join("\n");
}

/**
 * Build Amp tooltip
 */
function buildAmpTooltip(p: ProviderQuota): string {
  const lines: string[] = [];
  const v = s(C.mauve, B.v);
  const m = p.meta ?? {};
  const W = 40; // box width

  // Thin tree connectors
  const tee = s(C.muted, "├─"); // branch
  const end = s(C.muted, "└─"); // last branch
  const vt = s(C.muted, "│"); // thin vertical (continuation)

  lines.push(
    s(C.mauve, B.tl + B.h) +
      " " +
      s(C.mauve, "Amp", true) +
      " " +
      s(C.mauve, B.h.repeat(W)),
  );
  lines.push(v);

  if (p.error) {
    lines.push(v + "  " + s(C.red, `⚠️ ${p.error}`));
  } else {
    // --- Free Tier ---
    const free = p.models?.["Free Tier"];
    if (free) {
      const b = bar(free.remaining);
      const pctS = s(
        getColorForPercent(free.remaining),
        formatPercent(free.remaining).padStart(4),
      );
      lines.push(label("Free Tier", C.mauve));
      lines.push(v + "  " + indicator(free.remaining) + " " + b + " " + pctS);

      // Build sub-details as tree branches
      const subs: string[] = [];

      // Dollar + rate on one line
      const dollarParts: string[] = [];
      if (m.replenishRate) dollarParts.push(s(C.teal, m.replenishRate));
      const dollars = [m.freeRemaining, m.freeTotal]
        .filter(Boolean)
        .join(" / ");
      if (dollars) dollarParts.push(s(C.text, `( ${dollars} )`));
      if (m.bonus) dollarParts.push(s(C.teal, m.bonus));
      if (dollarParts.length > 0) subs.push(dollarParts.join("  "));

      // ETA to full
      if (free.resetsAt && free.remaining !== 100) {
        subs.push(
          s(
            C.teal,
            `Full in ${formatEta(free.resetsAt, free.remaining)}  ${formatResetTime(free.resetsAt, free.remaining)}`,
          ),
        );
      }

      // Render tree branches
      for (let i = 0; i < subs.length; i++) {
        const connector = i === subs.length - 1 ? end : tee;
        lines.push(v + "  " + connector + " " + subs[i]);
      }
    }

    // --- Credits ---
    const credits = p.models?.["Credits"];
    if (credits) {
      lines.push(v);
      const balance = m.creditsBalance ?? "$0";
      const color = credits.remaining > 0 ? C.green : C.muted;
      lines.push(label("Credits", C.mauve));
      lines.push(
        v + "  " + indicator(credits.remaining) + " " + s(color, balance),
      );
    }
  }

  if (p.account) {
    lines.push(v);
    lines.push(v + "  " + s(C.muted, `Account: ${p.account}`));
  }

  lines.push(v);
  lines.push(s(C.mauve, B.bl + B.h.repeat(W + 2)));

  return lines.join("\n");
}

function buildTooltip(quotas: AllQuotas): string {
  const sections: string[] = [];

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;

    switch (p.provider) {
      case "claude":
        sections.push(buildClaudeTooltip(p));
        break;
      case "codex":
        sections.push(buildCodexTooltip(p));
        break;
      case "amp":
        sections.push(buildAmpTooltip(p));
        break;
    }
  }

  return sections.join("\n\n");
}

function buildText(quotas: AllQuotas): string {
  const parts: string[] = [];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    const val = p.primary?.remaining ?? null;
    parts.push(pctColored(val));
  }

  if (parts.length === 0) return s(C.muted, "No Providers");
  return parts.join(" " + s(C.muted, "│") + " ");
}

function getClass(quotas: AllQuotas): string {
  const classes: string[] = ["qbar"];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    const val = p.primary?.remaining ?? 100;
    let status = "ok";
    if (val < 10) status = "critical";
    else if (val < 30) status = "warn";
    else if (val < 60) status = "low";
    classes.push(`${p.provider}-${status}`);
  }

  return classes.join(" ");
}

export function formatForWaybar(quotas: AllQuotas): WaybarOutput {
  return {
    text: buildText(quotas),
    tooltip: buildTooltip(quotas),
    class: getClass(quotas),
  };
}

export function outputWaybar(quotas: AllQuotas): void {
  console.log(JSON.stringify(formatForWaybar(quotas)));
}

export function formatProviderForWaybar(quota: ProviderQuota): WaybarOutput {
  // Check if disconnected (not available or has error)
  if (!quota.available || quota.error) {
    let tooltip = "";
    switch (quota.provider) {
      case "claude":
        tooltip = buildClaudeTooltip(quota);
        break;
      case "codex":
        tooltip = buildCodexTooltip(quota);
        break;
      case "amp":
        tooltip = buildAmpTooltip(quota);
        break;
    }

    return {
      text: `<span foreground='${C.red}'>󱘖</span>`,
      tooltip,
      class: `qbar-${quota.provider} disconnected`,
    };
  }

  const val = quota.primary?.remaining ?? null;
  let status = "ok";
  if (val !== null) {
    if (val < 10) status = "critical";
    else if (val < 30) status = "warn";
    else if (val < 60) status = "low";
  }

  let tooltip = "";
  switch (quota.provider) {
    case "claude":
      tooltip = buildClaudeTooltip(quota);
      break;
    case "codex":
      tooltip = buildCodexTooltip(quota);
      break;
    case "amp":
      tooltip = buildAmpTooltip(quota);
      break;
  }

  return {
    text: pctColored(val),
    tooltip,
    class: `qbar-${quota.provider} ${status}`,
  };
}
