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
import { ANSI, BOX, PROVIDER_ANSI } from "../theme";

function getColor(pct: number | null): string {
  if (pct === null) return ANSI.text;
  if (pct >= CONFIG.thresholds.green) return ANSI.green;
  if (pct >= CONFIG.thresholds.yellow) return ANSI.yellow;
  if (pct >= CONFIG.thresholds.orange) return ANSI.orange;
  return ANSI.red;
}

function bar(pct: number | null): string {
  if (pct === null) return `${ANSI.comment}${"░".repeat(20)}${ANSI.reset}`;
  const filled = Math.floor(pct / 5);
  const color = getColor(pct);
  return `${color}${"█".repeat(filled)}${ANSI.comment}${"░".repeat(20 - filled)}${ANSI.reset}`;
}

function indicator(val: number | null): string {
  if (val === null) return `${ANSI.comment}${BOX.dotO}${ANSI.reset}`;
  const color = getColor(val);
  return `${color}${BOX.dot}${ANSI.reset}`;
}

// Vertical bar with provider color
const v = (color: string) => `${color}${BOX.v}${ANSI.reset}`;

// Section label: ┣━ ◆ Label
const label = (text: string, color: string) =>
  `${color}${BOX.lt}${BOX.h}${ANSI.reset} ${ANSI.magenta}${ANSI.bold}${BOX.diamond} ${text}${ANSI.reset}`;

// Model line
function modelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  vColor: string,
): string {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
  const barS = bar(rem);
  const pctS = `${getColor(rem)}${formatPercent(rem).padStart(4)}${ANSI.reset}`;
  const etaS = `${ANSI.cyan}→ ${formatEta(reset, rem)} ${formatResetTime(reset, rem)}${ANSI.reset}`;
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function codexModelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  vColor: string,
): string {
  const rem = window?.remaining ?? null;
  const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
  const barS = bar(rem);
  const pctS = `${getColor(rem)}${formatPercent(rem).padStart(4)}${ANSI.reset}`;
  const etaS = window?.resetsAt
    ? `${ANSI.cyan}→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}${ANSI.reset}`
    : `${ANSI.cyan}→ N/A${ANSI.reset}`;
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function buildClaude(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = PROVIDER_ANSI.claude;

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}Claude${ANSI.reset} ${vc}${BOX.h.repeat(50)}${ANSI.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${ANSI.red}⚠️ ${p.error}${ANSI.reset}`);
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
      const nameS = `${ANSI.textBright}${"Budget".padEnd(maxLen)}${ANSI.reset}`;
      const barS = bar(remaining);
      const pctS = `${getColor(remaining)}${formatPercent(remaining).padStart(4)}${ANSI.reset}`;
      const usedS = `${ANSI.cyan}$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}${ANSI.reset}`;
      lines.push(
        `${v(vc)}  ${indicator(remaining)} ${nameS} ${barS} ${pctS} ${usedS}`,
      );
    }
  }

  lines.push(v(vc));
  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);

  return lines;
}

function buildCodex(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = PROVIDER_ANSI.codex;
  const settings = loadSettingsSync();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? "both";
  const planLabel = normalizePlanLabel(p);

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}Codex${ANSI.reset} ${vc}${BOX.h.repeat(51)}${ANSI.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${ANSI.red}⚠️ ${p.error}${ANSI.reset}`);
  } else {
    const maxLen = 20;
    lines.push(`${v(vc)}  ${ANSI.muted}Plan: ${planLabel}${ANSI.reset}`);

    let models = codexModelsFromQuota(p);
    models = applyCodexModelFilter(models, settings.models?.[p.provider]);

    if (models.length === 0) {
      lines.push(v(vc));
      lines.push(label("Available Models", vc));
      lines.push(`${v(vc)}  ${ANSI.comment}No models selected${ANSI.reset}`);
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
      const nameS = `${ANSI.textBright}${"Balance".padEnd(maxLen)}${ANSI.reset}`;
      const barS = bar(p.extraUsage.remaining);
      const pctS = `${getColor(p.extraUsage.remaining)}${formatPercent(p.extraUsage.remaining).padStart(4)}${ANSI.reset}`;
      const infoS =
        p.extraUsage.limit === -1
          ? `${ANSI.cyan}Unlimited${ANSI.reset}`
          : `${ANSI.cyan}Balance${ANSI.reset}`;
      lines.push(
        `${v(vc)}  ${indicator(p.extraUsage.remaining)} ${nameS} ${barS} ${pctS} ${infoS}`,
      );
    }
  }

  lines.push(v(vc));
  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);

  return lines;
}

