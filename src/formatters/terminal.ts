import { CONFIG } from "../config";
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
import { ANSI } from "../theme";

const C = {
  reset: ANSI.reset,
  bold: ANSI.bold,
  green: ANSI.green,
  yellow: ANSI.yellow,
  orange: ANSI.orange,
  red: ANSI.red,
  muted: ANSI.comment,
  text: ANSI.text,
  subtext: ANSI.muted,
  lavender: ANSI.textBright,
  teal: ANSI.cyan,
  mauve: ANSI.magenta,
  blue: ANSI.blue,
  sapphire: ANSI.brightBlue,
  peach: ANSI.orange,
};

// Box drawing characters
const B = {
  tl: "┏",
  bl: "┗",
  lt: "┣",
  h: "━",
  v: "┃",
  dot: "●",
  dotO: "○",
  diamond: "◆",
};

function getColor(pct: number | null): string {
  if (pct === null) return C.text;
  if (pct >= CONFIG.thresholds.green) return C.green;
  if (pct >= CONFIG.thresholds.yellow) return C.yellow;
  if (pct >= CONFIG.thresholds.orange) return C.orange;
  return C.red;
}

function bar(pct: number | null): string {
  if (pct === null) return `${C.muted}${"░".repeat(20)}${C.reset}`;
  const filled = Math.floor(pct / 5);
  const color = getColor(pct);
  return `${color}${"█".repeat(filled)}${C.muted}${"░".repeat(20 - filled)}${C.reset}`;
}

function indicator(val: number | null): string {
  if (val === null) return `${C.muted}${B.dotO}${C.reset}`;
  const color = getColor(val);
  return `${color}${B.dot}${C.reset}`;
}

// Vertical bar with provider color
const v = (color: string) => `${color}${B.v}${C.reset}`;

// Section label: ┣━ ◆ Label
const label = (text: string, color: string) =>
  `${color}${B.lt}${B.h}${C.reset} ${C.mauve}${C.bold}${B.diamond} ${text}${C.reset}`;

// Model line
function modelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  vColor: string,
): string {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const nameS = `${C.lavender}${name.padEnd(maxLen)}${C.reset}`;
  const barS = bar(rem);
  const pctS = `${getColor(rem)}${formatPercent(rem).padStart(4)}${C.reset}`;
  const etaS = `${C.teal}→ ${formatEta(reset, rem)} ${formatResetTime(reset, rem)}${C.reset}`;
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function codexModelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  vColor: string,
): string {
  const rem = window?.remaining ?? null;
  const nameS = `${C.lavender}${name.padEnd(maxLen)}${C.reset}`;
  const barS = bar(rem);
  const pctS = `${getColor(rem)}${formatPercent(rem).padStart(4)}${C.reset}`;
  const etaS = window?.resetsAt
    ? `${C.teal}→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}${C.reset}`
    : `${C.teal}→ N/A${C.reset}`;
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function buildClaude(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = C.peach;

  lines.push(
    `${vc}${B.tl}${B.h}${C.reset} ${vc}${C.bold}Claude${C.reset} ${vc}${B.h.repeat(50)}${C.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${C.red}⚠️ ${p.error}${C.reset}`);
  } else {
    const maxLen = 20;

    if (p.primary) {
      lines.push(label("5-hour limit (shared)", vc));
      lines.push(modelLine("All Models", p.primary, maxLen, vc));
    }

    // Per-model weekly quotas (when API provides them)
    if (p.weeklyModels && Object.keys(p.weeklyModels).length > 0) {
      lines.push(v(vc));
      lines.push(label("Weekly per model", vc));
      const entries = Object.entries(p.weeklyModels);
      const maxLenWeekly = Math.max(
        ...entries.map(([name]) => name.length),
        maxLen,
      );
      for (const [name, window] of entries) {
        lines.push(modelLine(name, window, maxLenWeekly, vc));
      }
    }

    // Generic weekly (shared)
    if (p.secondary) {
      lines.push(v(vc));
      lines.push(label("Weekly limit (shared)", vc));
      lines.push(modelLine("All Models", p.secondary, maxLen, vc));
    }

    if (p.extraUsage?.enabled && p.extraUsage.limit > 0) {
      const { remaining, used, limit } = p.extraUsage;
      lines.push(v(vc));
      lines.push(label("Extra Usage", vc));
      const nameS = `${C.lavender}${"Budget".padEnd(maxLen)}${C.reset}`;
      const barS = bar(remaining);
      const pctS = `${getColor(remaining)}${formatPercent(remaining).padStart(4)}${C.reset}`;
      const usedS = `${C.teal}$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}${C.reset}`;
      lines.push(
        `${v(vc)}  ${indicator(remaining)} ${nameS} ${barS} ${pctS} ${usedS}`,
      );
    }
  }

  lines.push(v(vc));
  lines.push(`${vc}${B.bl}${B.h.repeat(55)}${C.reset}`);

  return lines;
}

function buildCodex(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = C.green;
  const settings = loadSettingsSync();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? "both";
  const planLabel = normalizePlanLabel(p);

  lines.push(
    `${vc}${B.tl}${B.h}${C.reset} ${vc}${C.bold}Codex${C.reset} ${vc}${B.h.repeat(51)}${C.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${C.red}⚠️ ${p.error}${C.reset}`);
  } else {
    const maxLen = 20;
    lines.push(`${v(vc)}  ${C.subtext}Plan: ${planLabel}${C.reset}`);

    let models = codexModelsFromQuota(p);
    models = applyCodexModelFilter(models, settings.models?.[p.provider]);

    if (models.length === 0) {
      lines.push(v(vc));
      lines.push(label("Available Models", vc));
      lines.push(`${v(vc)}  ${C.muted}No models selected${C.reset}`);
    } else {
      const modelLen = Math.max(...models.map((m) => m.name.length), maxLen);

      if (policy !== "seven_day") {
        lines.push(v(vc));
        lines.push(label("5-hour limit", vc));
        for (const model of models) {
          lines.push(
            codexModelLine(model.name, model.windows.fiveHour, modelLen, vc),
          );
        }
      }

      if (policy !== "five_hour") {
        lines.push(v(vc));
        lines.push(label("7-day limit", vc));
        for (const model of models) {
          lines.push(
            codexModelLine(model.name, model.windows.sevenDay, modelLen, vc),
          );
        }
      }
    }

    if (p.extraUsage?.enabled) {
      lines.push(v(vc));
      lines.push(label("Credits", vc));
      const nameS = `${C.lavender}${"Balance".padEnd(maxLen)}${C.reset}`;
      const barS = bar(p.extraUsage.remaining);
      const pctS = `${getColor(p.extraUsage.remaining)}${formatPercent(p.extraUsage.remaining).padStart(4)}${C.reset}`;
      const infoS =
        p.extraUsage.limit === -1
          ? `${C.teal}Unlimited${C.reset}`
          : `${C.teal}Balance${C.reset}`;
      lines.push(
        `${v(vc)}  ${indicator(p.extraUsage.remaining)} ${nameS} ${barS} ${pctS} ${infoS}`,
      );
    }
  }

  lines.push(v(vc));
  lines.push(`${vc}${B.bl}${B.h.repeat(55)}${C.reset}`);

  return lines;
}

function buildAmp(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = C.mauve;
  const m = p.meta ?? {};

  lines.push(
    `${vc}${B.tl}${B.h}${C.reset} ${vc}${C.bold}Amp${C.reset} ${vc}${B.h.repeat(53)}${C.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${C.red}⚠️ ${p.error}${C.reset}`);
  } else {
    // Thin tree connectors
    const tee = `${C.muted}├─${C.reset}`;
    const end = `${C.muted}└─${C.reset}`;

    // Free Tier
    const free = p.models?.["Free Tier"];
    if (free) {
      lines.push(label("Free Tier", vc));
      const barS = bar(free.remaining);
      const pctS = `${getColor(free.remaining)}${formatPercent(free.remaining).padStart(4)}${C.reset}`;
      lines.push(`${v(vc)}  ${indicator(free.remaining)} ${barS} ${pctS}`);

      // Build sub-details
      const subs: string[] = [];

      const dollarParts: string[] = [];
      if (m.replenishRate)
        dollarParts.push(`${C.teal}${m.replenishRate}${C.reset}`);
      const dollars = [m.freeRemaining, m.freeTotal]
        .filter(Boolean)
        .join(" / ");
      if (dollars) dollarParts.push(`${C.text}( ${dollars} )${C.reset}`);
      if (m.bonus) dollarParts.push(`${C.teal}${m.bonus}${C.reset}`);
      if (dollarParts.length > 0) subs.push(dollarParts.join("  "));

      if (free.resetsAt && free.remaining !== 100) {
        subs.push(
          `${C.teal}Full in ${formatEta(free.resetsAt, free.remaining)}  ${formatResetTime(free.resetsAt, free.remaining)}${C.reset}`,
        );
      }

      for (let i = 0; i < subs.length; i++) {
        const conn = i === subs.length - 1 ? end : tee;
        lines.push(`${v(vc)}  ${conn} ${subs[i]}`);
      }
    }

    // Credits
    const credits = p.models?.["Credits"];
    if (credits) {
      lines.push(v(vc));
      lines.push(label("Credits", vc));
      const balance = m.creditsBalance ?? "$0";
      const color = credits.remaining > 0 ? C.green : C.muted;
      lines.push(
        `${v(vc)}  ${indicator(credits.remaining)} ${color}${balance}${C.reset}`,
      );
    }

    // Fallback for unknown models
    if (!free && !credits && p.models && Object.keys(p.models).length > 0) {
      const entries = Object.entries(p.models);
      const maxLen = Math.max(...entries.map(([name]) => name.length), 20);
      lines.push(label("Usage", vc));
      for (const [name, window] of entries) {
        const nameS = `${C.lavender}${name.padEnd(maxLen)}${C.reset}`;
        const barS = bar(window.remaining);
        const pctS = `${getColor(window.remaining)}${formatPercent(window.remaining).padStart(4)}${C.reset}`;
        lines.push(
          `${v(vc)}  ${indicator(window.remaining)} ${nameS} ${barS} ${pctS}`,
        );
      }
    }
  }

  if (p.account) {
    lines.push(v(vc));
    lines.push(`${v(vc)}  ${C.muted}Account: ${p.account}${C.reset}`);
  }

  lines.push(v(vc));
  lines.push(`${vc}${B.bl}${B.h.repeat(55)}${C.reset}`);

  return lines;
}

export function formatForTerminal(quotas: AllQuotas): string {
  const sections: string[][] = [];

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;

    switch (p.provider) {
      case "claude":
        sections.push(buildClaude(p));
        break;
      case "codex":
        sections.push(buildCodex(p));
        break;
      case "amp":
        sections.push(buildAmp(p));
        break;
    }
  }

  if (sections.length === 0) {
    return `${C.muted}No providers connected${C.reset}`;
  }

  return sections.map((s) => s.join("\n")).join("\n\n");
}

export function outputTerminal(quotas: AllQuotas): void {
  console.log(formatForTerminal(quotas));
}