function buildAmp(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = PROVIDER_ANSI.amp;
  const m = p.meta ?? {};

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}Amp${ANSI.reset} ${vc}${BOX.h.repeat(53)}${ANSI.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${ANSI.red}⚠️ ${p.error}${ANSI.reset}`);
  } else {
    // Thin tree connectors
    const tee = `${ANSI.comment}├─${ANSI.reset}`;
    const end = `${ANSI.comment}└─${ANSI.reset}`;

    // Free Tier
    const free = p.models?.["Free Tier"];
    if (free) {
      lines.push(label("Free Tier", vc));
      const barS = bar(free.remaining);
      const pctS = `${getColor(free.remaining)}${formatPercent(free.remaining).padStart(4)}${ANSI.reset}`;
      lines.push(`${v(vc)}  ${indicator(free.remaining)} ${barS} ${pctS}`);

      // Build sub-details
      const subs: string[] = [];

      const dollarParts: string[] = [];
      if (m.replenishRate)
        dollarParts.push(`${ANSI.cyan}${m.replenishRate}${ANSI.reset}`);
      const dollars = [m.freeRemaining, m.freeTotal]
        .filter(Boolean)
        .join(" / ");
      if (dollars) dollarParts.push(`${ANSI.text}( ${dollars} )${ANSI.reset}`);
      if (m.bonus) dollarParts.push(`${ANSI.cyan}${m.bonus}${ANSI.reset}`);
      if (dollarParts.length > 0) subs.push(dollarParts.join("  "));

      if (free.resetsAt && free.remaining !== 100) {
        subs.push(
          `${ANSI.cyan}Full in ${formatEta(free.resetsAt, free.remaining)}  ${formatResetTime(free.resetsAt, free.remaining)}${ANSI.reset}`,
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
      const balance = m.creditsBalance ?? "$0";
      const color = credits.remaining > 0 ? ANSI.green : ANSI.comment;
      lines.push(label("Credits", vc));
      lines.push(
        `${v(vc)}  ${indicator(credits.remaining)} ${color}${balance}${ANSI.reset}`,
      );
    }

    // Fallback for unknown models
    if (!free && !credits && p.models && Object.keys(p.models).length > 0) {
      const entries = Object.entries(p.models);
      const maxLen = Math.max(...entries.map(([name]) => name.length), 20);
      lines.push(label("Usage", vc));
      for (const [name, window] of entries) {
        const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
        const barS = bar(window.remaining);
        const pctS = `${getColor(window.remaining)}${formatPercent(window.remaining).padStart(4)}${ANSI.reset}`;
        lines.push(
          `${v(vc)}  ${indicator(window.remaining)} ${nameS} ${barS} ${pctS}`,
        );
      }
    }
  }

  if (p.account) {
    lines.push(v(vc));
    lines.push(`${v(vc)}  ${ANSI.comment}Account: ${p.account}${ANSI.reset}`);
  }

  lines.push(v(vc));
  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);

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
    return `${ANSI.comment}No providers connected${ANSI.reset}`;
  }

  return sections.map((s) => s.join("\n")).join("\n\n");
}

export function outputTerminal(quotas: AllQuotas): void {
  console.log(formatForTerminal(quotas));
}
